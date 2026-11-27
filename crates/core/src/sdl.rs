// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) rustyrian contributors

//! Native versions of the pure-memory SDL helpers upstream uses
//! (`SDL_CreateRGBSurface`, `SDL_FillRect`, `SDL_UpperBlit`, `SDL_AllocFormat`,
//! `SDL_MapRGB`, `SDL_GetPixelFormatName`), following SDL 2.32's
//! `SDL_surface.c`/`SDL_fillrect.c`/`SDL_pixels.c` for the formats upstream
//! creates. The oracle links the real SDL for these, so every differential test
//! that touches a surface checks these against SDL.

use crate::platform::Rect;
use serde::{Deserialize, Serialize};

/// An 8-bit `SDL_Surface` (the only kind upstream creates).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Surface {
    pub w: i32,
    pub h: i32,
    pub pitch: i32,
    pub pixels: Vec<u8>,
    pub clip_rect: Rect,
}

impl Surface {
    /// `SDL_CreateRGBSurface(0, w, h, 8, 0, 0, 0, 0)`: zeroed pixels, pitch
    /// rounded up to 4 bytes, clip rectangle covering the surface.
    pub fn new_8bit(w: i32, h: i32) -> Surface {
        let pitch = (w + 3) & !3;
        let len = usize::try_from(pitch).unwrap_or(0) * usize::try_from(h).unwrap_or(0);
        Surface {
            w,
            h,
            pitch,
            pixels: vec![0; len],
            clip_rect: Rect { x: 0, y: 0, w, h },
        }
    }

    /// `SDL_FillRect(surface, rect, color)` for 8 bpp.
    pub fn fill_rect(&mut self, rect: Option<Rect>, color: u32) -> i32 {
        let rect = match rect {
            Some(r) => r,
            None => {
                if rect_empty(&self.clip_rect) {
                    return 0;
                }
                self.clip_rect
            }
        };
        if self.w == 0 || self.h == 0 {
            return 0;
        }
        let Some(c) = intersect_rect(&rect, &self.clip_rect) else {
            return 0;
        };
        let pitch = self.pitch as usize;
        for y in c.y..c.y + c.h {
            let start = y as usize * pitch + c.x as usize;
            self.pixels[start..start + c.w as usize].fill(color as u8);
        }
        0
    }
}

pub fn rect_empty(r: &Rect) -> bool {
    r.w <= 0 || r.h <= 0
}

/// `SDL_IntersectRect`; `None` where SDL returns `SDL_FALSE`.
pub fn intersect_rect(a: &Rect, b: &Rect) -> Option<Rect> {
    if rect_empty(a) || rect_empty(b) {
        return None;
    }
    let x = a.x.max(b.x);
    let w = (a.x + a.w).min(b.x + b.w) - x;
    let y = a.y.max(b.y);
    let h = (a.y + a.h).min(b.y + b.h) - y;
    let r = Rect { x, y, w, h };
    if rect_empty(&r) { None } else { Some(r) }
}

/// `SDL_UpperBlit` (`SDL_BlitSurface`) between two 8-bit surfaces with
/// identical palettes: a clipped copy. Updates `dstrect` as SDL does.
pub fn upper_blit(
    src: &Surface,
    srcrect: Option<&Rect>,
    dst: &mut Surface,
    dstrect: Option<&mut Rect>,
) -> i32 {
    let mut r_src = Rect {
        x: 0,
        y: 0,
        w: src.w,
        h: src.h,
    };
    let (mut dx, mut dy) = dstrect.as_ref().map_or((0, 0), |d| (d.x, d.y));
    let clipped = (|| {
        if let Some(s) = srcrect {
            let tmp = intersect_rect(s, &r_src)?;
            dx += tmp.x - s.x;
            dy += tmp.y - s.y;
            r_src = tmp;
        }
        let r_dst = Rect {
            x: dx,
            y: dy,
            w: r_src.w,
            h: r_src.h,
        };
        let tmp = intersect_rect(&r_dst, &dst.clip_rect)?;
        r_src.x += tmp.x - r_dst.x;
        r_src.y += tmp.y - r_dst.y;
        r_src.w = tmp.w;
        r_src.h = tmp.h;
        Some(tmp)
    })();
    match clipped {
        Some(r_dst) if r_dst.w > 0 && r_dst.h > 0 => {
            if let Some(d) = dstrect {
                *d = r_dst;
            }
            let (sp, dp) = (src.pitch as usize, dst.pitch as usize);
            for row in 0..r_dst.h as usize {
                let s = (r_src.y as usize + row) * sp + r_src.x as usize;
                let d = (r_dst.y as usize + row) * dp + r_dst.x as usize;
                dst.pixels[d..d + r_dst.w as usize]
                    .copy_from_slice(&src.pixels[s..s + r_dst.w as usize]);
            }
            0
        }
        _ => {
            if let Some(d) = dstrect {
                d.w = 0;
                d.h = 0;
            }
            0
        }
    }
}

/// `SDL_PIXELFORMAT_RGB888`.
pub const PIXELFORMAT_RGB888: u32 = 0x1616_1804;
/// `SDL_PIXELFORMAT_RGB565`.
pub const PIXELFORMAT_RGB565: u32 = 0x1515_1002;

/// The fields of `SDL_PixelFormat` upstream reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PixelFormat {
    pub format: u32,
    pub bits_per_pixel: u8,
    pub bytes_per_pixel: u8,
}

/// `SDL_AllocFormat` for the two formats upstream uses; `None` otherwise.
pub fn alloc_format(format: u32) -> Option<PixelFormat> {
    match format {
        PIXELFORMAT_RGB888 => Some(PixelFormat {
            format,
            bits_per_pixel: 32,
            bytes_per_pixel: 4,
        }),
        PIXELFORMAT_RGB565 => Some(PixelFormat {
            format,
            bits_per_pixel: 16,
            bytes_per_pixel: 2,
        }),
        _ => None,
    }
}

/// `SDL_MapRGB`.
pub fn map_rgb(format: &PixelFormat, r: u8, g: u8, b: u8) -> u32 {
    let (r, g, b) = (u32::from(r), u32::from(g), u32::from(b));
    match format.format {
        PIXELFORMAT_RGB565 => ((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3),
        _ => (r << 16) | (g << 8) | b,
    }
}

/// `SDL_GetPixelFormatName` (SDL 2 names).
pub fn pixel_format_name(format: u32) -> &'static str {
    match format {
        PIXELFORMAT_RGB888 => "SDL_PIXELFORMAT_RGB888",
        PIXELFORMAT_RGB565 => "SDL_PIXELFORMAT_RGB565",
        _ => "SDL_PIXELFORMAT_UNKNOWN",
    }
}
