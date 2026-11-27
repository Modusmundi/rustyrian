// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `font.c` / `font.h`.

use serde::{Deserialize, Serialize};

/// `Font`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum Font {
    /// `FONT_LARGE`.
    Large = 0,
    /// `FONT_NORMAL`.
    Normal = 1,
    /// `FONT_SMALL`.
    Small = 2,
}

pub const FONT_LARGE: Font = Font::Large;
pub const FONT_NORMAL: Font = Font::Normal;
pub const FONT_SMALL: Font = Font::Small;

/// `FontAlignment`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum FontAlignment {
    /// `ALIGN_LEFT`.
    Left = 0,
    /// `ALIGN_CENTER`.
    Center = 1,
    /// `ALIGN_RIGHT`.
    Right = 2,
}

pub const ALIGN_LEFT: FontAlignment = FontAlignment::Left;
pub const ALIGN_CENTER: FontAlignment = FontAlignment::Center;
pub const ALIGN_RIGHT: FontAlignment = FontAlignment::Right;

/// `font.c` has no mutable globals.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {}
