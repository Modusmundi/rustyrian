// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `vga256d.c` / `vga256d.h`.
//!
//! Upstream clips these primitives only against the right and bottom edges;
//! a pixel index outside the surface (negative coordinates, or a line that
//! runs past the end) is skipped here, where C would write outside the pixel
//! buffer (divergence `D-UB-VGA-1`). Every in-bounds write matches C.

use crate::game::Game;
use crate::platform::Rect;
use crate::video::ScreenId;
use serde::{Deserialize, Serialize};

/// Mutable globals of this module (rule R3).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {}

/// `vga[i]`, if `i` is inside the buffer.
fn at(vga: &mut [u8], i: i64) -> Option<&mut u8> {
    usize::try_from(i).ok().and_then(|i| vga.get_mut(i))
}

/// `memset(&vga[start], e, len)`, restricted to the buffer.
fn fill(vga: &mut [u8], start: i64, len: u64, e: u8) {
    let end = i128::from(start) + i128::from(len);
    let lo = start.clamp(0, vga.len() as i64) as usize;
    let hi = end.clamp(0, vga.len() as i128) as usize;
    if lo < hi {
        vga[lo..hi].fill(e);
    }
}

#[doc(alias = "JE_pix")]
pub fn je_pix(g: &mut Game, surface: ScreenId, x: i32, y: i32, c: u8) {
    let s = g.video.screen_mut(surface);
    // upstream clips only on the right and bottom
    if x < s.pitch && y < s.h {
        let i = i64::from(y.wrapping_mul(s.pitch).wrapping_add(x));
        if let Some(p) = at(&mut s.pixels, i) {
            *p = c;
        }
    }
}

#[doc(alias = "JE_pix3")]
pub fn je_pix3(g: &mut Game, surface: ScreenId, x: i32, y: i32, c: u8) {
    je_pix(g, surface, x, y, c);
    je_pix(g, surface, x.wrapping_sub(1), y, c);
    je_pix(g, surface, x.wrapping_add(1), y, c);
    je_pix(g, surface, x, y.wrapping_sub(1), c);
    je_pix(g, surface, x, y.wrapping_add(1), c);
}

/// Outline from (`a`, `b`) to (`c`, `d`) in color `e`.
#[doc(alias = "JE_rectangle")]
pub fn je_rectangle(g: &mut Game, surface: ScreenId, a: i32, b: i32, c: i32, d: i32, e: i32) {
    let s = g.video.screen_mut(surface);
    if a < s.pitch && b < s.h && c < s.pitch && d < s.h {
        let pitch = s.pitch;
        let vga = &mut s.pixels;
        let e = e as u8;
        let len = c.wrapping_sub(a).wrapping_add(1) as i64 as u64;

        // top and bottom edges
        fill(
            vga,
            i64::from(b.wrapping_mul(pitch).wrapping_add(a)),
            len,
            e,
        );
        fill(
            vga,
            i64::from(d.wrapping_mul(pitch).wrapping_add(a)),
            len,
            e,
        );

        // left and right edges
        for col in [a, c] {
            let start = i64::from(b) * i64::from(pitch) + i64::from(pitch) + i64::from(col);
            let end = i64::from(d) * i64::from(pitch) + i64::from(col);
            for i in strided(start, end, i64::from(pitch), 0, vga.len() as i64) {
                vga[i as usize] = e;
            }
        }
    }
}

pub fn fill_rectangle_xy(
    g: &mut Game,
    surface: ScreenId,
    x: i32,
    y: i32,
    x2: i32,
    y2: i32,
    color: u8,
) {
    let rect = Rect {
        x,
        y,
        w: x2.wrapping_sub(x).wrapping_add(1),
        h: y2.wrapping_sub(y).wrapping_add(1),
    };
    g.video
        .screen_mut(surface)
        .fill_rect(Some(rect), u32::from(color));
}

/// `start, start + step, …` below `end`, restricted to `[lo, hi)`: the
/// indices a C loop `for (i = start; i < end; i += step)` visits there
/// (`step > 0`).
fn strided(start: i64, end: i64, step: i64, lo: i64, hi: i64) -> impl Iterator<Item = i64> {
    let ceil_div = |n: i64| -(-n).div_euclid(step);
    let first = ceil_div(lo - start).max(0);
    let last = ceil_div(end - start).min(ceil_div(hi - start));
    (first..last).map(move |k| start + k * step)
}

/// Applies `op` to every pixel of the box (`a`, `b`)-(`c`, `d`), walking it
/// as upstream's `JE_barShade`/`JE_barBright` do.
fn bar(g: &mut Game, surface: ScreenId, a: i32, b: i32, c: i32, d: i32, op: impl Fn(u8) -> u8) {
    let s = g.video.screen_mut(surface);
    if a < s.pitch && b < s.h && c < s.pitch && d < s.h {
        let pitch = i64::from(s.pitch);
        let len = s.pixels.len() as i64;
        let width = i64::from(c) - i64::from(a) + 1;
        if width <= 0 {
            return;
        }
        let start = i64::from(b) * pitch + i64::from(a);
        let end = i64::from(d) * pitch + i64::from(a) + 1;
        for i in strided(start, end, pitch, 1 - width, len) {
            let lo = (-i).max(0);
            let hi = width.min(len - i);
            for j in lo..hi {
                let p = &mut s.pixels[(i + j) as usize];
                *p = op(*p);
            }
        }
    }
}

/// Halves the brightness of the box (`a`, `b`)-(`c`, `d`).
#[doc(alias = "JE_barShade")]
pub fn je_bar_shade(g: &mut Game, surface: ScreenId, a: i32, b: i32, c: i32, d: i32) {
    bar(g, surface, a, b, c, d, |p| ((p & 0x0F) >> 1) | (p & 0xF0));
}

/// Brightens the box (`a`, `b`)-(`c`, `d`) by 2, saturating at 15.
#[doc(alias = "JE_barBright")]
pub fn je_bar_bright(g: &mut Game, surface: ScreenId, a: i32, b: i32, c: i32, d: i32) {
    bar(g, surface, a, b, c, d, |p| {
        let ah = p & 0xF0;
        let al = ((p & 0x0F) + 2).min(0x0F);
        al.wrapping_add(ah)
    });
}

pub fn fill_rectangle_wh(
    g: &mut Game,
    surface: ScreenId,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    color: u8,
) {
    let rect = Rect {
        x,
        y,
        w: w as i32,
        h: h as i32,
    };
    g.video
        .screen_mut(surface)
        .fill_rect(Some(rect), u32::from(color));
}

#[allow(clippy::too_many_arguments)]
pub fn draw_segmented_gauge(
    g: &mut Game,
    surface: ScreenId,
    mut x: i32,
    y: i32,
    color: u8,
    segment_width: u32,
    segment_height: u32,
    segment_value: u32,
    value: u32,
) {
    let segments = value / segment_value;
    let partial_segment = value % segment_value;

    for _ in 0..segments {
        fill_rectangle_wh(
            g,
            surface,
            x,
            y,
            segment_width,
            segment_height,
            color.wrapping_add(12),
        );
        x = (x as u32).wrapping_add(segment_width).wrapping_add(1) as i32;
    }
    if partial_segment > 0 {
        let shade = (12u32.wrapping_mul(partial_segment) / segment_value) as u8;
        fill_rectangle_wh(
            g,
            surface,
            x,
            y,
            segment_width,
            segment_height,
            color.wrapping_add(shade),
        );
    }
}
