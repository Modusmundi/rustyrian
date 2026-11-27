// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `video.c` / `video.h`.
//!
//! The three 8-bit software surfaces that `init_video` creates live in
//! [`State::screens`], indexed by [`ScreenId`]. The C pointer globals
//! `VGAScreen`, `VGAScreenSeg`, `VGAScreen2` and `game_screen` become
//! `Option<ScreenId>` (rule R8): `None` is NULL (before `init_video`).

use crate::Game;
use crate::game::{EXIT_FAILURE, Flow, Halt};
use crate::logging::log_fatal;
use crate::platform::{FrameView, Rect, RendererId, TextureId, WindowId};
use crate::sdl::{
    PIXELFORMAT_RGB565, PIXELFORMAT_RGB888, PixelFormat, Surface, alloc_format, pixel_format_name,
};
use crate::video_scale::{SCALERS, ScalerFunction, call_scaler};
use serde::{Deserialize, Serialize};

/// `vga_width`.
pub const VGA_WIDTH: i32 = 320;
/// `vga_height`.
pub const VGA_HEIGHT: i32 = 200;

/// `ScalingMode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u32)]
pub enum ScalingMode {
    #[default]
    ScaleCenter = 0,
    ScaleInteger = 1,
    ScaleAspect85 = 2,
    ScaleAspect43 = 3,
}

impl ScalingMode {
    /// The C enumerator value.
    pub fn to_c(self) -> u32 {
        self as u32
    }

    /// The enumerator with C value `v`, if any.
    pub fn from_c(v: u32) -> Option<ScalingMode> {
        match v {
            0 => Some(ScalingMode::ScaleCenter),
            1 => Some(ScalingMode::ScaleInteger),
            2 => Some(ScalingMode::ScaleAspect85),
            3 => Some(ScalingMode::ScaleAspect43),
            _ => None,
        }
    }
}

/// `SCALE_CENTER`.
pub const SCALE_CENTER: ScalingMode = ScalingMode::ScaleCenter;
/// `SCALE_INTEGER`.
pub const SCALE_INTEGER: ScalingMode = ScalingMode::ScaleInteger;
/// `SCALE_ASPECT_8_5`.
pub const SCALE_ASPECT_8_5: ScalingMode = ScalingMode::ScaleAspect85;
/// `SCALE_ASPECT_4_3`.
pub const SCALE_ASPECT_4_3: ScalingMode = ScalingMode::ScaleAspect43;
/// `ScalingMode_MAX`.
pub const SCALING_MODE_MAX: usize = 4;

/// `scaling_mode_names`.
pub const SCALING_MODE_NAMES: [&str; SCALING_MODE_MAX] =
    ["Center", "Integer", "Fit 8:5", "Fit 4:3"];

/// One of the three `SDL_Surface`s upstream ever creates (`video.c:72-74`).
/// Every `SDL_Surface *` global or local in upstream points to one of these.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(usize)]
pub enum ScreenId {
    /// The surface `VGAScreenSeg` points to (the initial `VGAScreen`).
    VgaScreenSeg = 0,
    /// The surface `VGAScreen2` points to.
    VgaScreen2 = 1,
    /// The surface `game_screen` points to.
    GameScreen = 2,
}

impl ScreenId {
    /// All surfaces, in index order.
    pub const ALL: [ScreenId; 3] = [
        ScreenId::VgaScreenSeg,
        ScreenId::VgaScreen2,
        ScreenId::GameScreen,
    ];
}

/// Mutable globals of `video.c` (rule R3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// `fullscreen_display`; -1 means windowed.
    pub fullscreen_display: i32,
    pub scaling_mode: ScalingMode,
    pub last_output_rect: Rect,
    pub next_output_rect: Rect,

    /// The surfaces themselves, indexed by `ScreenId as usize`; `None` until
    /// `SDL_CreateRGBSurface` created it.
    pub screens: [Option<Surface>; 3],

    /// `VGAScreen`.
    pub vga_screen: Option<ScreenId>,
    /// `VGAScreenSeg`.
    pub vga_screen_seg: Option<ScreenId>,
    /// `VGAScreen2`.
    pub vga_screen2: Option<ScreenId>,
    /// `game_screen`.
    pub game_screen: Option<ScreenId>,

    pub main_window: Option<WindowId>,
    pub main_window_renderer: Option<RendererId>,
    /// `main_window_tex_format` (`SDL_AllocFormat` result).
    pub main_window_tex_format: Option<PixelFormat>,
    pub main_window_texture: Option<TextureId>,

    /// `scaler_function` (rule R8); `None` is NULL.
    pub scaler_function: Option<ScalerFunction>,
}

impl Default for State {
    fn default() -> Self {
        let full = Rect {
            x: 0,
            y: 0,
            w: VGA_WIDTH,
            h: VGA_HEIGHT,
        };
        State {
            fullscreen_display: 0,
            scaling_mode: SCALE_INTEGER,
            last_output_rect: full,
            next_output_rect: full,
            screens: [None, None, None],
            vga_screen: None,
            vga_screen_seg: None,
            vga_screen2: None,
            game_screen: None,
            main_window: None,
            main_window_renderer: None,
            main_window_tex_format: None,
            main_window_texture: None,
            scaler_function: None,
        }
    }
}

impl State {
    /// The surface `id`. Upstream dereferences these only after `init_video`
    /// (a NULL dereference in C), so a missing surface is a bug.
    pub fn screen(&self, id: ScreenId) -> &Surface {
        self.screens[id as usize]
            .as_ref()
            .expect("surface used before init_video")
    }

    /// Mutable [`State::screen`].
    pub fn screen_mut(&mut self, id: ScreenId) -> &mut Surface {
        self.screens[id as usize]
            .as_mut()
            .expect("surface used before init_video")
    }

    /// The surface `VGAScreen` points to.
    pub fn vga_screen_id(&self) -> ScreenId {
        self.vga_screen.expect("VGAScreen used before init_video")
    }

    /// `*VGAScreen`.
    pub fn vga(&self) -> &Surface {
        self.screen(self.vga_screen_id())
    }

    /// Mutable `*VGAScreen`.
    pub fn vga_mut(&mut self) -> &mut Surface {
        self.screen_mut(self.vga_screen_id())
    }

    /// Two distinct surfaces at once (source, destination), for copies
    /// between screens.
    pub fn screen_pair_mut(&mut self, a: ScreenId, b: ScreenId) -> (&mut Surface, &mut Surface) {
        assert_ne!(a, b, "screen_pair_mut needs two distinct surfaces");
        let [s0, s1, s2] = &mut self.screens;
        let mut slots = [Some(s0), Some(s1), Some(s2)];
        let sa = slots[a as usize].take().and_then(Option::as_mut);
        let sb = slots[b as usize].take().and_then(Option::as_mut);
        (
            sa.expect("surface used before init_video"),
            sb.expect("surface used before init_video"),
        )
    }
}

impl std::ops::Index<ScreenId> for State {
    type Output = Surface;
    fn index(&self, id: ScreenId) -> &Surface {
        self.screen(id)
    }
}

impl std::ops::IndexMut<ScreenId> for State {
    fn index_mut(&mut self, id: ScreenId) -> &mut Surface {
        self.screen_mut(id)
    }
}

/// `SDL_INIT_VIDEO`.
pub const SDL_INIT_VIDEO: u32 = 0x0000_0020;
/// `SDL_WINDOWPOS_CENTERED`.
pub const SDL_WINDOWPOS_CENTERED: i32 = 0x2FFF_0000;
/// `SDL_WINDOW_RESIZABLE`.
pub const SDL_WINDOW_RESIZABLE: u32 = 0x0000_0020;
/// `SDL_WINDOW_HIDDEN`.
pub const SDL_WINDOW_HIDDEN: u32 = 0x0000_0008;
/// `SDL_WINDOW_FULLSCREEN_DESKTOP`.
pub const SDL_WINDOW_FULLSCREEN_DESKTOP: u32 = 0x0000_1001;
/// `SDL_TEXTUREACCESS_STREAMING`.
pub const SDL_TEXTUREACCESS_STREAMING: i32 = 1;

// C passes the handle globals to SDL even while they are NULL; the fake
// platform's handle for NULL is id 0.
fn window(g: &Game) -> WindowId {
    g.video.main_window.unwrap_or(WindowId(0))
}

fn renderer(g: &Game) -> RendererId {
    g.video.main_window_renderer.unwrap_or(RendererId(0))
}

fn texture(g: &Game) -> TextureId {
    g.video.main_window_texture.unwrap_or(TextureId(0))
}

pub fn init_video(g: &mut Game) -> Flow {
    if g.platform.init_sub_system(SDL_INIT_VIDEO) != 0 {
        let e = g.platform.get_error();
        log_fatal(g, &format!("Failed to initialize SDL video: {e}"));
        return Err(Halt { code: EXIT_FAILURE });
    }

    // Create the software surfaces that the game renders to. These are all 320x200x8 regardless
    // of the window size or monitor resolution.
    g.video.screens = [
        Some(Surface::new_8bit(VGA_WIDTH, VGA_HEIGHT)),
        Some(Surface::new_8bit(VGA_WIDTH, VGA_HEIGHT)),
        Some(Surface::new_8bit(VGA_WIDTH, VGA_HEIGHT)),
    ];
    g.video.vga_screen = Some(ScreenId::VgaScreenSeg);
    g.video.vga_screen_seg = Some(ScreenId::VgaScreenSeg);
    g.video.vga_screen2 = Some(ScreenId::VgaScreen2);
    g.video.game_screen = Some(ScreenId::GameScreen);

    je_clr256(g, g.video.vga_screen_id());

    // Create the window with a temporary initial size, hidden until we set up the
    // scaler and find the true window size
    g.video.main_window = g.platform.create_window(
        "OpenTyrian",
        SDL_WINDOWPOS_CENTERED,
        SDL_WINDOWPOS_CENTERED,
        VGA_WIDTH,
        VGA_HEIGHT,
        SDL_WINDOW_RESIZABLE | SDL_WINDOW_HIDDEN,
    );

    if g.video.main_window.is_none() {
        let e = g.platform.get_error();
        log_fatal(g, &format!("Failed to create window: {e}"));
        return Err(Halt { code: EXIT_FAILURE });
    }

    reinit_fullscreen(g, g.video.fullscreen_display);
    init_renderer(g)?;
    init_texture(g)?;
    init_scaler(g, g.video_scale.scaler)?;

    g.platform.show_window(window(g));

    let r = renderer(g);
    g.platform.set_render_draw_color(r, 0, 0, 0, 255);
    g.platform.render_clear(r);
    present_frame(g, r);
    Ok(())
}

pub fn deinit_video(g: &mut Game) {
    deinit_texture(g);
    deinit_renderer(g);

    g.platform.destroy_window(window(g));

    // SDL_FreeSurface; C leaves the pointer globals dangling.
    g.video.screens = [None, None, None];

    g.platform.quit_sub_system(SDL_INIT_VIDEO);
}

pub fn init_renderer(g: &mut Game) -> Flow {
    g.video.main_window_renderer = g.platform.create_renderer(window(g), -1, 0);

    if g.video.main_window_renderer.is_none() {
        let e = g.platform.get_error();
        log_fatal(g, &format!("Failed to create renderer: {e}"));
        return Err(Halt { code: EXIT_FAILURE });
    }
    Ok(())
}

pub fn deinit_renderer(g: &mut Game) {
    if let Some(r) = g.video.main_window_renderer {
        g.platform.destroy_renderer(r);
        g.video.main_window_renderer = None;
    }
}

pub fn init_texture(g: &mut Game) -> Flow {
    debug_assert!(g.video.main_window_renderer.is_some());

    let bpp = 32; // TODOSDL2
    let format = if bpp == 32 {
        PIXELFORMAT_RGB888
    } else {
        PIXELFORMAT_RGB565
    };
    let scaler = &SCALERS[g.video_scale.scaler as usize];
    let (scaler_w, scaler_h) = (scaler.width, scaler.height);

    g.video.main_window_tex_format = alloc_format(format);

    g.video.main_window_texture = g.platform.create_texture(
        renderer(g),
        format,
        SDL_TEXTUREACCESS_STREAMING,
        scaler_w,
        scaler_h,
    );

    if g.video.main_window_texture.is_none() {
        let e = g.platform.get_error();
        log_fatal(
            g,
            &format!(
                "Failed to create scaler texture ({scaler_w}x{scaler_h}x{}): {e}",
                pixel_format_name(format)
            ),
        );
        return Err(Halt { code: EXIT_FAILURE });
    }
    Ok(())
}

pub fn deinit_texture(g: &mut Game) {
    if let Some(t) = g.video.main_window_texture {
        g.platform.destroy_texture(t);
        g.video.main_window_texture = None;
    }

    // SDL_FreeFormat
    g.video.main_window_tex_format = None;
}

pub fn window_get_display_index(g: &mut Game) -> i32 {
    g.platform.get_window_display_index(window(g))
}

pub fn window_center_in_display(g: &mut Game, display_index: i32) {
    let (mut win_w, mut win_h) = (0, 0);
    let w = window(g);
    g.platform
        .get_window_size(w, Some(&mut win_w), Some(&mut win_h));

    // C leaves `bounds` uninitialised when SDL_GetDisplayBounds fails; the
    // oracle zero-initialises it (D-UB-VIDEO-BOUNDS).
    let mut bounds = Rect::default();
    g.platform.get_display_bounds(display_index, &mut bounds);

    g.platform.set_window_position(
        w,
        bounds.x.wrapping_add(bounds.w.wrapping_sub(win_w) / 2),
        bounds.y.wrapping_add(bounds.h.wrapping_sub(win_h) / 2),
    );
}

pub fn reinit_fullscreen(g: &mut Game, new_display: i32) {
    g.video.fullscreen_display = new_display;

    if g.video.fullscreen_display >= g.platform.get_num_video_displays() {
        g.video.fullscreen_display = 0;
    }

    let w = window(g);
    g.platform.set_window_fullscreen(w, 0);
    let scaler = &SCALERS[g.video_scale.scaler as usize];
    g.platform.set_window_size(w, scaler.width, scaler.height);

    if g.video.fullscreen_display == -1 {
        let i = window_get_display_index(g);
        window_center_in_display(g, i);
    } else {
        window_center_in_display(g, g.video.fullscreen_display);

        if g.platform
            .set_window_fullscreen(w, SDL_WINDOW_FULLSCREEN_DESKTOP)
            != 0
        {
            reinit_fullscreen(g, -1);
        }
    }
}

pub fn video_on_win_resize(g: &mut Game) {
    let (mut w, mut h) = (0, 0);

    // Tell video to reinit if the window was manually resized by the user.
    // Also enforce a minimum size on the window.

    let win = window(g);
    g.platform.get_window_size(win, Some(&mut w), Some(&mut h));
    let scaler = &SCALERS[g.video_scale.scaler as usize];
    let (scaler_w, scaler_h) = (scaler.width, scaler.height);

    if w < scaler_w || h < scaler_h {
        w = if w < scaler_w { scaler_w } else { w };
        h = if h < scaler_h { scaler_h } else { h };

        g.platform.set_window_size(win, w, h);
    }
}

pub fn toggle_fullscreen(g: &mut Game) {
    if g.video.fullscreen_display != -1 {
        reinit_fullscreen(g, -1);
    } else {
        let i = g.platform.get_window_display_index(window(g));
        reinit_fullscreen(g, i);
    }
}

/// `init_scaler`: `Err` only where `init_texture` exits.
pub fn init_scaler(g: &mut Game, new_scaler: u32) -> Flow<bool> {
    let s = &SCALERS[new_scaler as usize];
    let (w, h) = (s.width, s.height);
    let bpp = g
        .video
        .main_window_tex_format
        .as_ref()
        .expect("init_scaler before init_texture")
        .bits_per_pixel; // TODOSDL2

    g.video_scale.scaler = new_scaler;

    deinit_texture(g);
    init_texture(g)?;

    if g.video.fullscreen_display == -1 {
        // Changing scalers, when not in fullscreen mode, forces the window
        // to resize to exactly match the scaler's output dimensions.
        g.platform.set_window_size(window(g), w, h);
        let i = window_get_display_index(g);
        window_center_in_display(g, i);
    }

    let s = &SCALERS[g.video_scale.scaler as usize];
    g.video.scaler_function = match bpp {
        32 => s.scaler32,
        16 => s.scaler16,
        _ => None,
    };

    if g.video.scaler_function.is_none() {
        return Ok(false);
    }

    Ok(true)
}

/// `set_scaling_mode_by_name`; `name` is a C string.
pub fn set_scaling_mode_by_name(g: &mut Game, name: &[u8]) -> bool {
    let n = name.iter().position(|&b| b == 0).unwrap_or(name.len());
    for (i, m) in SCALING_MODE_NAMES.iter().enumerate() {
        if &name[..n] == m.as_bytes() {
            g.video.scaling_mode =
                ScalingMode::from_c(i as u32).expect("index below ScalingMode_MAX");
            return true;
        }
    }
    false
}

#[doc(alias = "JE_clr256")]
pub fn je_clr256(g: &mut Game, screen: ScreenId) {
    g.video.screen_mut(screen).fill_rect(None, 0);
}

/// `calc_dst_render_rect`: decides how the logical output texture (after
/// software scaling applied) will fit in the window.
pub fn calc_dst_render_rect(g: &mut Game, src_surface: ScreenId, dst_rect: &mut Rect) {
    let (mut win_w, mut win_h) = (0, 0);
    g.platform
        .get_window_size(window(g), Some(&mut win_w), Some(&mut win_h));

    let (src_w, src_h) = {
        let s = g.video.screen(src_surface);
        (s.w, s.h)
    };

    match g.video.scaling_mode {
        ScalingMode::ScaleCenter => {
            let t = texture(g);
            g.platform
                .query_texture(t, None, None, Some(&mut dst_rect.w), Some(&mut dst_rect.h));
        }
        ScalingMode::ScaleInteger => {
            dst_rect.w = src_w;
            dst_rect.h = src_h;
            while dst_rect.w.wrapping_add(src_w) <= win_w && dst_rect.h.wrapping_add(src_h) <= win_h
            {
                dst_rect.w = dst_rect.w.wrapping_add(src_w);
                dst_rect.h = dst_rect.h.wrapping_add(src_h);
            }
        }
        ScalingMode::ScaleAspect85 => {
            let maxh_width = (win_h as f32 * (8.0f32 / 5.0f32)) as i32;
            let maxw_height = (win_w as f32 * (5.0f32 / 8.0f32)) as i32;

            if maxh_width > win_w {
                dst_rect.w = win_w;
                dst_rect.h = maxw_height;
            } else {
                dst_rect.w = maxh_width;
                dst_rect.h = win_h;
            }
        }
        ScalingMode::ScaleAspect43 => {
            let maxh_width = (win_h as f32 * (4.0f32 / 3.0f32)) as i32;
            let maxw_height = (win_w as f32 * (3.0f32 / 4.0f32)) as i32;

            if maxh_width > win_w {
                dst_rect.w = win_w;
                dst_rect.h = maxw_height;
            } else {
                dst_rect.w = maxh_width;
                dst_rect.h = win_h;
            }
        }
    }

    dst_rect.x = win_w.wrapping_sub(dst_rect.w) / 2;
    dst_rect.y = win_h.wrapping_sub(dst_rect.h) / 2;
}

/// Scales a surface and copies it to the renderer without presenting it.
pub fn scale_and_render_screen(g: &mut Game, src_surface: ScreenId) {
    // Do software scaling. C calls through a NULL `scaler_function` here
    // only if `init_scaler` failed, which upstream then reverts.
    let f = g
        .video
        .scaler_function
        .expect("scale_and_render_screen before a successful init_scaler");
    let t = texture(g);
    call_scaler(g, f, src_surface, t);

    // C's caller-uninitialised `dst_rect` (zero in the oracle) keeps its w/h
    // when SDL_QueryTexture fails in SCALE_CENTER mode.
    let mut dst_rect = Rect::default();
    calc_dst_render_rect(g, src_surface, &mut dst_rect);

    // Clear the window and blit the output texture to it
    let r = renderer(g);
    g.platform.set_render_draw_color(r, 0, 0, 0, 255);
    g.platform.render_clear(r);
    g.platform.render_copy(r, t, None, Some(dst_rect));
    g.platform.render_flush(r);

    g.video.next_output_rect = dst_rect;
}

#[doc(alias = "JE_showVGA")]
pub fn je_show_vga(g: &mut Game) {
    let screen = g.video.vga_screen_id();
    scale_and_render_screen(g, screen);
    present_screen(g);
}

/// `SDL_RenderPresent` with the instrumentation view of the current frame.
fn present_frame(g: &mut Game, r: RendererId) {
    let (platform, states) = g.split();
    let snapshot =
        || postcard::to_stdvec(&states.snapshot()).expect("a snapshot always serialises");
    let view = FrameView {
        screen: &states.video.vga().pixels,
        palette: &states.palette.rgb_palette,
        sim: None,
        snapshot: Some(&snapshot),
    };
    platform.render_present(r, &view);
}

/// Presents what scale_and_render_screen() copied to the renderer.
pub fn present_screen(g: &mut Game) {
    let r = renderer(g);
    present_frame(g, r);

    // Save output rect to be used by mouse functions
    g.video.last_output_rect = g.video.next_output_rect;
}

/// Maps a specified point in game screen coordinates to window coordinates.
#[doc(alias = "mapScreenPointToWindow")]
pub fn map_screen_point_to_window(g: &mut Game, inout_x: &mut i32, inout_y: &mut i32) {
    let r = g.video.last_output_rect;
    let (vw, vh) = (g.video.vga().w, g.video.vga().h);
    *inout_x = (2i32.wrapping_mul(*inout_x).wrapping_add(1))
        .wrapping_mul(r.w)
        .wrapping_div(2i32.wrapping_mul(vw))
        .wrapping_add(r.x);
    *inout_y = (2i32.wrapping_mul(*inout_y).wrapping_add(1))
        .wrapping_mul(r.h)
        .wrapping_div(2i32.wrapping_mul(vh))
        .wrapping_add(r.y);
}

/// Maps a specified point in window coordinates to game screen coordinates.
#[doc(alias = "mapWindowPointToScreen")]
pub fn map_window_point_to_screen(g: &mut Game, inout_x: &mut i32, inout_y: &mut i32) {
    let r = g.video.last_output_rect;
    let (vw, vh) = (g.video.vga().w, g.video.vga().h);
    *inout_x = (2i32.wrapping_mul(inout_x.wrapping_sub(r.x)).wrapping_add(1))
        .wrapping_mul(vw)
        .wrapping_div(2i32.wrapping_mul(r.w));
    *inout_y = (2i32.wrapping_mul(inout_y.wrapping_sub(r.y)).wrapping_add(1))
        .wrapping_mul(vh)
        .wrapping_div(2i32.wrapping_mul(r.h));
}

/// Scales a distance in window coordinates to game screen coordinates.
#[doc(alias = "scaleWindowDistanceToScreen")]
pub fn scale_window_distance_to_screen(g: &mut Game, inout_x: &mut i32, inout_y: &mut i32) {
    let r = g.video.last_output_rect;
    let (vw, vh) = (g.video.vga().w, g.video.vga().h);
    *inout_x = (2i32.wrapping_mul(*inout_x).wrapping_add(1))
        .wrapping_mul(vw)
        .wrapping_div(2i32.wrapping_mul(r.w));
    *inout_y = (2i32.wrapping_mul(*inout_y).wrapping_add(1))
        .wrapping_mul(vh)
        .wrapping_div(2i32.wrapping_mul(r.h));
}
