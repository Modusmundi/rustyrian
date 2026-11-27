// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `opentyr.c` / `opentyr.h`.

use serde::{Deserialize, Serialize};

/// `OPENTYRIAN_VERSION`, as the oracle is built
/// (`-DOPENTYRIAN_VERSION="rustyrian-oracle"`). Upstream logs it and shows it
/// on screen, so both engines must agree on it for parity.
pub const OPENTYRIAN_VERSION: &str = "rustyrian-oracle";

/// `M_PI`.
pub const M_PI: f64 = std::f64::consts::PI;
/// `M_PI_2`.
pub const M_PI_2: f64 = std::f64::consts::FRAC_PI_2;
/// `M_PI_4`.
pub const M_PI_4: f64 = std::f64::consts::FRAC_PI_4;

/// Mutable globals of `opentyr.c` (rule R3).
///
/// Both are non-`const` pointers to string literals; they hold the string
/// bytes without the NUL.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub opentyrian_str: Vec<u8>,
    pub opentyrian_version: Vec<u8>,
}

impl Default for State {
    fn default() -> Self {
        State {
            opentyrian_str: b"OpenTyrian".to_vec(),
            opentyrian_version: OPENTYRIAN_VERSION.as_bytes().to_vec(),
        }
    }
}
