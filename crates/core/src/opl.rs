// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2002-2010 The DOSBox Team
// Copyright (C) 1998-2001 Ken Silverman
// Copyright (C) rustyrian contributors

//! Port of upstream `opl.c` / `opl.h`.

use serde::{Deserialize, Serialize};

/// Mutable globals of this module (rule R3).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {}
