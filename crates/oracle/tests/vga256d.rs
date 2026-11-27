//! Differential tests for `vga256d.c` / `vga256d.h` (no mutable globals).

use proptest::prelude::*;
use rustyrian_core::Game;
use rustyrian_core::sdl::Surface;
use rustyrian_core::vga256d::{self as v, State};
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

/// Runs `c` on the C surface and `rust` on the Rust one; compares surfaces.
fn check(
    d: &mut Diff,
    c: impl FnOnce(*mut b::SDL_Surface),
    rust: impl FnOnce(&mut Game, ScreenId),
) {
    let id = ScreenId::VgaScreen2;
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
    /// Coordinates whose pixel lies inside the buffer, or that upstream's
    /// right/bottom clip rejects (C writes outside the buffer otherwise).
    #[test]
    fn je_pix_and_je_pix3_match(x in -2i32..330, y in 0i32..210, c in any::<u8>(), salt in any::<u32>()) {
        let mut d = setup(salt);
        let inside = |x: i32, y: i32| x >= 320 || y >= 200 || (0..64000).contains(&(y * 320 + x));
        prop_assume!(inside(x, y));
        // SAFETY: (each) C call on a valid surface with an in-bounds index.
        check(&mut d, |s| unsafe { b::JE_pix(s, x, y, c) }, |g, s| v::je_pix(g, s, x, y, c));
        let all = [(x, y), (x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)];
        if all.iter().all(|&(x, y)| inside(x, y)) {
            // SAFETY: as above.
            check(&mut d, |s| unsafe { b::JE_pix3(s, x, y, c) }, |g, s| v::je_pix3(g, s, x, y, c));
        }
    }

    /// Corners inside the surface (or rejected by the clip); a top-left
    /// corner past the right one by more than a pixel makes C's `memset`
    /// length negative, so `a <= c + 1`.
    #[test]
    fn je_rectangle_matches(a in 0i32..330, b_ in 0i32..210, c in 0i32..330, d_ in 0i32..210,
                            e in any::<i32>(), salt in any::<u32>()) {
        prop_assume!(a <= c + 1);
        let mut d = setup(salt);
        // SAFETY: C call on a valid surface with in-bounds writes.
        check(&mut d, |s| unsafe { b::JE_rectangle(s, a, b_, c, d_, e) }, |g, s| v::je_rectangle(g, s, a, b_, c, d_, e));
    }

    #[test]
    fn je_bar_shade_and_bright_match(a in 0i32..330, b_ in 0i32..210, c in 0i32..330, d_ in 0i32..210,
                                     salt in any::<u32>()) {
        let mut d = setup(salt);
        // SAFETY: (each) C call on a valid surface with in-bounds writes.
        check(&mut d, |s| unsafe { b::JE_barShade(s, a, b_, c, d_) }, |g, s| v::je_bar_shade(g, s, a, b_, c, d_));
        // Bright on fresh pixels (shaded ones never saturate).
        d.game.video.screens[ScreenId::VgaScreen2 as usize] = Some(pattern(salt ^ 1));
        // SAFETY: as above.
        check(&mut d, |s| unsafe { b::JE_barBright(s, a, b_, c, d_) }, |g, s| v::je_bar_bright(g, s, a, b_, c, d_));
    }

    #[test]
    fn fill_rectangles_match(x in -500i32..1000, y in -500i32..1000, x2 in -500i32..1000, y2 in -500i32..1000,
                             w in 0u32..1200, h in 0u32..1200, color in any::<u8>(), salt in any::<u32>()) {
        let mut d = setup(salt);
        // SAFETY: (each) C call on a valid surface; SDL clips.
        check(&mut d, |s| unsafe { b::fill_rectangle_xy(s, x, y, x2, y2, color) },
              |g, s| v::fill_rectangle_xy(g, s, x, y, x2, y2, color));
        // SAFETY: as above.
        check(&mut d, |s| unsafe { b::ot_i_vga256d_fill_rectangle_wh(s, x, y, w, h, color) },
              |g, s| v::fill_rectangle_wh(g, s, x, y, w, h, color));
    }

    #[test]
    fn draw_segmented_gauge_matches(x in -50i32..330, y in -20i32..210, color in any::<u8>(),
                                    sw in 1u32..20, sh in 1u32..20, sv in 1u32..50, value in 0u32..3000,
                                    salt in any::<u32>()) {
        let mut d = setup(salt);
        // SAFETY: C call on a valid surface; SDL clips.
        check(&mut d, |s| unsafe { b::draw_segmented_gauge(s, x, y, color, sw, sh, sv, value) },
              |g, s| v::draw_segmented_gauge(g, s, x, y, color, sw, sh, sv, value));
    }

    /// Rust only (C writes outside the pixel buffer for these,
    /// `D-UB-VGA-1`): any coordinates, never panic.
    #[test]
    fn primitives_never_panic(a in any::<i32>(), b_ in any::<i32>(), c in any::<i32>(), d_ in any::<i32>(), e in any::<u8>()) {
        let mut g = setup(0).game;
        let s = ScreenId::GameScreen;
        let (a, b_, c, d_) = (a % 100_000, b_ % 100_000, c % 100_000, d_ % 100_000);
        v::je_pix(&mut g, s, a, b_, e);
        v::je_pix3(&mut g, s, a, b_, e);
        v::je_rectangle(&mut g, s, a, b_, c, d_, i32::from(e));
        v::je_bar_shade(&mut g, s, a, b_, c, d_);
        v::je_bar_bright(&mut g, s, a, b_, c, d_);
    }
}

/// The edge cases the random tests rarely hit: a zero-length rectangle
/// side, corners on the last row and column, and an empty gauge.
#[test]
fn edge_cases_match() {
    let mut d = setup(7);
    for (a, b_, c, d_) in [
        (0, 0, 319, 199),
        (5, 5, 4, 9),
        (319, 199, 319, 199),
        (10, 20, 30, 10),
        (0, 199, 319, 0),
    ] {
        // SAFETY: (each) C call on a valid surface with in-bounds writes.
        check(
            &mut d,
            |s| unsafe { b::JE_rectangle(s, a, b_, c, d_, 0x1234) },
            |g, s| v::je_rectangle(g, s, a, b_, c, d_, 0x1234),
        );
        // SAFETY: as above.
        check(
            &mut d,
            |s| unsafe { b::JE_barShade(s, a, b_, c, d_) },
            |g, s| v::je_bar_shade(g, s, a, b_, c, d_),
        );
        // SAFETY: as above.
        check(
            &mut d,
            |s| unsafe { b::JE_barBright(s, a, b_, c, d_) },
            |g, s| v::je_bar_bright(g, s, a, b_, c, d_),
        );
    }
    // SAFETY: as above.
    check(
        &mut d,
        |s| unsafe { b::draw_segmented_gauge(s, 10, 10, 3, 4, 5, 10, 0) },
        |g, s| v::draw_segmented_gauge(g, s, 10, 10, 3, 4, 5, 10, 0),
    );
}
