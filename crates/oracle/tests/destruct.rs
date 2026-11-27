//! Differential tests for `destruct.c`.

use rustyrian_core::destruct::{
    DestructExploS, DestructShotS, DestructUnitS, DestructWallS, MODE_NONE, State, UNIT_HELI,
};
use rustyrian_core::video::ScreenId;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

fn sample() -> State {
    let mut s = State::default();
    s.weapon_systems[3][4] = true;
    s.basetypes[9][10] = 3;
    s.default_key_config[1][7][3] = 100;
    s.destruct_temp_screen = Some(ScreenId::GameScreen);
    s.destruct_first_time = true;
    s.config.max_shots = 3;
    s.config.max_explosions = 2;
    s.config.max_walls = 4;
    s.config.max_installations = 2;
    s.config.allow_custom = true;
    s.config.ai = [false, true];
    for (p, pl) in s.destruct_player.iter_mut().enumerate() {
        pl.is_cpu = p == 1;
        pl.ai_memory.c_angle = -3;
        pl.ai_memory.c_no_down = 9;
        pl.unit = Some(
            (0..2)
                .map(|i| DestructUnitS {
                    unit_x: 10 * i + p as u32,
                    unit_y: 1.5,
                    unit_y_mov: -0.25,
                    is_y_in_air: true,
                    unit_type: UNIT_HELI,
                    shot_type: -1,
                    angle: 0.75,
                    power: 3.0,
                    last_move: -1,
                    ani_frame: 2,
                    health: 77,
                })
                .collect(),
        );
        pl.moves.actions[5] = true;
        pl.keys.config[2][1] = 42;
        pl.team = p as u32;
        pl.units_remaining = 4;
        pl.unit_selected = 1;
        pl.shot_delay = 30;
        pl.score = 12;
    }
    s.world.base_map[319] = 150;
    s.world.vga_screen = Some(ScreenId::VgaScreenSeg);
    s.world.map_walls = Some(vec![
        DestructWallS {
            wall_exist: true,
            wall_x: 5,
            wall_y: 6,
        };
        4
    ]);
    s.world.destruct_mode = MODE_NONE;
    s.world.map_flags = 0x1f;
    s.shot_rec = Some(vec![
        DestructShotS {
            is_available: true,
            x: 1.0,
            y: 2.0,
            xmov: 0.5,
            ymov: -0.5,
            gravity: true,
            shottype: 4,
            trailx: [1, 2, 3, 4],
            traily: [5, 6, 7, 8],
            trailc: [9, 10, 11, 12],
        };
        3
    ]);
    s.explo_rec = Some(vec![
        DestructExploS {
            is_available: false,
            x: 1,
            y: 2,
            explowidth: 3,
            explomax: 4,
            explofill: 5,
            explo_type: 2,
        };
        2
    ]);
    s.je_e_sound_explo_sound_channel = 3;
    s.de_run_tick_end_delay = 80;
    s
}

#[test]
fn mirror_round_trip() {
    let d = Diff::new();
    // `destructTempScreen`/`world.VGAScreen` refer to surfaces, which exist
    // after `init_video`.
    let mut video = rustyrian_core::video::State::default();
    for (i, slot) in video.screens.iter_mut().enumerate() {
        let mut surf = rustyrian_core::sdl::Surface::new_8bit(320, 200);
        surf.pixels.fill(i as u8);
        *slot = Some(surf);
    }
    video.vga_screen_seg = Some(ScreenId::VgaScreenSeg);
    video.vga_screen2 = Some(ScreenId::VgaScreen2);
    video.game_screen = Some(ScreenId::GameScreen);
    video.store_to_c();
    drop(d);

    let s = sample();
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
    State::default().store_to_c();
    assert_eq!(diff::load::<State>(), State::default());
}

/// The mirror's hand-written `#[repr(C)]` layouts against C: C writes late
/// fields of every heap and player struct (and reads `world.mapFlags`), and
/// the mirror must load exactly those writes.
#[test]
fn c_writes_land_in_mirrored_fields() {
    use rustyrian_core::destruct::{DEFAULT_CPU_WEAPON_B, UNIT_HELI};
    use rustyrian_oracle::bindings as b;
    let d = Diff::new();
    let mut video = rustyrian_core::video::State::default();
    for slot in video.screens.iter_mut() {
        *slot = Some(rustyrian_core::sdl::Surface::new_8bit(320, 200));
    }
    video.vga_screen_seg = Some(ScreenId::VgaScreenSeg);
    video.vga_screen2 = Some(ScreenId::VgaScreen2);
    video.game_screen = Some(ScreenId::GameScreen);
    video.store_to_c();
    drop(d);

    let mut s = sample();
    s.destruct_player[0].is_cpu = false;
    s.destruct_player[1].is_cpu = true;
    for sh in s.shot_rec.iter_mut().flatten() {
        sh.is_available = false;
    }
    s.store_to_c();
    // SAFETY: the statics these use are allocated and sized above.
    unsafe {
        b::ot_s_destruct_DE_ResetAI();
        b::ot_s_destruct_DE_ResetPlayers();
        b::ot_s_destruct_DE_ResetUnits();
        b::ot_s_destruct_DE_ResetWeapons();
    }

    let mut want = s.clone();
    // DE_ResetAI: player 1 is the CPU; `world.mapFlags` has MAP_WALLS.
    for u in want.destruct_player[1].unit.iter_mut().flatten() {
        assert_eq!(u.unit_type, UNIT_HELI);
        u.angle = std::f64::consts::FRAC_PI_4 as f32;
        u.power = 4.0;
        u.shot_type = DEFAULT_CPU_WEAPON_B[UNIT_HELI as usize];
    }
    for (i, p) in want.destruct_player.iter_mut().enumerate() {
        p.is_cpu = false;
        p.unit_selected = 0;
        p.shot_delay = 0;
        p.score = 0;
        p.ai_memory = Default::default();
        p.keys.config = want.default_key_config[i];
        for u in p.unit.iter_mut().flatten() {
            u.health = 0;
        }
    }
    for sh in want.shot_rec.iter_mut().flatten() {
        sh.is_available = true;
    }
    for e in want.explo_rec.iter_mut().flatten() {
        e.is_available = true;
    }
    assert_eq!(diff::load::<State>(), want);
}

// ---------------------------------------------------------------------------
// Function-level differential tests.
//
// Every case stores one Rust game into C (all mirrors), runs the C function
// (through its `ot_s_destruct_*` thunk) and the Rust port, and compares the
// return values, the recorded platform calls and the whole game state
// (destruct state, surfaces, sound queue, mtrand, ...).
// ---------------------------------------------------------------------------

mod net_support;

use net_support::Rig;
use proptest::prelude::*;
use proptest::strategy::ValueTree;
use proptest::test_runner::{TestCaseError, TestRunner};
use rustyrian_core::Game;
use rustyrian_core::destruct::{
    self as de, DEFAULT_KEY_CONFIG_INIT, DestructAiS, DestructConfigS, DestructKeysS,
    DestructMovesS, DestructPlayerS, DestructWorldS, EXPL_DIRT, EXPL_NORMAL, MAP_FUZZY, MAP_TALL,
    MAP_WALLS, SHOT_INVALID, SHOT_SMALL, UNIT_SATELLITE, UNIT_TANK,
};
use rustyrian_core::varz::Sprite2Ref;
use rustyrian_core::{config_file as cf, helptext, mtrand, nortsong, sprite};
use rustyrian_oracle::bindings as ob;
use rustyrian_oracle::mirror::destruct::{CPlayer, CShot, CWorld};
use rustyrian_oracle::mirror::video::sv;
use std::cell::RefCell;
use std::ffi::{CString, c_char, c_void};

/// Both engines after `init_video`, with the shape tables, help text, the
/// destruct sprite sheet and the sound effects loaded, on input `script`.
fn setup(script: &str) -> Rig {
    let mut rig = Rig::new(script);
    rig.video();
    // SAFETY: plain C calls inside the guard.
    let r = rig.run(
        || unsafe { ob::JE_loadHelpText() },
        helptext::je_load_help_text,
    );
    assert_eq!(r, Ok(()));
    let r = rig.run(
        // SAFETY: a static sprite array of the oracle.
        || unsafe { ob::JE_loadCompShapes(&raw mut ob::destructSpriteSheet, b'~' as c_char) },
        |g| sprite::je_load_comp_shapes(g, Sprite2Ref::DestructSpriteSheet, b'~'),
    );
    assert_eq!(r, Ok(()));
    // SAFETY: plain C call.
    let r = rig.run(
        || unsafe { ob::loadSndFile(false) },
        |g| nortsong::load_snd_file(g, false),
    );
    assert_eq!(r, Ok(()));
    rig.check();
    rig
}

/// Asserts that the C state equals the Rust state: destruct state and
/// surfaces first (readable diffs), then calls and the whole game.
fn compare(rig: &mut Rig) -> Result<(), TestCaseError> {
    prop_assert_eq!(&diff::load::<State>(), &rig.g.destruct);
    let cv: rustyrian_core::video::State = diff::load();
    for id in ScreenId::ALL {
        let (c, r) = (&cv.screens[id as usize], &rig.g.video.screens[id as usize]);
        if c != r {
            let (c, r) = (c.as_ref().expect("C surface"), r.as_ref().expect("surface"));
            let i = c.pixels.iter().zip(&r.pixels).position(|(a, b)| a != b);
            return Err(TestCaseError::fail(format!(
                "{id:?} differs first at {i:?}: C {:?} Rust {:?}",
                i.map(|i| c.pixels[i]),
                i.map(|i| r.pixels[i]),
            )));
        }
    }
    rig.check();
    Ok(())
}

/// Stores the Rust game into C, then runs `c` and `r` and compares them.
fn same<T: PartialEq + std::fmt::Debug>(
    rig: &mut Rig,
    c: impl FnOnce() -> T,
    r: impl FnOnce(&mut Game) -> T,
) -> Result<(), TestCaseError> {
    rig.store();
    rig.clear();
    rig.run(c, |g| Ok(r(g))).expect("no exit");
    compare(rig)
}

/// [`same`] for functions that can exit.
fn same_flow<T: PartialEq + std::fmt::Debug>(
    rig: &mut Rig,
    c: impl FnOnce() -> T,
    r: impl FnOnce(&mut Game) -> rustyrian_core::game::Flow<T>,
) -> Result<(), TestCaseError> {
    rig.store();
    rig.clear();
    rig.run(c, r).expect("no exit");
    compare(rig)
}

/// Runs `f` for `cases / div` generated values on one pair of engines.
fn run_cases<S: Strategy>(
    div: u32,
    strategy: S,
    f: impl Fn(&mut Rig, S::Value) -> Result<(), TestCaseError>,
) {
    let rig = RefCell::new(setup("ots 1\n"));
    let config = ProptestConfig::default();
    TestRunner::new(ProptestConfig {
        cases: (config.cases / div).max(1),
        max_shrink_iters: 32,
        ..config
    })
    .run(&strategy, |v| f(&mut rig.borrow_mut(), v))
    .unwrap();
}

// --- C pointers into the stored state (valid until the next store) ---

fn c_players() -> &'static mut [CPlayer; 2] {
    // SAFETY: the address of `destruct_player`, used on the test thread.
    unsafe { sv(ob::ot_sv_destruct_destruct_player) }
}

fn c_unit(p: usize, u: usize) -> *mut c_void {
    // SAFETY: `u` is below the stored array's length.
    unsafe { c_players()[p].unit.add(u).cast() }
}

fn c_shot(i: usize) -> *mut c_void {
    // SAFETY: `i` is below the stored `shotRec` length.
    unsafe {
        (*sv::<*mut CShot>(ob::ot_sv_destruct_shotRec))
            .add(i)
            .cast()
    }
}

fn c_world() -> *mut CWorld {
    // SAFETY: the address of `world`.
    unsafe { sv::<CWorld>(ob::ot_sv_destruct_world) }
}

fn c_screen(id: ScreenId) -> *mut ob::SDL_Surface {
    // SAFETY: plain reads of the stored surface pointers.
    unsafe {
        match id {
            ScreenId::VgaScreenSeg => ob::VGAScreenSeg,
            ScreenId::VgaScreen2 => ob::VGAScreen2,
            ScreenId::GameScreen => ob::game_screen,
        }
    }
}

// --- generators ---

/// A destruct world in a state the game can reach: unit, shot, explosion and
/// wall positions inside the ranges the game keeps them in, satellites
/// without a weapon, the selected unit an armed non-satellite, and
/// `unitsRemaining` zero exactly when no armed unit is alive.
#[derive(Clone, Debug)]
struct World {
    s: State,
    seed: u64,
    keys: Vec<usize>,
    queue: [u8; 8],
}

fn arb_unit() -> impl Strategy<Value = DestructUnitS> {
    (
        (
            prop_oneof![3 => 1u32..=308, 1 => 290u32..=308],
            prop_oneof![2 => 24.0f32..199.0, 1 => 24.0f32..30.0, 1 => 196.0f32..199.0],
            prop_oneof![Just(0.0f32), -3.0f32..3.0],
            any::<bool>(),
            0i32..8,
            0i32..17,
        ),
        (
            prop_oneof![
                Just(0.0f32),
                0.0f32..1.56,
                1.55f32..std::f32::consts::FRAC_PI_2
            ],
            1.0f32..6.0,
            -5i32..=5,
            0u32..4,
            prop_oneof![1 => -3i32..=0, 4 => 1i32..300],
        ),
    )
        .prop_map(
            |(
                (unit_x, unit_y, unit_y_mov, is_y_in_air, unit_type, shot_type),
                (angle, power, last_move, ani_frame, health),
            )| DestructUnitS {
                unit_x,
                unit_y,
                unit_y_mov,
                is_y_in_air,
                unit_type,
                shot_type: if unit_type == UNIT_SATELLITE {
                    SHOT_INVALID
                } else {
                    shot_type
                },
                angle,
                power,
                last_move,
                ani_frame,
                health,
            },
        )
}

fn arb_player(inst: u32) -> impl Strategy<Value = DestructPlayerS> {
    (
        prop::collection::vec(arb_unit(), inst as usize),
        any::<bool>(),
        (-2i32..=2, -2i32..=2, -1i32..=1, 0u32..6),
        any::<[bool; 8]>(),
        0..inst,
        prop_oneof![Just(0u32), 0u32..90],
        0u32..500,
        1u32..6,
    )
        .prop_map(
            |(mut units, is_cpu, (a, p, f, nd), actions, sel, shot_delay, score, remaining)| {
                let s = &mut units[sel as usize];
                if s.unit_type == UNIT_SATELLITE {
                    s.unit_type = UNIT_TANK;
                    s.shot_type = SHOT_SMALL;
                }
                let armed = units
                    .iter()
                    .any(|u| u.health > 0 && u.shot_type != SHOT_INVALID);
                DestructPlayerS {
                    is_cpu,
                    ai_memory: DestructAiS {
                        c_angle: a,
                        c_power: p,
                        c_fire: f,
                        c_no_down: nd,
                    },
                    unit: Some(units),
                    moves: DestructMovesS { actions },
                    keys: DestructKeysS::default(),
                    team: 0,
                    units_remaining: if armed { remaining } else { 0 },
                    unit_selected: sel,
                    shot_delay,
                    score,
                }
            },
        )
}

fn arb_shot() -> impl Strategy<Value = DestructShotS> {
    (
        (
            any::<bool>(),
            1.0f32..318.0,
            // Rows past 198 can reach row 200 (D-UB-DESTRUCT-3).
            prop_oneof![2 => 0.0f32..198.0, 1 => 190.0f32..198.0],
            prop_oneof![Just(0.0f32), -4.0f32..4.0],
            -4.0f32..4.0,
            any::<bool>(),
            0u32..17,
        ),
        (
            any::<[u16; 4]>().prop_map(|a| a.map(|v| u32::from(v) % 321)),
            any::<[u16; 4]>().prop_map(|a| a.map(|v| u32::from(v) % 200)),
            any::<[u16; 4]>().prop_map(|a| a.map(|v| u32::from(v) % 260)),
        ),
    )
        .prop_map(
            |((is_available, x, y, xmov, ymov, gravity, shottype), (trailx, traily, trailc))| {
                DestructShotS {
                    is_available,
                    x,
                    y,
                    xmov,
                    ymov,
                    gravity,
                    shottype,
                    trailx,
                    traily,
                    trailc,
                }
            },
        )
}

fn arb_explo() -> impl Strategy<Value = DestructExploS> {
    (
        any::<bool>(),
        0u32..=330,
        0u32..200,
        0u32..40,
        0u32..60,
        0u32..30,
        any::<bool>(),
    )
        .prop_map(
            |(is_available, x, y, explowidth, explomax, explofill, dirt)| DestructExploS {
                is_available,
                x,
                // A dirt flare at (320, 199) is past the surface (D-UB-DESTRUCT-3).
                y: if dirt { y.min(150) } else { y },
                explowidth,
                explomax,
                explofill,
                explo_type: if dirt { EXPL_DIRT } else { EXPL_NORMAL },
            },
        )
}

fn arb_wall() -> impl Strategy<Value = DestructWallS> {
    (any::<bool>(), 10u32..=309, -40i32..200).prop_map(|(wall_exist, wall_x, y)| DestructWallS {
        wall_exist,
        wall_x,
        wall_y: y as u32,
    })
}

/// Scancodes the tests press: every default destruct key plus the tick's
/// control keys (F10, F11, ESCAPE, BACKSPACE; P and F1 wait for input).
fn keys() -> Vec<usize> {
    let mut k: Vec<usize> = DEFAULT_KEY_CONFIG_INIT
        .iter()
        .flatten()
        .flatten()
        .filter(|&&s| s != 0)
        .map(|&s| s as usize)
        .collect();
    k.extend([67, 68, 41, 42]);
    k
}

fn arb_world() -> impl Strategy<Value = World> {
    (1u32..=10, 0u32..=10, 0u32..=10, 0u32..=10)
        .prop_flat_map(|(inst, shots, explos, walls)| {
            (
                (
                    Just((inst, shots, explos, walls)),
                    arb_player(inst),
                    arb_player(inst),
                ),
                (
                    prop::collection::vec(arb_shot(), shots as usize),
                    prop::collection::vec(arb_explo(), explos as usize),
                    prop::collection::vec(arb_wall(), walls as usize),
                ),
                (
                    any::<[bool; 7]>(),
                    0u32..=walls,
                    0i32..6,
                    0u32..32,
                    0i32..=5,
                    0u32..3,
                ),
                (
                    prop::collection::vec(0u32..200, 320),
                    any::<u64>(),
                    prop::collection::vec(prop::sample::select(keys()), 0..6),
                    prop::array::uniform8(prop_oneof![2 => Just(0u8), 1 => 1u8..=30]),
                ),
            )
        })
        .prop_map(
            |(
                ((inst, shots, explos, walls), p0, p1),
                (shot_rec, explo_rec, map_walls),
                (f, min_walls, mode, flags, channel, end_delay),
                (base_map, seed, keys, queue),
            )| {
                let mut players = [p0, p1];
                for (i, p) in players.iter_mut().enumerate() {
                    p.keys.config = DEFAULT_KEY_CONFIG_INIT[i];
                    p.team = i as u32;
                }
                let s = State {
                    config: DestructConfigS {
                        max_shots: shots,
                        min_walls,
                        max_walls: walls,
                        max_explosions: explos,
                        max_installations: inst,
                        allow_custom: f[5],
                        alwaysalias: f[0],
                        jumper_straight: [f[3], f[4]],
                        ai: [f[1], f[2]],
                    },
                    destruct_player: players,
                    world: DestructWorldS {
                        base_map: base_map.try_into().expect("320 columns"),
                        vga_screen: Some(ScreenId::VgaScreenSeg),
                        map_walls: Some(map_walls),
                        destruct_mode: mode,
                        map_flags: flags,
                    },
                    shot_rec: Some(shot_rec),
                    explo_rec: Some(explo_rec),
                    destruct_temp_screen: Some(ScreenId::GameScreen),
                    destruct_first_time: f[6],
                    je_e_sound_explo_sound_channel: channel,
                    de_run_tick_end_delay: end_delay,
                    ..State::default()
                };
                World {
                    s,
                    seed,
                    keys,
                    queue,
                }
            },
        )
}

/// Terrain-like contents for `VGAScreen` and `destructTempScreen`: dirt
/// below a random-walk horizon, explosion and alias colours above it.
fn fill_screens(g: &mut Game, seed: u64) {
    let mut x = seed | 1;
    let mut next = move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    for id in [ScreenId::VgaScreenSeg, ScreenId::GameScreen] {
        let mut h = 40 + (next() % 150) as i64;
        let heights: Vec<i64> = (0..320)
            .map(|_| {
                h = (h + (next() % 7) as i64 - 3).clamp(12, 199);
                h
            })
            .collect();
        let s = g.video.screen_mut(id);
        for y in 0..200usize {
            for (col, &top) in heights.iter().enumerate() {
                let r = next() % 100;
                s.pixels[y * 320 + col] = if y as i64 >= top {
                    if r < 92 { 25 } else { 0 }
                } else if r < 3 {
                    241 + (next() % 15) as u8
                } else if r < 5 {
                    16 + (next() % 9) as u8
                } else {
                    0
                };
            }
        }
    }
}

/// Puts `w` into the Rust game (stored into C by [`same`]).
fn apply(g: &mut Game, w: &World) {
    g.destruct = w.s.clone();
    fill_screens(g, w.seed);
    g.keyboard.keysactive.fill(false);
    for &k in &w.keys {
        g.keyboard.keysactive[k] = true;
    }
    g.varz.sound_queue = w.queue;
    mtrand::mt_srand(g, w.seed);
}

fn unit_count(w: &World) -> usize {
    w.s.config.max_installations as usize
}

// --- tests ---

#[test]
fn get_unit_by_name_matches_c() {
    let _d = Diff::new();
    let mut names: Vec<&str> = de::UNIT_NAMES.to_vec();
    names.extend(["", "Tank", "tanks", "hel", "heli ", "satellite\u{1}"]);
    for name in names {
        let c = CString::new(name).expect("no NUL");
        // SAFETY: a valid C string.
        let cr = unsafe { ob::ot_s_destruct_get_unit_by_name(c.as_ptr()) };
        assert_eq!(cr, de::get_unit_by_name(name.as_bytes()), "{name:?}");
    }
}

/// One option of a generated configuration: absent, a single item, or a
/// list.
#[derive(Clone, Debug)]
enum Opt {
    Absent,
    Item(String),
    List(Vec<String>),
}

fn arb_opt(values: &'static [&'static str], max: usize) -> impl Strategy<Value = Opt> {
    let v = prop::sample::select(values).prop_map(String::from);
    prop_oneof![
        Just(Opt::Absent),
        v.clone().prop_map(Opt::Item),
        prop::collection::vec(v, 0..=max).prop_map(Opt::List),
    ]
}

fn put_opt(section: &mut cf::ConfigSection, key: &str, o: &Opt) {
    match o {
        Opt::Absent => {}
        Opt::Item(v) => cf::config_set_string_option(section, key.as_bytes(), Some(v.as_bytes())),
        Opt::List(vs) => {
            let option = cf::config_get_or_set_option(section, key.as_bytes(), None);
            cf::config_set_value(option, None);
            for v in vs {
                cf::config_add_value(option, v.as_bytes());
            }
        }
    }
}

const BOOLS: &[&str] = &["yes", "no", "on", "off", "true", "false", "1", "maybe"];
const INTS: &[&str] = &["40", "0", "20", "7", "-3", "255", "300", "x", ""];
const KEY_NAMES: &[&str] = &[
    "C",
    "V",
    "A",
    "Z",
    "Left Alt",
    "X",
    "Left Shift",
    "Space",
    "Left",
    "Keypad 4",
    "Return",
    "Keypad Enter",
    "Bogus",
    "",
];
const UNIT_VALUES: &[&str] = &[
    "tank",
    "nuke",
    "dirt",
    "satellite",
    "magnet",
    "laser",
    "jumper",
    "heli",
    "bogus",
];

#[test]
fn load_destruct_config_matches_c() {
    let sections = (
        any::<[bool; 4]>(),
        prop::collection::vec(arb_opt(BOOLS, 2), 6),
        prop::collection::vec(arb_opt(INTS, 2), 4),
        prop::collection::vec(arb_opt(KEY_NAMES, 6), 16),
        (
            arb_opt(BOOLS, 1),
            prop::collection::vec(arb_opt(INTS, 1), 2),
        ),
        prop::collection::vec(arb_opt(UNIT_VALUES, 13), 2),
    );
    run_cases(
        2,
        sections,
        |rig, (present, bools, ints, keys, (enable, nums), units)| {
            let cfg = &mut rig.g.config.opentyrian_config;
            *cfg = cf::Config::default();
            if present[0] {
                let s = cf::config_add_section(cfg, b"destruct", None);
                let names = [
                    "antialias craters",
                    "tracer laser",
                    "left ai",
                    "left jumper fires straight",
                    "right ai",
                    "right jumper fires straight",
                ];
                for (k, o) in names.iter().zip(&bools) {
                    put_opt(s, k, o);
                }
                for (k, o) in ["max shots", "max explosions", "min walls", "max walls"]
                    .iter()
                    .zip(&ints)
                {
                    put_opt(s, k, o);
                }
            }
            for (p, name) in ["left", "right"].iter().enumerate() {
                if present[1 + p] {
                    let s =
                        cf::config_add_section(cfg, b"destruct keyboard", Some(name.as_bytes()));
                    for (k, key) in de::KEY_NAMES.iter().enumerate() {
                        put_opt(s, key, &keys[p * 8 + k]);
                    }
                }
            }
            if present[3] {
                let s = cf::config_add_section(cfg, b"destruct custom", None);
                put_opt(s, "enable", &enable);
                put_opt(s, "left num units", &nums[0]);
                put_opt(s, "right num units", &nums[1]);
                put_opt(s, "left unit", &units[0]);
                put_opt(s, "right unit", &units[1]);
            }
            rig.g.destruct = State::default();
            same(
                rig,
                // SAFETY: the oracle's config object.
                || unsafe {
                    ob::ot_s_destruct_load_destruct_config(&raw mut ob::opentyrian_config)
                },
                de::load_destruct_config,
            )
        },
    );
}

/// `DE_RaiseAngle`, `DE_LowerAngle`, `DE_RaisePower`, `DE_LowerPower`,
/// `DE_isValidUnit`, `DE_CycleWeaponUp`, `DE_CycleWeaponDown`.
#[test]
fn unit_functions_match_c() {
    run_cases(
        1,
        (arb_world(), 0usize..2, 0usize..10, 0u8..7),
        |rig, (mut w, p, u, which)| {
            let u = u % unit_count(&w);
            let unit = &mut w.s.destruct_player[p].unit.as_mut().expect("units")[u];
            if which >= 5 && unit.unit_type == UNIT_SATELLITE {
                // A satellite has no weapon to cycle to (the game never asks).
                unit.unit_type = UNIT_TANK;
            }
            apply(&mut rig.g, &w);
            same(
                rig,
                // SAFETY: a stored unit.
                move || unsafe {
                    let cu = c_unit(p, u);
                    match which {
                        0 => ob::ot_s_destruct_DE_RaiseAngle(cu),
                        1 => ob::ot_s_destruct_DE_LowerAngle(cu),
                        2 => ob::ot_s_destruct_DE_RaisePower(cu),
                        3 => ob::ot_s_destruct_DE_LowerPower(cu),
                        4 => return ob::ot_s_destruct_DE_isValidUnit(cu),
                        5 => ob::ot_s_destruct_DE_CycleWeaponUp(cu),
                        _ => ob::ot_s_destruct_DE_CycleWeaponDown(cu),
                    }
                    false
                },
                move |g| {
                    let unit = &mut g.destruct.destruct_player[p].unit.as_mut().expect("units")[u];
                    match which {
                        0 => de::de_raise_angle(unit),
                        1 => de::de_lower_angle(unit),
                        2 => de::de_raise_power(unit),
                        3 => de::de_lower_power(unit),
                        4 => return de::de_is_valid_unit(unit),
                        5 => de::de_cycle_weapon_up(g, p, u),
                        _ => de::de_cycle_weapon_down(g, p, u),
                    }
                    false
                },
            )
        },
    );
}

/// `DE_RunTickGravity`, `DE_GravityLowerUnit`, `DE_GravityFlyUnit`,
/// `DE_GravityDrawUnit`, `JE_stabilityCheck`.
#[test]
fn gravity_matches_c() {
    run_cases(
        1,
        (arb_world(), 0usize..2, 0usize..10, 0u8..5),
        |rig, (w, p, u, which)| {
            let u = u % unit_count(&w);
            apply(&mut rig.g, &w);
            let unit = w.s.destruct_player[p].unit.as_ref().expect("units")[u];
            let (x, y) = (unit.unit_x, unit.unit_y.round() as u32);
            same(
                rig,
                // SAFETY: stored units; the checked rows are inside the surface.
                move || unsafe {
                    match which {
                        0 => ob::ot_s_destruct_DE_RunTickGravity(),
                        1 => ob::ot_s_destruct_DE_GravityLowerUnit(c_unit(p, u)),
                        2 => ob::ot_s_destruct_DE_GravityFlyUnit(c_unit(p, u)),
                        3 => ob::ot_s_destruct_DE_GravityDrawUnit(p as i32, c_unit(p, u)),
                        _ => return ob::ot_s_destruct_JE_stabilityCheck(x, y),
                    }
                    false
                },
                move |g| {
                    match which {
                        0 => de::de_run_tick_gravity(g),
                        1 => de::de_gravity_lower_unit(g, p, u),
                        2 => de::de_gravity_fly_unit(g, p, u),
                        3 => de::de_gravity_draw_unit(g, p as u32, u),
                        _ => return de::je_stability_check(g, x, y),
                    }
                    false
                },
            )
        },
    );
}

/// `DE_RunTickShots`, `DE_DrawTrails`, `JE_pixCool`, `DE_MakeShot`,
/// `DE_RunMagnet`.
#[test]
fn shots_match_c() {
    let args = (
        arb_world(),
        0usize..2,
        0usize..10,
        0u8..5,
        (0u32..=330, 1u32..=210, any::<u8>(), any::<bool>()),
    );
    run_cases(1, args, |rig, (mut w, p, i, which, (x, y, c, heli))| {
        let sel = w.s.destruct_player[p].unit_selected as usize;
        if which >= 3 && heli {
            w.s.destruct_player[p].unit.as_mut().expect("units")[sel].unit_type = de::UNIT_HELI;
        }
        apply(&mut rig.g, &w);
        let shots = w.s.config.max_shots as usize;
        // `DE_DrawTrails` needs a shot slot; without one, exercise the others.
        let which = if which == 1 && shots == 0 { 0 } else { which };
        let i = if shots == 0 { 0 } else { i % shots };
        let (count, decay, start) = if p == 0 { (2, 4, 250) } else { (4, 3, 252) };
        let sel = w.s.destruct_player[p].unit_selected as usize;
        let dir = if p == 0 { -1 } else { 1 };
        same(
            rig,
            // SAFETY: stored shots and units.
            move || unsafe {
                match which {
                    0 => ob::ot_s_destruct_DE_RunTickShots(),
                    1 => ob::ot_s_destruct_DE_DrawTrails(c_shot(i), count, decay, start),
                    2 => ob::ot_s_destruct_JE_pixCool(x, y, c),
                    3 => ob::ot_s_destruct_DE_MakeShot(p as i32, c_unit(p, sel), dir),
                    _ => ob::ot_s_destruct_DE_RunMagnet(p as i32, c_unit(p, sel)),
                }
            },
            move |g| match which {
                0 => de::de_run_tick_shots(g),
                1 => de::de_draw_trails(g, i, count, decay, start),
                2 => de::je_pix_cool(g, x, y, c),
                3 => de::de_make_shot(g, p as u32, sel, dir),
                _ => de::de_run_magnet(g, p as u32, sel),
            },
        )
    });
}

/// `DE_RunTickExplosions`, `JE_superPixel`, `DE_TestExplosionCollision`,
/// `DE_DestroyUnit`, `JE_makeExplosion`, `JE_eSound`.
#[test]
fn explosions_match_c() {
    let args = (
        arb_world(),
        0usize..2,
        0usize..10,
        0u8..6,
        (0u32..=320, 16u32..200, -1i32..17, 0u32..40),
    );
    run_cases(1, args, |rig, (w, p, u, which, (x, y, shot, sound))| {
        let u = u % unit_count(&w);
        apply(&mut rig.g, &w);
        same(
            rig,
            // SAFETY: stored units; positions inside the playfield.
            move || unsafe {
                match which {
                    0 => ob::ot_s_destruct_DE_RunTickExplosions(),
                    1 => ob::ot_s_destruct_JE_superPixel(x, y),
                    2 => ob::ot_s_destruct_DE_TestExplosionCollision(x, y),
                    3 => ob::ot_s_destruct_DE_DestroyUnit(p as i32, c_unit(p, u)),
                    4 => ob::ot_s_destruct_JE_makeExplosion(x, y, shot),
                    _ => ob::ot_s_destruct_JE_eSound(sound),
                }
            },
            move |g| match which {
                0 => de::de_run_tick_explosions(g),
                1 => de::je_super_pixel(g, x, y),
                2 => de::de_test_explosion_collision(g, x, y),
                3 => de::de_destroy_unit(g, p as u32, u),
                4 => de::je_make_explosion(g, x, y, shot),
                _ => de::je_e_sound(g, sound),
            },
        )
    });
}

/// The once-per-tick helpers and resets.
#[test]
fn tick_parts_match_c() {
    run_cases(1, (arb_world(), 0u8..16), |rig, (w, which)| {
        apply(&mut rig.g, &w);
        same(
            rig,
            // SAFETY: the stored world.
            move || unsafe {
                match which {
                    0 => ob::ot_s_destruct_DE_ResetUnits(),
                    1 => ob::ot_s_destruct_DE_ResetPlayers(),
                    2 => ob::ot_s_destruct_DE_ResetWeapons(),
                    3 => ob::ot_s_destruct_DE_ResetAI(),
                    4 => ob::ot_s_destruct_DE_ResetActions(),
                    5 => ob::ot_s_destruct_DE_RunTickCycleDeadUnits(),
                    6 => ob::ot_s_destruct_DE_RunTickAnimate(),
                    7 => ob::ot_s_destruct_DE_RunTickDrawWalls(),
                    8 => ob::ot_s_destruct_DE_RunTickAI(),
                    9 => ob::ot_s_destruct_DE_RunTickDrawCrosshairs(),
                    10 => ob::ot_s_destruct_DE_RunTickDrawHUD(),
                    11 => ob::ot_s_destruct_DE_RunTickGetInput(),
                    12 => ob::ot_s_destruct_DE_ProcessInput(),
                    13 => return ob::ot_s_destruct_DE_RunTickCheckEndgame(),
                    14 => ob::ot_s_destruct_DE_RunTickPlaySounds(),
                    _ => ob::ot_s_destruct_JE_tempScreenChecking(),
                }
                false
            },
            move |g| {
                match which {
                    0 => de::de_reset_units(g),
                    1 => de::de_reset_players(g),
                    2 => de::de_reset_weapons(g),
                    3 => de::de_reset_ai(g),
                    4 => de::de_reset_actions(g),
                    5 => de::de_run_tick_cycle_dead_units(g),
                    6 => de::de_run_tick_animate(g),
                    7 => de::de_run_tick_draw_walls(g),
                    8 => de::de_run_tick_ai(g),
                    9 => de::de_run_tick_draw_crosshairs(g),
                    10 => de::de_run_tick_draw_hud(g),
                    11 => de::de_run_tick_get_input(g).expect("no exit"),
                    12 => de::de_process_input(g),
                    13 => return de::de_run_tick_check_endgame(g),
                    14 => de::de_run_tick_play_sounds(g),
                    _ => de::je_temp_screen_checking(g),
                }
                false
            },
        )
    });
}

/// One whole `DE_RunTick` (ESCAPE, BACKSPACE, F10 and F11 pressed at random).
#[test]
fn run_tick_matches_c() {
    run_cases(2, arb_world(), |rig, w| {
        apply(&mut rig.g, &w);
        same_flow(
            rig,
            // SAFETY: the stored world.
            || unsafe { ob::ot_s_destruct_DE_RunTick() as u32 },
            de::de_run_tick,
        )
    });
}

/// Whether `DE_generateBaseTerrain` would compute a negative column height
/// from `g`'s random stream (D-UB-DESTRUCT-2, mirrored per architecture).
/// `flags` `None` first draws them as `JE_generateTerrain` does.
fn negative_terrain(g: &Game, flags: Option<u32>) -> bool {
    use std::f64::consts::PI;
    let mut t = Game::new(Box::new(rustyrian_harness::test_session().0));
    t.mtrand = g.mtrand.clone();
    let flags = flags.unwrap_or_else(|| {
        let mut f = 0;
        if mtrand::mt_rand(&mut t).is_multiple_of(2) {
            f |= MAP_WALLS;
        }
        if mtrand::mt_rand(&mut t).is_multiple_of(4) {
            f |= de::MAP_HOLES;
        }
        match mtrand::mt_rand(&mut t) % 4 {
            0 => f |= MAP_FUZZY,
            1 => f |= MAP_TALL,
            2 => f |= de::MAP_RINGS,
            _ => {}
        }
        mtrand::mt_rand(&mut t);
        f
    });
    let wave =
        |t: &mut Game| (f64::from(mtrand::mt_rand_lt1(t)) * PI / 50.0 + f64::from(0.01f32)) as f32;
    let (mut s1, mut s2) = (wave(&mut t), wave(&mut t));
    let (c1, c2) = (wave(&mut t), wave(&mut t));
    if flags & MAP_FUZZY != 0 {
        s1 = (PI - f64::from(mtrand::mt_rand_lt1(&mut t) * 0.3f32)) as f32;
        s2 = (PI - f64::from(mtrand::mt_rand_lt1(&mut t) * 0.3f32)) as f32;
    }
    let hm = if flags & MAP_TALL != 0 { 100.0 } else { 20.0 };
    (1..=318).any(|i| {
        let f = i as f32;
        (libm::sinf(s1 * f) * hm
            + libm::sinf(s2 * f) * 15.0
            + libm::cosf(c1 * f) * 10.0
            + libm::sinf(c2 * f) * 15.0)
            .round()
            + 130.0
            < 0.0
    })
}

/// Runs `r` on the Rust game and restores it; true if that placed a wall at
/// x 309 (D-UB-DESTRUCT-1: C then reads past `baseMap`).
fn places_wall_at_309(rig: &mut Rig, r: impl FnOnce(&mut Game)) -> bool {
    let snap = rig.g.snapshot();
    r(&mut rig.g);
    let bad = rig
        .g
        .destruct
        .world
        .map_walls
        .iter()
        .flatten()
        .any(|w| w.wall_exist && w.wall_x == 309);
    rig.g.restore(snap);
    bad
}

/// Terrain generation and its helpers.
#[test]
fn terrain_matches_c() {
    let args = (
        arb_world(),
        0u8..10,
        (0u32..32, any::<bool>(), 0u32..320, 1u32..=320, 0u32..200),
    );
    run_cases(
        1,
        args,
        |rig, (mut w, which, (flags, flag, x, width, y))| {
            // Enough units for every mode's fleet, as `JE_destructGame` sizes them.
            w.s.config.max_installations = 10;
            for p in &mut w.s.destruct_player {
                p.unit
                    .as_mut()
                    .expect("units")
                    .resize(10, DestructUnitS::default());
            }
            for wall in w.s.world.map_walls.iter_mut().flatten() {
                wall.wall_x = wall.wall_x.min(308);
            }
            apply(&mut rig.g, &w);
            let screen = if flag {
                ScreenId::VgaScreenSeg
            } else {
                ScreenId::GameScreen
            };
            let pixel = if flag { 25 } else { 0 };
            let width = 1 + (width - 1) % (320 - x);
            match which {
                0 => {}
                3 => prop_assume!(!places_wall_at_309(rig, de::de_generate_walls)),
                8 | 9 => {
                    let bad = places_wall_at_309(rig, |g| {
                        de::de_reset_level(g).expect("no exit");
                    });
                    prop_assume!(!bad);
                }
                _ => {}
            }
            same_flow(
                rig,
                // SAFETY: the stored world and surfaces; indices inside them.
                move || unsafe {
                    let base = (*c_world()).base_map.as_mut_ptr();
                    match which {
                        0 => ob::ot_s_destruct_DE_generateBaseTerrain(flags, base),
                        1 => ob::ot_s_destruct_DE_drawBaseTerrain(base),
                        2 => ob::ot_s_destruct_DE_generateUnits(base),
                        3 => ob::ot_s_destruct_DE_generateWalls(c_world().cast()),
                        4 => ob::ot_s_destruct_DE_generateRings(
                            c_screen(ScreenId::VgaScreenSeg),
                            pixel,
                        ),
                        5 => ob::ot_s_destruct_JE_aliasDirt(c_screen(screen)),
                        6 => {
                            let s = c_screen(ScreenId::VgaScreenSeg);
                            let p = (*s).pixels.cast::<u8>().add((x + y * 320) as usize);
                            return ob::ot_s_destruct_aliasDirtPixel(s, x, y, p);
                        }
                        7 => return ob::ot_s_destruct_JE_placementPosition(x, width, base),
                        8 => ob::ot_s_destruct_JE_generateTerrain(),
                        _ => ob::ot_s_destruct_DE_ResetLevel(),
                    }
                    0
                },
                move |g| {
                    match which {
                        0 => de::de_generate_base_terrain(g, flags),
                        1 => de::de_draw_base_terrain(g),
                        2 => de::de_generate_units(g),
                        3 => de::de_generate_walls(g),
                        4 => de::de_generate_rings(g, ScreenId::VgaScreenSeg, pixel),
                        5 => de::je_alias_dirt(g, screen),
                        6 => {
                            let s = g.video.screen(ScreenId::VgaScreenSeg);
                            return Ok(de::alias_dirt_pixel(
                                200,
                                320,
                                x,
                                y,
                                &s.pixels,
                                (x + y * 320) as usize,
                            ));
                        }
                        7 => return Ok(de::je_placement_position(g, x, width)),
                        8 => de::je_generate_terrain(g)?,
                        _ => de::de_reset_level(g)?,
                    }
                    Ok(0)
                },
            )
        },
    );
}

/// An input script tapping `keys` 200 ms apart from 500 ms on.
fn key_script(keys: &[&str]) -> String {
    let mut s = String::from("ots 1\n");
    for (i, k) in keys.iter().enumerate() {
        let t = 500 + 200 * i;
        s.push_str(&format!("@{t} key-down {k}\n@{} key-up {k}\n", t + 45));
    }
    s
}

/// `JE_modeSelect` walking past both ends of the menu, with and without the
/// custom mode: UP from the first mode, DOWN onto and past the last ones.
#[test]
fn mode_select_wraps_match_c() {
    let walks: [&[&str]; 3] = [
        &["UP", "DOWN", "DOWN", "UP", "UP", "RETURN"],
        &[
            "DOWN", "DOWN", "DOWN", "DOWN", "DOWN", "DOWN", "DOWN", "RETURN",
        ],
        &["UP", "UP", "ESCAPE"],
    ];
    for custom in [false, true] {
        for keys in walks {
            let mut rig = setup(&key_script(keys));
            rig.g.destruct.config.allow_custom = custom;
            same_flow(
                &mut rig,
                // SAFETY: the stored game; the script ends the wait.
                || unsafe { ob::ot_s_destruct_JE_modeSelect() },
                de::je_mode_select,
            )
            .unwrap_or_else(|e| panic!("custom {custom}, {keys:?}: {e}"));
        }
    }
}

/// `JE_introScreen`, `JE_modeSelect`, `DrawModeSelectMenu`, `JE_helpScreen`
/// and `JE_pauseScreen` on scripted keys (each case is a fresh pair of
/// engines on its own script).
#[test]
fn screens_match_c() {
    let keys = prop::sample::select(&["UP", "DOWN", "RETURN", "ESCAPE", "SPACE", "A"][..]);
    let args = (
        arb_world(),
        prop_oneof![3 => Just(1u8), 2 => 0u8..5],
        prop::collection::vec(keys, 0..8),
        prop_oneof![Just("RETURN"), Just("ESCAPE")],
        -1i32..7,
    );
    let config = ProptestConfig::default();
    TestRunner::new(ProptestConfig {
        cases: (config.cases / 8).max(4),
        max_shrink_iters: 8,
        ..config
    })
    .run(&args, |(w, which, mut keys, last, mode)| {
        keys.push(last);
        let mut rig = setup(&key_script(&keys));
        apply(&mut rig.g, &w);
        same_flow(
            &mut rig,
            // SAFETY: the stored game; the script ends every wait.
            move || unsafe {
                match which {
                    0 => ob::ot_s_destruct_JE_introScreen(),
                    1 => return ob::ot_s_destruct_JE_modeSelect(),
                    2 => ob::ot_s_destruct_DrawModeSelectMenu(mode),
                    3 => ob::ot_s_destruct_JE_helpScreen(),
                    _ => ob::ot_s_destruct_JE_pauseScreen(),
                }
                0
            },
            move |g| {
                match which {
                    0 => de::je_intro_screen(g)?,
                    1 => return de::je_mode_select(g),
                    2 => de::draw_mode_select_menu(g, mode),
                    3 => de::je_help_screen(g)?,
                    _ => de::je_pause_screen(g)?,
                }
                Ok(0)
            },
        )
    })
    .unwrap();
}

/// A recorded session on `script` that writes a per-frame trace to `trace`.
fn traced_session(
    script: &str,
    trace: &std::path::Path,
) -> (rustyrian_harness::Session, rustyrian_harness::CallLog) {
    use rustyrian_harness::{CallLog, FakeConfig, FakePlatform, InstrumentConfig, Instrumented};
    let mut s = Instrumented::new(
        FakePlatform::new(FakeConfig::default()),
        InstrumentConfig {
            engine: "destruct".into(),
            scenario: "destruct".into(),
            script: Some(rustyrian_harness::script::parse(script).expect("valid script")),
            trace_out: Some(trace.to_path_buf()),
            ..InstrumentConfig::default()
        },
    )
    .expect("trace file");
    let log = CallLog::default();
    s.recorder.log = Some(log.clone());
    (s, log)
}

/// Whole sessions of `JE_destructGame` on a timed script: intro, mode
/// select, AI-against-AI rounds, pause, help, a reload, a second mode and
/// quitting. Both engines must produce the same frames (indices, palette,
/// texture and per-frame platform calls), the same platform calls and the
/// same final state.
#[test]
fn destruct_game_matches_c() {
    let mut script = String::from("ots 1\n");
    for (t, k) in [
        (1000, "RETURN"),     // intro
        (3000, "RETURN"),     // mode select: 5 card war
        (5000, "F11"),        // right player to the AI
        (20000, "P"),         // pause
        (21000, "SPACE"),     // resume
        (22000, "F1"),        // help
        (23000, "SPACE"),     // back
        (26000, "BACKSPACE"), // new map
        (40000, "ESCAPE"),    // to mode select
        (43000, "DOWN"),
        (44000, "RETURN"), // traditional
        (45000, "F10"),    // left player to a human
        (46000, "UP"),     // left player: aim
        (47000, "SPACE"),  // left player: fire
        (60000, "ESCAPE"),
        (63000, "ESCAPE"), // quit
    ] {
        script.push_str(&format!("@{t} key-down {k}\n@{} key-up {k}\n", t + 45));
    }

    let dir = net_support::tmp("destruct-game");
    let (ct, rt) = (dir.join("c.jsonl"), dir.join("r.jsonl"));
    let mut rig = Rig::new("ots 1\n");
    let (cs, c_log) = traced_session(&script, &ct);
    let (rs, r_log) = traced_session(&script, &rt);
    rustyrian_oracle::session::install(cs);
    rig.g.platform = Box::new(rs);
    rig.store();
    rig.video();
    // SAFETY: plain C calls.
    let r = rig.run(
        || unsafe { ob::JE_loadHelpText() },
        helptext::je_load_help_text,
    );
    assert_eq!(r, Ok(()));
    // SAFETY: plain C call.
    let r = rig.run(
        || unsafe { ob::loadSndFile(false) },
        |g| nortsong::load_snd_file(g, false),
    );
    assert_eq!(r, Ok(()));
    mtrand::mt_srand(&mut rig.g, 7);
    rig.store();
    c_log.take();
    r_log.take();

    // SAFETY: plain C call; `exit` is caught by the guard.
    let r = rig.run(|| unsafe { ob::JE_destructGame() }, de::je_destruct_game);
    assert_eq!(r, Ok(()));

    assert_eq!(diff::load::<State>(), rig.g.destruct);
    let calls = (c_log.take(), r_log.take());
    assert!(calls.0 == calls.1, "platform calls differ");
    rig.check();

    // Finish both traces and compare them frame by frame.
    drop(rustyrian_oracle::session::take());
    rig.g.platform = Box::new(rustyrian_harness::test_session().0);
    let read = |p: &std::path::Path| std::fs::read_to_string(p).expect("trace");
    let (c, r) = (read(&ct), read(&rt));
    let (c, r): (Vec<&str>, Vec<&str>) = (c.lines().collect(), r.lines().collect());
    assert!(c.len() > 2000, "{} frames", c.len());
    for (i, (a, b)) in c.iter().zip(&r).enumerate() {
        assert_eq!(a, b, "trace line {i}");
    }
    assert_eq!(c.len(), r.len());
}

/// D-UB-DESTRUCT-2 against C: tall maps whose raw height goes negative
/// convert the way the C build of this architecture converts.
#[test]
fn negative_heights_match_c() {
    let mut rig = setup("ots 1\n");
    let seeds: Vec<u64> = (0..200_000u64)
        .filter(|&s| {
            mtrand::mt_srand(&mut rig.g, s);
            negative_terrain(&rig.g, Some(MAP_TALL))
        })
        .take(8)
        .collect();
    assert_eq!(seeds.len(), 8);
    for seed in seeds {
        mtrand::mt_srand(&mut rig.g, seed);
        same(
            &mut rig,
            // SAFETY: the stored world's height map.
            || unsafe {
                ob::ot_s_destruct_DE_generateBaseTerrain(
                    MAP_TALL,
                    (*c_world()).base_map.as_mut_ptr(),
                );
            },
            |g| de::de_generate_base_terrain(g, MAP_TALL),
        )
        .unwrap_or_else(|e| panic!("seed {seed}: {e}"));
    }
}

/// Fixed states pinning rare branches the generated worlds reach only by
/// chance: an AI heli at the right edge (moving left, hovering over an enemy
/// satellite) and a falling shot that lands with no horizontal speed.
#[test]
fn edge_cases_match_c() {
    let mut rig = setup("ots 1\n");
    let mut w = arb_world()
        .new_tree(&mut TestRunner::deterministic())
        .expect("a world")
        .current();
    w.s.config.max_installations = 2;
    for (p, pl) in w.s.destruct_player.iter_mut().enumerate() {
        pl.is_cpu = p == 0;
        pl.unit_selected = 0;
        pl.units_remaining = 1;
        let heli = DestructUnitS {
            unit_x: 305,
            unit_y: 100.0,
            is_y_in_air: true,
            unit_type: de::UNIT_HELI,
            shot_type: SHOT_SMALL,
            power: 3.0,
            last_move: 3,
            health: 40,
            ..DestructUnitS::default()
        };
        let satellite = DestructUnitS {
            unit_x: 301,
            unit_y: 50.0,
            unit_type: UNIT_SATELLITE,
            shot_type: SHOT_INVALID,
            health: 300,
            ..DestructUnitS::default()
        };
        pl.unit = Some(vec![heli, satellite]);
    }
    for seed in 0..48 {
        w.seed = seed;
        apply(&mut rig.g, &w);
        same(
            &mut rig,
            // SAFETY: the stored world.
            || unsafe { ob::ot_s_destruct_DE_RunTickAI() },
            de::de_run_tick_ai,
        )
        .unwrap_or_else(|e| panic!("AI seed {seed}: {e}"));
    }

    w.s.config.max_shots = 1;
    w.s.config.max_walls = 0;
    w.s.world.map_walls = Some(Vec::new());
    w.s.shot_rec = Some(vec![DestructShotS {
        is_available: false,
        x: 100.0,
        y: 199.0,
        ymov: 0.5,
        shottype: SHOT_SMALL as u32,
        ..DestructShotS::default()
    }]);
    apply(&mut rig.g, &w);
    same(
        &mut rig,
        // SAFETY: the stored world.
        || unsafe { ob::ot_s_destruct_DE_RunTickShots() },
        de::de_run_tick_shots,
    )
    .expect("floor bounce");

    // A heli pressing up at the top of the screen fires a slow shot whose
    // sideways speed is reduced toward 0, from either side.
    w.s.shot_rec = Some(vec![DestructShotS {
        is_available: true,
        ..DestructShotS::default()
    }]);
    for last_move in [-2, 2] {
        let pl = &mut w.s.destruct_player[0];
        pl.moves.actions[de::MOVE_UP as usize] = true;
        let u = &mut pl.unit.as_mut().expect("units")[0];
        u.unit_y = 25.0;
        u.last_move = last_move;
        apply(&mut rig.g, &w);
        same(
            &mut rig,
            // SAFETY: a stored unit.
            || unsafe { ob::ot_s_destruct_DE_MakeShot(0, c_unit(0, 0), -1) },
            |g| de::de_make_shot(g, 0, 0, -1),
        )
        .unwrap_or_else(|e| panic!("heli shot, last move {last_move}: {e}"));
    }
}

/// The R10 guards of the Rust port (D-UB-DESTRUCT-1, -3 to -6; -2 mirrors C):
/// where upstream reads or writes out of bounds, divides by zero or converts
/// an out-of-range float, Rust takes the documented defined path and never
/// panics. Rust only: C is undefined in these states.
#[test]
fn ub_guards_hold() {
    let mut rig = setup("ots 1\n");
    let mut w = arb_world()
        .new_tree(&mut TestRunner::deterministic())
        .expect("a world")
        .current();
    w.s.config.max_installations = 10;
    for p in &mut w.s.destruct_player {
        p.unit
            .as_mut()
            .expect("units")
            .resize(10, DestructUnitS::default());
    }
    apply(&mut rig.g, &w);
    let g = &mut rig.g;

    // D-UB-DESTRUCT-1: a wall at x 309 carves columns 309..=319 only.
    g.destruct.world.base_map = [50; 320];
    g.destruct.world.base_map[319] = 120;
    assert_eq!(de::je_placement_position(g, 309, 12), 120);
    assert!(g.destruct.world.base_map[309..].iter().all(|&h| h == 120));
    assert_eq!(g.destruct.world.base_map[308], 50);

    // D-UB-DESTRUCT-2: a negative height converts as the target's C does,
    // then clamps: to 40 on arm64 (saturated to 0), to 195 on x86-64
    // (wrapped). `terrain_matches_c` compares these maps against C.
    let seed = (0..100_000u64)
        .find(|&s| {
            mtrand::mt_srand(g, s);
            negative_terrain(g, Some(MAP_TALL))
        })
        .expect("a tall map with a negative column");
    mtrand::mt_srand(g, seed);
    de::de_generate_base_terrain(g, MAP_TALL);
    let clamped = if cfg!(target_arch = "x86_64") {
        195
    } else {
        40
    };
    assert!(g.destruct.world.base_map[1..=318].contains(&clamped));

    // D-UB-DESTRUCT-3: a dirt flare at (320, 199) and a shot on row 200
    // touch nothing past the surface.
    let temp = g.video.screen(ScreenId::GameScreen).clone();
    g.destruct.explo_rec = Some(vec![DestructExploS {
        is_available: false,
        x: 320,
        y: 199,
        explowidth: 0,
        explomax: 5,
        explofill: 3,
        explo_type: EXPL_DIRT,
    }]);
    g.destruct.config.max_explosions = 1;
    de::de_run_tick_explosions(g);
    assert_eq!(
        g.video.screen(ScreenId::GameScreen).pixels[..63999],
        temp.pixels[..63999]
    );
    g.destruct.shot_rec = Some(vec![DestructShotS {
        is_available: false,
        x: 100.0,
        y: 200.0,
        ymov: 0.0,
        shottype: 1,
        ..DestructShotS::default()
    }]);
    g.destruct.config.max_shots = 1;
    g.destruct.config.max_walls = 0;
    de::de_run_tick_shots(g);

    // D-UB-DESTRUCT-4: the HUD of a team of only satellites prints no name.
    for p in &mut g.destruct.destruct_player {
        let u = &mut p.unit.as_mut().expect("units")[p.unit_selected as usize];
        u.unit_type = UNIT_SATELLITE;
        u.shot_type = SHOT_INVALID;
    }
    de::de_run_tick_draw_hud(g);

    // D-UB-DESTRUCT-5: `% 0` yields the dividend (an AI heli at x 0;
    // `min walls` one more than `max walls`).
    for p in &mut g.destruct.destruct_player {
        p.is_cpu = true;
        let u = &mut p.unit.as_mut().expect("units")[p.unit_selected as usize];
        u.unit_type = de::UNIT_HELI;
        u.unit_x = 0;
        u.health = 10;
    }
    de::de_run_tick_ai(g);

    // D-UB-DESTRUCT-6: malformed wall limits never index past `mapWalls`.
    for (min, max) in [(21, 20), (40, 3)] {
        g.destruct.config.min_walls = min;
        g.destruct.config.max_walls = max;
        g.destruct.world.map_walls = Some(vec![DestructWallS::default(); max as usize]);
        g.destruct.world.map_flags = MAP_WALLS;
        de::de_generate_walls(g);
        assert_eq!(
            g.destruct.world.map_walls.as_ref().map(Vec::len),
            Some(max as usize)
        );
    }
}
