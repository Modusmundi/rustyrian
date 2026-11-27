// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `logging.c` / `logging.h`.
//!
//! The `logDebug`/`logInfo`/`logWarn`/`logError` macros are `SDL_Log*` calls:
//! callers use `g.platform.log_debug(SDL_LOG_CATEGORY_APPLICATION, ..)`,
//! `.log(..)`, `.log_warn(..)`, `.log_error(..)` with the message formatted as
//! C's printf would.

use crate::Game;
use serde::{Deserialize, Serialize};

/// `SDL_LOG_CATEGORY_APPLICATION`.
pub const SDL_LOG_CATEGORY_APPLICATION: i32 = 0;
/// `SDL_LOG_PRIORITY_CRITICAL`.
pub const SDL_LOG_PRIORITY_CRITICAL: i32 = 6;
/// `SDL_MESSAGEBOX_ERROR`.
pub const SDL_MESSAGEBOX_ERROR: u32 = 0x10;

/// Mutable globals of this module (rule R3): none.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {}

/// `logFatal(fmt, ...)`. C is variadic; callers format `message` exactly as
/// C's `vsnprintf` would. C appends `"\n"` (its buffer grows to fit, and the
/// oracle's `malloc` cannot fail), passes the result to SDL's log output
/// function at critical priority and shows it in an error message box.
#[doc(alias = "logFatal")]
pub fn log_fatal(g: &mut Game, message: &str) {
    let buffer = format!("{message}\n");

    if let Some(log_output) = g.platform.log_get_output_function() {
        g.platform.log_output(
            log_output,
            SDL_LOG_CATEGORY_APPLICATION,
            SDL_LOG_PRIORITY_CRITICAL,
            &buffer,
        );
    }

    g.platform
        .show_simple_message_box(SDL_MESSAGEBOX_ERROR, "Error", &buffer, None);
}
