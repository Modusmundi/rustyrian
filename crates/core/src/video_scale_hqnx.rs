// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) 2003 MaxSt ( maxst@hiend3d.com )
// Copyright (C) rustyrian contributors

//! Port of upstream `video_scale_hqNx.c`.

use serde::{Deserialize, Serialize};

/// `Ymask`.
pub const YMASK: i32 = 0x00FF_0000;
/// `Umask`.
pub const UMASK: i32 = 0x0000_FF00;
/// `Vmask`.
pub const VMASK: i32 = 0x0000_00FF;
/// `trY`.
pub const TR_Y: i32 = 0x0030_0000;
/// `trU`.
pub const TR_U: i32 = 0x0000_0700;
/// `trV`.
pub const TR_V: i32 = 0x0000_0006;

/// Mutable globals of `video_scale_hqNx.c` (rule R3).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// `YUV1`.
    pub yuv1: i32,
    /// `YUV2`.
    pub yuv2: i32,
}
