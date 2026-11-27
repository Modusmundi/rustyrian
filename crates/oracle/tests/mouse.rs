//! Differential tests for `mouse.c` / `mouse.h`.

use rustyrian_core::mouse::State;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let mut s = State {
        has_mouse: false,
        mouse_has_three_buttons: false,
        mouse_inactive: false,
        mouse_cursor: 3,
        mouse_grab_x: 301,
        mouse_grab_y: 187,
        ..State::default()
    };
    for (i, p) in s.mouse_grab_shape.iter_mut().enumerate() {
        *p = (i * 31) as u8;
    }
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}

mod platform_support;
use platform_support::Both;
use proptest::prelude::*;
use rustyrian_core::mouse::{self as ms, MOUSE_GRAB_SHAPE_LEN};
use rustyrian_oracle::bindings as b;

fn fill(d: &mut Both, seed: u8) {
    for id in rustyrian_core::video::ScreenId::ALL {
        for (i, p) in d.game.video.screen_mut(id).pixels.iter_mut().enumerate() {
            *p = seed.wrapping_add((i * 7 + i / 320) as u8);
        }
    }
}

proptest! {
    #[test]
    fn shape_grab_and_draw_match_c(
        x in prop_oneof![0u16..320, any::<u16>()],
        y in prop_oneof![0u16..200, 180u16..240, any::<u16>()],
        seed: u8,
        shape in prop::collection::vec(any::<u8>(), MOUSE_GRAB_SHAPE_LEN),
    ) {
        let mut d = Both::new(&[]);
        prop_assert_eq!(d.init_video(), None);
        fill(&mut d, seed);
        d.game.mouse.mouse_grab_shape.copy_from_slice(&shape);
        d.store();
        d.clear_calls();
        let ptr = || {
            // SAFETY: address of the C static.
            unsafe { b::ot_sv_mouse_mouseGrabShape() }.cast::<u8>()
        };

        // SAFETY: plain C call on the live VGAScreen and C's grab buffer.
        unsafe { b::ot_s_mouse_JE_grabShapeTypeOne(x, y, ptr()) };
        {
            let g = &mut *d.game;
            ms::je_grab_shape_type_one(g.video.vga(), x, y, &mut g.mouse.mouse_grab_shape);
        }
        d.assert_same();

        d.game.mouse.mouse_grab_shape.copy_from_slice(&shape);
        d.store();
        // SAFETY: as above.
        unsafe { b::ot_s_mouse_JE_drawShapeTypeOne(x, y, ptr()) };
        {
            let g = &mut *d.game;
            let shape = g.mouse.mouse_grab_shape;
            ms::je_draw_shape_type_one(g.video.vga_mut(), x, y, &shape);
        }
        d.assert_same();
    }

    #[test]
    fn mouse_replace_matches_c(
        has_mouse: bool,
        x in prop_oneof![0u16..320, any::<u16>()],
        y in prop_oneof![0u16..200, any::<u16>()],
        seed: u8,
        shape in prop::collection::vec(any::<u8>(), MOUSE_GRAB_SHAPE_LEN),
    ) {
        let mut d = Both::new(&[]);
        prop_assert_eq!(d.init_video(), None);
        fill(&mut d, seed);
        d.game.mouse.has_mouse = has_mouse;
        d.game.mouse.mouse_grab_x = x;
        d.game.mouse.mouse_grab_y = y;
        d.game.mouse.mouse_grab_shape.copy_from_slice(&shape);
        d.store();
        d.clear_calls();
        // SAFETY: plain C call.
        unsafe { b::JE_mouseReplace() };
        ms::je_mouse_replace(&mut d.game);
        d.assert_same();
    }
}

/// Two engines with video up and the shop sprite sheet (`newsh1.shp`)
/// loaded by C and mirrored into Rust.
fn with_shop_sheet() -> Both {
    let mut d = Both::new(&[]);
    assert_eq!(d.init_video(), None);
    {
        use std::os::unix::ffi::OsStrExt;
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data")
            .canonicalize()
            .expect("data/ (run `cargo xtask fetch-data`)");
        d.game.file.custom_data_dir_path = Some(data.as_os_str().as_bytes().to_vec());
    }
    d.store();
    // SAFETY: a valid sheet global; exit is caught by the guard.
    let exit = unsafe {
        rustyrian_oracle::guard_call(|| {
            b::JE_loadCompShapes(&raw mut b::shopSpriteSheet, b'1' as _)
        })
    };
    assert_eq!(exit, None);
    let c = d.c_state();
    d.game.restore(c);
    d.clear_calls();
    d
}

proptest! {
    #[test]
    fn mouse_start_matches_c(
        has_mouse: bool,
        inactive: bool,
        cursor in 0u8..5,
        mouse_x in prop_oneof![-40i32..360, any::<i32>()],
        mouse_y in prop_oneof![-40i32..240, any::<i32>()],
        filter: Option<u8>,
        seed: u8,
    ) {
        let mut d = with_shop_sheet();
        fill(&mut d, seed);
        d.game.mouse.has_mouse = has_mouse;
        d.game.mouse.mouse_inactive = inactive;
        d.game.mouse.mouse_cursor = cursor;
        d.game.keyboard.mouse_x = mouse_x;
        d.game.keyboard.mouse_y = mouse_y;
        d.store();
        // SAFETY: plain C calls.
        unsafe {
            match filter {
                None => b::JE_mouseStart(),
                Some(f) => b::JE_mouseStartFilter(f),
            }
        }
        match filter {
            None => ms::je_mouse_start(&mut d.game),
            Some(f) => ms::je_mouse_start_filter(&mut d.game, f),
        }
        d.assert_same();
    }
}

#[test]
fn mouse_pointer_sprites_match_c() {
    // `mousePointerSprites` is a C file-scope constant without a getter; its
    // values show through JE_mouseStart: the grab position clamps to the
    // pointer's width/height and focus point.
    for cursor in 0u8..5 {
        for (mx, my) in [(-100, -100), (1000, 1000)] {
            let mut d = with_shop_sheet();
            d.game.mouse.mouse_cursor = cursor;
            d.game.mouse.mouse_inactive = false;
            d.game.keyboard.mouse_x = mx;
            d.game.keyboard.mouse_y = my;
            d.store();
            // SAFETY: plain C call.
            unsafe { b::JE_mouseStart() };
            ms::je_mouse_start(&mut d.game);
            d.assert_same();
        }
    }
}
