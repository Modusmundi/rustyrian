// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `fonthand.c` / `fonthand.h`.

use serde::{Deserialize, Serialize};

pub const PART_SHADE: u32 = 0;
pub const FULL_SHADE: u32 = 1;
pub const DARKEN: u32 = 2;
pub const TRICK: u32 = 3;
pub const NO_SHADE: u32 = 255;

/// `fontMap`: mapping from CP437 to font sprite index (`[33..168]`).
#[rustfmt::skip]
pub const FONT_MAP: [i8; 256] = [
     -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,
     -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,
     -1,  26,  33,  60,  61,  62,  -1,  32,  64,  65,  63,  84,  29,  83,  28,  80,
     79,  70,  71,  72,  73,  74,  75,  76,  77,  78,  31,  30,  -1,  85,  -1,  27,
     -1,   0,   1,   2,   3,   4,   5,   6,   7,   8,   9,  10,  11,  12,  13,  14,
     15,  16,  17,  18,  19,  20,  21,  22,  23,  24,  25,  68,  82,  69,  -1,  -1,
     -1,  34,  35,  36,  37,  38,  39,  40,  41,  42,  43,  44,  45,  46,  47,  48,
     49,  50,  51,  52,  53,  54,  55,  56,  57,  58,  59,  66,  81,  67,  -1,  -1,

     86,  87,  88,  89,  90,  91,  92,  93,  94,  95,  96,  97,  98,  99, 100, 101,
    102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117,
    118, 119, 120, 121, 122, 123, 124, 125, 126,  -1,  -1,  -1,  -1,  -1,  -1,  -1,
     -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,
     -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,
     -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,
     -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,
     -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,
];

/// Mutable globals of this module (rule R3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub text_glow_font: u8,
    pub text_glow_brightness: u8,
    pub level_warning_display: bool,
    pub level_warning_lines: u8,
    /// `[1..10] of string [60]`.
    #[serde(with = "crate::serde_big::d2")]
    pub level_warning_text: [[u8; 61]; 10],
    pub warning_red: bool,
    pub warning_sound_delay: u8,
    pub armor_ship_delay: u16,
    pub warning_col: u8,
    pub warning_col_change: i8,
}

impl Default for State {
    fn default() -> Self {
        State {
            text_glow_font: 0,
            text_glow_brightness: 6,
            level_warning_display: false,
            level_warning_lines: 0,
            level_warning_text: [[0; 61]; 10],
            warning_red: false,
            warning_sound_delay: 0,
            armor_ship_delay: 0,
            warning_col: 0,
            warning_col_change: 0,
        }
    }
}
