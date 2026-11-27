// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `opentyr.c` / `opentyr.h`.

use crate::Game;
use crate::episodes::snprintf_into;
use crate::video::{SCALING_MODE_MAX, SCALING_MODE_NAMES};
use crate::video_scale::{SCALERS, SCALERS_COUNT};
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

/// The bytes before the first NUL of `buffer`.
fn c_str(buffer: &[u8]) -> &[u8] {
    let n = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    &buffer[..n]
}

/// Setup-menu picker: the window plus one entry per display.
#[doc(alias = "getDisplayPickerItemsCount")]
pub fn get_display_picker_items_count(g: &mut Game) -> usize {
    // `(size_t)` of the `int` count.
    1usize.wrapping_add(g.platform.get_num_video_displays() as usize)
}

/// Setup-menu picker item `i`; display names are formatted into `buffer`
/// (truncated as `snprintf` does). Returns the text without its NUL.
#[doc(alias = "getDisplayPickerItem")]
pub fn get_display_picker_item(i: usize, buffer: &mut [u8]) -> Vec<u8> {
    if i == 0 {
        return b"Window".to_vec();
    }

    snprintf_into(buffer, &format!("Display {}", i as i32));
    c_str(buffer).to_vec()
}

#[doc(alias = "getScalerPickerItemsCount")]
pub fn get_scaler_picker_items_count() -> usize {
    SCALERS_COUNT as usize
}

#[doc(alias = "getScalerPickerItem")]
pub fn get_scaler_picker_item(i: usize, _buffer: &mut [u8]) -> Vec<u8> {
    SCALERS[i].name.as_bytes().to_vec()
}

#[doc(alias = "getScalingModePickerItemsCount")]
pub fn get_scaling_mode_picker_items_count() -> usize {
    SCALING_MODE_MAX
}

#[doc(alias = "getScalingModePickerItem")]
pub fn get_scaling_mode_picker_item(i: usize, _buffer: &mut [u8]) -> Vec<u8> {
    SCALING_MODE_NAMES[i].as_bytes().to_vec()
}
