// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `config.c` / `config.h`.

use crate::Game;
use crate::config_file::{self as cf, Config};
use crate::episodes::je_init_episode;
use crate::file;
use crate::game::Flow;
use crate::memreader::{self as mr, MemReader};
use crate::memwriter::{self as mw, MemWriter};
use crate::mtrand::mt_rand;
use crate::player::{FRONT_WEAPON, LEFT_SIDEKICK, PlayerItems, REAR_WEAPON, RIGHT_SIDEKICK};
use crate::varz::{SA_ARCADE, SA_NONE, SA_NORTSHIPZ, SA_SUPERTYRIAN};
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

/// `defaultDosKeySettings` (FKA `defaultKeySettings`).
pub const DEFAULT_DOS_KEY_SETTINGS: [u8; 8] = [72, 80, 75, 77, 57, 28, 29, 56];

const OPENTYRIAN_CONFIG_FILENAME: &str = "opentyrian.cfg";
const TYRIAN_CONFIG_FILENAME: &str = "tyrian.cfg";
const TYRIAN_SAVE_FILENAME: &str = "tyrian.sav";

/// `SDL_LOG_CATEGORY_APPLICATION` (upstream's `logWarn`/`logError`).
const LOG_CATEGORY: i32 = 0;

/// `strlen` of a C string held in `b` (its length if it has no NUL).
fn strlen(b: &[u8]) -> usize {
    b.iter().position(|&c| c == 0).unwrap_or(b.len())
}

/// `strcmp(a, s) == 0` for the C string in `a`.
fn c_str_eq(a: &[u8], s: &[u8]) -> bool {
    &a[..strlen(a)] == s
}

/// The scaler `loadOpenTyrianConfig` selects before reading the file.
const DEFAULT_SCALER_NAME: &[u8] = b"Scale2x";

/// `SDL_SCANCODE_UNKNOWN`.
const SDL_SCANCODE_UNKNOWN: i32 = 0;

#[doc(alias = "loadOpenTyrianConfig")]
pub fn load_open_tyrian_config(g: &mut Game) {
    // defaults
    g.video.fullscreen_display = -1;
    crate::video_scale::set_scaler_by_name(g, DEFAULT_SCALER_NAME);
    g.config.key_settings = DEFAULT_KEY_SETTINGS;

    let mut file = file::user_file_open(g, OPENTYRIAN_CONFIG_FILENAME, "r");
    if file.error {
        let message = format!(
            "Failed to open '{}': {}",
            OPENTYRIAN_CONFIG_FILENAME,
            file::file_get_error(&file)
        );
        g.platform.log_warn(LOG_CATEGORY, &message);

        return;
    }

    let stream = file.f.as_mut().expect("open file has a stream");
    let success = cf::config_parse(&mut g.config.opentyrian_config, stream);
    file.error |= stream.ferror();

    if file.error {
        let message = format!(
            "Failed to read from '{}': {}",
            OPENTYRIAN_CONFIG_FILENAME,
            file::file_get_error(&file)
        );
        g.platform.log_error(LOG_CATEGORY, &message);
    } else if !success {
        let message = format!("Failed to parse '{OPENTYRIAN_CONFIG_FILENAME}'.");
        g.platform.log_error(LOG_CATEGORY, &message);
    }

    file::file_close(&mut file);

    if !success {
        return;
    }

    // The option values are copied out of the section before calls that
    // need the whole game (C keeps pointers into the section).
    let fullscreen_display = &mut g.video.fullscreen_display;
    let video = cf::config_find_section(&mut g.config.opentyrian_config, b"video", None).map(|s| {
        cf::config_get_int_option(s, b"fullscreen", fullscreen_display);
        let mut scaler = None;
        let found_scaler = cf::config_get_string_option(s, b"scaler", &mut scaler);
        let mut mode = None;
        let found_mode = cf::config_get_string_option(s, b"scaling_mode", &mut mode);
        (
            found_scaler.then(|| scaler.unwrap_or(b"").to_vec()),
            found_mode.then(|| mode.unwrap_or(b"").to_vec()),
        )
    });
    if let Some((scaler, mode)) = video {
        if let Some(scaler) = scaler {
            crate::video_scale::set_scaler_by_name(g, &scaler);
        }
        if let Some(mode) = mode {
            crate::video::set_scaling_mode_by_name(g, &mode);
        }
    }

    let keys: Option<Vec<Option<Vec<u8>>>> =
        cf::config_find_section(&mut g.config.opentyrian_config, b"keyboard", None).map(|s| {
            KEY_SETTING_NAMES
                .iter()
                .map(|name| {
                    let mut key_name = None;
                    cf::config_get_string_option(s, name.as_bytes(), &mut key_name)
                        .then(|| key_name.unwrap_or(b"").to_vec())
                })
                .collect()
        });
    if let Some(keys) = keys {
        for (i, key_name) in keys.into_iter().enumerate() {
            if let Some(key_name) = key_name {
                // The platform takes UTF-8 names; SDL's scancode names are
                // ASCII, so lossy conversion only affects names that match
                // nothing either way.
                let scancode = g
                    .platform
                    .get_scancode_from_name(&String::from_utf8_lossy(&key_name));
                if scancode != SDL_SCANCODE_UNKNOWN {
                    g.config.key_settings[i] = scancode as u32;
                }
            }
        }
    }
}

#[doc(alias = "JE_setNewGameSpeed")]
pub fn je_set_new_game_speed(g: &mut Game) {
    let speed: u16;
    let c = &mut g.config;
    let frame_count_max = &mut g.nortsong.frame_count_max;
    match c.fast_play {
        // Pentium Hyper
        1 => {
            speed = 0x3000;
            c.smooth_scroll = true;
            *frame_count_max = 2;
        }
        2 => {
            speed = 0x2000;
            c.smooth_scroll = false;
            *frame_count_max = 2;
        }
        // Slug mode
        3 => {
            speed = 0x5300;
            c.smooth_scroll = true;
            *frame_count_max = 4;
        }
        // Slower
        4 => {
            speed = 0x4300;
            c.smooth_scroll = true;
            *frame_count_max = 3;
        }
        // Slow: 2.5 periods of 0x4300 per frame
        5 => {
            speed = 0x53C0;
            c.smooth_scroll = true;
            *frame_count_max = 2;
        }
        // Normal (upstream asserts on other values, then falls through to
        // Normal; NDEBUG builds only fall through)
        _ => {
            speed = 0x4300;
            c.smooth_scroll = true;
            *frame_count_max = 2;
        }
    }

    crate::nortsong::set_frame_speed(g, speed);
    let frame_count_max = g.nortsong.frame_count_max;
    crate::nortsong::set_frame_count(g, frame_count_max);
}

#[doc(alias = "loadConfiguration")]
pub fn load_configuration(g: &mut Game) {
    let mut invalid = false;

    let mut file = file::user_file_open(g, TYRIAN_CONFIG_FILENAME, "rb");
    if file.error {
        let message = format!(
            "Failed to open '{}': {}",
            TYRIAN_CONFIG_FILENAME,
            file::file_get_error(&file)
        );
        g.platform.log_warn(LOG_CATEGORY, &message);

        invalid = true;
    } else {
        // Upstream leaves this uninitialized; the oracle (and every path that
        // keeps these values) zero-fills it.
        let mut data = [0u8; 28];
        file::file_read_exactly(&mut file, &mut data);

        invalid |= file::file_get_length(&mut file) != data.len() as i64;

        invalid |= file.error;

        if file.error {
            let message = format!(
                "Failed to read from '{}': {}",
                TYRIAN_CONFIG_FILENAME,
                file::file_get_error(&file)
            );
            g.platform.log_error(LOG_CATEGORY, &message);
        }

        file::file_close(&mut file);

        let mut reader = MemReader::new(&data);
        let r = &mut reader;
        let c = &mut g.config;
        let n = &mut g.nortsong;

        c.background2 = mr::mem_read_bool(r);
        c.game_speed = mr::mem_read_u8(r);
        c.input_device_ = mr::mem_read_u8(r);
        c.j_configure = mr::mem_read_u8(r);
        c.version_num = mr::mem_read_u8(r);
        c.processor_type = mr::mem_read_u8(r);
        c.midi_port = mr::mem_read_u8(r);
        c.sound_effects = mr::mem_read_u8(r);
        c.gamma_correction = mr::mem_read_u8(r);
        c.difficulty_level = mr::mem_read_s8(r);
        mr::mem_read_u8_array(r, &mut c.joy_button_assign);
        n.tyr_music_volume = mr::mem_read_u16(r);
        n.fx_volume = mr::mem_read_u16(r);
        c.input_device1 = mr::mem_read_u8(r);
        c.input_device2 = mr::mem_read_u8(r);
        mr::mem_read_u8_array(r, &mut c.dos_key_settings);

        debug_assert!(reader.size() == 0 || reader.error);
        invalid |= reader.error;

        c.input_device_ = 0;
        if c.j_configure == 0 {
            c.j_configure = 1;
        }
        // Game resets version number; ShipEdit doesn't.
        c.version_num = 2;

        if n.tyr_music_volume > 255 {
            n.tyr_music_volume = 255;
        }
        if n.fx_volume > 255 {
            n.fx_volume = 255;
        }
    }

    if invalid {
        let message = format!("'{TYRIAN_CONFIG_FILENAME}' is invalid or missing.");
        g.platform.log_warn(LOG_CATEGORY, &message);

        let c = &mut g.config;
        c.background2 = true;
        c.game_speed = 4;
        c.input_device_ = 0;
        c.j_configure = 0;
        c.version_num = 2;
        c.processor_type = 3;
        c.midi_port = 1;
        c.sound_effects = 1;
        c.gamma_correction = 0;
        c.difficulty_level = 0;
        c.joy_button_assign = DEFAULT_JOY_BUTTON_ASSIGN;
        g.nortsong.tyr_music_volume = 223;
        g.nortsong.fx_volume = 223;
        c.input_device1 = 0;
        c.input_device2 = 0;
        c.dos_key_settings = DEFAULT_DOS_KEY_SETTINGS;
    }

    load_open_tyrian_config(g);

    let (music, fx) = (g.nortsong.tyr_music_volume, g.nortsong.fx_volume);
    crate::loudness::set_volume(g, music as u8, fx as u8);

    je_init_processor_type(g);
}

#[doc(alias = "saveOpenTyrianConfig")]
pub fn save_open_tyrian_config(g: &mut Game) {
    let config = &mut g.config.opentyrian_config;

    // Upstream exits when adding a section runs out of memory; Rust aborts
    // on allocation failure instead.
    let section = cf::config_find_or_add_section(config, b"video", None);

    cf::config_set_int_option(section, b"fullscreen", g.video.fullscreen_display);

    let scaler_name = crate::video_scale::SCALERS[g.video_scale.scaler as usize].name;
    cf::config_set_string_option(section, b"scaler", Some(scaler_name.as_bytes()));

    let mode_name = crate::video::SCALING_MODE_NAMES[g.video.scaling_mode.to_c() as usize];
    cf::config_set_string_option(section, b"scaling_mode", Some(mode_name.as_bytes()));

    let section = cf::config_find_or_add_section(config, b"keyboard", None);

    for (i, name) in KEY_SETTING_NAMES.iter().enumerate() {
        let key_name = g
            .platform
            .get_scancode_name(g.config.key_settings[i] as i32);
        let key_name = (!key_name.is_empty()).then_some(key_name.as_bytes());
        cf::config_set_string_option(section, name.as_bytes(), key_name);
    }

    let mut file = file::user_file_open(g, OPENTYRIAN_CONFIG_FILENAME, "w");
    if file.error {
        let message = format!(
            "Failed to open '{}': {}",
            OPENTYRIAN_CONFIG_FILENAME,
            file::file_get_error(&file)
        );
        g.platform.log_error(LOG_CATEGORY, &message);

        return;
    }

    let stream = file.f.as_mut().expect("open file has a stream");
    cf::config_write(&g.config.opentyrian_config, stream);
    file.error |= stream.ferror();

    file::file_flush(&mut file);

    if file.error {
        let message = format!(
            "Failed to write to '{}': {}",
            OPENTYRIAN_CONFIG_FILENAME,
            file::file_get_error(&file)
        );
        g.platform.log_error(LOG_CATEGORY, &message);
    }

    file::file_close(&mut file);
}

pub fn playeritems_to_pitems(
    p_items: &mut JePItemsType,
    items: &PlayerItems,
    initial_episode_num: u8,
) {
    p_items[0] = items.weapon[FRONT_WEAPON].id;
    p_items[1] = items.weapon[REAR_WEAPON].id;
    p_items[2] = items.super_arcade_mode;
    p_items[3] = items.sidekick[LEFT_SIDEKICK];
    p_items[4] = items.sidekick[RIGHT_SIDEKICK];
    p_items[5] = items.generator;
    p_items[6] = items.sidekick_level;
    p_items[7] = items.sidekick_series;
    p_items[8] = initial_episode_num;
    p_items[9] = items.shield;
    p_items[10] = items.special;
    p_items[11] = items.ship;
}

pub fn pitems_to_playeritems(
    items: &mut PlayerItems,
    p_items: &JePItemsType,
    initial_episode_num: Option<&mut u8>,
) {
    items.weapon[FRONT_WEAPON].id = p_items[0];
    items.weapon[REAR_WEAPON].id = p_items[1];
    items.super_arcade_mode = p_items[2];
    items.sidekick[LEFT_SIDEKICK] = p_items[3];
    items.sidekick[RIGHT_SIDEKICK] = p_items[4];
    items.generator = p_items[5];
    items.sidekick_level = p_items[6];
    items.sidekick_series = p_items[7];
    if let Some(initial_episode_num) = initial_episode_num {
        *initial_episode_num = p_items[8];
    }
    items.shield = p_items[9];
    items.special = p_items[10];
    items.ship = p_items[11];
}

/// `JE_saveGame(slot, name)`: `name` holds at least 14 bytes.
#[doc(alias = "JE_saveGame")]
pub fn je_save_game(g: &mut Game, slot: u8, name: &[u8]) {
    let s = usize::from(slot) - 1;
    let c = &mut g.config;
    let players = &mut g.player.player;

    c.save_files[s].initial_difficulty = c.initial_difficulty as u8;
    c.save_files[s].game_has_repeated = c.game_has_repeated;
    c.save_files[s].level = u16::from(c.save_level);

    players[0].items.super_arcade_mode = if c.super_tyrian {
        SA_SUPERTYRIAN
    } else if c.super_arcade_mode == SA_NONE && c.one_player_action {
        SA_ARCADE
    } else {
        c.super_arcade_mode
    };

    let initial_episode_num = g.episodes.initial_episode_num;
    playeritems_to_pitems(
        &mut c.save_files[s].items,
        &players[0].items,
        initial_episode_num,
    );

    if c.two_player_mode {
        playeritems_to_pitems(&mut c.save_files[s].last_items, &players[1].items, 0);
    } else {
        playeritems_to_pitems(&mut c.save_files[s].last_items, &players[0].last_items, 0);
    }

    c.save_files[s].score = players[0].cash as i32;
    c.save_files[s].score2 = players[1].cash as i32;

    c.save_files[s].level_name = c.last_level_name;
    c.save_files[s].cubes = c.last_cube_max as u8;

    if c_str_eq(&c.last_level_name, b"Completed") {
        g.varz.temp = g.episodes.episode_num.wrapping_sub(1);
        if g.varz.temp < 1 {
            // JE: {Episodemax is 4 for completion purposes}
            g.varz.temp = crate::episodes::EPISODE_AVAILABLE as u8;
        }
        c.save_files[s].episode = g.varz.temp;
    } else {
        c.save_files[s].episode = g.episodes.episode_num;
    }

    c.save_files[s].difficulty = c.difficulty_level as u8;
    c.save_files[s].secret_hint = c.secret_hint;
    c.save_files[s].input1 = c.input_device[0];
    c.save_files[s].input2 = c.input_device[1];

    c.save_files[s].name[..14].copy_from_slice(&name[..14]);
    c.save_files[s].name[14] = 0;

    for port in 0..2 {
        // if two-player, use first player's front and second player's rear weapon
        let p = if c.two_player_mode { port } else { 0 };
        c.save_files[s].power[port] = players[p].items.weapon[port].power;
    }

    save_saves(g);
}

#[doc(alias = "JE_loadGame")]
pub fn je_load_game(g: &mut Game, slot: u8) -> Flow {
    let s = usize::from(slot) - 1;
    let c = &mut g.config;
    let players = &mut g.player.player;

    c.super_tyrian = false;
    c.one_player_action = false;
    c.two_player_mode = false;
    c.extra_game = false;
    c.galaga_mode = false;

    c.initial_difficulty = c.save_files[s].initial_difficulty as i8;
    c.game_has_repeated = c.save_files[s].game_has_repeated;
    c.two_player_mode = s > 10;
    c.difficulty_level = c.save_files[s].difficulty as i8;

    pitems_to_playeritems(
        &mut players[0].items,
        &c.save_files[s].items,
        Some(&mut g.episodes.initial_episode_num),
    );

    c.super_arcade_mode = players[0].items.super_arcade_mode;

    if c.super_arcade_mode == SA_SUPERTYRIAN {
        c.super_tyrian = true;
    }
    if c.super_arcade_mode != SA_NONE {
        c.one_player_action = true;
    }
    if c.super_arcade_mode > SA_NORTSHIPZ {
        c.super_arcade_mode = SA_NONE;
    }

    if c.two_player_mode {
        c.one_player_action = false;

        pitems_to_playeritems(&mut players[1].items, &c.save_files[s].last_items, None);
    } else {
        pitems_to_playeritems(
            &mut players[0].last_items,
            &c.save_files[s].last_items,
            None,
        );
    }

    // Compatibility with old version
    if players[1].items.sidekick_level < 101 {
        players[1].items.sidekick_level = 101;
        players[1].items.sidekick_series = players[1].items.sidekick[LEFT_SIDEKICK];
    }

    players[0].cash = i64::from(c.save_files[s].score) as u64;
    players[1].cash = i64::from(c.save_files[s].score2) as u64;

    c.main_level = c.save_files[s].level as u8;
    c.cube_max = u16::from(c.save_files[s].cubes);
    c.last_cube_max = c.cube_max;

    c.secret_hint = c.save_files[s].secret_hint;
    c.input_device[0] = c.save_files[s].input1;
    c.input_device[1] = c.save_files[s].input2;

    for port in 0..2 {
        // if two-player, use first player's front and second player's rear weapon
        let p = if c.two_player_mode { port } else { 0 };
        players[p].items.weapon[port].power = c.save_files[s].power[port];
    }

    let mut episode = i32::from(c.save_files[s].episode);

    c.level_name = c.save_files[s].level_name;

    if c_str_eq(&c.level_name, b"Completed") {
        // Increment episode.  Episode EPISODE_AVAILABLE goes to 1.
        let available = crate::episodes::EPISODE_AVAILABLE as i32;
        if episode == available {
            episode = 1;
        } else if episode < available {
            episode += 1;
        }
    }

    je_init_episode(g, episode as u8)?;
    g.config.save_level = g.config.main_level;
    g.config.last_level_name = g.config.level_name;
    Ok(())
}

#[doc(alias = "JE_initProcessorType")]
pub fn je_init_processor_type(g: &mut Game) {
    // SYN: Originally this proc looked at your hardware specs and chose
    // appropriate options. We don't care, so I'll just set decent defaults
    // here.
    let c = &mut g.config;

    c.wild = false;
    c.super_wild = false;
    c.smooth_scroll = true;
    c.explosion_transparent = true;
    c.filtration_avail = false;
    c.background2 = true;
    c.display_score = true;

    match c.processor_type {
        // 386
        1 => {
            c.background2 = false;
            c.display_score = false;
            c.explosion_transparent = false;
        }
        // 486 - Default
        2 => {}
        // High Detail
        3 => c.smooth_scroll = false,
        // Pentium
        4 => {
            c.wild = true;
            c.filtration_avail = true;
        }
        // Nonstandard VGA
        5 => c.smooth_scroll = false,
        // SuperWild
        6 => {
            c.wild = true;
            c.super_wild = true;
            c.filtration_avail = true;
        }
        _ => {}
    }

    match c.game_speed {
        1 => c.fast_play = 3, // Slug Mode
        2 => c.fast_play = 4, // Slower
        3 => c.fast_play = 5, // Slow
        4 => c.fast_play = 0, // Normal
        5 => c.fast_play = 1, // Pentium Hyper
        _ => {}
    }
}

#[doc(alias = "saveConfiguration")]
pub fn save_configuration(g: &mut Game) {
    let mut data = [0u8; 28];

    let mut writer = MemWriter::new(&mut data);
    let c = &g.config;

    mw::mem_write_bool(&mut writer, c.background2);
    mw::mem_write_u8(&mut writer, c.game_speed);
    mw::mem_write_u8(&mut writer, c.input_device_);
    mw::mem_write_u8(&mut writer, c.j_configure);
    mw::mem_write_u8(&mut writer, c.version_num);
    mw::mem_write_u8(&mut writer, c.processor_type);
    mw::mem_write_u8(&mut writer, c.midi_port);
    mw::mem_write_u8(&mut writer, c.sound_effects);
    mw::mem_write_u8(&mut writer, c.gamma_correction);
    mw::mem_write_s8(&mut writer, c.difficulty_level);
    mw::mem_write_u8_array(&mut writer, &c.joy_button_assign);
    mw::mem_write_u16(&mut writer, g.nortsong.tyr_music_volume);
    mw::mem_write_u16(&mut writer, g.nortsong.fx_volume);
    mw::mem_write_u8(&mut writer, c.input_device1);
    mw::mem_write_u8(&mut writer, c.input_device2);
    mw::mem_write_u8_array(&mut writer, &c.dos_key_settings);

    debug_assert!(writer.size() == 0 && !writer.error);

    let mut file = file::user_file_open(g, TYRIAN_CONFIG_FILENAME, "wb");
    if file.error {
        let message = format!(
            "Failed to open '{}': {}",
            TYRIAN_CONFIG_FILENAME,
            file::file_get_error(&file)
        );
        g.platform.log_error(LOG_CATEGORY, &message);
    } else {
        file::file_write(&mut file, &data);
        file::file_flush(&mut file);

        if file.error {
            let message = format!(
                "Failed to write to '{}': {}",
                TYRIAN_CONFIG_FILENAME,
                file::file_get_error(&file)
            );
            g.platform.log_error(LOG_CATEGORY, &message);
        }

        file::file_close(&mut file);
    }

    save_open_tyrian_config(g);
}

#[doc(alias = "loadSaves")]
pub fn load_saves(g: &mut Game) {
    let mut invalid = false;

    let mut file = file::user_file_open(g, TYRIAN_SAVE_FILENAME, "rb");
    if file.error {
        let message = format!(
            "Failed to open '{}': {}",
            TYRIAN_SAVE_FILENAME,
            file::file_get_error(&file)
        );
        g.platform.log_warn(LOG_CATEGORY, &message);

        invalid = true;
    } else {
        let mut data = [0u8; SAVE_FILE_SIZE + 4];
        file::file_read_exactly(&mut file, &mut data);

        invalid |= file.error;

        if file.error {
            let message = format!(
                "Failed to read from '{}': {}",
                TYRIAN_SAVE_FILENAME,
                file::file_get_error(&file)
            );
            g.platform.log_error(LOG_CATEGORY, &message);
        }

        file::file_close(&mut file);

        invalid |= !decrypt_save_data(&mut data);

        let mut reader = MemReader::new(&data);
        let r = &mut reader;
        let c = &mut g.config;

        for f in c.save_files.iter_mut() {
            f.encode = mr::mem_read_u16(r);
            f.level = mr::mem_read_u16(r);
            mr::mem_read_u8_array(r, &mut f.items);
            f.score = mr::mem_read_u32(r) as i32;
            f.score2 = mr::mem_read_u32(r) as i32;
            let level_name_len = mr::mem_read_u8(r);
            mr::mem_read_char_array(r, &mut f.level_name[..9]);
            f.level_name[usize::from(level_name_len.min(9))] = 0;
            mr::mem_read_char_array(r, &mut f.name[..14]);
            f.name[14] = 0;
            f.cubes = mr::mem_read_u8(r);
            mr::mem_read_u8_array(r, &mut f.power);
            f.episode = mr::mem_read_u8(r);
            mr::mem_read_u8_array(r, &mut f.last_items);
            f.difficulty = mr::mem_read_u8(r);
            f.secret_hint = mr::mem_read_u8(r);
            f.input1 = mr::mem_read_u8(r);
            f.input2 = mr::mem_read_u8(r);
            f.game_has_repeated = mr::mem_read_bool(r);
            f.initial_difficulty = mr::mem_read_u8(r);
            f.high_score1 = mr::mem_read_s32(r);
            f.high_score2 = mr::mem_read_s32(r);
            let high_score_name_len = mr::mem_read_u8(r);
            mr::mem_read_char_array(r, &mut f.high_score_name[..29]);
            f.high_score_name[usize::from(high_score_name_len.min(29))] = 0;
            f.high_score_diff = mr::mem_read_u8(r);
        }

        mr::mem_read_u8_array(r, &mut c.editor_item_avail);

        c.editor_level =
            (u16::from(c.editor_item_avail[98]) << 8) | u16::from(c.editor_item_avail[99]);

        debug_assert!(reader.size() == 4 || reader.error);
        invalid |= reader.error;
    }

    if invalid {
        let message = format!("'{TYRIAN_SAVE_FILENAME}' is invalid or missing.");
        g.platform.log_warn(LOG_CATEGORY, &message);

        g.config.save_files = [JeSaveFileType::default(); SAVE_FILES_NUM];

        for i in 0..SAVE_FILES_NUM {
            g.config.save_files[i].level = 0;

            g.config.save_files[i].name[..14].fill(b' ');
            g.config.save_files[i].name[14] = 0;

            g.config.save_files[i].high_score1 = ((mt_rand(g) % 20 + 1) * 1000) as i32;
            let name: &[u8] = if i % 6 < 3 {
                g.config.save_files[i].high_score2 = 0;
                let k = (mt_rand(g) % DEFAULT_HIGH_SCORE_NAMES.len() as u64) as usize;
                &DEFAULT_HIGH_SCORE_NAMES[k]
            } else {
                g.config.save_files[i].high_score2 = ((mt_rand(g) % 20 + 1) * 1000) as i32;
                let k = (mt_rand(g) % DEFAULT_TEAM_NAMES.len() as u64) as usize;
                &DEFAULT_TEAM_NAMES[k]
            };
            let n = strlen(name);
            g.config.save_files[i].high_score_name[..=n].copy_from_slice(&name[..=n]);
            g.config.save_files[i].high_score_diff = 0;
        }

        g.config.editor_item_avail = INITIAL_EDITOR_ITEM_AVAIL;

        g.config.editor_level = 800;
    }
}

#[doc(alias = "saveSaves")]
pub fn save_saves(g: &mut Game) {
    let mut data = [0u8; SAVE_FILE_SIZE + 4];

    let mut writer = MemWriter::new(&mut data);
    let w = &mut writer;
    let c = &mut g.config;

    for f in c.save_files.iter() {
        mw::mem_write_u16(w, f.encode);
        mw::mem_write_u16(w, f.level);
        mw::mem_write_u8_array(w, &f.items);
        mw::mem_write_u32(w, f.score as u32);
        mw::mem_write_u32(w, f.score2 as u32);
        mw::mem_write_u8(w, strlen(&f.level_name) as u8);
        mw::mem_write_char_array(w, &f.level_name[..9]);
        mw::mem_write_char_array(w, &f.name[..14]);
        mw::mem_write_u8(w, f.cubes);
        mw::mem_write_u8_array(w, &f.power);
        mw::mem_write_u8(w, f.episode);
        mw::mem_write_u8_array(w, &f.last_items);
        mw::mem_write_u8(w, f.difficulty);
        mw::mem_write_u8(w, f.secret_hint);
        mw::mem_write_u8(w, f.input1);
        mw::mem_write_u8(w, f.input2);
        mw::mem_write_bool(w, f.game_has_repeated);
        mw::mem_write_u8(w, f.initial_difficulty);
        mw::mem_write_s32(w, f.high_score1);
        mw::mem_write_s32(w, f.high_score2);
        mw::mem_write_u8(w, strlen(&f.high_score_name) as u8);
        mw::mem_write_char_array(w, &f.high_score_name[..29]);
        mw::mem_write_u8(w, f.high_score_diff);
    }

    c.editor_item_avail[98] = (c.editor_level >> 8) as u8;
    c.editor_item_avail[99] = c.editor_level as u8;

    mw::mem_write_u8_array(w, &c.editor_item_avail);

    debug_assert!(writer.size() == 4 && !writer.error);

    encrypt_save_data(&mut data);

    let mut file = file::user_file_open(g, TYRIAN_SAVE_FILENAME, "wb");
    if file.error {
        let message = format!(
            "Failed to open '{}': {}",
            TYRIAN_SAVE_FILENAME,
            file::file_get_error(&file)
        );
        g.platform.log_error(LOG_CATEGORY, &message);
    } else {
        file::file_write(&mut file, &data);
        file::file_flush(&mut file);

        if file.error {
            let message = format!(
                "Failed to write to '{}': {}",
                TYRIAN_SAVE_FILENAME,
                file::file_get_error(&file)
            );
            g.platform.log_error(LOG_CATEGORY, &message);
        }

        file::file_close(&mut file);
    }
}

/// The four check bytes over `data[..SAVE_FILE_SIZE]`: sum, negated sum,
/// `y * d + 1` chain, xor.
fn save_checksums(data: &[u8]) -> [u8; 4] {
    let body = &data[..SAVE_FILE_SIZE];
    let sum = body.iter().fold(0u8, |y, &d| y.wrapping_add(d));
    let neg = body.iter().fold(0u8, |y, &d| y.wrapping_sub(d));
    let mul = body
        .iter()
        .fold(1u8, |y, &d| y.wrapping_mul(d).wrapping_add(1));
    let xor = body.iter().fold(0u8, |y, &d| y ^ d);
    [sum, neg, mul, xor]
}

/// `encryptSaveData(data)`; `data` holds `SAVE_FILE_SIZE + 4` bytes.
#[doc(alias = "encryptSaveData")]
pub fn encrypt_save_data(data: &mut [u8]) {
    let check = save_checksums(data);
    data[SAVE_FILE_SIZE..SAVE_FILE_SIZE + 4].copy_from_slice(&check);

    for i in 0..SAVE_FILE_SIZE {
        data[i] ^= CRYPT_KEY[(i + 1) % 10];
        if i > 0 {
            data[i] ^= data[i - 1];
        }
    }
}

/// `decryptSaveData(data)`; `data` holds `SAVE_FILE_SIZE + 4` bytes.
#[doc(alias = "decryptSaveData")]
pub fn decrypt_save_data(data: &mut [u8]) -> bool {
    for i in (0..SAVE_FILE_SIZE).rev() {
        data[i] ^= CRYPT_KEY[(i + 1) % 10];
        if i > 0 {
            data[i] ^= data[i - 1];
        }
    }

    let check = save_checksums(data);
    // C compares the four bytes in order and stops at the first mismatch;
    // the result is the same.
    data[SAVE_FILE_SIZE..SAVE_FILE_SIZE + 4] == check
}
