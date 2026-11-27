// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `mainint.c` / `mainint.h`.

use serde::{Deserialize, Serialize};

pub const MAX_PAGE: u8 = 8;
pub const TOPICS: usize = 6;
/// `topicStart`.
pub const TOPIC_START: [u8; TOPICS] = [0, 1, 2, 3, 7, 255];

/// Mutable globals of `mainint.c` (rule R3).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// Fire, left fire, right fire, mode swap.
    pub button: [bool; 4],
    pub constant_last_x: i8,
    pub text_erase: u16,
    pub upgrade_cost: u16,
    pub downgrade_cost: u16,
    pub perform_save: bool,
    pub jump_section: bool,
    /// Whether DisplayText uses the last 16 colors.
    pub use_last_bank: bool,
    pub pause_pressed: bool,
    pub ingamemenu_pressed: bool,
}
