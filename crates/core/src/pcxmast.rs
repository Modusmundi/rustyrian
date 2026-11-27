// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `pcxmast.c` / `pcxmast.h`.

use serde::{Deserialize, Serialize};

pub const PCX_NUM: usize = 13;

/// `JE_pcxpostype` `[1..PCXnum + 1]`.
pub type JePcxpostype = [i32; PCX_NUM + 1];

/// `pcxpal` `[1..PCXnum]`.
pub const PCXPAL: [u8; PCX_NUM] = [0, 7, 5, 8, 10, 5, 18, 19, 19, 20, 21, 22, 5];

/// `facepal` `[1..12]`.
pub const FACEPAL: [u8; 12] = [1, 2, 3, 4, 6, 9, 11, 12, 16, 13, 14, 15];

/// Mutable globals of this module (rule R3).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub pcxpos: JePcxpostype,
}
