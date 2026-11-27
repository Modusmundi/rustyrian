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

/// Runs `test` for each generated case on one fixture rig (each case stores
/// its state into both engines first). Storing and comparing the whole game
/// costs about 0.1 s a case, so these run a quarter of the configured cases
/// (64 by default, 2500 at `PROPTEST_CASES=10000`).
fn run_on_rig<S: Strategy>(
    strategy: S,
    frame: u64,
    test: impl Fn(&mut Rig, S::Value) -> Result<(), TestCaseError>,
) where
    S::Value: std::fmt::Debug,
{
    let rig = RefCell::new(Rig::from_fixture("ots 1\n", frame));
    let base = rig.borrow().g.snapshot();
    let mut config = RunnerConfig::default();
    config.cases = (config.cases / 4).max(16);
    let mut runner = TestRunner::new(config);
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
            rig.check();
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
            rig.check();
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
            rig.check();
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
        rig.check();
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
            rig.check();
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
        rig.check();
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
        rig.check();
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
            rig.check();
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
/// checks calls, state, files and frames.
fn screen_run<T: PartialEq + std::fmt::Debug>(
    script: &str,
    setup: impl FnOnce(&mut rustyrian_core::Game),
    c: impl FnOnce() -> T,
    r: impl FnOnce(&mut rustyrian_core::Game) -> rustyrian_core::game::Flow<T>,
) -> Result<T, i32> {
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
    rig.check_frames();
    out
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
    let out = screen_run(
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
        let out = screen_run(
            &s,
            |g| g.config.super_pause = super_pause,
            // SAFETY: plain C call on the test thread.
            || unsafe { b::JE_pauseGame() },
            mi::je_pause_game,
        );
        assert_eq!(out, Ok(()));
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
    let out = screen_run(
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
        let out = screen_run(
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
        let _ = out;
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
        let out = screen_run(
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
#[ignore = "about 160 s: the whole credits roll on both engines; run with --run-ignored"]
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
    let out = screen_run(
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
    let out = screen_run(
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
