//! Differential tests for `tyrian2.c` / `tyrian2.h`.

use proptest::prelude::*;
use rustyrian_core::lvlmast::{ENEMY_NUM, EVENT_MAXIMUM};
use rustyrian_core::sprite::Sprite2Array;
use rustyrian_core::tyrian2::{self as t2, BossBarT, State};
use rustyrian_core::varz::{self, JeEventRecType, Sprite2Ref};
use rustyrian_core::video::ScreenId;
use rustyrian_core::{Game, episodes, helptext};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::{Mirror, global_mut};
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;

mod net_support;
use net_support::{Rig, data_dir, taps};
mod menus_support;
use menus_support::{Act, output_rect, script as menu_script};

/// Mid-level fixture frames of `demo-cycle` (run `cargo xtask fixtures`).
const FRAMES: [u64; 2] = [500, 2000];

/// The fixture state, with the freeware data directory found afresh.
fn fixture_game(d: &mut Diff, frame: u64) {
    d.game.restore(diff::fixture("demo-cycle", frame));
    d.game.file.custom_data_dir_path = Some(data_dir().as_os_str().as_bytes().to_vec());
    d.game.file.data_dir_path = None;
    d.game.file.data_dir_path_len = 0;
}

/// The C globals the enemy functions write, compared with Rust's.
fn check_enemies(d: &Diff) -> Result<(), TestCaseError> {
    let c = diff::load::<varz::State>();
    let r = &d.game.varz;
    prop_assert!(c.enemy == r.enemy, "enemies differ");
    prop_assert_eq!(c.enemy_avail, r.enemy_avail);
    prop_assert_eq!(
        (c.total_enemy, c.b, c.temp_w, c.x, c.y),
        (r.total_enemy, r.b, r.temp_w, r.x, r.y)
    );
    prop_assert_eq!(
        diff::load::<rustyrian_core::mtrand::State>(),
        d.game.mtrand.clone()
    );
    prop_assert_eq!(d.c_calls(), d.rust_calls());
    Ok(())
}

/// Writes `enemyAvail[i]` in both engines.
fn set_avail(d: &mut Diff, i: usize, v: u8) {
    d.game.varz.enemy_avail[i] = v;
    // SAFETY: C runs only on this thread; no other reference is live.
    unsafe { global_mut(&raw mut b::enemyAvail)[i] = v };
}

/// Every enemy record of every episode through `JE_newEnemy`/`JE_makeEnemy`,
/// cycling enemy offsets, every difficulty (and two outside the table),
/// super arcade mode, and unique shape tables (coins, two-player, an enemy
/// sheet, an unloaded table, none).
#[test]
fn je_new_enemy_matches_on_every_enemy_record() {
    for (ep, frame) in [(1u8, 500u64), (2, 2000), (3, 500), (4, 2000)] {
        let mut d = Diff::new();
        fixture_game(&mut d, frame);
        d.store_to_c();
        // SAFETY: JE_initEpisode may exit, which the guard catches.
        let c = unsafe { rustyrian_oracle::guard_call(|| b::JE_initEpisode(ep)) };
        assert_eq!(c, None);
        episodes::je_init_episode(&mut d.game, ep).expect("episode loads");
        assert_eq!(d.c_calls(), d.rust_calls());
        let ids = d.game.sprite.enemy_sprite_sheet_ids;
        let uniques: [i16; 8] = [0, 0, 0, 21, 26, i16::from(ids[0]), 99, -5];

        for e_dat in 0..=ENEMY_NUM as u16 {
            let k = usize::from(e_dat);
            let offset = [0i32, 25, 50, 75][k % 4];
            let difficulty = (k / 4 % 13) as i8 - 1;
            let arcade = if k % 7 == 0 { 3 } else { 0 };
            let unique = uniques[k % 8];
            d.game.config.difficulty_level = difficulty;
            d.game.config.super_arcade_mode = arcade;
            // SAFETY: C runs only on this thread; no other reference is live.
            unsafe {
                *global_mut(&raw mut b::difficultyLevel) = difficulty;
                *global_mut(&raw mut b::superArcadeMode) = arcade;
            }
            let start = offset as usize;
            for i in start..start + 25 {
                set_avail(&mut d, i, if k % 29 == 0 { 0 } else { 2 });
            }
            if k % 29 != 0 {
                set_avail(&mut d, start + k % 25, 1);
            }
            // SAFETY: plain C call; `eDatI <= ENEMY_NUM`.
            let c = unsafe { b::JE_newEnemy(offset, e_dat, unique) };
            let r = t2::je_new_enemy(&mut d.game, offset, e_dat, unique);
            assert_eq!(c, r, "episode {ep} enemy {e_dat}");
            if k % 25 == 24 || k == ENEMY_NUM {
                check_enemies(&d).unwrap_or_else(|e| panic!("episode {ep} enemy {e_dat}: {e}"));
            }
        }
    }
}

/// An enemy-slot range to search (`JE_createNewEventEnemy` places enemies
/// in the 25 slots from it; 30 takes no position branch).
fn any_enemy_offset() -> impl Strategy<Value = u16> {
    prop::sample::select(vec![0u16, 25, 50, 75, 30])
}

// The enemy, boss-bar and drawing tests below are capped: each case stores
// the full Game (most restore a fixture) and loads large C state back
// (enemies, screens; about 0.05-0.35 s per case).

#[test]
fn je_create_new_event_enemy_matches() {
    let strategy = (
        prop::sample::select(FRAMES.to_vec()),
        1u16..=EVENT_MAXIMUM as u16,
        0i16..=800,
        0u8..50,
        prop_oneof![Just(-99i16), any::<i16>()],
        any::<(i8, i8, i8, u8)>(),
        any_enemy_offset(),
        // Often no free slot in the searched range (the early return).
        prop_oneof![
            prop::collection::vec(0u8..3, 100),
            prop::collection::vec(prop_oneof![Just(0u8), Just(2u8)], 100),
        ],
        prop::sample::select(vec![0i16, 21, 26, 99]),
        any::<(u16, u16, u16, u16, u16)>(),
        any::<(bool, bool, bool)>(),
    );
    diff::capped_runner(48)
        .run(
            &strategy,
            |(frame, loc, eventdat, type_ofs, dat2, dats, offset, avail, unique, map, flags)| {
                let mut d = Diff::new();
                fixture_game(&mut d, frame);
                let g = &mut d.game;
                g.varz.event_loc = loc;
                g.tyrian2.event_rec[usize::from(loc) - 1] = JeEventRecType {
                    eventdat,
                    eventdat2: dat2,
                    eventdat3: dats.0,
                    eventdat5: dats.1,
                    eventdat6: dats.2,
                    eventdat4: dats.3,
                    ..JeEventRecType::default()
                };
                g.varz.enemy_avail.copy_from_slice(&avail);
                let bk = &mut g.backgrnd;
                (
                    bk.map_x,
                    bk.map_x3,
                    bk.back_move,
                    bk.back_move2,
                    bk.back_move3,
                ) = map;
                (
                    g.varz.background3x1,
                    g.varz.background3x1b,
                    g.varz.small_enemy_adjust,
                ) = flags;
                d.store_to_c();
                // SAFETY: plain C call; `eventLoc >= 1` and the enemy index is valid.
                unsafe { b::JE_createNewEventEnemy(type_ofs, offset, unique) };
                t2::je_create_new_event_enemy(&mut d.game, type_ofs, offset, unique);
                check_enemies(&d)?;
                Ok(())
            },
        )
        .expect("JE_createNewEventEnemy matches");
}

/// Jumps to any location (or back with 65535) in an event list that ends at
/// time 65535, as every level's does.
#[test]
fn je_event_jump_matches() {
    let strategy = (
        prop::collection::vec(0u16..40, EVENT_MAXIMUM - 1),
        prop_oneof![Just(65535u16), any::<u16>()],
        any::<u16>(),
        any::<u16>(),
    );
    diff::capped_runner(64)
        .run(&strategy, |(steps, jump, cur, ret)| {
            let mut d = Diff::new();
            let mut t = 0u16;
            for (e, s) in d.game.tyrian2.event_rec.iter_mut().zip(steps) {
                t = t.saturating_add(s);
                e.eventtime = t;
            }
            d.game.tyrian2.event_rec[EVENT_MAXIMUM - 1].eventtime = 65535;
            (d.game.varz.cur_loc, d.game.varz.return_loc) = (cur, ret);
            d.store_to_c();
            // SAFETY: plain C call; an event at time 65535 ends the search.
            unsafe { b::JE_eventJump(jump) };
            t2::je_event_jump(&mut d.game, jump);
            let c = diff::load::<varz::State>();
            prop_assert_eq!(
                (c.cur_loc, c.return_loc, c.event_loc),
                (
                    d.game.varz.cur_loc,
                    d.game.varz.return_loc,
                    d.game.varz.event_loc
                )
            );
            Ok(())
        })
        .expect("JE_eventJump matches");
}

#[test]
fn je_search_for_matches() {
    let strategy = (
        prop::collection::vec(0u8..3, 100),
        prop::collection::vec(0u8..4, 100),
        prop::collection::vec(any::<i16>(), 100),
        any::<bool>(),
        0u8..5,
        any::<bool>(),
    );
    diff::capped_runner(64)
        .run(
            &strategy,
            |(avail, links, values, galaga, pl_type, with_out)| {
                let mut d = Diff::new();
                d.game.varz.enemy_avail.copy_from_slice(&avail);
                for (i, e) in d.game.varz.enemy.iter_mut().enumerate() {
                    e.linknum = links[i];
                    e.evalue = values[i];
                }
                d.game.config.galaga_mode = galaga;
                d.store_to_c();
                let (mut c_out, mut r_out) = (200u8, 200u8);
                let c_ptr = if with_out {
                    &raw mut c_out
                } else {
                    std::ptr::null_mut()
                };
                // SAFETY: plain C call; the out pointer is valid or NULL.
                let c = unsafe { b::JE_searchFor(pl_type, c_ptr) };
                let r = t2::je_search_for(&mut d.game, pl_type, with_out.then_some(&mut r_out));
                prop_assert_eq!(c, r);
                prop_assert_eq!(c_out, r_out);
                check_enemies(&d)?;
                Ok(())
            },
        )
        .expect("JE_searchFor matches");
}

/// Boss bars on a fixture screen: live, dead and missing bosses, one or two
/// bars, with and without the level timer.
#[test]
fn draw_boss_bar_matches() {
    let strategy = (
        prop::sample::select(FRAMES.to_vec()),
        // Bar 0 tracks link 1 and bar 1 link 2 (or none), each with or
        // without a live boss, so one, two and moved bars all occur.
        prop::array::uniform2((any::<bool>(), any::<bool>(), any::<u8>(), 0u8..20)),
        prop::array::uniform2(0usize..100),
        prop::collection::vec(0u8..3, 100),
        // Mostly links no bar tracks, so a live boss usually keeps its bar;
        // sometimes a boss link (a dead twin ends the bar).
        prop::collection::vec(
            prop_oneof![200 => prop::sample::select(vec![0u8, 3]), 1 => 1u8..3],
            100,
        ),
        prop::collection::vec(prop_oneof![Just(0u8), Just(255u8), any::<u8>()], 100),
        any::<bool>(),
    );
    diff::capped_runner(48)
        .run(
            &strategy,
            |(frame, bars, boss_slots, avail, links, armor, level_timer)| {
                let mut d = Diff::new();
                fixture_game(&mut d, frame);
                let g = &mut d.game;
                g.varz.enemy_avail.copy_from_slice(&avail);
                for (i, e) in g.varz.enemy.iter_mut().enumerate() {
                    e.linknum = links[i];
                    e.armorleft = armor[i];
                }
                for (b, (on, alive, armor, color)) in bars.into_iter().enumerate() {
                    let link_num = if on { b as u8 + 1 } else { 0 };
                    g.tyrian2.boss_bar[b] = BossBarT {
                        link_num,
                        armor,
                        color,
                    };
                    if on && alive {
                        let e = &mut g.varz.enemy[boss_slots[b]];
                        e.linknum = link_num;
                        e.armorleft = e.armorleft.max(1);
                        g.varz.enemy_avail[boss_slots[b]] = 0;
                    }
                }
                g.varz.level_timer = level_timer;
                d.store_to_c();
                // SAFETY: plain C call on fixture state.
                unsafe { b::draw_boss_bar() };
                t2::draw_boss_bar(&mut d.game);
                prop_assert_eq!(diff::load::<State>(), d.game.tyrian2.clone());
                prop_assert!(
                    diff::load::<rustyrian_core::video::State>() == d.game.video,
                    "surfaces differ"
                );
                Ok(())
            },
        )
        .expect("draw_boss_bar matches");
}

#[test]
fn je_bar_x_matches() {
    let strategy = (0u16..330, 0u16..210, 0u16..60, 0u16..12, any::<u8>());
    diff::capped_runner(48)
        .run(&strategy, |(x1, y1, w, h, col)| {
            let mut d = Diff::new();
            fixture_game(&mut d, 500);
            d.store_to_c();
            // SAFETY: plain C call on fixture state.
            unsafe { b::ot_s_tyrian2_JE_barX(x1, y1, x1 + w, y1 + h, col) };
            t2::je_bar_x(&mut d.game, x1, y1, x1 + w, y1 + h, col);
            prop_assert!(
                diff::load::<rustyrian_core::video::State>() == d.game.video,
                "surfaces differ"
            );
            Ok(())
        })
        .expect("JE_barX matches");
}

/// Enemies drawn from every loaded sheet, with and without a filter, and
/// enemies without a sheet.
#[test]
fn blit_enemy_matches() {
    let strategy = (
        prop::sample::select(FRAMES.to_vec()),
        0u32..100,
        0usize..8,
        1u8..=20,
        any::<u16>(),
        prop::sample::select(vec![0i32, 1, 19, 20]),
        (-30i16..300, -30i16..200, -7i32..8, -7i32..8),
        prop_oneof![Just(0u8), any::<u8>()],
        0u16..30,
    );
    diff::capped_runner(48)
        .run(
            &strategy,
            |(frame, i, sheet, cycle, gr, sprite_offset, pos, filter, map_x_ofs)| {
                let mut d = Diff::new();
                fixture_game(&mut d, frame);
                let g = &mut d.game;
                let sheets: Vec<Option<Sprite2Ref>> = [
                    Sprite2Ref::EnemySpriteSheets(0),
                    Sprite2Ref::EnemySpriteSheets(1),
                    Sprite2Ref::EnemySpriteSheets(2),
                    Sprite2Ref::EnemySpriteSheets(3),
                    Sprite2Ref::SpriteSheet10,
                    Sprite2Ref::SpriteSheet11,
                ]
                .into_iter()
                .filter(|&r| loaded_count(g, r) > 21)
                .map(Some)
                .chain([None])
                .collect();
                let sprite2s = sheets[sheet % sheets.len()];
                let e = &mut g.varz.enemy[i as usize];
                e.sprite2s = sprite2s;
                e.enemycycle = cycle;
                // Frames inside the sheet's offset table only: others are
                // D-UB-SPRITE-1 territory (`tyrian2.c:84` → `sprite.c:550`).
                let count = sprite2s.map_or(21, |r| loaded_count(g, r));
                let e = &mut g.varz.enemy[i as usize];
                e.egr[usize::from(cycle) - 1] = 1 + gr % (count - 20);
                (e.ex, e.ey) = (pos.0, pos.1);
                e.filter = filter;
                g.backgrnd.temp_map_x_ofs = map_x_ofs;
                d.store_to_c();
                // SAFETY: plain C call; the enemy's sheet is loaded or NULL.
                unsafe { b::ot_s_tyrian2_blit_enemy(b::VGAScreen, i, pos.2, pos.3, sprite_offset) };
                let vga = d.game.video.vga_screen_id();
                t2::blit_enemy(&mut d.game, vga, i, pos.2, pos.3, sprite_offset);
                prop_assert!(
                    diff::load::<rustyrian_core::video::State>() == d.game.video,
                    "surfaces differ"
                );
                Ok(())
            },
        )
        .expect("blit_enemy matches");
}

/// The offset-table entries of a loaded sheet (0 when it is not loaded).
fn loaded_count(g: &Game, r: Sprite2Ref) -> u16 {
    let s = &g.sprite;
    let sheet: &Sprite2Array = match r {
        Sprite2Ref::EnemySpriteSheets(i) => &s.enemy_sprite_sheets[usize::from(i)],
        Sprite2Ref::SpriteSheet10 => &s.sprite_sheet10,
        Sprite2Ref::SpriteSheet11 => &s.sprite_sheet11,
        _ => unreachable!("only enemy sheets are drawn here"),
    };
    sheet
        .data
        .as_deref()
        .filter(|d| d.len() >= 2)
        .map_or(0, |d| u16::from_le_bytes([d[0], d[1]]) / 2)
}

/// `readEpisodeString` over the whole of each episode's `levels?.dat`, with
/// a full-size and a short destination, up to the read error at the end
/// (which exits).
#[test]
fn read_episode_string_walks_every_level_script() {
    for ep in 1..=4u8 {
        for size in [256usize, 20] {
            let mut d = Diff::new();
            let name = format!("levels{ep}.dat");
            episodes::snprintf_into(&mut d.game.episodes.episode_filename, &name);
            d.store_to_c();
            let path = data_dir().join(&name);
            let (cp, cm) = (
                CString::new(path.as_os_str().as_bytes()).expect("no NUL"),
                CString::new("rb").expect("no NUL"),
            );
            // SAFETY: valid C strings.
            let mut c = unsafe { b::ot_s_file_fileOpen(cp.as_ptr(), cm.as_ptr()) };
            let mut r = rustyrian_core::file::file_open(path.as_os_str().as_bytes(), "rb");
            let mut strings = 0;
            loop {
                let (mut cd, mut rd) = (vec![0u8; size], vec![0u8; size]);
                // SAFETY: `cd` holds `size` bytes; exit is caught.
                let cx = unsafe {
                    rustyrian_oracle::guard_call(|| {
                        b::ot_s_tyrian2_readEpisodeString(
                            &raw mut c,
                            cd.as_mut_ptr().cast(),
                            size as _,
                        )
                    })
                };
                let rx = t2::read_episode_string(&mut d.game, &mut r, &mut rd);
                assert_eq!(
                    cx,
                    rx.err().map(|h| h.code),
                    "episode {ep} string {strings}"
                );
                assert_eq!(cd, rd, "episode {ep} string {strings}");
                assert_eq!((c.errnum, c.error), (r.errnum, r.error));
                assert_eq!(d.c_calls(), d.rust_calls());
                if cx.is_some() {
                    break;
                }
                strings += 1;
            }
            assert!(strings > 100, "episode {ep}: {strings} strings");
            // SAFETY: from fileOpen.
            unsafe { b::fileClose(&raw mut c) };
        }
    }
}

#[test]
fn je_read_text_sync_matches() {
    let mut d = Diff::new();
    // SAFETY: plain C call.
    unsafe { b::JE_readTextSync() };
    t2::je_read_text_sync(&mut d.game);
    assert_eq!(d.c_calls(), d.rust_calls());
}

/// Both engines with video, shape tables and help text, on an input script
/// that taps RETURN from `taps_from_ms`.
fn rig(taps_from_ms: u32) -> Rig {
    rig_on(&taps(taps_from_ms, taps_from_ms + 20_000))
}

/// [`rig`] on the given input script.
fn rig_on(script: &str) -> Rig {
    let mut rig = Rig::new(script);
    rig.video();
    // SAFETY: plain C call inside the guard.
    let ok = rig.run(
        || unsafe { b::JE_loadHelpText() },
        helptext::je_load_help_text,
    );
    assert_eq!(ok, Ok(()));
    rig.clear();
    rig
}

/// The logos, waiting out both pictures or cut short by input.
#[test]
fn intro_logos_matches() {
    for from in [100, 5_000, 60_000] {
        let mut rig = rig(from);
        // SAFETY: plain C call inside the guard.
        let r = rig.run(|| unsafe { b::intro_logos() }, t2::intro_logos);
        assert_eq!(r, Ok(()));
        rig.check();
    }
}

/// The SuperTyrian start screen, with and without SCROLL LOCK held, ended
/// by a key, by a mouse click, or by a key after SCROLL LOCK presses (which
/// do not count).
#[test]
fn new_super_tyrian_game_matches() {
    let scripts = [
        taps(1_000, 21_000),
        "ots 1\n@1000 mouse-down 1\n@1045 mouse-up 1\n".to_owned(),
        "ots 1\n@900 key-down 71\n@945 key-up 71\n@2000 key-down SPACE\n@2045 key-up SPACE\n"
            .to_owned(),
    ];
    for scroll_lock in [false, true] {
        for script in &scripts {
            let mut rig = rig_on(script);
            rig.g.keyboard.keysactive[71] = scroll_lock;
            rig.g.helptext.p_name[0][..15].copy_from_slice(b"A long pilot nm");
            rig.store();
            // SAFETY: plain C call inside the guard.
            let r = rig.run(
                || unsafe { b::newSuperTyrianGame() },
                t2::new_super_tyrian_game,
            );
            assert_eq!(r, Ok(()));
            rig.check();
        }
    }
}

/// One enemy record built from arbitrary field values (including the
/// `-99`/`0` reversal codes and the super-arcade substitution of record 534)
/// through `JE_makeEnemy`.
#[test]
fn je_make_enemy_matches_on_synthetic_records() {
    // Capped: each case stores the full Game and loads `varz` back (about
    // 0.05 s per case).
    let mut runner = diff::capped_runner(64);
    let rec = (
        (
            any::<i8>(),
            any::<i8>(),
            any::<i8>(),
            any::<i8>(),
            any::<i8>(),
            any::<i8>(),
        ),
        (any::<i16>(), any::<i16>(), any::<i8>(), any::<i8>()),
        (
            prop_oneof![Just(0i8), Just(-99i8), any::<i8>()],
            prop_oneof![Just(0i8), Just(-99i8), any::<i8>()],
            0u8..4,
            any::<u8>(),
            any::<u16>(),
            any::<i16>(),
        ),
        (any::<u8>(), any::<[u8; 3]>(), any::<i8>(), 0i8..12),
        (
            prop::sample::select(vec![533u16, 534, 0, 850]),
            any::<bool>(),
            0usize..100,
        ),
    );
    runner
        .run(
            &rec,
            |(
                m,
                s,
                (xrev, yrev, animate, launch_freq, launch_type, value),
                (armor, tur, dani, difficulty),
                (e_dat, arcade, slot),
            )| {
                let mut d = Diff::new();
                let g = &mut d.game;
                g.config.super_arcade_mode = u8::from(arcade);
                g.config.difficulty_level = difficulty - 1;
                for r in [usize::from(e_dat), 533] {
                    let e = &mut g.episodes.enemy_dat[r];
                    (e.xmove, e.ymove, e.xaccel, e.yaccel, e.xcaccel, e.ycaccel) = m;
                    (e.startx, e.starty, e.startxc, e.startyc) = s;
                    (e.xrev, e.yrev, e.animate, e.elaunchfreq) = (xrev, yrev, animate, launch_freq);
                    (e.elaunchtype, e.value, e.armor, e.tur, e.dani) =
                        (launch_type, value, armor, tur, dani);
                }
                d.store_to_c();
                // SAFETY: plain C call; `slot < 100` and `eDatI <= ENEMY_NUM`.
                let c = unsafe {
                    b::JE_makeEnemy(
                        (&raw mut b::enemy)
                            .cast::<b::JE_SingleEnemyType>()
                            .add(slot),
                        e_dat,
                        0,
                    )
                };
                let r = t2::je_make_enemy(&mut d.game, slot, e_dat, 0);
                prop_assert_eq!(c, r);
                check_enemies(&d)
            },
        )
        .expect("JE_makeEnemy matches");
}

/// The 'engage' fade on a fixture-like screen: cut short by input, and run
/// to its end.
#[test]
fn je_whoa_matches() {
    for from in [700, 60_000] {
        let mut rig = rig(from);
        let mut x = 0x1234_5678u32;
        for p in rig
            .g
            .video
            .screen_mut(ScreenId::VgaScreenSeg)
            .pixels
            .iter_mut()
        {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12345);
            *p = (x >> 16) as u8;
        }
        rig.store();
        // SAFETY: plain C call inside the guard.
        let r = rig.run(|| unsafe { b::JE_whoa() }, t2::je_whoa);
        assert_eq!(r, Ok(()));
        rig.check();
    }
}

/// Per-slot perturbations of one enemy for `JE_drawEnemy`.
#[derive(Clone, Debug)]
struct EnemyTweak {
    live: bool,
    accel: (u8, u8),
    circle: (i8, i8, i8),
    shots: ([u8; 3], [usize; 3], u8),
    iced: u8,
    damaged: bool,
    launch: (u8, u8, u16, u8),
    score: bool,
    gone: bool,
    shift: (i16, i16),
    /// Reversal points at the current velocities; bounce bounds at the
    /// current position; the launched record's own launch type (with no
    /// launch frequency, so the child is aimed or scattered).
    edges: (bool, bool, Option<u16>),
}

fn any_tweak() -> impl Strategy<Value = EnemyTweak> {
    let accel = || prop_oneof![Just(0u8), 89u8..100];
    (
        any::<bool>(),
        (accel(), accel()),
        (-3i8..4, -3i8..4, 0i8..3),
        (
            prop::array::uniform3(0u8..3),
            prop::array::uniform3(0usize..64),
            1u8..3,
        ),
        prop_oneof![4 => Just(0u8), 1 => 1u8..3],
        prop::bool::weighted(0.1),
        (
            0u8..3,
            1u8..3,
            prop_oneof![Just(0u16), 1u16..=90, 91u16..=850],
            0u8..3,
        ),
        any::<bool>(),
        prop::bool::weighted(0.05),
        (-30i16..30, -30i16..30),
        (
            prop::bool::weighted(0.2),
            prop::bool::weighted(0.2),
            prop::option::weighted(0.3, prop_oneof![1u16..=90, 91u16..=150]),
        ),
    )
        .prop_map(
            |(live, accel, circle, shots, iced, damaged, launch, score, gone, shift, edges)| {
                EnemyTweak {
                    live,
                    accel,
                    circle,
                    shots,
                    iced,
                    damaged,
                    launch,
                    score,
                    gone,
                    shift,
                    edges,
                }
            },
        )
}

/// One `JE_drawEnemy` pass over a fixture's enemy group, with every slot
/// perturbed: steering, circles, bounds, firing every turret kind (weapons
/// the episode's enemies use, magnets, repulsors, the Savara missiles),
/// galaga mode, difficulty, two players, launching all launch kinds, ice and
/// damage.
#[test]
fn je_draw_enemy_matches() {
    // Capped: each case restores a fixture, runs a whole enemy pass and
    // compares the full Game (about 0.75 s per case).
    let strategy = (
        prop::sample::select(FRAMES.to_vec()),
        prop::sample::select(vec![25i32, 50, 75, 100]),
        prop::collection::vec(any_tweak(), 25),
        (any::<bool>(), 0u16..400),
        -1i8..=10,
        prop::array::uniform2((0i32..280, 0i32..190, any::<bool>())),
        // A player placed on one of the group's enemies (magnets reach it).
        prop::array::uniform2(prop::option::weighted(0.3, 0usize..25)),
        any::<bool>(),
        0u16..3,
        any::<bool>(),
        0u16..30,
    );
    diff::capped_runner(48)
        .run(
            &strategy,
            |(
                frame,
                offset,
                tweaks,
                galaga,
                difficulty,
                players,
                near,
                two_player,
                back_move,
                shot_avail,
                map_x_ofs,
            )| {
                let mut d = Diff::new();
                fixture_game(&mut d, frame);
                let g = &mut d.game;
                // Turrets: weapons the loaded episode's enemies fire (valid data keeps
                // their multi-shot positions inside the 8-entry arrays), plus the
                // special turrets.
                let mut turs: Vec<u8> = g
                    .episodes
                    .enemy_dat
                    .iter()
                    .flat_map(|e| e.tur)
                    .filter(|&t| t != 0 && t < 251)
                    .collect();
                turs.sort_unstable();
                turs.dedup();
                turs.extend([0, 251, 252, 253, 254, 255]);
                let live: Vec<usize> = (0..100).filter(|&i| g.varz.enemy_avail[i] != 1).collect();
                prop_assert!(!live.is_empty());

                for (k, t) in tweaks.iter().enumerate() {
                    let i = offset as usize - 25 + k;
                    if t.live && g.varz.enemy_avail[i] == 1 {
                        g.varz.enemy[i] = g.varz.enemy[live[k % live.len()]];
                        g.varz.enemy_avail[i] = 0;
                    }
                    let e = &mut g.varz.enemy[i];
                    (e.xaccel, e.yaccel) = t.accel;
                    (e.excc, e.eycc, e.exccw) = t.circle;
                    e.freq = t.shots.0;
                    // Half the turrets are the special kinds (none, magnets,
                    // repulsor, Savara missiles), half real enemy weapons.
                    e.tur = t.shots.1.map(|j| {
                        if j % 2 == 0 {
                            [0, 251, 252, 253, 254, 255][j / 2 % 6]
                        } else {
                            turs[j % turs.len()]
                        }
                    });
                    e.eshotwait = [t.shots.2; 3];
                    e.iced = t.iced;
                    e.edamaged = t.damaged;
                    (e.launchfreq, e.launchwait, e.launchtype, e.launchspecial) = t.launch;
                    e.scoreitem = t.score;
                    if t.gone && e.enemycycle > 0 {
                        e.egr[usize::from(e.enemycycle) - 1] = 999;
                    }
                    e.ex = e.ex.wrapping_add(t.shift.0);
                    let (rev, bounce, child) = t.edges;
                    if rev {
                        (e.exrev, e.eyrev) = (e.exc, e.eyc);
                    }
                    if bounce {
                        e.xminbounce = e.ex.saturating_add(40);
                        e.yminbounce = e.ey.saturating_add(40);
                    }
                    if let (Some(child), launched @ 1..) = (child, t.launch.2) {
                        let r = &mut g.episodes.enemy_dat[usize::from(launched)];
                        (r.elaunchtype, r.elaunchfreq) = (child, 0);
                    }
                    e.ey = e.ey.wrapping_add(t.shift.1);
                }
                (g.config.galaga_mode, g.varz.galaga_shot_freq) = galaga;
                g.config.difficulty_level = difficulty;
                g.config.two_player_mode = two_player;
                for (p, (x, y, alive)) in g.player.player.iter_mut().zip(players) {
                    (p.x, p.y, p.is_alive) = (x, y, alive);
                }
                for (p, k) in near.iter().enumerate() {
                    if let Some(k) = k {
                        let e = g.varz.enemy[offset as usize - 25 + k];
                        let pl = &mut g.player.player[p];
                        (pl.x, pl.y) = (i32::from(e.ex) + 14, i32::from(e.ey));
                    }
                }
                g.varz.temp_back_move = back_move;
                g.varz.enemy_shot_avail = [shot_avail; varz::ENEMY_SHOT_MAX];
                g.backgrnd.temp_map_x_ofs = map_x_ofs;
                d.store_to_c();
                // SAFETY: plain C call on fixture state; `offset` is 25..=100.
                unsafe { b::JE_drawEnemy(offset) };
                t2::je_draw_enemy(&mut d.game, offset);
                if let Some(diff) = diff::first_difference(&d.c_game_state(), &d.game.snapshot()) {
                    panic!("states differ at {diff}");
                }
                prop_assert_eq!(d.c_calls(), d.rust_calls());
                Ok(())
            },
        )
        .expect("JE_drawEnemy matches");
}

/// Every branch of `JE_createNewEventEnemy` as fixed cases: each slot range
/// (and 30, which positions nothing), with and without a position
/// (`eventdat2` -99), each background3x1/background3x1b combination, the
/// small-enemy adjustment on a size-0 and a size-1 enemy, and a full range.
#[test]
fn je_create_new_event_enemy_every_branch() {
    // One fixture store, then per case only `varz`/`tyrian2`/`backgrnd`
    // (41 cases, about 0.05 s each).
    let mut d = Diff::new();
    fixture_game(&mut d, 500);
    d.game.episodes.enemy_dat[1].esize = 0;
    d.game.episodes.enemy_dat[2].esize = 1;
    d.store_to_c();
    let mut case = 0usize;
    let mut run = |d: &mut Diff, offset: u16, dat2: i16, flags: (bool, bool), free: bool| {
        case += 1;
        let g = &mut d.game;
        let eventdat = 1 + (case % 2) as i16;
        g.varz.event_loc = 3;
        g.tyrian2.event_rec[2] = JeEventRecType {
            eventdat,
            eventdat2: dat2,
            eventdat3: 2,
            eventdat5: -3,
            eventdat6: 4,
            eventdat4: 9,
            ..JeEventRecType::default()
        };
        g.varz.enemy_avail = [2; 100];
        if free {
            g.varz.enemy_avail[usize::from(offset) + case % 25] = 1;
        }
        (g.varz.background3x1, g.varz.background3x1b) = flags;
        g.varz.small_enemy_adjust = !case.is_multiple_of(3);
        (g.backgrnd.map_x, g.backgrnd.map_x3) = (3, 2);
        (
            g.backgrnd.back_move,
            g.backgrnd.back_move2,
            g.backgrnd.back_move3,
        ) = (1, 2, 3);
        g.varz.store_to_c();
        g.tyrian2.store_to_c();
        g.backgrnd.store_to_c();
        // SAFETY: plain C call; `eventLoc >= 1` and the enemy index is valid.
        unsafe { b::JE_createNewEventEnemy(0, offset, 0) };
        t2::je_create_new_event_enemy(&mut d.game, 0, offset, 0);
        check_enemies(d).unwrap_or_else(|e| panic!("case {case} offset {offset}: {e}"));
    };
    for offset in [0u16, 25, 50, 75, 30] {
        for dat2 in [-99i16, 40] {
            for flags in [(false, false), (true, false), (false, true), (true, true)] {
                run(&mut d, offset, dat2, flags, true);
            }
        }
    }
    run(&mut d, 50, 40, (true, true), false);
}

/// A small deterministic generator for event data.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// Uniform in `lo..=hi`.
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() % (hi - lo + 1) as u64) as i64
    }

    /// True one time in `n`.
    fn one_in(&mut self, n: u64) -> bool {
        self.next().is_multiple_of(n)
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.next() as usize % items.len()]
    }
}

/// Event types that create enemies through `JE_createNewEventEnemy`.
const CREATING_EVENTS: [u8; 14] = [6, 7, 10, 12, 15, 17, 18, 23, 32, 49, 50, 51, 52, 56];

/// Shape-bank ids (`shapeFile` index + 1) whose file ships with the data.
fn shipped_shape_banks() -> Vec<i16> {
    let dir = data_dir();
    (1..=rustyrian_core::lvlmast::SHAPE_FILE.len())
        .filter(|&id| {
            let c = rustyrian_core::lvlmast::SHAPE_FILE[id - 1].to_ascii_lowercase();
            dir.join(format!("newsh{}.shp", c as char)).exists()
        })
        .map(|id| id as i16)
        .collect()
}

/// Writes `eventLoc` in both engines.
fn set_event_loc(d: &mut Diff, loc: u16) {
    d.game.varz.event_loc = loc;
    // SAFETY: C runs only on this thread; no other reference is live.
    unsafe { *global_mut(&raw mut b::eventLoc) = loc };
}

/// Event 75 picks random link numbers until `JE_searchFor` finds one whose
/// last live enemy is not moving vertically; with no such link number in the
/// range C loops forever, so give the live enemies in range `eyc == 0`.
/// Returns whether the Rust state changed.
fn ensure_event_75_ends(g: &mut Game, ev: &JeEventRecType) -> bool {
    let (lo, hi) = (ev.eventdat, ev.eventdat2);
    let in_range = |l: u8| (lo..=hi).contains(&i16::from(l));
    let v = &g.varz;
    let clue = (0..100)
        .any(|i| v.enemy_avail[i] == 0 && v.enemy[i].eyc == 0 && in_range(v.enemy[i].linknum));
    let ends = (0..100).rev().any(|i| {
        v.enemy_avail[i] == 0
            && in_range(v.enemy[i].linknum)
            && v.enemy[i].eyc == 0
            && !(i + 1..100)
                .any(|j| v.enemy_avail[j] == 0 && v.enemy[j].linknum == v.enemy[i].linknum)
    });
    if !clue || ends {
        return false;
    }
    for i in 0..100 {
        if g.varz.enemy_avail[i] == 0 && in_range(g.varz.enemy[i].linknum) {
            g.varz.enemy[i].eyc = 0;
        }
    }
    true
}

/// Runs the event at `loc` in both engines; checks the outcome and the
/// platform calls, and with `deep` the module states events write.
fn step_event(d: &mut Diff, loc: u16, deep: bool, what: &dyn std::fmt::Display) {
    set_event_loc(d, loc);
    // SAFETY: plain C call inside the guard; the event data is valid.
    let c = unsafe { rustyrian_oracle::guard_call(|| b::JE_eventSystem()) };
    let r = t2::je_event_system(&mut d.game);
    assert_eq!(c, r.err().map(|h| h.code), "{what}");
    assert_eq!(d.c_calls(), d.rust_calls(), "{what}: platform calls");
    let same = || {
        diff::load::<varz::State>() == d.game.varz
            && diff::load::<State>() == d.game.tyrian2
            && diff::load::<rustyrian_core::config::State>() == d.game.config
            && diff::load::<rustyrian_core::backgrnd::State>() == d.game.backgrnd
            && diff::load::<rustyrian_core::mtrand::State>() == d.game.mtrand
    };
    if deep && !same() {
        check_whole(d, what);
        panic!("{what}: module states differ");
    }
}

/// The whole C state equals the Rust state.
fn check_whole(d: &Diff, what: &dyn std::fmt::Display) {
    if let Some(diff) = diff::first_difference(&d.c_game_state(), &d.game.snapshot()) {
        panic!("{what}: states differ at {diff}");
    }
}

/// Random game-mode flags and counters the events read, in both engines.
fn shuffle_modes(d: &mut Diff, rng: &mut Rng) {
    let g = &mut d.game;
    let c = &mut g.config;
    c.galaga_mode = rng.one_in(4);
    c.two_player_mode = rng.one_in(3);
    c.one_player_action = rng.one_in(3);
    c.super_tyrian = rng.one_in(4);
    c.super_arcade_mode = if rng.one_in(3) {
        rng.range(1, 7) as u8
    } else {
        0
    };
    c.difficulty_level = rng.range(0, 10) as i8;
    c.initial_difficulty = rng.range(0, 10) as i8;
    let v = &mut g.varz;
    v.all_players_gone = rng.one_in(4);
    v.end_level = rng.one_in(3);
    v.first_game_over = !rng.one_in(4);
    v.galaga_shot_freq = rng.range(8, 10) as u16;
    for f in v.global_flags.iter_mut() {
        *f = rng.one_in(2);
    }
    let lives = rng.pick(&[0u8, 3, 11]);
    g.player.player[0].set_lives(lives);
    g.config.store_to_c();
    g.varz.store_to_c();
    g.player.store_to_c();
}

/// Frees a few enemy slots in both engines, so creating events find room.
fn free_some_slots(d: &mut Diff, rng: &mut Rng) {
    for _ in 0..3 {
        let i = rng.range(0, 99) as usize;
        set_avail(d, i, 1);
    }
}

/// Synthetic data for an event of type `ty` that keeps C defined: indices
/// into fixed tables stay in range, as `JE_loadMap`'s validation of level
/// data guarantees.
fn synthetic_event(ty: u8, rng: &mut Rng, g: &Game, banks: &[i16]) -> JeEventRecType {
    let any16 = |rng: &mut Rng| match rng.range(0, 3) {
        0 => -99,
        1 => 0,
        _ => rng.range(-300, 900) as i16,
    };
    let link = |rng: &mut Rng| -> u8 {
        if rng.one_in(2) {
            g.varz.enemy[rng.range(0, 99) as usize].linknum
        } else {
            rng.range(0, 3) as u8
        }
    };
    let mut ev = JeEventRecType {
        eventtime: rng.range(0, 3000) as u16,
        eventtype: ty,
        eventdat: any16(rng),
        eventdat2: any16(rng),
        eventdat3: rng.pick(&[0i8, 1, 2, 3, 16, 17, 99, -99, 80, 85, 89, -5]),
        eventdat5: rng.pick(&[0i8, 3, -2, -99, 40]),
        eventdat6: rng.pick(&[0i8, 1, 2, 3, 4, 5, -99, 7]),
        eventdat4: link(rng),
    };
    match ty {
        5 => {
            let bank = |rng: &mut Rng| {
                if rng.one_in(3) {
                    rng.range(-2, 0) as i16
                } else {
                    rng.pick(banks)
                }
            };
            ev.eventdat = bank(rng);
            ev.eventdat2 = bank(rng);
            ev.eventdat3 = bank(rng) as i8;
            ev.eventdat4 = bank(rng).max(0) as u8;
        }
        _ if CREATING_EVENTS.contains(&ty) => {
            ev.eventdat = rng.range(0, ENEMY_NUM as i64 - 3) as i16;
            if (49..=52).contains(&ty) {
                ev.eventdat3 = rng.pick(&[0i8, 21, 26, 99, -5]);
            }
        }
        4 => ev.eventdat = rng.range(0, 4) as i16,
        11 => ev.eventdat = rng.range(0, 2) as i16,
        16 => ev.eventdat = rng.range(-1, 11) as i16,
        25 => ev.eventdat = rng.range(0, 51) as i16,
        35 => ev.eventdat = rng.range(1, rustyrian_core::musmast::MUSIC_NUM as i64) as i16,
        46 => ev.eventdat = rng.range(-12, 12) as i16,
        53 => ev.eventdat = rng.pick(&[99i16, 0, 1]),
        61 => {
            ev.eventdat = rng.range(1, 10) as i16;
            ev.eventdat2 = rng.range(0, 1) as i16;
            ev.eventdat3 = rng.range(0, 4) as i8;
        }
        64 => {
            ev.eventdat = rng.range(1, 9) as i16;
            ev.eventdat2 = rng.range(0, 1) as i16;
        }
        66 | 63 | 80 => ev.eventdat2 = rng.range(0, 4) as i16,
        70 => {
            let (l2, l3, l4) = (link(rng), link(rng), link(rng));
            ev.eventdat2 = rng.pick(&[0i16, i16::from(l2), 5]);
            ev.eventdat3 = rng.pick(&[0i8, l3 as i8]);
            ev.eventdat4 = rng.pick(&[0u8, l4]);
        }
        71 => ev.eventdat2 = rng.pick(&[0i16, 300, 2000, -1]),
        75 => {
            ev.eventdat = i16::from(link(rng)).min(250);
            ev.eventdat2 = ev.eventdat + rng.range(0, 5) as i16;
            ev.eventdat3 = rng.range(80, 89) as i8;
            ev.eventdat4 = rng.range(0, 4) as u8;
        }
        // Positions inside `mainmap` (a pointer outside it is undefined).
        77 | 81 => {
            ev.eventdat = rng.range(0, 8000) as i16;
            let far = rng.range(0, 8000) as i16;
            // 77 ignores a non-positive `eventdat2`; 81 uses it as a position.
            ev.eventdat2 = rng.pick(&[0i16, if ty == 77 { -3 } else { 2 }, far]);
        }
        _ => {}
    }
    if matches!(ty, 1 | 19 | 20 | 24 | 27 | 55) && rng.one_in(2) {
        ev.eventdat = rng.range(1, 12) as i16;
    }
    ev
}

/// `JE_eventSystem` on synthetic events of every type (and invalid types)
/// with data that reaches every branch, under random game modes; runs both
/// engines in lock step from each mid-level fixture.
#[test]
fn je_event_system_matches_on_synthetic_events() {
    // Lock step from one fixture store per frame; about 2 ms per event.
    let banks = shipped_shape_banks();
    for (round, frame) in FRAMES.into_iter().enumerate() {
        let mut d = Diff::new();
        fixture_game(&mut d, frame);
        // Synthetic jumps (and returns to locations set by event 75) may
        // target any time: the last slot ends every search, as the level's
        // final event at time 65535 does for its own script.
        d.game.tyrian2.event_rec[EVENT_MAXIMUM - 1].eventtime = 65535;
        d.store_to_c();
        let max_event = usize::from(d.game.varz.max_event);
        assert!(max_event > 10);
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ frame);
        for pass in 0..16 {
            for ty in 0..=90u8 {
                let what = format!("frame {frame} pass {pass} type {ty}");
                if rng.one_in(4) {
                    shuffle_modes(&mut d, &mut rng);
                }
                if CREATING_EVENTS.contains(&ty) {
                    free_some_slots(&mut d, &mut rng);
                }
                // Keep the level's final event (time 65535) as the end of
                // every jump's search.
                let loc = rng.range(1, max_event as i64 - 6) as u16;
                let ev = synthetic_event(ty, &mut rng, &d.game, &banks);
                d.game.tyrian2.event_rec[usize::from(loc) - 1] = ev;
                if ty == 70 && rng.one_in(2) {
                    // No live enemy: event 70's search finds nothing.
                    d.game.varz.enemy_avail = [1; 100];
                }
                if ty == 75 && rng.one_in(2) {
                    // A live, still enemy in the link range.
                    let i = rng.range(0, 99) as usize;
                    set_avail(&mut d, i, 0);
                    d.game.varz.enemy[i].linknum = ev.eventdat2 as u8;
                    d.game.varz.enemy[i].eyc = 0;
                }
                if ty == 75 {
                    ensure_event_75_ends(&mut d.game, &ev);
                }
                d.game.varz.store_to_c();
                d.game.tyrian2.store_to_c();
                step_event(&mut d, loc, true, &what);
            }
            check_whole(&d, &format!("frame {frame} pass {pass} (round {round})"));
        }
    }
}

/// The event lists of every level of episode `ep`, read as `JE_loadMap`
/// reads them.
fn shipped_level_events(ep: u8) -> Vec<Vec<JeEventRecType>> {
    let bytes = std::fs::read(data_dir().join(format!("tyrian{ep}.lvl"))).expect("level file");
    let u16_at = |p: usize| u16::from_le_bytes([bytes[p], bytes[p + 1]]);
    let lvl_num = usize::from(u16_at(0));
    let pos = |i: usize| {
        u32::from_le_bytes(bytes[2 + 4 * i..6 + 4 * i].try_into().expect("4 bytes")) as usize
    };
    (0..lvl_num / 2)
        .map(|k| {
            // char_mapFile, char_shapeFile, mapX, mapX2, mapX3
            let mut p = pos(k * 2) + 2 + 6;
            let enemies = usize::from(u16_at(p));
            p += 2 + 2 * enemies;
            let max_event = usize::from(u16_at(p));
            p += 2;
            assert!(max_event < EVENT_MAXIMUM, "episode {ep} level {k}");
            (0..max_event)
                .map(|i| {
                    let e = &bytes[p + 11 * i..p + 11 * (i + 1)];
                    JeEventRecType {
                        eventtime: u16::from_le_bytes([e[0], e[1]]),
                        eventtype: e[2],
                        eventdat: i16::from_le_bytes([e[3], e[4]]),
                        eventdat2: i16::from_le_bytes([e[5], e[6]]),
                        eventdat3: e[7] as i8,
                        eventdat5: e[8] as i8,
                        eventdat6: e[9] as i8,
                        eventdat4: e[10],
                    }
                })
                .collect()
        })
        .collect()
}

/// Every event of every level of the four episodes, in file order, under
/// game modes that vary per level, in both engines in lock step.
#[test]
fn je_event_system_runs_every_shipped_event() {
    // Lock step from one fixture store per episode; whole-state comparison
    // per level (about 0.3 s), module comparison every 16 events.
    for ep in 1..=4u8 {
        let mut d = Diff::new();
        fixture_game(&mut d, FRAMES[usize::from(ep) % 2]);
        d.store_to_c();
        // SAFETY: JE_initEpisode may exit, which the guard catches.
        let c = unsafe { rustyrian_oracle::guard_call(|| b::JE_initEpisode(ep)) };
        assert_eq!(c, None);
        episodes::je_init_episode(&mut d.game, ep).expect("episode loads");
        assert_eq!(d.c_calls(), d.rust_calls());
        let mut rng = Rng(0xD1B5_4A32_D192_ED03 ^ u64::from(ep));
        let levels = shipped_level_events(ep);
        assert!(levels.len() > 5, "episode {ep}");
        for (k, events) in levels.iter().enumerate() {
            shuffle_modes(&mut d, &mut rng);
            let g = &mut d.game;
            g.tyrian2.event_rec[..events.len()].copy_from_slice(events);
            g.tyrian2.event_rec[events.len()].eventtime = 65500;
            g.varz.max_event = events.len() as u16;
            g.tyrian2.store_to_c();
            g.varz.store_to_c();
            for loc in 1..=events.len() as u16 {
                let what = format!("episode {ep} level {k} event {loc}");
                let ev = d.game.tyrian2.event_rec[usize::from(loc) - 1];
                if CREATING_EVENTS.contains(&ev.eventtype) {
                    free_some_slots(&mut d, &mut rng);
                }
                if ev.eventtype == 75 && ensure_event_75_ends(&mut d.game, &ev) {
                    d.game.varz.store_to_c();
                }
                step_event(&mut d, loc, loc % 16 == 0, &what);
            }
            check_whole(&d, &format!("episode {ep} level {k}"));
        }
    }
}

/// The level warning text: red or normal placement, ended by input, by the
/// slow countdown or by ESC already held, with the warning animation on or
/// off, and a network halt while it is drawn.
#[test]
fn je_display_text_matches() {
    for case in 0..10u32 {
        // Only input ends the wait (in the slow case through the glow
        // animation, which zeroes `frameCountMax`); early or late taps.
        let script = if case % 3 == 2 {
            taps(200, 20_000)
        } else {
            taps(1_500, 20_000)
        };
        let mut rig = rig_on(&script);
        let f = &mut rig.g.fonthand;
        f.level_warning_lines = (case % 4 + 1) as u8;
        for (i, line) in f.level_warning_text.iter_mut().enumerate() {
            let text = format!("WARNING LINE {i}");
            line[..text.len()].copy_from_slice(text.as_bytes());
        }
        f.warning_red = case % 2 == 1;
        f.level_warning_display = case % 4 < 2;
        rig.g.nortsong.frame_count_max = if case % 3 == 2 { 3 } else { 0 };
        rig.g.keyboard.esc_pressed = case == 5;
        rig.g.network.halt_game = case == 9;
        rig.store();
        rig.clear();
        // SAFETY: plain C call inside the guard.
        let r = rig.run(|| unsafe { b::JE_displayText() }, t2::je_display_text);
        if case == 9 {
            assert_eq!(r, Err(0));
            rig.check_halted();
        } else {
            assert_eq!(r, Ok(()));
            rig.check();
        }
    }
}

/// Key taps one per `gap` ms from 1 s (then ESC taps, so every loop ends).
fn key_script(keys: &[&'static str], gap: u32) -> String {
    let acts: Vec<(Act, u32)> = keys.iter().map(|&k| (Act::Key(k), gap)).collect();
    menu_script(output_rect(), 1_000, &acts)
}

/// `newGame`: every game mode, every episode's starting cash, the rich
/// mode, and cancelling in each of the three menus.
#[test]
fn new_game_matches() {
    let runs: [(&[&'static str], bool); 10] = [
        (&["RETURN", "RETURN", "RETURN"], false),
        (&["RETURN", "DOWN", "RETURN", "RETURN"], false),
        (&["RETURN", "DOWN", "DOWN", "RETURN", "RETURN"], false),
        (&["RETURN", "UP", "RETURN", "RETURN"], false),
        (&["DOWN", "RETURN", "RETURN", "RETURN"], false),
        (&["DOWN", "DOWN", "RETURN", "RETURN", "RETURN"], false),
        (&["RETURN", "RETURN", "RETURN"], true),
        (&["RETURN", "ESCAPE"], false),
        (&["RETURN", "RETURN", "ESCAPE"], false),
        (&["ESCAPE"], false),
    ];
    for (keys, rich) in runs {
        let mut rig = rig_on(&key_script(keys, 1_500));
        rig.g.episodes.episode_avail = [true; 5];
        rig.g.params.rich_mode = rich;
        rig.store();
        rig.clear();
        // SAFETY: plain C call inside the guard.
        let r = rig.run(|| unsafe { b::newGame() }, t2::new_game);
        assert!(r.is_ok(), "{keys:?}");
        rig.check();
    }
}

/// `newSuperArcadeGame` for every super-arcade ship, started or cancelled.
#[test]
fn new_super_arcade_game_matches() {
    for i in 0..7u32 {
        let keys: &[&'static str] = if i == 4 {
            &["ESCAPE"]
        } else {
            &["RETURN", "RETURN", "RETURN"]
        };
        let mut rig = rig_on(&key_script(keys, 1_500));
        rig.g.episodes.episode_avail = [true; 5];
        rig.store();
        rig.clear();
        // SAFETY: plain C call inside the guard.
        let r = rig.run(
            || unsafe { b::newSuperArcadeGame(i) },
            |g| t2::new_super_arcade_game(g, i),
        );
        assert_eq!(r, Ok(i != 4), "ship {i}");
        rig.check();
    }
}

/// A packet: header (`type`, `sync`) then `body`.
fn net_pkt(type_: u16, sync: u16, body: &[u8]) -> Vec<u8> {
    let mut v = type_.to_be_bytes().to_vec();
    v.extend_from_slice(&sync.to_be_bytes());
    v.extend_from_slice(body);
    v
}

/// `networkStartScreen` as the host (choosing an episode and difficulty, or
/// cancelling) and as the guest (waiting for the details packet).
#[test]
fn network_start_screen_matches() {
    use rustyrian_core::network as net;
    // (player, cancel, late): the guest's details either arrive with the
    // handshake or later, while it waits (receives fail for a while).
    for (player, cancel, late) in [
        (1u32, false, false),
        (1, true, false),
        (2, false, false),
        (2, false, true),
    ] {
        let keys: &[&'static str] = if cancel {
            &["ESCAPE"]
        } else {
            &["RETURN", "RETURN"]
        };
        let acts: Vec<(Act, u32)> = keys.iter().map(|&k| (Act::Key(k), 1_500)).collect();
        let mut rig = rig_on(&menu_script(output_rect(), 3_000, &acts));
        rig.net();
        rig.g.episodes.episode_avail = [true; 5];
        let mut s = rig.g.network.clone();
        s.is_network_game = true;
        s.network_delay = 1;
        s.this_player_num = player;
        s.network_opponent_host = Some(b"127.0.0.1".to_vec());
        s.network_player_name = Some(b"HOST".to_vec());
        net_support::store_net(&mut rig, s);
        rig.store();
        rig.clear();
        let mut connect = Vec::new();
        // Every episode available on the opponent's side too.
        for v in [net::NET_VERSION, 1, 0x1F, (3 - player) as u16] {
            connect.extend_from_slice(&v.to_be_bytes());
        }
        connect.extend_from_slice(b"PEER\0");
        rig.push(&net_pkt(net::PACKET_CONNECT, 7, &connect));
        for sync in 0..4 {
            rig.push(&net_pkt(net::PACKET_ACKNOWLEDGE, sync, &[]));
        }
        if player == 2 {
            let mut details = 2u16.to_be_bytes().to_vec();
            details.extend_from_slice(&3u16.to_be_bytes());
            rig.push(&net_pkt(net::PACKET_DETAILS, 8, &details));
        }
        if late {
            // Each `fail_after(k)` fails receive call k+1 (failed calls
            // count too): the details wait behind the CONNECT and four
            // acknowledgments.
            for k in 5..600 {
                rig.c_fake.fail_after("SDLNet_UDP_Recv", k);
                rig.r_fake.fail_after("SDLNet_UDP_Recv", k);
            }
        }
        // SAFETY: plain C call inside the guard.
        let r = rig.run(
            || unsafe { b::networkStartScreen() },
            t2::network_start_screen,
        );
        // Only a cancel exits (through network_tyrian_halt).
        assert_eq!(r.is_err(), cancel, "player {player}: {r:?}");
        match r {
            Ok(()) => rig.check(),
            Err(_) => rig.check_halted(),
        }
    }
}

/// The special names typed on the title screen, from the help text.
fn special_names() -> Vec<String> {
    let rig = rig_on("ots 1\n");
    rig.g
        .helptext
        .special_name
        .iter()
        .map(|n| {
            let len = n.iter().position(|&c| c == 0).unwrap_or(n.len());
            String::from_utf8_lossy(&n[..len]).into_owned()
        })
        .collect()
}

/// One title-screen run on both engines; returns its result.
fn run_title(acts: &[(Act, u32)], logo_up: bool) -> bool {
    // Without interactions: no input at all (the idle demo).
    let script = if acts.is_empty() {
        "ots 1\n".to_owned()
    } else {
        menu_script(output_rect(), 500, acts)
    };
    let mut rig = rig_on(&script);
    rig.g.episodes.episode_avail = [true; 5];
    rig.g.varz.move_tyrian_logo_up = logo_up;
    rig.store();
    rig.clear();
    // SAFETY: plain C call inside the guard.
    let r = rig.run(|| unsafe { b::titleScreen() }, t2::title_screen);
    let r = r.unwrap_or_else(|code| panic!("{acts:?}: exit {code}"));
    rig.check();
    r
}

/// The script key name of an uppercase letter.
fn letter(c: char) -> &'static str {
    const LETTERS: [&str; 26] = [
        "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R",
        "S", "T", "U", "V", "W", "X", "Y", "Z",
    ];
    LETTERS[(c.to_ascii_uppercase() as u8 - b'A') as usize]
}

/// `titleScreen`: every menu item by keyboard (with wrap-around) and by
/// mouse, ESC and right-click, the idle demo, the logo animation, and every
/// special name (super-arcade ships, Destruct, SuperTyrian), including a
/// broken-off name and a cancelled super-arcade start.
#[test]
fn title_screen_matches() {
    use Act::{Click, Key, Move};
    let k = |name: &'static str| (Key(name), 1_200);
    let item = |i: i32| (160, 104 + 13 * i + 5);
    // Idle until the demo starts.
    assert!(run_title(&[], true));
    assert!(!run_title(&[k("UP"), k("RETURN")], false));
    assert!(run_title(&[k("UP"), k("UP"), k("SPACE")], false));
    assert!(!run_title(&[k("ESCAPE")], false));
    // New game started, then cancelled (back to the menu, then quit).
    assert!(run_title(
        &[k("RETURN"), k("RETURN"), k("RETURN"), k("RETURN")],
        false
    ));
    assert!(!run_title(&[k("RETURN"), k("ESCAPE")], false));
    // Load (left again), high scores, instructions, setup; then ESC taps.
    for down in 1..=4 {
        let mut acts = vec![k("DOWN"); down];
        acts.push(k("RETURN"));
        assert!(!run_title(&acts, false), "item {down}");
    }
    // Mouse: hover an item, click one, right-click.
    let (x, y) = item(6);
    assert!(!run_title(&[(Move(x, y), 1_200), (Click(1), 300)], false));
    let (x, y) = item(5);
    assert!(run_title(
        &[
            (Move(x, y), 1_200),
            (Move(x, y - 13), 300),
            (Move(x, y), 300),
            (Click(1), 300)
        ],
        false
    ));
    assert!(!run_title(&[(Move(5, 5), 1_200), (Click(3), 300)], false));
    // Special names: typed letters; F1 is not ASCII.
    let names = special_names();
    for (i, name) in names.iter().enumerate() {
        let mut acts: Vec<(Act, u32)> = vec![(Key("F1"), 1_200)];
        if i == 0 {
            // A broken-off name first.
            acts.extend(name.chars().take(2).map(|c| (Key(letter(c)), 200)));
            acts.push((Key("Z"), 200));
        }
        acts.extend(name.chars().map(|c| (Key(letter(c)), 200)));
        if i < 7 {
            let confirm = if i == 1 { "ESCAPE" } else { "RETURN" };
            acts.extend([
                (Key(confirm), 1_500),
                (Key("RETURN"), 1_500),
                (Key("RETURN"), 1_500),
            ]);
        }
        let started = run_title(&acts, false);
        assert_eq!(started, i != 1, "name {i} {name}");
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
        boss_bar: [
            BossBarT {
                link_num: 1,
                armor: 2,
                color: 3,
            },
            BossBarT {
                link_num: 4,
                armor: 5,
                color: 6,
            },
        ],
        quit: true,
        load_level_ok: true,
        level_enemy_max: 40,
        level_enemy_frequency: 96,
        ..State::default()
    };
    for (i, e) in s.event_rec.iter_mut().enumerate() {
        *e = JeEventRecType {
            eventtime: i as u16,
            eventtype: (i % 256) as u8,
            eventdat: -(i as i16),
            eventdat2: i as i16 * 3,
            eventdat3: -1,
            eventdat5: 2,
            eventdat6: -3,
            eventdat4: 4,
        };
    }
    s.level_enemy[39] = 900;
    s.temp_str[..4].copy_from_slice(b"abc\0");
    s.temp_str[30] = 0xfe;
    s.item_avail[8][9] = 7;
    s.item_avail_max[8] = 10;
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}
