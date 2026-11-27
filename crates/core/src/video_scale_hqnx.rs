// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) 2003 MaxSt ( maxst@hiend3d.com )
// Copyright (C) rustyrian contributors

//! Port of upstream `video_scale_hqNx.c`: the hq2x, hq3x and hq4x scalers.
//!
//! The `switch (pattern)` tables of the three scalers are a mechanical
//! translation of upstream's (each `PIXELxy_z` macro expanded in place, case
//! labels and arm order kept); everything else follows the C statement for
//! statement.
//!
//! The C `Uint32 *pc` / `Uint8 *dst` pointers into the locked texture are byte
//! offsets into the locked buffer (rule R8); `pc` arguments are the buffer
//! suffix starting at the pointed-to pixel. Pixels are stored in native byte
//! order, as C's `*(Uint32 *)` stores are.

use crate::Game;
use crate::platform::TextureId;
use crate::video::{ScreenId, VGA_HEIGHT, VGA_WIDTH};
use serde::{Deserialize, Serialize};

/// `Ymask`.
pub const YMASK: i32 = 0x00FF_0000;
/// `Umask`.
pub const UMASK: i32 = 0x0000_FF00;
/// `Vmask`.
pub const VMASK: i32 = 0x0000_00FF;
/// `trY`.
pub const TR_Y: i32 = 0x0030_0000;
/// `trU`.
pub const TR_U: i32 = 0x0000_0700;
/// `trV`.
pub const TR_V: i32 = 0x0000_0006;

/// Mutable globals of `video_scale_hqNx.c` (rule R3).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// `YUV1`.
    pub yuv1: i32,
    /// `YUV2`.
    pub yuv2: i32,
}

/// `*(Uint32 *)pc = v`.
fn store(pc: &mut [u8], v: u32) {
    pc[..4].copy_from_slice(&v.to_ne_bytes());
}

pub fn interp1(pc: &mut [u8], c1: u32, c2: u32) {
    store(pc, c1.wrapping_mul(3).wrapping_add(c2) >> 2);
}

pub fn interp2(pc: &mut [u8], c1: u32, c2: u32, c3: u32) {
    store(
        pc,
        c1.wrapping_mul(2).wrapping_add(c2).wrapping_add(c3) >> 2,
    );
}

pub fn interp3(pc: &mut [u8], c1: u32, c2: u32) {
    store(
        pc,
        (((c1 & 0x00FF00).wrapping_mul(7).wrapping_add(c2 & 0x00FF00) & 0x0007_F800).wrapping_add(
            (c1 & 0xFF00FF).wrapping_mul(7).wrapping_add(c2 & 0xFF00FF) & 0x07F8_07F8,
        )) >> 3,
    );
}

pub fn interp4(pc: &mut [u8], c1: u32, c2: u32, c3: u32) {
    store(
        pc,
        (((c1 & 0x00FF00)
            .wrapping_mul(2)
            .wrapping_add((c2 & 0x00FF00).wrapping_add(c3 & 0x00FF00).wrapping_mul(7))
            & 0x000F_F000)
            .wrapping_add(
                (c1 & 0xFF00FF)
                    .wrapping_mul(2)
                    .wrapping_add((c2 & 0xFF00FF).wrapping_add(c3 & 0xFF00FF).wrapping_mul(7))
                    & 0x0FF0_0FF0,
            ))
            >> 4,
    );
}

pub fn interp5(pc: &mut [u8], c1: u32, c2: u32) {
    store(pc, c1.wrapping_add(c2) >> 1);
}

pub fn interp6(pc: &mut [u8], c1: u32, c2: u32, c3: u32) {
    store(
        pc,
        (((c1 & 0x00FF00)
            .wrapping_mul(5)
            .wrapping_add((c2 & 0x00FF00).wrapping_mul(2))
            .wrapping_add(c3 & 0x00FF00)
            & 0x0007_F800)
            .wrapping_add(
                (c1 & 0xFF00FF)
                    .wrapping_mul(5)
                    .wrapping_add((c2 & 0xFF00FF).wrapping_mul(2))
                    .wrapping_add(c3 & 0xFF00FF)
                    & 0x07F8_07F8,
            ))
            >> 3,
    );
}

pub fn interp7(pc: &mut [u8], c1: u32, c2: u32, c3: u32) {
    store(
        pc,
        (((c1 & 0x00FF00)
            .wrapping_mul(6)
            .wrapping_add(c2 & 0x00FF00)
            .wrapping_add(c3 & 0x00FF00)
            & 0x0007_F800)
            .wrapping_add(
                (c1 & 0xFF00FF)
                    .wrapping_mul(6)
                    .wrapping_add(c2 & 0xFF00FF)
                    .wrapping_add(c3 & 0xFF00FF)
                    & 0x07F8_07F8,
            ))
            >> 3,
    );
}

pub fn interp8(pc: &mut [u8], c1: u32, c2: u32) {
    store(
        pc,
        (((c1 & 0x00FF00)
            .wrapping_mul(5)
            .wrapping_add((c2 & 0x00FF00).wrapping_mul(3))
            & 0x0007_F800)
            .wrapping_add(
                (c1 & 0xFF00FF)
                    .wrapping_mul(5)
                    .wrapping_add((c2 & 0xFF00FF).wrapping_mul(3))
                    & 0x07F8_07F8,
            ))
            >> 3,
    );
}

pub fn interp9(pc: &mut [u8], c1: u32, c2: u32, c3: u32) {
    store(
        pc,
        (((c1 & 0x00FF00)
            .wrapping_mul(2)
            .wrapping_add((c2 & 0x00FF00).wrapping_add(c3 & 0x00FF00).wrapping_mul(3))
            & 0x0007_F800)
            .wrapping_add(
                (c1 & 0xFF00FF)
                    .wrapping_mul(2)
                    .wrapping_add((c2 & 0xFF00FF).wrapping_add(c3 & 0xFF00FF).wrapping_mul(3))
                    & 0x07F8_07F8,
            ))
            >> 3,
    );
}

pub fn interp10(pc: &mut [u8], c1: u32, c2: u32, c3: u32) {
    store(
        pc,
        (((c1 & 0x00FF00)
            .wrapping_mul(14)
            .wrapping_add(c2 & 0x00FF00)
            .wrapping_add(c3 & 0x00FF00)
            & 0x000F_F000)
            .wrapping_add(
                (c1 & 0xFF00FF)
                    .wrapping_mul(14)
                    .wrapping_add(c2 & 0xFF00FF)
                    .wrapping_add(c3 & 0xFF00FF)
                    & 0x0FF0_0FF0,
            ))
            >> 4,
    );
}

/// Takes `yuv_palette` (`palette.c`) rather than the `Game`: the scalers call
/// it while the locked texture borrows `g.platform`.
pub fn diff(yuv_palette: &[u32; 256], w1: u32, w2: u32) -> bool {
    let yuv1 = yuv_palette[w1 as usize];
    let yuv2 = yuv_palette[w2 as usize];
    ((yuv1 & YMASK as u32) as i32 - (yuv2 & YMASK as u32) as i32).abs() > TR_Y
        || ((yuv1 & UMASK as u32) as i32 - (yuv2 & UMASK as u32) as i32).abs() > TR_U
        || ((yuv1 & VMASK as u32) as i32 - (yuv2 & VMASK as u32) as i32).abs() > TR_V
}

pub fn hq2x_32(g: &mut Game, src_surface: ScreenId, dst_texture: TextureId) {
    let src_surface = g.video.screen(src_surface);
    let pixels = &src_surface.pixels;
    let mut src: usize = 0;

    let src_pitch = src_surface.pitch;

    const DST_BPP: usize = 4; // dst_surface->format->BytesPerPixel
    let height = VGA_HEIGHT; // src_surface->h
    let width = VGA_WIDTH; // src_surface->w

    let Some((buf, dst_pitch)) = g.platform.lock_texture(dst_texture, None) else {
        // D-UB-SCALER-LOCK: C ignores the failure and writes through the
        // unset pixel pointer.
        g.platform.unlock_texture(dst_texture);
        return;
    };
    let dst_pitch = dst_pitch as usize;
    let mut dst: usize = 0;

    let yuv_palette = &g.palette.yuv_palette;
    let rgb_palette = &g.palette.rgb_palette;
    let st = &mut g.video_scale_hqnx;

    let mut w = [0u32; 10];
    let mut c = [0u32; 10];

    //   +----+----+----+
    //   |    |    |    |
    //   | w1 | w2 | w3 |
    //   +----+----+----+
    //   |    |    |    |
    //   | w4 | w5 | w6 |
    //   +----+----+----+
    //   |    |    |    |
    //   | w7 | w8 | w9 |
    //   +----+----+----+

    for j in 0..height {
        let src_temp = src;
        let dst_temp = dst;

        let prevline: isize = if j > 0 { -(width as isize) } else { 0 };
        let nextline: isize = if j < height - 1 { width as isize } else { 0 };

        for i in 0..width {
            w[2] = u32::from(pixels[src.wrapping_add_signed(prevline)]);
            w[5] = u32::from(pixels[src]);
            w[8] = u32::from(pixels[src.wrapping_add_signed(nextline)]);

            if i > 0 {
                w[1] = u32::from(pixels[src.wrapping_add_signed(prevline - 1)]);
                w[4] = u32::from(pixels[src - 1]);
                w[7] = u32::from(pixels[src.wrapping_add_signed(nextline - 1)]);
            } else {
                w[1] = w[2];
                w[4] = w[5];
                w[7] = w[8];
            }

            if i < width - 1 {
                w[3] = u32::from(pixels[src.wrapping_add_signed(prevline + 1)]);
                w[6] = u32::from(pixels[src + 1]);
                w[9] = u32::from(pixels[src.wrapping_add_signed(nextline + 1)]);
            } else {
                w[3] = w[2];
                w[6] = w[5];
                w[9] = w[8];
            }

            let mut pattern: i32 = 0;
            let mut flag: i32 = 1;

            st.yuv1 = yuv_palette[w[5] as usize] as i32;

            for k in 1..=9 {
                if k == 5 {
                    continue;
                }

                if w[k] != w[5] {
                    st.yuv2 = yuv_palette[w[k] as usize] as i32;
                    if ((st.yuv1 & YMASK) - (st.yuv2 & YMASK)).abs() > TR_Y
                        || ((st.yuv1 & UMASK) - (st.yuv2 & UMASK)).abs() > TR_U
                        || ((st.yuv1 & VMASK) - (st.yuv2 & VMASK)).abs() > TR_V
                    {
                        pattern |= flag;
                    }
                }
                flag <<= 1;
            }

            for k in 1..=9 {
                // hq2x has a nasty inability to accept more than 6 bits for each component
                c[k] = rgb_palette[w[k] as usize] & 0xfcfc_fcfc;
            }

            match pattern {
                0 | 1 | 4 | 32 | 128 | 5 | 132 | 160 | 33 | 129 | 36 | 133 | 164 | 161 | 37
                | 165 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                }
                2 | 34 | 130 | 162 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[4]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[6]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                }
                16 | 17 | 48 | 49 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[2]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[8]);
                }
                64 | 65 | 68 | 69 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[4]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[6]);
                }
                8 | 12 | 136 | 140 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[8]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                }
                3 | 35 | 131 | 163 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[6]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                }
                6 | 38 | 134 | 166 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[4]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[6]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                }
                20 | 21 | 52 | 53 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[8]);
                }
                144 | 145 | 176 | 177 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[2]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[8]);
                }
                192 | 193 | 196 | 197 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[4]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6]);
                }
                96 | 97 | 100 | 101 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[6]);
                }
                40 | 44 | 168 | 172 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[8]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                }
                9 | 13 | 137 | 141 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[8]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                }
                18 | 50 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[4]);
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[3]);
                    } else {
                        interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[8]);
                }
                80 | 81 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[2]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[4]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                72 | 76 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[7]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[6]);
                }
                10 | 138 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp1(&mut buf[dst..], c[5], c[1]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[6]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[8]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                }
                66 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[4]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[6]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[4]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[6]);
                }
                24 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[2]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[8]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[8]);
                }
                7 | 39 | 135 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[6]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                }
                148 | 149 | 180 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[8]);
                }
                224 | 228 | 225 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6]);
                }
                41 | 169 | 45 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[8]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                }
                22 | 54 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[4]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[8]);
                }
                208 | 209 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[2]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[4]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                104 | 108 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[6]);
                }
                11 | 139 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[6]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[8]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                }
                19 | 51 => {
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst..], c[5], c[4]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[3]);
                    } else {
                        interp6(&mut buf[dst..], c[5], c[2], c[4]);
                        interp9(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[8]);
                }
                146 | 178 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[4]);
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[3]);
                        interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[8]);
                    } else {
                        interp9(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                        interp6(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                }
                84 | 85 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9]);
                    } else {
                        interp6(&mut buf[dst + DST_BPP..], c[5], c[6], c[2]);
                        interp9(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[4]);
                }
                112 | 113 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[2]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9]);
                    } else {
                        interp6(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                        interp9(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                200 | 204 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[7]);
                        interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6]);
                    } else {
                        interp9(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                        interp6(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[8], c[6]);
                    }
                }
                73 | 77 => {
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst..], c[5], c[2]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[7]);
                    } else {
                        interp6(&mut buf[dst..], c[5], c[4], c[2]);
                        interp9(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[6]);
                }
                42 | 170 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp1(&mut buf[dst..], c[5], c[1]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[8]);
                    } else {
                        interp9(&mut buf[dst..], c[5], c[4], c[2]);
                        interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[8]);
                    }
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[6]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                }
                14 | 142 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp1(&mut buf[dst..], c[5], c[1]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[6]);
                    } else {
                        interp9(&mut buf[dst..], c[5], c[4], c[2]);
                        interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[8]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                }
                67 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[6]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[4]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[6]);
                }
                70 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[4]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[6]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[4]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[6]);
                }
                28 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[8]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[8]);
                }
                152 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[2]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[8]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[8]);
                }
                194 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[4]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[6]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[4]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6]);
                }
                98 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[4]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[6]);
                }
                56 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[2]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[8]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[8]);
                }
                25 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[2]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[8]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[8]);
                }
                26 | 31 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[8]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[8]);
                }
                82 | 214 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[4]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[4]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                88 | 248 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[2]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                74 | 107 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[6]);
                }
                27 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[3]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[8]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[8]);
                }
                86 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[4]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[4]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9]);
                }
                216 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[2]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[7]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                106 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[6]);
                }
                30 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[8]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[8]);
                }
                210 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[4]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[3]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[4]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                120 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[2]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9]);
                }
                75 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[7]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[6]);
                }
                29 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[8]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[8]);
                }
                198 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[4]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[6]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[4]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6]);
                }
                184 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[2]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[8]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[8]);
                }
                99 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[6]);
                }
                57 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[2]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[8]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[8]);
                }
                71 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[6]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[4]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[6]);
                }
                156 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[8]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[8]);
                }
                226 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[4]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6]);
                }
                60 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[8]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[8]);
                }
                195 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[6]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[4]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6]);
                }
                102 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[4]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[6]);
                }
                153 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[2]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[8]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[8]);
                }
                58 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp1(&mut buf[dst..], c[5], c[1]);
                    } else {
                        interp7(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[3]);
                    } else {
                        interp7(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[8]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[8]);
                }
                83 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[3]);
                    } else {
                        interp7(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[4]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9]);
                    } else {
                        interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                92 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[7]);
                    } else {
                        interp7(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9]);
                    } else {
                        interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                202 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp1(&mut buf[dst..], c[5], c[1]);
                    } else {
                        interp7(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[7]);
                    } else {
                        interp7(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6]);
                }
                78 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp1(&mut buf[dst..], c[5], c[1]);
                    } else {
                        interp7(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[7]);
                    } else {
                        interp7(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[6]);
                }
                154 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp1(&mut buf[dst..], c[5], c[1]);
                    } else {
                        interp7(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[3]);
                    } else {
                        interp7(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[8]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[8]);
                }
                114 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[4]);
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[3]);
                    } else {
                        interp7(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9]);
                    } else {
                        interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                89 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[2]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[7]);
                    } else {
                        interp7(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9]);
                    } else {
                        interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                90 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp1(&mut buf[dst..], c[5], c[1]);
                    } else {
                        interp7(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[3]);
                    } else {
                        interp7(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[7]);
                    } else {
                        interp7(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9]);
                    } else {
                        interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                55 | 23 => {
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst..], c[5], c[4]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                    } else {
                        interp6(&mut buf[dst..], c[5], c[2], c[4]);
                        interp9(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[8]);
                }
                182 | 150 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[4]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[8]);
                    } else {
                        interp9(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                        interp6(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                }
                213 | 212 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp6(&mut buf[dst + DST_BPP..], c[5], c[6], c[2]);
                        interp9(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[4]);
                }
                241 | 240 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[2]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp6(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                        interp9(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                236 | 232 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                        interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6]);
                    } else {
                        interp9(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                        interp6(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[8], c[6]);
                    }
                }
                109 | 105 => {
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst..], c[5], c[2]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp6(&mut buf[dst..], c[5], c[4], c[2]);
                        interp9(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[6]);
                }
                171 | 43 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[8]);
                    } else {
                        interp9(&mut buf[dst..], c[5], c[4], c[2]);
                        interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[8]);
                    }
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[6]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                }
                143 | 15 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[6]);
                    } else {
                        interp9(&mut buf[dst..], c[5], c[4], c[2]);
                        interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[8]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                }
                124 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9]);
                }
                203 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6]);
                }
                62 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[8]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[8]);
                }
                211 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[3]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[4]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                118 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[4]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9]);
                }
                217 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[2]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[7]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                110 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[6]);
                }
                155 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[3]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[8]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[8]);
                }
                188 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[8]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[8]);
                }
                185 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[2]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[8]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[8]);
                }
                61 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[8]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[8]);
                }
                157 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[8]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[8]);
                }
                103 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[6]);
                }
                227 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6]);
                }
                230 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[4]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6]);
                }
                199 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[6]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[4]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6]);
                }
                220 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[7]);
                    } else {
                        interp7(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                158 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp1(&mut buf[dst..], c[5], c[1]);
                    } else {
                        interp7(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[8]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[8]);
                }
                234 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp1(&mut buf[dst..], c[5], c[1]);
                    } else {
                        interp7(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6]);
                }
                242 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[4]);
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[3]);
                    } else {
                        interp7(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                59 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[3]);
                    } else {
                        interp7(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[8]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[8]);
                }
                121 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[2]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9]);
                    } else {
                        interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                87 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[4]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9]);
                    } else {
                        interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                79 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[7]);
                    } else {
                        interp7(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[6]);
                }
                122 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp1(&mut buf[dst..], c[5], c[1]);
                    } else {
                        interp7(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[3]);
                    } else {
                        interp7(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9]);
                    } else {
                        interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                94 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp1(&mut buf[dst..], c[5], c[1]);
                    } else {
                        interp7(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[7]);
                    } else {
                        interp7(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9]);
                    } else {
                        interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                218 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp1(&mut buf[dst..], c[5], c[1]);
                    } else {
                        interp7(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[3]);
                    } else {
                        interp7(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[7]);
                    } else {
                        interp7(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                91 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[3]);
                    } else {
                        interp7(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[7]);
                    } else {
                        interp7(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9]);
                    } else {
                        interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                229 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6]);
                }
                167 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[6]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                }
                173 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[8]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                }
                181 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[8]);
                }
                186 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp1(&mut buf[dst..], c[5], c[1]);
                    } else {
                        interp7(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[3]);
                    } else {
                        interp7(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[8]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[8]);
                }
                115 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[3]);
                    } else {
                        interp7(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9]);
                    } else {
                        interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                93 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[7]);
                    } else {
                        interp7(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9]);
                    } else {
                        interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                206 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp1(&mut buf[dst..], c[5], c[1]);
                    } else {
                        interp7(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[7]);
                    } else {
                        interp7(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6]);
                }
                205 | 201 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[7]);
                    } else {
                        interp7(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6]);
                }
                174 | 46 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp1(&mut buf[dst..], c[5], c[1]);
                    } else {
                        interp7(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[8]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                }
                179 | 147 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[3]);
                    } else {
                        interp7(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[8]);
                }
                117 | 116 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9]);
                    } else {
                        interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                189 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[8]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[8]);
                }
                231 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6]);
                }
                126 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9]);
                }
                219 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[7]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                125 => {
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst..], c[5], c[2]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp6(&mut buf[dst..], c[5], c[4], c[2]);
                        interp9(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9]);
                }
                221 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp6(&mut buf[dst + DST_BPP..], c[5], c[6], c[2]);
                        interp9(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[7]);
                }
                207 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[6]);
                    } else {
                        interp9(&mut buf[dst..], c[5], c[4], c[2]);
                        interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6]);
                }
                238 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                        interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6]);
                    } else {
                        interp9(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                        interp6(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[8], c[6]);
                    }
                }
                190 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[8]);
                    } else {
                        interp9(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                        interp6(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[8]);
                }
                187 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[8]);
                    } else {
                        interp9(&mut buf[dst..], c[5], c[4], c[2]);
                        interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[8]);
                    }
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[8]);
                }
                243 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[3]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp6(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                        interp9(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                119 => {
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst..], c[5], c[4]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                    } else {
                        interp6(&mut buf[dst..], c[5], c[2], c[4]);
                        interp9(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9]);
                }
                237 | 233 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp10(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6]);
                }
                175 | 47 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp10(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[8]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                }
                183 | 151 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                    } else {
                        interp10(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[8]);
                }
                245 | 244 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp10(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                250 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[3]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                123 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[3]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9]);
                }
                95 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9]);
                }
                222 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[7]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                252 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp10(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                249 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[2]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp10(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                235 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    interp2(&mut buf[dst + DST_BPP..], c[5], c[3], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp10(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6]);
                }
                111 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp10(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[6]);
                }
                63 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp10(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[8]);
                    interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9], c[8]);
                }
                159 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                    } else {
                        interp10(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[8]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[8]);
                }
                215 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                    } else {
                        interp10(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp2(&mut buf[dst + dst_pitch..], c[5], c[7], c[4]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                246 => {
                    interp2(&mut buf[dst..], c[5], c[1], c[4]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp10(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                254 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp10(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                253 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp10(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp10(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                251 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[3]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp10(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                239 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp10(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp10(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6]);
                }
                127 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp10(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[9]);
                }
                191 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp10(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                    } else {
                        interp10(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[8]);
                    interp1(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[8]);
                }
                223 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                    } else {
                        interp10(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[7]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                247 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                    } else {
                        interp10(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp10(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                255 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp10(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                    } else {
                        interp10(&mut buf[dst + DST_BPP..], c[5], c[2], c[6]);
                    }
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp10(&mut buf[dst + dst_pitch..], c[5], c[8], c[4]);
                    }
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp10(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[6], c[8]);
                    }
                }
                _ => {}
            }

            src += 1;
            dst += 2 * DST_BPP;
        }

        src = src_temp.wrapping_add_signed(src_pitch as isize);
        dst = dst_temp + 2 * dst_pitch;
    }

    g.platform.unlock_texture(dst_texture);
}

pub fn hq3x_32(g: &mut Game, src_surface: ScreenId, dst_texture: TextureId) {
    let src_surface = g.video.screen(src_surface);
    let pixels = &src_surface.pixels;
    let mut src: usize = 0;

    let src_pitch = src_surface.pitch;

    const DST_BPP: usize = 4; // dst_surface->format->BytesPerPixel
    let height = VGA_HEIGHT; // src_surface->h
    let width = VGA_WIDTH; // src_surface->w

    let Some((buf, dst_pitch)) = g.platform.lock_texture(dst_texture, None) else {
        // D-UB-SCALER-LOCK: C ignores the failure and writes through the
        // unset pixel pointer.
        g.platform.unlock_texture(dst_texture);
        return;
    };
    let dst_pitch = dst_pitch as usize;
    let mut dst: usize = 0;

    let yuv_palette = &g.palette.yuv_palette;
    let rgb_palette = &g.palette.rgb_palette;
    let st = &mut g.video_scale_hqnx;

    let mut w = [0u32; 10];
    let mut c = [0u32; 10];

    //   +----+----+----+
    //   |    |    |    |
    //   | w1 | w2 | w3 |
    //   +----+----+----+
    //   |    |    |    |
    //   | w4 | w5 | w6 |
    //   +----+----+----+
    //   |    |    |    |
    //   | w7 | w8 | w9 |
    //   +----+----+----+

    for j in 0..height {
        let src_temp = src;
        let dst_temp = dst;

        let prevline: isize = if j > 0 { -(width as isize) } else { 0 };
        let nextline: isize = if j < height - 1 { width as isize } else { 0 };

        for i in 0..width {
            w[2] = u32::from(pixels[src.wrapping_add_signed(prevline)]);
            w[5] = u32::from(pixels[src]);
            w[8] = u32::from(pixels[src.wrapping_add_signed(nextline)]);

            if i > 0 {
                w[1] = u32::from(pixels[src.wrapping_add_signed(prevline - 1)]);
                w[4] = u32::from(pixels[src - 1]);
                w[7] = u32::from(pixels[src.wrapping_add_signed(nextline - 1)]);
            } else {
                w[1] = w[2];
                w[4] = w[5];
                w[7] = w[8];
            }

            if i < width - 1 {
                w[3] = u32::from(pixels[src.wrapping_add_signed(prevline + 1)]);
                w[6] = u32::from(pixels[src + 1]);
                w[9] = u32::from(pixels[src.wrapping_add_signed(nextline + 1)]);
            } else {
                w[3] = w[2];
                w[6] = w[5];
                w[9] = w[8];
            }

            let mut pattern: i32 = 0;
            let mut flag: i32 = 1;

            st.yuv1 = yuv_palette[w[5] as usize] as i32;

            for k in 1..=9 {
                if k == 5 {
                    continue;
                }

                if w[k] != w[5] {
                    st.yuv2 = yuv_palette[w[k] as usize] as i32;
                    if ((st.yuv1 & YMASK) - (st.yuv2 & YMASK)).abs() > TR_Y
                        || ((st.yuv1 & UMASK) - (st.yuv2 & UMASK)).abs() > TR_U
                        || ((st.yuv1 & VMASK) - (st.yuv2 & VMASK)).abs() > TR_V
                    {
                        pattern |= flag;
                    }
                }
                flag <<= 1;
            }

            for k in 1..=9 {
                // hq3x has a nasty inability to accept more than 6 bits for each component
                c[k] = rgb_palette[w[k] as usize] & 0xfcfc_fcfc;
            }

            match pattern {
                0 | 1 | 4 | 32 | 128 | 5 | 132 | 160 | 33 | 129 | 36 | 133 | 164 | 161 | 37
                | 165 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp2(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                }
                2 | 34 | 130 | 162 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp2(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                }
                16 | 17 | 48 | 49 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                64 | 65 | 68 | 69 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                8 | 12 | 136 | 140 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp2(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                }
                3 | 35 | 131 | 163 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp2(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                }
                6 | 38 | 134 | 166 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp2(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                }
                20 | 21 | 52 | 53 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                144 | 145 | 176 | 177 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                }
                192 | 193 | 196 | 197 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                }
                96 | 97 | 100 | 101 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                40 | 44 | 168 | 172 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp2(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                }
                9 | 13 | 137 | 141 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp2(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                }
                18 | 50 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp4(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                80 | 81 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    } else {
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                        interp4(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                72 | 76 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        interp4(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    }
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                10 | 138 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp1(&mut buf[dst..], c[5], c[1]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp4(&mut buf[dst..], c[5], c[4], c[2]);
                        interp3(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp3(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    }
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp2(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                }
                66 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                24 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                7 | 39 | 135 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp2(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                }
                148 | 149 | 180 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                }
                224 | 228 | 225 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                }
                41 | 169 | 45 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp2(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                }
                22 | 54 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp4(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                208 | 209 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                        interp4(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                104 | 108 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        interp4(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    }
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                11 | 139 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp4(&mut buf[dst..], c[5], c[4], c[2]);
                        interp3(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp3(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    }
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp2(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                }
                19 | 51 => {
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst..], c[5], c[4]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                        interp1(&mut buf[dst + DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + 2 * DST_BPP..], c[2], c[6]);
                        interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                146 | 178 => {
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    } else {
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp5(&mut buf[dst + 2 * DST_BPP..], c[2], c[6]);
                        interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[6], c[5]);
                        interp2(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                    interp1(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                }
                84 | 85 => {
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    } else {
                        interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                        interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[6], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                        interp5(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[6], c[8]);
                    }
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                }
                112 | 113 => {
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    } else {
                        interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                        interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                        interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[8], c[5]);
                        interp5(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[6], c[8]);
                    }
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                }
                200 | 204 => {
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    } else {
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        interp5(&mut buf[dst + 2 * dst_pitch..], c[8], c[4]);
                        interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[8], c[5]);
                        interp2(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                    interp1(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                }
                73 | 77 => {
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst..], c[5], c[2]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                        interp1(&mut buf[dst + dst_pitch..], c[4], c[5]);
                        interp5(&mut buf[dst + 2 * dst_pitch..], c[8], c[4]);
                        interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    }
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                42 | 170 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp1(&mut buf[dst..], c[5], c[1]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    } else {
                        interp5(&mut buf[dst..], c[4], c[2]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp1(&mut buf[dst + dst_pitch..], c[4], c[5]);
                        interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    }
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp2(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                }
                14 | 142 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp1(&mut buf[dst..], c[5], c[1]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp5(&mut buf[dst..], c[4], c[2]);
                        interp1(&mut buf[dst + DST_BPP..], c[2], c[5]);
                        interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    }
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp2(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                }
                67 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                70 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                28 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                152 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                }
                194 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                }
                98 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                56 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                25 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                26 | 31 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp4(&mut buf[dst..], c[5], c[4], c[2]);
                        interp3(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    }
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp4(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    }
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                82 | 214 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp4(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                        interp4(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                88 | 248 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                    } else {
                        interp3(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        interp4(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                        interp4(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                74 | 107 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                    } else {
                        interp4(&mut buf[dst..], c[5], c[4], c[2]);
                        interp3(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    }
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp4(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    }
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                27 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp4(&mut buf[dst..], c[5], c[4], c[2]);
                        interp3(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp3(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    }
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                86 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp4(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                216 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                        interp4(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                106 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        interp4(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    }
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                30 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp4(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    }
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                210 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                        interp4(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                120 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        interp4(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    }
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                75 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp4(&mut buf[dst..], c[5], c[4], c[2]);
                        interp3(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp3(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    }
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                29 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                198 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                }
                184 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                }
                99 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                57 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                71 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                156 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                }
                226 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                }
                60 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                195 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                }
                102 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                153 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                }
                58 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp1(&mut buf[dst..], c[5], c[1]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    } else {
                        interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    }
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                83 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    } else {
                        interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    } else {
                        interp2(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                92 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    } else {
                        interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    } else {
                        interp2(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                202 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp1(&mut buf[dst..], c[5], c[1]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    } else {
                        interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                }
                78 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp1(&mut buf[dst..], c[5], c[1]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    } else {
                        interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                154 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp1(&mut buf[dst..], c[5], c[1]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    } else {
                        interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    }
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                }
                114 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    } else {
                        interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    } else {
                        interp2(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                89 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    } else {
                        interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    } else {
                        interp2(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                90 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp1(&mut buf[dst..], c[5], c[1]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    } else {
                        interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    }
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    } else {
                        interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    } else {
                        interp2(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                55 | 23 => {
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst..], c[5], c[4]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                        interp1(&mut buf[dst + DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + 2 * DST_BPP..], c[2], c[6]);
                        interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                182 | 150 => {
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    } else {
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp5(&mut buf[dst + 2 * DST_BPP..], c[2], c[6]);
                        interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[6], c[5]);
                        interp2(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                    interp1(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                }
                213 | 212 => {
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                        interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[6], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                        interp5(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[6], c[8]);
                    }
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                }
                241 | 240 => {
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                        interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                        interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[8], c[5]);
                        interp5(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[6], c[8]);
                    }
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                }
                236 | 232 => {
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    } else {
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        interp5(&mut buf[dst + 2 * dst_pitch..], c[8], c[4]);
                        interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[8], c[5]);
                        interp2(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                    interp1(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                }
                109 | 105 => {
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst..], c[5], c[2]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                        interp1(&mut buf[dst + dst_pitch..], c[4], c[5]);
                        interp5(&mut buf[dst + 2 * dst_pitch..], c[8], c[4]);
                        interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    }
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                171 | 43 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    } else {
                        interp5(&mut buf[dst..], c[4], c[2]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp1(&mut buf[dst + dst_pitch..], c[4], c[5]);
                        interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    }
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp2(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                }
                143 | 15 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp5(&mut buf[dst..], c[4], c[2]);
                        interp1(&mut buf[dst + DST_BPP..], c[2], c[5]);
                        interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    }
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp2(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                }
                124 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        interp4(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    }
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                203 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp4(&mut buf[dst..], c[5], c[4], c[2]);
                        interp3(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp3(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    }
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                }
                62 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp4(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    }
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                211 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                        interp4(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                118 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp4(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                217 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                        interp4(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                110 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        interp4(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    }
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                155 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp4(&mut buf[dst..], c[5], c[4], c[2]);
                        interp3(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp3(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    }
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                }
                188 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                }
                185 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                }
                61 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                157 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                }
                103 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                227 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                }
                230 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                }
                199 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                }
                220 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    } else {
                        interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    }
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                        interp4(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                158 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp1(&mut buf[dst..], c[5], c[1]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp4(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    }
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                }
                234 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp1(&mut buf[dst..], c[5], c[1]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        interp4(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    }
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                }
                242 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    } else {
                        interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                        interp4(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                59 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp4(&mut buf[dst..], c[5], c[4], c[2]);
                        interp3(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp3(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    } else {
                        interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    }
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                121 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        interp4(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    }
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    } else {
                        interp2(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                87 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp4(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    } else {
                        interp2(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                79 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp4(&mut buf[dst..], c[5], c[4], c[2]);
                        interp3(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp3(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    }
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    } else {
                        interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                122 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp1(&mut buf[dst..], c[5], c[1]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    } else {
                        interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    }
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        interp4(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    }
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    } else {
                        interp2(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                94 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp1(&mut buf[dst..], c[5], c[1]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp4(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    }
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    } else {
                        interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    } else {
                        interp2(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                218 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp1(&mut buf[dst..], c[5], c[1]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    } else {
                        interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    }
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    } else {
                        interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    }
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                        interp4(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                91 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp4(&mut buf[dst..], c[5], c[4], c[2]);
                        interp3(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp3(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    } else {
                        interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    }
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    } else {
                        interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    } else {
                        interp2(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                229 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                }
                167 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp2(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                }
                173 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp2(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                }
                181 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                }
                186 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp1(&mut buf[dst..], c[5], c[1]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    } else {
                        interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    }
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                }
                115 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    } else {
                        interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    } else {
                        interp2(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                93 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    } else {
                        interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    } else {
                        interp2(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                206 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp1(&mut buf[dst..], c[5], c[1]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    } else {
                        interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                }
                205 | 201 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    } else {
                        interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                }
                174 | 46 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp1(&mut buf[dst..], c[5], c[1]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp2(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                }
                179 | 147 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    } else {
                        interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                }
                117 | 116 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    } else {
                        interp2(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                189 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                }
                231 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                }
                126 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp4(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    }
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        interp4(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    }
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                219 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp4(&mut buf[dst..], c[5], c[4], c[2]);
                        interp3(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp3(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    }
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                        interp4(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                125 => {
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst..], c[5], c[2]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                        interp1(&mut buf[dst + dst_pitch..], c[4], c[5]);
                        interp5(&mut buf[dst + 2 * dst_pitch..], c[8], c[4]);
                        interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    }
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                221 => {
                    if diff(yuv_palette, w[6], w[8]) {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                        interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[6], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                        interp5(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[6], c[8]);
                    }
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                }
                207 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp5(&mut buf[dst..], c[4], c[2]);
                        interp1(&mut buf[dst + DST_BPP..], c[2], c[5]);
                        interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    }
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                }
                238 => {
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    } else {
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        interp5(&mut buf[dst + 2 * dst_pitch..], c[8], c[4]);
                        interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[8], c[5]);
                        interp2(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                    interp1(&mut buf[dst..], c[5], c[1]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                }
                190 => {
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    } else {
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp5(&mut buf[dst + 2 * DST_BPP..], c[2], c[6]);
                        interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[6], c[5]);
                        interp2(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                    interp1(&mut buf[dst..], c[5], c[1]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                }
                187 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    } else {
                        interp5(&mut buf[dst..], c[4], c[2]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp1(&mut buf[dst + dst_pitch..], c[4], c[5]);
                        interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    }
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                }
                243 => {
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                        interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                        interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[8], c[5]);
                        interp5(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[6], c[8]);
                    }
                    interp1(&mut buf[dst..], c[5], c[4]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                }
                119 => {
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst..], c[5], c[4]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                        interp1(&mut buf[dst + DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + 2 * DST_BPP..], c[2], c[6]);
                        interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                237 | 233 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                    } else {
                        interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                }
                175 | 47 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp2(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                }
                183 | 151 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                }
                245 | 244 => {
                    interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp2(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                250 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                    } else {
                        interp3(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        interp4(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                        interp4(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                123 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                    } else {
                        interp4(&mut buf[dst..], c[5], c[4], c[2]);
                        interp3(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    }
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp4(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    }
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                95 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp4(&mut buf[dst..], c[5], c[4], c[2]);
                        interp3(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    }
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp4(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    }
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                222 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp4(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    }
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                        interp4(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                252 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                    } else {
                        interp3(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        interp4(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp2(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                249 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                    } else {
                        interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                        interp4(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                235 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                    } else {
                        interp4(&mut buf[dst..], c[5], c[4], c[2]);
                        interp3(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    }
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                    } else {
                        interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                }
                111 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp4(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    }
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                63 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp4(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    }
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                159 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp4(&mut buf[dst..], c[5], c[4], c[2]);
                        interp3(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    }
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    }
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                }
                215 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                        interp4(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                246 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp4(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp2(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                254 => {
                    interp1(&mut buf[dst..], c[5], c[1]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp4(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    }
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                    } else {
                        interp3(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        interp4(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    }
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                        interp2(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                253 => {
                    interp1(&mut buf[dst..], c[5], c[2]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                    } else {
                        interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp2(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                251 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                    } else {
                        interp4(&mut buf[dst..], c[5], c[4], c[2]);
                        interp3(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    }
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    }
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                        interp4(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                239 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                    } else {
                        interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                }
                127 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                        interp3(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp3(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp4(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    }
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp4(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    }
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                }
                191 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    }
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                }
                223 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp4(&mut buf[dst..], c[5], c[4], c[2]);
                        interp3(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    }
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                        interp4(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                247 => {
                    interp1(&mut buf[dst..], c[5], c[4]);
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp2(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                255 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[4], c[2]);
                    }
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    }
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                    } else {
                        interp2(&mut buf[dst + 2 * dst_pitch..], c[5], c[8], c[4]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    } else {
                        interp2(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                    }
                }
                _ => {}
            }

            src += 1;
            dst += 3 * DST_BPP;
        }

        src = src_temp.wrapping_add_signed(src_pitch as isize);
        dst = dst_temp + 3 * dst_pitch;
    }

    g.platform.unlock_texture(dst_texture);
}

pub fn hq4x_32(g: &mut Game, src_surface: ScreenId, dst_texture: TextureId) {
    let src_surface = g.video.screen(src_surface);
    let pixels = &src_surface.pixels;
    let mut src: usize = 0;

    let src_pitch = src_surface.pitch;

    const DST_BPP: usize = 4; // dst_surface->format->BytesPerPixel
    let height = VGA_HEIGHT; // src_surface->h
    let width = VGA_WIDTH; // src_surface->w

    let Some((buf, dst_pitch)) = g.platform.lock_texture(dst_texture, None) else {
        // D-UB-SCALER-LOCK: C ignores the failure and writes through the
        // unset pixel pointer.
        g.platform.unlock_texture(dst_texture);
        return;
    };
    let dst_pitch = dst_pitch as usize;
    let mut dst: usize = 0;

    let yuv_palette = &g.palette.yuv_palette;
    let rgb_palette = &g.palette.rgb_palette;
    let st = &mut g.video_scale_hqnx;

    let mut w = [0u32; 10];
    let mut c = [0u32; 10];

    //   +----+----+----+
    //   |    |    |    |
    //   | w1 | w2 | w3 |
    //   +----+----+----+
    //   |    |    |    |
    //   | w4 | w5 | w6 |
    //   +----+----+----+
    //   |    |    |    |
    //   | w7 | w8 | w9 |
    //   +----+----+----+

    for j in 0..height {
        let src_temp = src;
        let dst_temp = dst;

        let prevline: isize = if j > 0 { -(width as isize) } else { 0 };
        let nextline: isize = if j < height - 1 { width as isize } else { 0 };

        for i in 0..width {
            w[2] = u32::from(pixels[src.wrapping_add_signed(prevline)]);
            w[5] = u32::from(pixels[src]);
            w[8] = u32::from(pixels[src.wrapping_add_signed(nextline)]);

            if i > 0 {
                w[1] = u32::from(pixels[src.wrapping_add_signed(prevline - 1)]);
                w[4] = u32::from(pixels[src - 1]);
                w[7] = u32::from(pixels[src.wrapping_add_signed(nextline - 1)]);
            } else {
                w[1] = w[2];
                w[4] = w[5];
                w[7] = w[8];
            }

            if i < width - 1 {
                w[3] = u32::from(pixels[src.wrapping_add_signed(prevline + 1)]);
                w[6] = u32::from(pixels[src + 1]);
                w[9] = u32::from(pixels[src.wrapping_add_signed(nextline + 1)]);
            } else {
                w[3] = w[2];
                w[6] = w[5];
                w[9] = w[8];
            }

            let mut pattern: i32 = 0;
            let mut flag: i32 = 1;

            st.yuv1 = yuv_palette[w[5] as usize] as i32;

            for k in 1..=9 {
                if k == 5 {
                    continue;
                }

                if w[k] != w[5] {
                    st.yuv2 = yuv_palette[w[k] as usize] as i32;
                    if ((st.yuv1 & YMASK) - (st.yuv2 & YMASK)).abs() > TR_Y
                        || ((st.yuv1 & UMASK) - (st.yuv2 & UMASK)).abs() > TR_U
                        || ((st.yuv1 & VMASK) - (st.yuv2 & VMASK)).abs() > TR_V
                    {
                        pattern |= flag;
                    }
                }
                flag <<= 1;
            }

            for k in 1..=9 {
                // hq4x has a nasty inability to accept more than 6 bits for each component
                c[k] = rgb_palette[w[k] as usize] & 0xfcfc_fcfc;
            }

            match pattern {
                0 | 1 | 4 | 32 | 128 | 5 | 132 | 160 | 33 | 129 | 36 | 133 | 164 | 161 | 37
                | 165 => {
                    interp2(&mut buf[dst..], c[5], c[2], c[4]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[4]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[2]);
                    interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4], c[2]);
                    interp7(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6], c[2]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[2]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[8]);
                    interp7(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4], c[8]);
                    interp7(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[4]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                    interp2(
                        &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                }
                2 | 34 | 130 | 162 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[3]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[8]);
                    interp7(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4], c[8]);
                    interp7(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[4]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                    interp2(
                        &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                }
                16 | 17 | 48 | 49 => {
                    interp2(&mut buf[dst..], c[5], c[2], c[4]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[4]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[2]);
                    interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[8]);
                    interp7(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[4]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[9],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                64 | 65 | 68 | 69 => {
                    interp2(&mut buf[dst..], c[5], c[2], c[4]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[4]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[2]);
                    interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4], c[2]);
                    interp7(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6], c[2]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[2]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[9],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                8 | 12 | 136 | 140 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[1]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp7(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6], c[2]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[2]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp7(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[7]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                    interp2(
                        &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                }
                3 | 35 | 131 | 163 => {
                    interp8(&mut buf[dst..], c[5], c[4]);
                    interp3(&mut buf[dst + DST_BPP..], c[5], c[4]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[3]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[8]);
                    interp7(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4], c[8]);
                    interp7(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[4]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                    interp2(
                        &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                }
                6 | 38 | 134 | 166 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[6]);
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[8]);
                    interp7(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4], c[8]);
                    interp7(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[4]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                    interp2(
                        &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                }
                20 | 21 | 52 | 53 => {
                    interp2(&mut buf[dst..], c[5], c[2], c[4]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[4]);
                    interp8(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[2]);
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[2]);
                    interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[2]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[8]);
                    interp7(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[4]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[9],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                144 | 145 | 176 | 177 => {
                    interp2(&mut buf[dst..], c[5], c[2], c[4]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[4]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[2]);
                    interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[8]);
                    interp7(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                    interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[4]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                }
                192 | 193 | 196 | 197 => {
                    interp2(&mut buf[dst..], c[5], c[2], c[4]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[4]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[2]);
                    interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4], c[2]);
                    interp7(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6], c[2]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[2]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                }
                96 | 97 | 100 | 101 => {
                    interp2(&mut buf[dst..], c[5], c[2], c[4]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[4]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[2]);
                    interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4], c[2]);
                    interp7(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6], c[2]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[2]);
                    interp8(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[9],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[4]);
                    interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                40 | 44 | 168 | 172 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[1]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp7(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6], c[2]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[2]);
                    interp3(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp7(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                    interp2(
                        &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                }
                9 | 13 | 137 | 141 => {
                    interp8(&mut buf[dst..], c[5], c[2]);
                    interp8(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                    interp3(&mut buf[dst + dst_pitch..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[2]);
                    interp7(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6], c[2]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[2]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp7(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[7]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                    interp2(
                        &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                }
                18 | 50 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                        interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                        interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    } else {
                        interp5(&mut buf[dst + 2 * DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + 3 * DST_BPP..], c[2], c[6]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        interp5(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                    }
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[8]);
                    interp7(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[4]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[9],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                80 | 81 => {
                    interp2(&mut buf[dst..], c[5], c[2], c[4]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[4]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[2]);
                    interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                        interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    } else {
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                        interp5(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[8], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[8], c[6]);
                    }
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                }
                72 | 76 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[1]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp7(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6], c[2]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[2]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                        interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                        interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch..], c[4], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch..], c[8], c[4]);
                        interp5(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[8], c[5]);
                    }
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[9],
                    );
                    interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                10 | 138 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp8(&mut buf[dst..], c[5], c[1]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                        interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    } else {
                        interp5(&mut buf[dst..], c[2], c[4]);
                        interp5(&mut buf[dst + DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + dst_pitch..], c[4], c[5]);
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    }
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[3]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp7(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[7]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                    interp2(
                        &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                }
                66 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[3]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[9],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                24 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[1]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[7]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[9],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                7 | 39 | 135 => {
                    interp8(&mut buf[dst..], c[5], c[4]);
                    interp3(&mut buf[dst + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[8]);
                    interp7(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4], c[8]);
                    interp7(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[4]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                    interp2(
                        &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                }
                148 | 149 | 180 => {
                    interp2(&mut buf[dst..], c[5], c[2], c[4]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[4]);
                    interp8(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[2]);
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[2]);
                    interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[2]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[8]);
                    interp7(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                    interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[4]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                }
                224 | 228 | 225 => {
                    interp2(&mut buf[dst..], c[5], c[2], c[4]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[4]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[2]);
                    interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4], c[2]);
                    interp7(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6], c[2]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[2]);
                    interp8(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                }
                41 | 169 | 45 => {
                    interp8(&mut buf[dst..], c[5], c[2]);
                    interp8(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                    interp3(&mut buf[dst + dst_pitch..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[2]);
                    interp7(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6], c[2]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[2]);
                    interp3(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp7(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                    interp2(
                        &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                }
                22 | 54 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + 3 * DST_BPP..], c[2], c[6]);
                        interp5(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                    }
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[8]);
                    interp7(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[4]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[9],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                208 | 209 => {
                    interp2(&mut buf[dst..], c[5], c[2], c[4]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[4]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[2]);
                    interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[8], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[8], c[6]);
                    }
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                }
                104 | 108 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[1]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp7(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6], c[2]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[2]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch..], c[4], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch..], c[8], c[4]);
                        interp5(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[8], c[5]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[9],
                    );
                    interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                11 | 139 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp5(&mut buf[dst..], c[2], c[4]);
                        interp5(&mut buf[dst + DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + dst_pitch..], c[4], c[5]);
                    }
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[3]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp7(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[7]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                    interp2(
                        &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                }
                19 | 51 => {
                    if diff(yuv_palette, w[2], w[6]) {
                        interp8(&mut buf[dst..], c[5], c[4]);
                        interp3(&mut buf[dst + DST_BPP..], c[5], c[4]);
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                        interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                        interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    } else {
                        interp1(&mut buf[dst..], c[5], c[2]);
                        interp1(&mut buf[dst + DST_BPP..], c[2], c[5]);
                        interp8(&mut buf[dst + 2 * DST_BPP..], c[2], c[6]);
                        interp5(&mut buf[dst + 3 * DST_BPP..], c[2], c[6]);
                        interp7(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6], c[2]);
                        interp2(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[6], c[5], c[2]);
                    }
                    interp8(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[8]);
                    interp7(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[4]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[9],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                146 | 178 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                        interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                        interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                        interp3(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                        interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                    } else {
                        interp2(&mut buf[dst + 2 * DST_BPP..], c[2], c[5], c[6]);
                        interp5(&mut buf[dst + 3 * DST_BPP..], c[2], c[6]);
                        interp7(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6], c[2]);
                        interp8(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[6], c[2]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    }
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[8]);
                    interp7(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[4]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                }
                84 | 85 => {
                    interp2(&mut buf[dst..], c[5], c[2], c[4]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[4]);
                    interp8(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[2]);
                        interp3(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[2]);
                        interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                        interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    } else {
                        interp1(&mut buf[dst + 3 * DST_BPP..], c[5], c[6]);
                        interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                        interp7(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                        interp8(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[6], c[8]);
                        interp2(
                            &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                            c[8],
                            c[5],
                            c[6],
                        );
                        interp5(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[8], c[6]);
                    }
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[2]);
                    interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[2]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                }
                112 | 113 => {
                    interp2(&mut buf[dst..], c[5], c[2], c[4]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[4]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[2]);
                    interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                        interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[4]);
                        interp3(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[4]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                        interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    } else {
                        interp7(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                        interp2(
                            &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                            c[6],
                            c[5],
                            c[8],
                        );
                        interp1(&mut buf[dst + 3 * dst_pitch..], c[5], c[8]);
                        interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[8], c[5]);
                        interp8(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[8], c[6]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[8], c[6]);
                    }
                }
                200 | 204 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[1]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp7(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6], c[2]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[2]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                        interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                        interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                        interp3(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                        interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    } else {
                        interp2(&mut buf[dst + 2 * dst_pitch..], c[4], c[5], c[8]);
                        interp7(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4], c[8]);
                        interp5(&mut buf[dst + 3 * dst_pitch..], c[8], c[4]);
                        interp8(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[8], c[4]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[8], c[5]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                    }
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                }
                73 | 77 => {
                    if diff(yuv_palette, w[8], w[4]) {
                        interp8(&mut buf[dst..], c[5], c[2]);
                        interp3(&mut buf[dst + dst_pitch..], c[5], c[2]);
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                        interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                        interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                    } else {
                        interp1(&mut buf[dst..], c[5], c[4]);
                        interp1(&mut buf[dst + dst_pitch..], c[4], c[5]);
                        interp8(&mut buf[dst + 2 * dst_pitch..], c[4], c[8]);
                        interp7(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4], c[8]);
                        interp5(&mut buf[dst + 3 * dst_pitch..], c[8], c[4]);
                        interp2(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[8], c[5], c[4]);
                    }
                    interp8(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[2]);
                    interp7(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6], c[2]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[2]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[9],
                    );
                    interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                42 | 170 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp8(&mut buf[dst..], c[5], c[1]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                        interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                        interp3(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                        interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[8]);
                    } else {
                        interp5(&mut buf[dst..], c[2], c[4]);
                        interp2(&mut buf[dst + DST_BPP..], c[2], c[5], c[4]);
                        interp8(&mut buf[dst + dst_pitch..], c[4], c[2]);
                        interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4], c[2]);
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[4], c[5]);
                        interp1(&mut buf[dst + 3 * dst_pitch..], c[5], c[4]);
                    }
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[3]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp7(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                    interp2(
                        &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                }
                14 | 142 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp8(&mut buf[dst..], c[5], c[1]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                        interp3(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                        interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[6]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                        interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    } else {
                        interp5(&mut buf[dst..], c[2], c[4]);
                        interp8(&mut buf[dst + DST_BPP..], c[2], c[4]);
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[2], c[5]);
                        interp1(&mut buf[dst + 3 * DST_BPP..], c[5], c[2]);
                        interp2(&mut buf[dst + dst_pitch..], c[4], c[5], c[2]);
                        interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4], c[2]);
                    }
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp7(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[7]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                    interp2(
                        &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                }
                67 => {
                    interp8(&mut buf[dst..], c[5], c[4]);
                    interp3(&mut buf[dst + DST_BPP..], c[5], c[4]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[3]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[9],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                70 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[6]);
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[9],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                28 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[1]);
                    interp8(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[7]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[9],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                152 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[1]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[7]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                }
                194 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[3]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                }
                98 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[3]);
                    interp8(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[9],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[4]);
                    interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                56 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[1]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    interp3(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[9],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                25 => {
                    interp8(&mut buf[dst..], c[5], c[2]);
                    interp8(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp3(&mut buf[dst + dst_pitch..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[7]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[9],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                26 | 31 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp5(&mut buf[dst..], c[2], c[4]);
                        interp5(&mut buf[dst + DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + dst_pitch..], c[4], c[5]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + 3 * DST_BPP..], c[2], c[6]);
                        interp5(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                    }
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[7]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[9],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                82 | 214 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + 3 * DST_BPP..], c[2], c[6]);
                        interp5(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                    }
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[8], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[8], c[6]);
                    }
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                }
                88 | 248 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[1]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch..], c[4], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch..], c[8], c[4]);
                        interp5(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[8], c[5]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[8], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[8], c[6]);
                    }
                }
                74 | 107 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp5(&mut buf[dst..], c[2], c[4]);
                        interp5(&mut buf[dst + DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + dst_pitch..], c[4], c[5]);
                    }
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[3]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch..], c[4], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch..], c[8], c[4]);
                        interp5(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[8], c[5]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[9],
                    );
                    interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                27 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp5(&mut buf[dst..], c[2], c[4]);
                        interp5(&mut buf[dst + DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + dst_pitch..], c[4], c[5]);
                    }
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[7]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[9],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                86 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + 3 * DST_BPP..], c[2], c[6]);
                        interp5(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                    }
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                216 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[1]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[8], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[8], c[6]);
                    }
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                }
                106 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[3]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch..], c[4], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch..], c[8], c[4]);
                        interp5(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[8], c[5]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[9],
                    );
                    interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                30 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + 3 * DST_BPP..], c[2], c[6]);
                        interp5(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[7]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[9],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                210 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[8], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[8], c[6]);
                    }
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                }
                120 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[1]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch..], c[4], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch..], c[8], c[4]);
                        interp5(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[8], c[5]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                75 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp5(&mut buf[dst..], c[2], c[4]);
                        interp5(&mut buf[dst + DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + dst_pitch..], c[4], c[5]);
                    }
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[3]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[9],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                29 => {
                    interp8(&mut buf[dst..], c[5], c[2]);
                    interp8(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp8(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[7]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[9],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                198 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[6]);
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                }
                184 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[1]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    interp3(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                }
                99 => {
                    interp8(&mut buf[dst..], c[5], c[4]);
                    interp3(&mut buf[dst + DST_BPP..], c[5], c[4]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[3]);
                    interp8(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[9],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[4]);
                    interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                57 => {
                    interp8(&mut buf[dst..], c[5], c[2]);
                    interp8(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp3(&mut buf[dst + dst_pitch..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    interp3(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[9],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                71 => {
                    interp8(&mut buf[dst..], c[5], c[4]);
                    interp3(&mut buf[dst + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[9],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                156 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[1]);
                    interp8(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[7]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                }
                226 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[3]);
                    interp8(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                }
                60 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[1]);
                    interp8(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[9],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                195 => {
                    interp8(&mut buf[dst..], c[5], c[4]);
                    interp3(&mut buf[dst + DST_BPP..], c[5], c[4]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[3]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                }
                102 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[6]);
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[9],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[4]);
                    interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                153 => {
                    interp8(&mut buf[dst..], c[5], c[2]);
                    interp8(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp3(&mut buf[dst + dst_pitch..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[7]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                }
                58 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp8(&mut buf[dst..], c[5], c[1]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                        interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[2], c[4]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                        interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                        interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    } else {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                        interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    }
                    interp3(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[9],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                83 => {
                    interp8(&mut buf[dst..], c[5], c[4]);
                    interp3(&mut buf[dst + DST_BPP..], c[5], c[4]);
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                        interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                        interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    } else {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                        interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    }
                    interp8(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                        interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    } else {
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                        interp2(
                            &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                            c[5],
                            c[8],
                            c[6],
                        );
                    }
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                }
                92 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[1]);
                    interp8(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[2]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                        interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                        interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                    } else {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                        interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    }
                    if diff(yuv_palette, w[6], w[8]) {
                        interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                        interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    } else {
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                        interp2(
                            &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                            c[5],
                            c[8],
                            c[6],
                        );
                    }
                }
                202 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp8(&mut buf[dst..], c[5], c[1]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                        interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[2], c[4]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    }
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[3]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                        interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                        interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                    } else {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                        interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    }
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    interp3(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                }
                78 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp8(&mut buf[dst..], c[5], c[1]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                        interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[2], c[4]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    }
                    interp3(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[6]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                        interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                        interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                    } else {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                        interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    }
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[9],
                    );
                    interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                154 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp8(&mut buf[dst..], c[5], c[1]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                        interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[2], c[4]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                        interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                        interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    } else {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                        interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    }
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[7]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                }
                114 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                        interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                        interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    } else {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                        interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    }
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp8(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                        interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    } else {
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                        interp2(
                            &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                            c[5],
                            c[8],
                            c[6],
                        );
                    }
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[4]);
                }
                89 => {
                    interp8(&mut buf[dst..], c[5], c[2]);
                    interp8(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp3(&mut buf[dst + dst_pitch..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                        interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                        interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                    } else {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                        interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    }
                    if diff(yuv_palette, w[6], w[8]) {
                        interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                        interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    } else {
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                        interp2(
                            &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                            c[5],
                            c[8],
                            c[6],
                        );
                    }
                }
                90 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp8(&mut buf[dst..], c[5], c[1]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                        interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[2], c[4]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                        interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                        interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    } else {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                        interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    }
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                        interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                        interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                    } else {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                        interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    }
                    if diff(yuv_palette, w[6], w[8]) {
                        interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                        interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    } else {
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                        interp2(
                            &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                            c[5],
                            c[8],
                            c[6],
                        );
                    }
                }
                55 | 23 => {
                    if diff(yuv_palette, w[2], w[6]) {
                        interp8(&mut buf[dst..], c[5], c[4]);
                        interp3(&mut buf[dst + DST_BPP..], c[5], c[4]);
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp1(&mut buf[dst..], c[5], c[2]);
                        interp1(&mut buf[dst + DST_BPP..], c[2], c[5]);
                        interp8(&mut buf[dst + 2 * DST_BPP..], c[2], c[6]);
                        interp5(&mut buf[dst + 3 * DST_BPP..], c[2], c[6]);
                        interp7(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6], c[2]);
                        interp2(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[6], c[5], c[2]);
                    }
                    interp8(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[8]);
                    interp7(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[4]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[9],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                182 | 150 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5]);
                        interp3(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                        interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                    } else {
                        interp2(&mut buf[dst + 2 * DST_BPP..], c[2], c[5], c[6]);
                        interp5(&mut buf[dst + 3 * DST_BPP..], c[2], c[6]);
                        interp7(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6], c[2]);
                        interp8(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[6], c[2]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    }
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[8]);
                    interp7(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[4]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                }
                213 | 212 => {
                    interp2(&mut buf[dst..], c[5], c[2], c[4]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[4]);
                    interp8(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[2]);
                        interp3(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[2]);
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp1(&mut buf[dst + 3 * DST_BPP..], c[5], c[6]);
                        interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                        interp7(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                        interp8(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[6], c[8]);
                        interp2(
                            &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                            c[8],
                            c[5],
                            c[6],
                        );
                        interp5(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[8], c[6]);
                    }
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[2]);
                    interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[2]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                }
                241 | 240 => {
                    interp2(&mut buf[dst..], c[5], c[2], c[4]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[4]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[2]);
                    interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5]);
                        interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[4]);
                        interp3(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[4]);
                        store(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp7(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                        interp2(
                            &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                            c[6],
                            c[5],
                            c[8],
                        );
                        interp1(&mut buf[dst + 3 * dst_pitch..], c[5], c[8]);
                        interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[8], c[5]);
                        interp8(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[8], c[6]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[8], c[6]);
                    }
                }
                236 | 232 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[1]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp7(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6], c[2]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[2]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5]);
                        interp3(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                        interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    } else {
                        interp2(&mut buf[dst + 2 * dst_pitch..], c[4], c[5], c[8]);
                        interp7(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4], c[8]);
                        interp5(&mut buf[dst + 3 * dst_pitch..], c[8], c[4]);
                        interp8(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[8], c[4]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[8], c[5]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                    }
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                }
                109 | 105 => {
                    if diff(yuv_palette, w[8], w[4]) {
                        interp8(&mut buf[dst..], c[5], c[2]);
                        interp3(&mut buf[dst + dst_pitch..], c[5], c[2]);
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp1(&mut buf[dst..], c[5], c[4]);
                        interp1(&mut buf[dst + dst_pitch..], c[4], c[5]);
                        interp8(&mut buf[dst + 2 * dst_pitch..], c[4], c[8]);
                        interp7(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4], c[8]);
                        interp5(&mut buf[dst + 3 * dst_pitch..], c[8], c[4]);
                        interp2(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[8], c[5], c[4]);
                    }
                    interp8(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[2]);
                    interp7(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6], c[2]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[2]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[9],
                    );
                    interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                171 | 43 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                        interp3(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                        interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[8]);
                    } else {
                        interp5(&mut buf[dst..], c[2], c[4]);
                        interp2(&mut buf[dst + DST_BPP..], c[2], c[5], c[4]);
                        interp8(&mut buf[dst + dst_pitch..], c[4], c[2]);
                        interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4], c[2]);
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[4], c[5]);
                        interp1(&mut buf[dst + 3 * dst_pitch..], c[5], c[4]);
                    }
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[3]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp7(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                    interp2(
                        &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                }
                143 | 15 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        interp3(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                        interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[6]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst..], c[2], c[4]);
                        interp8(&mut buf[dst + DST_BPP..], c[2], c[4]);
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[2], c[5]);
                        interp1(&mut buf[dst + 3 * DST_BPP..], c[5], c[2]);
                        interp2(&mut buf[dst + dst_pitch..], c[4], c[5], c[2]);
                        interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4], c[2]);
                    }
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp7(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[7]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                    interp2(
                        &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                }
                124 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[1]);
                    interp8(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[2]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch..], c[4], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch..], c[8], c[4]);
                        interp5(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[8], c[5]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                203 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp5(&mut buf[dst..], c[2], c[4]);
                        interp5(&mut buf[dst + DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + dst_pitch..], c[4], c[5]);
                    }
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[3]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                }
                62 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + 3 * DST_BPP..], c[2], c[6]);
                        interp5(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp3(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[9],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                211 => {
                    interp8(&mut buf[dst..], c[5], c[4]);
                    interp3(&mut buf[dst + DST_BPP..], c[5], c[4]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[8], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[8], c[6]);
                    }
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                }
                118 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + 3 * DST_BPP..], c[2], c[6]);
                        interp5(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                    }
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp8(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[4]);
                    interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                217 => {
                    interp8(&mut buf[dst..], c[5], c[2]);
                    interp8(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp3(&mut buf[dst + dst_pitch..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[8], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[8], c[6]);
                    }
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                }
                110 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch..], c[4], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch..], c[8], c[4]);
                        interp5(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[8], c[5]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[9],
                    );
                    interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                155 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp5(&mut buf[dst..], c[2], c[4]);
                        interp5(&mut buf[dst + DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + dst_pitch..], c[4], c[5]);
                    }
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[7]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                }
                188 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[1]);
                    interp8(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                }
                185 => {
                    interp8(&mut buf[dst..], c[5], c[2]);
                    interp8(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp3(&mut buf[dst + dst_pitch..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    interp3(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                }
                61 => {
                    interp8(&mut buf[dst..], c[5], c[2]);
                    interp8(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp8(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[9],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                157 => {
                    interp8(&mut buf[dst..], c[5], c[2]);
                    interp8(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp8(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[7]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                }
                103 => {
                    interp8(&mut buf[dst..], c[5], c[4]);
                    interp3(&mut buf[dst + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[9],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[4]);
                    interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                227 => {
                    interp8(&mut buf[dst..], c[5], c[4]);
                    interp3(&mut buf[dst + DST_BPP..], c[5], c[4]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[3]);
                    interp8(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                }
                230 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[6]);
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                }
                199 => {
                    interp8(&mut buf[dst..], c[5], c[4]);
                    interp3(&mut buf[dst + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                }
                220 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[1]);
                    interp8(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[2]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                        interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                        interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                    } else {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                        interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[8], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[8], c[6]);
                    }
                }
                158 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp8(&mut buf[dst..], c[5], c[1]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                        interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[2], c[4]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + 3 * DST_BPP..], c[2], c[6]);
                        interp5(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                    }
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[7]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                }
                234 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp8(&mut buf[dst..], c[5], c[1]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                        interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[2], c[4]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    }
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[3]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch..], c[4], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch..], c[8], c[4]);
                        interp5(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[8], c[5]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    interp3(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                }
                242 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                        interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                        interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    } else {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                        interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    }
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp8(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4]);
                    store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[8], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[8], c[6]);
                    }
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[4]);
                }
                59 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp5(&mut buf[dst..], c[2], c[4]);
                        interp5(&mut buf[dst + DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + dst_pitch..], c[4], c[5]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                        interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                        interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    } else {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                        interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    }
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[9],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                121 => {
                    interp8(&mut buf[dst..], c[5], c[2]);
                    interp8(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp3(&mut buf[dst + dst_pitch..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch..], c[4], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch..], c[8], c[4]);
                        interp5(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[8], c[5]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                        interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    } else {
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                        interp2(
                            &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                            c[5],
                            c[8],
                            c[6],
                        );
                    }
                }
                87 => {
                    interp8(&mut buf[dst..], c[5], c[4]);
                    interp3(&mut buf[dst + DST_BPP..], c[5], c[4]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + 3 * DST_BPP..], c[2], c[6]);
                        interp5(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                    }
                    interp8(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                        interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    } else {
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                        interp2(
                            &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                            c[5],
                            c[8],
                            c[6],
                        );
                    }
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                }
                79 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp5(&mut buf[dst..], c[2], c[4]);
                        interp5(&mut buf[dst + DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + dst_pitch..], c[4], c[5]);
                    }
                    interp3(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[6]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                        interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                        interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                    } else {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                        interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    }
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[9],
                    );
                    interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                122 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp8(&mut buf[dst..], c[5], c[1]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                        interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[2], c[4]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                        interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                        interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    } else {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                        interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    }
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch..], c[4], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch..], c[8], c[4]);
                        interp5(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[8], c[5]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                        interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    } else {
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                        interp2(
                            &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                            c[5],
                            c[8],
                            c[6],
                        );
                    }
                }
                94 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp8(&mut buf[dst..], c[5], c[1]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                        interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[2], c[4]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + 3 * DST_BPP..], c[2], c[6]);
                        interp5(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                    }
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                        interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                        interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                    } else {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                        interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    }
                    if diff(yuv_palette, w[6], w[8]) {
                        interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                        interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    } else {
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                        interp2(
                            &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                            c[5],
                            c[8],
                            c[6],
                        );
                    }
                }
                218 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp8(&mut buf[dst..], c[5], c[1]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                        interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[2], c[4]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                        interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                        interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    } else {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                        interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    }
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                        interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                        interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                    } else {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                        interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[8], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[8], c[6]);
                    }
                }
                91 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp5(&mut buf[dst..], c[2], c[4]);
                        interp5(&mut buf[dst + DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + dst_pitch..], c[4], c[5]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                        interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                        interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    } else {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                        interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    }
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                        interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                        interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                    } else {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                        interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    }
                    if diff(yuv_palette, w[6], w[8]) {
                        interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                        interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    } else {
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                        interp2(
                            &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                            c[5],
                            c[8],
                            c[6],
                        );
                    }
                }
                229 => {
                    interp2(&mut buf[dst..], c[5], c[2], c[4]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[4]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[2]);
                    interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4], c[2]);
                    interp7(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6], c[2]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[2]);
                    interp8(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                }
                167 => {
                    interp8(&mut buf[dst..], c[5], c[4]);
                    interp3(&mut buf[dst + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[8]);
                    interp7(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4], c[8]);
                    interp7(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[4]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                    interp2(
                        &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                }
                173 => {
                    interp8(&mut buf[dst..], c[5], c[2]);
                    interp8(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                    interp3(&mut buf[dst + dst_pitch..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[2]);
                    interp7(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6], c[2]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[2]);
                    interp3(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp7(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                    interp2(
                        &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                }
                181 => {
                    interp2(&mut buf[dst..], c[5], c[2], c[4]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[4]);
                    interp8(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[2]);
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[2]);
                    interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[2]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[8]);
                    interp7(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                    interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[4]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                }
                186 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp8(&mut buf[dst..], c[5], c[1]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                        interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[2], c[4]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                        interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                        interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    } else {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                        interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    }
                    interp3(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                }
                115 => {
                    interp8(&mut buf[dst..], c[5], c[4]);
                    interp3(&mut buf[dst + DST_BPP..], c[5], c[4]);
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                        interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                        interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    } else {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                        interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    }
                    interp8(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4]);
                    interp8(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                        interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    } else {
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                        interp2(
                            &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                            c[5],
                            c[8],
                            c[6],
                        );
                    }
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[4]);
                }
                93 => {
                    interp8(&mut buf[dst..], c[5], c[2]);
                    interp8(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp8(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[2]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                        interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                        interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                    } else {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                        interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    }
                    if diff(yuv_palette, w[6], w[8]) {
                        interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                        interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    } else {
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                        interp2(
                            &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                            c[5],
                            c[8],
                            c[6],
                        );
                    }
                }
                206 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp8(&mut buf[dst..], c[5], c[1]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                        interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[2], c[4]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    }
                    interp3(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[6]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                        interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                        interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                    } else {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                        interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    }
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    interp3(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                }
                205 | 201 => {
                    interp8(&mut buf[dst..], c[5], c[2]);
                    interp8(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                    interp3(&mut buf[dst + dst_pitch..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[2]);
                    interp7(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6], c[2]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[2]);
                    if diff(yuv_palette, w[8], w[4]) {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                        interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                        interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                        interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                    } else {
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                        interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    }
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    interp3(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                }
                174 | 46 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        interp8(&mut buf[dst..], c[5], c[1]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                        interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[2], c[4]);
                        interp1(&mut buf[dst + DST_BPP..], c[5], c[2]);
                        interp1(&mut buf[dst + dst_pitch..], c[5], c[4]);
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    }
                    interp3(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[6]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    interp3(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp7(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                    interp2(
                        &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                }
                179 | 147 => {
                    interp8(&mut buf[dst..], c[5], c[4]);
                    interp3(&mut buf[dst + DST_BPP..], c[5], c[4]);
                    if diff(yuv_palette, w[2], w[6]) {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                        interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                        interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                        interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    } else {
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                        interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    }
                    interp8(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[8]);
                    interp7(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                    interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[4]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                }
                117 | 116 => {
                    interp2(&mut buf[dst..], c[5], c[2], c[4]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[4]);
                    interp8(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[2]);
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[2]);
                    interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[2]);
                    interp8(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                        interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    } else {
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                        interp2(
                            &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                            c[5],
                            c[8],
                            c[6],
                        );
                    }
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[4]);
                }
                189 => {
                    interp8(&mut buf[dst..], c[5], c[2]);
                    interp8(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp8(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                }
                231 => {
                    interp8(&mut buf[dst..], c[5], c[4]);
                    interp3(&mut buf[dst + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                }
                126 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + 3 * DST_BPP..], c[2], c[6]);
                        interp5(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch..], c[4], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch..], c[8], c[4]);
                        interp5(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[8], c[5]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                219 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp5(&mut buf[dst..], c[2], c[4]);
                        interp5(&mut buf[dst + DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + dst_pitch..], c[4], c[5]);
                    }
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[8], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[8], c[6]);
                    }
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                }
                125 => {
                    if diff(yuv_palette, w[8], w[4]) {
                        interp8(&mut buf[dst..], c[5], c[2]);
                        interp3(&mut buf[dst + dst_pitch..], c[5], c[2]);
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp1(&mut buf[dst..], c[5], c[4]);
                        interp1(&mut buf[dst + dst_pitch..], c[4], c[5]);
                        interp8(&mut buf[dst + 2 * dst_pitch..], c[4], c[8]);
                        interp7(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4], c[8]);
                        interp5(&mut buf[dst + 3 * dst_pitch..], c[8], c[4]);
                        interp2(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[8], c[5], c[4]);
                    }
                    interp8(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp8(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                221 => {
                    interp8(&mut buf[dst..], c[5], c[2]);
                    interp8(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp8(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    if diff(yuv_palette, w[6], w[8]) {
                        interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[2]);
                        interp3(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[2]);
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp1(&mut buf[dst + 3 * DST_BPP..], c[5], c[6]);
                        interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                        interp7(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                        interp8(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[6], c[8]);
                        interp2(
                            &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                            c[8],
                            c[5],
                            c[6],
                        );
                        interp5(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[8], c[6]);
                    }
                    interp3(&mut buf[dst + dst_pitch..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                }
                207 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        interp3(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                        interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[6]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst..], c[2], c[4]);
                        interp8(&mut buf[dst + DST_BPP..], c[2], c[4]);
                        interp1(&mut buf[dst + 2 * DST_BPP..], c[2], c[5]);
                        interp1(&mut buf[dst + 3 * DST_BPP..], c[5], c[2]);
                        interp2(&mut buf[dst + dst_pitch..], c[4], c[5], c[2]);
                        interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4], c[2]);
                    }
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                }
                238 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[6]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5]);
                        interp3(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                        interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    } else {
                        interp2(&mut buf[dst + 2 * dst_pitch..], c[4], c[5], c[8]);
                        interp7(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4], c[8]);
                        interp5(&mut buf[dst + 3 * dst_pitch..], c[8], c[4]);
                        interp8(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[8], c[4]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[8], c[5]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                    }
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                }
                190 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5]);
                        interp3(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                        interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                    } else {
                        interp2(&mut buf[dst + 2 * DST_BPP..], c[2], c[5], c[6]);
                        interp5(&mut buf[dst + 3 * DST_BPP..], c[2], c[6]);
                        interp7(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6], c[2]);
                        interp8(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[6], c[2]);
                        interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                        interp1(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                }
                187 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                        store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                        interp3(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                        interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[8]);
                    } else {
                        interp5(&mut buf[dst..], c[2], c[4]);
                        interp2(&mut buf[dst + DST_BPP..], c[2], c[5], c[4]);
                        interp8(&mut buf[dst + dst_pitch..], c[4], c[2]);
                        interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4], c[2]);
                        interp1(&mut buf[dst + 2 * dst_pitch..], c[4], c[5]);
                        interp1(&mut buf[dst + 3 * dst_pitch..], c[5], c[4]);
                    }
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                }
                243 => {
                    interp8(&mut buf[dst..], c[5], c[4]);
                    interp3(&mut buf[dst + DST_BPP..], c[5], c[4]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5]);
                        interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[4]);
                        interp3(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[4]);
                        store(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp7(
                            &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                            c[5],
                            c[6],
                            c[8],
                        );
                        interp2(
                            &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                            c[6],
                            c[5],
                            c[8],
                        );
                        interp1(&mut buf[dst + 3 * dst_pitch..], c[5], c[8]);
                        interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[8], c[5]);
                        interp8(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[8], c[6]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[8], c[6]);
                    }
                }
                119 => {
                    if diff(yuv_palette, w[2], w[6]) {
                        interp8(&mut buf[dst..], c[5], c[4]);
                        interp3(&mut buf[dst + DST_BPP..], c[5], c[4]);
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp1(&mut buf[dst..], c[5], c[2]);
                        interp1(&mut buf[dst + DST_BPP..], c[2], c[5]);
                        interp8(&mut buf[dst + 2 * DST_BPP..], c[2], c[6]);
                        interp5(&mut buf[dst + 3 * DST_BPP..], c[2], c[6]);
                        interp7(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6], c[2]);
                        interp2(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[6], c[5], c[2]);
                    }
                    interp8(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4]);
                    interp8(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[4]);
                    interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                237 | 233 => {
                    interp8(&mut buf[dst..], c[5], c[2]);
                    interp8(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[6]);
                    interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                    interp3(&mut buf[dst + dst_pitch..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[2]);
                    interp7(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6], c[2]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[2]);
                    store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 3 * dst_pitch..], c[5]);
                    } else {
                        interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                    }
                    store(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                }
                175 | 47 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[2], c[4]);
                    }
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[6]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    interp3(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp7(
                        &mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[8],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                    interp2(
                        &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[8],
                        c[6],
                    );
                }
                183 | 151 => {
                    interp8(&mut buf[dst..], c[5], c[4]);
                    interp3(&mut buf[dst + DST_BPP..], c[5], c[4]);
                    store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 3 * DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp8(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[8]);
                    interp7(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                    interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[4]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                }
                245 | 244 => {
                    interp2(&mut buf[dst..], c[5], c[2], c[4]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[4]);
                    interp8(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[2]);
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[2]);
                    interp7(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[2]);
                    interp8(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4]);
                    store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    store(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[4]);
                    store(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp2(
                            &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                            c[5],
                            c[8],
                            c[6],
                        );
                    }
                }
                250 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch..], c[4], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch..], c[8], c[4]);
                        interp5(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[8], c[5]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[8], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[8], c[6]);
                    }
                }
                123 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp5(&mut buf[dst..], c[2], c[4]);
                        interp5(&mut buf[dst + DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + dst_pitch..], c[4], c[5]);
                    }
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch..], c[4], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch..], c[8], c[4]);
                        interp5(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[8], c[5]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                95 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp5(&mut buf[dst..], c[2], c[4]);
                        interp5(&mut buf[dst + DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + dst_pitch..], c[4], c[5]);
                    }
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + 3 * DST_BPP..], c[2], c[6]);
                        interp5(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                    }
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                222 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + 3 * DST_BPP..], c[2], c[6]);
                        interp5(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[8], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[8], c[6]);
                    }
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                }
                252 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp6(&mut buf[dst + DST_BPP..], c[5], c[2], c[1]);
                    interp8(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[2]);
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[2]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch..], c[4], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch..], c[8], c[4]);
                        interp5(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[8], c[5]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    store(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5]);
                    store(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp2(
                            &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                            c[5],
                            c[8],
                            c[6],
                        );
                    }
                }
                249 => {
                    interp8(&mut buf[dst..], c[5], c[2]);
                    interp8(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp6(&mut buf[dst + 2 * DST_BPP..], c[5], c[2], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    interp3(&mut buf[dst + dst_pitch..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[8], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[8], c[6]);
                    }
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 3 * dst_pitch..], c[5]);
                    } else {
                        interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                    }
                    store(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5]);
                }
                235 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp5(&mut buf[dst..], c[2], c[4]);
                        interp5(&mut buf[dst + DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + dst_pitch..], c[4], c[5]);
                    }
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp6(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6], c[3]);
                    store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 3 * dst_pitch..], c[5]);
                    } else {
                        interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                    }
                    store(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                }
                111 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[2], c[4]);
                    }
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[6]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch..], c[4], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch..], c[8], c[4]);
                        interp5(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[8], c[5]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp6(
                        &mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..],
                        c[5],
                        c[6],
                        c[9],
                    );
                    interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                63 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[2], c[4]);
                    }
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + 3 * DST_BPP..], c[2], c[6]);
                        interp5(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                    }
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp3(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp6(
                        &mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..],
                        c[5],
                        c[8],
                        c[9],
                    );
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                159 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp5(&mut buf[dst..], c[2], c[4]);
                        interp5(&mut buf[dst + DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + dst_pitch..], c[4], c[5]);
                    }
                    store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 3 * DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                    }
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp6(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8], c[7]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                }
                215 => {
                    interp8(&mut buf[dst..], c[5], c[4]);
                    interp3(&mut buf[dst + DST_BPP..], c[5], c[4]);
                    store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 3 * DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp8(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5]);
                    interp6(&mut buf[dst + 2 * dst_pitch..], c[5], c[4], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[8], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[8], c[6]);
                    }
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                }
                246 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + 3 * DST_BPP..], c[2], c[6]);
                        interp5(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                    }
                    interp6(&mut buf[dst + dst_pitch..], c[5], c[4], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    interp8(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4]);
                    store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    store(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[4]);
                    store(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp2(
                            &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                            c[5],
                            c[8],
                            c[6],
                        );
                    }
                }
                254 => {
                    interp8(&mut buf[dst..], c[5], c[1]);
                    interp1(&mut buf[dst + DST_BPP..], c[5], c[1]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + 3 * DST_BPP..], c[2], c[6]);
                        interp5(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                    }
                    interp1(&mut buf[dst + dst_pitch..], c[5], c[1]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[1]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch..], c[4], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch..], c[8], c[4]);
                        interp5(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[8], c[5]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    store(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5]);
                    store(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp2(
                            &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                            c[5],
                            c[8],
                            c[6],
                        );
                    }
                }
                253 => {
                    interp8(&mut buf[dst..], c[5], c[2]);
                    interp8(&mut buf[dst + DST_BPP..], c[5], c[2]);
                    interp8(&mut buf[dst + 2 * DST_BPP..], c[5], c[2]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[2]);
                    interp3(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[2]);
                    store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    store(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 3 * dst_pitch..], c[5]);
                    } else {
                        interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                    }
                    store(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp2(
                            &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                            c[5],
                            c[8],
                            c[6],
                        );
                    }
                }
                251 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp5(&mut buf[dst..], c[2], c[4]);
                        interp5(&mut buf[dst + DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + dst_pitch..], c[4], c[5]);
                    }
                    interp1(&mut buf[dst + 2 * DST_BPP..], c[5], c[3]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[3]);
                    interp1(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[3]);
                    store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[8], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[8], c[6]);
                    }
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 3 * dst_pitch..], c[5]);
                    } else {
                        interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                    }
                    store(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5]);
                }
                239 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[2], c[4]);
                    }
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * DST_BPP..], c[5], c[6]);
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 3 * dst_pitch..], c[5]);
                    } else {
                        interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                    }
                    store(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[6]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[6]);
                }
                127 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[2], c[4]);
                    }
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + 3 * DST_BPP..], c[2], c[6]);
                        interp5(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                    }
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch..], c[4], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch..], c[8], c[4]);
                        interp5(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[8], c[5]);
                    }
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                    interp1(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[9]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[9]);
                }
                191 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[2], c[4]);
                    }
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 3 * DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                    }
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5]);
                    interp3(&mut buf[dst + 2 * dst_pitch..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp3(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5], c[8]);
                    interp8(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5], c[8]);
                }
                223 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                        store(&mut buf[dst + DST_BPP..], c[5]);
                        store(&mut buf[dst + dst_pitch..], c[5]);
                    } else {
                        interp5(&mut buf[dst..], c[2], c[4]);
                        interp5(&mut buf[dst + DST_BPP..], c[2], c[5]);
                        interp5(&mut buf[dst + dst_pitch..], c[4], c[5]);
                    }
                    store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 3 * DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                    }
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5]);
                    interp1(&mut buf[dst + 2 * dst_pitch..], c[5], c[7]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[7]);
                    store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5]);
                        store(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp5(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[6], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[8], c[5]);
                        interp5(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[8], c[6]);
                    }
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[7]);
                    interp1(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[7]);
                }
                247 => {
                    interp8(&mut buf[dst..], c[5], c[4]);
                    interp3(&mut buf[dst + DST_BPP..], c[5], c[4]);
                    store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 3 * DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                    }
                    interp8(&mut buf[dst + dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + dst_pitch + DST_BPP..], c[5], c[4]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5]);
                    interp8(&mut buf[dst + 2 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5], c[4]);
                    store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    store(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5]);
                    interp8(&mut buf[dst + 3 * dst_pitch..], c[5], c[4]);
                    interp3(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5], c[4]);
                    store(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp2(
                            &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                            c[5],
                            c[8],
                            c[6],
                        );
                    }
                }
                255 => {
                    if diff(yuv_palette, w[4], w[2]) {
                        store(&mut buf[dst..], c[5]);
                    } else {
                        interp2(&mut buf[dst..], c[5], c[2], c[4]);
                    }
                    store(&mut buf[dst + DST_BPP..], c[5]);
                    store(&mut buf[dst + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[2], w[6]) {
                        store(&mut buf[dst + 3 * DST_BPP..], c[5]);
                    } else {
                        interp2(&mut buf[dst + 3 * DST_BPP..], c[5], c[2], c[6]);
                    }
                    store(&mut buf[dst + dst_pitch..], c[5]);
                    store(&mut buf[dst + dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 2 * DST_BPP..], c[5]);
                    store(&mut buf[dst + dst_pitch + 3 * DST_BPP..], c[5]);
                    store(&mut buf[dst + 2 * dst_pitch..], c[5]);
                    store(&mut buf[dst + 2 * dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + 2 * dst_pitch + 2 * DST_BPP..], c[5]);
                    store(&mut buf[dst + 2 * dst_pitch + 3 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[8], w[4]) {
                        store(&mut buf[dst + 3 * dst_pitch..], c[5]);
                    } else {
                        interp2(&mut buf[dst + 3 * dst_pitch..], c[5], c[8], c[4]);
                    }
                    store(&mut buf[dst + 3 * dst_pitch + DST_BPP..], c[5]);
                    store(&mut buf[dst + 3 * dst_pitch + 2 * DST_BPP..], c[5]);
                    if diff(yuv_palette, w[6], w[8]) {
                        store(&mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..], c[5]);
                    } else {
                        interp2(
                            &mut buf[dst + 3 * dst_pitch + 3 * DST_BPP..],
                            c[5],
                            c[8],
                            c[6],
                        );
                    }
                }
                _ => {}
            }

            src += 1;
            dst += 4 * DST_BPP;
        }

        src = src_temp.wrapping_add_signed(src_pitch as isize);
        dst = dst_temp + 4 * dst_pitch;
    }

    g.platform.unlock_texture(dst_texture);
}
