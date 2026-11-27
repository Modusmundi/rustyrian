// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `config.c` / `config.h`.

use crate::config_file::Config;
use serde::{Deserialize, Serialize};

pub const SAVE_FILES_NUM: usize = 11 * 2;
pub const SAVE_FILES_SIZE: usize = 109 * SAVE_FILES_NUM;
pub const SAVE_FILE_SIZE: usize = SAVE_FILES_SIZE + 100;

pub const DIFFICULTY_WIMP: i8 = 0;
pub const DIFFICULTY_EASY: i8 = 1;
pub const DIFFICULTY_NORMAL: i8 = 2;
pub const DIFFICULTY_HARD: i8 = 3;
pub const DIFFICULTY_IMPOSSIBLE: i8 = 4;
pub const DIFFICULTY_INSANITY: i8 = 5;
pub const DIFFICULTY_SUICIDE: i8 = 6;
pub const DIFFICULTY_MANIACAL: i8 = 7;
/// Aka Lord of the Game.
pub const DIFFICULTY_ZINGLON: i8 = 8;
pub const DIFFICULTY_NORTANEOUS: i8 = 9;
pub const DIFFICULTY_10: i8 = 10;

// NOTE: Do not reorder. This ordering corresponds to the keyboard
//       configuration menu and to the bits stored in demo files.
pub const KEY_SETTING_UP: usize = 0;
pub const KEY_SETTING_DOWN: usize = 1;
pub const KEY_SETTING_LEFT: usize = 2;
pub const KEY_SETTING_RIGHT: usize = 3;
pub const KEY_SETTING_FIRE: usize = 4;
pub const KEY_SETTING_CHANGE_FIRE: usize = 5;
pub const KEY_SETTING_LEFT_SIDEKICK: usize = 6;
pub const KEY_SETTING_RIGHT_SIDEKICK: usize = 7;

pub const SHOT_FRONT: usize = 0;
pub const SHOT_REAR: usize = 1;
pub const SHOT_LEFT_SIDEKICK: usize = 2;
pub const SHOT_RIGHT_SIDEKICK: usize = 3;
pub const SHOT_MISC: usize = 4;
pub const SHOT_P2_CHARGE: usize = 5;
pub const SHOT_P1_SUPERBOMB: usize = 6;
pub const SHOT_P2_SUPERBOMB: usize = 7;
pub const SHOT_SPECIAL: usize = 8;
pub const SHOT_NORTSPARKS: usize = 9;
pub const SHOT_SPECIAL2: usize = 10;

/// `KeySettings`: `SDL_Scancode[8]`.
pub type KeySettings = [u32; 8];
/// `JE_PItemsType` `[1..12]`.
pub type JePItemsType = [u8; 12];
/// `JE_EditorItemAvailType` `[1..100]`.
pub type JeEditorItemAvailType = [u8; 100];

/// `JE_SaveFileType`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JeSaveFileType {
    pub encode: u16,
    pub level: u16,
    pub items: JePItemsType,
    pub score: i32,
    pub score2: i32,
    /// `string [9]` plus one byte to match `lastLevelName`.
    pub level_name: [u8; 11],
    /// `[1..14]` plus NUL.
    pub name: [u8; 15],
    pub cubes: u8,
    /// `[1..2]`.
    pub power: [u8; 2],
    pub episode: u8,
    pub last_items: JePItemsType,
    pub difficulty: u8,
    pub secret_hint: u8,
    pub input1: u8,
    pub input2: u8,
    /// See if you went from one episode to another.
    pub game_has_repeated: bool,
    pub initial_difficulty: u8,
    // High scores: each episode has both sets of 1&2 player selections, with
    // 3 in each.
    pub high_score1: i32,
    /// Unused.
    pub high_score2: i32,
    /// `string [29]`.
    pub high_score_name: [u8; 30],
    pub high_score_diff: u8,
}

/// `JE_SaveFilesType` `[1..savefilesnum]`.
pub type JeSaveFilesType = [JeSaveFileType; SAVE_FILES_NUM];

/// SDL 2 scancode values used by `defaultKeySettings`.
const SDL_SCANCODE_RETURN: u32 = 40;
const SDL_SCANCODE_SPACE: u32 = 44;
const SDL_SCANCODE_RIGHT: u32 = 79;
const SDL_SCANCODE_LEFT: u32 = 80;
const SDL_SCANCODE_DOWN: u32 = 81;
const SDL_SCANCODE_UP: u32 = 82;
const SDL_SCANCODE_LCTRL: u32 = 224;
const SDL_SCANCODE_LALT: u32 = 226;

/// `defaultKeySettings`.
pub const DEFAULT_KEY_SETTINGS: KeySettings = [
    SDL_SCANCODE_UP,
    SDL_SCANCODE_DOWN,
    SDL_SCANCODE_LEFT,
    SDL_SCANCODE_RIGHT,
    SDL_SCANCODE_SPACE,
    SDL_SCANCODE_RETURN,
    SDL_SCANCODE_LCTRL,
    SDL_SCANCODE_LALT,
];

/// `cryptKey` `[1..10]`.
pub const CRYPT_KEY: [u8; 10] = [15, 50, 89, 240, 147, 34, 86, 9, 32, 208];

/// `keySettingNames`.
pub const KEY_SETTING_NAMES: [&str; 8] = [
    "up",
    "down",
    "left",
    "right",
    "fire",
    "change fire",
    "left sidekick",
    "right sidekick",
];

const fn cstr<const N: usize>(s: &str) -> [u8; N] {
    let b = s.as_bytes();
    assert!(b.len() < N);
    let mut out = [0u8; N];
    let mut i = 0;
    while i < b.len() {
        out[i] = b[i];
        i += 1;
    }
    out
}

/// `defaultHighScoreNames` `[1..34] of string [22]`.
pub const DEFAULT_HIGH_SCORE_NAMES: [[u8; 23]; 34] = [
    // 1P, TYR (13)
    cstr("The Prime Chair"),
    cstr("Transon Lohk"),
    cstr("Javi Onukala"),
    cstr("Mantori"),
    cstr("Nortaneous"),
    cstr("Dougan"),
    cstr("Reid"),
    cstr("General Zinglon"),
    cstr("Late Gyges Phildren"),
    cstr("Vykromod"),
    cstr("Beppo"),
    cstr("Borogar"),
    cstr("ShipMaster Carlos"),
    // OTHER (5)
    cstr("Jill"),
    cstr("Darcy"),
    cstr("Jake Stone"),
    cstr("Malvineous Havershim"),
    cstr("Marta Louise Velasquez"),
    // JAZZ (3)
    cstr("Jazz Jackrabbit"),
    cstr("Eva Earlong"),
    cstr("Devan Shell"),
    // OMF (11)
    cstr("Crystal Devroe"),
    cstr("Steffan Tommas"),
    cstr("Milano Angston"),
    cstr("Christian"),
    cstr("Shirro"),
    cstr("Jean-Paul"),
    cstr("Ibrahim Hothe"),
    cstr("Angel"),
    cstr("Cossette Akira"),
    cstr("Raven"),
    cstr("Hans Kreissack"),
    // DARE (2)
    cstr("Tyler"),
    cstr("Rennis the Rat Guard"),
];

/// `defaultTeamNames` `[1..22] of string [24]`.
pub const DEFAULT_TEAM_NAMES: [[u8; 25]; 22] = [
    cstr("Jackrabbits"),
    cstr("Team Tyrian"),
    cstr("The Elam Brothers"),
    cstr("Dare to Dream Team"),
    cstr("Pinball Freaks"),
    cstr("Extreme Pinball Freaks"),
    cstr("Team Vykromod"),
    cstr("Epic All-Stars"),
    cstr("Hans Keissack's WARriors"),
    cstr("Team Overkill"),
    cstr("Pied Pipers"),
    cstr("Gencore Growlers"),
    cstr("Microsol Masters"),
    cstr("Beta Warriors"),
    cstr("Team Loco"),
    cstr("The Shellians"),
    cstr("Jungle Jills"),
    cstr("Murderous Malvineous"),
    cstr("The Traffic Department"),
    cstr("Clan Mikal"),
    cstr("Clan Patrok"),
    cstr("Carlos' Crawlers"),
];

/// `initialEditorItemAvail` (FKA `initialItemAvail`).
#[rustfmt::skip]
pub const INITIAL_EDITOR_ITEM_AVAIL: JeEditorItemAvailType = [
    1,1,1,0,0,1,1,0,1,1,1,1,1,0,1,0,1,1,1,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0, // Front/Rear Weapons 1-38
    0,0,0,0,0,0,0,0,0,0,1,                                                           // Fill
    1,0,0,0,0,1,0,0,0,1,1,0,1,0,0,0,0,0,                                             // Sidekicks          51-68
    0,0,0,0,0,0,0,0,0,0,0,                                                           // Fill
    1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,                                                   // Special Weapons    81-93
    0,0,0,0,0,                                                                       // Fill
];

/// `defaultJoyButtonAssign` (FKA `Joystick.defaultJoyButtonAssign`).
pub const DEFAULT_JOY_BUTTON_ASSIGN: [u8; 4] = [1, 4, 5, 5];

/// Mutable globals of this module (rule R3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// `[1..9]`.
    pub smoothies: [bool; 9],
    pub star_show_vga_special_code: u8,
    // CubeData
    pub last_cube_max: u16,
    pub cube_max: u16,
    /// `[1..4]`.
    pub cube_list: [u16; 4],
    /// Can only get a high score on the first play-through.
    pub game_has_repeated: bool,
    pub difficulty_level: i8,
    pub old_difficulty_level: i8,
    /// Can only get a high score on the initial episode.
    pub initial_difficulty: i8,
    pub power: u32,
    pub last_power: u32,
    pub power_add: u32,
    pub shield_wait: u8,
    pub shield_t: u8,
    pub shot_repeat: [u8; 11],
    pub shot_multi_pos: [u8; 11],
    pub port_config_change: bool,
    pub port_config_done: bool,
    /// `string [10]`.
    pub last_level_name: [u8; 11],
    pub level_name: [u8; 11],
    pub main_level: u8,
    pub next_level: u8,
    pub save_level: u8,
    pub key_settings: KeySettings,
    pub level_filter: i8,
    pub level_filter_new: i8,
    pub level_brightness: i8,
    pub level_brightness_chg: i8,
    pub filtration_avail: bool,
    pub filter_active: bool,
    pub filter_fade: bool,
    pub filter_fade_start: bool,
    pub game_just_loaded: bool,
    pub galaga_mode: bool,
    pub extra_game: bool,
    pub two_player_mode: bool,
    pub two_player_linked: bool,
    pub one_player_action: bool,
    pub super_tyrian: bool,
    pub trent_win: bool,
    pub super_arcade_mode: u8,
    pub super_arcade_power_up: u8,
    pub link_gun_direc: f32,
    /// 0: any, 1: keyboard, 2: mouse, 3+: joystick.
    pub input_device: [u8; 2],
    pub secret_hint: u8,
    pub background3over: u8,
    pub background2over: u8,
    pub gamma_correction: u8,
    pub super_pause: bool,
    pub explosion_transparent: bool,
    pub you_are_cheating: bool,
    pub display_score: bool,
    pub background2: bool,
    pub smooth_scroll: bool,
    pub wild: bool,
    pub super_wild: bool,
    pub star_active: bool,
    pub top_enemy_over: bool,
    pub sky_enemy_over_all: bool,
    pub background2not_transparent: bool,
    pub fast_play: u8,
    pub game_speed: u8,
    /// 1 = 386, 2 = 486, 3 = Pentium Hyper.
    pub processor_type: u8,
    pub save_files: JeSaveFilesType,
    #[serde(with = "crate::serde_big")]
    pub editor_item_avail: JeEditorItemAvailType,
    /// Initial value 800.
    pub editor_level: u16,
    pub opentyrian_config: Config,
    // Fields of TYRIAN.CFG that are preserved for compatibility (file statics).
    /// FKA `inputDevice`.
    pub input_device_: u8,
    /// FKA `NortSong.jConfigure`.
    pub j_configure: u8,
    /// FKA `NortSong.midiPort`.
    pub midi_port: u8,
    /// FKA `NortSong.soundEffects`.
    pub sound_effects: u8,
    /// SW 1.0 and SW/Reg 1.1 = 0 or 1; EA 1.2 = 2.
    pub version_num: u8,
    /// FKA `Joystick.joyButtonAssign`.
    pub joy_button_assign: [u8; 4],
    pub input_device1: u8,
    pub input_device2: u8,
    /// FKA `keySettings`.
    pub dos_key_settings: [u8; 8],
}

impl Default for State {
    fn default() -> Self {
        State {
            smoothies: [false; 9],
            star_show_vga_special_code: 0,
            last_cube_max: 0,
            cube_max: 0,
            cube_list: [0; 4],
            game_has_repeated: false,
            difficulty_level: 0,
            old_difficulty_level: 0,
            initial_difficulty: 0,
            power: 0,
            last_power: 0,
            power_add: 0,
            shield_wait: 0,
            shield_t: 0,
            shot_repeat: [0; 11],
            shot_multi_pos: [0; 11],
            port_config_change: false,
            port_config_done: false,
            last_level_name: [0; 11],
            level_name: [0; 11],
            main_level: 0,
            next_level: 0,
            save_level: 0,
            key_settings: [0; 8],
            level_filter: 0,
            level_filter_new: 0,
            level_brightness: 0,
            level_brightness_chg: 0,
            filtration_avail: false,
            filter_active: false,
            filter_fade: false,
            filter_fade_start: false,
            game_just_loaded: false,
            galaga_mode: false,
            extra_game: false,
            two_player_mode: false,
            two_player_linked: false,
            one_player_action: false,
            super_tyrian: false,
            trent_win: false,
            super_arcade_mode: 0,
            super_arcade_power_up: 0,
            link_gun_direc: 0.0,
            input_device: [1, 2],
            secret_hint: 0,
            background3over: 0,
            background2over: 0,
            gamma_correction: 0,
            super_pause: false,
            explosion_transparent: false,
            you_are_cheating: false,
            display_score: false,
            background2: false,
            smooth_scroll: false,
            wild: false,
            super_wild: false,
            star_active: false,
            top_enemy_over: false,
            sky_enemy_over_all: false,
            background2not_transparent: false,
            fast_play: 0,
            game_speed: 0,
            processor_type: 0,
            save_files: [JeSaveFileType::default(); SAVE_FILES_NUM],
            editor_item_avail: [0; 100],
            editor_level: 0,
            opentyrian_config: Config::default(),
            input_device_: 0,
            j_configure: 0,
            midi_port: 0,
            sound_effects: 0,
            version_num: 0,
            joy_button_assign: [0; 4],
            input_device1: 0,
            input_device2: 0,
            dos_key_settings: [0; 8],
        }
    }
}
