// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) 2003 MaxSt ( maxst@hiend3d.com )
// Copyright (C) rustyrian contributors

//! Port of upstream `video_scale_hqNx.c`.

use serde::{Deserialize, Serialize};

/// Mutable globals of this module (rule R3).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {}
