//! Differential tests for `nortvars.c` / `nortvars.h` (no mutable globals).

use proptest::prelude::*;
use rustyrian_core::Game;
use rustyrian_core::nortvars::{self as n, State};
use rustyrian_core::sdl::Surface;
use rustyrian_core::video::{self, ScreenId};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;
use rustyrian_oracle::mirror::video::screen_to_c;

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let s = State::default();
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}

fn pattern(salt: u32) -> Surface {
    let mut surf = Surface::new_8bit(320, 200);
    for (i, p) in surf.pixels.iter_mut().enumerate() {
        *p = ((i as u32).wrapping_mul(2_654_435_761).wrapping_add(salt) >> 13) as u8;
    }
    surf
}

/// Both engines with the three surfaces of `init_video`, patterned.
fn setup(salt: u32) -> Diff {
    let mut d = Diff::new();
    let vs = &mut d.game.video;
    for (i, slot) in vs.screens.iter_mut().enumerate() {
        *slot = Some(pattern(salt.wrapping_add(i as u32)));
    }
    vs.vga_screen = Some(ScreenId::VgaScreenSeg);
    vs.vga_screen_seg = Some(ScreenId::VgaScreenSeg);
    vs.vga_screen2 = Some(ScreenId::VgaScreen2);
    vs.game_screen = Some(ScreenId::GameScreen);
    d.store_to_c();
    d
}

fn check(
    d: &mut Diff,
    c: impl FnOnce(*mut b::SDL_Surface),
    rust: impl FnOnce(&mut Game, ScreenId),
) {
    let id = ScreenId::GameScreen;
    d.game.video.store_to_c();
    c(screen_to_c(Some(id)));
    rust(&mut d.game, id);
    assert!(
        diff::load::<video::State>().screens == d.game.video.screens,
        "surfaces differ"
    );
    assert_eq!(d.c_calls(), d.rust_calls());
}

proptest! {
    /// Bars whose rectangles stay on the surface (C writes outside the
    /// buffer above the top row), and bars clipped at the right/bottom.
    #[test]
    fn je_d_bar3_matches(x in 0i16..330, num in 0i16..40, top in 0i16..120, col in any::<i16>(), salt in any::<u32>()) {
        let y = top + 2 * num + 1;
        let mut d = setup(salt);
        // SAFETY: C call on a valid surface with in-bounds writes.
        check(&mut d, |s| unsafe { b::JE_dBar3(s, x, y, num, col) }, |g, s| n::je_d_bar3(g, s, x, y, num, col));
    }

    #[test]
    fn je_bar_draw_shadow_matches(x in any::<u16>(), y in any::<u16>(), res in 1u16..30, col in any::<u16>(),
                                  amt in 0u16..1500, xsize in any::<u16>(), ysize in any::<u16>(), salt in any::<u32>()) {
        let (x, y) = (x % 400, y % 260);
        let (xsize, ysize) = (xsize % 40, ysize % 30);
        let mut d = setup(salt);
        // SAFETY: C call on a valid surface; every write is clipped.
        check(&mut d, |s| unsafe { b::JE_barDrawShadow(s, x, y, res, col, amt, xsize, ysize) },
              |g, s| n::je_bar_draw_shadow(g, s, x, y, res, col, amt, xsize, ysize));
    }
}

/// The in-game uses: the shield/armor bars and the gauge-like shadows.
#[test]
fn typical_calls_match() {
    let mut d = setup(3);
    // SAFETY: (each) C call on a valid surface with in-bounds writes.
    check(
        &mut d,
        |s| unsafe { b::JE_dBar3(s, 270, 60, 28, 144) },
        |g, s| n::je_d_bar3(g, s, 270, 60, 28, 144),
    );
    // SAFETY: as above.
    check(
        &mut d,
        |s| unsafe { b::JE_barDrawShadow(s, 42, 152, 3, 14, 13, 2, 10) },
        |g, s| n::je_bar_draw_shadow(g, s, 42, 152, 3, 14, 13, 2, 10),
    );
    // SAFETY: as above.
    check(
        &mut d,
        |s| unsafe { b::JE_barDrawShadow(s, 42, 152, 1, 14, 0, 0, 0) },
        |g, s| n::je_bar_draw_shadow(g, s, 42, 152, 1, 14, 0, 0, 0),
    );
}
