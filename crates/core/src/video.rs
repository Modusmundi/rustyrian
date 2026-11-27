// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `video.c` / `video.h`.
//!
//! The three 8-bit software surfaces that `init_video` creates live in
//! [`State::screens`], indexed by [`ScreenId`]. The C pointer globals
//! `VGAScreen`, `VGAScreenSeg`, `VGAScreen2` and `game_screen` become
//! `Option<ScreenId>` (rule R8): `None` is NULL (before `init_video`).

use crate::platform::{Rect, RendererId, TextureId, WindowId};
use crate::sdl::{PixelFormat, Surface};
use crate::video_scale::ScalerFunction;
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
