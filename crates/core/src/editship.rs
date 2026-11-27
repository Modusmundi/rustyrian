// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `editship.c` / `editship.h`.

use crate::sprite::Sprite2Array;
use serde::{Deserialize, Serialize};

/// `JE_ShipsType`.
pub type JeShipsType = [u8; 154];

/// `extraCryptKey`.
pub const EXTRA_CRYPT_KEY: [u8; 10] = [58, 23, 16, 192, 254, 82, 113, 147, 62, 99];

/// Mutable globals of `editship.c` (rule R3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub extra_avail: bool,
    #[serde(with = "crate::serde_big")]
    pub extra_ships: JeShipsType,
    pub extra_shapes: Sprite2Array,
}

impl Default for State {
    fn default() -> Self {
        State {
            extra_avail: false,
            extra_ships: [0; 154],
            extra_shapes: Sprite2Array::default(),
        }
    }
}
