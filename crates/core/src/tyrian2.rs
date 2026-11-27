// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `tyrian2.c` / `tyrian2.h`.

use crate::lvlmast::EVENT_MAXIMUM;
use crate::varz::JeEventRecType;
use serde::{Deserialize, Serialize};

/// `boss_bar_t`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BossBarT {
    pub link_num: u8,
    pub armor: u8,
    pub color: u8,
}

/// Mutable globals of `tyrian2.c` (rule R3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub boss_bar: [BossBarT; 2],
    pub quit: bool,
    pub load_level_ok: bool,
    #[serde(with = "crate::serde_big::boxed")]
    pub event_rec: Box<[JeEventRecType; EVENT_MAXIMUM]>,
    pub level_enemy_max: u16,
    pub level_enemy_frequency: u16,
    #[serde(with = "crate::serde_big")]
    pub level_enemy: [u16; 40],
    pub temp_str: [u8; 31],
    pub item_avail: [[u8; 10]; 9],
    pub item_avail_max: [u8; 9],
}

impl Default for State {
    fn default() -> Self {
        State {
            boss_bar: [BossBarT::default(); 2],
            quit: false,
            load_level_ok: false,
            event_rec: vec![JeEventRecType::default(); EVENT_MAXIMUM]
                .into_boxed_slice()
                .try_into()
                .unwrap_or_else(|_| unreachable!("length is EVENT_MAXIMUM")),
            level_enemy_max: 0,
            level_enemy_frequency: 0,
            level_enemy: [0; 40],
            temp_str: [0; 31],
            item_avail: [[0; 10]; 9],
            item_avail_max: [0; 9],
        }
    }
}
