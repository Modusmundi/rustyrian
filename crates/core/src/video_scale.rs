// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Scale2x, Scale3x: Copyright (C) 2001, 2002, 2003, 2004 Andrea Mazzoleni
// Copyright (C) rustyrian contributors

//! Port of upstream `video_scale.c` / `video_scale.h`.

use crate::video::{VGA_HEIGHT, VGA_WIDTH};
use serde::{Deserialize, Serialize};

/// `ScalerFunction` (rule R8): one variant per function upstream stores in
/// the `scalers` table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScalerFunction {
    /// `nn_16`.
    Nn16,
    /// `nn_32`.
    Nn32,
    /// `scale2x_16`.
    Scale2x16,
    /// `scale2x_32`.
    Scale2x32,
    /// `scale3x_16`.
    Scale3x16,
    /// `scale3x_32`.
    Scale3x32,
    /// `hq2x_32` (`video_scale_hqNx.c`).
    Hq2x32,
    /// `hq3x_32` (`video_scale_hqNx.c`).
    Hq3x32,
    /// `hq4x_32` (`video_scale_hqNx.c`).
    Hq4x32,
}

/// `struct Scalers`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scalers {
    pub width: i32,
    pub height: i32,
    pub scaler16: Option<ScalerFunction>,
    pub scaler32: Option<ScalerFunction>,
    pub name: &'static str,
}

const fn s(
    mul: i32,
    scaler16: Option<ScalerFunction>,
    scaler32: ScalerFunction,
    name: &'static str,
) -> Scalers {
    Scalers {
        width: mul * VGA_WIDTH,
        height: mul * VGA_HEIGHT,
        scaler16,
        scaler32: Some(scaler32),
        name,
    }
}

use ScalerFunction::*;

/// `scalers`.
pub const SCALERS: [Scalers; 9] = [
    s(1, Some(Nn16), Nn32, "None"),
    s(2, Some(Nn16), Nn32, "2x"),
    s(2, Some(Scale2x16), Scale2x32, "Scale2x"),
    s(2, None, Hq2x32, "hq2x"),
    s(3, Some(Nn16), Nn32, "3x"),
    s(3, Some(Scale3x16), Scale3x32, "Scale3x"),
    s(3, None, Hq3x32, "hq3x"),
    s(4, Some(Nn16), Nn32, "4x"),
    s(4, None, Hq4x32, "hq4x"),
];

/// `scalers_count`.
pub const SCALERS_COUNT: u32 = SCALERS.len() as u32;

/// Mutable globals of `video_scale.c` (rule R3).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// `scaler`: index into [`SCALERS`].
    pub scaler: u32,
}
