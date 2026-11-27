// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `keyboard.c` / `keyboard.h`.

use crate::Game;
use crate::game::{Flow, Halt};
use crate::joystick::push_joysticks_as_keyboard;
use crate::network::network_keep_alive;
use crate::nortsong::get_frame_count_ticks;
use crate::platform::Event;
use crate::video::{
    VGA_HEIGHT, VGA_WIDTH, map_window_point_to_screen, scale_window_distance_to_screen,
    toggle_fullscreen, video_on_win_resize,
};
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

/// `SDL_HINT_MOUSE_RELATIVE_SYSTEM_SCALE`.
pub const SDL_HINT_MOUSE_RELATIVE_SYSTEM_SCALE: &str = "SDL_MOUSE_RELATIVE_SYSTEM_SCALE";
/// `SDL_DISABLE`.
pub const SDL_DISABLE: i32 = 0;
/// `SDL_ENABLE`.
pub const SDL_ENABLE: i32 = 1;
/// `KMOD_NONE`.
pub const KMOD_NONE: u16 = 0x0000;
/// `KMOD_ALT`.
pub const KMOD_ALT: u16 = 0x0100 | 0x0200;
/// `SDL_SCANCODE_RETURN`.
pub const SDL_SCANCODE_RETURN: u16 = 40;

/// `SDL_BUTTON(X)`: `1 << ((X)-1)` as C evaluates it in `int`.
fn sdl_button(x: u8) -> i32 {
    1i32.wrapping_shl((i32::from(x) - 1) as u32)
}

pub fn init_keyboard(g: &mut Game) {
    g.platform.stop_text_input();

    g.platform.show_cursor(SDL_DISABLE);

    g.platform
        .set_hint(SDL_HINT_MOUSE_RELATIVE_SYSTEM_SCALE, "1");
}

#[doc(alias = "keyboardHasInput")]
pub fn keyboard_has_input(g: &mut Game) -> bool {
    g.keyboard.keyboard_inputs_count > 0
}

#[doc(alias = "keyboardGetInput")]
pub fn keyboard_get_input(g: &mut Game, out_input: Option<&mut KeyboardInput>) -> bool {
    let k = &mut g.keyboard;
    if k.keyboard_inputs_count > 0 {
        debug_assert!(k.keyboard_inputs_front < k.keyboard_inputs.len());
        if let Some(out) = out_input {
            *out = k.keyboard_inputs[k.keyboard_inputs_front];
        }
        k.keyboard_inputs_front = if k.keyboard_inputs_front == k.keyboard_inputs.len() - 1 {
            0
        } else {
            k.keyboard_inputs_front + 1
        };
        k.keyboard_inputs_count -= 1;
        return true;
    }

    false
}

#[doc(alias = "keyboardClearInput")]
pub fn keyboard_clear_input(g: &mut Game) {
    g.keyboard.keyboard_inputs_front = 0;
    g.keyboard.keyboard_inputs_back = 0;
    g.keyboard.keyboard_inputs_count = 0;
}

#[doc(alias = "mouseHasInput")]
pub fn mouse_has_input(g: &mut Game, flags: InputFlags) -> bool {
    g.keyboard.mouse_inputs_count > 0
        || ((flags & INPUT_NO_MOTION) == 0 && g.keyboard.mouse_has_motion_input)
}

#[doc(alias = "mouseGetInput")]
pub fn mouse_get_input(
    g: &mut Game,
    flags: InputFlags,
    out_input: Option<&mut MouseInput>,
) -> bool {
    let k = &mut g.keyboard;
    if k.mouse_inputs_count > 0 {
        debug_assert!(k.mouse_inputs_front < k.mouse_inputs.len());
        if let Some(out) = out_input {
            *out = k.mouse_inputs[k.mouse_inputs_front];
        }
        k.mouse_inputs_front = if k.mouse_inputs_front == k.mouse_inputs.len() - 1 {
            0
        } else {
            k.mouse_inputs_front + 1
        };
        k.mouse_inputs_count -= 1;
        return true;
    }

    if (flags & INPUT_NO_MOTION) == 0 && k.mouse_has_motion_input {
        if let Some(out) = out_input {
            *out = MouseInput {
                x: k.mouse_x,
                y: k.mouse_y,
                button: 0,
            };
        }
        k.mouse_has_motion_input = false;
        return true;
    }

    false
}

#[doc(alias = "mouseClearInput")]
pub fn mouse_clear_input(g: &mut Game) {
    g.keyboard.mouse_inputs_front = 0;
    g.keyboard.mouse_inputs_back = 0;
    g.keyboard.mouse_inputs_count = 0;

    g.keyboard.mouse_has_motion_input = false;
}

#[doc(alias = "mouseSetRelative")]
pub fn mouse_set_relative(g: &mut Game, enable: bool) {
    g.platform
        .set_relative_mouse_mode(enable && g.keyboard.window_has_focus);

    g.keyboard.mouse_relative_enabled = enable;

    g.keyboard.mouse_window_x_relative = 0;
    g.keyboard.mouse_window_y_relative = 0;
}

#[doc(alias = "mouseGetRelativePosition")]
pub fn mouse_get_relative_position(g: &mut Game, out_x: &mut i32, out_y: &mut i32) {
    let (mut x, mut y) = (
        g.keyboard.mouse_window_x_relative,
        g.keyboard.mouse_window_y_relative,
    );
    scale_window_distance_to_screen(g, &mut x, &mut y);
    g.keyboard.mouse_window_x_relative = x;
    g.keyboard.mouse_window_y_relative = y;
    *out_x = g.keyboard.mouse_window_x_relative;
    *out_y = g.keyboard.mouse_window_y_relative;

    g.keyboard.mouse_window_x_relative = 0;
    g.keyboard.mouse_window_y_relative = 0;
}

/// Appends to the `keyboardInputs` ring if it has room.
fn push_keyboard_input(k: &mut State, input: KeyboardInput) {
    if k.keyboard_inputs_count < k.keyboard_inputs.len() {
        debug_assert!(k.keyboard_inputs_back < k.keyboard_inputs.len());
        k.keyboard_inputs[k.keyboard_inputs_back] = input;
        k.keyboard_inputs_back = if k.keyboard_inputs_back == k.keyboard_inputs.len() - 1 {
            0
        } else {
            k.keyboard_inputs_back + 1
        };
        k.keyboard_inputs_count += 1;
    }
}

/// `handleSdlEvents`; `SDL_QUIT` is `exit(0)`.
#[doc(alias = "handleSdlEvents")]
pub fn handle_sdl_events(g: &mut Game) -> Flow {
    while let Some(ev) = g.platform.poll_event() {
        match ev {
            Event::WindowFocusLost => {
                g.keyboard.window_has_focus = false;

                mouse_set_relative(g, g.keyboard.mouse_relative_enabled);
            }
            Event::WindowFocusGained => {
                g.keyboard.window_has_focus = true;

                mouse_set_relative(g, g.keyboard.mouse_relative_enabled);
            }
            Event::WindowResized { .. } => {
                video_on_win_resize(g);
            }
            Event::KeyDown {
                scancode,
                keycode,
                modifiers,
                repeat,
            } => {
                if modifiers & KMOD_ALT != 0 && scancode == SDL_SCANCODE_RETURN {
                    toggle_fullscreen(g);
                    continue;
                }

                // SDL scancodes are below SDL_NUM_SCANCODES (D-UB-KEYBOARD-SCANCODE).
                if !repeat && let Some(k) = g.keyboard.keysactive.get_mut(usize::from(scancode)) {
                    *k = true;
                }

                for (down, &sym) in g.keyboard.lord_key_syms_down.iter_mut().zip(&LORD_KEY_SYMS) {
                    *down |= keycode == sym;
                }

                push_keyboard_input(
                    &mut g.keyboard,
                    KeyboardInput {
                        sym: keycode,
                        scancode,
                        r#mod: modifiers,
                        ch: 0,
                    },
                );

                g.mouse.mouse_inactive = true;
            }
            Event::KeyUp {
                scancode, keycode, ..
            } => {
                // SDL scancodes are below SDL_NUM_SCANCODES (D-UB-KEYBOARD-SCANCODE).
                if let Some(k) = g.keyboard.keysactive.get_mut(usize::from(scancode)) {
                    *k = false;
                }

                for (down, &sym) in g.keyboard.lord_key_syms_down.iter_mut().zip(&LORD_KEY_SYMS) {
                    *down &= keycode != sym;
                }
            }
            Event::TextInput(s) => {
                // `ev.text.text`: char[32], NUL-terminated, zero-filled.
                let mut text = [0u8; 32];
                let n = s.len().min(31);
                text[..n].copy_from_slice(&s.as_bytes()[..n]);

                let mut i = 0;
                while i < text.len() {
                    // Decode codepoint from UTF-8.
                    let mut cp = u16::from(text[i]);
                    if cp == 0 {
                        break;
                    } else if cp < 0x80 {
                        // ASCII.
                    } else if cp < 0xC0 {
                        // Invalid.
                        i += 1;
                        continue;
                    } else if cp < 0xE0 {
                        if i + 1 >= text.len() {
                            i += 1;
                            continue;
                        }

                        cp &= 0x1F;
                        i += 1;
                        cp = (cp << 6) | u16::from(text[i] & 0x3F);
                    } else if cp < 0xF0 {
                        if i + 2 >= text.len() {
                            i += 1;
                            continue;
                        }

                        cp &= 0x0F;
                        i += 1;
                        cp = (cp << 6) | u16::from(text[i] & 0x3F);
                        i += 1;
                        cp = (cp << 6) | u16::from(text[i] & 0x3F);
                    } else {
                        // Outside the BMP.
                        i += 1;
                        continue;
                    }

                    // Map codepoint to CP437 character.
                    let mut ch: u8 = 0;
                    if cp < 0x80 {
                        ch = cp as u8;
                    } else {
                        if let Some(j) = UCS_MAP.iter().position(|&u| u == cp) {
                            ch = 0x80 + j as u8;
                        }

                        if ch == 0 {
                            i += 1;
                            continue;
                        }
                    }

                    push_keyboard_input(
                        &mut g.keyboard,
                        KeyboardInput {
                            sym: -1,            // Text; not a key.
                            scancode: u16::MAX, // Text; not a key.
                            r#mod: KMOD_NONE,
                            ch,
                        },
                    );
                    i += 1;
                }
            }
            Event::MouseMotion { x, y, xrel, yrel } => {
                let (mut mx, mut my) = (x, y);
                map_window_point_to_screen(g, &mut mx, &mut my);
                g.keyboard.mouse_x = mx;
                g.keyboard.mouse_y = my;

                if g.keyboard.mouse_relative_enabled && g.keyboard.window_has_focus {
                    g.keyboard.mouse_window_x_relative =
                        g.keyboard.mouse_window_x_relative.wrapping_add(xrel);
                    g.keyboard.mouse_window_y_relative =
                        g.keyboard.mouse_window_y_relative.wrapping_add(yrel);
                }

                // Show system mouse pointer if outside screen.
                g.platform.show_cursor(
                    if !(0..VGA_WIDTH).contains(&mx) || !(0..VGA_HEIGHT).contains(&my) {
                        SDL_ENABLE
                    } else {
                        SDL_DISABLE
                    },
                );

                if xrel != 0 || yrel != 0 {
                    g.keyboard.mouse_has_motion_input = true;

                    g.mouse.mouse_inactive = false;
                }
            }
            Event::MouseButtonDown { button, x, y } => {
                let (mut bx, mut by) = (x, y);
                map_window_point_to_screen(g, &mut bx, &mut by);

                let k = &mut g.keyboard;
                if k.mouse_inputs_count < k.mouse_inputs.len() {
                    debug_assert!(k.mouse_inputs_back < k.mouse_inputs.len());
                    k.mouse_inputs[k.mouse_inputs_back] = MouseInput {
                        button,
                        x: bx,
                        y: by,
                    };
                    k.mouse_inputs_back = if k.mouse_inputs_back == k.mouse_inputs.len() - 1 {
                        0
                    } else {
                        k.mouse_inputs_back + 1
                    };
                    k.mouse_inputs_count += 1;
                }

                k.mouse_buttons_down = (i32::from(k.mouse_buttons_down) | sdl_button(button)) as u8;

                g.mouse.mouse_inactive = false;
            }
            Event::MouseButtonUp { button, x, y } => {
                let (mut bx, mut by) = (x, y);
                map_window_point_to_screen(g, &mut bx, &mut by);

                g.keyboard.mouse_buttons_down =
                    (i32::from(g.keyboard.mouse_buttons_down) & !sdl_button(button)) as u8;
            }
            Event::Quit => {
                return Err(Halt { code: 0 });
            }
        }
    }
    Ok(())
}

#[doc(alias = "hasInput")]
pub fn has_input(g: &mut Game, flags: InputFlags) -> bool {
    keyboard_has_input(g) || mouse_has_input(g, flags)
}

#[doc(alias = "getInput")]
pub fn get_input(g: &mut Game) -> bool {
    keyboard_get_input(g, None) || mouse_get_input(g, INPUT_NO_MOTION, None)
}

/// One poll of the wait loops: network keep-alive, joystick-as-keyboard input
/// and OS events.
fn poll_once(g: &mut Game) -> Flow {
    network_keep_alive(g)?;

    push_joysticks_as_keyboard(g);
    handle_sdl_events(g)
}

#[doc(alias = "waitUntilHasInput")]
pub fn wait_until_has_input(g: &mut Game, flags: InputFlags) -> Flow {
    loop {
        poll_once(g)?;

        if has_input(g, flags) {
            return Ok(());
        }

        g.platform.delay(SDL_POLL_INTERVAL);
    }
}

#[doc(alias = "waitUntilGetInput")]
pub fn wait_until_get_input(g: &mut Game) -> Flow {
    loop {
        poll_once(g)?;

        if get_input(g) {
            return Ok(());
        }

        g.platform.delay(SDL_POLL_INTERVAL);
    }
}

#[doc(alias = "waitUntilElapsed")]
pub fn wait_until_elapsed(g: &mut Game) -> Flow {
    loop {
        poll_once(g)?;

        let delay = get_frame_count_ticks(g);
        if delay == 0 {
            return Ok(());
        }

        g.platform.delay(delay.min(SDL_POLL_INTERVAL));
    }
}

#[doc(alias = "waitUntilHasInputOrElapsed")]
pub fn wait_until_has_input_or_elapsed(g: &mut Game) -> Flow<bool> {
    loop {
        poll_once(g)?;

        if has_input(g, INPUT_NO_MOTION) {
            return Ok(true);
        }

        let delay = get_frame_count_ticks(g);
        if delay == 0 {
            return Ok(false);
        }

        g.platform.delay(delay.min(SDL_POLL_INTERVAL));
    }
}

#[doc(alias = "waitUntilGetInputOrElapsed")]
pub fn wait_until_get_input_or_elapsed(g: &mut Game) -> Flow<bool> {
    loop {
        poll_once(g)?;

        if get_input(g) {
            return Ok(true);
        }

        let delay = get_frame_count_ticks(g);
        if delay == 0 {
            return Ok(false);
        }

        g.platform.delay(delay.min(SDL_POLL_INTERVAL));
    }
}
