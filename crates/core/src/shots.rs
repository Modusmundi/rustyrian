// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `shots.c` / `shots.h`.

use serde::{Deserialize, Serialize};

/// `PlayerShotDataType`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerShotDataType {
    pub shot_x: i16,
    pub shot_y: i16,
    pub shot_xm: i16,
    pub shot_ym: i16,
    pub shot_xc: i16,
    pub shot_yc: i16,
    pub shot_complicated: bool,
    pub shot_dev_x: i16,
    pub shot_dir_x: i16,
    pub shot_dev_y: i16,
    pub shot_dir_y: i16,
    pub shot_cir_size_x: i16,
    pub shot_cir_size_y: i16,
    pub shot_trail: u8,
    pub shot_gr: u16,
    pub shot_ani: u16,
    pub shot_ani_max: u16,
    pub shot_dmg: u8,
    pub shot_blast_filter: u8,
    pub chain_reaction: u8,
    pub player_number: u8,
    pub aim_at_enemy: u8,
    pub aim_delay: u8,
    pub aim_delay_max: u8,
}

pub const MAX_PWEAPON: usize = 81;

/// `linkMultiGr` `[0..16]`.
pub const LINK_MULTI_GR: [u16; 17] = [
    77, 221, 183, 301, 1, 282, 164, 202, 58, 201, 163, 281, 39, 300, 182, 220, 77,
];
/// `linkSonicGr` `[0..16]`.
pub const LINK_SONIC_GR: [u16; 17] = [
    85, 242, 131, 303, 47, 284, 150, 223, 66, 224, 149, 283, 9, 302, 130, 243, 85,
];
/// `linkMult2Gr` `[0..16]`.
pub const LINK_MULT2_GR: [u16; 17] = [
    78, 299, 295, 297, 2, 278, 276, 280, 59, 279, 275, 277, 40, 296, 294, 298, 78,
];

/// Mutable globals of this module (rule R3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    #[serde(with = "crate::serde_big::boxed")]
    pub player_shot_data: Box<[PlayerShotDataType; MAX_PWEAPON + 1]>,
    #[serde(with = "crate::serde_big")]
    pub shot_avail: [u8; MAX_PWEAPON],
}

impl Default for State {
    fn default() -> Self {
        State {
            player_shot_data: Box::new([PlayerShotDataType::default(); MAX_PWEAPON + 1]),
            shot_avail: [0; MAX_PWEAPON],
        }
    }
}
