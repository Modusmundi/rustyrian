// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `font.c` / `font.h`: text drawing.

use crate::fonthand::{FONT_MAP, c_str, je_text_width};
use crate::game::Game;
use crate::sprite::{
    blit_sprite_dark, blit_sprite_hv, blit_sprite_hv_blend, sprite, sprite_exists,
};
use crate::video::ScreenId;
use serde::{Deserialize, Serialize};

/// `Font`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum Font {
    /// `FONT_LARGE`.
    Large = 0,
    /// `FONT_NORMAL`.
    Normal = 1,
    /// `FONT_SMALL`.
    Small = 2,
}

pub const FONT_LARGE: Font = Font::Large;
pub const FONT_NORMAL: Font = Font::Normal;
pub const FONT_SMALL: Font = Font::Small;

/// `FontAlignment`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum FontAlignment {
    /// `ALIGN_LEFT`.
    Left = 0,
    /// `ALIGN_CENTER`.
    Center = 1,
    /// `ALIGN_RIGHT`.
    Right = 2,
}

pub const ALIGN_LEFT: FontAlignment = FontAlignment::Left;
pub const ALIGN_CENTER: FontAlignment = FontAlignment::Center;
pub const ALIGN_RIGHT: FontAlignment = FontAlignment::Right;

/// `font.c` has no mutable globals.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {}

/// `x` moved for `alignment` of `text` (the switch every `*Aligned` function
/// starts with).
fn align(g: &Game, x: i32, text: &[u8], font: Font, alignment: FontAlignment) -> i32 {
    match alignment {
        FontAlignment::Left => x,
        FontAlignment::Center => x.wrapping_sub(je_text_width(g, text, font as u32) / 2),
        FontAlignment::Right => x.wrapping_sub(je_text_width(g, text, font as u32)),
    }
}

/// The sprite of `c` in `font`, if the character has one.
fn glyph(g: &Game, font: Font, c: u8) -> Option<u32> {
    let sprite_id = i32::from(FONT_MAP[usize::from(c)]);
    (sprite_id != -1 && sprite_exists(g, font as u32, sprite_id as u32)).then_some(sprite_id as u32)
}

/// The width of glyph `id` plus one pixel of spacing.
fn advance(g: &Game, font: Font, id: u32) -> i32 {
    i32::from(sprite(g, font as u32, id).width) + 1
}

/// Text in `hue`/`value` over a shadow `shadow_dist` pixels down and right;
/// the shadow is black if `black`, otherwise a darkening.
#[doc(alias = "drawFontHvShadow")]
#[allow(clippy::too_many_arguments)]
pub fn draw_font_hv_shadow(
    g: &mut Game,
    surface: ScreenId,
    x: i32,
    y: i32,
    text: &[u8],
    font: Font,
    hue: u8,
    value: i8,
    black: bool,
    shadow_dist: i32,
) {
    let (sx, sy) = (x.wrapping_add(shadow_dist), y.wrapping_add(shadow_dist));
    draw_font_dark(g, surface, sx, sy, text, font, black);

    draw_font_hv(g, surface, x, y, text, font, hue, value);
}

#[doc(alias = "drawFontHvShadowAligned")]
#[allow(clippy::too_many_arguments)]
pub fn draw_font_hv_shadow_aligned(
    g: &mut Game,
    surface: ScreenId,
    x: i32,
    y: i32,
    text: &[u8],
    font: Font,
    alignment: FontAlignment,
    hue: u8,
    value: i8,
    black: bool,
    shadow_dist: i32,
) {
    let x = align(g, x, text, font, alignment);

    draw_font_hv_shadow(g, surface, x, y, text, font, hue, value, black, shadow_dist);
}

/// Text in `hue`/`value` over four shadows `shadow_dist` pixels away in each
/// direction.
#[doc(alias = "drawFontHvFullShadow")]
#[allow(clippy::too_many_arguments)]
pub fn draw_font_hv_full_shadow(
    g: &mut Game,
    surface: ScreenId,
    x: i32,
    y: i32,
    text: &[u8],
    font: Font,
    hue: u8,
    value: i8,
    black: bool,
    shadow_dist: i32,
) {
    let d = shadow_dist;
    draw_font_dark(g, surface, x, y.wrapping_sub(d), text, font, black);
    draw_font_dark(g, surface, x.wrapping_add(d), y, text, font, black);
    draw_font_dark(g, surface, x, y.wrapping_add(d), text, font, black);
    draw_font_dark(g, surface, x.wrapping_sub(d), y, text, font, black);

    draw_font_hv(g, surface, x, y, text, font, hue, value);
}

#[doc(alias = "drawFontHvFullShadowAligned")]
#[allow(clippy::too_many_arguments)]
pub fn draw_font_hv_full_shadow_aligned(
    g: &mut Game,
    surface: ScreenId,
    x: i32,
    y: i32,
    text: &[u8],
    font: Font,
    alignment: FontAlignment,
    hue: u8,
    value: i8,
    black: bool,
    shadow_dist: i32,
) {
    let x = align(g, x, text, font, alignment);

    draw_font_hv_full_shadow(g, surface, x, y, text, font, hue, value, black, shadow_dist);
}

/// Text in `hue`/`value`; each `~` toggles highlighting (`value + 4`).
#[doc(alias = "drawFontHv")]
#[allow(clippy::too_many_arguments)]
pub fn draw_font_hv(
    g: &mut Game,
    surface: ScreenId,
    mut x: i32,
    y: i32,
    text: &[u8],
    font: Font,
    hue: u8,
    mut value: i8,
) {
    let mut highlight = false;

    for &c in c_str(text) {
        match c {
            b' ' => x = x.wrapping_add(6),
            b'~' => {
                highlight = !highlight;
                value = if highlight {
                    value.wrapping_add(4)
                } else {
                    value.wrapping_sub(4)
                };
            }
            _ => {
                if let Some(id) = glyph(g, font, c) {
                    blit_sprite_hv(g, surface, x, y, font as u32, id, hue, value);

                    x = x.wrapping_add(advance(g, font, id));
                }
            }
        }
    }
}

#[doc(alias = "drawFontHvAligned")]
#[allow(clippy::too_many_arguments)]
pub fn draw_font_hv_aligned(
    g: &mut Game,
    surface: ScreenId,
    x: i32,
    y: i32,
    text: &[u8],
    font: Font,
    alignment: FontAlignment,
    hue: u8,
    value: i8,
) {
    let x = align(g, x, text, font, alignment);

    draw_font_hv(g, surface, x, y, text, font, hue, value);
}

/// Text blended into the surface in `hue`/`value`.
#[doc(alias = "drawFontHvBlend")]
#[allow(clippy::too_many_arguments)]
pub fn draw_font_hv_blend(
    g: &mut Game,
    surface: ScreenId,
    mut x: i32,
    y: i32,
    text: &[u8],
    font: Font,
    hue: u8,
    value: i8,
) {
    for &c in c_str(text) {
        match c {
            b' ' => x = x.wrapping_add(6),
            b'~' => {}
            _ => {
                if let Some(id) = glyph(g, font, c) {
                    blit_sprite_hv_blend(g, surface, x, y, font as u32, id, hue, value);

                    x = x.wrapping_add(advance(g, font, id));
                }
            }
        }
    }
}

#[doc(alias = "drawFontHvBlendAligned")]
#[allow(clippy::too_many_arguments)]
pub fn draw_font_hv_blend_aligned(
    g: &mut Game,
    surface: ScreenId,
    x: i32,
    y: i32,
    text: &[u8],
    font: Font,
    alignment: FontAlignment,
    hue: u8,
    value: i8,
) {
    let x = align(g, x, text, font, alignment);

    draw_font_hv_blend(g, surface, x, y, text, font, hue, value);
}

/// Text as black (`black`) or as a darkening of the surface.
#[doc(alias = "drawFontDark")]
pub fn draw_font_dark(
    g: &mut Game,
    surface: ScreenId,
    mut x: i32,
    y: i32,
    text: &[u8],
    font: Font,
    black: bool,
) {
    for &c in c_str(text) {
        match c {
            b' ' => x = x.wrapping_add(6),
            b'~' => {}
            _ => {
                if let Some(id) = glyph(g, font, c) {
                    blit_sprite_dark(g, surface, x, y, font as u32, id, black);

                    x = x.wrapping_add(advance(g, font, id));
                }
            }
        }
    }
}

#[doc(alias = "drawFontDarkAligned")]
#[allow(clippy::too_many_arguments)]
pub fn draw_font_dark_aligned(
    g: &mut Game,
    surface: ScreenId,
    x: i32,
    y: i32,
    text: &[u8],
    font: Font,
    alignment: FontAlignment,
    black: bool,
) {
    let x = align(g, x, text, font, alignment);

    draw_font_dark(g, surface, x, y, text, font, black);
}
