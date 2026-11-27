// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `episodes.c` / `episodes.h`.
//!
//! C's array typedefs of anonymous structs (`JE_WeaponPortType` and friends)
//! become a named element struct (`JeWeaponPort`, ...) plus the array alias.

use crate::Game;
use crate::file::{self, File};
use crate::game::{EXIT_FAILURE, Flow, Halt};
use crate::logging::log_fatal;
use crate::lvllib::{analyze_level, c_string};
use crate::lvlmast::{
    ENEMY_NUM, OPTION_NUM, PORT_NUM, POWER_NUM, SHIELD_NUM, SHIP_NUM, SPECIAL_NUM, WEAP_NUM,
};
use serde::{Deserialize, Serialize};
use std::slice::from_mut as one;

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

/// `snprintf(dst, sizeof dst, …)` of the already formatted `s`: copies what
/// fits, NUL-terminates, leaves the rest of `dst` untouched.
pub fn snprintf_into(dst: &mut [u8], s: &str) {
    let n = s.len().min(dst.len() - 1);
    dst[..n].copy_from_slice(&s.as_bytes()[..n]);
    dst[n] = 0;
}

/// A length-prefixed `string [30]` item name.
fn read_name(file: &mut File, name: &mut [u8; 31]) {
    let name_len = file::file_read_u8(file);
    file::file_read_char_array(file, &mut name[..30]);
    name[usize::from(name_len.min(30))] = 0;
}

/// Message of the R10 guard for an episode-4 level file without levels
/// (upstream would index `lvlPos[-1]`).
fn no_levels_message(filename: &str) -> String {
    format!("Failed to read from file '{filename}': no levels")
}

#[doc(alias = "JE_loadItemDat")]
pub fn je_load_item_dat(g: &mut Game) -> Flow {
    let filename: String;
    let mut file: File;

    if g.episodes.episode_num <= 3 {
        filename = "tyrian.hdt".to_owned();

        file = file::data_file_open(g, &filename, "rb");
        if file.error {
            let message = format!(
                "Failed to open file '{}': {}",
                filename,
                file::file_get_error(&file)
            );
            log_fatal(g, &message);
            return Err(Halt { code: EXIT_FAILURE });
        }

        let position = i64::from(file::file_read_u32(&mut file));

        file::file_set_position(&mut file, position);
    } else {
        // Episode 4 stores item data in the level file.
        filename = c_string(&g.lvllib.level_filename);

        file = file::data_file_open(g, &filename, "rb");
        if file.error {
            let message = format!(
                "Failed to open file '{}': {}",
                filename,
                file::file_get_error(&file)
            );
            log_fatal(g, &message);
            return Err(Halt { code: EXIT_FAILURE });
        }

        // R10 guard (UF-5): upstream reads `lvlPos[lvlNum-1]` unchecked.
        let Some(last) = usize::from(g.lvllib.lvl_num).checked_sub(1) else {
            log_fatal(g, &no_levels_message(&filename));
            return Err(Halt { code: EXIT_FAILURE });
        };
        file::file_set_position(&mut file, i64::from(g.lvllib.lvl_pos[last]));
    }

    // Upstream asserts that the counts equal the table sizes; NDEBUG builds
    // (rule R11) ignore them and always read the fixed tables. They come from
    // data, so they are not checked here either (R10: never panic on input).
    let mut counts = [0u16; 7];
    file::file_read_u16_array(&mut file, &mut counts);

    let e = &mut g.episodes;
    let f = &mut file;

    for w in e.weapons.iter_mut() {
        file::file_read_u16_array(f, one(&mut w.drain));
        file::file_read_u8_array(f, one(&mut w.shotrepeat));
        file::file_read_u8_array(f, one(&mut w.multi));
        file::file_read_u16_array(f, one(&mut w.weapani));
        file::file_read_u8_array(f, one(&mut w.max));
        file::file_read_u8_array(f, one(&mut w.tx));
        file::file_read_u8_array(f, one(&mut w.ty));
        file::file_read_u8_array(f, one(&mut w.aim));
        file::file_read_u8_array(f, &mut w.attack);
        file::file_read_u8_array(f, &mut w.del);
        file::file_read_s8_array(f, &mut w.sx);
        file::file_read_s8_array(f, &mut w.sy);
        file::file_read_s8_array(f, &mut w.bx);
        file::file_read_s8_array(f, &mut w.by);
        file::file_read_u16_array(f, &mut w.sg);
        file::file_read_s8_array(f, one(&mut w.acceleration));
        file::file_read_s8_array(f, one(&mut w.accelerationx));
        file::file_read_u8_array(f, one(&mut w.circlesize));
        file::file_read_u8_array(f, one(&mut w.sound));
        file::file_read_u8_array(f, one(&mut w.trail));
        file::file_read_u8_array(f, one(&mut w.shipblastfilter));
    }

    for p in e.weapon_port.iter_mut() {
        read_name(f, &mut p.name);
        file::file_read_u8_array(f, one(&mut p.opnum));
        file::file_read_u16_array(f, &mut p.op[0]);
        file::file_read_u16_array(f, &mut p.op[1]);
        file::file_read_u16_array(f, one(&mut p.cost));
        file::file_read_u16_array(f, one(&mut p.itemgraphic));
        file::file_read_u16_array(f, one(&mut p.poweruse));
    }

    for s in e.special.iter_mut() {
        read_name(f, &mut s.name);
        file::file_read_u16_array(f, one(&mut s.itemgraphic));
        file::file_read_u8_array(f, one(&mut s.pwr));
        file::file_read_u8_array(f, one(&mut s.stype));
        file::file_read_u16_array(f, one(&mut s.wpn));
    }

    for p in e.power_sys.iter_mut() {
        read_name(f, &mut p.name);
        file::file_read_u16_array(f, one(&mut p.itemgraphic));
        file::file_read_u8_array(f, one(&mut p.power));
        file::file_read_s8_array(f, one(&mut p.speed));
        file::file_read_u16_array(f, one(&mut p.cost));
    }

    for s in e.ships.iter_mut() {
        read_name(f, &mut s.name);
        file::file_read_u16_array(f, one(&mut s.shipgraphic));
        file::file_read_u16_array(f, one(&mut s.itemgraphic));
        file::file_read_u8_array(f, one(&mut s.ani));
        file::file_read_s8_array(f, one(&mut s.spd));
        file::file_read_u8_array(f, one(&mut s.dmg));
        file::file_read_u16_array(f, one(&mut s.cost));
        file::file_read_u8_array(f, one(&mut s.bigshipgraphic));
    }

    for o in e.options.iter_mut() {
        read_name(f, &mut o.name);
        file::file_read_u8_array(f, one(&mut o.pwr));
        file::file_read_u16_array(f, one(&mut o.itemgraphic));
        file::file_read_u16_array(f, one(&mut o.cost));
        file::file_read_u8_array(f, one(&mut o.tr));
        file::file_read_u8_array(f, one(&mut o.option));
        file::file_read_s8_array(f, one(&mut o.opspd));
        file::file_read_u8_array(f, one(&mut o.ani));
        file::file_read_u16_array(f, &mut o.gr);
        file::file_read_u8_array(f, one(&mut o.wport));
        file::file_read_u16_array(f, one(&mut o.wpnum));
        file::file_read_u8_array(f, one(&mut o.ammo));
        file::file_read_bool_array(f, one(&mut o.stop));
        file::file_read_u8_array(f, one(&mut o.icongr));
    }

    for s in e.shields.iter_mut() {
        read_name(f, &mut s.name);
        file::file_read_u8_array(f, one(&mut s.tpwr));
        file::file_read_u8_array(f, one(&mut s.mpwr));
        file::file_read_u16_array(f, one(&mut s.itemgraphic));
        file::file_read_u16_array(f, one(&mut s.cost));
    }

    for d in e.enemy_dat.iter_mut() {
        file::file_read_u8_array(f, one(&mut d.ani));
        file::file_read_u8_array(f, &mut d.tur);
        file::file_read_u8_array(f, &mut d.freq);
        file::file_read_s8_array(f, one(&mut d.xmove));
        file::file_read_s8_array(f, one(&mut d.ymove));
        file::file_read_s8_array(f, one(&mut d.xaccel));
        file::file_read_s8_array(f, one(&mut d.yaccel));
        file::file_read_s8_array(f, one(&mut d.xcaccel));
        file::file_read_s8_array(f, one(&mut d.ycaccel));
        file::file_read_s16_array(f, one(&mut d.startx));
        file::file_read_s16_array(f, one(&mut d.starty));
        file::file_read_s8_array(f, one(&mut d.startxc));
        file::file_read_s8_array(f, one(&mut d.startyc));
        file::file_read_u8_array(f, one(&mut d.armor));
        file::file_read_u8_array(f, one(&mut d.esize));
        file::file_read_u16_array(f, &mut d.egraphic);
        file::file_read_u8_array(f, one(&mut d.explosiontype));
        file::file_read_u8_array(f, one(&mut d.animate));
        file::file_read_u8_array(f, one(&mut d.shapebank));
        file::file_read_s8_array(f, one(&mut d.xrev));
        file::file_read_s8_array(f, one(&mut d.yrev));
        file::file_read_u16_array(f, one(&mut d.dgr));
        file::file_read_s8_array(f, one(&mut d.dlevel));
        file::file_read_s8_array(f, one(&mut d.dani));
        file::file_read_u8_array(f, one(&mut d.elaunchfreq));
        file::file_read_u16_array(f, one(&mut d.elaunchtype));
        file::file_read_s16_array(f, one(&mut d.value));
        file::file_read_u16_array(f, one(&mut d.eenemydie));
    }

    if file.error {
        let message = format!(
            "Failed to read from file '{}': {}",
            filename,
            file::file_get_error(&file)
        );
        log_fatal(g, &message);
        return Err(Halt { code: EXIT_FAILURE });
    }

    file::file_close(&mut file);
    Ok(())
}

#[doc(alias = "JE_initEpisode")]
pub fn je_init_episode(g: &mut Game, new_episode: u8) -> Flow {
    if new_episode == g.episodes.episode_num {
        return Ok(());
    }

    g.episodes.episode_num = new_episode;
    let n = g.episodes.episode_num;

    snprintf_into(&mut g.lvllib.level_filename, &format!("tyrian{n}.lvl"));
    snprintf_into(&mut g.episodes.cube_filename, &format!("cubetxt{n}.dat"));
    snprintf_into(&mut g.episodes.episode_filename, &format!("levels{n}.dat"));

    analyze_level(g)?;
    je_load_item_dat(g)
}

#[doc(alias = "JE_scanForEpisodes")]
pub fn je_scan_for_episodes(g: &mut Game) {
    for i in 0..EPISODE_MAX {
        let mut filename = [0u8; 13];
        snprintf_into(&mut filename, &format!("tyrian{}.lvl", i + 1));
        g.episodes.episode_avail[i] = file::data_file_exists(g, &c_string(&filename));
    }
}

#[doc(alias = "JE_findNextEpisode")]
pub fn je_find_next_episode(g: &mut Game) -> u32 {
    let mut new_episode = u32::from(g.episodes.episode_num);

    g.episodes.jump_back_to_episode1 = false;

    loop {
        new_episode = new_episode.wrapping_add(1);

        if new_episode > EPISODE_MAX as u32 {
            new_episode = 1;
            g.episodes.jump_back_to_episode1 = true;
            g.config.game_has_repeated = true;
        }

        if g.episodes.episode_avail[(new_episode - 1) as usize]
            || new_episode == u32::from(g.episodes.episode_num)
        {
            break;
        }
    }

    new_episode
}
