//! Differential tests for `video.c` / `video.h`.

use rustyrian_core::platform::{Rect, RendererId, TextureId, WindowId};
use rustyrian_core::sdl::{PIXELFORMAT_RGB888, Surface, alloc_format};
use rustyrian_core::video::{SCALE_ASPECT_4_3, ScreenId, State};
use rustyrian_core::video_scale::ScalerFunction;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;

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
    let names = unsafe { rustyrian_oracle::bindings::scaling_mode_names };
    for (r, &c) in rustyrian_core::video::SCALING_MODE_NAMES
        .iter()
        .zip(names.iter())
    {
        // SAFETY: NUL-terminated literal.
        assert_eq!(*r, unsafe { std::ffi::CStr::from_ptr(c) }.to_str().unwrap());
    }
}
