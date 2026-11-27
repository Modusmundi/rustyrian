//! Differential tests for `config.c` / `config.h`.

use rustyrian_core::config::{
    CRYPT_KEY, DEFAULT_KEY_SETTINGS, JeSaveFileType, SAVE_FILES_NUM, State,
};
use rustyrian_core::config_file::{ConfigOption, ConfigSection, ConfigString, ConfigValue};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;

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
