// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Scale2x, Scale3x: Copyright (C) 2001, 2002, 2003, 2004 Andrea Mazzoleni
// Copyright (C) rustyrian contributors

//! Port of upstream `video_scale.c` / `video_scale.h`.

use crate::Game;
use crate::platform::TextureId;
use crate::video::{ScreenId, VGA_HEIGHT, VGA_WIDTH};
use serde::{Deserialize, Serialize};

/// `ScalerFunction` (rule R8): one variant per function upstream stores in
/// the `scalers` table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScalerFunction {
    /// `nn_16`.
    Nn16,
    /// `nn_32`.
    Nn32,
    /// `scale2x_16`.
    Scale2x16,
    /// `scale2x_32`.
    Scale2x32,
    /// `scale3x_16`.
    Scale3x16,
    /// `scale3x_32`.
    Scale3x32,
    /// `hq2x_32` (`video_scale_hqNx.c`).
    Hq2x32,
    /// `hq3x_32` (`video_scale_hqNx.c`).
    Hq3x32,
    /// `hq4x_32` (`video_scale_hqNx.c`).
    Hq4x32,
}

/// `struct Scalers`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scalers {
    pub width: i32,
    pub height: i32,
    pub scaler16: Option<ScalerFunction>,
    pub scaler32: Option<ScalerFunction>,
    pub name: &'static str,
}

const fn s(
    mul: i32,
    scaler16: Option<ScalerFunction>,
    scaler32: ScalerFunction,
    name: &'static str,
) -> Scalers {
    Scalers {
        width: mul * VGA_WIDTH,
        height: mul * VGA_HEIGHT,
        scaler16,
        scaler32: Some(scaler32),
        name,
    }
}

use ScalerFunction::*;

/// `scalers`.
pub const SCALERS: [Scalers; 9] = [
    s(1, Some(Nn16), Nn32, "None"),
    s(2, Some(Nn16), Nn32, "2x"),
    s(2, Some(Scale2x16), Scale2x32, "Scale2x"),
    s(2, None, Hq2x32, "hq2x"),
    s(3, Some(Nn16), Nn32, "3x"),
    s(3, Some(Scale3x16), Scale3x32, "Scale3x"),
    s(3, None, Hq3x32, "hq3x"),
    s(4, Some(Nn16), Nn32, "4x"),
    s(4, None, Hq4x32, "hq4x"),
];

/// `scalers_count`.
pub const SCALERS_COUNT: u32 = SCALERS.len() as u32;

/// Mutable globals of `video_scale.c` (rule R3).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// `scaler`: index into [`SCALERS`].
    pub scaler: u32,
}

/// `strcmp(a, b) == 0` for C strings (each read up to its first NUL).
fn c_str_eq(a: &[u8], b: &[u8]) -> bool {
    let c = |s: &[u8]| -> usize { s.iter().position(|&x| x == 0).unwrap_or(s.len()) };
    a[..c(a)] == b[..c(b)]
}

/// `set_scaler_by_name(name)`; `name` is a C string.
pub fn set_scaler_by_name(g: &mut Game, name: &[u8]) {
    for (i, s) in SCALERS.iter().enumerate() {
        if c_str_eq(name, s.name.as_bytes()) {
            g.video_scale.scaler = i as u32;
            break;
        }
    }
}

/// A destination pixel of `BPP` bytes: C stores `rgb_palette[i]` through a
/// `Uint32 *` or (truncating) a `Uint16 *`, and the 16-bit scalers compare
/// the truncated values.
#[inline]
fn px<const BPP: usize>(v: u32) -> u32 {
    if BPP == 2 { v & 0xFFFF } else { v }
}

#[inline]
fn put<const BPP: usize>(dst: &mut [u8], off: usize, v: u32) {
    if BPP == 2 {
        dst[off..off + 2].copy_from_slice(&(v as u16).to_ne_bytes());
    } else {
        dst[off..off + 4].copy_from_slice(&v.to_ne_bytes());
    }
}

/// `nn_32` / `nn_16`: nearest-neighbour scaling by `dst_width / vga_width`.
fn nn<const BPP: usize>(g: &mut Game, src_surface: ScreenId, dst_texture: TextureId) {
    let src_pitch = g.video.screen(src_surface).pitch as usize;

    let dst_bpp = BPP;
    let (mut dst_width, mut dst_height) = (0, 0);
    g.platform.query_texture(
        dst_texture,
        None,
        None,
        Some(&mut dst_width),
        Some(&mut dst_height),
    );

    let height = VGA_HEIGHT;
    let width = VGA_WIDTH;
    let scale = dst_width / width;
    debug_assert!(scale == dst_height / height);

    let palette = &g.palette.rgb_palette;
    let src = &g.video.screen(src_surface).pixels;
    // D-UB-SCALER-LOCK: C writes through NULL when the lock fails.
    if let Some((dst, dst_pitch)) = g.platform.lock_texture(dst_texture, None) {
        let dst_pitch = dst_pitch as usize;
        let row_bytes = (dst_width as usize).wrapping_mul(dst_bpp);
        let mut s = 0usize;
        let mut d = 0usize;
        for _ in 0..height {
            let src_temp = s;
            let dst_temp = d;

            for _ in 0..width {
                let v = px::<BPP>(palette[usize::from(src[s])]);
                for _ in 0..scale.max(0) {
                    put::<BPP>(dst, d, v);
                    d += dst_bpp;
                }
                s += 1;
            }

            s = src_temp + src_pitch;
            d = dst_temp + dst_pitch;

            for _ in 1..scale.max(1) {
                dst.copy_within(dst_temp..dst_temp + row_bytes, d);
                d += dst_pitch;
            }
        }
    }

    g.platform.unlock_texture(dst_texture);
}

pub fn nn_32(g: &mut Game, src_surface: ScreenId, dst_texture: TextureId) {
    nn::<4>(g, src_surface, dst_texture);
}

pub fn nn_16(g: &mut Game, src_surface: ScreenId, dst_texture: TextureId) {
    nn::<2>(g, src_surface, dst_texture);
}

/// `scale2x_32` / `scale2x_16`.
fn scale2x<const BPP: usize>(g: &mut Game, src_surface: ScreenId, dst_texture: TextureId) {
    let src_pitch = g.video.screen(src_surface).pitch as usize;
    let dst_bpp = BPP;
    let height = VGA_HEIGHT as usize;
    let width = VGA_WIDTH as usize;

    let palette = &g.palette.rgb_palette;
    let src = &g.video.screen(src_surface).pixels;
    // D-UB-SCALER-LOCK: C writes through NULL when the lock fails.
    if let Some((dst, dst_pitch)) = g.platform.lock_texture(dst_texture, None) {
        let dst_pitch = dst_pitch as usize;
        let p = |i: usize| px::<BPP>(palette[usize::from(src[i])]);
        let mut s = 0usize;
        let mut d = 0usize;
        for y in 0..height {
            let src_temp = s;
            let dst_temp = d;

            let prevline = |s: usize| if y > 0 { s - src_pitch } else { s };
            let nextline = |s: usize| if y < height - 1 { s + src_pitch } else { s };

            for x in 0..width {
                let b = p(prevline(s));
                let dd = p(if x > 0 { s - 1 } else { s });
                let e = p(s);
                let f = p(if x < width - 1 { s + 1 } else { s });
                let h = p(nextline(s));

                let (e0, e1, e2, e3);
                if b != h && dd != f {
                    e0 = if dd == b { dd } else { e };
                    e1 = if b == f { f } else { e };
                    e2 = if dd == h { dd } else { e };
                    e3 = if h == f { f } else { e };
                } else {
                    (e0, e1, e2, e3) = (e, e, e, e);
                }

                put::<BPP>(dst, d, e0);
                put::<BPP>(dst, d + dst_bpp, e1);
                put::<BPP>(dst, d + dst_pitch, e2);
                put::<BPP>(dst, d + dst_pitch + dst_bpp, e3);

                s += 1;
                d += 2 * dst_bpp;
            }

            s = src_temp + src_pitch;
            d = dst_temp + 2 * dst_pitch;
        }
    }

    g.platform.unlock_texture(dst_texture);
}

pub fn scale2x_32(g: &mut Game, src_surface: ScreenId, dst_texture: TextureId) {
    scale2x::<4>(g, src_surface, dst_texture);
}

pub fn scale2x_16(g: &mut Game, src_surface: ScreenId, dst_texture: TextureId) {
    scale2x::<2>(g, src_surface, dst_texture);
}

/// `scale3x_32` / `scale3x_16`.
fn scale3x<const BPP: usize>(g: &mut Game, src_surface: ScreenId, dst_texture: TextureId) {
    let src_pitch = g.video.screen(src_surface).pitch as usize;
    let dst_bpp = BPP;
    let height = VGA_HEIGHT as usize;
    let width = VGA_WIDTH as usize;

    let palette = &g.palette.rgb_palette;
    let src = &g.video.screen(src_surface).pixels;
    // D-UB-SCALER-LOCK: C writes through NULL when the lock fails.
    if let Some((dst, dst_pitch)) = g.platform.lock_texture(dst_texture, None) {
        let dst_pitch = dst_pitch as usize;
        let p = |i: usize| px::<BPP>(palette[usize::from(src[i])]);
        let mut s = 0usize;
        let mut d = 0usize;
        for y in 0..height {
            let src_temp = s;
            let dst_temp = d;

            let prevline = |s: usize| if y > 0 { s - src_pitch } else { s };
            let nextline = |s: usize| if y < height - 1 { s + src_pitch } else { s };

            for x in 0..width {
                let l = |s: usize| if x > 0 { s - 1 } else { s };
                let r = |s: usize| if x < width - 1 { s + 1 } else { s };
                let a = p(l(prevline(s)));
                let b = p(prevline(s));
                let c = p(r(prevline(s)));
                let dd = p(l(s));
                let e = p(s);
                let f = p(r(s));
                let gg = p(l(nextline(s)));
                let h = p(nextline(s));
                let i = p(r(nextline(s)));

                let out: [u32; 9] = if b != h && dd != f {
                    [
                        if dd == b { dd } else { e },
                        if (dd == b && e != c) || (b == f && e != a) {
                            b
                        } else {
                            e
                        },
                        if b == f { f } else { e },
                        if (dd == b && e != gg) || (dd == h && e != a) {
                            dd
                        } else {
                            e
                        },
                        e,
                        if (b == f && e != i) || (h == f && e != c) {
                            f
                        } else {
                            e
                        },
                        if dd == h { dd } else { e },
                        if (dd == h && e != i) || (h == f && e != gg) {
                            h
                        } else {
                            e
                        },
                        if h == f { f } else { e },
                    ]
                } else {
                    [e; 9]
                };

                for (k, v) in out.into_iter().enumerate() {
                    put::<BPP>(dst, d + (k / 3) * dst_pitch + (k % 3) * dst_bpp, v);
                }

                s += 1;
                d += 3 * dst_bpp;
            }

            s = src_temp + src_pitch;
            d = dst_temp + 3 * dst_pitch;
        }
    }

    g.platform.unlock_texture(dst_texture);
}

pub fn scale3x_32(g: &mut Game, src_surface: ScreenId, dst_texture: TextureId) {
    scale3x::<4>(g, src_surface, dst_texture);
}

pub fn scale3x_16(g: &mut Game, src_surface: ScreenId, dst_texture: TextureId) {
    scale3x::<2>(g, src_surface, dst_texture);
}
