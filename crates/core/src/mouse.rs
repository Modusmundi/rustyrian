// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `mouse.c` / `mouse.h`.

use crate::Game;
use crate::sdl::Surface;
use crate::sprite::{blit_sprite2x2_clip, blit_sprite2x2_filter_clip};
use crate::varz::Sprite2Ref;
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

/// `JE_drawShapeTypeOne(x, y, shape)` on `*VGAScreen` (passed as `screen`).
#[doc(alias = "JE_drawShapeTypeOne")]
pub fn je_draw_shape_type_one(
    screen: &mut Surface,
    x: u16,
    y: u16,
    shape: &[u8; MOUSE_GRAB_SHAPE_LEN],
) {
    let pitch = screen.pitch as usize;
    let mut s = usize::from(y) * pitch + usize::from(x);
    let s_limit = screen.h as usize * pitch;
    let mut p = 0;

    for _yloop in 0..28 {
        for _xloop in 0..24 {
            if s >= s_limit {
                return;
            }
            screen.pixels[s] = shape[p];
            s += 1;
            p += 1;
        }
        s -= 24;
        s += pitch;
    }
}

/// `JE_grabShapeTypeOne(x, y, shape)` from `*VGAScreen` (passed as `screen`).
#[doc(alias = "JE_grabShapeTypeOne")]
pub fn je_grab_shape_type_one(
    screen: &Surface,
    x: u16,
    y: u16,
    shape: &mut [u8; MOUSE_GRAB_SHAPE_LEN],
) {
    let pitch = screen.pitch as usize;
    let mut s = usize::from(y) * pitch + usize::from(x);
    let s_limit = screen.h as usize * pitch;
    let mut p = 0;

    for _yloop in 0..28 {
        for _xloop in 0..24 {
            if s >= s_limit {
                return;
            }
            shape[p] = screen.pixels[s];
            s += 1;
            p += 1;
        }
        s -= 24;
        s += pitch;
    }
}

/// `MousePointerSpriteInfo`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MousePointerSpriteInfo {
    pub index: u16,
    pub x: u8,
    pub y: u8,
    pub w: u8,
    pub h: u8,
    pub fx: u8,
    pub fy: u8,
}

const fn sprite_info(
    index: u16,
    x: u8,
    y: u8,
    w: u8,
    h: u8,
    fx: u8,
    fy: u8,
) -> MousePointerSpriteInfo {
    MousePointerSpriteInfo {
        index,
        x,
        y,
        w,
        h,
        fx,
        fy,
    }
}

/// `mousePointerSprites` (fka `mouseCursorGr`), indexed by `mouseCursor`.
pub const MOUSE_POINTER_SPRITES: [MousePointerSpriteInfo; 5] = [
    sprite_info(273, 0, 0, 11, 16, 0, 0),
    sprite_info(275, 0, 0, 21, 16, 10, 8),
    sprite_info(277, 0, 0, 21, 16, 10, 7),
    sprite_info(279, 0, 0, 16, 21, 8, 10),
    sprite_info(281, 8, 0, 16, 21, 7, 10),
];

/// The common head of `JE_mouseStart`/`JE_mouseStartFilter`: saves the
/// screen under the pointer and returns where the pointer sprite goes, or
/// `None` while the mouse is inactive.
fn mouse_start_grab(g: &mut Game) -> Option<(i32, i32, u32)> {
    let sprite_info = MOUSE_POINTER_SPRITES[usize::from(g.mouse.mouse_cursor)];
    let (fx, fy) = (i32::from(sprite_info.fx), i32::from(sprite_info.fy));
    let (mouse_x, mouse_y) = (g.keyboard.mouse_x, g.keyboard.mouse_y);

    g.mouse.mouse_grab_x = (fx
        .max(mouse_x)
        .min(320 - (i32::from(sprite_info.w) - fx))
        .wrapping_sub(fx)) as u16;
    g.mouse.mouse_grab_y = (fy
        .max(mouse_y)
        .min(200 - (i32::from(sprite_info.h) - fy))
        .wrapping_sub(fy)) as u16;

    let m = &mut g.mouse;
    je_grab_shape_type_one(
        g.video.vga(),
        m.mouse_grab_x,
        m.mouse_grab_y,
        &mut m.mouse_grab_shape,
    );

    if g.mouse.mouse_inactive {
        return None;
    }
    let x = mouse_x
        .wrapping_sub(i32::from(sprite_info.x))
        .wrapping_sub(fx);
    let y = mouse_y
        .wrapping_sub(i32::from(sprite_info.y))
        .wrapping_sub(fy);
    Some((x, y, u32::from(sprite_info.index)))
}

/// `JE_mouseStart` (FKA NewShape.mouseStart).
#[doc(alias = "JE_mouseStart")]
pub fn je_mouse_start(g: &mut Game) {
    if !g.mouse.has_mouse {
        return;
    }

    if let Some((x, y, index)) = mouse_start_grab(g) {
        let screen = g.video.vga_screen_id();
        blit_sprite2x2_clip(g, screen, x, y, Sprite2Ref::ShopSpriteSheet, index);
    }
}

#[doc(alias = "JE_mouseStartFilter")]
pub fn je_mouse_start_filter(g: &mut Game, filter: u8) {
    if !g.mouse.has_mouse {
        return;
    }

    if let Some((x, y, index)) = mouse_start_grab(g) {
        let screen = g.video.vga_screen_id();
        blit_sprite2x2_filter_clip(g, screen, x, y, Sprite2Ref::ShopSpriteSheet, index, filter);
    }
}

/// `JE_mouseReplace` (FKA NewShape.MouseReplace).
#[doc(alias = "JE_mouseReplace")]
pub fn je_mouse_replace(g: &mut Game) {
    if !g.mouse.has_mouse {
        return;
    }

    let m = &g.mouse;
    je_draw_shape_type_one(
        g.video.vga_mut(),
        m.mouse_grab_x,
        m.mouse_grab_y,
        &m.mouse_grab_shape,
    );
}
