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

use crate::Game;
use crate::config::{SHOT_SPECIAL, SHOT_SPECIAL2};
use crate::episodes::{JeOptionType, JePower, JeShip, JeSpecial, JeWeaponPort};
use crate::game::{Flow, Halt};
use crate::lvlmast::PORT_NUM;
use crate::player::{FRONT_WEAPON, LEFT_SIDEKICK, REAR_WEAPON, RIGHT_SIDEKICK};
use crate::shots::MAX_PWEAPON;
use crate::sndmast::{S_EXPLOSION_22, S_HULL_HIT, S_POWERUP, S_SHIELD_HIT, S_SOUL_OF_ZINGLON};
use crate::sprite::OPTION_SHAPES;
use crate::video::ScreenId;
use crate::{config, joystick, loudness, mtrand, nortvars, shots, sprite, vga256d, video};
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

/// Shuts the subsystems down, writes the configuration and the saves, and
/// exits with `code` (5 becomes 0).
#[doc(alias = "JE_tyrianHalt")]
pub fn je_tyrian_halt(g: &mut Game, code: u8) -> Flow {
    loudness::deinit_audio(g);
    video::deinit_video(g);
    joystick::deinit_joysticks(g);

    sprite::free_main_shape_tables(g);

    sprite::free_sprite2s(&mut g.sprite.shop_sprite_sheet);
    sprite::free_sprite2s(&mut g.sprite.explosion_sprite_sheet);
    sprite::free_sprite2s(&mut g.sprite.destruct_sprite_sheet);

    g.nortsong.sound_samples.fill(None);

    config::save_configuration(g);
    config::save_saves(g);

    let code = if code == 5 { 0 } else { code };

    if g.config.trent_win {
        g.platform.log("");
        g.platform.log("Sleep well, Trent, you deserve the rest.");
        g.platform
            .log("You now have permission to borrow my ship on your next mission.");
        g.platform.log("");
    }

    Err(Halt {
        code: i32::from(code),
    })
}

// Item-table reads guarded per divergence D-UB-VARZ-2: an index outside its
// table reads a zeroed entry.

fn power_sys(g: &Game, i: u8) -> JePower {
    g.episodes
        .power_sys
        .get(usize::from(i))
        .copied()
        .unwrap_or_default()
}

fn ship(g: &Game, i: u8) -> JeShip {
    g.episodes
        .ships
        .get(usize::from(i))
        .copied()
        .unwrap_or_default()
}

fn option(g: &Game, i: u8) -> JeOptionType {
    g.episodes
        .options
        .get(usize::from(i))
        .copied()
        .unwrap_or_default()
}

fn special(g: &Game, i: u8) -> JeSpecial {
    g.episodes
        .special
        .get(usize::from(i))
        .copied()
        .unwrap_or_default()
}

fn weapon_port(g: &Game, i: u8) -> JeWeaponPort {
    g.episodes
        .weapon_port
        .get(usize::from(i))
        .copied()
        .unwrap_or_default()
}

fn extra_ship(g: &Game, i: usize) -> u8 {
    g.editship.extra_ships.get(i).copied().unwrap_or(0)
}

/// `VGAScreenSeg`.
fn seg(g: &Game) -> ScreenId {
    g.video
        .vga_screen_seg
        .expect("VGAScreenSeg used before init_video")
}

#[doc(alias = "JE_getShipInfo")]
pub fn je_get_ship_info(g: &mut Game) {
    g.varz.ship_gr_ptr = Some(Sprite2Ref::SpriteSheet9);
    g.varz.ship_gr2ptr = Some(Sprite2Ref::SpriteSheet9);

    g.config.power_add = u32::from(power_sys(g, g.player.player[0].items.generator).power);

    let ship0 = g.player.player[0].items.ship;
    let extra_ship1 = ship0 > 90;
    if extra_ship1 {
        let base = ((i32::from(ship0) - 91) * 15) as u8;
        let mut ptr = g.varz.ship_gr_ptr;
        g.varz.ship_gr = je_s_gr(g, u16::from(ship0) - 90, &mut ptr);
        g.varz.ship_gr_ptr = ptr;
        g.player.player[0].armor = u32::from(extra_ship(g, usize::from(base) + 7));
    } else {
        g.varz.ship_gr = ship(g, ship0).shipgraphic;
        g.player.player[0].armor = u32::from(ship(g, ship0).dmg);
    }

    let ship1 = g.player.player[1].items.ship;
    let extra_ship2 = ship1 > 90;
    if extra_ship2 {
        let base2 = ((i32::from(ship1) - 91) * 15) as u8;
        let mut ptr = g.varz.ship_gr2ptr;
        g.varz.ship_gr2 = je_s_gr(g, u16::from(ship1) - 90, &mut ptr);
        g.varz.ship_gr2ptr = ptr;
        g.player.player[1].armor = u32::from(extra_ship(g, usize::from(base2) + 7));
    } else {
        g.varz.ship_gr2 = 0;
        g.player.player[1].armor = 10;
    }

    for i in 0..g.player.player.len() {
        let p = &g.player.player[i];
        let temp: u32 = if (i == 0 && extra_ship1) || (i == 1 && extra_ship2) {
            2
        } else {
            u32::from(ship(g, p.items.ship).ani)
        };

        let p = &mut g.player.player[i];
        p.initial_armor = p.armor;
        if temp == 0 {
            p.shot_hit_area_x = 12;
            p.shot_hit_area_y = 10;
        } else {
            p.shot_hit_area_x = 11;
            p.shot_hit_area_y = 14;
        }
    }
}

/// `JE_SGr(ship, &ptr)`: the graphic of custom ship `ship` (1-based); points
/// `ptr` at `extraShapes` for the custom graphics. A graphic number outside
/// `1..=15` returns 0 (divergence D-UB-VARZ-2).
#[doc(alias = "JE_SGr")]
pub fn je_s_gr(g: &mut Game, ship: u16, ptr: &mut Option<Sprite2Ref>) -> u16 {
    const GR: [u16; 15] = [
        233, 157, 195, 271, 81, 0, 119, 5, 43, 81, 119, 157, 195, 233, 271,
    ];

    let temp_w = u16::from(extra_ship(
        g,
        usize::from(ship).wrapping_sub(1).wrapping_mul(15),
    ));
    if temp_w > 7 {
        *ptr = Some(Sprite2Ref::ExtraShapes);
    }

    usize::from(temp_w)
        .checked_sub(1)
        .and_then(|i| GR.get(i))
        .copied()
        .unwrap_or(0)
}

#[doc(alias = "JE_drawOptions")]
pub fn je_draw_options(g: &mut Game) {
    let temp_surface = g.video.vga_screen;
    g.video.vga_screen = g.video.vga_screen_seg;

    let tpm = usize::from(g.config.two_player_mode);

    #[allow(clippy::needless_range_loop)] // `i` indexes the sidekicks and the HUD rows
    for i in 0..2 {
        let this_option = option(g, g.player.player[tpm].items.sidekick[i]);
        let s = &mut g.player.player[tpm].sidekick[i];

        s.ammo = i32::from(this_option.ammo);
        s.ammo_max = s.ammo;

        s.ammo_refill_ticks = ((105 - s.ammo) * 4) as u32;
        s.ammo_refill_ticks_max = s.ammo_refill_ticks;

        s.style = u32::from(this_option.tr);

        s.animation_enabled = this_option.option == 1;
        s.animation_frame = 0;

        s.charge = 0;
        s.charge_ticks = 20;
        let (ammo_max, ammo) = (s.ammo_max, s.ammo);

        // the initial sidekick HUD
        let y = HUD_SIDEKICK_Y[tpm][i];
        let surface = seg(g);

        vga256d::fill_rectangle_xy(g, surface, 284, y, 284 + 28, y + 15, 0);
        if this_option.icongr > 0 {
            sprite::blit_sprite(
                g,
                surface,
                284,
                y,
                OPTION_SHAPES as u32,
                u32::from(this_option.icongr) - 1,
            );
        }
        vga256d::draw_segmented_gauge(
            g,
            surface,
            284,
            y + 13,
            112,
            2,
            2,
            (ammo_max / 10).max(1) as u32,
            ammo as u32,
        );
    }

    g.video.vga_screen = temp_surface;

    je_draw_option_level(g);
}

#[doc(alias = "JE_drawOptionLevel")]
pub fn je_draw_option_level(g: &mut Game) {
    if g.config.two_player_mode {
        g.varz.temp = 1;
        while g.varz.temp <= 3 {
            let temp = i32::from(g.varz.temp);
            let lit = i32::from(g.player.player[1].items.sidekick_level) - 100 == temp;
            let surface = seg(g);
            vga256d::fill_rectangle_xy(
                g,
                surface,
                268,
                127 + (temp - 1) * 6,
                269,
                127 + 3 + (temp - 1) * 6,
                (193 + i32::from(lit) * 11) as u8,
            );
            g.varz.temp += 1;
        }
    }
}

/// `JE_specialComplete(playerNum, specialType)`.
#[doc(alias = "JE_specialComplete")]
pub fn je_special_complete(g: &mut Game, player_num: u8, special_type: u8) {
    g.varz.next_special_wait = 0;
    let sp = special(g, special_type);
    let front_power = u16::from(g.player.player[0].items.weapon[FRONT_WEAPON].power);
    match sp.stype {
        // Weapon
        1 => {
            let shooter = if player_num == 1 { 0 } else { 1 };
            let (px, py) = (g.player.player[shooter].x, g.player.player[shooter].y);
            let (mx, my) = (g.player.player[0].mouse_x, g.player.player[0].mouse_y);
            g.varz.b = shots::player_shot_create(
                g,
                0,
                SHOT_SPECIAL2 as u32,
                px as u16,
                py as u16,
                mx,
                my,
                sp.wpn,
                player_num,
            );

            g.config.shot_repeat[SHOT_SPECIAL] = g.config.shot_repeat[SHOT_SPECIAL2];
        }
        // Repulsor
        2 => {
            let (px, py) = (g.player.player[0].x, g.player.player[0].y);
            g.varz.temp = 0;
            while usize::from(g.varz.temp) < ENEMY_SHOT_MAX {
                let t = usize::from(g.varz.temp);
                if !g.varz.enemy_shot_avail[t] {
                    let s = &mut g.varz.enemy_shot[t];
                    if px > i32::from(s.sx) {
                        s.sxm = s.sxm.wrapping_sub(1);
                    } else if px < i32::from(s.sx) {
                        s.sxm = s.sxm.wrapping_add(1);
                    }

                    if py > i32::from(s.sy) {
                        s.sym = s.sym.wrapping_sub(1);
                    } else if py < i32::from(s.sy) {
                        s.sym = s.sym.wrapping_add(1);
                    }
                }
                g.varz.temp += 1;
            }
        }
        // Zinglon Blast
        3 => {
            g.varz.zinglon_duration = 50;
            g.config.shot_repeat[SHOT_SPECIAL] = 100;
            g.varz.sound_queue[7] = S_SOUL_OF_ZINGLON;
        }
        // Attractor
        4 => {
            let (px, py) = (g.player.player[0].x, g.player.player[0].y);
            g.varz.temp = 0;
            while g.varz.temp < 100 {
                let t = usize::from(g.varz.temp);
                let e = &mut g.varz.enemy[t];
                if g.varz.enemy_avail[t] != 1 && e.scoreitem && e.evalue != 0 {
                    if px > i32::from(e.ex) {
                        e.exc = e.exc.wrapping_add(1);
                    } else if px < i32::from(e.ex) {
                        e.exc = e.exc.wrapping_sub(1);
                    }

                    if py > i32::from(e.ey) {
                        e.eyc = e.eyc.wrapping_add(1);
                    } else if py < i32::from(e.ey) {
                        e.eyc = e.eyc.wrapping_sub(1);
                    }
                }
                g.varz.temp += 1;
            }
        }
        // Flare
        5..=11 | 16 => {
            let v = &mut g.varz;
            if v.flare_duration == 0 {
                v.flare_start = true;
            }

            v.special_weapon_wpn = sp.wpn;
            v.link_to_player = false;
            v.spray_special = false;
            match sp.stype {
                5 => {
                    v.special_weapon_filter = 7;
                    v.special_weapon_freq = 2;
                    v.flare_duration = 50;
                }
                6 => {
                    v.special_weapon_filter = 1;
                    v.special_weapon_freq = 7;
                    v.flare_duration = 200u16.wrapping_add(25 * front_power);
                }
                7 => {
                    v.special_weapon_filter = 3;
                    v.special_weapon_freq = 3;
                    v.flare_duration = 50u16.wrapping_add(10 * front_power);
                    v.zinglon_duration = 50;
                    g.config.shot_repeat[SHOT_SPECIAL] = 100;
                    v.sound_queue[7] = S_SOUL_OF_ZINGLON;
                }
                8 => {
                    v.special_weapon_filter = -99;
                    v.special_weapon_freq = 7;
                    v.flare_duration = 10 + front_power;
                }
                9 => {
                    v.special_weapon_filter = -99;
                    v.special_weapon_freq = 8;
                    v.flare_duration = 8 + 2 * front_power;
                    v.link_to_player = true;
                    v.next_special_wait = sp.pwr;
                }
                10 => {
                    v.special_weapon_filter = -99;
                    v.special_weapon_freq = 8;
                    v.flare_duration = 14 + 4 * front_power;
                    v.link_to_player = true;
                }
                11 => {
                    v.special_weapon_filter = -99;
                    v.special_weapon_freq = sp.pwr as i8;
                    v.flare_duration = 10u16.wrapping_add(10 * front_power);
                    v.astral_duration = (20 + 10 * front_power) as u8;
                }
                _ => {
                    // 16
                    v.special_weapon_filter = -99;
                    v.special_weapon_freq = 8;
                    v.flare_duration = u16::from(v.temp2) * 16 + 8;
                    v.link_to_player = true;
                    v.spray_special = true;
                }
            }
        }
        // Invulnerability
        12 => {
            g.player.player[usize::from(player_num) - 1].invulnerable_ticks =
                u32::from(g.varz.temp2) * 10;

            let sam = g.config.super_arcade_mode;
            if sam > 0 && usize::from(sam) <= SA {
                g.config.shot_repeat[SHOT_SPECIAL] = 250;
                let p = &g.player.player[0];
                let (px, py, mx, my) = (p.x as u16, p.y as u16, p.mouse_x, p.mouse_y);
                g.varz.b =
                    shots::player_shot_create(g, 0, SHOT_SPECIAL2 as u32, px, py, mx, my, 707, 1);
                g.player.player[0].invulnerable_ticks = 100;
            }
        }
        // Repair Player 1
        13 => {
            let a = &mut g.player.player[0].armor;
            *a = a.wrapping_add(u32::from(g.varz.temp2) / 4 + 1);

            g.varz.sound_queue[3] = S_POWERUP;
        }
        // Repair Player 2
        14 => {
            let a = &mut g.player.player[1].armor;
            *a = a.wrapping_add(u32::from(g.varz.temp2) / 4 + 1);

            g.varz.sound_queue[3] = S_POWERUP;
        }
        // Spawn left or right sidekick
        17 => {
            g.varz.sound_queue[3] = S_POWERUP;

            let items = &mut g.player.player[0].items;
            if u16::from(items.sidekick[LEFT_SIDEKICK]) == sp.wpn {
                items.sidekick[RIGHT_SIDEKICK] = sp.wpn as u8;
                g.config.shot_multi_pos[RIGHT_SIDEKICK] = 0;
            } else {
                items.sidekick[LEFT_SIDEKICK] = sp.wpn as u8;
                g.config.shot_multi_pos[LEFT_SIDEKICK] = 0;
            }

            je_draw_options(g);
        }
        // Spawn right sidekick
        18 => {
            g.player.player[0].items.sidekick[RIGHT_SIDEKICK] = sp.wpn as u8;

            je_draw_options(g);

            g.varz.sound_queue[4] = S_POWERUP;

            g.config.shot_multi_pos[RIGHT_SIDEKICK] = 0;
        }
        _ => {}
    }
}

/// `JE_doSpecialShot(playerNum, &armor, &shield)`; the only caller passes
/// the armor and shield of one player, `player[p]` here (rule R8).
#[doc(alias = "JE_doSpecialShot")]
pub fn je_do_special_shot(g: &mut Game, player_num: u8, p: usize) {
    if g.player.player[0].items.special > 0 {
        let ready = g.config.shot_repeat[SHOT_SPECIAL] == 0
            && g.varz.special_wait == 0
            && g.varz.flare_duration < 2
            && g.varz.zinglon_duration < 2;
        let surface = g.video.vga_screen_id();
        let index = if ready { 94 } else { 93 };
        sprite::blit_sprite2(g, surface, 47, 4, Sprite2Ref::SpriteSheet9, index);
    }

    if g.config.shot_repeat[SHOT_SPECIAL] > 0 {
        g.config.shot_repeat[SHOT_SPECIAL] -= 1;
    }
    if g.varz.special_wait > 0 {
        g.varz.special_wait -= 1;
    }
    g.varz.temp = g.varz.sf_executed[usize::from(player_num) - 1];
    if g.varz.temp > 0 && g.config.shot_repeat[SHOT_SPECIAL] == 0 && g.varz.flare_duration == 0 {
        g.varz.temp2 = special(g, g.varz.temp).pwr;

        let mut can_afford = true;
        let temp2 = g.varz.temp2;
        let pl = &mut g.player.player[p];

        if temp2 > 0 {
            if temp2 < 98 {
                // costs some shield
                if pl.shield >= u32::from(temp2) {
                    pl.shield -= u32::from(temp2);
                } else {
                    can_afford = false;
                }
            } else if temp2 == 98 {
                // costs all shield
                if pl.shield < 4 {
                    can_afford = false;
                }
                g.varz.temp2 = pl.shield as u8;
                pl.shield = 0;
            } else if temp2 == 99 {
                // costs half shield
                g.varz.temp2 = (pl.shield / 2) as u8;
                pl.shield = u32::from(g.varz.temp2);
            } else {
                // costs some armor
                g.varz.temp2 = temp2 - 100;
                if pl.armor > u32::from(g.varz.temp2) {
                    pl.armor -= u32::from(g.varz.temp2);
                } else {
                    can_afford = false;
                }
            }
        }

        g.config.shot_multi_pos[SHOT_SPECIAL] = 0;
        g.config.shot_multi_pos[SHOT_SPECIAL2] = 0;

        if can_afford {
            let t = g.varz.temp;
            je_special_complete(g, player_num, t);
        }

        g.varz.sf_executed[usize::from(player_num) - 1] = 0;

        je_wipe_shield_armor_bars(g);
        g.video.vga_screen = g.video.vga_screen_seg; // side-effect of game_screen
        je_draw_shield(g);
        je_draw_armor(g);
        g.video.vga_screen = g.video.game_screen; // side-effect of game_screen
    }

    if player_num == 1 && g.player.player[0].items.special > 0 {
        let button = g.mainint.button;
        let sam = g.config.super_arcade_mode;
        if sam > 0 && (button[1] || button[2]) {
            g.varz.fire_button_held = false;
        }
        if !button[0] && !(sam != SA_NONE && (button[1] || button[2])) {
            g.varz.fire_button_held = false;
        } else if g.config.shot_repeat[SHOT_SPECIAL] == 0
            && !g.varz.fire_button_held
            && g.varz.flare_duration == 0
            && g.varz.special_wait == 0
        {
            g.varz.fire_button_held = true;
            let sp = g.player.player[0].items.special;
            je_special_complete(g, player_num, sp);
        }
    }

    if g.varz.astral_duration > 0 {
        g.varz.astral_duration -= 1;
    }

    g.shots.shot_avail[MAX_PWEAPON - 1] = 0;
    if g.varz.flare_duration > 1 {
        if g.varz.special_weapon_filter != -99 {
            let c = &mut g.config;
            if c.level_filter == -99 && c.level_brightness == -99 {
                c.filter_active = false;
            }
            if !c.filter_active {
                c.level_filter = g.varz.special_weapon_filter;
                if c.level_filter == 7 {
                    c.level_brightness = 0;
                }
                c.filter_active = true;
            }

            g.varz.flare_col_chg = if mtrand::mt_rand(g).is_multiple_of(2) {
                -1
            } else {
                1
            };

            if g.config.level_filter == 7 {
                if g.config.level_brightness < -6 {
                    g.varz.flare_col_chg = 1;
                }
                if g.config.level_brightness > 6 {
                    g.varz.flare_col_chg = -1;
                }
                g.config.level_brightness =
                    g.config.level_brightness.wrapping_add(g.varz.flare_col_chg);
            }
        }

        if ((mtrand::mt_rand(g) % 6) as i32) < i32::from(g.varz.special_weapon_freq) {
            g.varz.b = MAX_PWEAPON as i16;

            let wpn = g.varz.special_weapon_wpn;
            let (mx, my) = (g.player.player[0].mouse_x, g.player.player[0].mouse_y);
            if g.varz.link_to_player {
                if g.config.shot_repeat[SHOT_SPECIAL] == 0 {
                    let (px, py) = (g.player.player[0].x as u16, g.player.player[0].y as u16);
                    g.varz.b = shots::player_shot_create(
                        g,
                        0,
                        SHOT_SPECIAL as u32,
                        px,
                        py,
                        mx,
                        my,
                        wpn,
                        player_num,
                    );
                }
            } else {
                let px = (mtrand::mt_rand(g) % 280) as u16;
                let py = (mtrand::mt_rand(g) % 180) as u16;
                g.varz.b = shots::player_shot_create(
                    g,
                    0,
                    SHOT_SPECIAL as u32,
                    px,
                    py,
                    mx,
                    my,
                    wpn,
                    player_num,
                );
            }

            if g.varz.spray_special && g.varz.b != MAX_PWEAPON as i16 {
                let b = g.varz.b as usize;
                let xm = (mtrand::mt_rand(g) % 5).wrapping_sub(2) as i16;
                let ym = (mtrand::mt_rand(g) % 5).wrapping_sub(2) as i16;
                let shot = &mut g.shots.player_shot_data[b];
                shot.shot_xm = xm;
                shot.shot_ym = ym;
                if shot.shot_ym == 0 {
                    shot.shot_ym += 1;
                }
            }
        }

        g.varz.flare_duration -= 1;
        if g.varz.flare_duration == 1 {
            g.varz.special_wait = g.varz.next_special_wait;
        }
    } else if g.varz.flare_start {
        g.varz.flare_start = false;
        g.config.shot_repeat[SHOT_SPECIAL] = if g.varz.link_to_player { 15 } else { 200 };
        g.varz.flare_duration = 0;
        if g.config.level_filter == g.varz.special_weapon_filter {
            g.config.level_filter = -99;
            g.config.level_brightness = -99;
            g.config.filter_active = false;
        }
    }

    if g.varz.zinglon_duration > 1 {
        g.varz.temp = (25 - (i32::from(g.varz.zinglon_duration) - 25).abs()) as u8;
        let temp = i32::from(g.varz.temp);
        let x = g.player.player[0].x;
        let surface = g.video.vga_screen_id();

        vga256d::je_bar_bright(g, surface, x + 7 - temp, 0, x + 7 + temp, 184);
        vga256d::je_bar_bright(g, surface, x + 7 - temp - 2, 0, x + 7 + temp + 2, 184);

        g.varz.zinglon_duration -= 1;
        if g.varz.zinglon_duration.is_multiple_of(5) {
            g.shots.shot_avail[MAX_PWEAPON - 1] = 1;
        }
    }
}

/// `explosion_data[type]`: sprite and time to live of each explosion type.
const EXPLOSION_DATA: [(u16, u8); 53] = [
    (144, 7),
    (120, 12),
    (190, 12),
    (209, 12),
    (152, 12),
    (171, 12),
    (133, 7), // White Smoke
    (1, 12),
    (20, 12),
    (39, 12),
    (58, 12),
    (110, 3),
    (76, 7),
    (91, 3),
    (227, 3), // 15
    (230, 3),
    (233, 3),
    (252, 3),
    (246, 3),
    (249, 3), // 20
    (265, 3),
    (268, 3),
    (271, 3),
    (236, 3),
    (239, 3), // 25
    (242, 3),
    (261, 3),
    (274, 3),
    (277, 3),
    (280, 3), // 30
    (299, 3),
    (284, 3),
    (287, 3),
    (290, 3),
    (293, 3), // 35
    (165, 8), // Coin Values
    (184, 8),
    (203, 8),
    (222, 8),
    (168, 8),
    (187, 8),
    (206, 8),
    (225, 10),
    (169, 10),
    (188, 10),
    (207, 20),
    (226, 14),
    (170, 14),
    (189, 14),
    (208, 14),
    (246, 14),
    (227, 14),
    (265, 14),
];

/// `JE_setupExplosion(x, y, deltaY, type, fixedPosition, followPlayer)`:
/// `fixedPosition` for coin/gem values, `followPlayer` for the player's
/// shield (one player). A type with no table entry sets up nothing
/// (divergence D-UB-VARZ-1).
#[doc(alias = "JE_setupExplosion")]
pub fn je_setup_explosion(
    g: &mut Game,
    x: i16,
    y: i16,
    delta_y: i16,
    r#type: i16,
    fixed_position: bool,
    follow_player: bool,
) {
    if y > -16 && y < 190 {
        for e in g.varz.explosions.iter_mut() {
            if e.ttl == 0 {
                let mut t = r#type;
                let (mut ex, mut ey) = (x, y);
                if t == 6 {
                    ey = ey.wrapping_add(12);
                    ex = ex.wrapping_add(2);
                } else if t == 98 {
                    t = 6;
                }
                let Some(&(sprite, ttl)) =
                    usize::try_from(t).ok().and_then(|t| EXPLOSION_DATA.get(t))
                else {
                    return;
                };
                e.x = ex;
                e.y = ey;
                e.sprite = sprite;
                e.ttl = ttl;
                e.follow_player = follow_player;
                e.fixed_position = fixed_position;
                e.delta_y = delta_y;
                break;
            }
        }
    }
}

#[doc(alias = "JE_setupExplosionLarge")]
pub fn je_setup_explosion_large(g: &mut Game, enemy_ground: bool, explo_num: u8, x: i16, y: i16) {
    if y >= 0 {
        let (l, r) = (x.wrapping_sub(6), x.wrapping_add(6));
        let top = y.wrapping_sub(14);
        let types = if enemy_ground {
            [2, 4, 3, 5]
        } else {
            [7, 9, 8, 10]
        };
        je_setup_explosion(g, l, top, 0, types[0], false, false);
        je_setup_explosion(g, r, top, 0, types[1], false, false);
        je_setup_explosion(g, l, y, 0, types[2], false, false);
        je_setup_explosion(g, r, y, 0, types[3], false, false);

        let (explo_num, big) = if explo_num > 10 {
            (explo_num - 10, true)
        } else {
            (explo_num, false)
        };

        if explo_num != 0 {
            for e in g.varz.rep_explosions.iter_mut() {
                if e.ttl == 0 {
                    e.ttl = u32::from(explo_num);
                    e.delay = 2;
                    e.x = x as u32;
                    e.y = y as u32;
                    e.big = big;
                    break;
                }
            }
        }
    }
}

#[doc(alias = "JE_wipeShieldArmorBars")]
pub fn je_wipe_shield_armor_bars(g: &mut Game) {
    let surface = seg(g);
    let one_bar = !g.config.two_player_mode || g.config.galaga_mode;
    let p0 = &g.player.player[0];
    let (shield, armor) = (p0.shield, p0.armor);
    if one_bar {
        let y2 = 194u32.wrapping_sub(shield.wrapping_mul(2)) as i32;
        vga256d::fill_rectangle_xy(g, surface, 270, 137, 278, y2, 0);
    } else {
        vga256d::fill_rectangle_xy(g, surface, 270, 60 - 44, 278, 60, 0);
        vga256d::fill_rectangle_xy(g, surface, 270, 194 - 44, 278, 194, 0);
    }
    if one_bar {
        let y2 = 194u32.wrapping_sub(armor.wrapping_mul(2)) as i32;
        vga256d::fill_rectangle_xy(g, surface, 307, 137, 315, y2, 0);
    } else {
        vga256d::fill_rectangle_xy(g, surface, 307, 60 - 44, 315, 60, 0);
        vga256d::fill_rectangle_xy(g, surface, 307, 194 - 44, 315, 194, 0);
    }
}

/// `JE_playerDamage(temp, &player[p])`: applies `temp` damage to shield,
/// then armor; returns the damage when it went past the shield, else 0.
#[doc(alias = "JE_playerDamage")]
pub fn je_player_damage(g: &mut Game, temp: u8, p: usize) -> u8 {
    let mut temp = temp;
    let mut player_damage = 0i32;
    g.varz.sound_queue[7] = S_SHIELD_HIT;

    if g.player.player[p].shield < u32::from(temp) {
        player_damage = i32::from(temp);
        temp = (u32::from(temp) - g.player.player[p].shield) as u8;
        g.player.player[p].shield = 0;

        if temp > 0 {
            // through the shield, now the armor
            let pl = &mut g.player.player[p];
            if pl.armor < u32::from(temp) {
                pl.armor = 0;

                if pl.is_alive && !g.config.you_are_cheating {
                    g.varz.level_timer = false;
                    pl.is_alive = false;
                    pl.exploding_ticks = 60;
                    g.varz.level_end = 40;
                    g.nortsong.temp_volume = g.nortsong.tyr_music_volume;
                    g.varz.sound_queue[1] = S_EXPLOSION_22;
                }
            } else {
                pl.armor -= u32::from(temp);
                g.varz.sound_queue[7] = S_HULL_HIT;
            }
        }
    } else {
        g.player.player[p].shield -= u32::from(temp);

        let follow = !g.config.two_player_mode;
        let (x, y) = (g.player.player[p].x, g.player.player[p].y);
        for (dx, dy, t) in [
            (-17, -12, 14),
            (-5, -12, 15),
            (7, -12, 16),
            (19, -12, 17),
            (-17, 2, 18),
            (19, 2, 19),
            (-17, 16, 20),
            (-5, 16, 21),
            (7, 16, 22),
        ] {
            je_setup_explosion(g, (x + dx) as i16, (y + dy) as i16, 0, t, false, follow);
        }
    }

    je_wipe_shield_armor_bars(g);
    g.video.vga_screen = g.video.vga_screen_seg; // side-effect of game_screen
    je_draw_shield(g);
    je_draw_armor(g);
    g.video.vga_screen = g.video.game_screen; // side-effect of game_screen

    player_damage as u8
}

#[doc(alias = "JE_portConfigs")]
pub fn je_port_configs(g: &mut Game) -> u16 {
    let player_index = usize::from(g.config.two_player_mode);
    let id = g.player.player[player_index].items.weapon[REAR_WEAPON].id;
    g.varz.temp_w = u16::from(weapon_port(g, id).opnum);
    g.varz.temp_w
}

#[doc(alias = "JE_drawShield")]
pub fn je_draw_shield(g: &mut Game) {
    let surface = g.video.vga_screen_id();
    if g.config.two_player_mode && !g.config.galaga_mode {
        for i in 0..g.player.player.len() {
            let num = (g.player.player[i].shield as f32 * 0.8f32).round() as i16;
            nortvars::je_d_bar3(g, surface, 270, (60 + 134 * i) as i16, num, 144);
        }
    } else {
        let p0 = &g.player.player[0];
        let (shield, shield_max) = (p0.shield, p0.shield_max);
        nortvars::je_d_bar3(g, surface, 270, 194, shield as i16, 144);
        if shield != shield_max {
            let y = 193u32.wrapping_sub(shield_max.wrapping_mul(2)) as i32;
            vga256d::je_rectangle(g, surface, 270, y, 278, y, 68);
        }
    }
}

#[doc(alias = "JE_drawArmor")]
pub fn je_draw_armor(g: &mut Game) {
    for p in g.player.player.iter_mut() {
        if p.armor > 28 {
            p.armor = 28;
        }
    }

    let surface = g.video.vga_screen_id();
    if g.config.two_player_mode && !g.config.galaga_mode {
        for i in 0..g.player.player.len() {
            let num = (g.player.player[i].armor as f32 * 0.8f32).round() as i16;
            nortvars::je_d_bar3(g, surface, 307, (60 + 134 * i) as i16, num, 224);
        }
    } else {
        let armor = g.player.player[0].armor as i16;
        nortvars::je_d_bar3(g, surface, 307, 194, armor, 224);
    }
}

/// `JE_doSP(x, y, num, explowidth, color)`: `num` super pixels.
#[doc(alias = "JE_doSP")]
pub fn je_do_sp(g: &mut Game, x: u16, y: u16, num: u16, explowidth: u8, color: u8) {
    g.varz.temp = 0;
    while u16::from(g.varz.temp) < num {
        let tempr = (f64::from(mtrand::mt_rand_lt1(g)) * (2.0 * std::f64::consts::PI)) as f32;
        let tempy =
            (libm::cosf(tempr) * mtrand::mt_rand_1(g) * f32::from(explowidth)).round() as i32;
        let tempx =
            (libm::sinf(tempr) * mtrand::mt_rand_1(g) * f32::from(explowidth)).round() as i32;

        let v = &mut g.varz;
        v.last_superpixel += 1;
        if v.last_superpixel as usize >= MAX_SUPERPIXELS {
            v.last_superpixel = 0;
        }
        let s = &mut v.superpixels[v.last_superpixel as usize];
        s.x = (tempx + i32::from(x)) as u32;
        s.y = (tempy + i32::from(y)) as u32;
        s.delta_x = tempx;
        s.delta_y = tempy + 1;
        s.color = color;
        s.z = 15;

        g.varz.temp = g.varz.temp.wrapping_add(1);
    }
}

#[doc(alias = "JE_drawSP")]
pub fn je_draw_sp(g: &mut Game) {
    let id = g.video.vga_screen_id();
    for i in (0..MAX_SUPERPIXELS).rev() {
        let sp = &mut g.varz.superpixels[i];
        if sp.z == 0 {
            continue;
        }
        sp.x = sp.x.wrapping_add(sp.delta_x as u32);
        sp.y = sp.y.wrapping_add(sp.delta_y as u32);
        let sp = *sp;

        let s = g.video.screen_mut(id);
        let (w, h, pitch) = (s.w as u32, s.h as u32, s.pitch as usize);
        if sp.x < w && sp.y < h {
            let at = sp.y as usize * pitch + sp.x as usize;
            let px = &mut s.pixels;
            let blend = |p: u8, z: u32| {
                ((u32::from(p) & 0x0f).wrapping_add(z) >> 1).wrapping_add(u32::from(sp.color)) as u8
            };

            px[at] = blend(px[at], sp.z);
            if sp.x > 0 {
                px[at - 1] = blend(px[at - 1], sp.z >> 1);
            }
            if sp.x < w - 1 {
                px[at + 1] = blend(px[at + 1], sp.z >> 1);
            }
            if sp.y > 0 {
                px[at - pitch] = blend(px[at - pitch], sp.z >> 1);
            }
            if sp.y < h - 1 {
                px[at + pitch] = blend(px[at + pitch], sp.z >> 1);
            }
        }

        g.varz.superpixels[i].z -= 1;
    }
}
