// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `episodes.c` / `episodes.h`.
//!
//! C's array typedefs of anonymous structs (`JE_WeaponPortType` and friends)
//! become a named element struct (`JeWeaponPort`, ...) plus the array alias.

use crate::lvlmast::{
    ENEMY_NUM, OPTION_NUM, PORT_NUM, POWER_NUM, SHIELD_NUM, SHIP_NUM, SPECIAL_NUM, WEAP_NUM,
};
use serde::{Deserialize, Serialize};

/* Episodes and general data */

pub const FIRST_LEVEL: u8 = 1;
pub const EPISODE_MAX: usize = 5;
pub const EPISODE_AVAILABLE: usize = 4;

/// `JE_WeaponType`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JeWeaponType {
    pub drain: u16,
    pub shotrepeat: u8,
    pub multi: u8,
    pub weapani: u16,
    pub max: u8,
    pub tx: u8,
    pub ty: u8,
    pub aim: u8,
    /// `[1..8]`.
    pub attack: [u8; 8],
    pub del: [u8; 8],
    pub sx: [i8; 8],
    pub sy: [i8; 8],
    pub bx: [i8; 8],
    pub by: [i8; 8],
    pub sg: [u16; 8],
    pub acceleration: i8,
    pub accelerationx: i8,
    pub circlesize: u8,
    pub sound: u8,
    pub trail: u8,
    pub shipblastfilter: u8,
}

/// Element of `JE_WeaponPortType`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JeWeaponPort {
    /// `string [30]`.
    pub name: [u8; 31],
    pub opnum: u8,
    /// `[1..2, 1..11]`.
    pub op: [[u16; 11]; 2],
    pub cost: u16,
    pub itemgraphic: u16,
    pub poweruse: u16,
}

/// `JE_WeaponPortType` `[0..portnum]`.
pub type JeWeaponPortType = [JeWeaponPort; PORT_NUM + 1];

/// Element of `JE_PowerType`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JePower {
    pub name: [u8; 31],
    pub itemgraphic: u16,
    pub power: u8,
    pub speed: i8,
    pub cost: u16,
}

/// `JE_PowerType` `[0..powernum]`.
pub type JePowerType = [JePower; POWER_NUM + 1];

/// Element of `JE_SpecialType`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JeSpecial {
    pub name: [u8; 31],
    pub itemgraphic: u16,
    pub pwr: u8,
    pub stype: u8,
    pub wpn: u16,
}

/// `JE_SpecialType` `[0..specialnum]`.
pub type JeSpecialType = [JeSpecial; SPECIAL_NUM + 1];

/// `JE_OptionType`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JeOptionType {
    pub name: [u8; 31],
    pub pwr: u8,
    pub itemgraphic: u16,
    pub cost: u16,
    pub tr: u8,
    pub option: u8,
    pub opspd: i8,
    pub ani: u8,
    /// `[1..20]`.
    pub gr: [u16; 20],
    pub wport: u8,
    pub wpnum: u16,
    pub ammo: u8,
    pub stop: bool,
    pub icongr: u8,
}

/// Element of `JE_ShieldType`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JeShield {
    pub name: [u8; 31],
    pub tpwr: u8,
    pub mpwr: u8,
    pub itemgraphic: u16,
    pub cost: u16,
}

/// `JE_ShieldType` `[0..shieldnum]`.
pub type JeShieldType = [JeShield; SHIELD_NUM + 1];

/// Element of `JE_ShipType`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JeShip {
    pub name: [u8; 31],
    pub shipgraphic: u16,
    pub itemgraphic: u16,
    pub ani: u8,
    pub spd: i8,
    pub dmg: u8,
    pub cost: u16,
    pub bigshipgraphic: u8,
}

/// `JE_ShipType` `[0..shipnum]`.
pub type JeShipType = [JeShip; SHIP_NUM + 1];

/// Element of `JE_EnemyDatType`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JeEnemyDat {
    pub ani: u8,
    /// `[1..3]`.
    pub tur: [u8; 3],
    pub freq: [u8; 3],
    pub xmove: i8,
    pub ymove: i8,
    pub xaccel: i8,
    pub yaccel: i8,
    pub xcaccel: i8,
    pub ycaccel: i8,
    pub startx: i16,
    pub starty: i16,
    pub startxc: i8,
    pub startyc: i8,
    pub armor: u8,
    pub esize: u8,
    /// `[1..20]`.
    pub egraphic: [u16; 20],
    pub explosiontype: u8,
    /// 0: not yet, 1: always, 2: when firing only.
    pub animate: u8,
    /// See LEVELMAK.DOC.
    pub shapebank: u8,
    pub xrev: i8,
    pub yrev: i8,
    pub dgr: u16,
    pub dlevel: i8,
    pub dani: i8,
    pub elaunchfreq: u8,
    pub elaunchtype: u16,
    pub value: i16,
    pub eenemydie: u16,
}

/// `JE_EnemyDatType` `[0..enemynum]`.
pub type JeEnemyDatType = [JeEnemyDat; ENEMY_NUM + 1];

fn boxed<T: Clone, const N: usize>(v: T) -> Box<[T; N]> {
    vec![v; N]
        .into_boxed_slice()
        .try_into()
        .unwrap_or_else(|_| unreachable!("vec of length N"))
}

/// Mutable globals of this module (rule R3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    #[serde(with = "crate::serde_big")]
    pub weapon_port: JeWeaponPortType,
    /// `[0..weapnum]`.
    #[serde(with = "crate::serde_big::boxed")]
    pub weapons: Box<[JeWeaponType; WEAP_NUM + 1]>,
    pub power_sys: JePowerType,
    pub ships: JeShipType,
    /// `[0..optionnum]`.
    pub options: [JeOptionType; OPTION_NUM + 1],
    pub shields: JeShieldType,
    #[serde(with = "crate::serde_big")]
    pub special: JeSpecialType,
    #[serde(with = "crate::serde_big::boxed")]
    pub enemy_dat: Box<JeEnemyDatType>,
    pub initial_episode_num: u8,
    pub episode_num: u8,
    /// `[1..episodemax]`.
    pub episode_avail: [bool; EPISODE_MAX],
    /// FKA `Episodes.macroFile`.
    pub episode_filename: [u8; 13],
    /// FKA `Episodes.cubeFile`.
    pub cube_filename: [u8; 13],
    /// Whether the level currently loaded is a bonus level.
    pub bonus_level: bool,
    /// Whether the game jumped back to Episode 1.
    pub jump_back_to_episode1: bool,
}

impl Default for State {
    fn default() -> Self {
        State {
            weapon_port: [JeWeaponPort::default(); PORT_NUM + 1],
            weapons: boxed(JeWeaponType::default()),
            power_sys: [JePower::default(); POWER_NUM + 1],
            ships: [JeShip::default(); SHIP_NUM + 1],
            options: [JeOptionType::default(); OPTION_NUM + 1],
            shields: [JeShield::default(); SHIELD_NUM + 1],
            special: [JeSpecial::default(); SPECIAL_NUM + 1],
            enemy_dat: boxed(JeEnemyDat::default()),
            initial_episode_num: 0,
            episode_num: 0,
            episode_avail: [false; EPISODE_MAX],
            episode_filename: [0; 13],
            cube_filename: [0; 13],
            bonus_level: false,
            jump_back_to_episode1: false,
        }
    }
}
