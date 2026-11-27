// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `menus.c` / `menus.h`.

use serde::{Deserialize, Serialize};

/// Mutable globals of `menus.c` (rule R3). The name tables are NUL-terminated
/// `char` buffers filled from the episode data.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    #[serde(with = "crate::serde_big::d2")]
    pub episode_name: [[u8; 31]; 6],
    pub difficulty_name: [[u8; 21]; 7],
    pub gameplay_name: [[u8; 26]; 5],
}
