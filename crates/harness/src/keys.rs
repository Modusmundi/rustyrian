// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) rustyrian contributors

//! Script key names → SDL2 scancodes and US-layout keycodes.

const SCANCODE_MASK: i32 = 1 << 30;

/// `(scancode, keycode)` for a script key name, or for a decimal scancode.
pub fn lookup(name: &str) -> Option<(u16, i32)> {
    if let Ok(sc) = name.parse::<u16>() {
        if usize::from(sc) >= 512 {
            return None;
        }
        return Some((sc, default_keycode(sc)));
    }
    let b = name.as_bytes();
    if b.len() == 1 && b[0].is_ascii_uppercase() {
        let sc = 4 + u16::from(b[0] - b'A');
        return Some((sc, default_keycode(sc)));
    }
    if b.len() == 1 && b[0].is_ascii_digit() {
        let sc = if b[0] == b'0' {
            39
        } else {
            30 + u16::from(b[0] - b'1')
        };
        return Some((sc, default_keycode(sc)));
    }
    if let Some(n) = name.strip_prefix('F').and_then(|n| n.parse::<u16>().ok())
        && (1..=12).contains(&n)
    {
        let sc = 57 + n;
        return Some((sc, default_keycode(sc)));
    }
    let sc = match name {
        "RETURN" => 40,
        "ESCAPE" => 41,
        "BACKSPACE" => 42,
        "TAB" => 43,
        "SPACE" => 44,
        "RIGHT" => 79,
        "LEFT" => 80,
        "DOWN" => 81,
        "UP" => 82,
        "LCTRL" => 224,
        "LSHIFT" => 225,
        "LALT" => 226,
        "RCTRL" => 228,
        "RSHIFT" => 229,
        "RALT" => 230,
        _ => return None,
    };
    Some((sc, default_keycode(sc)))
}

/// SDL2's default (US) keymap for the scancodes the table names, otherwise
/// `SDL_SCANCODE_TO_KEYCODE`.
pub fn default_keycode(sc: u16) -> i32 {
    match sc {
        4..=29 => i32::from(b'a') + i32::from(sc - 4),
        30..=38 => i32::from(b'1') + i32::from(sc - 30),
        39 => i32::from(b'0'),
        40 => 13,
        41 => 27,
        42 => 8,
        43 => 9,
        44 => 32,
        _ => i32::from(sc) | SCANCODE_MASK,
    }
}

/// `SDL_Keymod` bit for a modifier scancode, or 0.
pub fn modifier_bit(sc: u16) -> u16 {
    match sc {
        225 => 0x0001, // KMOD_LSHIFT
        229 => 0x0002, // KMOD_RSHIFT
        224 => 0x0040, // KMOD_LCTRL
        228 => 0x0080, // KMOD_RCTRL
        226 => 0x0100, // KMOD_LALT
        230 => 0x0200, // KMOD_RALT
        227 => 0x0400, // KMOD_LGUI
        231 => 0x0800, // KMOD_RGUI
        _ => 0,
    }
}
