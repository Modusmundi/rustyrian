// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `picload.c` / `picload.h`.

use serde::{Deserialize, Serialize};

/// Mutable globals of this module (rule R3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// `JE_loadPic`'s local `static bool first = true`.
    pub je_load_pic_first: bool,
}

impl Default for State {
    fn default() -> Self {
        State {
            je_load_pic_first: true,
        }
    }
}
