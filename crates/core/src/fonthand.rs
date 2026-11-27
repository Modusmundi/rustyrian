// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `fonthand.c` / `fonthand.h`.
//!
//! `const char *` text parameters are byte slices read up to the first NUL
//! (or their end).

use crate::game::Game;
use crate::sprite::{
    TINY_FONT, blit_sprite_dark, blit_sprite_hv, blit_sprite_hv_blend, blit_sprite_hv_unsafe,
    sprite, sprite_exists,
};
use crate::video::ScreenId;
use serde::{Deserialize, Serialize};

pub const PART_SHADE: u32 = 0;
pub const FULL_SHADE: u32 = 1;
pub const DARKEN: u32 = 2;
pub const TRICK: u32 = 3;
pub const NO_SHADE: u32 = 255;

/// `fontMap`: mapping from CP437 to font sprite index (`[33..168]`).
#[rustfmt::skip]
pub const FONT_MAP: [i8; 256] = [
     -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,
     -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,
     -1,  26,  33,  60,  61,  62,  -1,  32,  64,  65,  63,  84,  29,  83,  28,  80,
     79,  70,  71,  72,  73,  74,  75,  76,  77,  78,  31,  30,  -1,  85,  -1,  27,
     -1,   0,   1,   2,   3,   4,   5,   6,   7,   8,   9,  10,  11,  12,  13,  14,
     15,  16,  17,  18,  19,  20,  21,  22,  23,  24,  25,  68,  82,  69,  -1,  -1,
     -1,  34,  35,  36,  37,  38,  39,  40,  41,  42,  43,  44,  45,  46,  47,  48,
     49,  50,  51,  52,  53,  54,  55,  56,  57,  58,  59,  66,  81,  67,  -1,  -1,

     86,  87,  88,  89,  90,  91,  92,  93,  94,  95,  96,  97,  98,  99, 100, 101,
    102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117,
    118, 119, 120, 121, 122, 123, 124, 125, 126,  -1,  -1,  -1,  -1,  -1,  -1,  -1,
     -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,
     -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,
     -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,
     -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,
     -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,  -1,
];

/// Mutable globals of this module (rule R3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub text_glow_font: u8,
    pub text_glow_brightness: u8,
    pub level_warning_display: bool,
    pub level_warning_lines: u8,
    /// `[1..10] of string [60]`.
    #[serde(with = "crate::serde_big::d2")]
    pub level_warning_text: [[u8; 61]; 10],
    pub warning_red: bool,
    pub warning_sound_delay: u8,
    pub armor_ship_delay: u16,
    pub warning_col: u8,
    pub warning_col_change: i8,
}

impl Default for State {
    fn default() -> Self {
        State {
            text_glow_font: 0,
            text_glow_brightness: 6,
            level_warning_display: false,
            level_warning_lines: 0,
            level_warning_text: [[0; 61]; 10],
            warning_red: false,
            warning_sound_delay: 0,
            armor_ship_delay: 0,
            warning_col: 0,
            warning_col_change: 0,
        }
    }
}

/// The bytes of a C string: up to (not including) the first NUL, or all of
/// `s` if it has none.
pub fn c_str(s: &[u8]) -> &[u8] {
    s.iter().position(|&c| c == 0).map_or(s, |n| &s[..n])
}

/// `fontMap[c]` as C's `int sprite_id`.
fn sprite_id(c: u8) -> i32 {
    i32::from(FONT_MAP[usize::from(c)])
}

/// The width of a font sprite plus one pixel of spacing.
fn advance(g: &Game, font: u32, sprite_id: i32) -> i32 {
    i32::from(sprite(g, font, sprite_id as u32).width) + 1
}

/// Text with a darkened drop shadow, in the default brightness.
#[doc(alias = "JE_dString")]
pub fn je_d_string(g: &mut Game, screen: ScreenId, mut x: i32, y: i32, s: &[u8], font: u32) {
    const DEFAULT_BRIGHTNESS: i32 = -3;

    let mut bright = 0;

    for &c in c_str(s) {
        let sprite_id = sprite_id(c);

        match c {
            b' ' => x = x.wrapping_add(6),
            b'~' => bright = if bright == 0 { 2 } else { 0 },
            _ => {
                if sprite_id != -1 {
                    let id = sprite_id as u32;
                    blit_sprite_dark(
                        g,
                        screen,
                        x.wrapping_add(2),
                        y.wrapping_add(2),
                        font,
                        id,
                        false,
                    );
                    blit_sprite_hv_unsafe(
                        g,
                        screen,
                        x,
                        y,
                        font,
                        id,
                        0xf,
                        (DEFAULT_BRIGHTNESS + bright) as i8,
                    );

                    x = x.wrapping_add(advance(g, font, sprite_id));
                }
            }
        }
    }
}

#[doc(alias = "JE_fontCenter")]
pub fn je_font_center(g: &Game, s: &[u8], font: u32) -> i32 {
    160 - (je_text_width(g, s, font) / 2)
}

#[doc(alias = "JE_textWidth")]
pub fn je_text_width(g: &Game, s: &[u8], font: u32) -> i32 {
    let mut x: i32 = 0;

    for &c in c_str(s) {
        let sprite_id = sprite_id(c);

        if c == b' ' {
            x = x.wrapping_add(6);
        } else if sprite_id != -1 {
            x = x.wrapping_add(advance(g, font, sprite_id));
        }
    }

    x
}

#[doc(alias = "JE_textShade")]
#[allow(clippy::too_many_arguments)]
pub fn je_text_shade(
    g: &mut Game,
    screen: ScreenId,
    x: i32,
    y: i32,
    s: &[u8],
    colorbank: u32,
    brightness: i32,
    shadetype: u32,
) {
    match shadetype {
        PART_SHADE => {
            je_out_text(g, screen, x.wrapping_add(1), y.wrapping_add(1), s, 0, -1);
            je_out_text(g, screen, x, y, s, colorbank, brightness);
        }
        FULL_SHADE => {
            je_out_text(g, screen, x.wrapping_sub(1), y, s, 0, -1);
            je_out_text(g, screen, x.wrapping_add(1), y, s, 0, -1);
            je_out_text(g, screen, x, y.wrapping_sub(1), s, 0, -1);
            je_out_text(g, screen, x, y.wrapping_add(1), s, 0, -1);
            je_out_text(g, screen, x, y, s, colorbank, brightness);
        }
        DARKEN => {
            je_out_text_and_darken(
                g,
                screen,
                x.wrapping_add(1),
                y.wrapping_add(1),
                s,
                colorbank,
                brightness as u32,
                TINY_FONT as u32,
            );
        }
        TRICK => {
            je_out_text_modify(
                g,
                screen,
                x,
                y,
                s,
                colorbank,
                brightness as u32,
                TINY_FONT as u32,
            );
        }
        _ => {}
    }
}

#[doc(alias = "JE_outText")]
pub fn je_out_text(
    g: &mut Game,
    screen: ScreenId,
    mut x: i32,
    y: i32,
    s: &[u8],
    colorbank: u32,
    brightness: i32,
) {
    let tiny = TINY_FONT as u32;
    let mut bright = 0;

    for &c in c_str(s) {
        let sprite_id = sprite_id(c);

        match c {
            b' ' => x = x.wrapping_add(6),
            b'~' => bright = if bright == 0 { 4 } else { 0 },
            _ => {
                if sprite_id != -1 && sprite_exists(g, tiny, sprite_id as u32) {
                    let id = sprite_id as u32;
                    if brightness >= 0 {
                        let value = brightness.wrapping_add(bright) as i8;
                        blit_sprite_hv_unsafe(g, screen, x, y, tiny, id, colorbank as u8, value);
                    } else {
                        blit_sprite_dark(g, screen, x, y, tiny, id, true);
                    }

                    x = x.wrapping_add(advance(g, tiny, sprite_id));
                }
            }
        }
    }
}

#[doc(alias = "JE_outTextModify")]
#[allow(clippy::too_many_arguments)]
pub fn je_out_text_modify(
    g: &mut Game,
    screen: ScreenId,
    mut x: i32,
    y: i32,
    s: &[u8],
    filter: u32,
    brightness: u32,
    font: u32,
) {
    for &c in c_str(s) {
        let sprite_id = sprite_id(c);

        if c == b' ' {
            x = x.wrapping_add(6);
        } else if sprite_id != -1 {
            let id = sprite_id as u32;
            blit_sprite_hv_blend(g, screen, x, y, font, id, filter as u8, brightness as i8);

            x = x.wrapping_add(advance(g, font, sprite_id));
        }
    }
}

#[doc(alias = "JE_outTextAdjust")]
#[allow(clippy::too_many_arguments)]
pub fn je_out_text_adjust(
    g: &mut Game,
    screen: ScreenId,
    mut x: i32,
    y: i32,
    s: &[u8],
    filter: u32,
    brightness: i32,
    font: u32,
    shadow: bool,
) {
    let mut bright = 0;

    for &c in c_str(s) {
        let sprite_id = sprite_id(c);

        match c {
            b' ' => x = x.wrapping_add(6),
            b'~' => bright = if bright == 0 { 4 } else { 0 },
            _ => {
                if sprite_id != -1 && sprite_exists(g, TINY_FONT as u32, sprite_id as u32) {
                    let id = sprite_id as u32;
                    if shadow {
                        blit_sprite_dark(
                            g,
                            screen,
                            x.wrapping_add(2),
                            y.wrapping_add(2),
                            font,
                            id,
                            false,
                        );
                    }
                    let value = brightness.wrapping_add(bright) as i8;
                    blit_sprite_hv(g, screen, x, y, font, id, filter as u8, value);

                    x = x.wrapping_add(advance(g, font, sprite_id));
                }
            }
        }
    }
}

#[doc(alias = "JE_outTextAndDarken")]
#[allow(clippy::too_many_arguments)]
pub fn je_out_text_and_darken(
    g: &mut Game,
    screen: ScreenId,
    mut x: i32,
    y: i32,
    s: &[u8],
    colorbank: u32,
    brightness: u32,
    font: u32,
) {
    let mut bright: u32 = 0;

    for &c in c_str(s) {
        let sprite_id = sprite_id(c);

        match c {
            b' ' => x = x.wrapping_add(6),
            b'~' => bright = if bright == 0 { 4 } else { 0 },
            _ => {
                if sprite_id != -1 && sprite_exists(g, TINY_FONT as u32, sprite_id as u32) {
                    let id = sprite_id as u32;
                    blit_sprite_dark(
                        g,
                        screen,
                        x.wrapping_add(1),
                        y.wrapping_add(1),
                        font,
                        id,
                        false,
                    );
                    let value = brightness.wrapping_add(bright) as i8;
                    blit_sprite_hv_unsafe(g, screen, x, y, font, id, colorbank as u8, value);

                    x = x.wrapping_add(advance(g, font, sprite_id));
                }
            }
        }
    }
}
