// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `nortvars.c` / `nortvars.h`.

use crate::game::Game;
use crate::vga256d::{fill_rectangle_xy, je_bar_shade, je_pix, je_rectangle};
use crate::video::ScreenId;
use serde::{Deserialize, Serialize};

/// Mutable globals of this module (rule R3).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {}

/// A vertical bar of `num + 1` two-pixel steps upwards from (`x`, `y`).
/// Like upstream, the `u8` counter never exceeds a `num` of 255 or more.
#[doc(alias = "JE_dBar3")]
pub fn je_d_bar3(g: &mut Game, surface: ScreenId, x: i16, mut y: i16, num: i16, mut col: i16) {
    let mut z: u8 = 0;
    let mut z_wait: u8 = 2;

    col = col.wrapping_add(2);

    while i32::from(z) <= i32::from(num) {
        let (x, y32) = (i32::from(x), i32::from(y));
        je_rectangle(g, surface, x, y32 - 1, x + 8, y32, i32::from(col));
        if z_wait > 0 {
            z_wait -= 1;
        } else {
            col = col.wrapping_add(1);
            z_wait = 1;
        }
        y = y.wrapping_sub(2);
        z = z.wrapping_add(1);
    }
}

#[doc(alias = "JE_barDrawShadow")]
#[allow(clippy::too_many_arguments)]
pub fn je_bar_draw_shadow(
    g: &mut Game,
    surface: ScreenId,
    mut x: u16,
    y: u16,
    res: u16,
    col: u16,
    mut amt: u16,
    mut xsize: u16,
    mut ysize: u16,
) {
    xsize = xsize.wrapping_sub(1);
    ysize = ysize.wrapping_sub(1);

    let (y, col32) = (i32::from(y), i32::from(col));
    let (xs, ys) = (i32::from(xsize), i32::from(ysize));

    let mut z = 1;
    while z <= i32::from(amt) / i32::from(res) {
        let x32 = i32::from(x);
        je_bar_shade(g, surface, x32 + 2, y + 2, x32 + xs + 2, y + ys + 2);
        fill_rectangle_xy(g, surface, x32, y, x32 + xs, y + ys, (col32 + 12) as u8);
        fill_rectangle_xy(g, surface, x32, y, x32 + xs, y, (col32 + 13) as u8);
        je_pix(g, surface, x32, y, (col32 + 15) as u8);
        fill_rectangle_xy(
            g,
            surface,
            x32,
            y + ys,
            x32 + xs,
            y + ys,
            (col32 + 11) as u8,
        );
        x = (x32 + xs + 2) as u16;
        z += 1;
    }

    amt %= res;
    if amt > 0 {
        let x32 = i32::from(x);
        je_bar_shade(g, surface, x32 + 2, y + 2, x32 + xs + 2, y + ys + 2);
        let shade = 12 / i32::from(res) * i32::from(amt);
        fill_rectangle_xy(g, surface, x32, y, x32 + xs, y + ys, (col32 + shade) as u8);
    }
}
