//! Differential tests for `backgrnd.c` / `backgrnd.h`.

use proptest::prelude::*;
use rustyrian_core::backgrnd::{self as bg, StarfieldStar, State};
use rustyrian_core::video::ScreenId;
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;

/// Mid-level fixture frames of `demo-cycle` (run `cargo xtask fixtures`).
const FRAMES: [u64; 2] = [500, 2000];

/// Fixture state with every screen's pixels scrambled by `seed`, so filters
/// see varied input, plus perturbations applied by `f`.
fn setup(frame: u64, seed: u64, f: impl FnOnce(&mut rustyrian_core::Game)) -> Diff {
    let mut d = Diff::new();
    d.game.restore(diff::fixture("demo-cycle", frame));
    let mut x = seed | 1;
    for s in d.game.video.screens.iter_mut().flatten() {
        if seed != 0 {
            for p in s.pixels.iter_mut().step_by(3) {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                *p = x as u8;
            }
        }
    }
    f(&mut d.game);
    d.store_to_c();
    d
}

/// Screens, background, config, starfield and RNG state agree.
fn check(d: &Diff) -> Result<(), TestCaseError> {
    prop_assert!(
        diff::load::<rustyrian_core::video::State>() == d.game.video,
        "surfaces differ"
    );
    prop_assert_eq!(diff::load::<State>(), d.game.backgrnd.clone());
    prop_assert_eq!(
        diff::load::<rustyrian_core::config::State>(),
        d.game.config.clone()
    );
    prop_assert_eq!(
        diff::load::<rustyrian_core::mtrand::State>(),
        d.game.mtrand.clone()
    );
    prop_assert!(d.c_calls().is_empty() && d.rust_calls().is_empty());
    Ok(())
}

fn vga(d: &Diff) -> ScreenId {
    d.game.video.vga_screen.expect("VGAScreen")
}

fn game_screen(d: &Diff) -> ScreenId {
    d.game.video.game_screen.expect("game_screen")
}

/// A map position: `(row, column offset)` with every drawn row inside a
/// `rows`-row map.
fn map_pos(rows: isize, width: isize) -> impl Strategy<Value = (isize, isize)> {
    (1..rows - 9, 12isize..=14).prop_map(move |(r, xbp)| (r * width, xbp))
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 48, ..ProptestConfig::default() })]

    #[test]
    fn draw_background_1_matches(
        frame in prop::sample::select(FRAMES.to_vec()),
        pos in map_pos(300, 14),
        x in 0u16..24,
        back_pos in 0u16..28,
        seed: u64,
    ) {
        let mut d = setup(frame, seed, |g| {
            (g.backgrnd.map_y_pos, g.backgrnd.map_xbp_pos) = (Some(pos.0), pos.1);
            g.backgrnd.map_x_pos = x;
            g.backgrnd.back_pos = back_pos;
            // Opaque rows: every tile present.
            for e in g.varz.mega_data1.mainmap.as_flattened_mut() {
                e.get_or_insert(0);
            }
        });
        // SAFETY: C globals set up by the fixture.
        unsafe { b::draw_background_1(b::VGAScreen) };
        let s = vga(&d);
        bg::draw_background_1(&mut d.game, s);
        check(&d)?;
    }

    #[test]
    fn draw_background_2_matches(
        frame in prop::sample::select(FRAMES.to_vec()),
        pos in map_pos(600, 14),
        x in (0u16..24, 0u16..24),
        xbp2 in 12isize..=14,
        // Biased towards a delay that expires and a position that wraps, so
        // both renderers scroll to the previous map row in most runs.
        back in (prop_oneof![24u16..28, 0u16..28], prop_oneof![1u16..4, 0u16..4]),
        delay in (prop_oneof![Just(1u8), 0u8..4], 0u8..4),
        flags: (bool, bool, bool),
        seed: u64,
    ) {
        let mut d = setup(frame, seed, |g| {
            let bk = &mut g.backgrnd;
            (bk.map_y2_pos, bk.map_xbp_pos) = (Some(pos.0), pos.1);
            bk.map_x2bp_pos = xbp2;
            (bk.map_x_pos, bk.map_x2_pos) = x;
            (bk.back_pos2, bk.back_move2) = back;
            (bk.map2_y_delay, bk.map2_y_delay_max) = delay;
            g.config.background2 = flags.0;
            g.config.smoothies[1] = flags.1;
        });
        if flags.2 {
            // SAFETY: as above.
            unsafe { b::draw_background_2(b::VGAScreen) };
            let s = vga(&d);
            bg::draw_background_2(&mut d.game, s);
        } else {
            // SAFETY: as above.
            unsafe { b::draw_background_2_blend(b::VGAScreen) };
            let s = vga(&d);
            bg::draw_background_2_blend(&mut d.game, s);
        }
        check(&d)?;
    }

    #[test]
    fn draw_background_3_matches(
        frame in prop::sample::select(FRAMES.to_vec()),
        pos in map_pos(600, 15),
        x in 0u16..24,
        back in (0u16..28, 0u16..4),
        seed: u64,
    ) {
        let mut d = setup(frame, seed, |g| {
            let bk = &mut g.backgrnd;
            (bk.map_y3_pos, bk.map_x3bp_pos) = (Some(pos.0 + 15), pos.1);
            bk.map_x3_pos = x;
            (bk.back_pos3, bk.back_move3) = back;
        });
        // SAFETY: as above.
        unsafe { b::draw_background_3(b::VGAScreen) };
        let s = vga(&d);
        bg::draw_background_3(&mut d.game, s);
        check(&d)?;
    }

    #[test]
    fn je_darken_background_matches(frame in prop::sample::select(FRAMES.to_vec()), neat: u16, seed: u64) {
        let mut d = setup(frame, seed, |_| {});
        // SAFETY: as above.
        unsafe { b::JE_darkenBackground(neat) };
        bg::je_darken_background(&mut d.game, neat);
        check(&d)?;
    }

    #[test]
    fn je_filter_screen_matches(
        frame in prop::sample::select(FRAMES.to_vec()),
        col in prop_oneof![Just(-99i8), any::<i8>()],
        brightness in prop_oneof![Just(-99i8), Just(0i8), -16i8..=31, any::<i8>()],
        fade: (bool, bool, bool, bool),
        level: (i8, i8, i8),
        seed: u64,
    ) {
        let mut d = setup(frame, seed, |g| {
            let c = &mut g.config;
            (c.filter_fade, c.filter_fade_start, c.filtration_avail, c.explosion_transparent) = fade;
            c.level_brightness = level.0 % 20;
            c.level_brightness_chg = level.1 % 3;
            c.level_filter_new = level.2;
        });
        // SAFETY: as above.
        unsafe { b::JE_filterScreen(col, brightness) };
        bg::je_filter_screen(&mut d.game, col, brightness);
        check(&d)?;
    }

    #[test]
    fn je_check_smoothies_matches(processor in 0u8..5, smoothies: [bool; 9]) {
        let mut d = Diff::new();
        d.game.config.processor_type = processor;
        d.game.config.smoothies = smoothies;
        d.store_to_c();
        // SAFETY: plain C call.
        unsafe { b::JE_checkSmoothies() };
        bg::je_check_smoothies(&mut d.game);
        prop_assert_eq!(diff::load::<State>(), d.game.backgrnd.clone());
    }

    #[test]
    fn filters_match(
        frame in prop::sample::select(FRAMES.to_vec()),
        which in 0usize..4,
        hue: u8,
        seed: u64,
    ) {
        let mut d = setup(frame, seed, |g| g.backgrnd.smoothie_data[1] = hue);
        let (dst, src) = (game_screen(&d), vga(&d));
        // SAFETY: as above; game_screen and VGAScreen are distinct surfaces.
        unsafe {
            match which {
                0 => b::lava_filter(b::game_screen, b::VGAScreen),
                1 => b::water_filter(b::game_screen, b::VGAScreen),
                2 => b::iced_blur_filter(b::game_screen, b::VGAScreen),
                _ => b::blur_filter(b::game_screen, b::VGAScreen),
            }
        }
        match which {
            0 => bg::lava_filter(&mut d.game, dst, src),
            1 => bg::water_filter(&mut d.game, dst, src),
            2 => bg::iced_blur_filter(&mut d.game, dst, src),
            _ => bg::blur_filter(&mut d.game, dst, src),
        }
        check(&d)?;
    }

    #[test]
    fn show_functions_match(
        frame in prop::sample::select(FRAMES.to_vec()),
        which in 0usize..3,
        // Upstream clamps player positions to these bounds (`mainint.c`).
        player_x in 40i16..=256,
        player_y in 10i16..=160,
        seed: u64,
    ) {
        let mut d = setup(frame, seed, |_| {});
        let (dst, src) = (ScreenId::VgaScreenSeg, game_screen(&d));
        // SAFETY: as above; VGAScreenSeg and game_screen are distinct.
        unsafe {
            match which {
                0 => b::showFlipped(b::VGAScreenSeg, b::game_screen),
                1 => b::showHeadlight(b::VGAScreenSeg, b::game_screen, player_x, player_y),
                _ => b::showNormal(b::VGAScreenSeg, b::game_screen),
            }
        }
        match which {
            0 => bg::show_flipped(&mut d.game, dst, src),
            1 => bg::show_headlight(&mut d.game, dst, src, player_x, player_y),
            _ => bg::show_normal(&mut d.game, dst, src),
        }
        check(&d)?;
    }

    /// The starfield draws (rule R7: its `mt_rand` draws in C's order), then
    /// moves and draws for several frames.
    #[test]
    fn starfield_matches(frame in prop::sample::select(FRAMES.to_vec()), speeds in prop::collection::vec(-3i32..8, 1..6), rng: u64, seed: u64) {
        let mut d = setup(frame, seed, |g| rustyrian_core::mtrand::mt_srand(g, rng));
        // SAFETY: as above.
        unsafe { b::initialize_starfield() };
        bg::initialize_starfield(&mut d.game);
        check(&d)?;
        for s in speeds {
            // SAFETY: as above.
            unsafe { b::update_and_draw_starfield(b::VGAScreen, s) };
            let v = vga(&d);
            bg::update_and_draw_starfield(&mut d.game, v, s);
            check(&d)?;
        }
    }
}

/// Every brightness and hue value through the two static filters.
#[test]
fn filter_screen_hue_and_value_match_for_every_argument() {
    let mut d = setup(500, 7, |_| {});
    let s = vga(&d);
    for v in i8::MIN..=i8::MAX {
        // SAFETY: as above.
        unsafe {
            b::ot_s_backgrnd_filterScreenValue(b::VGAScreen, v);
            b::ot_s_backgrnd_filterScreenHue(b::VGAScreen, v as u8);
        }
        bg::filter_screen_value(d.game.video.screen_mut(s), v);
        bg::filter_screen_hue(d.game.video.screen_mut(s), v as u8);
        assert!(
            diff::load::<rustyrian_core::video::State>() == d.game.video,
            "{v}"
        );
    }
}

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let mut s = State {
        back_pos: 1,
        back_pos2: 2,
        back_pos3: 3,
        back_move: 4,
        back_move2: 5,
        back_move3: 6,
        map_x: 7,
        map_y: 8,
        map_x2: 9,
        map_x3: 10,
        map_y2: 11,
        map_y3: 12,
        // One-before-begin, as `tyrian2.c:693-695` forms for row 0.
        map_y_pos: Some(-1),
        map_y2_pos: Some(14 * 599 + 13),
        map_y3_pos: Some(15 * 3 - 1),
        map_x_pos: 13,
        old_map_x_ofs: 14,
        map_x_ofs: 15,
        map_x2_ofs: 16,
        map_x2_pos: 17,
        map_x3_pos: 18,
        old_map_x3_ofs: 19,
        map_x3_ofs: 20,
        temp_map_x_ofs: 21,
        map_xbp_pos: -5,
        map_x2bp_pos: 1,
        map_x3bp_pos: -9,
        map1_y_delay: 22,
        map1_y_delay_max: 23,
        map2_y_delay: 24,
        map2_y_delay_max: 25,
        any_smoothies: true,
        smoothie_data: [1, 2, 3, 4, 5, 6, 7, 8, 9],
        starfield_speed: -2,
        ..State::default()
    };
    for (i, st) in s.starfield_stars.iter_mut().enumerate() {
        *st = StarfieldStar {
            color: 0x90 + i as u8 % 16,
            position: (i * 653) as u16,
            speed: i as i32 % 3 + 2,
        };
    }
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}
