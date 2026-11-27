// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `keyboard.c` / `keyboard.h`.

use serde::{Deserialize, Serialize};

/// `SDL_POLL_INTERVAL`.
pub const SDL_POLL_INTERVAL: u32 = 10;
/// `SDL_NUM_SCANCODES`.
pub const SDL_NUM_SCANCODES: usize = 512;

/// `struct KeyboardInput`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyboardInput {
    /// `SDL_Keycode`; used for secret text input (e.g. super arcade codes).
    pub sym: i32,
    /// `SDL_Scancode`; used for non-text input.
    pub scancode: u16,
    /// `SDL_Keymod`.
    pub r#mod: u16,
    /// CP437 character; used for text input (e.g. save file name).
    pub ch: u8,
}

/// `struct MouseInput`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MouseInput {
    pub x: i32,
    pub y: i32,
    pub button: u8,
}

/// `enum InputFlags`.
pub type InputFlags = u32;
/// `INPUT_ANY`.
pub const INPUT_ANY: InputFlags = 0;
/// `INPUT_NO_MOTION`: ignore mouse motion.
pub const INPUT_NO_MOTION: InputFlags = 1;

/// `lordKeySyms`: `SDLK_l`, `SDLK_o`, `SDLK_r`, `SDLK_d`.
pub const LORD_KEY_SYMS: [i32; 4] = [b'l' as i32, b'o' as i32, b'r' as i32, b'd' as i32];

/// `ucsMap`: CP437 0x80..=0xA8 to UCS.
pub const UCS_MAP: [u16; 41] = [
    0x00C7, 0x00FC, 0x00E9, 0x00E2, 0x00E4, 0x00E0, 0x00E5, 0x00E7, //
    0x00EA, 0x00EB, 0x00E8, 0x00EF, 0x00EE, 0x00EC, 0x00C4, 0x00C5, //
    0x00C9, 0x00E6, 0x00C6, 0x00F4, 0x00F6, 0x00F2, 0x00FB, 0x00F9, //
    0x00FF, 0x00D6, 0x00DC, 0x00A2, 0x00A3, 0x00A5, 0x20A7, 0x0192, //
    0x00E1, 0x00ED, 0x00F3, 0x00FA, 0x00F1, 0x00D1, 0x00AA, 0x00BA, //
    0x00BF,
];

/// Mutable globals of `keyboard.c` (rule R3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// `ESCPressed`.
    pub esc_pressed: bool,
    pub window_has_focus: bool,
    #[serde(with = "crate::serde_big")]
    pub keysactive: [bool; SDL_NUM_SCANCODES],
    pub lord_key_syms_down: [bool; 4],
    pub keyboard_inputs: [KeyboardInput; 32],
    pub keyboard_inputs_front: usize,
    pub keyboard_inputs_back: usize,
    pub keyboard_inputs_count: usize,
    /// `mouseX`.
    pub mouse_x: i32,
    /// `mouseY`.
    pub mouse_y: i32,
    pub mouse_buttons_down: u8,
    pub mouse_inputs: [MouseInput; 4],
    pub mouse_inputs_front: usize,
    pub mouse_inputs_back: usize,
    pub mouse_inputs_count: usize,
    pub mouse_has_motion_input: bool,
    pub mouse_relative_enabled: bool,
    /// `mouseWindowXRelative`: relative mouse position in window coordinates.
    pub mouse_window_x_relative: i32,
    /// `mouseWindowYRelative`.
    pub mouse_window_y_relative: i32,
}

impl Default for State {
    fn default() -> Self {
        State {
            esc_pressed: false,
            window_has_focus: false,
            keysactive: [false; SDL_NUM_SCANCODES],
            lord_key_syms_down: [false; 4],
            keyboard_inputs: [KeyboardInput::default(); 32],
            keyboard_inputs_front: 0,
            keyboard_inputs_back: 0,
            keyboard_inputs_count: 0,
            mouse_x: 0,
            mouse_y: 0,
            mouse_buttons_down: 0,
            mouse_inputs: [MouseInput::default(); 4],
            mouse_inputs_front: 0,
            mouse_inputs_back: 0,
            mouse_inputs_count: 0,
            mouse_has_motion_input: false,
            mouse_relative_enabled: false,
            mouse_window_x_relative: 0,
            mouse_window_y_relative: 0,
        }
    }
}
