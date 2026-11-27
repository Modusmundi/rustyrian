// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `destruct.c` / `destruct.h`: the two-player artillery
//! mini-game.
//!
//! C enums used as integer types stay integer type aliases with `pub const`
//! enumerators, because upstream does arithmetic on them (`++unit`,
//! `unit < MAX_UNITS`, indexing). Signed aliases are the enums with a `-1`
//! enumerator (C gives those type `int`); the others are `unsigned int`.
//!
//! `struct destruct_unit_s *` parameters (rule R8) are a player index plus the
//! unit's index in that player's `unit` array; functions that only touch the
//! pointed-to unit take `&mut DestructUnitS`.

use crate::config_file::{
    self as cf, ConfigOption, ConfigValue, NO_YES, OFF_ON, config_string_to_cstr,
};
use crate::fonthand::{FULL_SHADE, je_font_center, je_out_text, je_text_shade};
use crate::game::{EXIT_FAILURE, Flow, Game, Halt};
use crate::helptext::DESTRUCT_MODES;
use crate::keyboard::{
    KeyboardInput, handle_sdl_events, keyboard_clear_input, keyboard_get_input, keyboard_has_input,
};
use crate::loudness::{ChannelSamples, multi_sample_play, play_song, set_volume};
use crate::mtrand::{mt_rand, mt_rand_lt1};
use crate::nortsong::{FX_PLAY_VOL, delay_until_elapsed, set_frame_count};
use crate::palette::{fade_black, fade_palette};
use crate::picload::je_load_pic;
use crate::sndmast::{S_NONE, V_CLEARED_PLATFORM};
use crate::sprite::{TINY_FONT, blit_sprite2, free_sprite2s, je_load_comp_shapes};
use crate::varz::Sprite2Ref;
use crate::vga256d::{fill_rectangle_xy, je_pix, je_rectangle};
use crate::video::{ScreenId, je_clr256, je_show_vga};
use serde::{Deserialize, Serialize};
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};

pub const UNIT_HEIGHT: u32 = 12;
pub const MAX_KEY_OPTIONS: usize = 4;

/// `enum de_state_t`.
pub type DeStateT = u32;
pub const STATE_INIT: DeStateT = 0;
pub const STATE_RELOAD: DeStateT = 1;
pub const STATE_CONTINUE: DeStateT = 2;

/// `enum de_player_t`.
pub type DePlayerT = u32;
pub const PLAYER_LEFT: DePlayerT = 0;
pub const PLAYER_RIGHT: DePlayerT = 1;
pub const MAX_PLAYERS: usize = 2;

/// `enum de_team_t`.
pub type DeTeamT = u32;
pub const TEAM_LEFT: DeTeamT = 0;
pub const TEAM_RIGHT: DeTeamT = 1;
pub const MAX_TEAMS: usize = 2;

/// `enum de_mode_t`.
pub type DeModeT = i32;
pub const MODE_5CARDWAR: DeModeT = 0;
pub const MODE_TRADITIONAL: DeModeT = 1;
pub const MODE_HELIASSAULT: DeModeT = 2;
pub const MODE_HELIDEFENSE: DeModeT = 3;
pub const MODE_OUTGUNNED: DeModeT = 4;
pub const MODE_CUSTOM: DeModeT = 5;
pub const MODE_FIRST: DeModeT = MODE_5CARDWAR;
pub const MODE_LAST: DeModeT = MODE_CUSTOM;
pub const MAX_MODES: usize = 6;
pub const MODE_NONE: DeModeT = -1;

/// `enum de_unit_t`.
pub type DeUnitT = i32;
pub const UNIT_TANK: DeUnitT = 0;
pub const UNIT_NUKE: DeUnitT = 1;
pub const UNIT_DIRT: DeUnitT = 2;
pub const UNIT_SATELLITE: DeUnitT = 3;
pub const UNIT_MAGNET: DeUnitT = 4;
pub const UNIT_LASER: DeUnitT = 5;
pub const UNIT_JUMPER: DeUnitT = 6;
pub const UNIT_HELI: DeUnitT = 7;
pub const UNIT_FIRST: DeUnitT = UNIT_TANK;
pub const UNIT_LAST: DeUnitT = UNIT_HELI;
pub const MAX_UNITS: usize = 8;
pub const UNIT_NONE: DeUnitT = -1;

/// `enum de_shot_t`.
pub type DeShotT = i32;
pub const SHOT_TRACER: DeShotT = 0;
pub const SHOT_SMALL: DeShotT = 1;
pub const SHOT_LARGE: DeShotT = 2;
pub const SHOT_MICRO: DeShotT = 3;
pub const SHOT_SUPER: DeShotT = 4;
pub const SHOT_DEMO: DeShotT = 5;
pub const SHOT_SMALLNUKE: DeShotT = 6;
pub const SHOT_LARGENUKE: DeShotT = 7;
pub const SHOT_SMALLDIRT: DeShotT = 8;
pub const SHOT_LARGEDIRT: DeShotT = 9;
pub const SHOT_MAGNET: DeShotT = 10;
pub const SHOT_MINILASER: DeShotT = 11;
pub const SHOT_MEGALASER: DeShotT = 12;
pub const SHOT_LASERTRACER: DeShotT = 13;
pub const SHOT_MEGABLAST: DeShotT = 14;
pub const SHOT_MINI: DeShotT = 15;
pub const SHOT_BOMB: DeShotT = 16;
pub const SHOT_FIRST: DeShotT = SHOT_TRACER;
pub const SHOT_LAST: DeShotT = SHOT_BOMB;
pub const MAX_SHOT_TYPES: usize = 17;
pub const SHOT_INVALID: DeShotT = -1;

/// `enum de_expl_t`.
pub type DeExplT = u32;
pub const EXPL_NONE: DeExplT = 0;
pub const EXPL_MAGNET: DeExplT = 1;
pub const EXPL_DIRT: DeExplT = 2;
pub const EXPL_NORMAL: DeExplT = 3;

/// `enum de_trails_t`.
pub type DeTrailsT = u32;
pub const TRAILS_NONE: DeTrailsT = 0;
pub const TRAILS_NORMAL: DeTrailsT = 1;
pub const TRAILS_FULL: DeTrailsT = 2;

/// `enum de_pixel_t`.
pub type DePixelT = u32;
pub const PIXEL_BLACK: DePixelT = 0;
pub const PIXEL_DIRT: DePixelT = 25;

/// `enum de_mapflags_t`.
pub type DeMapflagsT = u32;
pub const MAP_NORMAL: DeMapflagsT = 0x00;
pub const MAP_WALLS: DeMapflagsT = 0x01;
pub const MAP_RINGS: DeMapflagsT = 0x02;
pub const MAP_HOLES: DeMapflagsT = 0x04;
pub const MAP_FUZZY: DeMapflagsT = 0x08;
pub const MAP_TALL: DeMapflagsT = 0x10;

/// `enum de_keys_t`.
pub type DeKeysT = u32;
pub const KEY_LEFT: DeKeysT = 0;
pub const KEY_RIGHT: DeKeysT = 1;
pub const KEY_UP: DeKeysT = 2;
pub const KEY_DOWN: DeKeysT = 3;
pub const KEY_CHANGE: DeKeysT = 4;
pub const KEY_FIRE: DeKeysT = 5;
pub const KEY_CYUP: DeKeysT = 6;
pub const KEY_CYDN: DeKeysT = 7;
pub const MAX_KEY: usize = 8;

/// `enum de_move_t`.
pub type DeMoveT = u32;
pub const MOVE_LEFT: DeMoveT = 0;
pub const MOVE_RIGHT: DeMoveT = 1;
pub const MOVE_UP: DeMoveT = 2;
pub const MOVE_DOWN: DeMoveT = 3;
pub const MOVE_CHANGE: DeMoveT = 4;
pub const MOVE_FIRE: DeMoveT = 5;
pub const MOVE_CYUP: DeMoveT = 6;
pub const MOVE_CYDN: DeMoveT = 7;
pub const MAX_MOVE: usize = 8;

/// `struct destruct_config_s`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DestructConfigS {
    pub max_shots: u32,
    pub min_walls: u32,
    pub max_walls: u32,
    pub max_explosions: u32,
    pub max_installations: u32,
    pub allow_custom: bool,
    pub alwaysalias: bool,
    pub jumper_straight: [bool; 2],
    pub ai: [bool; 2],
}

/// `struct destruct_unit_s`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DestructUnitS {
    /// `unitX`.
    pub unit_x: u32,
    /// `unitY`.
    pub unit_y: f32,
    /// `unitYMov`.
    pub unit_y_mov: f32,
    /// `isYInAir`.
    pub is_y_in_air: bool,
    /// `unitType`.
    pub unit_type: DeUnitT,
    /// `shotType`.
    pub shot_type: DeShotT,
    pub angle: f32,
    pub power: f32,
    /// `lastMove`.
    pub last_move: i32,
    pub ani_frame: u32,
    pub health: i32,
}

/// `struct destruct_shot_s`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DestructShotS {
    /// `isAvailable`.
    pub is_available: bool,
    pub x: f32,
    pub y: f32,
    pub xmov: f32,
    pub ymov: f32,
    pub gravity: bool,
    pub shottype: u32,
    pub trailx: [u32; 4],
    pub traily: [u32; 4],
    pub trailc: [u32; 4],
}

/// `struct destruct_explo_s`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DestructExploS {
    /// `isAvailable`.
    pub is_available: bool,
    pub x: u32,
    pub y: u32,
    pub explowidth: u32,
    pub explomax: u32,
    pub explofill: u32,
    /// `exploType`.
    pub explo_type: DeExplT,
}

/// `struct destruct_moves_s`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DestructMovesS {
    pub actions: [bool; MAX_MOVE],
}

/// `struct destruct_keys_s`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DestructKeysS {
    /// `Config` (`SDL_Scancode`).
    pub config: [[u32; MAX_KEY_OPTIONS]; MAX_KEY],
}

/// `struct destruct_ai_s`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DestructAiS {
    /// `c_Angle`.
    pub c_angle: i32,
    /// `c_Power`.
    pub c_power: i32,
    /// `c_Fire`.
    pub c_fire: i32,
    /// `c_noDown`.
    pub c_no_down: u32,
}

/// `struct destruct_player_s`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DestructPlayerS {
    pub is_cpu: bool,
    /// `aiMemory`.
    pub ai_memory: DestructAiS,
    /// `unit`: the `malloc`ed array of `config.max_installations` units;
    /// `None` is NULL (rule R8).
    pub unit: Option<Vec<DestructUnitS>>,
    pub moves: DestructMovesS,
    pub keys: DestructKeysS,
    pub team: DeTeamT,
    /// `unitsRemaining`.
    pub units_remaining: u32,
    /// `unitSelected`.
    pub unit_selected: u32,
    /// `shotDelay`.
    pub shot_delay: u32,
    pub score: u32,
}

/// `struct destruct_wall_s`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DestructWallS {
    /// `wallExist`.
    pub wall_exist: bool,
    /// `wallX`.
    pub wall_x: u32,
    /// `wallY`.
    pub wall_y: u32,
}

/// `struct destruct_world_s`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DestructWorldS {
    /// `baseMap`.
    #[serde(with = "crate::serde_big")]
    pub base_map: [u32; 320],
    /// `VGAScreen` (rule R8).
    pub vga_screen: Option<ScreenId>,
    /// `mapWalls`: the `malloc`ed array of `config.max_walls` walls.
    pub map_walls: Option<Vec<DestructWallS>>,
    /// `destructMode`.
    pub destruct_mode: DeModeT,
    /// `mapFlags`.
    pub map_flags: u32,
}

impl Default for DestructWorldS {
    fn default() -> Self {
        DestructWorldS {
            base_map: [0; 320],
            vga_screen: None,
            map_walls: None,
            destruct_mode: 0,
            map_flags: 0,
        }
    }
}

/// `demolish`.
pub const DEMOLISH: [bool; MAX_SHOT_TYPES] = [
    false, false, false, false, false, true, true, true, false, false, false, false, true, false,
    true, false, true,
];
/// `shotTrail`.
pub const SHOT_TRAIL: [i32; MAX_SHOT_TYPES] = {
    const N: i32 = TRAILS_NONE as i32;
    const R: i32 = TRAILS_NORMAL as i32;
    const F: i32 = TRAILS_FULL as i32;
    [N, N, N, R, R, R, F, F, N, N, N, R, F, R, F, R, N]
};
/// `shotDelay`.
pub const SHOT_DELAY: [i32; MAX_SHOT_TYPES] = [
    10, 30, 80, 20, 60, 100, 140, 200, 20, 60, 5, 15, 50, 5, 80, 16, 0,
];
/// `shotSound`: `S_SELECT`, `S_WEAPON_2`, `S_WEAPON_1`, `S_WEAPON_7`,
/// `S_WEAPON_7`, `S_EXPLOSION_9`, `S_EXPLOSION_22`, `S_EXPLOSION_22`,
/// `S_WEAPON_5`, `S_WEAPON_13`, `S_WEAPON_10`, `S_WEAPON_15`, `S_WEAPON_15`,
/// `S_WEAPON_26`, `S_WEAPON_14`, `S_WEAPON_7`, `S_WEAPON_7`.
pub const SHOT_SOUND: [i32; MAX_SHOT_TYPES] =
    [8, 2, 1, 7, 7, 9, 22, 22, 5, 13, 10, 15, 15, 26, 14, 7, 7];
/// `exploSize`.
pub const EXPLO_SIZE: [i32; MAX_SHOT_TYPES] =
    [4, 20, 30, 14, 22, 16, 40, 60, 10, 30, 0, 5, 10, 3, 15, 7, 0];
/// `shotBounce`.
pub const SHOT_BOUNCE: [bool; MAX_SHOT_TYPES] = [
    false, false, false, false, false, false, false, false, false, false, false, true, true, true,
    true, false, true,
];
/// `exploDensity`.
pub const EXPLO_DENSITY: [i32; MAX_SHOT_TYPES] =
    [2, 5, 10, 15, 20, 15, 25, 30, 40, 80, 0, 30, 30, 4, 30, 5, 0];
/// `shotDirt`.
pub const SHOT_DIRT: [i32; MAX_SHOT_TYPES] = {
    const O: i32 = EXPL_NONE as i32;
    const M: i32 = EXPL_MAGNET as i32;
    const D: i32 = EXPL_DIRT as i32;
    const N: i32 = EXPL_NORMAL as i32;
    [N, N, N, N, N, N, N, N, D, D, M, N, N, N, N, N, O]
};
/// `shotColor`.
pub const SHOT_COLOR: [i32; MAX_SHOT_TYPES] = [
    16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 10, 10, 10, 10, 16, 0,
];
/// `defaultWeapon`.
pub const DEFAULT_WEAPON: [i32; MAX_UNITS] = [
    SHOT_SMALL,
    SHOT_MICRO,
    SHOT_SMALLDIRT,
    SHOT_INVALID,
    SHOT_MAGNET,
    SHOT_MINILASER,
    SHOT_MICRO,
    SHOT_MINI,
];
/// `defaultCpuWeapon`.
pub const DEFAULT_CPU_WEAPON: [i32; MAX_UNITS] = [
    SHOT_SMALL,
    SHOT_MICRO,
    SHOT_DEMO,
    SHOT_INVALID,
    SHOT_MAGNET,
    SHOT_MINILASER,
    SHOT_MICRO,
    SHOT_MINI,
];
/// `defaultCpuWeaponB`.
pub const DEFAULT_CPU_WEAPON_B: [i32; MAX_UNITS] = [
    SHOT_DEMO,
    SHOT_SMALLNUKE,
    SHOT_DEMO,
    SHOT_INVALID,
    SHOT_MAGNET,
    SHOT_MEGALASER,
    SHOT_MICRO,
    SHOT_MINI,
];
/// `systemAngle`.
pub const SYSTEM_ANGLE: [i32; MAX_UNITS] = [1, 1, 1, 0, 0, 1, 0, 0];
/// `baseDamage`.
pub const BASE_DAMAGE: [i32; MAX_UNITS] = [200, 120, 400, 300, 80, 150, 600, 40];
/// `systemAni`.
pub const SYSTEM_ANI: [i32; MAX_UNITS] = [0, 0, 0, 1, 0, 0, 0, 1];

/// Initial value of the mutable `weaponSystems`.
pub const WEAPON_SYSTEMS_INIT: [[bool; MAX_SHOT_TYPES]; MAX_UNITS] = {
    const fn row(bits: [u8; MAX_SHOT_TYPES]) -> [bool; MAX_SHOT_TYPES] {
        let mut r = [false; MAX_SHOT_TYPES];
        let mut i = 0;
        while i < MAX_SHOT_TYPES {
            r[i] = bits[i] != 0;
            i += 1;
        }
        r
    }
    [
        row([1, 1, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]), // normal
        row([0, 0, 0, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]), // nuke
        row([0, 0, 0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0]), // dirt
        row([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]), // worthless
        row([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0]), // magnet
        row([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0]), // laser
        row([1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0]), // jumper
        row([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0]), // helicopter
    ]
};

/// `goodsel`: music destruct plays.
pub const GOODSEL: [u8; 14] = [1, 2, 6, 12, 13, 14, 17, 23, 24, 26, 28, 29, 32, 33];

/// Initial value of the mutable `basetypes` (`[0]` is the amount of units).
pub const BASETYPES_INIT: [[u8; 11]; 10] = {
    const T: u8 = UNIT_TANK as u8;
    const N: u8 = UNIT_NUKE as u8;
    const D: u8 = UNIT_DIRT as u8;
    const S: u8 = UNIT_SATELLITE as u8;
    const M: u8 = UNIT_MAGNET as u8;
    const L: u8 = UNIT_LASER as u8;
    const J: u8 = UNIT_JUMPER as u8;
    const H: u8 = UNIT_HELI as u8;
    [
        [5, T, T, N, D, D, S, M, L, J, H], // normal
        [1, T, T, T, T, T, T, T, T, T, T], // traditional
        [4, H, H, H, H, H, H, H, H, H, H], // weak heli attack fleet
        [8, T, T, T, N, N, N, D, M, L, J], // strong heli defense fleet
        [8, H, H, H, H, H, H, H, H, H, H], // strong heli attack fleet
        [4, T, T, T, T, N, N, D, M, J, J], // weak heli defense fleet
        [8, T, N, D, S, M, L, J, H, T, N], // overpowering fleet
        [4, T, T, N, D, T, L, J, H, N, J], // weak fleet
        [5, T, T, N, D, D, S, M, L, J, H], // left custom
        [5, T, T, N, D, D, S, M, L, J, H], // right custom
    ]
};

/// `baseLookup`.
pub const BASE_LOOKUP: [[u32; MAX_MODES]; MAX_PLAYERS] = [[0, 1, 3, 4, 6, 8], [0, 1, 2, 5, 7, 9]];
/// `GraphicBase`.
pub const GRAPHIC_BASE: [[u8; MAX_UNITS]; MAX_PLAYERS] = [
    [1, 6, 11, 58, 63, 68, 96, 153],
    [20, 25, 30, 77, 82, 87, 115, 172],
];
/// `ModeScore`.
pub const MODE_SCORE: [[u8; MAX_MODES]; MAX_PLAYERS] = [[1, 0, 0, 5, 0, 1], [1, 0, 5, 0, 1, 1]];

/// Initial value of the mutable `defaultKeyConfig` (`SDL_Scancode`s).
pub const DEFAULT_KEY_CONFIG_INIT: [[[u32; MAX_KEY_OPTIONS]; MAX_KEY]; MAX_PLAYERS] = [
    [
        [6, 0, 0, 0],    // C
        [25, 0, 0, 0],   // V
        [4, 0, 0, 0],    // A
        [29, 0, 0, 0],   // Z
        [226, 0, 0, 0],  // LALT
        [27, 225, 0, 0], // X, LSHIFT
        [224, 0, 0, 0],  // LCTRL
        [44, 0, 0, 0],   // SPACE
    ],
    [
        [80, 92, 0, 0],   // LEFT, KP_4
        [79, 94, 0, 0],   // RIGHT, KP_6
        [82, 96, 0, 0],   // UP, KP_8
        [81, 90, 0, 0],   // DOWN, KP_2
        [49, 93, 0, 0],   // BACKSLASH, KP_5
        [73, 40, 98, 88], // INSERT, RETURN, KP_0, KP_ENTER
        [75, 97, 0, 0],   // PAGEUP, KP_9
        [78, 91, 0, 0],   // PAGEDOWN, KP_3
    ],
];

/// `player_names`.
pub const PLAYER_NAMES: [&str; 2] = ["left", "right"];
/// `key_names`.
pub const KEY_NAMES: [&str; 8] = [
    "left",
    "right",
    "up",
    "down",
    "change",
    "fire",
    "previous weapon",
    "next weapon",
];
/// `unit_names`.
pub const UNIT_NAMES: [&str; 8] = [
    "tank",
    "nuke",
    "dirt",
    "satellite",
    "magnet",
    "laser",
    "jumper",
    "heli",
];

/// Initial value of `config`.
pub const CONFIG_INIT: DestructConfigS = DestructConfigS {
    max_shots: 40,
    min_walls: 20,
    max_walls: 20,
    max_explosions: 40,
    max_installations: 10,
    allow_custom: false,
    alwaysalias: false,
    jumper_straight: [true, false],
    ai: [true, false],
};

/// Mutable globals of `destruct.c` (rule R3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub weapon_systems: [[bool; MAX_SHOT_TYPES]; MAX_UNITS],
    pub basetypes: [[u8; 11]; 10],
    pub default_key_config: [[[u32; MAX_KEY_OPTIONS]; MAX_KEY]; MAX_PLAYERS],
    /// `destructTempScreen` (rule R8).
    pub destruct_temp_screen: Option<ScreenId>,
    pub destruct_first_time: bool,
    pub config: DestructConfigS,
    pub destruct_player: [DestructPlayerS; MAX_PLAYERS],
    pub world: DestructWorldS,
    /// `shotRec`: the `malloc`ed array of `config.max_shots` shots.
    pub shot_rec: Option<Vec<DestructShotS>>,
    /// `exploRec`: the `malloc`ed array of `config.max_explosions`.
    pub explo_rec: Option<Vec<DestructExploS>>,
    /// `JE_eSound`'s `static int exploSoundChannel`.
    pub je_e_sound_explo_sound_channel: i32,
    /// `DE_RunTick`'s `static unsigned int endDelay`.
    pub de_run_tick_end_delay: u32,
}

impl Default for State {
    fn default() -> Self {
        State {
            weapon_systems: WEAPON_SYSTEMS_INIT,
            basetypes: BASETYPES_INIT,
            default_key_config: DEFAULT_KEY_CONFIG_INIT,
            destruct_temp_screen: None,
            destruct_first_time: false,
            config: CONFIG_INIT,
            destruct_player: Default::default(),
            world: DestructWorldS::default(),
            shot_rec: None,
            explo_rec: None,
            je_e_sound_explo_sound_channel: 0,
            de_run_tick_end_delay: 0,
        }
    }
}

/*** Platform constants (`SDL_Scancode`) ***/
const SDL_SCANCODE_UNKNOWN: u32 = 0;
const SDL_SCANCODE_P: usize = 19;
const SDL_SCANCODE_RETURN: u16 = 40;
const SDL_SCANCODE_ESCAPE: usize = 41;
const SDL_SCANCODE_BACKSPACE: usize = 42;
const SDL_SCANCODE_F1: usize = 58;
const SDL_SCANCODE_F10: usize = 67;
const SDL_SCANCODE_F11: usize = 68;
const SDL_SCANCODE_DOWN: u16 = 81;
const SDL_SCANCODE_UP: u16 = 82;

/*** Pointer helpers (rule R8) ***/

/// `destruct_player[p].unit`, which `JE_destructGame` allocates before any
/// other function runs.
fn units(g: &Game, p: usize) -> &[DestructUnitS] {
    g.destruct.destruct_player[p]
        .unit
        .as_deref()
        .expect("destruct units are allocated by JE_destructGame")
}

fn units_mut(g: &mut Game, p: usize) -> &mut [DestructUnitS] {
    g.destruct.destruct_player[p]
        .unit
        .as_deref_mut()
        .expect("destruct units are allocated by JE_destructGame")
}

fn unit_mut(g: &mut Game, p: usize, u: usize) -> &mut DestructUnitS {
    &mut units_mut(g, p)[u]
}

fn shots(g: &mut Game) -> &mut [DestructShotS] {
    g.destruct
        .shot_rec
        .as_deref_mut()
        .expect("shotRec is allocated by JE_destructGame")
}

fn explos(g: &mut Game) -> &mut [DestructExploS] {
    g.destruct
        .explo_rec
        .as_deref_mut()
        .expect("exploRec is allocated by JE_destructGame")
}

fn walls(g: &Game) -> &[DestructWallS] {
    g.destruct
        .world
        .map_walls
        .as_deref()
        .expect("mapWalls is allocated by JE_destructGame")
}

fn walls_mut(g: &mut Game) -> &mut [DestructWallS] {
    g.destruct
        .world
        .map_walls
        .as_deref_mut()
        .expect("mapWalls is allocated by JE_destructGame")
}

fn vga(g: &Game) -> ScreenId {
    g.video.vga_screen_id()
}

fn temp_screen(g: &Game) -> ScreenId {
    g.destruct
        .destruct_temp_screen
        .expect("destructTempScreen is set by JE_destructGame")
}

fn world_screen(g: &Game) -> ScreenId {
    g.destruct
        .world
        .vga_screen
        .expect("world.VGAScreen is set by JE_destructGame")
}

/// `memcpy(dst->pixels, src->pixels, len)` between two surfaces.
fn copy_pixels(g: &mut Game, dst: ScreenId, src: ScreenId, len: usize) {
    if dst == src {
        return;
    }
    let (d, s) = g.video.screen_pair_mut(dst, src);
    d.pixels[..len].copy_from_slice(&s.pixels[..len]);
}

fn surface_len(g: &Game, id: ScreenId) -> usize {
    let s = g.video.screen(id);
    (s.h as usize) * (s.pitch as usize)
}

/// `malloc(sizeof(T) * n)`, zero-filled as the oracle's allocator is. Sizes
/// that cannot be allocated make upstream write through NULL; Rust halts
/// instead (divergence D-UB-DESTRUCT-6).
fn alloc_zeroed<T: Default + Clone>(n: u32) -> Flow<Vec<T>> {
    let mut v = Vec::new();
    if v.try_reserve_exact(n as usize).is_err() {
        return Err(Halt { code: EXIT_FAILURE });
    }
    v.resize(n as usize, T::default());
    Ok(v)
}

/// The C `%` of an `unsigned long` by a divisor that malformed or rare
/// states make zero: upstream's arm64 build yields the dividend there
/// (divergence D-UB-DESTRUCT-5).
fn rem_or_dividend(a: u64, b: u64) -> u64 {
    a.checked_rem(b).unwrap_or(a)
}

/*** Configuration ***/

pub fn get_unit_by_name(unit_name: &[u8]) -> DeUnitT {
    let mut unit = UNIT_FIRST;
    while unit < MAX_UNITS as DeUnitT {
        if unit_name == UNIT_NAMES[unit as usize].as_bytes() {
            return unit;
        }
        unit += 1;
    }

    UNIT_NONE
}

/// Slot `i` of `option` for `foreach_option_i_value`: `None` past the end,
/// `Some(None)` for a NULL value (skipped), else the value.
fn option_value(option: &ConfigOption, i: usize) -> Option<Option<Vec<u8>>> {
    match &option.v {
        ConfigValue::Value(v) => (i == 0).then(|| config_string_to_cstr(v).map(<[u8]>::to_vec)),
        ConfigValue::Values(v) => v
            .get(i)
            .map(|v| config_string_to_cstr(v).map(<[u8]>::to_vec)),
    }
}

/// `load_destruct_config(&opentyrian_config)`. Upstream exits only when a
/// section or option cannot be allocated, which cannot happen here.
pub fn load_destruct_config(g: &mut Game) {
    let Game {
        config,
        destruct,
        platform,
        ..
    } = g;
    let config_ = &mut config.opentyrian_config;
    let s = destruct;

    let section = cf::config_find_or_add_section(config_, b"destruct", None);

    s.config.alwaysalias =
        cf::config_get_or_set_bool_option(section, b"antialias craters", false, NO_YES);

    s.weapon_systems[UNIT_LASER as usize][SHOT_LASERTRACER as usize] =
        cf::config_get_or_set_bool_option(section, b"tracer laser", false, OFF_ON);

    s.config.max_shots = cf::config_get_or_set_int_option(section, b"max shots", 40) as u32;
    s.config.max_explosions =
        cf::config_get_or_set_int_option(section, b"max explosions", 40) as u32;
    s.config.min_walls = cf::config_get_or_set_int_option(section, b"min walls", 20) as u32;
    s.config.max_walls = cf::config_get_or_set_int_option(section, b"max walls", 20) as u32;

    s.config.ai[0] = cf::config_get_or_set_bool_option(section, b"left ai", true, NO_YES);
    s.config.jumper_straight[0] =
        cf::config_get_or_set_bool_option(section, b"left jumper fires straight", true, NO_YES);
    s.config.ai[1] = cf::config_get_or_set_bool_option(section, b"right ai", false, NO_YES);
    s.config.jumper_straight[1] =
        cf::config_get_or_set_bool_option(section, b"right jumper fires straight", false, NO_YES);

    for (p, player_name) in PLAYER_NAMES.iter().enumerate() {
        let name = player_name.as_bytes();
        if cf::config_find_section(config_, b"destruct keyboard", Some(name)).is_none() {
            cf::config_add_section(config_, b"destruct keyboard", Some(name));
        }
        let section = cf::config_find_section(config_, b"destruct keyboard", Some(name))
            .expect("just found or added");

        for (k, key_name) in KEY_NAMES.iter().enumerate() {
            let option = cf::config_get_or_set_option(section, key_name.as_bytes(), None);

            let mut i = 0;
            while let Some(value) = option_value(option, i) {
                let Some(value) = value else {
                    i += 1;
                    continue;
                };
                let key = platform.get_scancode_from_name(&String::from_utf8_lossy(&value)) as u32;
                if key != SDL_SCANCODE_UNKNOWN && i < MAX_KEY_OPTIONS {
                    s.default_key_config[p][k][i] = key;
                    i += 1;
                } else {
                    cf::config_remove_value(option, i as u32);
                }
            }

            if cf::config_get_value_count(option) > 0 {
                for i in cf::config_get_value_count(option) as usize..MAX_KEY_OPTIONS {
                    s.default_key_config[p][k][i] = SDL_SCANCODE_UNKNOWN;
                }
            } else {
                for i in 0..MAX_KEY_OPTIONS {
                    if s.default_key_config[p][k][i] != SDL_SCANCODE_UNKNOWN {
                        let name = platform.get_scancode_name(s.default_key_config[p][k][i] as i32);
                        cf::config_add_value(option, name.as_bytes());
                    }
                }
            }
        }
    }

    if cf::config_find_section(config_, b"destruct custom", None).is_none() {
        cf::config_add_section(config_, b"destruct custom", None);
    }
    let section =
        cf::config_find_section(config_, b"destruct custom", None).expect("just found or added");

    s.config.allow_custom = cf::config_get_or_set_bool_option(section, b"enable", false, NO_YES);

    for p in 0..MAX_PLAYERS {
        let buffer = format!("{} num units", PLAYER_NAMES[p]);
        s.basetypes[8 + p][0] = cf::config_get_or_set_int_option(
            section,
            buffer.as_bytes(),
            i32::from(s.basetypes[8 + p][0]),
        ) as u8;

        let buffer = format!("{} unit", PLAYER_NAMES[p]);
        let option = cf::config_get_or_set_option(section, buffer.as_bytes(), None);

        let mut i = 0;
        while let Some(value) = option_value(option, i) {
            let Some(value) = value else {
                i += 1;
                continue;
            };
            let unit = get_unit_by_name(&value);
            if unit != UNIT_NONE && 1 + i < 11 {
                s.basetypes[8 + p][1 + i] = unit as u8;
                i += 1;
            } else {
                cf::config_remove_value(option, i as u32);
            }
        }

        if cf::config_get_value_count(option) > 0 {
            let mut i = cf::config_get_value_count(option) as usize;
            while 1 + i < 11 {
                s.basetypes[8 + p][1 + i] = UNIT_TANK as u8;
                cf::config_add_value(option, UNIT_NAMES[UNIT_TANK as usize].as_bytes());
                i += 1;
            }
        } else {
            let mut i = 0;
            while 1 + i < 11 {
                cf::config_add_value(
                    option,
                    UNIT_NAMES[usize::from(s.basetypes[8 + p][1 + i])].as_bytes(),
                );
                i += 1;
            }
        }
    }
}

#[doc(alias = "JE_destructGame")]
pub fn je_destruct_game(g: &mut Game) -> Flow {
    je_clr256(g, vga(g));
    je_show_vga(g);

    load_destruct_config(g);

    g.destruct.shot_rec = Some(alloc_zeroed(g.destruct.config.max_shots)?);
    g.destruct.explo_rec = Some(alloc_zeroed(g.destruct.config.max_explosions)?);
    g.destruct.world.map_walls = Some(alloc_zeroed(g.destruct.config.max_walls)?);

    for i in 0..10 {
        g.destruct.config.max_installations = g
            .destruct
            .config
            .max_installations
            .max(u32::from(g.destruct.basetypes[i][0]));
    }
    let n = g.destruct.config.max_installations;
    g.destruct.destruct_player[PLAYER_LEFT as usize].unit = Some(alloc_zeroed(n)?);
    g.destruct.destruct_player[PLAYER_RIGHT as usize].unit = Some(alloc_zeroed(n)?);

    g.destruct.destruct_temp_screen = g.video.game_screen;
    g.destruct.world.vga_screen = g.video.vga_screen;

    je_load_comp_shapes(g, Sprite2Ref::DestructSpriteSheet, b'~')?;

    fade_black(g, 1)?;

    je_destruct_main(g)?;

    free_sprite2s(&mut g.sprite.destruct_sprite_sheet);

    g.destruct.shot_rec = None;
    g.destruct.explo_rec = None;
    g.destruct.world.map_walls = None;
    g.destruct.destruct_player[PLAYER_LEFT as usize].unit = None;
    g.destruct.destruct_player[PLAYER_RIGHT as usize].unit = None;
    Ok(())
}

#[doc(alias = "JE_destructMain")]
pub fn je_destruct_main(g: &mut Game) -> Flow {
    je_load_pic(g, vga(g), 11, false)?;
    je_intro_screen(g)?;

    de_reset_players(g);

    g.destruct.destruct_player[PLAYER_LEFT as usize].is_cpu =
        g.destruct.config.ai[PLAYER_LEFT as usize];
    g.destruct.destruct_player[PLAYER_RIGHT as usize].is_cpu =
        g.destruct.config.ai[PLAYER_RIGHT as usize];

    loop {
        g.destruct.world.destruct_mode = je_mode_select(g)?;

        if g.destruct.world.destruct_mode == MODE_NONE {
            break;
        }

        loop {
            g.destruct.destruct_first_time = true;
            je_load_pic(g, vga(g), 11, false)?;

            de_reset_units(g);
            de_reset_level(g)?;
            let cur_state = loop {
                let cur_state = de_run_tick(g)?;
                if cur_state != STATE_CONTINUE {
                    break cur_state;
                }
            };

            fade_black(g, 25)?;
            if cur_state != STATE_RELOAD {
                break;
            }
        }
    }
    Ok(())
}

/// `while (true) { setFrameCount(1); delayUntilElapsed(); handleSdlEvents();
/// if (keyboardGetInput(NULL)) break; }`.
fn wait_for_any_key(g: &mut Game) -> Flow {
    loop {
        set_frame_count(g, 1);

        delay_until_elapsed(g);

        handle_sdl_events(g)?;

        if keyboard_get_input(g, None) {
            break;
        }
    }
    Ok(())
}

/// `memcpy(VGAScreen2->pixels, VGAScreen->pixels, VGAScreen2->h * VGAScreen2->pitch)`.
fn save_vga(g: &mut Game) {
    let vga2 = g
        .video
        .vga_screen2
        .expect("VGAScreen2 used before init_video");
    let len = surface_len(g, vga2);
    copy_pixels(g, vga2, vga(g), len);
}

/// `memcpy(VGAScreen->pixels, VGAScreen2->pixels, VGAScreen->h * VGAScreen->pitch)`.
fn restore_vga(g: &mut Game) {
    let vga2 = g
        .video
        .vga_screen2
        .expect("VGAScreen2 used before init_video");
    let len = surface_len(g, vga(g));
    copy_pixels(g, vga(g), vga2, len);
}

fn out_text_centered(g: &mut Game, y: i32, s: &[u8], colorbank: u32, brightness: i32) {
    let x = je_font_center(g, s, TINY_FONT as u32);
    je_out_text(g, vga(g), x, y, s, colorbank, brightness);
}

#[doc(alias = "JE_introScreen")]
pub fn je_intro_screen(g: &mut Game) -> Flow {
    save_vga(g);
    let special = g.helptext.special_name[7];
    out_text_centered(g, 90, &special, 12, 5);
    let misc = g.helptext.misc_text[64];
    out_text_centered(g, 180, &misc, 15, 2);
    let misc = g.helptext.misc_text[65];
    out_text_centered(g, 190, &misc, 15, 2);
    je_show_vga(g);
    let colors = g.palette.colors;
    fade_palette(g, &colors, 15, 0, 255)?;

    wait_for_any_key(g)?;

    fade_black(g, 15)?;
    restore_vga(g);
    je_show_vga(g);
    Ok(())
}

/// Helper of `JE_modeSelect`.
#[doc(alias = "DrawModeSelectMenu")]
pub fn draw_mode_select_menu(g: &mut Game, mode: DeModeT) {
    let mut i = 0;
    while i < DESTRUCT_MODES as i32 {
        let name = g.helptext.destruct_mode_name[i as usize];
        let x = je_font_center(g, &name, TINY_FONT as u32);
        je_text_shade(
            g,
            vga(g),
            x,
            82 + i * 12,
            &name,
            12,
            i32::from(i == mode) * 4,
            FULL_SHADE,
        );
        i += 1;
    }
    if g.destruct.config.allow_custom {
        let x = je_font_center(g, b"Custom", TINY_FONT as u32);
        je_text_shade(
            g,
            vga(g),
            x,
            82 + i * 12,
            b"Custom",
            12,
            i32::from(i == mode) * 4,
            FULL_SHADE,
        );
    }
}

/// The selected mode, or `MODE_NONE` if the user quits.
#[doc(alias = "JE_modeSelect")]
pub fn je_mode_select(g: &mut Game) -> Flow<DeModeT> {
    save_vga(g);
    let mut mode = MODE_5CARDWAR;

    draw_mode_select_menu(g, mode);

    je_show_vga(g);
    let colors = g.palette.colors;
    fade_palette(g, &colors, 15, 0, 255)?;

    loop {
        draw_mode_select_menu(g, mode);
        je_show_vga(g);

        loop {
            set_frame_count(g, 1);

            delay_until_elapsed(g);

            handle_sdl_events(g)?;

            if keyboard_has_input(g) {
                break;
            }
        }

        let mut done = false;

        let mut keyboard_input = KeyboardInput::default();

        if keyboard_get_input(g, Some(&mut keyboard_input)) {
            match keyboard_input.scancode {
                s if s == SDL_SCANCODE_ESCAPE as u16 => {
                    mode = MODE_NONE;
                    done = true;
                }
                SDL_SCANCODE_RETURN => {
                    done = true;
                }
                SDL_SCANCODE_UP => {
                    if mode == MODE_FIRST {
                        if g.destruct.config.allow_custom {
                            mode = MODE_LAST;
                        } else {
                            mode = MODE_LAST - 1;
                        }
                    } else {
                        mode -= 1;
                    }
                }
                SDL_SCANCODE_DOWN => {
                    if mode >= MODE_LAST - 1 {
                        if g.destruct.config.allow_custom && mode == MODE_LAST - 1 {
                            mode += 1;
                        } else {
                            mode = MODE_FIRST;
                        }
                    } else {
                        mode += 1;
                    }
                }
                _ => {}
            }
        }

        if done {
            break;
        }
    }

    fade_black(g, 15)?;
    restore_vga(g);
    je_show_vga(g);
    Ok(mode)
}

#[doc(alias = "JE_generateTerrain")]
pub fn je_generate_terrain(g: &mut Game) -> Flow {
    g.destruct.world.map_flags = MAP_NORMAL;

    if mt_rand(g).is_multiple_of(2) {
        g.destruct.world.map_flags |= MAP_WALLS;
    }
    if mt_rand(g).is_multiple_of(4) {
        g.destruct.world.map_flags |= MAP_HOLES;
    }
    match mt_rand(g) % 4 {
        0 => g.destruct.world.map_flags |= MAP_FUZZY,
        1 => g.destruct.world.map_flags |= MAP_TALL,
        2 => g.destruct.world.map_flags |= MAP_RINGS,
        _ => {}
    }

    let song = i32::from(GOODSEL[(mt_rand(g) % 14) as usize]) - 1;
    play_song(g, song as u32)?;

    let map_flags = g.destruct.world.map_flags;
    de_generate_base_terrain(g, map_flags);
    de_generate_units(g);
    de_generate_walls(g);
    de_draw_base_terrain(g);

    if g.destruct.world.map_flags & MAP_RINGS != 0 {
        de_generate_rings(g, world_screen(g), PIXEL_DIRT as u8);
    }
    if g.destruct.world.map_flags & MAP_HOLES != 0 {
        de_generate_rings(g, world_screen(g), PIXEL_BLACK as u8);
    }

    je_alias_dirt(g, world_screen(g));
    je_show_vga(g);

    let temp = temp_screen(g);
    let len = surface_len(g, temp);
    copy_pixels(g, temp, vga(g), len);
    Ok(())
}

/// `DE_generateBaseTerrain(mapFlags, world.baseMap)` (the only caller's
/// array).
#[doc(alias = "DE_generateBaseTerrain")]
pub fn de_generate_base_terrain(g: &mut Game, map_flags: u32) {
    let mut sinewave = (f64::from(mt_rand_lt1(g)) * PI / 50.0 + f64::from(0.01f32)) as f32;
    let mut sinewave2 = (f64::from(mt_rand_lt1(g)) * PI / 50.0 + f64::from(0.01f32)) as f32;
    let cosinewave = (f64::from(mt_rand_lt1(g)) * PI / 50.0 + f64::from(0.01f32)) as f32;
    let cosinewave2 = (f64::from(mt_rand_lt1(g)) * PI / 50.0 + f64::from(0.01f32)) as f32;
    let mut height_mul: u32 = 20;

    if map_flags & MAP_FUZZY != 0 {
        sinewave = (PI - f64::from(mt_rand_lt1(g) * 0.3f32)) as f32;
        sinewave2 = (PI - f64::from(mt_rand_lt1(g) * 0.3f32)) as f32;
    }
    if map_flags & MAP_TALL != 0 {
        height_mul = 100;
    }

    for i in 1..=318u32 {
        let fi = i as f32;
        let h = (libm::sinf(sinewave * fi) * height_mul as f32
            + libm::sinf(sinewave2 * fi) * 15.0
            + libm::cosf(cosinewave * fi) * 10.0
            + libm::sinf(cosinewave2 * fi) * 15.0)
            .round()
            + 130.0;
        let newheight = c_float_to_uint(h);

        let newheight = newheight.clamp(40, 195);
        g.destruct.world.base_map[i as usize] = newheight;
    }
}

/// The C conversion `(unsigned int)f` of `destruct.c:959-960`, which tall
/// maps can reach with a negative value (undefined behaviour). Upstream's
/// result depends on the CPU, so this mirrors the build's target
/// (divergence D-UB-DESTRUCT-2):
/// - x86-64: clang converts through a 64-bit `cvttss2si` and keeps the low
///   32 bits; NaN and values outside `[-2^63, 2^63)` give `i64::MIN`.
/// - other targets (arm64 `fcvtzu`): saturating, as Rust's `as`.
fn c_float_to_uint(f: f32) -> u32 {
    if cfg!(target_arch = "x86_64") {
        const LIMIT: f32 = 9_223_372_036_854_775_808.0; // 2^63
        let i = if f.is_nan() || !(-LIMIT..LIMIT).contains(&f) {
            i64::MIN
        } else {
            f as i64
        };
        i as u32
    } else {
        f as u32
    }
}

/// `DE_drawBaseTerrain(world.baseMap)`.
#[doc(alias = "DE_drawBaseTerrain")]
pub fn de_draw_base_terrain(g: &mut Game) {
    for i in 1..=318usize {
        let y = g.destruct.world.base_map[i];
        je_rectangle(
            g,
            vga(g),
            i as i32,
            y as i32,
            i as i32,
            199,
            PIXEL_DIRT as i32,
        );
    }
}

/// `DE_generateUnits(world.baseMap)`.
#[doc(alias = "DE_generateUnits")]
pub fn de_generate_units(g: &mut Game) {
    for (i, lookup) in BASE_LOOKUP.iter().enumerate() {
        let mut num_satellites: u32 = 0;
        g.destruct.destruct_player[i].units_remaining = 0;

        let base = lookup[g.destruct.world.destruct_mode as usize] as usize;
        let mut j = 0usize;
        while (j as u32) < u32::from(g.destruct.basetypes[base][0]) {
            let unit_x = if i == PLAYER_LEFT as usize {
                (mt_rand(g) % 120) as u32 + 10
            } else {
                320 - ((mt_rand(g) % 120) as u32 + 22)
            };
            unit_mut(g, i, j).unit_x = unit_x;

            let y = je_placement_position(g, unit_x.wrapping_sub(1), 14);
            unit_mut(g, i, j).unit_y = y as f32;
            let unit_type = i32::from(g.destruct.basetypes[base][(mt_rand(g) % 10) as usize + 1]);
            unit_mut(g, i, j).unit_type = unit_type;

            if unit_type == UNIT_SATELLITE {
                if num_satellites == u32::from(g.destruct.basetypes[base][0]) {
                    unit_mut(g, i, j).unit_type = UNIT_TANK;
                    g.destruct.destruct_player[i].units_remaining += 1;
                } else {
                    let y = 30 + (mt_rand(g) % 40) as u32;
                    unit_mut(g, i, j).unit_y = y as f32;
                    num_satellites += 1;
                }
            } else {
                g.destruct.destruct_player[i].units_remaining += 1;
            }

            let u = unit_mut(g, i, j);
            u.last_move = 0;
            u.unit_y_mov = 0.0;
            u.is_y_in_air = false;
            u.angle = 0.0;
            u.power = if u.unit_type == UNIT_LASER { 6.0 } else { 3.0 };
            u.shot_type = DEFAULT_WEAPON[u.unit_type as usize];
            u.health = BASE_DAMAGE[u.unit_type as usize];
            u.ani_frame = 0;
            j += 1;
        }
    }
}

/// `DE_generateWalls(&world)`.
#[doc(alias = "DE_generateWalls")]
pub fn de_generate_walls(g: &mut Game) {
    if g.destruct.world.map_flags & MAP_WALLS == 0 {
        for i in 0..g.destruct.config.max_walls as usize {
            walls_mut(g)[i].wall_exist = false;
        }
        return;
    }

    let c = g.destruct.config;
    let r = crate::crand::rand_shared(&g.crand) as u32;
    let mut remain_walls = (rem_or_dividend(
        u64::from(r),
        u64::from(c.max_walls.wrapping_sub(c.min_walls).wrapping_add(1)),
    ) as u32)
        .wrapping_add(c.min_walls);

    loop {
        let mut wall_height = (mt_rand(g) % 5) as u32 + 1;
        if wall_height > remain_walls {
            wall_height = remain_walls;
        }

        let mut tries = 0;
        let mut wall_x;
        loop {
            let mut is_good = true;
            wall_x = (mt_rand(g) % 300) as u32 + 10;

            'outer: for i in 0..MAX_PLAYERS {
                for j in 0..c.max_installations as usize {
                    let unit_x = units(g, i)[j].unit_x;
                    if wall_x > unit_x.wrapping_sub(12) && wall_x < unit_x.wrapping_add(13) {
                        is_good = false;
                        break 'outer;
                    }
                }
            }

            tries += 1;

            if !(!is_good && tries < 5) {
                break;
            }
        }

        for i in 1..=wall_height {
            let y = je_placement_position(g, wall_x, 12).wrapping_sub(14 * i);
            // Malformed limits (`min walls` > `max walls`) index past the
            // array in C; Rust drops those walls (divergence D-UB-DESTRUCT-6).
            if let Some(w) = walls_mut(g).get_mut(remain_walls.wrapping_sub(i) as usize) {
                w.wall_exist = true;
                w.wall_x = wall_x;
                w.wall_y = y;
            }
        }

        remain_walls = remain_walls.wrapping_sub(wall_height);

        if remain_walls == 0 {
            break;
        }
    }
}

#[doc(alias = "DE_generateRings")]
pub fn de_generate_rings(g: &mut Game, screen: ScreenId, pixel: u8) {
    let rings = (mt_rand(g) % 6) as u32 + 1;
    for _ in 1..=rings {
        let temp_pos_x1 = (mt_rand(g) % 320) as i32;
        let temp_pos_y1 = (mt_rand(g) % 160) as i32 + 20;
        let temp_size = (mt_rand(g) % 40) as u32 + 10;

        for _ in 1..=temp_size * temp_size * 2 {
            let temp_radian = (f64::from(mt_rand_lt1(g)) * (2.0 * PI)) as f32;
            let temp_pos_y2 = (temp_pos_y1 as f32
                + (libm::cosf(temp_radian) * (mt_rand_lt1(g) * 0.1f32 + 0.9f32) * temp_size as f32)
                    .round()) as i32;
            let temp_pos_x2 = (temp_pos_x1 as f32
                + (libm::sinf(temp_radian) * (mt_rand_lt1(g) * 0.1f32 + 0.9f32) * temp_size as f32)
                    .round()) as i32;
            if (temp_pos_y2 > 12) && (temp_pos_y2 < 200) && (temp_pos_x2 > 0) && (temp_pos_x2 < 319)
            {
                let s = g.video.screen_mut(screen);
                let i = temp_pos_x2 + temp_pos_y2 * s.pitch;
                s.pixels[i as usize] = pixel;
            }
        }
    }
}

/// `aliasDirtPixel(screen, x, y, s)`: `screen` gives the bounds, `s` is
/// `buf[at..]`.
#[doc(alias = "aliasDirtPixel")]
pub fn alias_dirt_pixel(h: i32, pitch: i32, x: u32, y: u32, buf: &[u8], at: usize) -> u32 {
    let mut new_color = PIXEL_BLACK;
    let pitch_u = pitch as usize;

    if (y > 0) && (u32::from(buf[at - pitch_u]) == PIXEL_DIRT) {
        new_color += 1;
    }
    if (y < (h as u32).wrapping_sub(1)) && (u32::from(buf[at + pitch_u]) == PIXEL_DIRT) {
        new_color += 3;
    }
    if (x > 0) && (u32::from(buf[at - 1]) == PIXEL_DIRT) {
        new_color += 2;
    }
    if (x < (pitch as u32).wrapping_sub(1)) && (u32::from(buf[at + 1]) == PIXEL_DIRT) {
        new_color += 2;
    }
    if new_color != PIXEL_BLACK {
        return new_color + 16;
    }

    PIXEL_BLACK
}

#[doc(alias = "JE_aliasDirt")]
pub fn je_alias_dirt(g: &mut Game, screen: ScreenId) {
    let s = g.video.screen_mut(screen);
    let (h, pitch) = (s.h, s.pitch);
    let mut at = 12 * pitch as usize;

    for y in 12..h as u32 {
        for x in 0..pitch as u32 {
            if u32::from(s.pixels[at]) == PIXEL_BLACK {
                s.pixels[at] = alias_dirt_pixel(h, pitch, x, y, &s.pixels, at) as u8;
            }

            at += 1;
        }
    }
}

/// `JE_placementPosition(passed_x, width, world.baseMap)` (both callers'
/// array). Columns past the 320 of `baseMap` are skipped (C reads and writes
/// beyond the array there: divergence D-UB-DESTRUCT-1).
#[doc(alias = "JE_placementPosition")]
pub fn je_placement_position(g: &mut Game, passed_x: u32, width: u32) -> u32 {
    let world = &mut g.destruct.world.base_map;
    let last = passed_x.wrapping_add(width).wrapping_sub(1);

    let mut new_y = 0;
    let mut i = passed_x;
    while i <= last {
        if let Some(&h) = world.get(i as usize)
            && new_y < h
        {
            new_y = h;
        }
        i += 1;
    }

    let mut i = passed_x;
    while i <= last {
        if let Some(h) = world.get_mut(i as usize) {
            *h = new_y;
        }
        i += 1;
    }

    new_y
}

#[doc(alias = "JE_stabilityCheck")]
pub fn je_stability_check(g: &Game, x: u32, y: u32) -> bool {
    let s = g.video.screen(temp_screen(g));
    let at = x
        .wrapping_add(y.wrapping_mul(s.pitch as u32))
        .wrapping_sub(1) as usize;

    let mut num_dirt_pixels = 0;
    for at in at..at + 12 {
        if u32::from(s.pixels[at]) == PIXEL_DIRT {
            num_dirt_pixels += 1;
        }
    }

    num_dirt_pixels < 10
}

/// Fades explosions in `destructTempScreen` and copies it to `VGAScreen`.
#[doc(alias = "JE_tempScreenChecking")]
pub fn je_temp_screen_checking(g: &mut Game) {
    let alwaysalias = g.destruct.config.alwaysalias;
    let temp = temp_screen(g);
    let (vs, ts) = g.video.screen_pair_mut(vga(g), temp);
    let (h, pitch) = (vs.h, vs.pitch);

    let mut s = 12 * vs.pitch as usize;
    let mut temps = 12 * ts.pitch as usize;

    for y in 12..h {
        for x in 0..pitch {
            if ts.pixels[temps] >= 241 {
                if ts.pixels[temps] == 241 {
                    ts.pixels[temps] = PIXEL_BLACK as u8;
                } else {
                    ts.pixels[temps] -= 1;
                }
            }

            if alwaysalias && u32::from(ts.pixels[temps]) == PIXEL_BLACK {
                ts.pixels[temps] =
                    alias_dirt_pixel(h, pitch, x as u32, y as u32, &ts.pixels, temps) as u8;
            }

            vs.pixels[s] = ts.pixels[temps];

            s += 1;
            temps += 1;
        }
    }
}

#[doc(alias = "JE_makeExplosion")]
pub fn je_make_explosion(g: &mut Game, temp_pos_x: u32, temp_pos_y: u32, shottype: DeShotT) {
    let max = g.destruct.config.max_explosions as usize;
    let mut i = 0;
    while i < max {
        if explos(g)[i].is_available {
            break;
        }
        i += 1;
    }
    if i == max {
        return;
    }

    let e = &mut explos(g)[i];
    e.is_available = false;
    e.x = temp_pos_x;
    e.y = temp_pos_y;
    e.explowidth = 2;

    if shottype != SHOT_INVALID {
        let temp_explo_size = EXPLO_SIZE[shottype as usize] as u32;
        if temp_explo_size < 5 {
            je_e_sound(g, 3);
        } else if temp_explo_size < 15 {
            je_e_sound(g, 4);
        } else if temp_explo_size < 20 {
            je_e_sound(g, 12);
        } else if temp_explo_size < 40 {
            je_e_sound(g, 11);
        } else {
            je_e_sound(g, 12);
            je_e_sound(g, 11);
        }

        let e = &mut explos(g)[i];
        e.explomax = temp_explo_size;
        e.explofill = EXPLO_DENSITY[shottype as usize] as u32;
        e.explo_type = SHOT_DIRT[shottype as usize] as u32;
    } else {
        je_e_sound(g, 4);
        let explomax = (mt_rand(g) % 40) as u32 + 10;
        explos(g)[i].explomax = explomax;
        let explofill = (mt_rand(g) % 60) as u32 + 20;
        explos(g)[i].explofill = explofill;
        explos(g)[i].explo_type = EXPL_NORMAL;
    }
}

#[doc(alias = "JE_eSound")]
pub fn je_e_sound(g: &mut Game, sound: u32) {
    let channel = &mut g.destruct.je_e_sound_explo_sound_channel;
    *channel += 1;
    if *channel > 5 {
        *channel = 1;
    }

    g.varz.sound_queue[*channel as usize] = sound as u8;
}

const STAR_PATTERN: [[u32; 5]; 5] = [
    [0, 0, 246, 0, 0],
    [0, 247, 249, 247, 0],
    [246, 249, 252, 249, 246],
    [0, 247, 249, 247, 0],
    [0, 0, 246, 0, 0],
];
const STAR_INTENSITY: [[u32; 5]; 5] = [
    [0, 0, 1, 0, 0],
    [0, 1, 2, 1, 0],
    [1, 2, 4, 2, 1],
    [0, 1, 2, 1, 0],
    [0, 0, 1, 0, 0],
];

#[doc(alias = "JE_superPixel")]
pub fn je_super_pixel(g: &mut Game, temp_pos_x: u32, temp_pos_y: u32) {
    let surface = g.video.screen_mut(temp_screen_of(&g.destruct));
    let max_x = surface.pitch;
    let max_y = surface.h;

    let row_len = surface.pitch as u32;
    let mut s = row_len
        .wrapping_mul(temp_pos_y.wrapping_sub(2))
        .wrapping_add(temp_pos_x.wrapping_sub(2)) as usize;

    for y in 0..5i32 {
        if (temp_pos_y as i32) + y - 2 < 0 || (temp_pos_y as i32) + y - 2 >= max_y {
            s = s.wrapping_add((row_len - 5) as usize);
            continue;
        }

        for x in 0..5i32 {
            if (temp_pos_x as i32) + x - 2 < 0 || (temp_pos_x as i32) + x - 2 >= max_x {
                s = s.wrapping_add(1);
                continue;
            }

            let (yu, xu) = (y as usize, x as usize);
            if STAR_PATTERN[yu][xu] == 0 {
                s = s.wrapping_add(1);
                continue;
            }

            let p = &mut surface.pixels[s];
            if u32::from(*p) < STAR_PATTERN[yu][xu] {
                *p = STAR_PATTERN[yu][xu] as u8;
            } else if u32::from(*p) + STAR_INTENSITY[yu][xu] > 255 {
                *p = 255;
            } else {
                *p += STAR_INTENSITY[yu][xu] as u8;
            }
            s = s.wrapping_add(1);
        }
        s = s.wrapping_add((row_len - 5) as usize);
    }
}

fn temp_screen_of(s: &State) -> ScreenId {
    s.destruct_temp_screen
        .expect("destructTempScreen is set by JE_destructGame")
}

#[doc(alias = "JE_helpScreen")]
pub fn je_help_screen(g: &mut Game) -> Flow {
    fade_black(g, 15)?;
    save_vga(g);
    je_clr256(g, vga(g));

    for i in 0..2u32 {
        let help = g.helptext.destruct_help;
        je_out_text(
            g,
            vga(g),
            100,
            5 + i as i32 * 90,
            &help[(i * 12) as usize],
            2,
            4,
        );
        je_out_text(
            g,
            vga(g),
            100,
            15 + i as i32 * 90,
            &help[(i * 12 + 1) as usize],
            2,
            1,
        );
        for j in 3..=12u32 {
            je_out_text(
                g,
                vga(g),
                (((j - 1) % 2) * 160 + 10) as i32,
                (15 + ((j - 1) / 2) * 12 + i * 90) as i32,
                &help[(i * 12 + j - 1) as usize],
                1,
                3,
            );
        }
    }
    let help = g.helptext.destruct_help[24];
    je_out_text(g, vga(g), 30, 190, &help, 3, 4);
    je_show_vga(g);
    let colors = g.palette.colors;
    fade_palette(g, &colors, 15, 0, 255)?;

    wait_for_any_key(g)?;

    fade_black(g, 15)?;
    restore_vga(g);
    je_show_vga(g);
    let colors = g.palette.colors;
    fade_palette(g, &colors, 15, 0, 255)
}

#[doc(alias = "JE_pauseScreen")]
pub fn je_pause_screen(g: &mut Game) -> Flow {
    set_volume(
        g,
        (g.nortsong.tyr_music_volume / 2) as u8,
        g.nortsong.fx_volume as u8,
    );

    save_vga(g);
    let misc = g.helptext.misc_text[22];
    out_text_centered(g, 90, &misc, 12, 5);
    je_show_vga(g);

    wait_for_any_key(g)?;

    restore_vga(g);
    je_show_vga(g);

    set_volume(
        g,
        g.nortsong.tyr_music_volume as u8,
        g.nortsong.fx_volume as u8,
    );
    Ok(())
}

#[doc(alias = "DE_ResetUnits")]
pub fn de_reset_units(g: &mut Game) {
    for p in 0..MAX_PLAYERS {
        for u in 0..g.destruct.config.max_installations as usize {
            unit_mut(g, p, u).health = 0;
        }
    }
}

#[doc(alias = "DE_ResetPlayers")]
pub fn de_reset_players(g: &mut Game) {
    let s = &mut g.destruct;
    for i in 0..MAX_PLAYERS {
        let p = &mut s.destruct_player[i];
        p.is_cpu = false;
        p.unit_selected = 0;
        p.shot_delay = 0;
        p.score = 0;
        p.ai_memory.c_angle = 0;
        p.ai_memory.c_power = 0;
        p.ai_memory.c_fire = 0;
        p.ai_memory.c_no_down = 0;
        p.keys.config = s.default_key_config[i];
    }
}

#[doc(alias = "DE_ResetWeapons")]
pub fn de_reset_weapons(g: &mut Game) {
    for i in 0..g.destruct.config.max_shots as usize {
        shots(g)[i].is_available = true;
    }

    for i in 0..g.destruct.config.max_explosions as usize {
        explos(g)[i].is_available = true;
    }
}

#[doc(alias = "DE_ResetLevel")]
pub fn de_reset_level(g: &mut Game) -> Flow {
    de_reset_weapons(g);

    je_generate_terrain(g)?;
    de_reset_ai(g);
    Ok(())
}

#[doc(alias = "DE_ResetAI")]
pub fn de_reset_ai(g: &mut Game) {
    let walls_flag = g.destruct.world.map_flags & MAP_WALLS != 0;
    for i in PLAYER_LEFT as usize..MAX_PLAYERS {
        if !g.destruct.destruct_player[i].is_cpu {
            continue;
        }

        for j in 0..g.destruct.config.max_installations as usize {
            let ptr = unit_mut(g, i, j);
            if !de_is_valid_unit(ptr) {
                continue;
            }

            if SYSTEM_ANGLE[ptr.unit_type as usize] != 0 || ptr.unit_type == UNIT_HELI {
                ptr.angle = FRAC_PI_4 as f32;
            } else {
                ptr.angle = 0.0;
            }

            ptr.power = if ptr.unit_type == UNIT_LASER {
                6.0
            } else {
                4.0
            };

            if walls_flag {
                ptr.shot_type = DEFAULT_CPU_WEAPON_B[ptr.unit_type as usize];
            } else {
                ptr.shot_type = DEFAULT_CPU_WEAPON[ptr.unit_type as usize];
            }
        }
    }
}

#[doc(alias = "DE_ResetActions")]
pub fn de_reset_actions(g: &mut Game) {
    for i in 0..MAX_PLAYERS {
        g.destruct.destruct_player[i].moves = DestructMovesS::default();
    }
}

#[doc(alias = "DE_RunTick")]
pub fn de_run_tick(g: &mut Game) -> Flow<DeStateT> {
    set_frame_count(g, 1);

    g.varz.sound_queue = [0; 8];
    je_temp_screen_checking(g);

    de_reset_actions(g);
    de_run_tick_cycle_dead_units(g);

    de_run_tick_gravity(g);
    de_run_tick_animate(g);
    de_run_tick_draw_walls(g);
    de_run_tick_explosions(g);
    de_run_tick_shots(g);
    de_run_tick_ai(g);
    de_run_tick_draw_crosshairs(g);
    de_run_tick_draw_hud(g);
    je_show_vga(g);

    if g.destruct.destruct_first_time {
        let colors = g.palette.colors;
        fade_palette(g, &colors, 25, 0, 255)?;
        g.destruct.destruct_first_time = false;
        g.destruct.de_run_tick_end_delay = 0;
    }

    de_run_tick_get_input(g)?;
    de_process_input(g);

    if g.destruct.de_run_tick_end_delay > 0 {
        g.destruct.de_run_tick_end_delay -= 1;
        if g.destruct.de_run_tick_end_delay == 0 {
            return Ok(STATE_RELOAD);
        }
    } else if de_run_tick_check_endgame(g) {
        g.destruct.de_run_tick_end_delay = 80;
    }

    de_run_tick_play_sounds(g);

    delay_until_elapsed(g);

    keyboard_clear_input(g);

    let keys = &mut g.keyboard.keysactive;
    if keys[SDL_SCANCODE_F10] {
        let p = &mut g.destruct.destruct_player[PLAYER_LEFT as usize];
        p.is_cpu = !p.is_cpu;
        keys[SDL_SCANCODE_F10] = false;
    }
    if keys[SDL_SCANCODE_F11] {
        let p = &mut g.destruct.destruct_player[PLAYER_RIGHT as usize];
        p.is_cpu = !p.is_cpu;
        keys[SDL_SCANCODE_F11] = false;
    }
    if g.keyboard.keysactive[SDL_SCANCODE_P] {
        je_pause_screen(g)?;
        g.keyboard.keysactive[SDL_SCANCODE_P] = false;
    }

    if g.keyboard.keysactive[SDL_SCANCODE_F1] {
        je_help_screen(g)?;
        g.keyboard.keysactive[SDL_SCANCODE_F1] = false;
    }

    if g.keyboard.keysactive[SDL_SCANCODE_ESCAPE] {
        g.keyboard.keysactive[SDL_SCANCODE_ESCAPE] = false;
        return Ok(STATE_INIT);
    }

    if g.keyboard.keysactive[SDL_SCANCODE_BACKSPACE] {
        g.keyboard.keysactive[SDL_SCANCODE_BACKSPACE] = false;
        return Ok(STATE_RELOAD);
    }

    Ok(STATE_CONTINUE)
}

#[doc(alias = "DE_RunTickCycleDeadUnits")]
pub fn de_run_tick_cycle_dead_units(g: &mut Game) {
    let max = g.destruct.config.max_installations;
    for i in 0..MAX_PLAYERS {
        if g.destruct.destruct_player[i].units_remaining == 0 {
            continue;
        }

        let mut unit = g.destruct.destruct_player[i].unit_selected as usize;
        while !de_is_valid_unit(&units(g, i)[unit]) || units(g, i)[unit].shot_type == SHOT_INVALID {
            g.destruct.destruct_player[i].unit_selected += 1;
            unit += 1;
            if g.destruct.destruct_player[i].unit_selected >= max {
                g.destruct.destruct_player[i].unit_selected = 0;
                unit = 0;
            }
        }
    }
}

#[doc(alias = "DE_RunTickGravity")]
pub fn de_run_tick_gravity(g: &mut Game) {
    for i in 0..MAX_PLAYERS {
        for j in 0..g.destruct.config.max_installations as usize {
            let unit = units(g, i)[j];
            if !de_is_valid_unit(&unit) {
                continue;
            }

            match unit.unit_type {
                UNIT_SATELLITE => {}
                UNIT_HELI | UNIT_JUMPER if unit.is_y_in_air => {
                    de_gravity_fly_unit(g, i, j);
                }
                _ => de_gravity_lower_unit(g, i, j),
            }

            de_gravity_draw_unit(g, i as DePlayerT, j);
        }
    }
}

#[doc(alias = "DE_GravityDrawUnit")]
pub fn de_gravity_draw_unit(g: &mut Game, team: DePlayerT, unit: usize) {
    let u = units(g, team as usize)[unit];
    let mut anim_index = u32::from(GRAPHIC_BASE[team as usize][u.unit_type as usize]) + u.ani_frame;
    if u.unit_type == UNIT_HELI {
        if u.last_move < -2 {
            anim_index += 5;
        } else if u.last_move > 2 {
            anim_index += 10;
        }
    } else {
        let f = ((f64::from(u.angle * 9.99f32) / PI) as f32).floor();
        anim_index = (anim_index as f32 + f) as u32;
    }

    blit_sprite2(
        g,
        vga(g),
        u.unit_x as i32,
        (u.unit_y.round() - 13.0) as i32,
        Sprite2Ref::DestructSpriteSheet,
        anim_index,
    );
}

#[doc(alias = "DE_GravityLowerUnit")]
pub fn de_gravity_lower_unit(g: &mut Game, p: usize, unit: usize) {
    let u = units(g, p)[unit];
    if u.unit_y < 199.0 && je_stability_check(g, u.unit_x, u.unit_y.round() as u32) {
        let u = unit_mut(g, p, unit);
        match u.unit_type {
            UNIT_HELI => {
                u.unit_y_mov = 1.5;
                u.unit_y += 0.2f32;
            }
            _ => u.unit_y += 1.0,
        }

        if u.unit_y > 199.0 {
            u.unit_y = 199.0;
        }
    }
}

#[doc(alias = "DE_GravityFlyUnit")]
pub fn de_gravity_fly_unit(g: &mut Game, p: usize, unit: usize) {
    let u = unit_mut(g, p, unit);
    if u.unit_y + u.unit_y_mov > 199.0 {
        u.unit_y = 199.0;
        u.unit_y_mov = 0.0;
        u.is_y_in_air = false;
        return;
    }

    u.unit_y += u.unit_y_mov;
    if u.unit_y < 24.0 {
        u.unit_y_mov = 0.0;
        u.unit_y = 24.0;
    }

    if u.unit_type == UNIT_HELI {
        u.unit_y_mov += 0.0001f32;
    } else {
        u.unit_y_mov += 0.03f32;
    }

    let (x, y) = (u.unit_x, u.unit_y.round() as u32);
    if !je_stability_check(g, x, y) {
        let u = unit_mut(g, p, unit);
        u.unit_y_mov = 0.0;
        u.is_y_in_air = false;
    }
}

#[doc(alias = "DE_RunTickAnimate")]
pub fn de_run_tick_animate(g: &mut Game) {
    for p in 0..MAX_PLAYERS {
        for u in 0..g.destruct.config.max_installations as usize {
            let ptr = unit_mut(g, p, u);
            if !de_is_valid_unit(ptr) {
                continue;
            }
            if SYSTEM_ANI[ptr.unit_type as usize] == 0 && ptr.ani_frame == 0 {
                continue;
            }

            ptr.ani_frame += 1;
            if ptr.ani_frame > 3 {
                ptr.ani_frame = 0;
            }
        }
    }
}

#[doc(alias = "DE_RunTickDrawWalls")]
pub fn de_run_tick_draw_walls(g: &mut Game) {
    for i in 0..g.destruct.config.max_walls as usize {
        let w = walls(g)[i];
        if w.wall_exist {
            blit_sprite2(
                g,
                vga(g),
                w.wall_x as i32,
                w.wall_y as i32,
                Sprite2Ref::DestructSpriteSheet,
                42,
            );
        }
    }
}

#[doc(alias = "DE_RunTickExplosions")]
pub fn de_run_tick_explosions(g: &mut Game) {
    for i in 0..g.destruct.config.max_explosions as usize {
        if explos(g)[i].is_available {
            continue;
        }

        let mut j = 0;
        while j < explos(g)[i].explofill {
            j += 1;
            let temp_radian = (f64::from(mt_rand_lt1(g)) * (2.0 * PI)) as f32;
            let e = explos(g)[i];
            let temp_pos_y = (e.y as f32
                + (libm::cosf(temp_radian) * mt_rand_lt1(g) * e.explowidth as f32).round())
                as i32;
            let temp_pos_x_f = e.x as f32
                + (libm::sinf(temp_radian) * mt_rand_lt1(g) * e.explowidth as f32).round();
            let mut temp_pos_x = temp_pos_x_f as i32;

            while temp_pos_x < 0 {
                temp_pos_x += 320;
            }
            while temp_pos_x > 320 {
                temp_pos_x -= 320;
            }

            if temp_pos_y >= 200 || temp_pos_y <= 15 {
                continue;
            }

            match e.explo_type {
                EXPL_DIRT => {
                    let s = g.video.screen_mut(temp_screen_of(&g.destruct));
                    let at = (temp_pos_x + temp_pos_y * s.pitch) as usize;
                    // (320, 199) is one past the surface in C: divergence
                    // D-UB-DESTRUCT-3.
                    if let Some(p) = s.pixels.get_mut(at) {
                        *p = PIXEL_DIRT as u8;
                    }
                }
                EXPL_NORMAL => {
                    je_super_pixel(g, temp_pos_x as u32, temp_pos_y as u32);
                    de_test_explosion_collision(g, temp_pos_x as u32, temp_pos_y as u32);
                }
                _ => debug_assert!(false, "explosion type {}", e.explo_type),
            }
        }

        let e = &mut explos(g)[i];
        e.explowidth += 1;
        if e.explowidth == e.explomax {
            e.is_available = true;
        }
    }
}

#[doc(alias = "DE_TestExplosionCollision")]
pub fn de_test_explosion_collision(g: &mut Game, pos_x: u32, pos_y: u32) {
    for i in PLAYER_LEFT as usize..MAX_PLAYERS {
        for j in 0..g.destruct.config.max_installations as usize {
            let unit = unit_mut(g, i, j);
            if de_is_valid_unit(unit)
                && pos_x > unit.unit_x
                && pos_x < unit.unit_x.wrapping_add(11)
                && (pos_y as f32) < unit.unit_y
                && (pos_y as f32) > unit.unit_y - 11.0
            {
                unit.health -= 1;
                if unit.health <= 0 {
                    de_destroy_unit(g, i as DePlayerT, j);
                }
            }
        }
    }
}

#[doc(alias = "DE_DestroyUnit")]
pub fn de_destroy_unit(g: &mut Game, player_id: DePlayerT, unit: usize) {
    let u = units(g, player_id as usize)[unit];
    je_make_explosion(
        g,
        u.unit_x.wrapping_add(5),
        (u.unit_y.round() - 5.0) as u32,
        if u.unit_type == UNIT_HELI {
            SHOT_SMALL
        } else {
            SHOT_INVALID
        },
    );

    if u.unit_type != UNIT_SATELLITE {
        let players = &mut g.destruct.destruct_player;
        players[player_id as usize].units_remaining =
            players[player_id as usize].units_remaining.wrapping_sub(1);
        let other = if player_id == PLAYER_LEFT {
            PLAYER_RIGHT
        } else {
            PLAYER_LEFT
        };
        players[other as usize].score = players[other as usize].score.wrapping_add(1);
    }
}

#[doc(alias = "DE_RunTickShots")]
pub fn de_run_tick_shots(g: &mut Game) {
    for i in 0..g.destruct.config.max_shots as usize {
        if shots(g)[i].is_available {
            continue;
        }

        let s = &mut shots(g)[i];
        s.x += s.xmov;
        s.y += s.ymov;

        if SHOT_BOUNCE[s.shottype as usize] {
            if s.y > 199.0 || s.y < 14.0 {
                s.y -= s.ymov;
                s.ymov = -s.ymov;
            }
            if s.x < 1.0 || s.x > 318.0 {
                s.x -= s.xmov;
                s.xmov = -s.xmov;
            }
        } else {
            s.ymov += 0.05f32;

            if s.y > 199.0 {
                s.y -= s.ymov;
                s.ymov = -s.ymov * 0.8f32;

                if s.xmov == 0.0 {
                    let r = mt_rand_lt1(g) - 0.5f32;
                    shots(g)[i].xmov += r;
                }
            }
        }

        let s = &mut shots(g)[i];
        if s.x > 318.0 || s.x < 1.0 {
            s.is_available = true;
            continue;
        }

        if s.y <= 14.0 {
            continue;
        }

        let mut temp_pos_x = s.x.round() as u32;
        let mut temp_pos_y = s.y.round() as u32;
        let shottype = s.shottype;

        for j in 0..MAX_PLAYERS {
            for k in 0..g.destruct.config.max_installations as usize {
                let unit = units(g, j)[k];
                if !de_is_valid_unit(&unit) {
                    continue;
                }

                if temp_pos_x > unit.unit_x
                    && temp_pos_x < unit.unit_x.wrapping_add(11)
                    && (temp_pos_y as f32) < unit.unit_y
                    && (temp_pos_y as f32) > unit.unit_y - 13.0
                {
                    shots(g)[i].is_available = true;
                    je_make_explosion(g, temp_pos_x, temp_pos_y, shottype as DeShotT);
                }
            }
        }

        let temp_trails = ((SHOT_COLOR[shottype as usize] << 4) - 3) as u32;
        je_pix_cool(g, temp_pos_x, temp_pos_y, temp_trails as u8);

        match SHOT_TRAIL[shottype as usize] as u32 {
            TRAILS_NONE => {}
            TRAILS_NORMAL => de_draw_trails(g, i, 2, 4, temp_trails.wrapping_sub(3)),
            TRAILS_FULL => de_draw_trails(g, i, 4, 3, temp_trails.wrapping_sub(1)),
            _ => {}
        }

        for j in 0..g.destruct.config.max_walls as usize {
            let w = walls(g)[j];
            if w.wall_exist
                && temp_pos_x >= w.wall_x
                && temp_pos_x <= w.wall_x.wrapping_add(11)
                && temp_pos_y >= w.wall_y
                && temp_pos_y <= w.wall_y.wrapping_add(14)
            {
                if DEMOLISH[shottype as usize] {
                    walls_mut(g)[j].wall_exist = false;
                    shots(g)[i].is_available = true;
                    je_make_explosion(g, temp_pos_x, temp_pos_y, shottype as DeShotT);
                    continue;
                } else {
                    let s = &mut shots(g)[i];
                    if s.x - s.xmov < w.wall_x as f32
                        || s.x - s.xmov > w.wall_x.wrapping_add(11) as f32
                    {
                        s.xmov = -s.xmov;
                    }
                    if s.y - s.ymov < w.wall_y as f32
                        || s.y - s.ymov > w.wall_y.wrapping_add(14) as f32
                    {
                        if s.ymov < 0.0 {
                            s.ymov = -s.ymov;
                        } else {
                            s.ymov = -s.ymov * 0.8f32;
                        }
                    }

                    temp_pos_x = s.x.round() as u32;
                    temp_pos_y = s.y.round() as u32;
                }
            }
        }

        let t = g.video.screen(temp_screen_of(&g.destruct));
        let at = temp_pos_x.wrapping_add(temp_pos_y.wrapping_mul(t.pitch as u32)) as usize;
        // Row 200 is past the surface in C: divergence D-UB-DESTRUCT-3.
        if u32::from(t.pixels.get(at).copied().unwrap_or(0)) == PIXEL_DIRT {
            shots(g)[i].is_available = true;
            je_make_explosion(g, temp_pos_x, temp_pos_y, shottype as DeShotT);
            continue;
        }
    }
}

/// `DE_DrawTrails(&shotRec[shot], count, decay, startColor)`.
#[doc(alias = "DE_DrawTrails")]
pub fn de_draw_trails(g: &mut Game, shot: usize, count: u32, decay: u32, start_color: u32) {
    let mut i = count as i32 - 1;
    while i >= 0 {
        let iu = i as usize;
        let s = shots(g)[shot];
        if s.trailc[iu] > 0 && s.traily[iu] > 12 {
            je_pix_cool(g, s.trailx[iu], s.traily[iu], s.trailc[iu] as u8);
        }

        let s = &mut shots(g)[shot];
        if i == 0 {
            s.trailx[iu] = s.x.round() as u32;
            s.traily[iu] = s.y.round() as u32;
            s.trailc[iu] = start_color;
        } else {
            s.trailx[iu] = s.trailx[iu - 1];
            s.traily[iu] = s.traily[iu - 1];
            if s.trailc[iu - 1] > 0 {
                s.trailc[iu] = s.trailc[iu - 1].wrapping_sub(decay);
            }
        }
        i -= 1;
    }
}

#[doc(alias = "DE_RunTickAI")]
pub fn de_run_tick_ai(g: &mut Game) {
    let max = g.destruct.config.max_installations as usize;
    for i in 0..MAX_PLAYERS {
        if !g.destruct.destruct_player[i].is_cpu {
            continue;
        }

        let mut j = i + 1;
        if j >= MAX_PLAYERS {
            j = 0;
        }

        let target = j;
        let cur = g.destruct.destruct_player[i].unit_selected as usize;

        let ai = &mut g.destruct.destruct_player[i].ai_memory;
        if ai.c_no_down > 0 {
            ai.c_no_down -= 1;
        }

        if mt_rand(g) % 100 > 80 {
            let r = (mt_rand(g) % 3).wrapping_sub(1);
            let ai = &mut g.destruct.destruct_player[i].ai_memory;
            ai.c_angle = (ai.c_angle as u64).wrapping_add(r) as i32;

            ai.c_angle = ai.c_angle.clamp(-1, 1);
        }
        if mt_rand(g) % 100 > 90 {
            let angle = f64::from(units(g, i)[cur].angle);
            let ai = &mut g.destruct.destruct_player[i].ai_memory;
            if (ai.c_angle > 0 && angle > FRAC_PI_2 - (PI / 9.0))
                || (ai.c_angle < 0 && angle < PI / 8.0)
            {
                ai.c_angle = 0;
            }
        }

        if mt_rand(g) % 100 > 93 {
            let r = (mt_rand(g) % 3).wrapping_sub(1);
            let ai = &mut g.destruct.destruct_player[i].ai_memory;
            ai.c_power = (ai.c_power as u64).wrapping_add(r) as i32;

            ai.c_power = ai.c_power.clamp(-1, 1);
        }
        if mt_rand(g) % 100 > 90 {
            let power = units(g, i)[cur].power;
            let ai = &mut g.destruct.destruct_player[i].ai_memory;
            if (ai.c_power > 0 && power > 4.0) || (ai.c_power < 0 && power < 3.0) {
                ai.c_power = 0;
            } else if power < 2.0 {
                ai.c_power = 1;
            }
        }

        for j in 0..max {
            let u = units(g, i)[j];
            if de_is_valid_unit(&u) && u.unit_type == UNIT_HELI {
                g.destruct.destruct_player[i].unit_selected = j as u32;
                break;
            }
        }

        let p = i;
        if units(g, p)[cur].unit_type == UNIT_HELI {
            let c = units(g, p)[cur];
            if !c.is_y_in_air {
                g.destruct.destruct_player[p].ai_memory.c_power = 1;
            }
            // `% unitX` with unitX 0: divergence D-UB-DESTRUCT-5.
            if rem_or_dividend(mt_rand(g), u64::from(c.unit_x)) > 100 {
                g.destruct.destruct_player[p].ai_memory.c_power = 1;
            }
            if mt_rand(g) % 240 > u64::from(c.unit_x) {
                g.destruct.destruct_player[p].moves.actions[MOVE_RIGHT as usize] = true;
            } else if (mt_rand(g) % 20) + 300 < u64::from(c.unit_x) {
                g.destruct.destruct_player[p].moves.actions[MOVE_LEFT as usize] = true;
            } else if mt_rand(g) % 30 == 1 {
                let r = (mt_rand(g) % 3).wrapping_sub(1);
                g.destruct.destruct_player[p].ai_memory.c_angle = r as i32;
            }
            let c = units(g, p)[cur];
            if c.unit_x > 295 && c.last_move > 1 {
                let m = &mut g.destruct.destruct_player[p].moves.actions;
                m[MOVE_LEFT as usize] = true;
                m[MOVE_RIGHT as usize] = false;
            }
            if c.unit_type != UNIT_HELI || c.last_move > 3 || (c.unit_x > 160 && c.last_move > -3) {
                let y = c.unit_y.round() as i32;
                if rem_or_dividend(mt_rand(g), y as u64) < 150
                    && c.unit_y_mov < 0.01f32
                    && (c.unit_x < 160 || c.last_move < 2)
                {
                    g.destruct.destruct_player[p].moves.actions[MOVE_FIRE as usize] = true;
                }
                let d = 5 - c.last_move.wrapping_abs();
                let ai = &mut g.destruct.destruct_player[p].ai_memory;
                ai.c_no_down = d.wrapping_mul(d).wrapping_add(3) as u32;
                ai.c_power = 1;
            } else {
                g.destruct.destruct_player[p].moves.actions[MOVE_FIRE as usize] = false;
            }

            for j in 0..max {
                let ptr_unit = units(g, target)[j];
                let cur_x = units(g, p)[cur].unit_x;
                if (ptr_unit.unit_x as i32)
                    .wrapping_sub(cur_x as i32)
                    .wrapping_abs()
                    < 8
                {
                    if ptr_unit.unit_type == UNIT_SATELLITE {
                        g.destruct.destruct_player[p].moves.actions[MOVE_FIRE as usize] = false;
                    } else {
                        let m = &mut g.destruct.destruct_player[p].moves.actions;
                        m[MOVE_LEFT as usize] = false;
                        m[MOVE_RIGHT as usize] = false;
                        let c = unit_mut(g, p, cur);
                        if c.last_move < -1 {
                            c.last_move += 1;
                        } else if c.last_move > 1 {
                            c.last_move -= 1;
                        }
                    }
                }
            }
        } else {
            g.destruct.destruct_player[p].moves.actions[MOVE_FIRE as usize] = true;
        }

        if mt_rand(g) % 200 > 198 {
            let pl = &mut g.destruct.destruct_player[p];
            pl.moves.actions[MOVE_CHANGE as usize] = true;
            pl.ai_memory.c_angle = 0;
            pl.ai_memory.c_power = 0;
            pl.ai_memory.c_fire = 0;
        }

        if mt_rand(g) % 100 > 98 || units(g, p)[cur].shot_type == SHOT_TRACER {
            g.destruct.destruct_player[p].moves.actions[MOVE_CYDN as usize] = true;
        }
        let c = units(g, p)[cur];
        let pl = &mut g.destruct.destruct_player[p];
        if pl.ai_memory.c_angle > 0 {
            pl.moves.actions[MOVE_LEFT as usize] = true;
        }
        if pl.ai_memory.c_angle < 0 {
            pl.moves.actions[MOVE_RIGHT as usize] = true;
        }
        if pl.ai_memory.c_power > 0 {
            pl.moves.actions[MOVE_UP as usize] = true;
        }
        if pl.ai_memory.c_power < 0 && pl.ai_memory.c_no_down == 0 {
            pl.moves.actions[MOVE_DOWN as usize] = true;
        }
        if pl.ai_memory.c_fire > 0 {
            pl.moves.actions[MOVE_FIRE as usize] = true;
        }

        if c.unit_y_mov < -0.1f32 && c.unit_type == UNIT_HELI {
            pl.moves.actions[MOVE_FIRE as usize] = false;
        }

        if c.unit_type == UNIT_LASER || c.is_y_in_air {
            pl.ai_memory.c_power = 0;
        }
    }
}

#[doc(alias = "DE_RunTickDrawCrosshairs")]
pub fn de_run_tick_draw_crosshairs(g: &mut Game) {
    for i in 0..MAX_PLAYERS {
        let direction: i32 = if i == PLAYER_LEFT as usize { -1 } else { 1 };
        let sel = g.destruct.destruct_player[i].unit_selected as usize;
        let cur = units(g, i)[sel];

        let (temp_pos_x, temp_pos_y): (i32, i32);
        if cur.unit_type == UNIT_HELI {
            let lm = cur.last_move as f32;
            temp_pos_x = (cur.unit_x as f32 + (0.1f32 * lm * lm * lm).round() + 5.0) as i32;
            temp_pos_y = (cur.unit_y.round() + 1.0) as i32;
        } else {
            temp_pos_x = (cur.unit_x.wrapping_add(6) as f32
                - libm::cosf(cur.angle) * (cur.power * 8.0 + 7.0) * direction as f32)
                .round() as i32;
            temp_pos_y =
                (cur.unit_y - 7.0 - libm::sinf(cur.angle) * (cur.power * 8.0 + 7.0)).round() as i32;
        }

        let v = vga(g);
        if temp_pos_y > 9 {
            if temp_pos_y > 11 {
                if temp_pos_y > 13 {
                    je_pix(g, v, temp_pos_x, temp_pos_y - 2, 3);
                }
                je_pix(g, v, temp_pos_x + 3, temp_pos_y, 3);
                je_pix(g, v, temp_pos_x, temp_pos_y, 14);
                je_pix(g, v, temp_pos_x - 3, temp_pos_y, 3);
            }
            je_pix(g, v, temp_pos_x, temp_pos_y + 2, 3);
        }
    }
}

#[doc(alias = "DE_RunTickDrawHUD")]
pub fn de_run_tick_draw_hud(g: &mut Game) {
    for i in 0..MAX_PLAYERS {
        let sel = g.destruct.destruct_player[i].unit_selected as usize;
        let cur = units(g, i)[sel];
        let start_x: i32 = if i == PLAYER_LEFT as usize {
            0
        } else {
            320 - 150
        };
        let v = vga(g);

        fill_rectangle_xy(g, v, start_x + 5, 3, start_x + 14, 8, 241);
        je_rectangle(g, v, start_x + 4, 2, start_x + 15, 9, 242);
        je_rectangle(g, v, start_x + 3, 1, start_x + 16, 10, 240);
        fill_rectangle_xy(g, v, start_x + 18, 3, start_x + 140, 8, 241);
        je_rectangle(g, v, start_x + 17, 2, start_x + 143, 9, 242);
        je_rectangle(g, v, start_x + 16, 1, start_x + 144, 10, 240);

        blit_sprite2(
            g,
            v,
            start_x + 4,
            0,
            Sprite2Ref::DestructSpriteSheet,
            (191 + cur.shot_type) as u32,
        );

        // `weaponNames[SHOT_INVALID]` (a team of only satellites) reads before
        // the array in C; Rust prints nothing (divergence D-UB-DESTRUCT-4).
        let name = usize::try_from(cur.shot_type)
            .ok()
            .and_then(|t| g.helptext.weapon_names.get(t).copied())
            .unwrap_or([0; 17]);
        je_out_text(g, v, start_x + 20, 3, &name, 15, 2);
        let tempstr = format!("dmg~{}~", cur.health);
        je_out_text(g, v, start_x + 75, 3, tempstr.as_bytes(), 15, 0);
        let tempstr = format!("pts~{}~", g.destruct.destruct_player[i].score as i32);
        je_out_text(g, v, start_x + 110, 3, tempstr.as_bytes(), 15, 0);
    }
}

#[doc(alias = "DE_RunTickGetInput")]
pub fn de_run_tick_get_input(g: &mut Game) -> Flow {
    handle_sdl_events(g)?;

    for player_index in 0..MAX_PLAYERS {
        for key_index in 0..MAX_KEY {
            for slot_index in 0..MAX_KEY_OPTIONS {
                let key =
                    g.destruct.destruct_player[player_index].keys.config[key_index][slot_index];
                if key == SDL_SCANCODE_UNKNOWN {
                    break;
                }
                if g.keyboard.keysactive[key as usize] {
                    g.destruct.destruct_player[player_index].moves.actions[key_index] = true;

                    if key_index == KEY_CHANGE as usize
                        || key_index == KEY_CYUP as usize
                        || key_index == KEY_CYDN as usize
                    {
                        g.keyboard.keysactive[key as usize] = false;
                    }
                    break;
                }
            }
        }
    }
    Ok(())
}

#[doc(alias = "DE_ProcessInput")]
pub fn de_process_input(g: &mut Game) {
    for player_index in 0..MAX_PLAYERS {
        if g.destruct.destruct_player[player_index].units_remaining == 0 {
            continue;
        }

        let direction: i32 = if player_index == PLAYER_LEFT as usize {
            -1
        } else {
            1
        };
        let cur = g.destruct.destruct_player[player_index].unit_selected as usize;
        let actions = |g: &Game, m: DeMoveT| {
            g.destruct.destruct_player[player_index].moves.actions[m as usize]
        };
        let unit_type = units(g, player_index)[cur].unit_type;

        if SYSTEM_ANGLE[unit_type as usize] == 1 {
            if actions(g, MOVE_LEFT) {
                if player_index == PLAYER_LEFT as usize {
                    de_raise_angle(unit_mut(g, player_index, cur));
                } else {
                    de_lower_angle(unit_mut(g, player_index, cur));
                }
            }
            if actions(g, MOVE_RIGHT) {
                if player_index == PLAYER_LEFT as usize {
                    de_lower_angle(unit_mut(g, player_index, cur));
                } else {
                    de_raise_angle(unit_mut(g, player_index, cur));
                }
            }
        } else if unit_type == UNIT_HELI {
            let u = units(g, player_index)[cur];
            if actions(g, MOVE_LEFT)
                && u.unit_x > 5
                && je_stability_check(g, u.unit_x - 5, u.unit_y.round() as u32)
            {
                let u = unit_mut(g, player_index, cur);
                if u.last_move > -5 {
                    u.last_move -= 1;
                }
                u.unit_x -= 1;
                let (x, y) = (u.unit_x, u.unit_y.round() as u32);
                if je_stability_check(g, x, y) {
                    unit_mut(g, player_index, cur).is_y_in_air = true;
                }
            }
            let u = units(g, player_index)[cur];
            if actions(g, MOVE_RIGHT)
                && u.unit_x < 305
                && je_stability_check(g, u.unit_x + 5, u.unit_y.round() as u32)
            {
                let u = unit_mut(g, player_index, cur);
                if u.last_move < 5 {
                    u.last_move += 1;
                }
                u.unit_x += 1;
                let (x, y) = (u.unit_x, u.unit_y.round() as u32);
                if je_stability_check(g, x, y) {
                    unit_mut(g, player_index, cur).is_y_in_air = true;
                }
            }
        }

        if unit_type != UNIT_LASER {
            if actions(g, MOVE_UP) {
                let u = unit_mut(g, player_index, cur);
                if u.unit_type == UNIT_HELI {
                    u.is_y_in_air = true;
                    u.unit_y_mov -= 0.1f32;
                } else if u.unit_type == UNIT_JUMPER && !u.is_y_in_air {
                    u.unit_y_mov = -3.0;
                    u.is_y_in_air = true;
                } else {
                    de_raise_power(u);
                }
            }
            if actions(g, MOVE_DOWN) {
                let u = unit_mut(g, player_index, cur);
                if u.unit_type == UNIT_HELI && u.is_y_in_air {
                    u.unit_y_mov += 0.1f32;
                } else {
                    de_lower_power(u);
                }
            }
        }

        if actions(g, MOVE_CYUP) {
            de_cycle_weapon_up(g, player_index, cur);
        }
        if actions(g, MOVE_CYDN) {
            de_cycle_weapon_down(g, player_index, cur);
        }

        if actions(g, MOVE_CHANGE) {
            let max = g.destruct.config.max_installations;
            let pl = &mut g.destruct.destruct_player[player_index];
            pl.unit_selected += 1;
            if pl.unit_selected >= max {
                pl.unit_selected = 0;
            }
        }

        let pl = &mut g.destruct.destruct_player[player_index];
        if pl.shot_delay > 0 {
            pl.shot_delay -= 1;
        }
        if actions(g, MOVE_FIRE) && g.destruct.destruct_player[player_index].shot_delay == 0 {
            let shot_type = units(g, player_index)[cur].shot_type as usize;
            g.destruct.destruct_player[player_index].shot_delay = SHOT_DELAY[shot_type] as u32;

            match SHOT_DIRT[shot_type] as u32 {
                EXPL_NONE => {}

                EXPL_MAGNET => de_run_magnet(g, player_index as DePlayerT, cur),

                EXPL_DIRT | EXPL_NORMAL => {
                    de_make_shot(g, player_index as DePlayerT, cur, direction);
                }

                _ => debug_assert!(false, "shot type {shot_type}"),
            }
        }
    }
}

#[doc(alias = "DE_CycleWeaponUp")]
pub fn de_cycle_weapon_up(g: &mut Game, p: usize, unit: usize) {
    let systems = g.destruct.weapon_systems;
    let u = unit_mut(g, p, unit);
    loop {
        u.shot_type += 1;
        if u.shot_type > SHOT_LAST {
            u.shot_type = SHOT_FIRST;
        }
        if systems[u.unit_type as usize][u.shot_type as usize] {
            break;
        }
    }
}

#[doc(alias = "DE_CycleWeaponDown")]
pub fn de_cycle_weapon_down(g: &mut Game, p: usize, unit: usize) {
    let systems = g.destruct.weapon_systems;
    let u = unit_mut(g, p, unit);
    loop {
        u.shot_type -= 1;
        if u.shot_type < SHOT_FIRST {
            u.shot_type = SHOT_LAST;
        }
        if systems[u.unit_type as usize][u.shot_type as usize] {
            break;
        }
    }
}

#[doc(alias = "DE_MakeShot")]
pub fn de_make_shot(g: &mut Game, cur_player: DePlayerT, cur_unit: usize, direction: i32) {
    let max = g.destruct.config.max_shots as usize;
    let mut i = 0;
    let shot_index = loop {
        if i >= max {
            return;
        }

        if shots(g)[i].is_available {
            break i;
        }
        i += 1;
    };

    let cu = units(g, cur_player as usize)[cur_unit];

    if cu.unit_type == UNIT_HELI && !cu.is_y_in_air {
        return;
    }

    g.varz.sound_queue[cur_player as usize] = SHOT_SOUND[cu.shot_type as usize] as u8;

    let move_up = g.destruct.destruct_player[cur_player as usize]
        .moves
        .actions[MOVE_UP as usize];
    let jumper_straight = g.destruct.config.jumper_straight[cur_player as usize];
    let dir = direction as f32;
    let s = &mut shots(g)[shot_index];

    let aimed = |s: &mut DestructShotS| {
        s.x = cu.unit_x.wrapping_add(6) as f32 - libm::cosf(cu.angle) * 10.0 * dir;
        s.y = cu.unit_y - 7.0 - libm::sinf(cu.angle) * 10.0;
        s.xmov = -libm::cosf(cu.angle) * cu.power * dir;
        s.ymov = -libm::sinf(cu.angle) * cu.power;
    };
    match cu.unit_type {
        UNIT_HELI => {
            s.x = cu
                .unit_x
                .wrapping_add((cu.last_move * 2) as u32)
                .wrapping_add(5) as f32;
            let lm = cu.last_move as f32;
            s.xmov = 0.02f32 * lm * lm * lm;

            if move_up && cu.unit_y < 30.0 {
                s.y = cu.unit_y;
                s.ymov = 0.1f32;

                if s.xmov < 0.0 {
                    s.xmov += 0.1f32;
                } else if s.xmov > 0.0 {
                    s.xmov -= 0.1f32;
                }
            } else {
                s.y = cu.unit_y + 1.0;
                s.ymov = 0.5f32 + cu.unit_y_mov * 0.1f32;
            }
        }

        UNIT_JUMPER => {
            if jumper_straight {
                aimed(s);
            } else {
                s.x = cu.unit_x.wrapping_add(2) as f32;
                s.xmov = -libm::cosf(cu.angle) * cu.power * dir;

                if cu.is_y_in_air {
                    s.ymov = 1.0;
                    s.y = cu.unit_y + 2.0;
                } else {
                    s.ymov = -2.0;
                    s.y = cu.unit_y - 12.0;
                }
            }
        }

        _ => aimed(s),
    }

    s.is_available = false;

    s.shottype = cu.shot_type as u32;

    s.trailc = [0; 4];
}

#[doc(alias = "DE_RunMagnet")]
pub fn de_run_magnet(g: &mut Game, cur_player: DePlayerT, magnet: usize) {
    let cur_enemy = if cur_player == PLAYER_LEFT {
        PLAYER_RIGHT
    } else {
        PLAYER_LEFT
    };
    let direction: i32 = if cur_player == PLAYER_LEFT { -1 } else { 1 };
    let m = units(g, cur_player as usize)[magnet];

    for i in 0..g.destruct.config.max_shots as usize {
        let s = &mut shots(g)[i];
        if !s.is_available
            && ((cur_player == PLAYER_LEFT && s.x > m.unit_x as f32)
                || (cur_player == PLAYER_RIGHT && s.x < m.unit_x as f32))
        {
            s.xmov += m.power * 0.1f32 * (-direction) as f32;
        }
    }

    for i in 0..g.destruct.config.max_installations as usize {
        let e = unit_mut(g, cur_enemy as usize, i);
        if de_is_valid_unit(e)
            && e.unit_type == UNIT_HELI
            && e.is_y_in_air
            && ((cur_enemy == PLAYER_RIGHT && e.unit_x.wrapping_add(11) < 318)
                || (cur_enemy == PLAYER_LEFT && e.unit_x > 1))
        {
            e.unit_x = e.unit_x.wrapping_sub((2 * direction) as u32);
        }
    }
    unit_mut(g, cur_player as usize, magnet).ani_frame = 1;
}

#[doc(alias = "DE_RaiseAngle")]
pub fn de_raise_angle(unit: &mut DestructUnitS) {
    unit.angle += 0.01f32;
    if f64::from(unit.angle) > FRAC_PI_2 - f64::from(0.01f32) {
        unit.angle = (FRAC_PI_2 - f64::from(0.01f32)) as f32;
    }
}

#[doc(alias = "DE_LowerAngle")]
pub fn de_lower_angle(unit: &mut DestructUnitS) {
    unit.angle -= 0.01f32;
    if unit.angle < 0.0 {
        unit.angle = 0.0;
    }
}

#[doc(alias = "DE_RaisePower")]
pub fn de_raise_power(unit: &mut DestructUnitS) {
    unit.power += 0.05f32;
    if unit.power > 5.0 {
        unit.power = 5.0;
    }
}

#[doc(alias = "DE_LowerPower")]
pub fn de_lower_power(unit: &mut DestructUnitS) {
    unit.power -= 0.05f32;
    if unit.power < 1.0 {
        unit.power = 1.0;
    }
}

/// True if the unit's health is above 0.
#[doc(alias = "DE_isValidUnit")]
pub fn de_is_valid_unit(unit: &DestructUnitS) -> bool {
    unit.health > 0
}

#[doc(alias = "DE_RunTickCheckEndgame")]
pub fn de_run_tick_check_endgame(g: &mut Game) -> bool {
    let mode = g.destruct.world.destruct_mode as usize;
    let p = &mut g.destruct.destruct_player;
    if p[PLAYER_LEFT as usize].units_remaining == 0 {
        p[PLAYER_RIGHT as usize].score = p[PLAYER_RIGHT as usize]
            .score
            .wrapping_add(u32::from(MODE_SCORE[PLAYER_LEFT as usize][mode]));
        g.varz.sound_queue[7] = V_CLEARED_PLATFORM;
        return true;
    }
    if p[PLAYER_RIGHT as usize].units_remaining == 0 {
        p[PLAYER_LEFT as usize].score = p[PLAYER_LEFT as usize]
            .score
            .wrapping_add(u32::from(MODE_SCORE[PLAYER_RIGHT as usize][mode]));
        g.varz.sound_queue[7] = V_CLEARED_PLATFORM;
        return true;
    }
    false
}

#[doc(alias = "DE_RunTickPlaySounds")]
pub fn de_run_tick_play_sounds(g: &mut Game) {
    for i in 0..g.varz.sound_queue.len() {
        if g.varz.sound_queue[i] != S_NONE {
            let temp_sample_index = usize::from(g.varz.sound_queue[i]);
            let temp_volume = if i == 7 { FX_PLAY_VOL } else { FX_PLAY_VOL / 2 };

            let k = temp_sample_index - 1;
            let samples = g.nortsong.sound_samples[k]
                .clone()
                .map(|buf| ChannelSamples {
                    sound: k,
                    buf,
                    pos: 0,
                });
            let count = g.nortsong.sound_sample_count[k];
            multi_sample_play(g, samples, count, i as u8, temp_volume as u8);
            g.varz.sound_queue[i] = S_NONE;
        }
    }
}

#[doc(alias = "JE_pixCool")]
pub fn je_pix_cool(g: &mut Game, x: u32, y: u32, c: u8) {
    let v = vga(g);
    je_pix(g, v, x as i32, y as i32, c);
    je_pix(g, v, x.wrapping_sub(1) as i32, y as i32, c.wrapping_sub(2));
    je_pix(g, v, x.wrapping_add(1) as i32, y as i32, c.wrapping_sub(2));
    je_pix(g, v, x as i32, y.wrapping_sub(1) as i32, c.wrapping_sub(2));
    je_pix(g, v, x as i32, y.wrapping_add(1) as i32, c.wrapping_sub(2));
}
