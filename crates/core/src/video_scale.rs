// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Scale2x, Scale3x: Copyright (C) 2001, 2002, 2003, 2004 Andrea Mazzoleni
// Copyright (C) rustyrian contributors

//! Port of upstream `video_scale.c` / `video_scale.h`.

use serde::{Deserialize, Serialize};

/// Mutable globals of this module (rule R3).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {}
