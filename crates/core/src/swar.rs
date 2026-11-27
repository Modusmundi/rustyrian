// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `swar.h`.

use serde::{Deserialize, Serialize};

/// `Swar8`: an unsigned integer with 8-bit lanes. The oracle builds on LP64
/// hosts (`UINTPTR_MAX >= UINT64_MAX`), so it is `uint64_t`.
pub type Swar8 = u64;

/// `SWAR8_LANES`.
pub const SWAR8_LANES: usize = 8;

/// `swar.h` has no mutable globals.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {}
