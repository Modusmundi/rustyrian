// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `demo.c` / `demo.h`.

use crate::file::File;
use serde::{Deserialize, Serialize};

/// `seed`: the RNG seed demos play and record with.
pub const SEED: u64 = 32_402_394;

/// Mutable globals of `demo.c` (rule R3).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub play_demo: bool,
    pub record_demo: bool,
    pub stopped_demo: bool,
    pub demo_num: u8,
    pub demo_file: File,
    pub demo_keys: u8,
    /// FKA `Varz.lastMoveWait`.
    pub demo_keys_wait: u16,
}
