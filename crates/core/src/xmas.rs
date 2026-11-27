// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `xmas.c` / `xmas.h`.

use serde::{Deserialize, Serialize};

/// Mutable globals of `xmas.c` (rule R3).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub xmas: bool,
}
