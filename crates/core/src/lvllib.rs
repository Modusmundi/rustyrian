// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `lvllib.c` / `lvllib.h`.

use serde::{Deserialize, Serialize};

/// `JE_LvlPosType` `[1..42 + 1]`.
pub type JeLvlPosType = [i32; 43];

/// Mutable globals of this module (rule R3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    #[serde(with = "crate::serde_big")]
    pub lvl_pos: JeLvlPosType,
    /// `string [12]` (FKA `LvlLib.levelFile`).
    pub level_filename: [u8; 13],
    pub lvl_num: u16,
}

impl Default for State {
    fn default() -> Self {
        State {
            lvl_pos: [0; 43],
            level_filename: [0; 13],
            lvl_num: 0,
        }
    }
}
