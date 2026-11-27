//! Differential tests for `episodes.c` / `episodes.h`.

use proptest::prelude::*;
use rustyrian_core::episodes::{self as e, State};
use rustyrian_core::game::Halt;
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;
use std::path::{Path, PathBuf};

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data")
        .canonicalize()
        .expect("data/ (run `cargo xtask fetch-data`)")
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir()
        .join(format!("rustyrian-episodes-{}", std::process::id()))
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("scratch dir");
    d
}

fn with_data_dir(d: &mut Diff, dir: &Path) {
    use std::os::unix::ffi::OsStrExt;
    d.game.file.custom_data_dir_path = Some(dir.as_os_str().as_bytes().to_vec());
}

fn check_states(d: &Diff) {
    assert_eq!(diff::load::<State>(), d.game.episodes);
    assert_eq!(diff::load::<rustyrian_core::lvllib::State>(), d.game.lvllib);
    assert_eq!(d.c_calls(), d.rust_calls());
}

/// `JE_initEpisode(ep)` in both engines from episode `from`.
fn init_both(dir: &Path, from: u8, ep: u8) -> (Diff, Option<i32>, Result<(), Halt>) {
    let mut d = Diff::new();
    with_data_dir(&mut d, dir);
    d.game.episodes.episode_num = from;
    d.game.lvllib.level_filename = *b"stale-name.x\0";
    d.store_to_c();
    // SAFETY: JE_initEpisode may exit, which the guard catches.
    let c = unsafe { rustyrian_oracle::guard_call(|| b::JE_initEpisode(ep)) };
    let r = e::je_init_episode(&mut d.game, ep);
    assert_eq!(c.is_some(), r.is_err(), "{from} -> {ep}");
    if let Some(code) = c {
        assert_eq!(r, Err(Halt { code }));
    }
    check_states(&d);
    (d, c, r)
}

/// Item and level data of all four episodes load identically (episodes 1-3
/// from `tyrian.hdt`, episode 4 from its level file).
#[test]
fn je_init_episode_loads_every_episode() {
    for ep in 1..=4 {
        let (d, c, _) = init_both(&data_dir(), 0, ep);
        assert_eq!(c, None, "episode {ep}");
        assert_ne!(
            d.game.episodes.weapons[1],
            Default::default(),
            "episode {ep}"
        );
    }
    // Switching between episodes, and staying on one (no reload).
    let (_, c, _) = init_both(&data_dir(), 4, 2);
    assert_eq!(c, None);
    let (d, c, _) = init_both(&data_dir(), 3, 3);
    assert_eq!((c, d.game.episodes.weapons[1]), (None, Default::default()));
}

/// Episode numbers without a level file, including ones whose names
/// `snprintf` truncates.
#[test]
fn je_init_episode_missing_files_match() {
    for ep in [5, 9, 10, 99, 100, 255] {
        let (_, c, _) = init_both(&data_dir(), 1, ep);
        assert_eq!(c, Some(1), "episode {ep}");
    }
    let (_, c, _) = init_both(&tmp("empty"), 0, 1);
    assert_eq!(c, Some(1));
}

/// Truncated item data (read errors) and a missing `tyrian.hdt`.
#[test]
fn je_load_item_dat_truncated_files_match() {
    let hdt = std::fs::read(data_dir().join("tyrian.hdt")).expect("tyrian.hdt");
    let lvl4 = std::fs::read(data_dir().join("tyrian4.lvl")).expect("tyrian4.lvl");
    let lvl1 = std::fs::read(data_dir().join("tyrian1.lvl")).expect("tyrian1.lvl");
    let pos = u32::from_le_bytes(hdt[..4].try_into().expect("4 bytes")) as usize;
    let mut failed = 0;
    for (i, len) in [0, 2, 4, pos, pos + 100, pos + 5000, hdt.len() - 1]
        .into_iter()
        .enumerate()
    {
        let dir = tmp(&format!("hdt{i}"));
        std::fs::write(dir.join("tyrian1.lvl"), &lvl1).expect("write");
        if len > 0 {
            std::fs::write(dir.join("tyrian.hdt"), &hdt[..len]).expect("write");
        }
        let (_, c, _) = init_both(&dir, 0, 1);
        failed += usize::from(c.is_some());
    }
    assert!(failed >= 5, "{failed}");
    let dir = tmp("lvl4");
    std::fs::write(dir.join("tyrian4.lvl"), &lvl4[..lvl4.len() - 10]).expect("write");
    let (_, c, _) = init_both(&dir, 0, 4);
    assert_eq!(c, Some(1));
}

/// R10 guard (UF-5): an episode-4 level file without levels halts instead of
/// indexing before `lvlPos`. Rust only: upstream's behaviour is undefined.
#[test]
fn je_load_item_dat_rejects_level_file_without_levels() {
    let dir = tmp("nolevels");
    std::fs::write(dir.join("tyrian4.lvl"), [0u8, 0, 1, 2, 3]).expect("write");
    let mut d = Diff::new();
    with_data_dir(&mut d, &dir);
    assert_eq!(e::je_init_episode(&mut d.game, 4), Err(Halt { code: 1 }));
    assert_eq!(d.game.lvllib.lvl_num, 0);
    let calls = format!("{:?}", d.rust_calls());
    assert!(calls.contains("no levels"), "{calls}");
}

/// `JE_loadItemDat` for episode 4 when the level file cannot be opened.
#[test]
fn je_load_item_dat_episode4_missing_level_file_matches() {
    let mut d = Diff::new();
    with_data_dir(&mut d, &tmp("ep4missing"));
    d.game.episodes.episode_num = 4;
    d.game.lvllib.level_filename = *b"tyrian4.lvl\0\0";
    d.game.lvllib.lvl_num = 3;
    d.store_to_c();
    // SAFETY: JE_loadItemDat may exit, which the guard catches.
    let c = unsafe { rustyrian_oracle::guard_call(|| b::JE_loadItemDat()) };
    let r = e::je_load_item_dat(&mut d.game);
    assert_eq!(c, Some(1));
    assert_eq!(r, Err(Halt { code: 1 }));
    check_states(&d);
}

#[test]
fn je_scan_for_episodes_matches() {
    let full = tmp("scan");
    for ep in [1, 3, 5] {
        std::fs::write(full.join(format!("tyrian{ep}.lvl")), b"").expect("write");
    }
    for dir in [data_dir(), full, tmp("none")] {
        let mut d = Diff::new();
        with_data_dir(&mut d, &dir);
        d.game.episodes.episode_avail = [true, false, true, false, true];
        d.store_to_c();
        // SAFETY: plain C call.
        unsafe { b::JE_scanForEpisodes() };
        e::je_scan_for_episodes(&mut d.game);
        check_states(&d);
    }
}

proptest! {
    #[test]
    fn je_find_next_episode_matches(ep in 0u8..8, avail: [bool; 5], repeated: bool) {
        prop_assume!(avail.iter().any(|&a| a) || (1..=5).contains(&ep));
        let mut d = Diff::new();
        d.game.episodes.episode_num = ep;
        d.game.episodes.episode_avail = avail;
        d.game.config.game_has_repeated = repeated;
        d.store_to_c();
        // SAFETY: plain C call.
        let c = unsafe { b::JE_findNextEpisode() };
        prop_assert_eq!(c, e::je_find_next_episode(&mut d.game));
        prop_assert_eq!(diff::load::<State>(), d.game.episodes.clone());
        prop_assert_eq!(
            diff::load::<rustyrian_core::config::State>(),
            d.game.config.clone()
        );
    }
}

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

fn name(i: usize) -> [u8; 31] {
    std::array::from_fn(|j| (i * 31 + j) as u8)
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let mut s = State::default();
    for (i, p) in s.weapon_port.iter_mut().enumerate() {
        p.name = name(i);
        p.opnum = i as u8;
        p.op = [[i as u16; 11], [!(i as u16); 11]];
        p.cost = 1000 + i as u16;
        p.itemgraphic = 7;
        p.poweruse = 3;
    }
    for (i, w) in s.weapons.iter_mut().enumerate() {
        w.drain = i as u16;
        w.shotrepeat = 1;
        w.multi = 2;
        w.weapani = 3;
        w.max = 4;
        w.tx = 5;
        w.ty = 6;
        w.aim = 7;
        w.attack = [i as u8; 8];
        w.del = [8; 8];
        w.sx = [-1; 8];
        w.sy = [-2; 8];
        w.bx = [-3; 8];
        w.by = [-4; 8];
        w.sg = [i as u16 ^ 0xaaaa; 8];
        w.acceleration = -5;
        w.accelerationx = 6;
        w.circlesize = 9;
        w.sound = 10;
        w.trail = 11;
        w.shipblastfilter = 12;
    }
    for (i, p) in s.power_sys.iter_mut().enumerate() {
        p.name = name(i + 1);
        p.itemgraphic = 1;
        p.power = 2;
        p.speed = -3;
        p.cost = 4;
    }
    for (i, x) in s.ships.iter_mut().enumerate() {
        x.name = name(i + 2);
        x.shipgraphic = 1;
        x.itemgraphic = 2;
        x.ani = 3;
        x.spd = -4;
        x.dmg = 5;
        x.cost = 6;
        x.bigshipgraphic = 7;
    }
    for (i, o) in s.options.iter_mut().enumerate() {
        o.name = name(i + 3);
        o.pwr = 1;
        o.itemgraphic = 2;
        o.cost = 3;
        o.tr = 4;
        o.option = 5;
        o.opspd = -6;
        o.ani = 7;
        o.gr = [i as u16; 20];
        o.wport = 8;
        o.wpnum = 9;
        o.ammo = 10;
        o.stop = true;
        o.icongr = 11;
    }
    for (i, x) in s.shields.iter_mut().enumerate() {
        x.name = name(i + 4);
        x.tpwr = 1;
        x.mpwr = 2;
        x.itemgraphic = 3;
        x.cost = 4;
    }
    for (i, x) in s.special.iter_mut().enumerate() {
        x.name = name(i + 5);
        x.itemgraphic = 1;
        x.pwr = 2;
        x.stype = 3;
        x.wpn = 4;
    }
    for (i, e) in s.enemy_dat.iter_mut().enumerate() {
        e.ani = 1;
        e.tur = [2, 3, 4];
        e.freq = [5, 6, 7];
        e.xmove = -1;
        e.ymove = -2;
        e.xaccel = -3;
        e.yaccel = -4;
        e.xcaccel = -5;
        e.ycaccel = -6;
        e.startx = -300;
        e.starty = 300;
        e.startxc = -7;
        e.startyc = 8;
        e.armor = 9;
        e.esize = 10;
        e.egraphic = [i as u16; 20];
        e.explosiontype = 11;
        e.animate = 2;
        e.shapebank = 12;
        e.xrev = -13;
        e.yrev = 14;
        e.dgr = 15;
        e.dlevel = -16;
        e.dani = 17;
        e.elaunchfreq = 18;
        e.elaunchtype = 19;
        e.value = -20;
        e.eenemydie = 21;
    }
    s.initial_episode_num = 2;
    s.episode_num = 3;
    s.episode_avail = [true, false, true, true, false];
    s.episode_filename[..10].copy_from_slice(b"tyrian.hdt");
    s.cube_filename[..9].copy_from_slice(b"cubetxt1.");
    s.bonus_level = true;
    s.jump_back_to_episode1 = true;
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}
