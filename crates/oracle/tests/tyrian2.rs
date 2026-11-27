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

proptest! {
    #![proptest_config(ProptestConfig { cases: 48, ..ProptestConfig::default() })]

    #[test]
    fn je_create_new_event_enemy_matches(
        frame in prop::sample::select(FRAMES.to_vec()),
        loc in 1u16..=EVENT_MAXIMUM as u16,
        eventdat in 0i16..=800,
        type_ofs in 0u8..50,
        dat2 in prop_oneof![Just(-99i16), any::<i16>()],
        dats: (i8, i8, i8, u8),
        offset in any_enemy_offset(),
        // Often no free slot in the searched range (the early return).
        avail in prop_oneof![
            prop::collection::vec(0u8..3, 100),
            prop::collection::vec(prop_oneof![Just(0u8), Just(2u8)], 100),
        ],
        unique in prop::sample::select(vec![0i16, 21, 26, 99]),
        map: (u16, u16, u16, u16, u16),
        flags: (bool, bool, bool),
    ) {
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
        (bk.map_x, bk.map_x3, bk.back_move, bk.back_move2, bk.back_move3) = map;
        (g.varz.background3x1, g.varz.background3x1b, g.varz.small_enemy_adjust) = flags;
        d.store_to_c();
        // SAFETY: plain C call; `eventLoc >= 1` and the enemy index is valid.
        unsafe { b::JE_createNewEventEnemy(type_ofs, offset, unique) };
        t2::je_create_new_event_enemy(&mut d.game, type_ofs, offset, unique);
        check_enemies(&d)?;
    }

    /// Jumps to any location (or back with 65535) in an event list that
    /// ends at time 65535, as every level's does.
    #[test]
    fn je_event_jump_matches(
        steps in prop::collection::vec(0u16..40, EVENT_MAXIMUM - 1),
        jump in prop_oneof![Just(65535u16), any::<u16>()],
        cur: u16,
        ret: u16,
    ) {
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
            (d.game.varz.cur_loc, d.game.varz.return_loc, d.game.varz.event_loc)
        );
    }

    #[test]
    fn je_search_for_matches(
        avail in prop::collection::vec(0u8..3, 100),
        links in prop::collection::vec(0u8..4, 100),
        values in prop::collection::vec(any::<i16>(), 100),
        galaga: bool,
        pl_type in 0u8..5,
        with_out: bool,
    ) {
        let mut d = Diff::new();
        d.game.varz.enemy_avail.copy_from_slice(&avail);
        for (i, e) in d.game.varz.enemy.iter_mut().enumerate() {
            e.linknum = links[i];
            e.evalue = values[i];
        }
        d.game.config.galaga_mode = galaga;
        d.store_to_c();
        let (mut c_out, mut r_out) = (200u8, 200u8);
        let c_ptr = if with_out { &raw mut c_out } else { std::ptr::null_mut() };
        // SAFETY: plain C call; the out pointer is valid or NULL.
        let c = unsafe { b::JE_searchFor(pl_type, c_ptr) };
        let r = t2::je_search_for(&mut d.game, pl_type, with_out.then_some(&mut r_out));
        prop_assert_eq!(c, r);
        prop_assert_eq!(c_out, r_out);
        check_enemies(&d)?;
    }

    /// Boss bars on a fixture screen: live, dead and missing bosses, one or
    /// two bars, with and without the level timer.
    #[test]
    fn draw_boss_bar_matches(
        frame in prop::sample::select(FRAMES.to_vec()),
        // Bar 0 tracks link 1 and bar 1 link 2 (or none), each with or
        // without a live boss, so one, two and moved bars all occur.
        bars in prop::array::uniform2((any::<bool>(), any::<bool>(), any::<u8>(), 0u8..20)),
        boss_slots in prop::array::uniform2(0usize..100),
        avail in prop::collection::vec(0u8..3, 100),
        // Mostly links no bar tracks, so a live boss usually keeps its bar;
        // sometimes a boss link (a dead twin ends the bar).
        links in prop::collection::vec(
            prop_oneof![200 => prop::sample::select(vec![0u8, 3]), 1 => 1u8..3],
            100,
        ),
        armor in prop::collection::vec(prop_oneof![Just(0u8), Just(255u8), any::<u8>()], 100),
        level_timer: bool,
    ) {
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
            g.tyrian2.boss_bar[b] = BossBarT { link_num, armor, color };
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
    }

    #[test]
    fn je_bar_x_matches(x1 in 0u16..330, y1 in 0u16..210, w in 0u16..60, h in 0u16..12, col: u8) {
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
    }

    /// Enemies drawn from every loaded sheet, with and without a filter, and
    /// enemies without a sheet.
    #[test]
    fn blit_enemy_matches(
        frame in prop::sample::select(FRAMES.to_vec()),
        i in 0u32..100,
        sheet in 0usize..8,
        cycle in 1u8..=20,
        gr: u16,
        sprite_offset in prop::sample::select(vec![0i32, 1, 19, 20]),
        pos in (-30i16..300, -30i16..200, -7i32..8, -7i32..8),
        filter in prop_oneof![Just(0u8), any::<u8>()],
        map_x_ofs in 0u16..30,
    ) {
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
    }
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
    let mut runner = proptest::test_runner::TestRunner::default();
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
