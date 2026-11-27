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

use crate::video::ScreenId;
use serde::{Deserialize, Serialize};

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
