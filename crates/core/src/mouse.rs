// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `mouse.c` / `mouse.h`.

use serde::{Deserialize, Serialize};

/// `MOUSE_POINTER_NORMAL`.
pub const MOUSE_POINTER_NORMAL: u8 = 0;
/// `MOUSE_POINTER_UP`.
pub const MOUSE_POINTER_UP: u8 = 1;
/// `MOUSE_POINTER_DOWN`.
pub const MOUSE_POINTER_DOWN: u8 = 2;
/// `MOUSE_POINTER_LEFT`.
pub const MOUSE_POINTER_LEFT: u8 = 3;
/// `MOUSE_POINTER_RIGHT`.
pub const MOUSE_POINTER_RIGHT: u8 = 4;

/// Size of `mouseGrabShape` (24 × 28).
pub const MOUSE_GRAB_SHAPE_LEN: usize = 24 * 28;

/// Mutable globals of `mouse.c` (rule R3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub has_mouse: bool,
    pub mouse_has_three_buttons: bool,
    pub mouse_inactive: bool,
    pub mouse_cursor: u8,
    pub mouse_grab_x: u16,
    pub mouse_grab_y: u16,
    #[serde(with = "crate::serde_big")]
    pub mouse_grab_shape: [u8; MOUSE_GRAB_SHAPE_LEN],
}

impl Default for State {
    fn default() -> Self {
        State {
            has_mouse: true,
            mouse_has_three_buttons: true,
            mouse_inactive: true,
            mouse_cursor: 0,
            mouse_grab_x: 0,
            mouse_grab_y: 0,
            mouse_grab_shape: [0; MOUSE_GRAB_SHAPE_LEN],
        }
    }
}
