// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `lvlmast.c` / `lvlmast.h`.

use serde::{Deserialize, Serialize};

pub const EVENT_MAXIMUM: usize = 2500;

pub const WEAP_NUM: usize = 780;
pub const PORT_NUM: usize = 42;
pub const ARMOR_NUM: usize = 4;
pub const POWER_NUM: usize = 6;
pub const ENGINE_NUM: usize = 6;
pub const OPTION_NUM: usize = 30;
pub const SHIP_NUM: usize = 13;
pub const SHIELD_NUM: usize = 10;
pub const SPECIAL_NUM: usize = 46;

pub const ENEMY_NUM: usize = 850;

/// `shapeFile` `[1..34]`. Entry 25 should be `'&'` rather than `'5'`
/// (upstream comment).
pub const SHAPE_FILE: [u8; 34] = *b"2478ABCDEFGHIJKLMNOPQRSTU5#V0@3^59";

/// `lvlmast.c` has no mutable globals.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {}
