// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `loudness.c` / `loudness.h`.

use serde::{Deserialize, Serialize};

/// Mutable globals of this module that the audio callback does not touch
/// (rule R3).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {}

/// Globals touched by `audioCallback`; shared with the platform's audio
/// thread behind `Game::audio`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AudioState {}
