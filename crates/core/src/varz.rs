// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `varz.c` / `varz.h`: shared in-game state.
//!
//! Pointer globals become indices (rule R8):
//! - `Sprite2_array *` → [`Sprite2Ref`], naming the sheet it points to.
//! - `void *enemydatofs` → index into `episodes` `enemyDat`.
//! - `JE_MapType*` entries (`JE_byte *` into `megaDataN.shapes[i].sh`) →
//!   `Option<u8>` shape index `i` (`None` is NULL).
//! - `BKwrapN`/`BKwrapNto` (`JE_byte **` into `megaDataN.mainmap`) → the
//!   element index into the flattened `mainmap` (`row * columns + column`).

use crate::lvlmast::PORT_NUM;
use serde::{Deserialize, Serialize};

pub const SA: usize = 7;
pub const SA_NONE: u8 = 0;
pub const SA_NORTSHIPZ: u8 = 7;
/// Only used for code entry.
pub const SA_DESTRUCT: u8 = 8;
/// Only used for code entry.
pub const SA_ENGAGE: u8 = 9;
/// Only used in `pItems[P_SUPERARCADE]`.
pub const SA_SUPERTYRIAN: u8 = 254;
pub const SA_ARCADE: u8 = 255;

pub const ENEMY_SHOT_MAX: usize = 60;
/// Keyboard/joystick movement rate.
pub const CURRENT_KEY_SPEED: i32 = 1;
pub const MAX_EXPLOSIONS: usize = 200;
pub const MAX_REPEATING_EXPLOSIONS: usize = 20;
pub const MAX_SUPERPIXELS: usize = 101;

/// A `Sprite2_array *` target (rule R8): the sheets in `sprite.c` plus
/// `editship.c`'s `extraShapes`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Sprite2Ref {
    ShopSpriteSheet,
    ExplosionSpriteSheet,
    /// `&enemySpriteSheets[i]`, `i < 4`.
    EnemySpriteSheets(u8),
    DestructSpriteSheet,
    SpriteSheet8,
    SpriteSheet9,
    SpriteSheet10,
    SpriteSheet11,
    SpriteSheet12,
    /// `&extraShapes` (`editship.c`).
    ExtraShapes,
}

/// `struct JE_SingleEnemyType`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JeSingleEnemyType {
    pub fillbyte: u8,
    pub ex: i16,
    pub ey: i16,
    pub exc: i8,
    pub eyc: i8,
    pub exca: i8,
    pub eyca: i8,
    pub excc: i8,
    pub eycc: i8,
    pub exccw: i8,
    pub eyccw: i8,
    pub armorleft: u8,
    pub eshotwait: [u8; 3],
    pub eshotmultipos: [u8; 3],
    pub enemycycle: u8,
    pub ani: u8,
    pub egr: [u16; 20],
    pub size: u8,
    pub linknum: u8,
    pub aniactive: u8,
    pub animax: u8,
    pub aniwhenfire: u8,
    /// `sprite2s`; `None` is NULL.
    pub sprite2s: Option<Sprite2Ref>,
    pub exrev: i8,
    pub eyrev: i8,
    pub exccadd: i16,
    pub eyccadd: i16,
    pub exccwmax: u8,
    pub eyccwmax: u8,
    /// `enemydatofs`: index into `enemyDat`; `None` is NULL.
    pub enemydatofs: Option<usize>,
    pub edamaged: bool,
    pub enemytype: u16,
    pub animin: u8,
    pub edgr: u16,
    pub edlevel: i8,
    pub edani: i8,
    pub fill1: u8,
    pub filter: u8,
    pub evalue: i16,
    pub fixedmovey: i16,
    pub freq: [u8; 3],
    pub launchwait: u8,
    pub launchtype: u16,
    pub launchfreq: u8,
    pub xaccel: u8,
    pub yaccel: u8,
    pub tur: [u8; 3],
    /// Enemy created when this one dies.
    pub enemydie: u16,
    pub enemyground: bool,
    pub explonum: u8,
    pub mapoffset: u16,
    pub scoreitem: bool,
    pub special: bool,
    pub flagnum: u8,
    pub setto: bool,
    /// Duration.
    pub iced: u8,
    pub launchspecial: u8,
    pub xminbounce: i16,
    pub xmaxbounce: i16,
    pub yminbounce: i16,
    pub ymaxbounce: i16,
    pub fill: [u8; 3],
}

/// `JE_MultiEnemyType`.
pub type JeMultiEnemyType = [JeSingleEnemyType; 100];
/// `JE_DanCShape`.
pub type JeDanCShape = [u8; 24 * 28];
/// `JE_EnemyAvailType`.
pub type JeEnemyAvailType = [u8; 100];
/// `JE_MapType` (entries: shape index, see the module docs).
pub type JeMapType = [[Option<u8>; 14]; 300];
/// `JE_MapType2`.
pub type JeMapType2 = [[Option<u8>; 14]; 600];
/// `JE_MapType3`.
pub type JeMapType3 = [[Option<u8>; 15]; 600];

/// `struct JE_EventRecType`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JeEventRecType {
    pub eventtime: u16,
    pub eventtype: u8,
    pub eventdat: i16,
    pub eventdat2: i16,
    pub eventdat3: i8,
    pub eventdat5: i8,
    pub eventdat6: i8,
    pub eventdat4: u8,
}

/// Element of `JE_MegaDataType1::shapes` (anonymous struct in C).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JeMegaDataType1Shape {
    #[serde(with = "crate::serde_big")]
    pub sh: JeDanCShape,
}

/// Element of `JE_MegaDataType2::shapes`/`JE_MegaDataType3::shapes`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JeMegaDataShape {
    pub nothing: [u8; 3],
    pub fill: u8,
    #[serde(with = "crate::serde_big")]
    pub sh: JeDanCShape,
}

impl Default for JeMegaDataType1Shape {
    fn default() -> Self {
        JeMegaDataType1Shape { sh: [0; 24 * 28] }
    }
}

impl Default for JeMegaDataShape {
    fn default() -> Self {
        JeMegaDataShape {
            nothing: [0; 3],
            fill: 0,
            sh: [0; 24 * 28],
        }
    }
}

fn boxed<T: Clone, const N: usize>(x: T) -> Box<[T; N]> {
    vec![x; N]
        .into_boxed_slice()
        .try_into()
        .unwrap_or_else(|_| unreachable!("length is N"))
}

/// `struct JE_MegaDataType1`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JeMegaDataType1 {
    #[serde(with = "crate::serde_big::d2_boxed")]
    pub mainmap: Box<JeMapType>,
    #[serde(with = "crate::serde_big::boxed")]
    pub shapes: Box<[JeMegaDataType1Shape; 72]>,
    pub tempdat1: u8,
}

/// `struct JE_MegaDataType2`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JeMegaDataType2 {
    #[serde(with = "crate::serde_big::d2_boxed")]
    pub mainmap: Box<JeMapType2>,
    #[serde(with = "crate::serde_big::boxed")]
    pub shapes: Box<[JeMegaDataShape; 71]>,
    pub tempdat2: u8,
}

/// `struct JE_MegaDataType3`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JeMegaDataType3 {
    #[serde(with = "crate::serde_big::d2_boxed")]
    pub mainmap: Box<JeMapType3>,
    #[serde(with = "crate::serde_big::boxed")]
    pub shapes: Box<[JeMegaDataShape; 70]>,
    pub tempdat3: u8,
}

impl Default for JeMegaDataType1 {
    fn default() -> Self {
        JeMegaDataType1 {
            mainmap: boxed([None; 14]),
            shapes: boxed(JeMegaDataType1Shape::default()),
            tempdat1: 0,
        }
    }
}

impl Default for JeMegaDataType2 {
    fn default() -> Self {
        JeMegaDataType2 {
            mainmap: boxed([None; 14]),
            shapes: boxed(JeMegaDataShape::default()),
            tempdat2: 0,
        }
    }
}

impl Default for JeMegaDataType3 {
    fn default() -> Self {
        JeMegaDataType3 {
            mainmap: boxed([None; 15]),
            shapes: boxed(JeMegaDataShape::default()),
            tempdat3: 0,
        }
    }
}

/// `EnemyShotType`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnemyShotType {
    pub sx: i16,
    pub sy: i16,
    pub sxm: i16,
    pub sym: i16,
    pub sxc: i8,
    pub syc: i8,
    pub tx: u8,
    pub ty: u8,
    pub sgr: u16,
    pub sdmg: u8,
    pub duration: u8,
    pub animate: u16,
    pub animax: u16,
    pub fill: [u8; 12],
}

/// `Explosion`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Explosion {
    pub ttl: u8,
    pub x: i16,
    pub y: i16,
    pub sprite: u16,
    pub follow_player: bool,
    pub fixed_position: bool,
    pub delta_y: i16,
}

/// `rep_explosion_type`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepExplosionType {
    pub delay: u32,
    pub ttl: u32,
    pub x: u32,
    pub y: u32,
    pub big: bool,
}

/// `superpixel_type`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuperpixelType {
    pub x: u32,
    pub y: u32,
    pub z: u32,
    pub delta_x: i32,
    pub delta_y: i32,
    pub color: u8,
}

/// `SANextShip`.
pub const SA_NEXT_SHIP: [u8; SA + 2] = [3, 9, 6, 2, 5, 1, 4, 3, 7];
/// `SASpecialWeapon`.
pub const SA_SPECIAL_WEAPON: [u16; SA] = [7, 8, 9, 10, 11, 12, 13];
/// `SASpecialWeaponB`.
pub const SA_SPECIAL_WEAPON_B: [u16; SA] = [37, 6, 15, 40, 16, 14, 41];
/// `SAShip`.
pub const SA_SHIP: [u8; SA] = [3, 1, 5, 10, 2, 11, 12];
/// `SAWeapon`.
pub const SA_WEAPON: [[u16; 5]; SA] = [
    [9, 31, 32, 33, 34],
    [19, 8, 22, 41, 34],
    [27, 5, 20, 42, 31],
    [15, 3, 28, 22, 12],
    [23, 35, 25, 14, 6],
    [2, 5, 21, 4, 7],
    [40, 38, 37, 41, 36],
];
/// `specialArcadeWeapon`.
pub const SPECIAL_ARCADE_WEAPON: [u8; PORT_NUM] = [
    17, 17, 18, 0, 0, 0, 10, 0, 0, 0, 0, 0, 44, 0, 10, 0, 19, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 45, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
/// `optionSelect`.
pub const OPTION_SELECT: [[[u8; 2]; 3]; 16] = [
    [[0, 0], [0, 0], [0, 0]],
    [[1, 1], [16, 16], [30, 30]],
    [[2, 2], [29, 29], [29, 20]],
    [[3, 3], [21, 21], [12, 0]],
    [[4, 4], [18, 18], [16, 23]],
    [[0, 0], [0, 0], [0, 0]],
    [[6, 6], [29, 16], [0, 22]],
    [[7, 7], [19, 19], [19, 28]],
    [[0, 0], [0, 0], [0, 0]],
    [[0, 0], [0, 0], [0, 0]],
    [[10, 10], [21, 21], [21, 27]],
    [[0, 0], [0, 0], [0, 0]],
    [[0, 0], [0, 0], [0, 0]],
    [[13, 13], [17, 17], [13, 26]],
    [[0, 0], [0, 0], [0, 0]],
    [[15, 15], [15, 16], [15, 16]],
];
/// `PGR`.
pub const PGR: [u16; 21] = [
    4,
    1,
    2,
    3,
    41 - 21,
    57 - 21,
    73 - 21,
    89 - 21,
    105 - 21,
    121 - 21,
    137 - 21,
    153 - 21,
    151,
    151,
    151,
    151,
    73 - 21,
    73 - 21,
    1,
    2,
    4,
];
/// `PAni`.
pub const P_ANI: [u8; 21] = [
    1, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1,
];
/// `linkGunWeapons`.
pub const LINK_GUN_WEAPONS: [u16; 38] = [
    0, 0, 0, 0, 0, 0, 0, 0, 444, 445, 446, 447, 0, 448, 449, 0, 0, 0, 0, 0, 450, 451, 0, 506, 0,
    564, 445, 446, 447, 448, 449, 445, 446, 447, 448, 449, 450, 451,
];
/// `chargeGunWeapons`.
pub const CHARGE_GUN_WEAPONS: [u16; 38] = [
    0, 0, 0, 0, 0, 0, 0, 0, 476, 458, 464, 482, 0, 488, 470, 0, 0, 0, 0, 0, 494, 500, 0, 528, 0,
    558, 458, 458, 458, 458, 458, 458, 458, 458, 458, 458, 458, 458,
];
/// `randomEnemyLaunchSounds`.
pub const RANDOM_ENEMY_LAUNCH_SOUNDS: [u8; 3] = [13, 6, 26];
/// `keyboardCombos`. Move codes: 1 up, 2 down, 3 left, 4 right, 5-8 the
/// same with fire, 9 release all; the code ends with the special number.
pub const KEYBOARD_COMBOS: [[u8; 8]; 26] = [
    [2, 1, 2, 5, 137, 0, 0, 0],
    [4, 3, 2, 5, 138, 0, 0, 0],
    [3, 4, 6, 139, 0, 0, 0, 0],
    [2, 5, 142, 0, 0, 0, 0, 0],
    [6, 2, 6, 143, 0, 0, 0, 0],
    [6, 7, 5, 8, 6, 7, 5, 112],
    [7, 8, 101, 0, 0, 0, 0, 0],
    [1, 7, 6, 146, 0, 0, 0, 0],
    [8, 6, 7, 1, 120, 0, 0, 0],
    [3, 6, 8, 5, 121, 0, 0, 0],
    [1, 2, 7, 8, 119, 0, 0, 0],
    [3, 4, 3, 6, 123, 0, 0, 0],
    [6, 7, 5, 8, 124, 0, 0, 0],
    [1, 6, 125, 0, 0, 0, 0, 0],
    [9, 5, 126, 0, 0, 0, 0, 0],
    [1, 7, 127, 0, 0, 0, 0, 0],
    [1, 8, 128, 0, 0, 0, 0, 0],
    [9, 7, 129, 0, 0, 0, 0, 0],
    [9, 8, 130, 0, 0, 0, 0, 0],
    [4, 2, 3, 5, 131, 0, 0, 0],
    [3, 1, 2, 8, 132, 0, 0, 0],
    [2, 4, 5, 133, 0, 0, 0, 0],
    [3, 4, 2, 8, 134, 0, 0, 0],
    [1, 4, 6, 135, 0, 0, 0, 0],
    [1, 3, 6, 137, 0, 0, 0, 0],
    [1, 4, 3, 4, 7, 136, 0, 0],
];
/// `shipCombosB`.
pub const SHIP_COMBOS_B: [u8; 21] = [
    15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 7, 8, 5, 25, 14, 4, 6, 3, 9, 2, 26,
];
/// `superTyrianSpecials`.
pub const SUPER_TYRIAN_SPECIALS: [u8; 4] = [1, 2, 4, 5];
/// `shipCombos` (the last row is a dummy entry).
pub const SHIP_COMBOS: [[u8; 3]; 14] = [
    [5, 4, 7],
    [1, 2, 0],
    [14, 4, 0],
    [4, 5, 0],
    [6, 5, 0],
    [7, 8, 0],
    [7, 9, 0],
    [10, 3, 5],
    [5, 8, 9],
    [1, 3, 0],
    [7, 16, 17],
    [2, 11, 12],
    [3, 8, 10],
    [0, 0, 0],
];
/// `shadowYDist`.
pub const SHADOW_Y_DIST: u32 = 10;
/// `hud_sidekick_y`: `[one player HUD, two player HUD]`.
pub const HUD_SIDEKICK_Y: [[i32; 2]; 2] = [[64, 82], [108, 126]];

/// Mutable globals of `varz.c` (rule R3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub temp_dat: i16,
    pub temp_dat2: i16,
    pub temp_dat3: i16,
    /// `SFCurrentCode`.
    pub sf_current_code: [[u8; 21]; 2],
    /// `SFExecuted`.
    pub sf_executed: [u8; 2],
    pub lvl_file_num: u8,
    pub max_event: u16,
    pub event_loc: u16,
    pub temp_back_move: u16,
    pub explode_move: u16,
    pub level_end: u8,
    pub level_end_fx_wait: u16,
    pub level_end_warp: i8,
    pub end_level: bool,
    pub really_end_level: bool,
    pub wait_to_end_level: bool,
    pub player_end_level: bool,
    pub normal_bonus_level_current: bool,
    pub bonus_level_current: bool,
    pub small_enemy_adjust: bool,
    pub ready_to_end_level: bool,
    pub quit_requested: bool,
    /// `newPL`.
    pub new_pl: [u8; 10],
    pub return_loc: u16,
    pub return_active: bool,
    pub galaga_shot_freq: u16,
    pub galaga_life: i32,
    pub debug: bool,
    pub debug_time: u32,
    pub last_debug_time: u32,
    pub debug_hist_count: u32,
    pub debug_hist: u32,
    /// Current pixel location of background 1.
    pub cur_loc: u16,
    pub first_game_over: bool,
    pub game_loaded: bool,
    pub enemy_still_exploding: bool,
    pub total_enemy: u16,
    pub enemy_killed: u16,
    pub mega_data1: JeMegaDataType1,
    pub mega_data2: JeMegaDataType2,
    pub mega_data3: JeMegaDataType3,
    pub flash: u8,
    pub flash_change: i8,
    pub display_time: u8,
    pub sound_queue: [u8; 8],
    pub enemy_continual_damage: bool,
    pub enemies_active: bool,
    pub force_events: bool,
    pub stop_backgrounds: bool,
    pub stop_background_num: u8,
    /// Rate at which a player takes damage.
    pub damage_rate: u8,
    /// Background 3 enemies use the background 1 X offset.
    pub background3x1: bool,
    /// Background 3 enemies moved 8 pixels left.
    pub background3x1b: bool,
    pub level_timer: bool,
    pub level_timer_countdown: u16,
    pub level_timer_jump_to: u16,
    pub random_explosions: bool,
    pub edit_ship1: bool,
    pub edit_ship2: bool,
    pub global_flags: [bool; 10],
    pub level_song: u8,
    pub load_destruct: bool,
    pub map_origin: u16,
    pub map_p_num: u16,
    pub map_planet: [u8; 5],
    pub map_section: [u8; 5],
    pub move_tyrian_logo_up: bool,
    pub skip_star_show_vga: bool,
    #[serde(with = "crate::serde_big::boxed")]
    pub enemy: Box<JeMultiEnemyType>,
    /// Values: 0 used, 1 free, 2 secret pick-up.
    #[serde(with = "crate::serde_big")]
    pub enemy_avail: JeEnemyAvailType,
    pub enemy_offset: u16,
    pub enemy_on_screen: u16,
    pub super_enemy254_jump: u16,
    pub fire_button_held: bool,
    #[serde(with = "crate::serde_big")]
    pub enemy_shot_avail: [bool; ENEMY_SHOT_MAX],
    #[serde(with = "crate::serde_big")]
    pub enemy_shot: [EnemyShotType; ENEMY_SHOT_MAX],
    pub zinglon_duration: u8,
    pub astral_duration: u8,
    pub flare_duration: u16,
    pub flare_start: bool,
    pub flare_col_chg: i8,
    pub special_wait: u8,
    pub next_special_wait: u8,
    pub spray_special: bool,
    pub do_iced: u8,
    pub infinite_shot: bool,
    /// Both players dead and finished exploding.
    pub all_players_gone: bool,
    pub option_satellite_rotate: f32,
    pub option_attachment_move: i16,
    pub option_attachment_linked: bool,
    pub option_attachment_return: bool,
    pub charge_wait: u8,
    pub charge_level: u8,
    pub charge_max: u8,
    pub charge_gr: u8,
    pub charge_gr_wait: u8,
    pub neat: u16,
    #[serde(with = "crate::serde_big")]
    pub explosions: [Explosion; MAX_EXPLOSIONS],
    pub explosion_follow_amount_x: i16,
    pub explosion_follow_amount_y: i16,
    pub rep_explosions: [RepExplosionType; MAX_REPEATING_EXPLOSIONS],
    #[serde(with = "crate::serde_big")]
    pub superpixels: [SuperpixelType; MAX_SUPERPIXELS],
    pub last_superpixel: u32,
    pub temp: u8,
    pub temp2: u8,
    pub temp3: u8,
    pub temp_w: u16,
    pub do_not_save_backup: bool,
    pub x: u16,
    pub y: u16,
    pub b: i16,
    /// `BKwrap1to` (index into `mega_data1.mainmap`, see module docs).
    pub bkwrap1to: Option<isize>,
    /// `BKwrap2to` (into `mega_data2.mainmap`).
    pub bkwrap2to: Option<isize>,
    /// `BKwrap3to` (into `mega_data3.mainmap`).
    pub bkwrap3to: Option<isize>,
    /// `BKwrap1` (into `mega_data1.mainmap`).
    pub bkwrap1: Option<isize>,
    /// `BKwrap2` (into `mega_data2.mainmap`).
    pub bkwrap2: Option<isize>,
    /// `BKwrap3` (into `mega_data3.mainmap`).
    pub bkwrap3: Option<isize>,
    pub special_weapon_filter: i8,
    pub special_weapon_freq: i8,
    pub special_weapon_wpn: u16,
    pub link_to_player: bool,
    pub ship_gr: u16,
    pub ship_gr2: u16,
    /// `shipGrPtr`; `None` is NULL.
    pub ship_gr_ptr: Option<Sprite2Ref>,
    /// `shipGr2ptr`; `None` is NULL.
    pub ship_gr2ptr: Option<Sprite2Ref>,
}

impl Default for State {
    fn default() -> Self {
        State {
            temp_dat: 0,
            temp_dat2: 0,
            temp_dat3: 0,
            sf_current_code: [[0; 21]; 2],
            sf_executed: [0; 2],
            lvl_file_num: 0,
            max_event: 0,
            event_loc: 0,
            temp_back_move: 0,
            explode_move: 0,
            level_end: 0,
            level_end_fx_wait: 0,
            level_end_warp: 0,
            end_level: false,
            really_end_level: false,
            wait_to_end_level: false,
            player_end_level: false,
            normal_bonus_level_current: false,
            bonus_level_current: false,
            small_enemy_adjust: false,
            ready_to_end_level: false,
            quit_requested: false,
            new_pl: [0; 10],
            return_loc: 0,
            return_active: false,
            galaga_shot_freq: 0,
            galaga_life: 0,
            debug: false,
            debug_time: 0,
            last_debug_time: 0,
            debug_hist_count: 0,
            debug_hist: 0,
            cur_loc: 0,
            first_game_over: false,
            game_loaded: false,
            enemy_still_exploding: false,
            total_enemy: 0,
            enemy_killed: 0,
            mega_data1: JeMegaDataType1::default(),
            mega_data2: JeMegaDataType2::default(),
            mega_data3: JeMegaDataType3::default(),
            flash: 0,
            flash_change: 0,
            display_time: 0,
            sound_queue: [0; 8],
            enemy_continual_damage: false,
            enemies_active: false,
            force_events: false,
            stop_backgrounds: false,
            stop_background_num: 0,
            damage_rate: 0,
            background3x1: false,
            background3x1b: false,
            level_timer: false,
            level_timer_countdown: 0,
            level_timer_jump_to: 0,
            random_explosions: false,
            edit_ship1: false,
            edit_ship2: false,
            global_flags: [false; 10],
            level_song: 0,
            load_destruct: false,
            map_origin: 0,
            map_p_num: 0,
            map_planet: [0; 5],
            map_section: [0; 5],
            move_tyrian_logo_up: false,
            skip_star_show_vga: false,
            enemy: boxed(JeSingleEnemyType::default()),
            enemy_avail: [0; 100],
            enemy_offset: 0,
            enemy_on_screen: 0,
            super_enemy254_jump: 0,
            fire_button_held: false,
            enemy_shot_avail: [false; ENEMY_SHOT_MAX],
            enemy_shot: [EnemyShotType::default(); ENEMY_SHOT_MAX],
            zinglon_duration: 0,
            astral_duration: 0,
            flare_duration: 0,
            flare_start: false,
            flare_col_chg: 0,
            special_wait: 0,
            next_special_wait: 0,
            spray_special: false,
            do_iced: 0,
            infinite_shot: false,
            all_players_gone: false,
            option_satellite_rotate: 0.0,
            option_attachment_move: 0,
            option_attachment_linked: false,
            option_attachment_return: false,
            charge_wait: 0,
            charge_level: 0,
            charge_max: 0,
            charge_gr: 0,
            charge_gr_wait: 0,
            neat: 0,
            explosions: [Explosion::default(); MAX_EXPLOSIONS],
            explosion_follow_amount_x: 0,
            explosion_follow_amount_y: 0,
            rep_explosions: [RepExplosionType::default(); MAX_REPEATING_EXPLOSIONS],
            superpixels: [SuperpixelType::default(); MAX_SUPERPIXELS],
            last_superpixel: 0,
            temp: 0,
            temp2: 0,
            temp3: 0,
            temp_w: 0,
            do_not_save_backup: false,
            x: 0,
            y: 0,
            b: 0,
            bkwrap1to: None,
            bkwrap2to: None,
            bkwrap3to: None,
            bkwrap1: None,
            bkwrap2: None,
            bkwrap3: None,
            special_weapon_filter: 0,
            special_weapon_freq: 0,
            special_weapon_wpn: 0,
            link_to_player: false,
            ship_gr: 0,
            ship_gr2: 0,
            ship_gr_ptr: None,
            ship_gr2ptr: None,
        }
    }
}
