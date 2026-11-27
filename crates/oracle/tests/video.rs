//! Differential tests for `video.c` / `video.h`.

use proptest::prelude::*;
use rustyrian_core::game::Halt;
use rustyrian_core::platform::{Rect, RendererId, TextureId, WindowId};
use rustyrian_core::sdl::{PIXELFORMAT_RGB888, Surface, alloc_format};
use rustyrian_core::video::{self, SCALE_ASPECT_4_3, ScalingMode, ScreenId, State};
use rustyrian_core::video_scale::{SCALERS_COUNT, ScalerFunction};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::guard_call;
use rustyrian_oracle::mirror::Mirror;
use rustyrian_oracle::mirror::video::screen_to_c;

mod platform_support;
use platform_support::{Both, fail_next};

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

fn sample() -> State {
    let mut s = State {
        fullscreen_display: -1,
        scaling_mode: SCALE_ASPECT_4_3,
        last_output_rect: Rect {
            x: 1,
            y: 2,
            w: 3,
            h: 4,
        },
        next_output_rect: Rect {
            x: -5,
            y: 6,
            w: 7,
            h: 8,
        },
        main_window: Some(WindowId(3)),
        main_window_renderer: Some(RendererId(4)),
        main_window_tex_format: alloc_format(PIXELFORMAT_RGB888),
        main_window_texture: Some(TextureId(5)),
        scaler_function: Some(ScalerFunction::Hq3x32),
        ..State::default()
    };
    for (i, slot) in s.screens.iter_mut().enumerate() {
        let mut surf = Surface::new_8bit(320, 200);
        for (j, p) in surf.pixels.iter_mut().enumerate() {
            *p = (j * 7 + i * 13) as u8;
        }
        surf.clip_rect = Rect {
            x: i as i32,
            y: 1,
            w: 100,
            h: 50,
        };
        *slot = Some(surf);
    }
    s.vga_screen = Some(ScreenId::GameScreen);
    s.vga_screen_seg = Some(ScreenId::VgaScreenSeg);
    s.vga_screen2 = Some(ScreenId::VgaScreen2);
    s.game_screen = Some(ScreenId::GameScreen);
    s
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let s = sample();
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);

    // Every scaler function survives the trip through C pointers.
    for f in [
        ScalerFunction::Nn16,
        ScalerFunction::Nn32,
        ScalerFunction::Scale2x16,
        ScalerFunction::Scale2x32,
        ScalerFunction::Scale3x16,
        ScalerFunction::Scale3x32,
        ScalerFunction::Hq2x32,
        ScalerFunction::Hq3x32,
        ScalerFunction::Hq4x32,
    ] {
        let t = State {
            scaler_function: Some(f),
            ..sample()
        };
        t.store_to_c();
        assert_eq!(diff::load::<State>(), t);
    }

    // And back to the initial state (surfaces freed, pointers NULL).
    State::default().store_to_c();
    assert_eq!(diff::load::<State>(), State::default());
}

#[test]
fn const_tables_match_c() {
    // SAFETY: C constant array of string literals.
    let names = unsafe { b::scaling_mode_names };
    for (r, &c) in video::SCALING_MODE_NAMES.iter().zip(names.iter()) {
        // SAFETY: NUL-terminated literal.
        assert_eq!(*r, unsafe { std::ffi::CStr::from_ptr(c) }.to_str().unwrap());
    }
}

/// Every SDL function the video code can see fail, plus "none".
const FAILURES: [&str; 9] = [
    "",
    "SDL_InitSubSystem",
    "SDL_CreateWindow",
    "SDL_CreateRenderer",
    "SDL_CreateTexture",
    "SDL_SetWindowFullscreen",
    "SDL_GetWindowDisplayIndex",
    "SDL_GetNumVideoDisplays",
    "SDL_GetDisplayBounds",
];

proptest! {
    #[test]
    fn init_video_matches_c(
        scaler in 0..SCALERS_COUNT,
        display in -2i32..3,
        failure in 0..FAILURES.len(),
    ) {
        let mut d = Both::new(&[]);
        d.game.video_scale.scaler = scaler;
        d.game.video.fullscreen_display = display;
        d.store();
        fail_next(&mut d, FAILURES[failure]);
        d.init_video();
        d.assert_same();
    }

    #[test]
    fn reinit_and_toggle_fullscreen_match_c(
        scaler in 0..SCALERS_COUNT,
        start in -1i32..2,
        new_display in -2i32..3,
        failure in 0..FAILURES.len(),
        toggle: bool,
    ) {
        let mut d = Both::new(&[]);
        d.game.video.fullscreen_display = start;
        d.store();
        prop_assert_eq!(d.init_video(), None);
        d.game.video_scale.scaler = scaler;
        d.store();
        d.assert_same();
        fail_next(&mut d, FAILURES[failure]);
        if toggle {
            // SAFETY: plain C call.
            unsafe { b::toggle_fullscreen() };
            video::toggle_fullscreen(&mut d.game);
        } else {
            // SAFETY: plain C call.
            unsafe { b::reinit_fullscreen(new_display) };
            video::reinit_fullscreen(&mut d.game, new_display);
        }
        d.assert_same();
    }

    #[test]
    fn window_center_in_display_matches_c(display in -2i32..3, w in 0i32..4000, h in 0i32..4000) {
        let mut d = Both::new(&[]);
        prop_assert_eq!(d.init_video(), None);
        set_window_size(&mut d, w, h);
        // SAFETY: plain C call.
        unsafe { b::ot_s_video_window_center_in_display(display) };
        video::window_center_in_display(&mut d.game, display);
        d.assert_same();
        // SAFETY: plain C call.
        let c = unsafe { b::ot_s_video_window_get_display_index() };
        prop_assert_eq!(c, video::window_get_display_index(&mut d.game));
        d.assert_same();
    }

    #[test]
    fn video_on_win_resize_matches_c(scaler in 0..SCALERS_COUNT, w in 0i32..2000, h in 0i32..2000) {
        let mut d = Both::new(&[]);
        prop_assert_eq!(d.init_video(), None);
        d.game.video_scale.scaler = scaler;
        d.store();
        set_window_size(&mut d, w, h);
        // SAFETY: plain C call.
        unsafe { b::video_on_win_resize() };
        video::video_on_win_resize(&mut d.game);
        d.assert_same();
    }

    #[test]
    fn init_scaler_matches_c(
        new_scaler in 0..SCALERS_COUNT,
        display in -1i32..1,
        failure in 0..FAILURES.len(),
        format in 0usize..3,
    ) {
        let mut d = Both::new(&[]);
        prop_assert_eq!(d.init_video(), None);
        d.game.video.fullscreen_display = display;
        // The texture format is always RGB888 upstream; 16-bit and 24-bit
        // formats exercise the `scaler16` and `default` branches.
        d.game.video.main_window_tex_format = match format {
            0 => alloc_format(rustyrian_core::sdl::PIXELFORMAT_RGB888),
            1 => alloc_format(rustyrian_core::sdl::PIXELFORMAT_RGB565),
            _ => Some(rustyrian_core::sdl::PixelFormat {
                format: 0x1710_1803, // SDL_PIXELFORMAT_RGB24
                bits_per_pixel: 24,
                bytes_per_pixel: 3,
            }),
        };
        d.store();
        fail_next(&mut d, FAILURES[failure]);
        let mut r = None;
        // SAFETY: plain C call.
        let c = unsafe { guard_call(|| r = Some(b::init_scaler(new_scaler))) };
        let rust = video::init_scaler(&mut d.game, new_scaler);
        match rust {
            Ok(v) => prop_assert_eq!((c, r), (None, Some(v))),
            Err(Halt { code }) => prop_assert_eq!((c, r), (Some(code), None)),
        }
        d.assert_same();
    }

    #[test]
    fn calc_dst_render_rect_matches_c(
        mode in 0u32..4,
        scaler in 0..SCALERS_COUNT,
        w in 0i32..4000,
        h in 0i32..4000,
    ) {
        let mut d = Both::new(&[]);
        d.game.video_scale.scaler = scaler;
        d.store();
        prop_assert_eq!(d.init_video(), None);
        d.game.video.scaling_mode = ScalingMode::from_c(mode).unwrap();
        d.store();
        set_window_size(&mut d, w, h);
        let src = ScreenId::ALL[(w % 3) as usize];
        let mut c_rect = b::SDL_Rect { x: 0, y: 0, w: 0, h: 0 };
        // SAFETY: plain C call with a live surface and rect.
        unsafe { b::ot_s_video_calc_dst_render_rect(screen_to_c(Some(src)), &raw mut c_rect) };
        let mut r_rect = Rect::default();
        video::calc_dst_render_rect(&mut d.game, src, &mut r_rect);
        prop_assert_eq!(
            (c_rect.x, c_rect.y, c_rect.w, c_rect.h),
            (r_rect.x, r_rect.y, r_rect.w, r_rect.h)
        );
        d.assert_same();
    }

    #[test]
    fn map_points_match_c(
        x: i32,
        y: i32,
        rx in -1000i32..1000,
        ry in -1000i32..1000,
        rw in 1i32..4000,
        rh in 1i32..4000,
    ) {
        let mut d = Both::new(&[]);
        prop_assert_eq!(d.init_video(), None);
        d.game.video.last_output_rect = Rect { x: rx, y: ry, w: rw, h: rh };
        d.store();
        type F = unsafe extern "C" fn(*mut i32, *mut i32);
        type R = fn(&mut rustyrian_core::Game, &mut i32, &mut i32);
        let fs: [(F, R); 3] = [
            (b::mapScreenPointToWindow, video::map_screen_point_to_window),
            (b::mapWindowPointToScreen, video::map_window_point_to_screen),
            (b::scaleWindowDistanceToScreen, video::scale_window_distance_to_screen),
        ];
        for (c, r) in fs {
            let (mut cx, mut cy) = (x, y);
            // SAFETY: plain C call.
            unsafe { c(&raw mut cx, &raw mut cy) };
            let (mut sx, mut sy) = (x, y);
            r(&mut d.game, &mut sx, &mut sy);
            prop_assert_eq!((cx, cy), (sx, sy));
        }
        d.assert_same();
    }

    #[test]
    fn je_clr256_matches_c(seed: u8, screen in 0usize..3) {
        let mut d = Both::new(&[]);
        prop_assert_eq!(d.init_video(), None);
        let id = ScreenId::ALL[screen];
        for (i, p) in d.game.video.screen_mut(id).pixels.iter_mut().enumerate() {
            *p = seed.wrapping_add(i as u8);
        }
        d.store();
        // SAFETY: plain C call with a live surface.
        unsafe { b::JE_clr256(screen_to_c(Some(id))) };
        video::je_clr256(&mut d.game, id);
        d.assert_same();
    }
}

fn set_window_size(d: &mut Both, w: i32, h: i32) {
    let win = d.game.video.main_window.expect("window");
    d.both(|p| p.set_window_size(win, w, h));
    d.clear_calls();
}

#[test]
fn set_scaling_mode_by_name_matches_c() {
    let mut d = Both::new(&[]);
    for name in [
        &b"Center"[..],
        b"Integer",
        b"Fit 8:5",
        b"Fit 4:3",
        b"",
        b"center",
        b"Fit 4:3 ",
        b"Fit",
    ] {
        d.game.video.scaling_mode = ScalingMode::ScaleAspect85;
        d.store();
        let n = diff::cstring(std::str::from_utf8(name).unwrap());
        // SAFETY: plain C call with a C string.
        let c = unsafe { b::set_scaling_mode_by_name(n.as_ptr()) };
        let r = video::set_scaling_mode_by_name(&mut d.game, name);
        assert_eq!(c, r);
        d.assert_same();
    }
}

#[test]
fn present_screen_and_deinit_video_match_c() {
    let mut d = Both::new(&[]);
    assert_eq!(d.init_video(), None);
    d.game.video.next_output_rect = Rect {
        x: 9,
        y: 8,
        w: 7,
        h: 6,
    };
    d.store();
    // SAFETY: plain C call.
    unsafe { b::present_screen() };
    video::present_screen(&mut d.game);
    d.assert_same();

    // SAFETY: plain C call.
    unsafe { b::deinit_video() };
    video::deinit_video(&mut d.game);
    assert_eq!(d.c_calls(), d.rust_calls());
    // C leaves the surface pointers dangling; check the rest by hand.
    // SAFETY: reading C statics on the test thread.
    unsafe {
        assert!(
            (*rustyrian_oracle::mirror::global(&raw const b::main_window_tex_format)).is_null()
        );
        assert!(
            (*rustyrian_oracle::mirror::video::sv::<*mut std::ffi::c_void>(
                b::ot_sv_video_main_window_renderer
            ))
            .is_null()
        );
        assert!(
            (*rustyrian_oracle::mirror::video::sv::<*mut std::ffi::c_void>(
                b::ot_sv_video_main_window_texture
            ))
            .is_null()
        );
    }
    assert_eq!(d.game.video.main_window_tex_format, None);
    assert_eq!(d.game.video.main_window_renderer, None);
    assert_eq!(d.game.video.main_window_texture, None);
    assert_eq!(d.game.video.screens, [None, None, None]);
}

#[test]
fn texture_and_renderer_helpers_match_c() {
    let mut d = Both::new(&[]);
    assert_eq!(d.init_video(), None);
    // SAFETY: plain C calls.
    unsafe {
        b::ot_s_video_deinit_texture();
        b::ot_s_video_deinit_renderer();
        // A second deinit sees NULL handles.
        b::ot_s_video_deinit_texture();
        b::ot_s_video_deinit_renderer();
    }
    video::deinit_texture(&mut d.game);
    video::deinit_renderer(&mut d.game);
    video::deinit_texture(&mut d.game);
    video::deinit_renderer(&mut d.game);
    d.assert_same();

    // Ends with a renderer, which init_texture needs.
    for failure in ["SDL_CreateRenderer", ""] {
        fail_next(&mut d, failure);
        // SAFETY: plain C call.
        let c = unsafe { guard_call(|| b::ot_s_video_init_renderer()) };
        let r = video::init_renderer(&mut d.game).err().map(|h| h.code);
        assert_eq!(c, r);
        d.assert_same();
    }
    for failure in ["", "SDL_CreateTexture"] {
        fail_next(&mut d, failure);
        // SAFETY: plain C call.
        let c = unsafe { guard_call(|| b::ot_s_video_init_texture()) };
        let r = video::init_texture(&mut d.game).err().map(|h| h.code);
        assert_eq!(c, r);
        d.assert_same();
    }
}

/// `reinit_fullscreen` falls back to windowed mode when entering desktop
/// fullscreen fails (the first, windowed `SDL_SetWindowFullscreen` fails too;
/// its result is ignored).
#[test]
fn reinit_fullscreen_falls_back_to_windowed() {
    for display in [0, 1] {
        let mut d = Both::new(&[]);
        assert_eq!(d.init_video(), None);
        d.clear_calls();
        fail_next(&mut d, "SDL_SetWindowFullscreen");
        fail_next(&mut d, "SDL_SetWindowFullscreen");
        // SAFETY: plain C call.
        unsafe { b::reinit_fullscreen(display) };
        video::reinit_fullscreen(&mut d.game, display);
        d.assert_same();
        assert_eq!(d.game.video.fullscreen_display, -1);
    }
}

/// `scale_and_render_screen` and `JE_showVGA` with every scaler and scaling
/// mode on every fixture frame; the scaled textures match too.
#[test]
fn scale_and_render_screen_matches_c_on_fixture_frames() {
    for frame in [60, 500, 2000] {
        let fx = diff::fixture("demo-cycle", frame);
        for scaler in 0..SCALERS_COUNT {
            let mut d = Both::new(&[]);
            d.game.video_scale.scaler = scaler;
            d.store();
            assert_eq!(d.init_video(), None);
            d.game.palette.rgb_palette = fx.palette.rgb_palette;
            for id in ScreenId::ALL {
                if let Some(s) = &fx.video.screens[id as usize] {
                    d.game
                        .video
                        .screen_mut(id)
                        .pixels
                        .copy_from_slice(&s.pixels);
                }
            }
            let src = fx.video.vga_screen.expect("VGAScreen");
            d.game.video.vga_screen = Some(src);
            d.game.video.scaling_mode = ScalingMode::from_c(scaler % 4).unwrap();
            d.store();
            set_window_size(&mut d, 1000 + 7 * scaler as i32, 700 + 3 * scaler as i32);
            let tex = d.game.video.main_window_texture.expect("texture");

            // SAFETY: plain C call on a live surface.
            unsafe { b::scale_and_render_screen(screen_to_c(Some(src))) };
            video::scale_and_render_screen(&mut d.game, src);
            d.assert_same();
            let pixels = d.both(|p| p.lock_texture(tex, None).map(|(px, _)| px.to_vec()));
            assert!(pixels.is_some(), "frame {frame}, scaler {scaler}");
            d.both(|p| p.unlock_texture(tex));
            d.clear_calls();

            // SAFETY: plain C call.
            unsafe { b::JE_showVGA() };
            video::je_show_vga(&mut d.game);
            d.assert_same();
        }
    }
}
