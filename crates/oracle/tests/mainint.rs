//! Differential tests for `mainint.c` / `mainint.h`.

use rustyrian_core::mainint::State;
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
    let s = State {
        button: [true, false, true, true],
        constant_last_x: -5,
        text_erase: 100,
        upgrade_cost: 2000,
        downgrade_cost: 1500,
        perform_save: true,
        jump_section: true,
        use_last_bank: true,
        pause_pressed: true,
        ingamemenu_pressed: true,
    };
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}

mod mainint_support;

use mainint_support::Rig;
use proptest::prelude::*;
use proptest::test_runner::{Config as RunnerConfig, TestRunner};
use rustyrian_core::mainint as mi;
use rustyrian_core::palette::{Color, Palette};
use rustyrian_oracle::bindings as b;
use std::cell::RefCell;
use std::ffi::c_int;

/// Equal platform calls, then equal C and Rust states of the named modules
/// (only what the test touches; loading the whole game per case is slow).
macro_rules! same {
    ($rig:expr; $($m:ident),* $(,)?) => {{
        assert_eq!($rig.c_calls(), $rig.rust_calls(), "platform calls differ");
        $(prop_assert_eq!(
            &rustyrian_oracle::diff::load::<rustyrian_core::$m::State>(),
            &$rig.g.$m,
            "{}",
            stringify!($m)
        );)*
    }};
}

/// Runs `test` for each generated case on one fixture rig (each case stores
/// the whole game into both engines first).
///
/// Hard cap of 16 cases (not scaled by `PROPTEST_CASES`): every case stores
/// the whole game into C and loads the touched modules back, about 0.1 s a
/// case.
fn run_on_rig<S: Strategy>(
    strategy: S,
    frame: u64,
    test: impl Fn(&mut Rig, S::Value) -> Result<(), TestCaseError>,
) where
    S::Value: std::fmt::Debug,
{
    let rig = RefCell::new(Rig::from_fixture("ots 1\n", frame));
    let base = rig.borrow().g.snapshot();
    let config = RunnerConfig {
        cases: 16,
        ..RunnerConfig::default()
    };
    // Deterministic draws: with only 16 cases, a fixed seed keeps these
    // functions' C line coverage reproducible across runs and machines.
    let rng = proptest::test_runner::TestRng::deterministic_rng(config.rng_algorithm);
    let mut runner = TestRunner::new_with_rng(config, rng);
    runner
        .run(&strategy, |v| {
            let mut rig = rig.borrow_mut();
            rig.g.restore(base.clone());
            rig.store();
            rig.clear();
            test(&mut rig, v)
        })
        .unwrap();
}

#[test]
fn fixture_rig_round_trips() {
    let mut rig = Rig::from_fixture("ots 1\n", 500);
    rig.check();
}

proptest! {
    #[test]
    // Upstream asserts `power <= 11` (`mainint.c:574`); every caller stays in it.
    fn weapon_upgrade_cost_matches_c(base in prop_oneof![0i64..100_000, any::<i64>()], power in 0u32..=11) {
        // SAFETY: pure C function.
        let c = unsafe { b::weapon_upgrade_cost(base, power) };
        prop_assert_eq!(c, mi::weapon_upgrade_cost(base, power));
    }

    #[test]
    fn gamma_correct_matches_c(palette in prop::collection::vec(any::<(u8, u8, u8)>(), 256), gamma: u8) {
        let mut r: Palette = [Color::default(); 256];
        for (c, &(x, y, z)) in r.iter_mut().zip(&palette) {
            *c = Color { r: x, g: y, b: z, a: 0 };
        }
        // SAFETY: `Palette` is plain data; all-zero is valid.
        let mut c: b::Palette = unsafe { std::mem::zeroed() };
        for (d, s) in c.iter_mut().zip(&r) {
            *d = b::SDL_Color { r: s.r, g: s.g, b: s.b, a: s.a };
        }
        // SAFETY: a valid palette.
        unsafe { b::JE_gammaCorrect(&raw mut c, gamma) };
        mi::je_gamma_correct(&mut r, gamma);
        for (c, r) in c.iter().zip(&r) {
            prop_assert_eq!((c.r, c.g, c.b, c.a), (r.r, r.g, r.b, r.a));
        }
        let (mut cc, mut rc) = (palette[0].0, palette[0].0);
        let f = 1.0 + f32::from(gamma) / 10.0;
        // SAFETY: a valid byte.
        unsafe { b::JE_gammaCorrect_func(&raw mut cc, f) };
        mi::je_gamma_correct_func(&mut rc, f);
        prop_assert_eq!(cc, rc);
    }

    #[test]
    fn str_pop_int_matches_c(
        ws in "[ \t\n]{0,3}",
        sign in prop::sample::select(vec!["", "-", "+", "--"]),
        digits in prop_oneof!["[0-9]{0,6}", "[0-9]{18,22}"],
        rest in "[ a-z0-9.-]{0,12}",
    ) {
        let text = format!("{ws}{sign}{digits}{rest}");
        let mut c = [0u8; 256];
        c[..text.len()].copy_from_slice(text.as_bytes());
        for (i, x) in c.iter_mut().enumerate().skip(text.len() + 1) {
            *x = (i % 7) as u8 + b'a';
        }
        let mut r = c;
        let (mut cv, mut rv): (c_int, i32) = (-7, -7);
        // SAFETY: a NUL-terminated 256-byte buffer.
        let cok = unsafe { b::str_pop_int(c.as_mut_ptr().cast(), &raw mut cv) };
        let rok = mi::str_pop_int(&mut r, &mut rv);
        prop_assert_eq!((cok, cv), (rok, rv), "{:?}", text);
        prop_assert_eq!(c.to_vec(), r.to_vec());
    }
}

#[test]
fn handle_chat_does_nothing() {
    let mut rig = Rig::new("ots 1\n");
    // SAFETY: plain C call.
    unsafe { b::JE_handleChat() };
    mi::je_handle_chat(&mut rig.g);
    rig.check();
}

use rustyrian_core::lvlmast::{OPTION_NUM, PORT_NUM, POWER_NUM, SHIELD_NUM, SHIP_NUM, SPECIAL_NUM};
use rustyrian_core::video::ScreenId;
use rustyrian_oracle::mirror::video::screen_to_c;

/// A valid `(itemType, itemNum)` for `JE_getCost`/`JE_getValue`.
fn any_item() -> impl Strategy<Value = (u8, u16)> {
    prop_oneof![
        (Just(2u8), prop_oneof![0..=SHIP_NUM as u16, 91u16..300]),
        (3u8..=4, 0..=PORT_NUM as u16),
        (Just(5u8), 0..=SHIELD_NUM as u16),
        (Just(6u8), 0..=POWER_NUM as u16),
        (7u8..=8, 0..=OPTION_NUM as u16),
        (prop_oneof![0u8..2, 9u8..=255], any::<u16>()),
    ]
}

/// Player item/score fields with valid item numbers and weapon powers.
#[derive(Clone, Debug)]
struct Loadout {
    cash: [u64; 2],
    ship: [u8; 2],
    weapons: [[(u8, u8); 2]; 2],
    shield: [u8; 2],
    generator: [u8; 2],
    sidekicks: [[u8; 2]; 2],
    special: [u8; 2],
}

fn any_loadout() -> impl Strategy<Value = Loadout> {
    let weapon = (0..=PORT_NUM as u8, 1u8..=11);
    (
        prop::array::uniform2(prop_oneof![0u64..2_000_000, any::<u64>()]),
        prop::array::uniform2(0..=SHIP_NUM as u8),
        prop::array::uniform2(prop::array::uniform2(weapon)),
        prop::array::uniform2(0..=SHIELD_NUM as u8),
        prop::array::uniform2(0..=POWER_NUM as u8),
        prop::array::uniform2(prop::array::uniform2(0..=OPTION_NUM as u8)),
        prop::array::uniform2(0..=SPECIAL_NUM as u8),
    )
        .prop_map(
            |(cash, ship, weapons, shield, generator, sidekicks, special)| Loadout {
                cash,
                ship,
                weapons,
                shield,
                generator,
                sidekicks,
                special,
            },
        )
}

fn apply_loadout(g: &mut rustyrian_core::Game, l: &Loadout) {
    for p in 0..2 {
        let pl = &mut g.player.player[p];
        pl.cash = l.cash[p];
        pl.items.ship = l.ship[p];
        for (w, &(id, power)) in pl.items.weapon.iter_mut().zip(&l.weapons[p]) {
            w.id = id;
            w.power = power;
        }
        pl.items.shield = l.shield[p];
        pl.items.generator = l.generator[p];
        pl.items.sidekick = l.sidekicks[p];
        pl.items.special = l.special[p];
    }
}

#[test]
fn item_costs_and_scores_match_c() {
    run_on_rig(
        (any_item(), any_loadout(), any::<bool>()),
        500,
        |rig, ((t, n), l, two)| {
            apply_loadout(&mut rig.g, &l);
            rig.g.config.two_player_mode = two;
            rig.store();
            // SAFETY: plain C calls with valid item numbers and players.
            unsafe {
                prop_assert_eq!(b::JE_getCost(t, n), mi::je_get_cost(&mut rig.g, t, n));
                // Only JE_getCost bounds ship numbers (`> 90`); JE_getValue
                // indexes `ships` directly (`mainint.c:925`).
                if !(t == 2 && usize::from(n) > SHIP_NUM) {
                    prop_assert_eq!(b::JE_getValue(t, n), mi::je_get_value(&mut rig.g, t, n));
                }
                for p in 0..2 {
                    let c = b::JE_totalScore(&raw const b::player[p]);
                    prop_assert_eq!(c, mi::je_total_score(&mut rig.g, p));
                }
            }
            same!(rig; mainint, player);
            Ok(())
        },
    );
}

#[test]
fn adjust_difficulty_matches_c() {
    run_on_rig(
        (any_loadout(), any::<bool>(), 1i8..10, any::<i8>()),
        500,
        |rig, (l, two, initial, level)| {
            apply_loadout(&mut rig.g, &l);
            rig.g.config.two_player_mode = two;
            rig.g.config.initial_difficulty = initial;
            rig.g.config.difficulty_level = level;
            rig.store();
            // SAFETY: plain C call.
            unsafe { b::adjust_difficulty() };
            mi::adjust_difficulty(&mut rig.g);
            same!(rig; config, player);
            Ok(())
        },
    );
}

/// High-score slots with ties and names of every length.
fn any_scores() -> impl Strategy<Value = Vec<(i32, String, u8)>> {
    prop::collection::vec(
        (
            prop_oneof![0i32..5, any::<i32>()],
            "[A-Z ]{0,29}",
            any::<u8>(),
        ),
        rustyrian_core::config::SAVE_FILES_NUM,
    )
}

fn apply_scores(g: &mut rustyrian_core::Game, scores: &[(i32, String, u8)]) {
    for (f, (score, name, diff)) in g.config.save_files.iter_mut().zip(scores) {
        f.high_score1 = *score;
        f.high_score_name = [b'#'; 30];
        f.high_score_name[..name.len()].copy_from_slice(name.as_bytes());
        f.high_score_name[name.len()] = 0;
        f.high_score_diff = *diff;
    }
}

#[test]
fn sort_high_scores_match_c() {
    run_on_rig(
        (any_scores(), 0u8..=15, any::<bool>()),
        500,
        |rig, (scores, temp, all)| {
            apply_scores(&mut rig.g, &scores);
            rig.g.varz.temp = temp;
            rig.store();
            // SAFETY: plain C calls; `temp + 2` is a save slot.
            unsafe {
                if all {
                    b::JE_sortHighScores();
                } else {
                    b::JE_sort();
                }
            }
            if all {
                mi::je_sort_high_scores(&mut rig.g);
            } else {
                mi::je_sort(&mut rig.g);
            }
            same!(rig; config, varz);
            Ok(())
        },
    );
}

#[test]
fn gamma_check_matches_c() {
    run_on_rig((any::<bool>(), any::<u8>()), 500, |rig, (f11, gamma)| {
        rig.g.keyboard.keysactive[68] = f11;
        rig.g.config.gamma_correction = gamma;
        rig.store();
        // SAFETY: plain C call.
        let c = unsafe { b::JE_gammaCheck() };
        prop_assert_eq!(c, mi::je_gamma_check(&mut rig.g));
        same!(rig; keyboard, config, palette);
        Ok(())
    });
}

/// Ships with combos (and ones that skip them); combo positions stay
/// within each combo row.
#[test]
fn sf_codes_match_c() {
    let strategy = (
        1u8..=2,
        prop_oneof![0u8..=13, 15u8..=255],
        (-2i16..=2, -2i16..=2),
        any::<bool>(),
        any::<bool>(),
        prop::array::uniform2(prop::array::uniform21(0u8..=7)),
        prop::collection::vec((-2i16..=2, -2i16..=2, any::<bool>()), 1..12),
    );
    run_on_rig(
        strategy,
        500,
        |rig, (pnum, ship, (px, py), _, super_tyrian, codes, moves)| {
            rig.g.player.player[usize::from(pnum) - 1].items.ship = ship;
            rig.g.config.super_tyrian = super_tyrian;
            rig.g.varz.sf_current_code = codes;
            rig.store();
            for (dx, dy, fire) in moves {
                rig.g.mainint.button[0] = fire;
                // SAFETY: a plain write of a C global on the test thread.
                unsafe { (*rustyrian_oracle::mirror::global_mut(&raw mut b::button))[0] = fire };
                // SAFETY: plain C call.
                unsafe { b::JE_SFCodes(pnum, px, py, px + dx, py + dy) };
                mi::je_sf_codes(&mut rig.g, pnum, px, py, px + dx, py + dy);
                // SAFETY: reads of C globals on the test thread.
                let (codes, executed) = unsafe {
                    (
                        *rustyrian_oracle::mirror::global(&raw const b::SFCurrentCode),
                        *rustyrian_oracle::mirror::global(&raw const b::SFExecuted),
                    )
                };
                prop_assert_eq!(codes, rig.g.varz.sf_current_code);
                prop_assert_eq!(executed, rig.g.varz.sf_executed);
            }
            same!(rig; varz, mainint, player);
            Ok(())
        },
    );
}

#[test]
fn init_player_data_matches_c() {
    run_on_rig(any_loadout(), 500, |rig, l| {
        apply_loadout(&mut rig.g, &l);
        rig.store();
        // SAFETY: plain C call.
        unsafe { b::JE_initPlayerData() };
        mi::je_init_player_data(&mut rig.g);
        same!(rig; player, config, mtrand, episodes);
        Ok(())
    });
}

#[test]
fn get_name_matches_c() {
    let strategy = (1u8..=2, 1u32..=2, "[a-z]{0,10}", "[a-z]{0,10}");
    run_on_rig(strategy, 500, |rig, (pnum, me, mine, theirs)| {
        rig.g.network.this_player_num = me;
        rig.g.network.network_player_name = Some(mine.into_bytes());
        rig.g.network.network_opponent_name = Some(theirs.into_bytes());
        rig.store();
        // SAFETY: plain C call; returns a C string.
        let c = unsafe { std::ffi::CStr::from_ptr(b::JE_getName(pnum)) }
            .to_bytes()
            .to_vec();
        prop_assert_eq!(c, mi::je_get_name(&rig.g, pnum));
        same!(rig; network, helptext);
        Ok(())
    });
}

#[test]
fn hud_drawing_matches_c() {
    let strategy = (
        any_loadout(),
        prop::array::uniform2(1u8..=12),
        prop::array::uniform2(0u32..=12),
        (
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
        ),
        (1u32..=2, "[A-Za-z0-9 ]{0,10}", "[A-Za-z0-9 ]{0,10}"),
        ("[ -~]{0,30}", prop_oneof![Just(0u16), 1u16..200]),
        (
            0u32..=3,
            any::<u16>(),
            any::<u16>(),
            any::<u8>(),
            any::<u8>(),
        ),
    );
    run_on_rig(
        strategy,
        500,
        |rig,
         (
            l,
            lives,
            bombs,
            (one, two, galaga, net, cheat),
            (me, mine, theirs),
            (text, erase),
            (mode, cx, cy, filter, bright),
        )| {
            apply_loadout(&mut rig.g, &l);
            for p in 0..2 {
                let pl = &mut rig.g.player.player[p];
                pl.lives = Some(p);
                pl.items.weapon[p].power = lives[p];
                pl.superbombs = bombs[p];
                pl.weapon_mode = mode;
            }
            // A special whose 2x2 graphic is 0 (twiddle-only specials) or
            // lies past the loaded sheet would make C read outside
            // `spriteSheet10` (D-UB-SPRITE-1); the game only grants item
            // specials it has graphics for.
            let sheet = rig.g.sprite.sprite_sheet10.data.as_deref().unwrap_or(&[]);
            let entries = sheet
                .get(..2)
                .map_or(0, |b| u32::from(u16::from_le_bytes([b[0], b[1]])) / 2);
            let special = usize::from(rig.g.player.player[0].items.special);
            let graphic = u32::from(rig.g.episodes.special[special].itemgraphic);
            if graphic == 0 || graphic + 20 > entries {
                rig.g.player.player[0].items.special = 0;
            }
            let c = &mut rig.g.config;
            (
                c.one_player_action,
                c.two_player_mode,
                c.galaga_mode,
                c.you_are_cheating,
            ) = (one, two, galaga, cheat);
            rig.g.network.is_network_game = net;
            rig.g.network.this_player_num = me;
            rig.g.network.network_player_name = Some(mine.into_bytes());
            rig.g.network.network_opponent_name = Some(theirs.into_bytes());
            rig.g.mainint.text_erase = erase;
            rig.store();
            let ctext = std::ffi::CString::new(text.clone()).unwrap();
            let seg = rig.g.video.vga_screen_seg;
            // SAFETY: plain C calls on live surfaces.
            unsafe {
                b::JE_inGameDisplays();
                b::JE_drawPortConfigButtons();
                b::JE_drawTextWindow(ctext.as_ptr());
                // `blit_sprite` does not clip: keep the cube on screen, as
                // upstream's callers do.
                b::JE_drawCube(screen_to_c(seg), cx % 280, cy % 170, filter, bright);
            }
            mi::je_in_game_displays(&mut rig.g);
            mi::je_draw_port_config_buttons(&mut rig.g);
            mi::je_draw_text_window(&mut rig.g, text.as_bytes());
            let seg = seg.unwrap_or(ScreenId::VgaScreenSeg);
            mi::je_draw_cube(&mut rig.g, seg, cx % 280, cy % 170, filter, bright);
            same!(rig; video, varz, mainint, sprite);
            Ok(())
        },
    );
}

// ---------------------------------------------------------------------------
// Interactive screens: both engines on the same timed input script, from a
// gameplay fixture. Calls, states, saved files and every frame are compared.
// ---------------------------------------------------------------------------

use mainint_support::script;

/// A click at screen (= window, scaler "None") coordinates; button 0 is a
/// hover (a move only: `SDL_BUTTON(0)` shifts by -1 in C).
fn click(ms: u32, x: i32, y: i32, button: u8) -> Vec<(u32, String)> {
    let mut v = vec![(ms, format!("mouse-move {x} {y}"))];
    if button != 0 {
        v.push((ms + 20, format!("mouse-down {button}")));
        v.push((ms + 65, format!("mouse-up {button}")));
    }
    v
}

/// Builds a script from key taps and clicks.
fn input(keys: &[(u32, &str)], clicks: &[(u32, i32, i32, u8)], extra: &[(u32, &str)]) -> String {
    let mut events: Vec<(u32, String)> = Vec::new();
    for &(t, k) in keys {
        events.push((t, format!("key-down {k}")));
        events.push((t + 45, format!("key-up {k}")));
    }
    for &(t, x, y, b) in clicks {
        events.extend(click(t, x, y, b));
    }
    events.extend(extra.iter().map(|&(t, e)| (t, e.to_owned())));
    // A script that does not reach its screen's exit ends the run through
    // `SDL_QUIT` (both engines exit) a minute after its last event, instead
    // of waiting for ever.
    let last = events.iter().map(|e| e.0).max().unwrap_or(0);
    events.push((last + 60_000, "quit".to_owned()));
    events.sort_by_key(|e| e.0);
    let refs: Vec<(u32, &str)> = events.iter().map(|(t, e)| (*t, e.as_str())).collect();
    script(&refs)
}

/// Runs a screen on both engines from fixture 500 after `setup`, then
/// checks calls, state, files and frames; the screen must present.
fn screen_run<T: PartialEq + std::fmt::Debug>(
    script: &str,
    setup: impl FnOnce(&mut rustyrian_core::Game),
    c: impl FnOnce() -> T,
    r: impl FnOnce(&mut rustyrian_core::Game) -> rustyrian_core::game::Flow<T>,
) -> Result<T, i32> {
    let (out, frames) = screen_run_frames(script, setup, c, r);
    assert!(frames > 0, "the screen presented nothing");
    out
}

/// [`screen_run`] for the paths that present nothing (no high score, a
/// load, no menu request, a fatal file error): asserts that.
fn screen_run_quiet<T: PartialEq + std::fmt::Debug>(
    script: &str,
    setup: impl FnOnce(&mut rustyrian_core::Game),
    c: impl FnOnce() -> T,
    r: impl FnOnce(&mut rustyrian_core::Game) -> rustyrian_core::game::Flow<T>,
) -> Result<T, i32> {
    let (out, frames) = screen_run_frames(script, setup, c, r);
    assert_eq!(frames, 0, "expected no presented frames");
    out
}

fn screen_run_frames<T: PartialEq + std::fmt::Debug>(
    script: &str,
    setup: impl FnOnce(&mut rustyrian_core::Game),
    c: impl FnOnce() -> T,
    r: impl FnOnce(&mut rustyrian_core::Game) -> rustyrian_core::game::Flow<T>,
) -> (Result<T, i32>, usize) {
    let mut rig = Rig::from_fixture(script, 500);
    setup(&mut rig.g);
    rig.store();
    rig.clear();
    let out = rig.run(c, r);
    if out.is_ok() {
        rig.check();
    } else {
        rig.check_halted();
        mainint_support::forget_freed_c();
    }
    // The rig's `init_video` presents one frame before the screen runs.
    (out, rig.check_frames() - 1)
}

#[test]
fn help_system_keyboard_matches_c() {
    // Menu: moves with wrap-around, then the first topic. Pages: every page
    // forward (and a key pages ignore), back past the first page to the
    // menu, then Escape in the menu. The shop sheet is loaded first.
    let mut keys = vec![
        (1500, "DOWN"),
        (2500, "UP"),
        (3500, "UP"),
        (4500, "DOWN"),
        (5500, "RETURN"),
    ];
    let page_keys = [
        "RIGHT", "SPACE", "RIGHT", "RETURN", "RIGHT", "RIGHT", "A", "RIGHT",
    ];
    keys.extend(
        page_keys
            .iter()
            .enumerate()
            .map(|(i, &k)| (7000 + 1000 * i as u32, k)),
    );
    keys.extend((0..8).map(|i| (15500 + 1000 * i, "LEFT")));
    keys.push((24500, "ESCAPE"));
    let s = input(&keys, &[], &[]);
    let out = screen_run(
        &s,
        |g| g.sprite.shop_sprite_sheet = Default::default(),
        // SAFETY: plain C call on the test thread.
        || unsafe { b::JE_helpSystem(1) },
        |g| mi::je_help_system(g, 1),
    );
    assert_eq!(out, Ok(()));
}

#[test]
fn help_system_page_edges_match_c() {
    // Starts on page 7 (topic 5); paging past the last page returns to the
    // menu with the last item selected, which then exits.
    let s = input(
        &[
            (1500, "RIGHT"),
            (2500, "RIGHT"),
            (3500, "RIGHT"),
            (5000, "RETURN"),
        ],
        &[],
        &[],
    );
    let out = screen_run(
        &s,
        |_| {},
        // SAFETY: plain C call on the test thread.
        || unsafe { b::JE_helpSystem(5) },
        |g| mi::je_help_system(g, 5),
    );
    assert_eq!(out, Ok(()));

    // Leaving from a page: Escape, then a right click.
    for (keys, clicks) in [
        (&[(1500, "ESCAPE")][..], &[][..]),
        (&[][..], &[(1500, 100, 100, 3)][..]),
    ] {
        let s = input(keys, clicks, &[]);
        let out = screen_run(
            &s,
            |_| {},
            // SAFETY: plain C call on the test thread.
            || unsafe { b::JE_helpSystem(3) },
            |g| mi::je_help_system(g, 3),
        );
        assert_eq!(out, Ok(()));
    }

    // The static page function on its own: paging back to page 0.
    let s = input(&[(1500, "LEFT")], &[], &[]);
    let out = screen_run(
        &s,
        |_| {},
        || {
            let (mut topic, mut restart) = (2u8, true);
            // SAFETY: plain C call with valid pointers.
            let r = unsafe { b::ot_s_mainint_helpSystemPage(&raw mut topic, &raw mut restart) };
            (r, topic, restart)
        },
        |g| {
            let (mut topic, mut restart) = (2u8, true);
            let r = mi::help_system_page(g, &mut topic, &mut restart)?;
            Ok((r, topic, restart))
        },
    );
    assert_eq!(out, Ok((true, 1, false)));
}

#[test]
fn help_system_mouse_matches_c() {
    let s = input(
        &[],
        &[
            (1500, 160, 82, 0),  // hover the second item
            (2500, 160, 64, 1),  // click the first item
            (4000, 250, 100, 1), // next page
            (5000, 60, 100, 1),  // previous page
            (6000, 60, 100, 2),  // middle button: nothing
            (7000, 60, 100, 1),  // previous page: back to the menu
            (8500, 160, 100, 1), // click the third item
            (10000, 60, 100, 1), // previous page
            (10500, 60, 100, 1), // previous page
            (11000, 60, 100, 1), // previous page: back to the menu
            (11500, 5, 5, 1),    // menu: click outside every item
            (12500, 5, 5, 3),    // right click: leave
        ],
        &[],
    );
    let out = screen_run(
        &s,
        |_| {},
        // SAFETY: plain C call on the test thread.
        || unsafe { b::JE_helpSystem(1) },
        |g| mi::je_help_system(g, 1),
    );
    assert_eq!(out, Ok(()));
}

/// Fills save slot `i` (0-based) as a saved game.
fn fill_slot(g: &mut rustyrian_core::Game, i: usize, name: &str, level: u16, episode: u8) {
    let f = &mut g.config.save_files[i];
    f.level = level;
    f.name = [0; 15];
    f.name[..name.len()].copy_from_slice(name.as_bytes());
    f.level_name = [0; 11];
    f.level_name[..6].copy_from_slice(b"TYRIAN");
    f.episode = episode;
}

fn with_saves(g: &mut rustyrian_core::Game) {
    fill_slot(g, 0, "ACE", 3, 1);
    fill_slot(g, 2, "LONG NAME HERE", 12, 2);
    fill_slot(g, 11 + 1, "TWO", 5, 3);
}

#[test]
fn load_screen_keyboard_matches_c() {
    // Empty slot (clink), paging both ways, wrap-around, then loading slot 3.
    let s = input(
        &[
            (1500, "DOWN"),
            (2500, "RETURN"),
            (3500, "RIGHT"),
            (4500, "LEFT"),
            (5500, "LEFT"),
            (6500, "RIGHT"),
            (7500, "UP"),
            (8500, "DOWN"),
            (9500, "A"),
            (10500, "DOWN"),
            (11500, "RETURN"),
        ],
        &[],
        &[],
    );
    let out = screen_run(
        &s,
        with_saves,
        // SAFETY: plain C call on the test thread.
        || unsafe { b::JE_loadScreen() },
        mi::je_load_screen,
    );
    assert_eq!(out, Ok(true));
}

#[test]
fn load_screen_exits_match_c() {
    // The last item ("exit"), then Escape on a second run.
    let s = input(&[(1500, "UP"), (2500, "SPACE")], &[], &[]);
    let out = screen_run(
        &s,
        with_saves,
        // SAFETY: plain C call on the test thread.
        || unsafe { b::JE_loadScreen() },
        mi::je_load_screen,
    );
    assert_eq!(out, Ok(false));

    let s = input(&[(1500, "ESCAPE")], &[], &[]);
    let out = screen_run(
        &s,
        |_| {},
        // SAFETY: plain C call on the test thread.
        || unsafe { b::JE_loadScreen() },
        mi::je_load_screen,
    );
    assert_eq!(out, Ok(false));
}

#[test]
fn load_screen_mouse_matches_c() {
    let s = input(
        &[],
        &[
            (1500, 220, 185, 1), // right control
            (2500, 90, 185, 1),  // left control
            (3500, 90, 185, 2),  // left control, middle button: nothing
            (4000, 220, 185, 2), // right control, middle button: nothing
            (4500, 50, 45, 0),   // hover the second item
            (5000, 50, 32, 1),   // click the first item: loads it
            (6000, 5, 100, 3),   // (never reached)
        ],
        &[],
    );
    let out = screen_run(
        &s,
        with_saves,
        // SAFETY: plain C call on the test thread.
        || unsafe { b::JE_loadScreen() },
        mi::je_load_screen,
    );
    assert_eq!(out, Ok(true));

    let s = input(
        &[],
        &[
            (1500, 5, 100, 1),
            (2500, 50, 45, 1),
            (3500, 50, 175, 1),
            (4500, 5, 100, 3),
        ],
        &[],
    );
    let out = screen_run(
        &s,
        with_saves,
        // SAFETY: plain C call on the test thread.
        || unsafe { b::JE_loadScreen() },
        mi::je_load_screen,
    );
    assert_eq!(out, Ok(false));
}

#[test]
fn high_score_screen_matches_c() {
    let setup = |g: &mut rustyrian_core::Game| {
        for (i, f) in g.config.save_files.iter_mut().enumerate() {
            f.high_score1 = 1000 * i as i32;
            f.high_score_name = [0; 30];
            f.high_score_name[..5].copy_from_slice(b"PILOT");
            f.high_score_diff = (i % 12) as u8;
        }
        g.episodes.episode_avail = [true, false, true, false, false];
    };
    let s = input(
        &[
            (1500, "LEFT"),
            (2500, "LEFT"),
            (3500, "RIGHT"),
            (4500, "A"),
            (5500, "RETURN"),
        ],
        &[],
        &[(4000, "mouse-move 30 30")],
    );
    let out = screen_run(
        &s,
        setup,
        // SAFETY: plain C call on the test thread.
        || unsafe { b::JE_highScoreScreen() },
        mi::je_high_score_screen,
    );
    assert_eq!(out, Ok(()));

    let s = input(
        &[],
        &[
            (1500, 220, 185, 1), // right control
            (2500, 220, 185, 1), // right control (wraps)
            (3500, 90, 185, 1),  // left control
            (4500, 90, 185, 2),  // middle button
            (5000, 160, 100, 1), // no control
            (5500, 160, 100, 3), // right click: leave
        ],
        &[],
    );
    let out = screen_run(
        &s,
        setup,
        // SAFETY: plain C call on the test thread.
        || unsafe { b::JE_highScoreScreen() },
        mi::je_high_score_screen,
    );
    assert_eq!(out, Ok(()));
}

fn setup_menu(g: &mut rustyrian_core::Game) {
    g.network.your_in_game_menu_request = true;
    g.config.processor_type = 2;
    g.config.game_speed = 3;
    g.nortsong.tyr_music_volume = 128;
    g.nortsong.fx_volume = 250;
}

#[test]
fn in_game_setup_keyboard_matches_c() {
    // Every item, both directions with wrap-around, toggles, the hidden
    // detail level, then "Return to game".
    let keys: Vec<(u32, &str)> = [
        "RIGHT", "RIGHT", "LEFT", "RETURN", "DOWN", "RIGHT", "LEFT", "LEFT", "SPACE", "DOWN",
        "LEFT", "LEFT", "RIGHT", "RIGHT", "RIGHT", "RIGHT", "W", "DOWN", "W", "LEFT", "LEFT",
        "LEFT", "RIGHT", "RIGHT", "RIGHT", "RIGHT", "A", "UP", "UP", "UP", "UP", "UP", "RETURN",
    ]
    .iter()
    .enumerate()
    .map(|(i, &k)| (1500 + 700 * i as u32, k))
    .collect();
    let s = input(&keys, &[], &[]);
    let out = screen_run(
        &s,
        setup_menu,
        // SAFETY: plain C call on the test thread.
        || unsafe { b::JE_doInGameSetup() },
        mi::je_do_in_game_setup,
    );
    assert_eq!(out, Ok(()));
}

#[test]
fn in_game_setup_quit_and_escape_match_c() {
    // "Quit" (ends the level).
    let s = input(&[(1500, "UP"), (2500, "RETURN")], &[], &[]);
    let out = screen_run(
        &s,
        setup_menu,
        // SAFETY: plain C call on the test thread.
        || unsafe { b::JE_inGameSetup() },
        mi::je_in_game_setup,
    );
    assert_eq!(out, Ok(true));

    let s = input(&[(1500, "UP"), (2500, "RETURN")], &[], &[]);
    let out = screen_run(
        &s,
        setup_menu,
        // SAFETY: plain C call on the test thread.
        || unsafe { b::JE_doInGameSetup() },
        mi::je_do_in_game_setup,
    );
    assert_eq!(out, Ok(()));

    // Escape.
    let s = input(&[(1500, "ESCAPE")], &[], &[]);
    let out = screen_run(
        &s,
        setup_menu,
        // SAFETY: plain C call on the test thread.
        || unsafe { b::JE_inGameSetup() },
        mi::je_in_game_setup,
    );
    assert_eq!(out, Ok(false));

    // No menu request: only the relative mouse mode toggles.
    let s = input(&[], &[], &[]);
    let out = screen_run_quiet(
        &s,
        |g| g.network.your_in_game_menu_request = false,
        // SAFETY: plain C call on the test thread.
        || unsafe { b::JE_doInGameSetup() },
        mi::je_do_in_game_setup,
    );
    assert_eq!(out, Ok(()));
}

#[test]
fn in_game_setup_quit_in_constant_play_halts_like_c() {
    let s = input(&[(1500, "UP"), (2500, "RETURN")], &[], &[]);
    let out = screen_run(
        &s,
        |g| {
            setup_menu(g);
            g.params.constant_play = true;
        },
        // SAFETY: plain C call on the test thread.
        || unsafe { b::JE_inGameSetup() },
        mi::je_in_game_setup,
    );
    assert_eq!(out, Err(0));
}

#[test]
fn in_game_setup_mouse_matches_c() {
    // Volume bars by value, detail/speed by value, items by name, misses,
    // then a right click.
    let s = input(
        &[],
        &[
            (1500, 160, 25, 1),  // music volume value
            (2500, 125, 45, 1),  // effects volume value (low)
            (3500, 200, 45, 1),  // effects volume value (high)
            (4500, 150, 65, 1),  // detail level value
            (5500, 150, 85, 1),  // game speed value
            (6500, 20, 25, 1),   // music by name: toggle
            (7500, 20, 25, 2),   // middle button: nothing
            (8000, 300, 25, 1),  // right of the menu: nothing
            (8500, 20, 30, 1),   // between items
            (9000, 20, 105, 0),  // hover "Return to game"
            (9500, 215, 105, 1), // "Return to game" by value: nothing
            (10500, 20, 105, 3), // right click: leave
        ],
        &[],
    );
    let out = screen_run(
        &s,
        setup_menu,
        // SAFETY: plain C call on the test thread.
        || unsafe { b::JE_inGameSetup() },
        mi::je_in_game_setup,
    );
    assert_eq!(out, Ok(false));
}

#[test]
fn in_game_help_matches_c() {
    for two in [false, true] {
        let s = input(&[(2500, "SPACE")], &[], &[(1500, "mouse-move 100 100")]);
        let out = screen_run(
            &s,
            |g| g.config.two_player_mode = two,
            // SAFETY: plain C call on the test thread.
            || unsafe { b::JE_inGameHelp() },
            mi::je_in_game_help,
        );
        assert_eq!(out, Ok(()));
    }
}

#[test]
fn pause_game_matches_c() {
    for super_pause in [false, true] {
        // Modifier keys do not unpause; a key does.
        let s = input(
            &[(1500, "LCTRL"), (2000, "RALT"), (3000, "SPACE")],
            &[],
            &[],
        );
        let (out, frames) = screen_run_frames(
            &s,
            |g| g.config.super_pause = super_pause,
            // SAFETY: plain C call on the test thread.
            || unsafe { b::JE_pauseGame() },
            mi::je_pause_game,
        );
        assert_eq!(out, Ok(()));
        // Super pause shows no "paused" text and presents nothing.
        assert_eq!(frames > 0, !super_pause);
    }
    // A mouse click unpauses.
    let s = input(&[], &[(1500, 100, 100, 1)], &[]);
    let out = screen_run(
        &s,
        |_| {},
        // SAFETY: plain C call on the test thread.
        || unsafe { b::JE_pauseGame() },
        mi::je_pause_game,
    );
    assert_eq!(out, Ok(()));
}

// ---------------------------------------------------------------------------
// Netplay paths of the pause and in-game menus (loopback network).
// ---------------------------------------------------------------------------

mod net_support;

use rustyrian_core::network::{
    PACKET_ACKNOWLEDGE, PACKET_GAME_MENU, PACKET_GAME_PAUSE, PACKET_GAME_QUIT, PACKET_WAITING,
};

/// A packet: header (`type`, `sync`) then `body`.
fn pkt(type_: u16, sync: u16) -> Vec<u8> {
    let mut v = type_.to_be_bytes().to_vec();
    v.extend_from_slice(&sync.to_be_bytes());
    v
}

/// Both engines from fixture 500 with video and a connected loopback
/// network game.
fn net_rig(script: &str, setup: impl FnOnce(&mut rustyrian_core::Game)) -> net_support::Rig {
    let mut rig = net_support::Rig::new(script);
    rig.g
        .restore(rustyrian_oracle::diff::fixture("demo-cycle", 500));
    rig.g.file.custom_data_dir_path =
        Some(std::os::unix::ffi::OsStrExt::as_bytes(net_support::data_dir().as_os_str()).to_vec());
    rig.g.video_scale.scaler = 0;
    rig.g.loudness.audio_disabled = true;
    rig.store();
    rig.video();
    rig.net();
    let now = rig.both(|p| p.get_ticks());
    let n = &mut rig.g.network;
    n.is_network_game = true;
    n.connected = true;
    n.last_in_tick = now;
    n.network_check_keep_alive_tick = now;
    setup(&mut rig.g);
    rig.store();
    rig.clear();
    rig
}

/// Queues the opponent's reliable packets (in order) and acknowledgements
/// of our next `acks` packets.
fn opponent(rig: &net_support::Rig, inbound: &[u16], acks: u16) {
    let (in_sync, out_sync) = (rig.g.network.queue_in_sync, rig.g.network.queue_out_sync);
    for (i, &t) in inbound.iter().enumerate() {
        rig.push(&pkt(t, in_sync.wrapping_add(i as u16)));
    }
    for i in 0..acks {
        rig.push(&pkt(PACKET_ACKNOWLEDGE, out_sync.wrapping_add(i)));
    }
}

#[test]
fn pause_game_netplay_matches_c() {
    for (inbound, acks, keys) in [
        // The opponent pauses back, then unpauses first.
        (&[PACKET_GAME_PAUSE, PACKET_WAITING][..], 1, &[][..]),
        // The opponent pauses back; we unpause.
        (&[PACKET_GAME_PAUSE][..], 2, &[(1500, "SPACE")][..]),
    ] {
        let s = input(keys, &[], &[]);
        let mut rig = net_rig(&s, |_| {});
        opponent(&rig, inbound, acks);
        // SAFETY: plain C call on the test thread.
        let out = rig.run(|| unsafe { b::JE_pauseGame() }, mi::je_pause_game);
        assert_eq!(out, Ok(()));
        rig.check();
    }
}

/// A netplay in-game menu case: menu requested here, the opponent's
/// packets, acknowledgements of ours, and our keys.
type NetMenuCase<'a> = (bool, &'a [u16], u16, &'a [(u32, &'a str)]);

#[test]
fn in_game_setup_netplay_matches_c() {
    let cases: [NetMenuCase; 4] = [
        // We open the menu and return to the game.
        (
            true,
            &[PACKET_GAME_MENU],
            2,
            &[(1500, "UP"), (2000, "UP"), (2500, "RETURN")],
        ),
        // We open the menu and quit.
        (
            true,
            &[PACKET_GAME_MENU],
            2,
            &[(1500, "UP"), (2500, "RETURN")],
        ),
        // The opponent's menu: they return.
        (false, &[PACKET_GAME_MENU, PACKET_WAITING], 1, &[]),
        // The opponent's menu: they quit.
        (false, &[PACKET_GAME_MENU, PACKET_GAME_QUIT], 1, &[]),
    ];
    for (request, inbound, acks, keys) in cases {
        let s = input(keys, &[], &[]);
        let mut rig = net_rig(&s, |g| {
            setup_menu(g);
            g.network.your_in_game_menu_request = request;
        });
        opponent(&rig, inbound, acks);
        // SAFETY: plain C call on the test thread.
        let out = rig.run(|| unsafe { b::JE_doInGameSetup() }, mi::je_do_in_game_setup);
        assert_eq!(out, Ok(()));
        rig.check();
    }
}

// ---------------------------------------------------------------------------
// Name entry, save/load, episode end, credits, level end, glowing text.
// ---------------------------------------------------------------------------

fn high_score_setup(
    two: bool,
    episode: u8,
    cash: [u64; 2],
) -> impl FnOnce(&mut rustyrian_core::Game) {
    move |g| {
        g.config.two_player_mode = two;
        g.episodes.initial_episode_num = episode;
        g.episodes.episode_num = episode;
        g.player.player[0].cash = cash[0];
        g.player.player[1].cash = cash[1];
        g.config.difficulty_level = 3;
        for (i, f) in g.config.save_files.iter_mut().enumerate() {
            f.high_score1 = 500 * (i as i32 % 3 + 1);
            f.high_score_name = [0; 30];
            f.high_score_name[..3].copy_from_slice(b"OLD");
        }
    }
}

#[test]
fn high_score_check_matches_c() {
    // Typing (with characters the font lacks), backspace, enter; the glowing
    // table, then a key.
    let s = input(
        // 76: SDL_SCANCODE_DELETE (the script has no name for it).
        &[
            (3000, "BACKSPACE"),
            (4000, "76"),
            (5000, "RETURN"),
            (12000, "SPACE"),
        ],
        &[],
        &[(2000, "text \"Ab~|\u{e9} 9\""), (3500, "text \"xy\"")],
    );
    let out = screen_run(
        &s,
        high_score_setup(false, 1, [1_000_000, 0]),
        // SAFETY: plain C call on the test thread.
        || unsafe { b::JE_highScoreCheck() },
        mi::je_high_score_check,
    );
    assert_eq!(out, Ok(()));

    // Two players, highest scorer asked first; cancel by Escape and by the
    // cancel button; skip the glow with a key.
    let escapes: Vec<(u32, &str)> = (0..20).map(|i| (2000 + 1000 * i, "ESCAPE")).collect();
    let s = input(&escapes, &[], &[]);
    let out = screen_run(
        &s,
        high_score_setup(true, 2, [600, 1200]),
        // SAFETY: plain C call on the test thread.
        || unsafe { b::JE_highScoreCheck() },
        mi::je_high_score_check,
    );
    assert_eq!(out, Ok(()));

    // Single player: a click outside the buttons, then the cancel button.
    let keys: Vec<(u32, &str)> = (0..15).map(|i| (6000 + 1000 * i, "SPACE")).collect();
    let s = input(&keys, &[(2000, 20, 20, 1), (3000, 190, 130, 1)], &[]);
    let out = screen_run(
        &s,
        high_score_setup(false, 2, [9_000_000, 0]),
        // SAFETY: plain C call on the test thread.
        || unsafe { b::JE_highScoreCheck() },
        mi::je_high_score_check,
    );
    assert_eq!(out, Ok(()));

    // No high score (the single-player score includes the items' value, so
    // every table entry is the maximum): nothing happens.
    let s = input(&[], &[], &[]);
    let setup = |g: &mut rustyrian_core::Game| {
        high_score_setup(false, 3, [0, 0])(g);
        for f in g.config.save_files.iter_mut() {
            f.high_score1 = i32::MAX;
        }
    };
    let out = screen_run_quiet(
        &s,
        setup,
        // SAFETY: plain C call on the test thread.
        || unsafe { b::JE_highScoreCheck() },
        mi::je_high_score_check,
    );
    assert_eq!(out, Ok(()));
}

#[test]
fn high_score_check_long_name_matches_c() {
    // A name longer than the box (28 characters) and the OK button.
    let s = input(
        &[(9000, "SPACE"), (16000, "SPACE")],
        &[(8000, 100, 130, 1)],
        &[
            (2000, "text \"ABCDEFGHIJKLMNOPQRSTUVWXYZ\""),
            (3000, "text \"0123456789\""),
        ],
    );
    let out = screen_run(
        &s,
        high_score_setup(false, 1, [1_000_000, 0]),
        // SAFETY: plain C call on the test thread.
        || unsafe { b::JE_highScoreCheck() },
        mi::je_high_score_check,
    );
    assert_eq!(out, Ok(()));
}

#[test]
fn operation_load_matches_c() {
    for (slot, save) in [(1u8, true), (2, false)] {
        let s = input(&[], &[], &[]);
        let out = screen_run_quiet(
            &s,
            |g| {
                with_saves(g);
                g.mainint.perform_save = false;
                if !save {
                    g.config.save_files[1].level = 0;
                }
            },
            // SAFETY: plain C call on the test thread.
            || unsafe { b::JE_operation(slot) },
            |g| mi::je_operation(g, slot),
        );
        // Loading presents nothing; the empty slot neither.
        assert_eq!(out, Ok(()));
    }
}

#[test]
fn operation_save_matches_c() {
    // Name editing (backspace, typing past the limit), then Enter saves.
    let s = input(
        &[(1500, "BACKSPACE"), (2000, "BACKSPACE"), (4000, "RETURN")],
        &[],
        &[(3000, "text \"PILOT 1234567890XYZ\"")],
    );
    let out = screen_run(
        &s,
        |g| {
            with_saves(g);
            g.mainint.perform_save = true;
        },
        // SAFETY: plain C call on the test thread.
        || unsafe { b::JE_operation(3) },
        |g| mi::je_operation(g, 3),
    );
    assert_eq!(out, Ok(()));

    // Escape, the cancel button, the OK button.
    for (keys, clicks) in [
        (&[(1500, "ESCAPE")][..], &[][..]),
        (&[][..], &[(1500, 20, 20, 1), (2500, 190, 130, 1)][..]),
        (&[][..], &[(1500, 100, 130, 1)][..]),
    ] {
        let s = input(keys, clicks, &[(1200, "text \"Q\"")]);
        let out = screen_run(
            &s,
            |g| {
                g.mainint.perform_save = true;
            },
            // SAFETY: plain C call on the test thread.
            || unsafe { b::JE_operation(5) },
            |g| mi::je_operation(g, 5),
        );
        assert_eq!(out, Ok(()));
    }
}

#[test]
fn operation_on_last_slot_does_nothing() {
    let mut rig = Rig::from_fixture("ots 1\n", 500);
    rig.g.mainint.perform_save = true;
    rig.store();
    rig.clear();
    let out = rig.run(
        // SAFETY: plain C call on the test thread.
        || unsafe { b::JE_operation(11) },
        |g| mi::je_operation(g, 11),
    );
    assert_eq!(out, Ok(()));
    rig.check();
}

#[test]
fn out_char_glow_matches_c() {
    // (frameCountMax, ESCPressed, level warning, key time)
    for (max, esc, warning, key) in [
        (0u16, false, false, None),
        (2, false, true, None),
        (3, false, false, Some(1500u32)),
        (2, true, false, None),
    ] {
        let s = match key {
            Some(t) => input(&[(t, "SPACE")], &[], &[]),
            None => input(&[], &[], &[]),
        };
        let text = "Glow ~text~ #1, with spaces!";
        let ctext = std::ffi::CString::new(text).unwrap();
        let (out, frames) = screen_run_frames(
            &s,
            |g| {
                g.nortsong.frame_count_max = max;
                g.keyboard.esc_pressed = esc;
                g.fonthand.level_warning_display = warning;
                g.fonthand.warning_red = warning;
                g.mainint.use_last_bank = !warning;
            },
            // SAFETY: plain C call with a C string.
            || unsafe { b::JE_outCharGlow(20, 100, ctext.as_ptr()) },
            |g| mi::je_out_char_glow(g, 20, 100, text.as_bytes()),
        );
        assert_eq!(out, Ok(()));
        // A pressed Escape stops the glow before its first frame.
        assert_eq!(frames > 0, !esc);
    }
}

#[test]
fn next_episode_matches_c() {
    // (initial/episode, constant play, jump back to episode 1, mtrand seed);
    // a key every second ends each wait (credits, the episode title).
    let cases: [(u8, bool, bool, u64); 5] = [
        // Episode 1 ends normally: no high score (scores are high).
        (1, false, false, 1),
        // Constant play: no high-score check and no wait.
        (2, true, false, 2),
        // Episode 3 ends with a jump back: credits and the SuperCarrot draw
        // for two seeds.
        (3, false, true, 4),
        (3, false, true, 7),
        // Episode 4 (the last available): no high-score check.
        (4, false, false, 3),
    ];
    let keys: Vec<(u32, &str)> = (0..30).map(|i| (2000 + 1000 * i, "SPACE")).collect();
    for (episode, constant, jump, seed) in cases {
        let s = input(if constant { &[] } else { &keys }, &[], &[]);
        let out = screen_run(
            &s,
            |g| {
                g.episodes.initial_episode_num = episode;
                g.episodes.episode_num = episode;
                g.config.game_has_repeated = false;
                g.params.constant_play = constant;
                g.episodes.jump_back_to_episode1 = jump;
                rustyrian_core::mtrand::mt_srand(g, seed);
                for f in g.config.save_files.iter_mut() {
                    f.high_score1 = i32::MAX;
                }
            },
            // SAFETY: plain C call on the test thread.
            || unsafe { b::JE_nextEpisode() },
            mi::je_next_episode,
        );
        assert_eq!(out, Ok(()), "episode {episode}");
    }
}

#[test]
fn next_episode_with_high_score_matches_c() {
    let s = input(
        &[(2000, "RETURN"), (9000, "SPACE"), (14000, "SPACE")],
        &[],
        &[(1500, "text \"NEW\"")],
    );
    let out = screen_run(
        &s,
        |g| {
            high_score_setup(false, 1, [5_000_000, 0])(g);
            g.config.game_has_repeated = false;
            g.params.constant_play = false;
            g.network.is_network_game = false;
            g.episodes.jump_back_to_episode1 = false;
        },
        // SAFETY: plain C call on the test thread.
        || unsafe { b::JE_nextEpisode() },
        mi::je_next_episode,
    );
    assert_eq!(out, Ok(()));
}

#[test]
fn play_credits_matches_c() {
    // Ended by a key early on.
    let s = input(&[(4000, "SPACE")], &[], &[]);
    let out = screen_run(
        &s,
        |_| {},
        // SAFETY: plain C call on the test thread.
        || unsafe { b::JE_playCredits() },
        mi::je_play_credits,
    );
    assert_eq!(out, Ok(()));
}

#[test]
#[ignore = "160 s: full credits roll; run by coverage and nightly"]
fn play_credits_to_the_end_matches_c() {
    // The whole roll: the song fades near the end and the last frame repeats
    // (restarting the closing song) until a key.
    let s = input(&[(260_000, "SPACE")], &[], &[]);
    let out = screen_run(
        &s,
        |_| {},
        // SAFETY: plain C call on the test thread.
        || unsafe { b::JE_playCredits() },
        mi::je_play_credits,
    );
    assert_eq!(out, Ok(()));
}

#[test]
fn play_credits_missing_file_halts_like_c() {
    let empty = mainint_support::tmp("no-credits");
    // Only the sprite file is present: tyrian.cdt is missing.
    std::fs::copy(
        mainint_support::data_dir().join("estsc.shp"),
        empty.join("estsc.shp"),
    )
    .unwrap();
    std::fs::copy(
        mainint_support::data_dir().join("music.mus"),
        empty.join("music.mus"),
    )
    .unwrap();
    let s = input(&[], &[], &[]);
    let out = screen_run_quiet(
        &s,
        |g| {
            let dir = std::os::unix::ffi::OsStrExt::as_bytes(empty.as_os_str()).to_vec();
            g.file.data_dir_path_len = dir.len();
            g.file.data_dir_path = Some(dir.clone());
            g.file.custom_data_dir_path = Some(dir);
        },
        // SAFETY: plain C call; the fatal error exits through the guard.
        || unsafe { b::JE_playCredits() },
        mi::je_play_credits,
    );
    assert_eq!(out, Err(1));
}

#[derive(Clone, Copy, Debug)]
struct LevelEnd {
    constant: bool,
    bonus: bool,
    alive: bool,
    two: bool,
    one_action: bool,
    cubes: u16,
    timer: bool,
    episode: u8,
    enemies: (u16, u16),
}

#[test]
fn end_level_ani_matches_c() {
    let base = LevelEnd {
        constant: false,
        bonus: false,
        alive: true,
        two: false,
        one_action: false,
        cubes: 0,
        timer: false,
        episode: 1,
        enemies: (0, 0),
    };
    let cases = [
        (base, &[(9000u32, "SPACE")][..]),
        (
            LevelEnd {
                bonus: true,
                enemies: (7, 9),
                ..base
            },
            &[(9000, "SPACE")][..],
        ),
        (
            LevelEnd {
                alive: false,
                cubes: 2,
                enemies: (12, 10),
                ..base
            },
            &[(30000, "SPACE")][..],
        ),
        (
            LevelEnd { cubes: 9, ..base },
            &[(1500, "SPACE"), (4000, "SPACE")][..],
        ),
        (
            LevelEnd {
                two: true,
                timer: true,
                episode: 4,
                ..base
            },
            &[(9000, "SPACE")][..],
        ),
        (
            LevelEnd {
                one_action: true,
                constant: true,
                timer: true,
                episode: 2,
                ..base
            },
            &[][..],
        ),
        (
            LevelEnd {
                constant: true,
                cubes: 1,
                ..base
            },
            &[(500, "SPACE")][..],
        ),
    ];
    for (case, keys) in cases {
        let s = input(keys, &[], &[]);
        let out = screen_run(
            &s,
            |g| {
                g.params.constant_play = case.constant;
                g.episodes.bonus_level = case.bonus;
                g.player.player[0].is_alive = case.alive;
                g.player.player[1].is_alive = true;
                g.config.two_player_mode = case.two;
                g.config.one_player_action = case.one_action;
                g.config.cube_max = case.cubes;
                g.varz.level_timer = case.timer;
                g.varz.level_timer_countdown = 0;
                g.episodes.episode_num = case.episode;
                (g.varz.enemy_killed, g.varz.total_enemy) = case.enemies;
                g.config.initial_difficulty = 2;
                g.player.player[0].items.weapon[0].id = 0; // grants slot (size_t)-1: skipped
                g.player.player[0].items.sidekick = [3, 7]; // 53, 57
                g.player.player[0].items.special = 30; // 110: skipped
            },
            // SAFETY: plain C call on the test thread.
            || unsafe { b::JE_endLevelAni() },
            mi::je_end_level_ani,
        );
        assert_eq!(out, Ok(()), "{case:?}");
    }
}

/// D-UB-SFCODES-SHIP: ship 14 has no row in `shipCombos` (C reads one row
/// past its end); Rust treats it as a ship without combos, which cancels
/// every combo in progress.
#[test]
fn sf_codes_ship_without_combo_row_cancels_combos() {
    let mut rig = Rig::from_fixture("ots 1\n", 500);
    let g = &mut rig.g;
    g.player.player[0].items.ship = 14;
    g.config.super_tyrian = false;
    g.varz.sf_current_code = [[3; 21]; 2];
    g.varz.sf_executed = [0; 2];
    mi::je_sf_codes(g, 1, 10, 10, 10, 5);
    assert_eq!(g.varz.sf_current_code[0][..3], [0, 0, 0]);
    assert_eq!(g.varz.sf_current_code[0][3..], [3; 18]);
    assert_eq!(g.varz.sf_executed, [0, 0]);
}

#[test]
fn out_char_glow_of_empty_text_draws_nothing() {
    let mut rig = Rig::from_fixture("ots 1\n", 500);
    rig.g.nortsong.frame_count_max = 3;
    rig.store();
    rig.clear();
    let empty = std::ffi::CString::new("").unwrap();
    let out = rig.run(
        // SAFETY: plain C call with a C string.
        || unsafe { b::JE_outCharGlow(5, 5, empty.as_ptr()) },
        |g| mi::je_out_char_glow(g, 5, 5, b""),
    );
    assert_eq!(out, Ok(()));
    rig.check();
}

/// Every difficulty band of both score tables, at each initial difficulty.
#[test]
fn adjust_difficulty_thresholds_match_c() {
    let mut rig = Rig::from_fixture("ots 1\n", 500);
    let base = rig.g.snapshot();
    let limits: [u64; 9] = [
        10_000, 20_000, 50_000, 80_000, 125_000, 200_000, 400_000, 600_000, 3_000_000,
    ];
    for two in [false, true] {
        for initial in [1i8, 4, 9] {
            for &limit in &limits {
                for cash in [limit / 2, limit * 2] {
                    rig.g.restore(base.clone());
                    rig.g.config.two_player_mode = two;
                    rig.g.config.initial_difficulty = initial;
                    rig.g.config.difficulty_level = 0;
                    rig.g.player.player[0].cash = cash;
                    rig.g.player.player[1].cash = cash / 3;
                    rig.store();
                    rig.clear();
                    // SAFETY: plain C call.
                    unsafe { b::adjust_difficulty() };
                    mi::adjust_difficulty(&mut rig.g);
                    // SAFETY: a read of a C global on the test thread.
                    let c =
                        unsafe { *rustyrian_oracle::mirror::global(&raw const b::difficultyLevel) };
                    assert_eq!(c, rig.g.config.difficulty_level, "{two} {initial} {cash}");
                }
            }
        }
    }
    rig.check();
}

#[test]
fn next_episode_super_carrot_matches_c() {
    // The last episode jumps back to episode 1: credits (ended by a key),
    // then a 1-in-6 draw grants the SuperCarrot. Seeds are tried in order
    // until one grants it, comparing both engines on every run.
    let keys: Vec<(u32, &str)> = (0..20).map(|i| (2000 + 1000 * i, "SPACE")).collect();
    let s = input(&keys, &[], &[]);
    let mut granted = false;
    for seed in 0..60u64 {
        let mut rig = Rig::from_fixture(&s, 500);
        rig.g.episodes.initial_episode_num = 1;
        rig.g.episodes.episode_num = 4;
        rig.g.params.constant_play = false;
        rig.g.player.player[0].items.weapon[0].id = 1;
        rustyrian_core::mtrand::mt_srand(&mut rig.g, seed);
        rig.store();
        rig.clear();
        // SAFETY: plain C call on the test thread.
        let out = rig.run(|| unsafe { b::JE_nextEpisode() }, mi::je_next_episode);
        assert_eq!(out, Ok(()));
        rig.check();
        let items = rig.g.player.player[0].items;
        granted = items.ship == 2 && items.weapon[0].id == 23 && items.weapon[1].id == 24;
        rig.check_frames();
        if granted {
            break;
        }
    }
    assert!(granted, "no seed below 60 drew the SuperCarrot");
}

/// The opponent's menu with a packet of another type queued first (the wait
/// discards it) and our acknowledgement arriving late (the sync wait loops).
#[test]
fn in_game_setup_netplay_waits_match_c() {
    use rustyrian_core::network::{PACKET_DETAILS, PACKET_KEEP_ALIVE};
    let s = input(&[], &[], &[]);
    let mut rig = net_rig(&s, |g| {
        setup_menu(g);
        g.network.your_in_game_menu_request = false;
    });
    let (in_sync, out_sync) = (rig.g.network.queue_in_sync, rig.g.network.queue_out_sync);
    rig.push(&pkt(PACKET_GAME_MENU, in_sync));
    let mut details = pkt(PACKET_DETAILS, in_sync.wrapping_add(1));
    details.extend_from_slice(&[0, 1, 0, 2]);
    rig.push(&details);
    rig.push(&pkt(PACKET_WAITING, in_sync.wrapping_add(2)));
    for _ in 0..3 {
        rig.push(&pkt(PACKET_KEEP_ALIVE, 0));
    }
    rig.push(&pkt(PACKET_ACKNOWLEDGE, out_sync));
    // SAFETY: plain C call on the test thread.
    let out = rig.run(|| unsafe { b::JE_doInGameSetup() }, mi::je_do_in_game_setup);
    assert_eq!(out, Ok(()));
    rig.check();
}

#[test]
fn play_credits_for_a_minute_matches_c() {
    // Pictures fade in and out and change, ships turn: a minute of the roll.
    let s = input(&[(70_000, "SPACE")], &[], &[]);
    let out = screen_run(
        &s,
        |_| {},
        // SAFETY: plain C call on the test thread.
        || unsafe { b::JE_playCredits() },
        mi::je_play_credits,
    );
    assert_eq!(out, Ok(()));
}

#[test]
fn play_credits_truncated_file_halts_like_c() {
    let dir = mainint_support::tmp("short-credits");
    let data = mainint_support::data_dir();
    for f in ["estsc.shp", "music.mus"] {
        std::fs::copy(data.join(f), dir.join(f)).unwrap();
    }
    let cdt = std::fs::read(data.join("tyrian.cdt")).unwrap();
    std::fs::write(dir.join("tyrian.cdt"), &cdt[..cdt.len() / 3]).unwrap();
    let s = input(&[], &[], &[]);
    let out = screen_run_quiet(
        &s,
        |g| {
            let d = std::os::unix::ffi::OsStrExt::as_bytes(dir.as_os_str()).to_vec();
            g.file.data_dir_path_len = d.len();
            g.file.data_dir_path = Some(d.clone());
            g.file.custom_data_dir_path = Some(d);
        },
        // SAFETY: plain C call; the fatal error exits through the guard.
        || unsafe { b::JE_playCredits() },
        mi::je_play_credits,
    );
    assert_eq!(out, Err(1));
}

/// The screens that need the shop sheet load it when it is missing.
#[test]
fn screens_load_the_shop_sheet_when_missing_match_c() {
    let clear = |g: &mut rustyrian_core::Game| g.sprite.shop_sprite_sheet = Default::default();
    let s = input(&[(1500, "ESCAPE")], &[], &[]);
    assert_eq!(
        screen_run(
            &s,
            clear,
            // SAFETY: plain C call on the test thread.
            || unsafe { b::JE_loadScreen() },
            mi::je_load_screen
        ),
        Ok(false)
    );
    assert_eq!(
        screen_run(
            &s,
            clear,
            // SAFETY: plain C call on the test thread.
            || unsafe { b::JE_highScoreScreen() },
            mi::je_high_score_screen
        ),
        Ok(())
    );
    let menu = |g: &mut rustyrian_core::Game| {
        clear(g);
        setup_menu(g);
    };
    assert_eq!(
        screen_run(
            &s,
            menu,
            // SAFETY: plain C call on the test thread.
            || unsafe { b::JE_inGameSetup() },
            mi::je_in_game_setup
        ),
        Ok(false)
    );
    let escapes: Vec<(u32, &str)> = (0..20).map(|i| (2000 + 1000 * i, "ESCAPE")).collect();
    let s = input(&escapes, &[], &[]);
    let check = |g: &mut rustyrian_core::Game| {
        clear(g);
        high_score_setup(false, 1, [9_000_000, 0])(g);
    };
    assert_eq!(
        screen_run(
            &s,
            check,
            // SAFETY: plain C call on the test thread.
            || unsafe { b::JE_highScoreCheck() },
            mi::je_high_score_check
        ),
        Ok(())
    );
}

/// D-UB-MAININT-ITEMS: item numbers outside their table (damaged saves or
/// demos) read a zeroed entry instead of past the table.
#[test]
fn out_of_range_items_cost_nothing() {
    let mut rig = Rig::from_fixture("ots 1\n", 500);
    let g = &mut rig.g;
    assert!(g.episodes.ships[1].cost > 0, "fixture has item data");
    for t in 2..=8u8 {
        assert_eq!(mi::je_get_value(g, t, 900), 0, "type {t}");
    }
    assert_eq!(mi::je_get_cost(g, 2, 50), 0);
    assert_eq!(mi::je_get_cost(g, 2, 91), 100);
    for t in 3..=8u8 {
        assert_eq!(mi::je_get_cost(g, t, 900), 0, "type {t}");
    }
    let p = &mut g.player.player[0];
    p.cash = 1234;
    p.items.ship = 200;
    p.items.weapon[0].id = 250;
    p.items.weapon[1].id = 250;
    p.items.shield = 250;
    p.items.generator = 250;
    p.items.sidekick = [250, 250];
    assert_eq!(mi::je_total_score(g, 0), 1234);
}

/// D-UB-SPRITE-1: a special whose 2x2 graphic is 0 (sprite index 0, which C
/// reads before `spriteSheet10`) draws nothing and does not panic.
#[test]
fn in_game_displays_special_graphic_zero_draws_nothing() {
    let mut rig = Rig::from_fixture("ots 1\n", 500);
    let g = &mut rig.g;
    let special = (1..g.episodes.special.len())
        .find(|&i| g.episodes.special[i].itemgraphic == 0)
        .unwrap_or_else(|| {
            g.episodes.special[1].itemgraphic = 0;
            1
        });
    g.player.player[0].items.special = 0;
    let base = g.snapshot();
    mi::je_in_game_displays(g);
    let without = g.video.vga().pixels.clone();
    g.restore(base);
    g.player.player[0].items.special = special as u8;
    mi::je_in_game_displays(g);
    // The special only moves the lives display down (y 35 instead of 15);
    // with no lives display (single player) the screens are equal.
    assert!(!g.config.one_player_action && !g.config.two_player_mode);
    assert_eq!(g.video.vga().pixels, without);
}

// ---------------------------------------------------------------------------
// Gameplay: `JE_mainKeyboardInput`, `JE_playerMovement`,
// `JE_mainGamePlayerFunctions` and `JE_playerCollide` on gameplay fixtures.
// ---------------------------------------------------------------------------

use rustyrian_core::varz::Sprite2Ref;
use rustyrian_oracle::mirror::varz::sprite2_ref_to_c;

/// C's `&player[p]`.
fn c_player(p: usize) -> *mut b::Player {
    // SAFETY: an in-bounds element address of the C array; no reference.
    unsafe { (&raw mut b::player).cast::<b::Player>().add(p) }
}

/// One game frame of the player functions, as `JE_main` calls them:
/// keyboard commands, movement of every player, then collisions.
fn player_frame_both(rig: &mut Rig) -> mainint_support::Outcome<()> {
    let two = rig.g.config.two_player_mode;
    rig.run(
        || {
            // SAFETY: plain C calls on the stored game state.
            unsafe {
                b::JE_mainKeyboardInput();
                b::JE_mainGamePlayerFunctions();
                b::JE_playerCollide(c_player(0), 1);
                if two {
                    b::JE_playerCollide(c_player(1), 2);
                }
            }
        },
        |g| {
            mi::je_main_keyboard_input(g)?;
            mi::je_main_game_player_functions(g)?;
            mi::je_player_collide(g, 0, 1)?;
            if two {
                mi::je_player_collide(g, 1, 2)?;
            }
            Ok(())
        },
    )
}

/// Runs `frames` player frames from a demo fixture (the demo drives player
/// 1 through `playDemoKeys`), comparing the players every frame and the
/// whole game at the end.
fn demo_frames(frame: u64, frames: usize) {
    let mut rig = Rig::from_fixture("ots 1\n", frame);
    assert!(rig.g.demo.play_demo, "the fixture plays a demo");
    for step in 0..frames {
        player_frame_both(&mut rig).expect("no exit");
        assert_eq!(
            diff::load::<rustyrian_core::player::State>(),
            rig.g.player,
            "frame {step}"
        );
    }
    rig.check();
}

#[test]
fn player_functions_follow_demo_500_match_c() {
    demo_frames(500, 300);
}

#[test]
fn player_functions_follow_demo_2000_match_c() {
    demo_frames(2000, 300);
}

/// Equal platform calls and whole games, naming the first differing field.
fn same_game(rig: &mut Rig) -> Result<(), TestCaseError> {
    prop_assert_eq!(rig.c_calls(), rig.rust_calls(), "platform calls differ");
    if let Some(d) = diff::first_difference(&rig.c_state(), &rig.g.snapshot()) {
        return Err(TestCaseError::fail(format!("states differ at {d}")));
    }
    Ok(())
}

/// One player's setup for the movement tests.
#[derive(Clone, Debug)]
struct Pilot {
    alive: bool,
    exploding: u32,
    pos: (i32, i32),
    mouse: (u16, u16),
    vel: (i32, i32),
    friction: (u32, u32),
    invulnerable: u32,
    /// `(id, power)` of the front and rear weapons; `power` of weapon `p`
    /// is also player `p`'s lives (`mainint.c:1053`).
    weapons: [(u8, u8); 2],
    mode: u32,
    /// Sidekick items; their style is the option's `tr`, as `JE_drawOptions`
    /// sets it (another style draws the graphics from the other sheet,
    /// past its end).
    sidekicks: [u8; 2],
    /// `(ammo_max, ammo, refill ticks, refill ticks max)`.
    ammo: [(i32, i32, u32, u32); 2],
    /// `(animation enabled, frame, charge, charge ticks)`; the frame and the
    /// charge are reduced below the option's `ani` and `pwr`.
    anim: [(bool, u32, u32, u32); 2],
    superbombs: u32,
    /// `(shield, shield_max, armor, initial_armor)`.
    hull: (u32, u32, u32, u32),
}

/// A pilot; `rear` bounds the rear weapon (player 2's indexes
/// `linkGunWeapons`/`chargeGunWeapons[id-1]`, 38 entries).
fn any_pilot(rear: std::ops::RangeInclusive<u8>) -> impl Strategy<Value = Pilot> {
    let side = 0u8..=OPTION_NUM as u8;
    let ammo = (0i32..=12).prop_flat_map(|max| (Just(max), 0..=max, 0u32..=2, 0u32..=3));
    let anim = (any::<bool>(), any::<u32>(), any::<u32>(), 0u32..=2);
    (
        (
            prop::bool::weighted(0.8),
            0u32..=2,
            (30i32..=270, 0i32..=175),
            (30u16..=270, 0u16..=175),
            (-6i32..=6, -6i32..=6),
            (0u32..=2, 0u32..=2),
            prop_oneof![3 => Just(0u32), 1 => 1u32..=2],
        ),
        (
            ((0u8..=PORT_NUM as u8, 1u8..=11), (rear, 1u8..=11)),
            1u32..=2,
        ),
        (
            prop::array::uniform2(side),
            prop::array::uniform2(ammo),
            prop::array::uniform2(anim),
        ),
        (0u32..=2, (0u32..=14, 10u32..=14, 0u32..=28, 1u32..=28)),
    )
        .prop_map(
            |(
                (alive, exploding, pos, mouse, vel, friction, invulnerable),
                ((front, rear), mode),
                (sidekicks, ammo, anim),
                (superbombs, hull),
            )| Pilot {
                alive,
                exploding,
                pos,
                mouse,
                vel,
                friction,
                invulnerable,
                weapons: [front, rear],
                mode,
                sidekicks,
                ammo,
                anim,
                superbombs,
                hull,
            },
        )
}

fn apply_pilot(g: &mut rustyrian_core::Game, p: usize, s: &Pilot) {
    let options = g.episodes.options;
    let pl = &mut g.player.player[p];
    pl.is_alive = s.alive;
    pl.exploding_ticks = s.exploding;
    (pl.x, pl.y) = s.pos;
    (pl.mouse_x, pl.mouse_y) = s.mouse;
    (pl.x_velocity, pl.y_velocity) = s.vel;
    (pl.x_friction_ticks, pl.y_friction_ticks) = s.friction;
    pl.invulnerable_ticks = s.invulnerable;
    for (w, &(id, power)) in s.weapons.iter().enumerate() {
        pl.items.weapon[w].id = id;
        pl.items.weapon[w].power = power;
    }
    pl.lives = Some(p);
    pl.weapon_mode = s.mode;
    for i in 0..2 {
        let item = s.sidekicks[i];
        let option = options[usize::from(item)];
        pl.items.sidekick[i] = item;
        let sk = &mut pl.sidekick[i];
        sk.style = u32::from(option.tr);
        (
            sk.ammo_max,
            sk.ammo,
            sk.ammo_refill_ticks,
            sk.ammo_refill_ticks_max,
        ) = s.ammo[i];
        let (enabled, frame, charge, ticks) = s.anim[i];
        sk.animation_enabled = enabled;
        // Frames past `ani` would draw unused (zero) graphics, which C
        // reads before the sheet (D-UB-SPRITE-1).
        sk.animation_frame = frame % u32::from(option.ani.max(1));
        sk.charge = charge % (u32::from(option.pwr) + 1);
        sk.charge_ticks = ticks;
    }
    pl.superbombs = s.superbombs;
    (pl.shield, pl.shield_max, pl.armor, pl.initial_armor) = s.hull;
}

/// Has mouse, three buttons, buttons down, relative motion x and y.
type MouseSetup = (bool, bool, u8, i32, i32);
/// Charge level, max, wait, graphic, graphic wait.
type ChargeSetup = (u8, u8, u8, u8, u8);

/// The rest of a movement scene.
#[derive(Clone, Debug)]
struct Scene {
    /// two players, galaga, linked, one-player arcade, super arcade mode
    mode: (bool, bool, bool, bool, u8),
    /// end level, levelEnd, levelEndWarp, levelEndFxWait, allPlayersGone
    level: (bool, u8, i8, u16, bool),
    /// constantDie, constantPlay, constantLastX
    cheats: (bool, bool, i8),
    /// playDemo, game keys, input devices, mouse
    input: (bool, [bool; 8], [u8; 2], MouseSetup),
    /// smoothie 9, background2, superWild, mapX2Ofs, ship graphics,
    /// background3x1
    look: (bool, bool, bool, u16, [u8; 2], bool),
    /// portConfigDone, shotRepeat, charge, difficulty, special
    weapons: (bool, [u8; 11], ChargeSetup, i8, u8),
    /// optionAttachmentMove, -Linked, -Return, optionSatelliteRotate,
    /// linkGunDirec
    attach: (i16, bool, bool, f32, f32),
    pilots: [Pilot; 2],
    frames: usize,
}

fn any_scene() -> impl Strategy<Value = Scene> {
    (
        (
            any::<bool>(),
            prop::bool::weighted(0.2),
            any::<bool>(),
            any::<bool>(),
            prop_oneof![3 => Just(0u8), 1 => 1u8..=8],
        ),
        (
            prop::bool::weighted(0.25),
            prop_oneof![Just(0u8), 1u8..=45],
            -3i8..=8,
            0u16..=2,
            any::<bool>(),
        ),
        (
            prop::bool::weighted(0.15),
            prop::bool::weighted(0.15),
            -2i8..=2,
        ),
        (
            prop::bool::weighted(0.3),
            prop::array::uniform8(any::<bool>()),
            prop::array::uniform2(0u8..=2),
            (
                any::<bool>(),
                any::<bool>(),
                0u8..8,
                -60i32..=60,
                -60i32..=60,
            ),
        ),
        (
            prop::bool::weighted(0.2),
            any::<bool>(),
            any::<bool>(),
            0u16..=72,
            prop::array::uniform2(0u8..=6),
            any::<bool>(),
        ),
        (
            any::<bool>(),
            prop::array::uniform11(prop_oneof![2 => Just(0u8), 1 => 1u8..=3]),
            (0u8..=5, 0u8..=5, 0u8..=2, 0u8..=3, 0u8..=1),
            1i8..=6,
            0u8..=SPECIAL_NUM as u8,
        ),
        (
            -25i16..=8,
            any::<bool>(),
            any::<bool>(),
            0f32..7.0,
            0f32..6.3,
        ),
        (any_pilot(0..=PORT_NUM as u8), any_pilot(1..=38)),
        1usize..=3,
    )
        .prop_map(
            |(mode, level, cheats, input, look, weapons, attach, (p0, p1), frames)| Scene {
                mode,
                level,
                cheats,
                input,
                look,
                weapons,
                attach,
                pilots: [p0, p1],
                frames,
            },
        )
}

fn apply_scene(g: &mut rustyrian_core::Game, s: &Scene) {
    for p in 0..2 {
        apply_pilot(g, p, &s.pilots[p]);
    }
    let c = &mut g.config;
    (
        c.two_player_mode,
        c.galaga_mode,
        c.two_player_linked,
        c.one_player_action,
        c.super_arcade_mode,
    ) = s.mode;
    let v = &mut g.varz;
    (
        v.end_level,
        v.level_end,
        v.level_end_warp,
        v.level_end_fx_wait,
        v.all_players_gone,
    ) = s.level;
    (
        g.params.constant_die,
        g.params.constant_play,
        g.mainint.constant_last_x,
    ) = s.cheats;

    let (play_demo, keys, devices, (has_mouse, three, buttons, rx, ry)) = s.input;
    // Without a demo, `playDemoKeys` would read a closed file.
    g.demo.play_demo &= play_demo;
    for (i, &down) in keys.iter().enumerate() {
        let k = g.config.key_settings[i] as usize;
        g.keyboard.keysactive[k] = down;
    }
    g.config.input_device = devices;
    g.mouse.has_mouse = has_mouse;
    g.mouse.mouse_has_three_buttons = three;
    g.keyboard.mouse_buttons_down = buttons;
    g.keyboard.mouse_window_x_relative = rx;
    g.keyboard.mouse_window_y_relative = ry;

    let (smoothie, background2, wild, map_x2_ofs, ships, bg3x1) = s.look;
    g.config.smoothies[8] = smoothie;
    g.config.background2 = background2;
    g.config.super_wild = wild;
    g.backgrnd.map_x2_ofs = map_x2_ofs;
    g.varz.background3x1 = bg3x1;
    let graphic = |g: &rustyrian_core::Game, k: u8| match k {
        0 | 1 => u16::from(k),
        k => g.episodes.ships[usize::from(k)].shipgraphic,
    };
    g.varz.ship_gr = graphic(g, ships[0]);
    g.varz.ship_gr2 = graphic(g, ships[1]);
    g.varz.ship_gr_ptr = Some(Sprite2Ref::SpriteSheet9);
    g.varz.ship_gr2ptr = Some(Sprite2Ref::SpriteSheet9);

    let (done, repeat, (level, max, wait, gr, gr_wait), difficulty, special) = s.weapons;
    g.config.port_config_done = done;
    g.config.shot_repeat = repeat;
    let v = &mut g.varz;
    (
        v.charge_level,
        v.charge_max,
        v.charge_wait,
        v.charge_gr,
        v.charge_gr_wait,
    ) = (level, max, wait, gr, gr_wait);
    g.config.difficulty_level = difficulty;
    g.player.player[0].items.special = special;

    let (attach_move, linked, ret, rotate, direc) = s.attach;
    let v = &mut g.varz;
    (
        v.option_attachment_move,
        v.option_attachment_linked,
        v.option_attachment_return,
        v.option_satellite_rotate,
    ) = (attach_move, linked, ret, rotate);
    g.config.link_gun_direc = direc;
}

/// Hard-capped fixture runner: `cases` cases from a fixed seed (the
/// coverage of these functions must be reproducible).
fn run_scenes<S: Strategy>(
    strategy: S,
    frame: u64,
    cases: u32,
    test: impl Fn(&mut Rig, S::Value) -> Result<(), TestCaseError>,
) where
    S::Value: std::fmt::Debug,
{
    let rig = RefCell::new(Rig::from_fixture("ots 1\n", frame));
    let base = rig.borrow().g.snapshot();
    let config = RunnerConfig {
        cases,
        ..RunnerConfig::default()
    };
    let rng = proptest::test_runner::TestRng::deterministic_rng(config.rng_algorithm);
    TestRunner::new_with_rng(config, rng)
        .run(&strategy, |v| {
            let mut rig = rig.borrow_mut();
            rig.g.restore(base.clone());
            rig.store();
            rig.clear();
            test(&mut rig, v)
        })
        .unwrap();
}

/// Movement of one or two players over a few frames from a wide range of
/// situations: alive, exploding, respawning, dying for ever, at the level
/// end, linked, every input device without joysticks, every ship graphic
/// kind, sidekick styles, charge and super bomb weapons.
#[test]
fn player_movement_matches_c() {
    run_scenes(any_scene(), 500, 48, |rig, scene| {
        apply_scene(&mut rig.g, &scene);
        rig.store();
        for _ in 0..scene.frames {
            let out = player_frame_both(rig);
            prop_assert_eq!(out, Ok(()));
        }
        same_game(rig)
    });
}

/// A living player at rest with a front and rear weapon and no sidekicks.
fn quiet_pilot() -> Pilot {
    Pilot {
        alive: true,
        exploding: 0,
        pos: (150, 100),
        mouse: (150, 100),
        vel: (0, 0),
        friction: (0, 0),
        invulnerable: 0,
        weapons: [(1, 2), (1, 2)],
        mode: 1,
        sidekicks: [0, 0],
        ammo: [(0, 0, 0, 0); 2],
        anim: [(false, 0, 0, 20); 2],
        superbombs: 0,
        hull: (5, 10, 10, 10),
    }
}

/// One player in a level with the keyboard as the input device.
fn quiet_scene() -> Scene {
    Scene {
        mode: (false, false, false, false, 0),
        level: (false, 0, 0, 0, false),
        cheats: (false, false, 0),
        input: (false, [false; 8], [1, 1], (false, false, 0, 0, 0)),
        look: (false, false, false, 0, [2, 2], false),
        weapons: (false, [0; 11], (0, 0, 0, 0, 0), 2, 0),
        attach: (0, false, false, 0.0, 0.0),
        pilots: [quiet_pilot(), quiet_pilot()],
        frames: 2,
    }
}

/// Game keys, by `KEY_SETTING_*` index.
const UP: usize = 0;
const DOWN: usize = 1;
const RIGHT: usize = 3;
const FIRE: usize = 4;
const CHANGE_FIRE: usize = 5;
const LEFT_SIDEKICK: usize = 6;

/// The first option with `pred`.
fn option_where(
    g: &rustyrian_core::Game,
    pred: impl Fn(&rustyrian_core::episodes::JeOptionType) -> bool,
) -> u8 {
    let i = g
        .episodes
        .options
        .iter()
        .position(pred)
        .expect("such an option");
    i as u8
}

/// Gives player 1 sidekick `item` on side `i` as `JE_drawOptions` sets it
/// up, with `charge`.
fn give_sidekick(g: &mut rustyrian_core::Game, i: usize, item: u8, charge: u32) {
    let option = g.episodes.options[usize::from(item)];
    let pl = &mut g.player.player[0];
    pl.items.sidekick[i] = item;
    let sk = &mut pl.sidekick[i];
    sk.style = u32::from(option.tr);
    sk.ammo_max = i32::from(option.ammo);
    sk.ammo = sk.ammo_max;
    sk.charge = charge;
}

type SceneEdit = fn(&mut Scene);
type GameEdit = fn(&mut rustyrian_core::Game);

/// Situations the generated scenes reach rarely: the last life (galaga,
/// network), dying for ever with no armor, mouse motion past the clamp,
/// the linked gun wrapping past 2π, a respawn that leaves every player
/// dead at the level end, the Nort ship's sparks, the super-arcade special
/// swap, orbiting and charged sidekicks.
#[test]
fn player_movement_edge_cases_match_c() {
    let cases: [(&str, SceneEdit, GameEdit); 11] = [
        (
            "last life, galaga",
            |s| {
                s.mode = (true, true, false, false, 0);
                s.pilots[0].alive = false;
                s.pilots[0].weapons[0].1 = 1;
            },
            |_| {},
        ),
        (
            "last life, network, all gone",
            |s| {
                s.mode = (true, false, false, false, 0);
                s.level.4 = true;
                s.pilots[0].alive = false;
                s.pilots[0].weapons[0].1 = 1;
            },
            |g| {
                g.network.is_network_game = true;
                g.network.this_player_num = 2;
                g.network.last_state_out_sync = 0;
            },
        ),
        (
            "dying for ever without armor",
            |s| {
                s.cheats.0 = true;
                s.pilots[0].hull.2 = 0;
            },
            |_| {},
        ),
        (
            "mouse past the clamp",
            |s| s.input.3 = (true, false, 0, -200, -200),
            |g| g.config.input_device = [0, 0],
        ),
        (
            "linked gun wraps",
            |s| {
                s.mode = (true, false, true, false, 0);
                s.attach.4 = 6.2;
                s.input.1[DOWN] = true;
                s.input.1[FIRE] = true;
                s.pilots[1].weapons[1].0 = 5;
            },
            |_| {},
        ),
        (
            "respawn leaves all dead at the level end",
            |s| {
                s.mode = (true, true, false, false, 0);
                s.level.1 = 5;
                s.pilots[0].alive = false;
                s.pilots[0].exploding = 2;
                s.pilots[1].alive = false;
                s.pilots[1].weapons[1].1 = 3;
            },
            |_| {},
        ),
        (
            "nort ship banking hard",
            |s| {
                s.look.4 = [1, 2];
                s.pilots[0].vel = (6, 0);
                s.input.1[RIGHT] = true;
            },
            |_| {},
        ),
        (
            "super arcade special swap",
            |s| {
                s.mode.4 = 2;
                s.weapons.0 = true;
                s.input.1[CHANGE_FIRE] = true;
            },
            |g| g.player.player[0].items.special = rustyrian_core::varz::SA_SPECIAL_WEAPON[1] as u8,
        ),
        (
            "two orbiting sidekicks",
            |s| s.input.1[UP] = true,
            |g| {
                let orbit = option_where(g, |o| o.tr == 4);
                give_sidekick(g, 0, orbit, 0);
                give_sidekick(g, 1, orbit, 0);
            },
        ),
        (
            "charged limited-ammo sidekick fires, right one orbits",
            |s| s.input.1[LEFT_SIDEKICK] = true,
            |g| {
                let orbit = option_where(g, |o| o.tr == 4);
                // No freeware limited-ammo option charges (`pwr` 0): the
                // charge comes from the test.
                let limited = option_where(g, |o| o.ammo > 0 && o.wport > 0);
                give_sidekick(g, 0, limited, 1);
                give_sidekick(g, 1, orbit, 0);
            },
        ),
        (
            "charged unlimited sidekick fires",
            |s| s.input.1[FIRE] = true,
            |g| {
                let unlimited = option_where(g, |o| o.ammo == 0 && o.wport > 0 && o.pwr > 0);
                give_sidekick(g, 0, unlimited, 1);
            },
        ),
    ];
    let mut rig = Rig::from_fixture("ots 1\n", 500);
    let base = rig.g.snapshot();
    for (name, scene_edit, game_edit) in cases {
        rig.g.restore(base.clone());
        let mut scene = quiet_scene();
        scene_edit(&mut scene);
        apply_scene(&mut rig.g, &scene);
        game_edit(&mut rig.g);
        rig.store();
        rig.clear();
        for _ in 0..scene.frames {
            assert_eq!(player_frame_both(&mut rig), Ok(()), "{name}");
        }
        if let Err(e) = same_game(&mut rig) {
            panic!("{name}: {e}");
        }
    }
}

/// An enemy (or pick-up) placed relative to the colliding player.
#[derive(Clone, Debug)]
struct Contact {
    slot: usize,
    evalue: i16,
    /// 0 used, 2 secret pick-up.
    avail: u8,
    scoreitem: bool,
    armorleft: u8,
    linknum: u8,
    push: (i8, i8),
    enemytype: u16,
    /// `(dx, dy, mapoffset)`: offset from the player; within (11, 13) it
    /// collides.
    at: (i32, i32, u16),
    ground: (bool, u8),
}

/// Pick-up values of every kind, kept inside the tables C indexes with
/// them: in super arcade mode every value above 30000 is a super-arcade
/// weapon (`SAWeapon[mode-1][value-30001]`, 5 per mode); otherwise
/// `optionSelect` series < 16, and the special, weapon and
/// `specialArcadeWeapon` tables.
fn any_evalue(sam: u8) -> BoxedStrategy<i16> {
    let common = prop_oneof![
        Just(30000i16),
        20001i16..=29999,
        10001i16..=10255,
        prop::sample::select(vec![1i16, -1, -2, -3, -4, -5]),
        -500i16..=10000,
    ];
    if sam != 0 {
        return prop_oneof![1 => 30001i16..=30005, 3 => common].boxed();
    }
    prop_oneof![
        30001i16..=30000 + PORT_NUM as i16,
        31001i16..=31000 + PORT_NUM as i16,
        32001i16..=32015,
        32101i16..=32100 + SPECIAL_NUM as i16,
        common,
    ]
    .boxed()
}

fn any_contact(sam: u8) -> impl Strategy<Value = Contact> {
    (
        0usize..100,
        any_evalue(sam),
        prop_oneof![3 => Just(0u8), 1 => Just(2u8)],
        any::<bool>(),
        prop_oneof![Just(255u8), any::<u8>()],
        prop_oneof![Just(0u8), 1u8..=4, 38u8..=110, any::<u8>()],
        (-3i8..=3, -3i8..=3),
        1u16..=850,
        (-15i32..=15, -16i32..=16, 0u16..=30),
        (any::<bool>(), 0u8..=13),
    )
        .prop_map(
            |(slot, evalue, avail, scoreitem, armorleft, linknum, push, enemytype, at, ground)| {
                Contact {
                    slot,
                    evalue,
                    avail,
                    scoreitem,
                    armorleft,
                    linknum,
                    push,
                    enemytype,
                    at,
                    ground,
                }
            },
        )
}

/// A collision scene: flags, the colliding player, and the enemies.
#[derive(Clone, Debug)]
struct Crash {
    /// two players, linked, galaga, one-player arcade, network, bonus
    /// level, cheating
    flags: [bool; 7],
    sam: u8,
    player_num: u8,
    /// `(x, y, invulnerable, shield, armor, superbombs)`.
    pilot: (i32, i32, u32, u32, u32, u32),
    /// Weapon `(id, power)` of both players.
    items: [[(u8, u8); 2]; 2],
    /// Player 2's sidekick series and level.
    series: (u8, u8),
    damage_rate: u8,
    contacts: Vec<Contact>,
}

fn any_crash() -> impl Strategy<Value = Crash> {
    let weapon = (0u8..=PORT_NUM as u8, 1u8..=11);
    (
        prop::array::uniform7(any::<bool>()),
        prop_oneof![2 => Just(0u8), 1 => 1u8..=7],
        1u8..=2,
        (
            40i32..=256,
            10i32..=160,
            prop_oneof![3 => Just(0u32), 1 => 1u32..=3],
            0u32..=10,
            0u32..=28,
            0u32..=10,
        ),
        prop::array::uniform2(prop::array::uniform2(weapon)),
        (0u8..=15, 101u8..=103),
        1u8..=40,
    )
        .prop_flat_map(
            |(flags, sam, player_num, pilot, items, series, damage_rate)| {
                (
                    Just((flags, sam, player_num, pilot, items, series, damage_rate)),
                    prop::collection::vec(any_contact(sam), 1..6),
                )
            },
        )
        .prop_map(
            |((flags, sam, player_num, pilot, items, series, damage_rate), contacts)| Crash {
                flags,
                sam,
                player_num,
                pilot,
                items,
                series,
                damage_rate,
                contacts,
            },
        )
}

fn apply_crash(g: &mut rustyrian_core::Game, s: &Crash) {
    let [two, linked, galaga, one, net, bonus, cheat] = s.flags;
    let c = &mut g.config;
    (
        c.two_player_mode,
        c.two_player_linked,
        c.galaga_mode,
        c.one_player_action,
        c.super_arcade_mode,
        c.you_are_cheating,
    ) = (two, linked, galaga, one, s.sam, cheat);
    g.network.is_network_game = net;
    g.network.network_player_name = Some(b"Ann".to_vec());
    g.network.network_opponent_name = Some(b"Bob".to_vec());
    g.episodes.bonus_level = bonus;
    g.varz.damage_rate = s.damage_rate;
    for p in 0..2 {
        let pl = &mut g.player.player[p];
        pl.lives = Some(p);
        for w in 0..2 {
            (pl.items.weapon[w].id, pl.items.weapon[w].power) = s.items[p][w];
        }
    }
    let items = &mut g.player.player[1].items;
    (items.sidekick_series, items.sidekick_level) = s.series;
    let pl = &mut g.player.player[usize::from(s.player_num) - 1];
    let (x, y, invulnerable, shield, armor, superbombs) = s.pilot;
    (pl.x, pl.y, pl.invulnerable_ticks) = (x, y, invulnerable);
    (pl.shield, pl.armor, pl.superbombs) = (shield, armor, superbombs);
    pl.is_alive = true;

    // Only the scene's enemies are on screen.
    g.varz.enemy_avail = [1; 100];
    for e in &s.contacts {
        g.varz.enemy_avail[e.slot] = e.avail;
        // Explosion types past `explosion_data` (D-UB-VARZ-1) belong to no
        // placed enemy.
        let mut t = e.enemytype;
        while g.episodes.enemy_dat[usize::from(t)].explosiontype >= 53 {
            t = t % 850 + 1;
        }
        let en = &mut g.varz.enemy[e.slot];
        let (dx, dy, offset) = e.at;
        en.mapoffset = offset;
        en.ex = (x + dx - i32::from(offset)) as i16;
        en.ey = (y + dy) as i16;
        en.evalue = e.evalue;
        en.scoreitem = e.scoreitem;
        en.armorleft = e.armorleft;
        en.linknum = e.linknum;
        (en.exc, en.eyc) = e.push;
        en.enemytype = t;
        (en.enemyground, en.explonum) = e.ground;
    }
}

/// Pick-ups of every kind and collisions (damage, push-back, linked kills)
/// for either player.
#[test]
fn player_collide_matches_c() {
    run_scenes(any_crash(), 500, 64, |rig, crash| {
        let mut crash = crash;
        for e in &mut crash.contacts {
            // `shotAvail[z] = 0` for the orbiting asteroid killer (-3)
            // writes past `shotAvail` from enemy slot 81 on
            // (D-UB-MAININT-SHOTAVAIL).
            if e.evalue == -3 {
                e.slot %= rustyrian_core::shots::MAX_PWEAPON;
            }
        }
        apply_crash(&mut rig.g, &crash);
        rig.store();
        let p = usize::from(crash.player_num) - 1;
        let num = crash.player_num;
        let out = rig.run(
            // SAFETY: plain C call on the stored game state.
            || unsafe { b::JE_playerCollide(c_player(p), num) },
            |g| mi::je_player_collide(g, p, num),
        );
        prop_assert_eq!(out, Ok(()));
        same_game(rig)
    });
}

/// The keys `JE_mainKeyboardInput` reads besides Backspace, Num Lock and P
/// (F1 opens the help screen; see `main_keyboard_input_help_matches_c`).
const COMMAND_KEYS: [usize; 22] = [
    43, 57, 30, 31, 34, 38, 39, 59, 60, 61, 62, 63, 64, 65, 66, 67, 69, 49, 56, 6, 45, 71,
];

/// Custom ships whose items lie inside the tables (`JE_SGr` indexes
/// `GR[graphic-1]`, `varz.c:381`).
fn any_extra_ships() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(
        (
            1u8..=15,
            (0u8..=PORT_NUM as u8, 0u8..=PORT_NUM as u8),
            0u8..=SPECIAL_NUM as u8,
            prop::array::uniform2(0u8..=OPTION_NUM as u8),
            0u8..=POWER_NUM as u8,
            any::<u8>(),
            0u8..=SHIELD_NUM as u8,
        ),
        10,
    )
    .prop_map(|rows| {
        let mut v = vec![0u8; 154];
        for (k, (gr, (front, rear), special, side, generator, armor, shield)) in
            rows.into_iter().enumerate()
        {
            let r = &mut v[k * 15..k * 15 + 15];
            (r[0], r[1], r[2], r[3]) = (gr, front, rear, special);
            (r[4], r[5], r[6], r[7], r[8]) = (side[0], side[1], generator, armor, shield);
        }
        v
    })
}

/// Every command key combination outside the help key: edited ships for
/// either player, the Nort ship, cheats, debug, level skip, pause and menu
/// requests, smoothies, filters, game speed and random music.
#[test]
fn main_keyboard_input_matches_c() {
    let strategy = (
        prop::collection::vec(prop::sample::select(&COMMAND_KEYS[..]), 0..7),
        prop::array::uniform4(any::<bool>()),
        (
            prop::bool::weighted(0.2),
            prop::bool::weighted(0.25),
            prop::bool::weighted(0.2),
            prop_oneof![3 => Just(0u8), 1 => 1u8..=7],
            any::<bool>(),
        ),
        (
            prop::bool::weighted(0.7),
            any_extra_ships(),
            prop::array::uniform2(any::<bool>()),
        ),
        (
            prop_oneof![Just(-99i8), -1i8..=15],
            0u8..=2,
            any::<bool>(),
            1u32..=3,
        ),
    );
    run_scenes(
        strategy,
        500,
        64,
        |rig,
         (
            keys,
            [backspace, numlock, p, escape],
            (net, two, super_tyrian, sam, constant),
            (extra, ships, edits),
            (filter, fast, pause, mode),
        )| {
            let g = &mut rig.g;
            for k in keys {
                g.keyboard.keysactive[k] = true;
            }
            g.keyboard.keysactive[42] = backspace;
            g.keyboard.keysactive[83] = numlock;
            g.keyboard.keysactive[19] = p;
            g.keyboard.keysactive[41] = escape;
            g.network.is_network_game = net;
            g.config.two_player_mode = two;
            g.config.super_tyrian = super_tyrian;
            g.config.super_arcade_mode = sam;
            g.params.constant_play = constant;
            g.editship.extra_avail = extra;
            g.editship.extra_ships.copy_from_slice(&ships);
            (g.varz.edit_ship1, g.varz.edit_ship2) = (edits[0], edits[1]);
            g.config.level_filter = filter;
            g.config.fast_play = fast;
            g.config.super_pause = pause;
            for pl in g.player.player.iter_mut() {
                pl.weapon_mode = mode;
            }
            rig.store();
            let out = rig.run(
                // SAFETY: plain C call on the stored game state.
                || unsafe { b::JE_mainKeyboardInput() },
                mi::je_main_keyboard_input,
            );
            prop_assert_eq!(out, Ok(()));
            same_game(rig)
        },
    );
}

/// F1 opens the in-game help (a key closes it); in a network game it only
/// requests it.
#[test]
fn main_keyboard_input_help_matches_c() {
    for net in [false, true] {
        let s = input(&[(2500, "SPACE")], &[], &[(1500, "mouse-move 100 100")]);
        let (out, frames) = screen_run_frames(
            &s,
            |g| {
                g.keyboard.keysactive[58] = true;
                g.network.is_network_game = net;
            },
            // SAFETY: plain C call on the test thread.
            || unsafe { b::JE_mainKeyboardInput() },
            mi::je_main_keyboard_input,
        );
        assert_eq!(out, Ok(()));
        assert_eq!(frames > 0, !net, "the help screen presents");
    }
}

/// `JE_playerMovement` on both engines for player `p` as
/// `JE_mainGamePlayerFunctions` passes it, with an explicit input device.
fn movement_both(rig: &mut Rig, p: usize, device: u8) -> mainint_support::Outcome<()> {
    let (num, gr, ptr) = if p == 0 {
        (1, rig.g.varz.ship_gr, rig.g.varz.ship_gr_ptr)
    } else {
        (2, rig.g.varz.ship_gr2, rig.g.varz.ship_gr2ptr)
    };
    rig.run(
        || {
            let pl = c_player(p);
            // SAFETY: plain C call; the pointers address the C player.
            unsafe {
                b::JE_playerMovement(
                    pl,
                    device,
                    num,
                    gr,
                    sprite2_ref_to_c(ptr),
                    &raw mut (*pl).mouseX,
                    &raw mut (*pl).mouseY,
                );
            }
        },
        |g| mi::je_player_movement(g, p, device, num, gr, ptr),
    )
}

/// Two pads (axes pushed, buttons held), digital and analog, as one
/// player's device, each player's device, and every pad at once; player 2
/// aims the linked gun with its stick.
#[test]
fn player_movement_joystick_matches_c() {
    let script = "ots 1\n\
        joystick \"Pad 0\" 2 6 0\n\
        joystick \"Pad 1\" 2 6 0\n\
        @0 joy-axis 0 0 30000\n\
        @0 joy-axis 0 1 -20000\n\
        @0 joy-button 0 0 1\n\
        @0 joy-button 0 1 1\n\
        @0 joy-button 0 2 1\n\
        @0 joy-axis 1 0 -32000\n\
        @0 joy-axis 1 1 25000\n\
        @0 joy-button 1 0 1\n\
        @0 joy-button 1 3 1\n\
        @0 joy-button 1 4 1\n\
        @0 joy-button 1 5 1\n";
    for (analog, two, linked, devices) in [
        ([false, false], false, false, [0, 0]),
        ([true, true], false, false, [0, 0]),
        ([false, true], true, false, [3, 4]),
        ([true, false], true, false, [4, 3]),
        ([false, true], true, true, [3, 4]),
        ([true, true], true, true, [1, 4]),
    ] {
        let mut rig = Rig::from_fixture(script, 500);
        let out = rig.run(
            // SAFETY: plain C call.
            || unsafe { b::init_joysticks() },
            |g| {
                rustyrian_core::joystick::init_joysticks(g);
                Ok(())
            },
        );
        assert_eq!(out, Ok(()));
        assert_eq!(rig.g.joystick.joysticks, 2);
        let g = &mut rig.g;
        for (j, a) in analog.into_iter().enumerate() {
            g.joystick.joystick.as_mut().expect("open")[j].analog = a;
        }
        g.demo.play_demo = false;
        g.config.two_player_mode = two;
        g.config.two_player_linked = linked;
        g.config.galaga_mode = false;
        g.config.input_device = devices;
        for p in 0..2 {
            let pl = &mut g.player.player[p];
            pl.is_alive = true;
            (pl.x, pl.y) = (150, 100);
            pl.items.weapon[REAR].id = pl.items.weapon[REAR].id.clamp(1, 38);
        }
        g.varz.ship_gr2 = 0;
        g.varz.ship_gr2ptr = Some(Sprite2Ref::SpriteSheet9);
        rig.store();
        rig.clear();
        for _ in 0..3 {
            assert_eq!(player_frame_both(&mut rig), Ok(()));
        }
        // An explicit device per call (the joystick of player 1, then the
        // mouse and the keyboard only).
        for device in [3, 2, 1] {
            assert_eq!(movement_both(&mut rig, 0, device), Ok(()));
        }
        rig.check();
    }
}

const REAR: usize = rustyrian_core::player::REAR_WEAPON;

/// A state packet for `JE_playerMovement`'s network reads.
fn state_packet(data: &[u8]) -> rustyrian_core::platform::UdpPacket {
    let mut d = vec![0u8; rustyrian_core::network::NET_PACKET_SIZE as usize];
    d[..data.len()].copy_from_slice(data);
    rustyrian_core::platform::UdpPacket {
        channel: 0,
        data: d,
        len: 28,
        maxlen: rustyrian_core::network::NET_PACKET_SIZE,
        status: 0,
        address: Default::default(),
    }
}

/// Network games: this PC's player writes its state packet (a fresh one,
/// or the unsent previous one), then both players move by the queued
/// packets once the state queues are full; player 2's PC takes the
/// difficulty from player 1's packets.
#[test]
fn player_movement_netplay_matches_c() {
    let net = (
        1u32..=2,
        1i32..=3,
        any::<bool>(),
        any::<bool>(),
        prop::collection::vec(any::<u8>(), 18),
        prop::collection::vec(any::<u8>(), 18),
    );
    run_scenes(
        (any_scene(), net),
        500,
        24,
        |rig, (scene, (me, delay, reset, unsent, inbound, outbound))| {
            apply_scene(&mut rig.g, &scene);
            let n = &mut rig.g.network;
            n.is_network_game = true;
            n.this_player_num = me;
            n.network_delay = delay;
            n.last_state_out_sync = if reset { 0 } else { delay as u16 + 3 };
            // Small deltas keep the players on screen.
            let mut inbound = inbound;
            let mut outbound = outbound;
            for d in [&mut inbound, &mut outbound] {
                for at in [4, 6, 8, 10] {
                    let v = (i16::from(d[at + 1] as i8) % 6) as u16;
                    d[at..at + 2].copy_from_slice(&v.to_be_bytes());
                }
            }
            n.packet_state_in[0] = Some(state_packet(&inbound));
            n.packet_state_out[delay as usize] = Some(state_packet(&outbound));
            n.packet_state_out[0] = unsent.then(|| state_packet(&[]));
            rig.store();
            for _ in 0..scene.frames {
                prop_assert_eq!(player_frame_both(rig), Ok(()));
            }
            same_game(rig)
        },
    );
}

/// Recording a demo: the keyboard is the device, and the keys are written
/// to `demorec.1` as they change.
#[test]
fn player_movement_records_demo_matches_c() {
    let mut rig = Rig::from_fixture("ots 1\n", 500);
    rig.g.demo.play_demo = false;
    rig.g.demo.demo_file = Default::default();
    rig.store();
    let out = rig.run(
        // SAFETY: plain C call.
        || unsafe { b::beginRecordDemo() },
        rustyrian_core::demo::begin_record_demo,
    );
    assert_eq!(out, Ok(()));
    // From here on, state changes go to both engines directly: a store
    // would point C's open demo file at Rust's.
    rig.g.demo.record_demo = true;
    // SAFETY: plain writes of C globals on the test thread.
    unsafe { *rustyrian_oracle::mirror::global_mut(&raw mut b::recordDemo) = true };
    rig.clear();
    let fire = rig.g.config.key_settings[4] as usize;
    let left = rig.g.config.key_settings[2] as usize;
    for k in [Some(fire), Some(left), None, Some(fire)] {
        if let Some(k) = k {
            let down = !rig.g.keyboard.keysactive[k];
            rig.g.keyboard.keysactive[k] = down;
            // SAFETY: as above.
            unsafe { (*rustyrian_oracle::mirror::global_mut(&raw mut b::keysactive))[k] = down };
        }
        assert_eq!(player_frame_both(&mut rig), Ok(()));
    }
    let out = rig.run(
        // SAFETY: plain C call.
        || unsafe { b::endRecordDemo() },
        rustyrian_core::demo::end_record_demo,
    );
    assert_eq!(out, Ok(()));
    let (c, r) = (
        std::fs::read(rig.c_dir.join("demorec.1")).expect("C demo"),
        std::fs::read(rig.r_dir.join("demorec.1")).expect("Rust demo"),
    );
    assert_eq!(c, r);
    rig.check();
}

/// The end of the recording ends the level.
#[test]
fn player_movement_demo_end_matches_c() {
    let mut rig = Rig::from_fixture("ots 1\n", 500);
    let path = mainint_support::tmp("demo-end").join("demo.short");
    std::fs::write(&path, [1u8, 0]).expect("short demo");
    let path = std::os::unix::ffi::OsStrExt::as_bytes(path.as_os_str()).to_vec();
    rig.g.demo.demo_file = rustyrian_core::file::file_open(&path, "rb");
    rig.g.demo.demo_keys_wait = 0;
    rig.g.demo.play_demo = true;
    rig.g.varz.end_level = false;
    rig.store();
    rig.clear();
    assert_eq!(player_frame_both(&mut rig), Ok(()));
    assert!(rig.g.varz.end_level);
    for _ in 0..3 {
        assert_eq!(player_frame_both(&mut rig), Ok(()));
    }
    rig.check();
}

/// The linked turret of player 2 pointing up from the top of the screen
/// puts its markers above the screen: C converts the negative float
/// coordinate to `JE_word` (D-UB-MAININT-FLOAT).
#[test]
fn link_gun_above_the_screen_matches_c() {
    for (direc, fire) in [
        (3.1f32, true),
        (3.0, true),
        (0.5, true),
        (6.2, true),
        (1.0, false),
    ] {
        let mut rig = Rig::from_fixture("ots 1\n", 500);
        let g = &mut rig.g;
        g.demo.play_demo = false;
        g.config.two_player_mode = true;
        g.config.two_player_linked = true;
        g.config.galaga_mode = false;
        g.config.link_gun_direc = direc;
        g.config.input_device = [1, 1];
        for p in 0..2 {
            let pl = &mut g.player.player[p];
            pl.is_alive = true;
            (pl.x, pl.y) = (120, 10);
            pl.items.weapon[REAR].id = pl.items.weapon[REAR].id.clamp(1, 38);
        }
        g.config.shot_repeat = [0; 11];
        g.varz.ship_gr2 = 0;
        g.varz.ship_gr2ptr = Some(Sprite2Ref::SpriteSheet9);
        let up = g.config.key_settings[0] as usize;
        let fire_key = g.config.key_settings[4] as usize;
        g.keyboard.keysactive[up] = true;
        g.keyboard.keysactive[fire_key] = fire;
        rig.store();
        rig.clear();
        for _ in 0..4 {
            assert_eq!(player_frame_both(&mut rig), Ok(()));
        }
        rig.check();
    }
}

/// A real loopback network game, frame by frame as `JE_main` runs it:
/// movement (this PC's player writes its state packet), then
/// `network_state_send` and `network_state_update`, with the opponent's
/// state packets arriving over the socket. Once the state queues are full,
/// the remote player moves by the opponent's packets and the local one by
/// its delayed own; on player 2's PC the difficulty comes from player 1.
#[test]
fn player_movement_net_session_matches_c() {
    for me in [1u32, 2] {
        let s = input(&[], &[], &[]);
        let mut rig = net_rig(&s, |g| {
            g.demo.play_demo = false;
            g.config.two_player_mode = true;
            g.config.two_player_linked = false;
            g.config.galaga_mode = false;
            g.config.input_device = [1, 1];
            g.network.this_player_num = me;
            for (p, pl) in g.player.player.iter_mut().enumerate() {
                pl.is_alive = true;
                (pl.x, pl.y) = (100 + 60 * p as i32, 120);
                pl.items.weapon[REAR].id = pl.items.weapon[REAR].id.clamp(1, 38);
            }
            g.varz.ship_gr2 = 0;
            g.varz.ship_gr2ptr = Some(Sprite2Ref::SpriteSheet9);
            let right = g.config.key_settings[3] as usize;
            let fire = g.config.key_settings[4] as usize;
            g.keyboard.keysactive[right] = true;
            g.keyboard.keysactive[fire] = true;
        });
        let out = rig.run(
            // SAFETY: plain C call.
            || unsafe { b::network_state_reset() },
            |g| {
                rustyrian_core::network::network_state_reset(g);
                Ok(())
            },
        );
        assert_eq!(out, Ok(()));
        // The opponent's state for frames 0..6: moving left and up, with
        // the fire and left-sidekick buttons, at difficulty 4.
        for sync in 0..6u16 {
            let mut p = vec![0u8; 28];
            p[..2].copy_from_slice(&rustyrian_core::network::PACKET_STATE.to_be_bytes());
            p[2..4].copy_from_slice(&sync.to_be_bytes());
            p[4..6].copy_from_slice(&(-2i16 as u16).to_be_bytes());
            p[6..8].copy_from_slice(&(-1i16 as u16).to_be_bytes());
            p[8..10].copy_from_slice(&(-1i16 as u16).to_be_bytes());
            p[12..14].copy_from_slice(&0b0011u16.to_be_bytes());
            p[16..18].copy_from_slice(&4u16.to_be_bytes());
            rig.push(&p);
        }
        rig.clear();
        for frame in 0..5 {
            let out = rig.run(
                || {
                    // SAFETY: plain C calls on the test thread.
                    unsafe {
                        b::JE_mainGamePlayerFunctions();
                        b::network_state_send();
                        b::network_state_update()
                    }
                },
                |g| {
                    mi::je_main_game_player_functions(g)?;
                    rustyrian_core::network::network_state_send(g);
                    rustyrian_core::network::network_state_update(g)
                },
            );
            assert_eq!(out, Ok(frame >= 1), "frame {frame}");
        }
        if me == 2 {
            assert_eq!(rig.g.config.difficulty_level, 4);
        }
        rig.check();
    }
}

/// D-UB-MAININT-TABLES, D-UB-MAININT-JOYSTICK, D-UB-MAININT-SHOTAVAIL:
/// item numbers, pick-up values and sidekick frames outside their tables
/// read zeroed entries, a device past the connected joysticks polls
/// nothing, and the asteroid killer from an enemy slot past `shotAvail`
/// writes nothing; no panic.
#[test]
fn movement_and_collision_guards_hold() {
    let script = "ots 1\njoystick \"Pad 0\" 2 6 0\n";
    let mut rig = Rig::from_fixture(script, 500);
    let g = &mut rig.g;
    rustyrian_core::joystick::init_joysticks(g);
    assert_eq!(g.joystick.joysticks, 1);
    g.demo.play_demo = false;
    g.config.two_player_mode = true;
    g.config.two_player_linked = false;
    g.config.galaga_mode = false;
    g.config.input_device = [7, 200];
    let fire = g.config.key_settings[4] as usize;
    g.keyboard.keysactive[fire] = true;
    for p in 0..2 {
        let pl = &mut g.player.player[p];
        pl.is_alive = true;
        pl.items.weapon = [rustyrian_core::player::PlayerItemsWeapon { id: 250, power: 0 }; 2];
        pl.weapon_mode = 0;
        pl.items.sidekick = [250, 250];
        pl.lives = Some(p);
    }
    g.config.shot_repeat = [0; 11];
    mi::je_main_game_player_functions(g).expect("no exit");
    // Weapon 250 has no `weaponPort` entry: its zeroed `opnum` keeps mode 0
    // and its zeroed `op` table fires nothing.
    assert_eq!(g.player.player[1].weapon_mode, 0);
    // The linked turret and charge shot of a player 2 without a rear weapon.
    g.config.two_player_linked = true;
    mi::je_main_game_player_functions(g).expect("no exit");

    let (x, y) = (g.player.player[0].x, g.player.player[0].y);
    let pick_up = |g: &mut rustyrian_core::Game, sam: u8, z: usize, evalue: i16, p: usize| {
        g.config.super_arcade_mode = sam;
        g.varz.enemy_avail = [1; 100];
        g.varz.enemy_avail[z] = 0;
        let e = &mut g.varz.enemy[z];
        (e.ex, e.ey, e.mapoffset) = (x as i16, y as i16, 0);
        e.evalue = evalue;
        e.scoreitem = true;
        e.enemytype = 1;
        let pl = &mut g.player.player[p];
        pl.is_alive = true;
        (pl.x, pl.y) = (x, y);
        mi::je_player_collide(g, p, p as u8 + 1).expect("no exit");
        assert_eq!(g.varz.enemy_avail[z], 1, "picked up {evalue}");
    };

    // A super-arcade pick-up past `SAWeapon`'s row reads weapon 0.
    pick_up(g, 3, 10, 30999, 0);
    assert_eq!(g.varz.temp_w, 0);
    assert_eq!(g.player.player[0].items.weapon[0].id, 0);
    // A special past `special` is stored as C stores it, named "".
    pick_up(g, 0, 11, 32767, 0);
    assert_eq!(g.player.player[0].items.special, (32767 - 32100) as u8);
    // A sidekick series past `optionSelect` gives no sidekicks.
    pick_up(g, 0, 12, 32050, 1);
    assert_eq!(g.player.player[1].items.sidekick_series, 50);
    assert_eq!(g.player.player[1].items.sidekick, [0, 0]);
    // The asteroid killer from slot 99 clears no shot.
    let before = g.shots.shot_avail;
    pick_up(g, 0, 99, -3, 0);
    for (i, (&b, &a)) in before.iter().zip(&g.shots.shot_avail).enumerate() {
        assert!(b == 0 || a == b, "shot {i} changed from {b} to {a}");
    }
}

/// The full key combinations: the Nort ship (local and requested), both
/// cheats, the level skip (local and requested), smoothies, the filter
/// wrap, and an edited ship keeping its lower armor.
#[test]
fn main_keyboard_input_combinations_match_c() {
    const NORT: &[usize] = &[59, 61, 63, 64, 66, 49, 56];
    let cases: [(&[usize], GameEdit); 9] = [
        (NORT, |_| {}),
        (NORT, |g| g.network.is_network_game = true),
        (&[59, 60, 63], |_| {}),
        (&[59, 60, 61], |_| {}),
        (&[59, 63, 64], |_| {}),
        (&[59, 63, 65], |g| g.network.is_network_game = true),
        (&[42, 69, 71, 31, 34, 38, 39], |_| {}),
        (&[42, 45], |g| g.config.level_filter = 15),
        (&[43, 30], |g| {
            g.editship.extra_avail = true;
            for k in 0..10 {
                g.editship.extra_ships[k * 15..k * 15 + 9]
                    .copy_from_slice(&[1, 1, 1, 0, 0, 0, 1, 28, 1]);
            }
            g.varz.edit_ship1 = true;
            g.player.player[0].armor = 3;
        }),
    ];
    let mut rig = Rig::from_fixture("ots 1\n", 500);
    let base = rig.g.snapshot();
    for (keys, edit) in cases {
        rig.g.restore(base.clone());
        rig.g.config.two_player_mode = false;
        rig.g.config.super_tyrian = false;
        rig.g.config.super_arcade_mode = 0;
        for &k in keys {
            rig.g.keyboard.keysactive[k] = true;
        }
        edit(&mut rig.g);
        rig.store();
        rig.clear();
        let out = rig.run(
            // SAFETY: plain C call on the stored game state.
            || unsafe { b::JE_mainKeyboardInput() },
            mi::je_main_keyboard_input,
        );
        assert_eq!(out, Ok(()), "{keys:?}");
        if let Err(e) = same_game(&mut rig) {
            panic!("{keys:?}: {e}");
        }
    }
}

/// One score item on player 1 in a one-player game.
fn quiet_crash() -> Crash {
    Crash {
        flags: [false; 7],
        sam: 0,
        player_num: 1,
        pilot: (150, 100, 0, 5, 10, 1),
        items: [[(1, 1), (1, 1)], [(1, 1), (1, 1)]],
        series: (0, 101),
        damage_rate: 5,
        contacts: vec![Contact {
            slot: 5,
            evalue: 1,
            avail: 0,
            scoreitem: true,
            armorleft: 10,
            linknum: 0,
            push: (0, 0),
            enemytype: 1,
            at: (0, 0, 0),
            ground: (false, 0),
        }],
    }
}

type CrashEdit = fn(&mut Crash);

/// Pick-ups the generated scenes reach rarely: a super-arcade weapon the
/// player already has, specials and weapons in each game mode, a repeated
/// sidekick series, a rear weapon without power, the weapon power-ups in
/// two-player and network games, super bombs and the hot dog.
#[test]
fn player_collide_edge_cases_match_c() {
    // Flags: two players, linked, galaga, one-player arcade, network,
    // bonus level, cheating.
    let cases: [CrashEdit; 12] = [
        |c| {
            c.sam = 1;
            c.contacts[0].evalue = 30001;
            c.items[0][0].0 = rustyrian_core::varz::SA_WEAPON[0][0] as u8;
        },
        |c| c.contacts[0].evalue = 32101,
        |c| {
            c.flags[0] = true;
            c.contacts[0].evalue = 32102;
        },
        |c| {
            c.player_num = 2;
            c.flags[0] = true;
            c.series = (3, 101);
            c.contacts[0].evalue = 32003;
        },
        |c| {
            c.flags[3] = true;
            c.items[0][1].1 = 0;
            c.contacts[0].evalue = 31005;
        },
        |c| {
            c.flags[0] = true;
            c.contacts[0].evalue = 30005;
        },
        |c| {
            c.flags[4] = true;
            c.contacts[0].evalue = -1;
        },
        |c| {
            c.flags[0] = true;
            c.contacts[0].evalue = -1;
        },
        |c| {
            c.flags[0] = true;
            c.contacts[0].evalue = -2;
        },
        |c| {
            c.pilot.5 = 3;
            c.contacts[0].evalue = -4;
        },
        |c| {
            c.pilot.5 = 10;
            c.contacts[0].evalue = -4;
        },
        |c| c.contacts[0].evalue = -5,
    ];
    let mut rig = Rig::from_fixture("ots 1\n", 500);
    let base = rig.g.snapshot();
    for (i, edit) in cases.into_iter().enumerate() {
        rig.g.restore(base.clone());
        let mut crash = quiet_crash();
        edit(&mut crash);
        apply_crash(&mut rig.g, &crash);
        rig.store();
        rig.clear();
        let p = usize::from(crash.player_num) - 1;
        let num = crash.player_num;
        let out = rig.run(
            // SAFETY: plain C call on the stored game state.
            || unsafe { b::JE_playerCollide(c_player(p), num) },
            |g| mi::je_player_collide(g, p, num),
        );
        assert_eq!(out, Ok(()), "case {i}");
        if let Err(e) = same_game(&mut rig) {
            panic!("case {i}: {e}");
        }
    }
}
