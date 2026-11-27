// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `animlib.c` / `animlib.h`.

use serde::{Deserialize, Serialize};

/// `FileHeader` (file-local type of `animlib.c`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FileHeader {
    pub page_count: u16,
    pub record_count: u32,
}

/// `PageDescriptor` (file-local type of `animlib.c`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PageDescriptor {
    pub first_record: u16,
    pub record_count: u16,
    pub records_size: u16,
}

/// `animlib.c` has no mutable globals.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {}
