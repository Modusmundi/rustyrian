//! Differential tests for `config.c` / `config.h`.

use proptest::prelude::*;
use rustyrian_core::config::{
    self as cfg, CRYPT_KEY, DEFAULT_KEY_SETTINGS, JeSaveFileType, SAVE_FILES_NUM, State,
};
use rustyrian_core::config_file::{ConfigOption, ConfigSection, ConfigString, ConfigValue};
use rustyrian_core::game::Halt;
use rustyrian_core::player::PlayerItems;
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;
use rustyrian_oracle::mirror::player::{player_items_from_c, player_items_to_c};
use std::path::{Path, PathBuf};

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data")
        .canonicalize()
        .expect("data/ (run `cargo xtask fetch-data`)")
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir()
        .join(format!("rustyrian-config-{}", std::process::id()))
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("scratch dir");
    d
}

fn bytes(p: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    p.as_os_str().as_bytes().to_vec()
}

fn set_user_dir(g: &mut rustyrian_core::Game, dir: &Path) {
    g.file.user_dir_path = Some(bytes(dir));
    g.file.user_dir_path_len = bytes(dir).len();
}

/// Stores `d.game` into C with user directory `c_dir`, then points the Rust
/// engine at `r_dir`, so each engine writes its own files.
fn split_user_dirs(d: &mut Diff, c_dir: &Path, r_dir: &Path) {
    set_user_dir(&mut d.game, c_dir);
    d.store_to_c();
    set_user_dir(&mut d.game, r_dir);
}

/// After a call: C state equals Rust state (the user directory differs by
/// design, so `file` is not compared).
fn check_after(d: &mut Diff) {
    let c = d.c_game_state();
    let r = d.game.snapshot();
    assert_eq!(c.config, r.config);
    assert_eq!(c.player, r.player);
    assert_eq!(c.episodes, r.episodes);
    assert_eq!(c.lvllib, r.lvllib);
    assert_eq!(c.mtrand, r.mtrand);
    assert_eq!(c.varz.temp, r.varz.temp);
    assert_eq!(d.c_calls(), d.rust_calls());
}

fn read_opt(p: PathBuf) -> Option<Vec<u8>> {
    std::fs::read(p).ok()
}

fn files_match(c_dir: &Path, r_dir: &Path) {
    for f in ["tyrian.sav", "tyrian.cfg", "opentyrian.cfg"] {
        assert_eq!(read_opt(c_dir.join(f)), read_opt(r_dir.join(f)), "{f}");
    }
}

fn any_items() -> impl Strategy<Value = PlayerItems> {
    any::<[u8; 12]>().prop_map(|v| {
        let mut p = PlayerItems::default();
        cfg::pitems_to_playeritems(&mut p, &v, None);
        p.weapon[0].power = v[3];
        p.weapon[1].power = v[7];
        p
    })
}

fn any_save() -> impl Strategy<Value = JeSaveFileType> {
    (
        any::<[u8; 12]>(),
        any::<[u8; 12]>(),
        any::<(u16, u16, i32, i32, u8, u8, u8, u8)>(),
        any::<(u8, u8, u8, bool, u8, i32, i32, u8)>(),
        (0usize..12, 0usize..31, any::<[u8; 2]>()),
        prop::sample::select(vec![&b"Completed"[..], b"LEVEL 7", b"", b"ABCDEFGHI"]),
    )
        .prop_map(|(items, last_items, a, c, (ln, hn, power), level)| {
            let mut s = JeSaveFileType {
                encode: a.0,
                level: a.1,
                items,
                score: a.2,
                score2: a.3,
                cubes: a.4,
                power,
                episode: a.5 % 7,
                last_items,
                difficulty: a.6,
                secret_hint: a.7,
                input1: c.0,
                input2: c.1,
                game_has_repeated: c.3,
                initial_difficulty: c.4,
                high_score1: c.5,
                high_score2: c.6,
                high_score_diff: c.7,
                ..JeSaveFileType::default()
            };
            s.level_name[..level.len()].copy_from_slice(level);
            for (i, x) in s.level_name.iter_mut().enumerate().take(ln.min(10)) {
                if *x == 0 && i > level.len() {
                    *x = b'x';
                }
            }
            s.name = std::array::from_fn(|j| {
                if j < 14 {
                    b'a' + (j as u8).wrapping_add(c.2) % 26
                } else {
                    0
                }
            });
            for (j, x) in s.high_score_name.iter_mut().enumerate().take(hn.min(29)) {
                *x = b'A' + j as u8;
            }
            s
        })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    #[test]
    fn playeritems_to_pitems_matches(items in any_items(), ep: u8) {
        let mut c_items = player_items_to_c(&items);
        let mut c = [0u8; 12];
        // SAFETY: valid arrays.
        unsafe { b::ot_s_config_playeritems_to_pitems(c.as_mut_ptr(), &raw mut c_items, ep) };
        let mut r = [0u8; 12];
        cfg::playeritems_to_pitems(&mut r, &items, ep);
        prop_assert_eq!(c, r);
    }

    #[test]
    fn pitems_to_playeritems_matches(start in any_items(), mut p: [u8; 12], ep: u8, with_ep: bool) {
        let mut c_items = player_items_to_c(&start);
        let (mut c_ep, mut r_ep) = (ep, ep);
        let c_ep_ptr = if with_ep { &raw mut c_ep } else { std::ptr::null_mut() };
        // SAFETY: valid pointers (or NULL).
        unsafe { b::ot_s_config_pitems_to_playeritems(&raw mut c_items, p.as_mut_ptr(), c_ep_ptr) };
        let mut r = start;
        cfg::pitems_to_playeritems(&mut r, &p, with_ep.then_some(&mut r_ep));
        prop_assert_eq!(player_items_from_c(&c_items), r);
        prop_assert_eq!(c_ep, r_ep);
    }

    #[test]
    fn je_init_processor_type_matches(processor in 0u8..8, speed in 0u8..7, start: [bool; 7], fast: u8) {
        let mut d = Diff::new();
        let c = &mut d.game.config;
        c.processor_type = processor;
        c.game_speed = speed;
        c.fast_play = fast;
        [c.wild, c.super_wild, c.smooth_scroll, c.explosion_transparent, c.filtration_avail, c.background2, c.display_score] = start;
        d.store_to_c();
        // SAFETY: plain C call.
        unsafe { b::JE_initProcessorType() };
        cfg::je_init_processor_type(&mut d.game);
        prop_assert_eq!(diff::load::<State>(), d.game.config.clone());
    }

    /// `saveSaves` writes byte-identical files; `loadSaves` reads them back
    /// into identical state (and equal to what was saved).
    #[test]
    fn save_saves_and_load_saves_round_trip(saves in prop::collection::vec(any_save(), SAVE_FILES_NUM), avail: [u8; 100], level: u16) {
        let (c_dir, r_dir) = (tmp("save-c"), tmp("save-r"));
        let mut d = Diff::new();
        d.game.config.save_files.copy_from_slice(&saves);
        d.game.config.editor_item_avail = avail;
        d.game.config.editor_level = level;
        split_user_dirs(&mut d, &c_dir, &r_dir);
        // SAFETY: plain C call.
        unsafe { b::saveSaves() };
        cfg::save_saves(&mut d.game);
        files_match(&c_dir, &r_dir);
        check_after(&mut d);
        let saved = d.game.config.clone();

        let mut d = Diff::new();
        split_user_dirs(&mut d, &c_dir, &r_dir);
        // SAFETY: plain C call.
        unsafe { b::loadSaves() };
        cfg::load_saves(&mut d.game);
        check_after(&mut d);
        prop_assert_eq!(d.game.config.editor_item_avail, saved.editor_item_avail);
        prop_assert_eq!(d.game.config.editor_level, saved.editor_level);
    }

    /// `loadSaves` on damaged files: any single corrupted byte (checksum
    /// failure, defaults drawn from `mt_rand`) and truncation.
    #[test]
    fn load_saves_damaged_files_match(at in 0usize..2502, x in 1u8..=255, len in 0usize..2600, seed: u64) {
        let (c_dir, r_dir) = (tmp("dmg-c"), tmp("dmg-r"));
        let mut d = Diff::new();
        split_user_dirs(&mut d, &c_dir, &r_dir);
        // SAFETY: plain C call.
        unsafe { b::saveSaves() };
        let mut data = std::fs::read(c_dir.join("tyrian.sav")).expect("saved");
        data[at] ^= x;
        data.truncate(len);
        for dir in [&c_dir, &r_dir] {
            std::fs::write(dir.join("tyrian.sav"), &data).expect("write");
        }
        let _ = d.c_calls();
        rustyrian_core::mtrand::mt_srand(&mut d.game, seed);
        d.game.config.save_files[3] = JeSaveFileType { level: 9, ..Default::default() };
        split_user_dirs(&mut d, &c_dir, &r_dir);
        // SAFETY: plain C call.
        unsafe { b::loadSaves() };
        cfg::load_saves(&mut d.game);
        check_after(&mut d);
    }
}

/// The freeware data ships no save; a missing file gives the defaults.
#[test]
fn load_saves_missing_file_matches() {
    let (c_dir, r_dir) = (tmp("missing-c"), tmp("missing-r"));
    let mut d = Diff::new();
    split_user_dirs(&mut d, &c_dir, &r_dir);
    // SAFETY: plain C call.
    unsafe { b::loadSaves() };
    cfg::load_saves(&mut d.game);
    check_after(&mut d);
    assert_eq!(d.game.config.editor_level, 800);
}

/// Unwritable user directory: every open fails and is logged identically.
#[test]
fn save_files_unwritable_dir_matches() {
    let base = tmp("unwritable");
    std::fs::write(base.join("file"), b"").expect("write");
    let dir = base.join("file").join("sub");
    let mut d = Diff::new();
    split_user_dirs(&mut d, &dir, &dir);
    // SAFETY: plain C calls.
    unsafe {
        b::saveSaves();
        b::saveConfiguration();
    }
    cfg::save_saves(&mut d.game);
    cfg::save_configuration(&mut d.game);
    let calls = d.c_calls();
    assert_eq!(calls, d.rust_calls());
    assert!(format!("{calls:?}").contains("Failed to open"));
}

/// Each of the four save checksums failing on its own.
#[test]
fn load_saves_each_checksum_failure_matches() {
    for k in 0..4 {
        let (c_dir, r_dir) = (tmp("sum-c"), tmp("sum-r"));
        let mut d = Diff::new();
        d.game.config.save_files[0].level = 3;
        split_user_dirs(&mut d, &c_dir, &r_dir);
        // SAFETY: plain C call.
        unsafe { b::saveSaves() };
        let _ = d.c_calls();
        let mut data = std::fs::read(c_dir.join("tyrian.sav")).expect("saved");
        data[cfg::SAVE_FILE_SIZE + k] ^= 0x5A;
        for dir in [&c_dir, &r_dir] {
            std::fs::write(dir.join("tyrian.sav"), &data).expect("write");
        }
        split_user_dirs(&mut d, &c_dir, &r_dir);
        // SAFETY: plain C call.
        unsafe { b::loadSaves() };
        cfg::load_saves(&mut d.game);
        check_after(&mut d);
        assert_eq!(d.game.config.editor_level, 800, "checksum {k}");
    }
}

/// Writes failing part-way (file size limit): both engines log the same
/// write errors, for all three files, and leave the same truncated files.
#[test]
fn save_files_write_errors_match() {
    let (c_dir, r_dir) = (tmp("efbig-c"), tmp("efbig-r"));
    let mut d = Diff::new();
    split_user_dirs(&mut d, &c_dir, &r_dir);
    // SAFETY: this test runs in its own process (nextest); ignoring SIGXFSZ
    // turns over-limit writes into EFBIG errors.
    unsafe {
        libc::signal(libc::SIGXFSZ, libc::SIG_IGN);
        let limit = libc::rlimit {
            rlim_cur: 10,
            rlim_max: libc::RLIM_INFINITY,
        };
        assert_eq!(libc::setrlimit(libc::RLIMIT_FSIZE, &limit), 0);
    }
    // SAFETY: plain C calls.
    unsafe {
        b::saveSaves();
        b::saveConfiguration();
    }
    cfg::save_saves(&mut d.game);
    cfg::save_configuration(&mut d.game);
    let calls = d.c_calls();
    assert_eq!(calls, d.rust_calls());
    let text = format!("{calls:?}");
    for f in ["tyrian.sav", "tyrian.cfg", "opentyrian.cfg"] {
        assert!(
            text.contains(&format!("Failed to write to '{f}'")),
            "{f}: {text}"
        );
    }
    files_match(&c_dir, &r_dir);
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 32, ..ProptestConfig::default() })]

    /// `saveConfiguration` writes byte-identical `tyrian.cfg` and
    /// `opentyrian.cfg` from any settings.
    #[test]
    fn save_configuration_matches(
        bytes12: [u8; 12],
        joy: [u8; 4],
        dos: [u8; 8],
        volumes: (u16, u16),
        keys in prop::array::uniform8(0u32..300),
        scaler in 0u32..9,
        mode in 0u32..4,
        fullscreen in -2i32..3,
        existing: bool,
    ) {
        let (c_dir, r_dir) = (tmp("cfg-c"), tmp("cfg-r"));
        let mut d = Diff::new();
        let c = &mut d.game.config;
        c.background2 = bytes12[0] & 1 != 0;
        c.game_speed = bytes12[1];
        c.input_device_ = bytes12[2];
        c.j_configure = bytes12[3];
        c.version_num = bytes12[4];
        c.processor_type = bytes12[5];
        c.midi_port = bytes12[6];
        c.sound_effects = bytes12[7];
        c.gamma_correction = bytes12[8];
        c.difficulty_level = bytes12[9] as i8;
        c.input_device1 = bytes12[10];
        c.input_device2 = bytes12[11];
        c.joy_button_assign = joy;
        c.dos_key_settings = dos;
        c.key_settings = keys;
        if existing {
            let s = rustyrian_core::config_file::config_add_section(&mut c.opentyrian_config, b"video", None);
            rustyrian_core::config_file::config_set_string_option(s, b"other", Some(b"kept"));
            rustyrian_core::config_file::config_add_section(&mut c.opentyrian_config, b"extra", Some(b"x"));
        }
        (d.game.nortsong.tyr_music_volume, d.game.nortsong.fx_volume) = volumes;
        d.game.video_scale.scaler = scaler;
        d.game.video.scaling_mode = rustyrian_core::video::ScalingMode::from_c(mode).expect("mode");
        d.game.video.fullscreen_display = fullscreen;
        split_user_dirs(&mut d, &c_dir, &r_dir);
        // SAFETY: plain C call.
        unsafe { b::saveConfiguration() };
        cfg::save_configuration(&mut d.game);
        files_match(&c_dir, &r_dir);
        check_after(&mut d);
    }

    /// `JE_saveGame` then `JE_loadGame` of the same slot (loading reads
    /// episode data, so the freeware data directory is used).
    #[test]
    fn je_save_game_and_je_load_game_match(
        slot in 1u8..=22,
        two_player: bool,
        flags in (any::<bool>(), any::<bool>(), prop_oneof![Just(0u8), 0u8..10]),
        items in prop::array::uniform2(any_items()),
        last in any_items(),
        cash: (u64, u64),
        ep in 1u8..=5,
        level in prop::sample::select(vec![&b"Completed"[..], b"LEVEL 7"]),
        misc: (u8, i8, i8, u8, [u8; 2], u16),
        sidekick_level: u8,
    ) {
        let (c_dir, r_dir) = (tmp("game-c"), tmp("game-r"));
        let mut d = Diff::new();
        d.game.file.custom_data_dir_path = Some(bytes(&data_dir()));
        let c = &mut d.game.config;
        c.two_player_mode = two_player;
        (c.super_tyrian, c.one_player_action, c.super_arcade_mode) = flags;
        c.save_level = misc.0;
        c.initial_difficulty = misc.1;
        c.difficulty_level = misc.2;
        c.secret_hint = misc.3;
        c.input_device = misc.4;
        c.last_cube_max = misc.5;
        c.game_has_repeated = flags.0;
        c.last_level_name = [0; 11];
        c.last_level_name[..level.len()].copy_from_slice(level);
        d.game.episodes.episode_num = ep - 1;
        d.game.episodes.initial_episode_num = ep;
        for (p, it) in d.game.player.player.iter_mut().zip(items) {
            p.items = it;
            p.last_items = last;
            p.lives = Some(0);
        }
        d.game.player.player[1].items.sidekick_level = sidekick_level;
        (d.game.player.player[0].cash, d.game.player.player[1].cash) = cash;
        split_user_dirs(&mut d, &c_dir, &r_dir);
        let name = b"Saved game nameXYZ\0";
        // SAFETY: `name` is NUL-terminated and longer than 14 bytes.
        unsafe { b::JE_saveGame(slot, name.as_ptr().cast()) };
        cfg::je_save_game(&mut d.game, slot, name);
        files_match(&c_dir, &r_dir);
        check_after(&mut d);

        // SAFETY: JE_loadGame may exit (missing episode), which the guard catches.
        let c = unsafe { rustyrian_oracle::guard_call(|| b::JE_loadGame(slot)) };
        let r = cfg::je_load_game(&mut d.game, slot);
        prop_assert_eq!(c.is_some(), r.is_err());
        if let Some(code) = c {
            prop_assert_eq!(r, Err(Halt { code }));
        }
        check_after(&mut d);
    }
}

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

fn save(i: usize) -> JeSaveFileType {
    let k = i as u8;
    JeSaveFileType {
        encode: 1000 + i as u16,
        level: 7,
        items: [k; 12],
        score: -(i as i32) * 1000,
        score2: i as i32,
        level_name: std::array::from_fn(|j| b'A' + (j as u8 + k) % 26),
        name: std::array::from_fn(|j| b'a' + (j as u8 + k) % 26),
        cubes: 3,
        power: [k, k + 1],
        episode: 2,
        last_items: [k + 1; 12],
        difficulty: 4,
        secret_hint: 5,
        input1: 1,
        input2: 2,
        game_has_repeated: i % 2 == 1,
        initial_difficulty: 6,
        high_score1: 12345,
        high_score2: -1,
        high_score_name: std::array::from_fn(|j| j as u8),
        high_score_diff: 9,
    }
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let mut s = State {
        smoothies: [true, false, true, false, true, false, true, false, true],
        star_show_vga_special_code: 1,
        last_cube_max: 2,
        cube_max: 3,
        cube_list: [4, 5, 6, 7],
        game_has_repeated: true,
        difficulty_level: -1,
        old_difficulty_level: 2,
        initial_difficulty: 3,
        power: 100,
        last_power: 200,
        power_add: 300,
        shield_wait: 4,
        shield_t: 5,
        shot_repeat: [6; 11],
        shot_multi_pos: [7; 11],
        port_config_change: true,
        port_config_done: true,
        last_level_name: *b"LEVEL ONE\0\0",
        level_name: *b"Savara V\0\0x",
        main_level: 8,
        next_level: 9,
        save_level: 10,
        key_settings: DEFAULT_KEY_SETTINGS.map(|k| k + 1),
        level_filter: -1,
        level_filter_new: -2,
        level_brightness: -3,
        level_brightness_chg: 4,
        filtration_avail: true,
        filter_active: true,
        filter_fade: true,
        filter_fade_start: true,
        game_just_loaded: true,
        galaga_mode: true,
        extra_game: true,
        two_player_mode: true,
        two_player_linked: true,
        one_player_action: true,
        super_tyrian: true,
        trent_win: true,
        super_arcade_mode: 11,
        super_arcade_power_up: 12,
        link_gun_direc: 1.25,
        input_device: [3, 0],
        secret_hint: 13,
        background3over: 14,
        background2over: 15,
        gamma_correction: 16,
        super_pause: true,
        explosion_transparent: true,
        you_are_cheating: true,
        display_score: true,
        background2: true,
        smooth_scroll: true,
        wild: true,
        super_wild: true,
        star_active: true,
        top_enemy_over: true,
        sky_enemy_over_all: true,
        background2not_transparent: true,
        fast_play: 17,
        game_speed: 18,
        processor_type: 3,
        editor_level: 800,
        input_device_: 19,
        j_configure: 20,
        midi_port: 21,
        sound_effects: 22,
        version_num: 2,
        joy_button_assign: [1, 4, 5, 5],
        input_device1: 23,
        input_device2: 24,
        dos_key_settings: [25, 26, 27, 28, 29, 30, 31, 32],
        ..State::default()
    };
    for (i, f) in s.save_files.iter_mut().enumerate() {
        *f = save(i);
    }
    for (i, a) in s.editor_item_avail.iter_mut().enumerate() {
        *a = (i % 3) as u8;
    }
    assert_eq!(s.save_files.len(), SAVE_FILES_NUM);
    let cs = |t: &str| ConfigString(Some(t.as_bytes().to_vec()));
    s.opentyrian_config.sections = vec![
        ConfigSection {
            r#type: cs("video"),
            name: ConfigString(None),
            options: vec![
                ConfigOption {
                    key: cs("scaler"),
                    v: ConfigValue::Value(cs("a value longer than sixteen bytes")),
                },
                ConfigOption {
                    key: cs("list"),
                    v: ConfigValue::Values(vec![cs("x"), cs("y")]),
                },
            ],
        },
        ConfigSection {
            r#type: cs("keyboard"),
            name: cs("player 1"),
            options: Vec::new(),
        },
    ];
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}

#[test]
fn default_key_settings_match_c() {
    // SAFETY: reading a const C table.
    assert_eq!(DEFAULT_KEY_SETTINGS, unsafe { b::defaultKeySettings });
    assert_eq!(CRYPT_KEY.len(), 10);
}
