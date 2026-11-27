// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `backgrnd.c` / `backgrnd.h`.

use crate::Game;
use crate::mtrand::mt_rand;
use crate::sdl::Surface;
use crate::swar::{Swar8, swar8, swar8_read, swar8_write};
use crate::video::{ScreenId, VGA_HEIGHT, VGA_WIDTH};
use serde::{Deserialize, Serialize};

/// `StarfieldStar`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StarfieldStar {
    pub color: u8,
    /// Relies on overflow wrap-around.
    pub position: u16,
    pub speed: i32,
}

pub const MAX_STARS: usize = 100;
pub const STARFIELD_HUE: u8 = 0x90;

/// Mutable globals of this module (rule R3).
///
/// `mapYPos`, `mapY2Pos` and `mapY3Pos` (`JE_byte **`) point into the
/// `mainmap` tile-pointer arrays of `megaData1`, `megaData2` and `megaData3`
/// (`varz`). Per rule R8 they are element indices into the flattened map
/// (`row * width + column`, widths 14, 14 and 15); upstream forms
/// one-before-begin pointers (`tyrian2.c:693-695`), hence `isize`. `None` is
/// NULL.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub back_pos: u16,
    pub back_pos2: u16,
    pub back_pos3: u16,
    pub back_move: u16,
    pub back_move2: u16,
    pub back_move3: u16,
    pub map_x: u16,
    pub map_y: u16,
    pub map_x2: u16,
    pub map_x3: u16,
    pub map_y2: u16,
    pub map_y3: u16,
    pub map_y_pos: Option<isize>,
    pub map_y2_pos: Option<isize>,
    pub map_y3_pos: Option<isize>,
    pub map_x_pos: u16,
    pub old_map_x_ofs: u16,
    pub map_x_ofs: u16,
    pub map_x2_ofs: u16,
    pub map_x2_pos: u16,
    pub map_x3_pos: u16,
    pub old_map_x3_ofs: u16,
    pub map_x3_ofs: u16,
    pub temp_map_x_ofs: u16,
    /// `intptr_t`.
    pub map_xbp_pos: isize,
    pub map_x2bp_pos: isize,
    pub map_x3bp_pos: isize,
    pub map1_y_delay: u8,
    pub map1_y_delay_max: u8,
    pub map2_y_delay: u8,
    pub map2_y_delay_max: u8,
    pub any_smoothies: bool,
    /// `[1..9]`.
    pub smoothie_data: [u8; 9],
    #[serde(with = "crate::serde_big")]
    pub starfield_stars: [StarfieldStar; MAX_STARS],
    pub starfield_speed: i32,
}

impl Default for State {
    fn default() -> Self {
        State {
            back_pos: 0,
            back_pos2: 0,
            back_pos3: 0,
            back_move: 0,
            back_move2: 0,
            back_move3: 0,
            map_x: 0,
            map_y: 0,
            map_x2: 0,
            map_x3: 0,
            map_y2: 0,
            map_y3: 0,
            map_y_pos: None,
            map_y2_pos: None,
            map_y3_pos: None,
            map_x_pos: 0,
            old_map_x_ofs: 0,
            map_x_ofs: 0,
            map_x2_ofs: 0,
            map_x2_pos: 0,
            map_x3_pos: 0,
            old_map_x3_ofs: 0,
            map_x3_ofs: 0,
            temp_map_x_ofs: 0,
            map_xbp_pos: 0,
            map_x2bp_pos: 0,
            map_x3bp_pos: 0,
            map1_y_delay: 0,
            map1_y_delay_max: 0,
            map2_y_delay: 0,
            map2_y_delay_max: 0,
            any_smoothies: false,
            smoothie_data: [0; 9],
            starfield_stars: [StarfieldStar::default(); MAX_STARS],
            starfield_speed: 0,
        }
    }
}

/// `sizeof(Swar8)`.
const SW: usize = size_of::<Swar8>();

/// One row of 12 map entries: the tile pixels (`24 * 28` bytes) each entry
/// points to, `None` for NULL.
pub type TileRow<'a> = [Option<&'a [u8]>; 12];

/// The 12 tiles at flattened `mainmap` index `start` (rule R8): `mainmap`
/// entries are shape indices into `shapes`.
fn tile_row<'a, S>(
    mainmap: &[Option<u8>],
    start: isize,
    shapes: &'a [S],
    sh: impl Fn(&'a S) -> &'a [u8],
) -> TileRow<'a> {
    std::array::from_fn(|ti| {
        let i = usize::try_from(start + ti as isize).expect("map row inside mainmap");
        mainmap[i].map(|k| sh(&shapes[usize::from(k)]))
    })
}

#[doc(alias = "JE_darkenBackground")]
pub fn je_darken_background(g: &mut Game, neat: u16) {
    let screen = g.video.vga_mut();
    let pitch = screen.pitch as usize;
    let p = &mut screen.pixels;
    let mut s = 24usize;

    for y in (1..=184i32).rev() {
        for x in (1..=264i32).rev() {
            let v = i32::from(p[s]);
            let above = if y == 184 {
                0
            } else {
                i32::from(p[s - (pitch - 1)])
            };
            let n = (((x - i32::from(neat) - y) >> 2) + i32::from(p[s - 2]) + above) & 0x0f;
            p[s] = (((((v & 0x0f) << 4) - (v & 0x0f) + n) >> 4) | (v & 0xf0)) as u8;
            s += 1;
        }
        s += pitch - 264;
    }
}

/// Start of the clipped rows: `(ty, th, di)`.
fn row_bounds(dst: &Surface, x: i32, mut y: i32) -> (i32, i32, isize) {
    debug_assert!((0..24).contains(&x));
    debug_assert!((-28..VGA_HEIGHT).contains(&y));

    let mut ty = 0;
    let mut th = 28;

    // Skip pixel rows that are above top of screen.
    if y < 0 {
        ty = -y;
        y = 0;
    }
    // Stop when pixel row would be below bottom of screen.
    else if y > 184 - th {
        th = 184 - y;
    }

    (ty, th, y as isize * dst.pitch as isize + x as isize)
}

/// Draw row of 12 tiles from map. Each tile is 24x28 and is opaque.
/// Visible area is located at (24, 0) and is 264x184. Tiles are drawn beyond
/// this area to avoid the need to clip the left and right edges and to
/// support wavey smoothies.
#[doc(alias = "blitBackground1Row")]
pub fn blit_background1_row(dst: &mut Surface, x: i32, y: i32, map: &TileRow<'_>) {
    let (mut ty, th, di) = row_bounds(dst, x, y);
    let dst_pitch = dst.pitch as usize;
    let mut di = di as usize;

    while ty < th {
        for tile in map {
            let data = tile.expect("opaque background row has every tile");
            let data = &data[ty as usize * 24..];

            dst.pixels[di..di + 24].copy_from_slice(&data[..24]);

            di += 24;
        }

        di += dst_pitch - 12 * 24;
        ty += 1;
    }
}

/// `0xFF` in each lane whose pixel is not `0x00`, otherwise `0x00`.
fn nonzero_mask(src_pixels: Swar8) -> Swar8 {
    let bit7 = (src_pixels & swar8(0x7F)).wrapping_add(swar8(0x7F));
    let bit = (src_pixels | bit7) & swar8(0x80);
    bit | bit.wrapping_sub(bit >> 7)
}

/// Shared loop of [`blit_background_row`] and [`blit_background_row_blend`]:
/// `blend` turns (source, destination) lanes into the lanes to store.
fn blit_row_with(
    dst: &mut Surface,
    x: i32,
    y: i32,
    map: &TileRow<'_>,
    blend: impl Fn(Swar8, Swar8) -> Swar8,
) {
    let (mut ty, th, di) = row_bounds(dst, x, y);
    let dst_pitch = dst.pitch as usize;
    let mut di = di as usize;

    while ty < th {
        for tile in map {
            // Skip transparent tile.
            let Some(data) = tile else {
                di += 24;
                continue;
            };

            let mut si = ty as usize * 24;

            for _ in 0..24 / SW {
                let mut dst_pixels = swar8_read(&dst.pixels[di..]);
                let src_pixels = swar8_read(&data[si..]);

                let mask = nonzero_mask(src_pixels);
                let src_pixels = blend(src_pixels, dst_pixels);

                // Conditionally select each lane by mask.
                dst_pixels ^= (dst_pixels ^ src_pixels) & mask;

                swar8_write(&mut dst.pixels[di..], dst_pixels);

                si += SW;
                di += SW;
            }
        }

        di += dst_pitch - 12 * 24;
        ty += 1;
    }
}

/// Draw row of 12 tiles from map. Each tile is 24x28 and can have
/// transparent pixels.
#[doc(alias = "blitBackgroundRow")]
pub fn blit_background_row(dst: &mut Surface, x: i32, y: i32, map: &TileRow<'_>) {
    blit_row_with(dst, x, y, map, |src, _| src);
}

/// Draw row of 12 tiles from map with blending. Each tile is 24x28 and can
/// have transparent pixels.
#[doc(alias = "blitBackgroundRowBlend")]
pub fn blit_background_row_blend(dst: &mut Surface, x: i32, y: i32, map: &TileRow<'_>) {
    blit_row_with(dst, x, y, map, |src, dst| {
        let hues = src & swar8(0xF0);
        let mut values = src & swar8(0x0F);
        values += dst & swar8(0x0F);
        values = (values >> 1) & swar8(0x0F);
        hues | values
    });
}

fn assert_vga_surface(s: &Surface) {
    debug_assert!(s.w == VGA_WIDTH && s.h >= VGA_HEIGHT);
}

pub fn draw_background_1(g: &mut Game, surface: ScreenId) {
    let dst = g.video.screen_mut(surface);
    assert_vga_surface(dst);
    let b = &g.backgrnd;
    let md = &g.varz.mega_data1;

    let mut map = b.map_y_pos.expect("mapYPos is set") + b.map_xbp_pos - 12;

    for i in -1..7 {
        let row = tile_row(
            md.mainmap.as_flattened(),
            map,
            &md.shapes[..],
            |s| &s.sh[..],
        );
        blit_background1_row(
            dst,
            i32::from(b.map_x_pos),
            i * 28 + i32::from(b.back_pos),
            &row,
        );

        map += 14;
    }
}

/// Background 2 movement shared by both background-2 renderers.
fn move_background_2(g: &mut Game) {
    let b = &mut g.backgrnd;
    b.map2_y_delay = b.map2_y_delay.wrapping_sub(1);
    if b.map2_y_delay == 0 {
        b.map2_y_delay = b.map2_y_delay_max;

        b.back_pos2 = b.back_pos2.wrapping_add(b.back_move2);

        if b.back_pos2 > 27 {
            b.back_pos2 -= 28;
            b.map_y2 = b.map_y2.wrapping_sub(1);
            b.map_y2_pos = b.map_y2_pos.map(|p| p - 14); // Map Width
        }
    }
}

/// `if (map2YDelayMax > 1 && backMove2 < 2) backMove2 = map2YDelay == 1;`
fn sync_back_move_2(g: &mut Game) {
    let b = &mut g.backgrnd;
    if b.map2_y_delay_max > 1 && b.back_move2 < 2 {
        b.back_move2 = u16::from(b.map2_y_delay == 1);
    }
}

pub fn draw_background_2(g: &mut Game, surface: ScreenId) {
    assert_vga_surface(g.video.screen(surface));

    sync_back_move_2(g);

    if g.config.background2 {
        let dst = g.video.screen_mut(surface);
        let b = &g.backgrnd;
        let md = &g.varz.mega_data2;
        let smoothie = g.config.smoothies[1];

        // water effect combines background 1 and 2 by synchronizing the x
        // coordinate
        let x = i32::from(if smoothie { b.map_x_pos } else { b.map_x2_pos });

        let mut map = b.map_y2_pos.expect("mapY2Pos is set")
            + if smoothie {
                b.map_xbp_pos
            } else {
                b.map_x2bp_pos
            }
            - 12;

        for i in -1..7 {
            let row = tile_row(
                md.mainmap.as_flattened(),
                map,
                &md.shapes[..],
                |s| &s.sh[..],
            );
            blit_background_row(dst, x, i * 28 + i32::from(b.back_pos2), &row);

            map += 14;
        }
    }

    // Set Movement of background
    move_background_2(g);
}

pub fn draw_background_2_blend(g: &mut Game, surface: ScreenId) {
    assert_vga_surface(g.video.screen(surface));

    sync_back_move_2(g);

    {
        let dst = g.video.screen_mut(surface);
        let b = &g.backgrnd;
        let md = &g.varz.mega_data2;

        let mut map = b.map_y2_pos.expect("mapY2Pos is set") + b.map_x2bp_pos - 12;

        for i in -1..7 {
            let row = tile_row(
                md.mainmap.as_flattened(),
                map,
                &md.shapes[..],
                |s| &s.sh[..],
            );
            blit_background_row_blend(
                dst,
                i32::from(b.map_x2_pos),
                i * 28 + i32::from(b.back_pos2),
                &row,
            );

            map += 14;
        }
    }

    // Set Movement of background
    move_background_2(g);
}

pub fn draw_background_3(g: &mut Game, surface: ScreenId) {
    let dst = g.video.screen_mut(surface);
    assert_vga_surface(dst);
    let b = &mut g.backgrnd;

    // Movement of background
    b.back_pos3 = b.back_pos3.wrapping_add(b.back_move3);

    if b.back_pos3 > 27 {
        b.back_pos3 -= 28;
        b.map_y3 = b.map_y3.wrapping_sub(1);
        b.map_y3_pos = b.map_y3_pos.map(|p| p - 15); // Map Width
    }

    let md = &g.varz.mega_data3;
    let mut map = b.map_y3_pos.expect("mapY3Pos is set") + b.map_x3bp_pos - 12;

    for i in -1..7 {
        let row = tile_row(
            md.mainmap.as_flattened(),
            map,
            &md.shapes[..],
            |s| &s.sh[..],
        );
        blit_background_row(
            dst,
            i32::from(b.map_x3_pos),
            i * 28 + i32::from(b.back_pos3),
            &row,
        );

        map += 15;
    }
}

/// Applies `f` to every 8-pixel group of the visible 264x184 area.
fn for_each_visible(screen: &mut Surface, f: impl Fn(Swar8) -> Swar8) {
    let pitch = screen.pitch as usize;
    let mut i = 24;

    for _ in 0..184 {
        for _ in 0..264 / SW {
            let v = f(swar8_read(&screen.pixels[i..]));
            swar8_write(&mut screen.pixels[i..], v);

            i += SW;
        }

        i += pitch - 264;
    }
}

/// Hue is configurable. Value is unmodified.
#[doc(alias = "filterScreenHue")]
pub fn filter_screen_hue(screen: &mut Surface, hue: u8) {
    let hue = hue << 4;
    let hues = swar8(hue);

    for_each_visible(screen, |p| hues | (p & swar8(0x0F)));
}

/// Hue is unmodified. Value is sum of value and brightness if sum is
/// 0x0..0xF, 0xF if sum is 0x10..0x1E, and 0x0 otherwise.
#[doc(alias = "filterScreenValue")]
pub fn filter_screen_value(screen: &mut Surface, brightness: i8) {
    if brightness == 0 {
        return;
    }

    if brightness <= -0xF || brightness > 0x1E {
        // In this case, value will always be 0x0.
        for_each_visible(screen, |p| p & swar8(0xF0));
    } else if brightness <= 0 {
        // -0xE..0x0: value will be the sum or 0x0 on overflow (borrow).
        // Addend is 0x02..0x10. Only lower nibble of sum needs to be correct,
        // so adding 0x10 does not affect the sum.
        let addend = swar8((brightness as i32 + 0x10) as u8);

        for_each_visible(screen, |p| {
            let hues = p & swar8(0xF0);
            let sum = (p & swar8(0x0F)) + addend;

            // Mask is 0x0F if value + brightness >= 0x00, otherwise 0x00.
            let bit = sum & swar8(0x10);
            let mask = bit - (bit >> 4);
            hues | (sum & mask)
        });
    } else if brightness <= 0xF {
        // 0x1..0xF: value will be the sum or 0xF on overflow (carry).
        let addend = swar8(brightness as u8);

        for_each_visible(screen, |p| {
            let hues = p & swar8(0xF0);
            let sum = (p & swar8(0x0F)) + addend;

            // Mask is 0x0F if value + brightness >= 0x10, otherwise 0x00.
            let bit = sum & swar8(0x10);
            let mask = bit - (bit >> 4);
            hues | (sum & swar8(0x0F)) | mask
        });
    } else {
        // 0x10..0x1E: value will be either 0xF or 0x0.
        let addend = swar8((brightness as i32 - 0xF) as u8);

        for_each_visible(screen, |p| {
            let hues = p & swar8(0xF0);
            let sum = (p & swar8(0x0F)) + addend;

            // Mask is 0x0F if value + brightness >= 0x1F, otherwise 0x00.
            let bit = sum & swar8(0x10);
            let mask = bit - (bit >> 4);
            hues | (swar8(0x0F) & !mask)
        });
    }
}

#[doc(alias = "JE_filterScreen")]
pub fn je_filter_screen(g: &mut Game, col: i8, brightness: i8) {
    assert_vga_surface(g.video.vga());

    let c = &mut g.config;
    if c.filter_fade {
        c.level_brightness = c.level_brightness.wrapping_add(c.level_brightness_chg);
        if (c.filter_fade_start && c.level_brightness < -14) || c.level_brightness > 14 {
            c.level_brightness_chg = c.level_brightness_chg.wrapping_neg();
            c.filter_fade_start = false;
            c.level_filter = c.level_filter_new;
        }
        if !c.filter_fade_start && c.level_brightness == 0 {
            c.filter_fade = false;
            c.level_brightness = -99;
        }
    }

    if col != -99 && g.config.filtration_avail {
        filter_screen_hue(g.video.vga_mut(), col as u8);
    }

    if brightness != -99 && g.config.explosion_transparent {
        filter_screen_value(g.video.vga_mut(), brightness);
    }
}

#[doc(alias = "JE_checkSmoothies")]
pub fn je_check_smoothies(g: &mut Game) {
    let c = &g.config;
    let s = &c.smoothies;
    g.backgrnd.any_smoothies = (c.processor_type > 2 && (s[0] || s[1]))
        || (c.processor_type > 1 && (s[2] || s[3] || s[4]));
}

/// `abs((int)((diw >> shift) & mask) - half) - 1`.
fn wavey(diw: u32, shift: u32, mask: u32, half: i32) -> isize {
    (((diw >> shift) & mask) as i32 - half).abs() as isize - 1
}

/// `pixels[i..]` for a C index computed in pointer arithmetic.
fn at(i: isize) -> usize {
    usize::try_from(i).expect("pixel index inside the surface")
}

pub fn lava_filter(g: &mut Game, dst: ScreenId, src: ScreenId) {
    let (src, dst) = g.video.screen_pair_mut(src, dst);
    assert_vga_surface(src);
    assert_vga_surface(dst);

    let src_pitch = src.pitch as isize;
    let dst_pitch = dst.pitch as isize;
    let src_pixels = &src.pixels;
    let dst_pixels = &mut dst.pixels;
    let vga_width = VGA_WIDTH as isize;

    // The original implementation also worked on 8-pixel chunks, but the
    // chunks were misaligned, so this implementation is slightly different,
    // but it's faster and essentially indistinguishable.

    let mut diw: u32 = VGA_WIDTH as u32 * 185;
    let mut di = dst_pitch * 185;
    let mut si = src_pitch * 185;
    let sw = SW as isize;

    for _ in 0..184 {
        di -= dst_pitch - vga_width;
        si -= src_pitch - vga_width;

        for _ in 0..VGA_WIDTH / 8 {
            let wavey = wavey(diw, 9, 0xF, 8); // -1..7

            // Value is average of values of wavey source pixel (2x), wavey
            // destination pixel below, wavey and destination pixel above.
            // Hue is red.
            for _ in 0..8 / SW {
                di -= sw;
                si -= sw;

                let mut values = (swar8_read(&src_pixels[at(si + wavey)..]) & swar8(0x0F)) << 1;
                values += swar8_read(&dst_pixels[at(di + wavey + dst_pitch)..]) & swar8(0x0F);
                values += swar8_read(&dst_pixels[at(di + wavey - dst_pitch)..]) & swar8(0x0F);
                values = (values >> 2) & swar8(0x0F);
                swar8_write(&mut dst_pixels[at(di)..], swar8(0x70) | values);
            }
            diw -= 8;
        }
    }

    di -= dst_pitch - vga_width;
    si -= src_pitch - vga_width;

    for _ in 0..VGA_WIDTH / 8 {
        let wavey = wavey(diw, 9, 0xF, 8);
        debug_assert!(wavey > 0);

        for _ in 0..8 / SW {
            di -= sw;
            si -= sw;

            let mut values = (swar8_read(&src_pixels[at(si + wavey)..]) & swar8(0x0F)) << 1;
            values += swar8_read(&dst_pixels[at(di + wavey + dst_pitch)..]) & swar8(0x0F);
            // Assume values above top row of pixels are zero.
            values = (values >> 2) & swar8(0x0F);
            swar8_write(&mut dst_pixels[at(di)..], swar8(0x70) | values);
        }
        diw -= 8;
    }

    debug_assert!(diw == 0 && di == 0 && si == 0);
}

pub fn water_filter(g: &mut Game, dst: ScreenId, src: ScreenId) {
    let hue = g.backgrnd.smoothie_data[2 - 1] << 4;
    let (src, dst) = g.video.screen_pair_mut(src, dst);
    assert_vga_surface(src);
    assert_vga_surface(dst);

    let src_pitch = src.pitch as isize;
    let dst_pitch = dst.pitch as isize;
    let src_pixels = &src.pixels;
    let dst_pixels = &mut dst.pixels;
    let vga_width = VGA_WIDTH as isize;

    let hues = swar8(hue);

    let mut diw: u32 = VGA_WIDTH as u32 * 185;
    let mut di = dst_pitch * 185;
    let mut si = src_pitch * 185;
    let sw = SW as isize;

    for _ in 0..185 {
        di -= dst_pitch - vga_width;
        si -= src_pitch - vga_width;

        for _ in 0..VGA_WIDTH / 8 {
            let wavey = wavey(diw, 10, 0x7, 4); // -1..3

            // Pixel is copied from source if hue is gray or cloud-blue.
            // Otherwise: value is average of values of source pixel and wavey
            // destination pixel below. Hue is configurable.
            for _ in 0..8 / SW {
                di -= sw;
                si -= sw;

                let src_px = swar8_read(&src_pixels[at(si)..]);

                let mut water_mask = src_px & swar8(0x30);
                // Put 0x01 in lanes where either of the bits is set, and 0x00
                // in others.
                water_mask = ((water_mask >> 5) | (water_mask >> 4)) & swar8(0x01);
                // Put 0xFF in lanes that were 0x01.
                water_mask = (water_mask << 8).wrapping_sub(water_mask);

                let mut values = src_px & swar8(0x0F);
                values += swar8_read(&dst_pixels[at(di + wavey + dst_pitch)..]) & swar8(0x0F);
                values = (values >> 1) & swar8(0x0F);
                let water_pixels = hues | values;

                // Conditionally select each lane by mask.
                let dst_px = src_px ^ ((src_px ^ water_pixels) & water_mask);

                swar8_write(&mut dst_pixels[at(di)..], dst_px);
            }
            diw -= 8;
        }
    }

    debug_assert!(diw == 0 && di == 0 && si == 0);
}

/// Shared loop of [`iced_blur_filter`] and [`blur_filter`]: `f` maps
/// (source, destination) lanes to the new destination lanes.
fn blur_with(g: &mut Game, dst: ScreenId, src: ScreenId, f: impl Fn(Swar8, Swar8) -> Swar8) {
    let (src, dst) = g.video.screen_pair_mut(src, dst);
    assert_vga_surface(src);
    assert_vga_surface(dst);

    let src_pitch = src.pitch as usize;
    let dst_pitch = dst.pitch as usize;
    let vga_width = VGA_WIDTH as usize;

    let mut di = dst_pitch * 184;
    let mut si = src_pitch * 184;

    for _ in 0..184 {
        di -= dst_pitch - vga_width;
        si -= src_pitch - vga_width;

        for _ in 0..vga_width / 8 {
            for _ in 0..8 / SW {
                di -= SW;
                si -= SW;

                let v = f(swar8_read(&src.pixels[si..]), swar8_read(&dst.pixels[di..]));
                swar8_write(&mut dst.pixels[di..], v);
            }
        }
    }

    debug_assert!(di == 0 && si == 0);
}

/// Value is average of values of source pixel and destination pixel. Hue is
/// ice-blue.
pub fn iced_blur_filter(g: &mut Game, dst: ScreenId, src: ScreenId) {
    blur_with(g, dst, src, |s, d| {
        let mut values = s & swar8(0x0F);
        values += d & swar8(0x0F);
        values = (values >> 1) & swar8(0x0F);
        swar8(0x80) | values
    });
}

/// Value is average of values of source pixel and destination pixel. Hue is
/// hue of source pixel.
pub fn blur_filter(g: &mut Game, dst: ScreenId, src: ScreenId) {
    blur_with(g, dst, src, |s, d| {
        let hues = s & swar8(0xF0);
        let mut values = s & swar8(0x0F);
        values += d & swar8(0x0F);
        values = (values >> 1) & swar8(0x0F);
        hues | values
    });
}

/// Dark pixel group: value divided by four, hue kept.
fn dark_lanes(p: Swar8) -> Swar8 {
    (p & swar8(0xF0)) | (((p & swar8(0x0F)) >> 2) & swar8(0x0F))
}

/// Smoothie #6.
#[doc(alias = "showHeadlight")]
pub fn show_headlight(g: &mut Game, dst: ScreenId, src: ScreenId, player_x: i16, player_y: i16) {
    let (src, dst) = g.video.screen_pair_mut(src, dst);
    assert_vga_surface(src);
    assert_vga_surface(dst);

    let src_pitch = src.pitch as usize;
    let dst_pitch = dst.pitch as usize;
    let sp = &src.pixels;
    let dp = &mut dst.pixels;

    let player_x = i32::from(player_x);
    let player_y = i32::from(player_y);
    let mut lx = (player_x - 17) - (player_y + 12) + 1; // light starting at X
    let mut rx = (player_x - 17) + (player_y + 12); // light until X

    let mut di = 0usize;
    let mut si = 24usize;
    let swi = SW as i32;

    for _ in 0..184 {
        let mut x = 264i32;

        if lx <= rx + 1 {
            let x1 = 264 - 0.max(lx - 5);
            let x2 = 264 - 0.max(lx);
            let x3 = 264 - rx.min(264);
            let x4 = 264 - (rx + 5).min(264);
            debug_assert!(x1 >= x2);
            debug_assert!(x2 >= 0 && x2 >= x3 - 1);
            debug_assert!(x3 >= x4);
            debug_assert!(x4 >= 0);

            lx += 1;
            rx -= 1;

            // Dark left side. Round up to multiple (C computes this in
            // size_t, which floors negative values the same way).
            let x1r = (x1 + swi - 1) - (x1 + swi - 1).rem_euclid(swi);
            while x > x1r {
                swar8_write(&mut dp[di..], dark_lanes(swar8_read(&sp[si..])));

                di += SW;
                si += SW;
                x -= swi;
            }
            while x > x1 {
                let pixel = sp[si];
                dp[di] = (pixel & 0xF0) | ((pixel & 0x0F) >> 2);

                di += 1;
                si += 1;
                x -= 1;
            }

            // Gradient left side.
            while x > x2 {
                let light = 3 * (6 - (x - x2));

                let pixel = sp[si];
                let value = ((i32::from(pixel & 0xF) + light) >> 2) as u8;
                dp[di] = (pixel & 0xF0) | value;

                di += 1;
                si += 1;
                x -= 1;
            }

            // Light.
            if x2 > x3 {
                let w = (x2 - x3) as usize;

                dp[di..di + w].copy_from_slice(&sp[si..si + w]);

                di += w;
                si += w;

                x -= w as i32;
            }

            // Gradient right side.
            while x > x4 {
                let light = 3 * (x - x4);

                let pixel = sp[si];
                let value = ((i32::from(pixel & 0xF) + light) >> 2) as u8;
                dp[di] = (pixel & 0xF0) | value;

                di += 1;
                si += 1;
                x -= 1;
            }

            // Dark right side. Round down to multiple.
            let x4r = x4 - x4.rem_euclid(swi);
            while x > x4r {
                let pixel = sp[si];
                dp[di] = (pixel & 0xF0) | ((pixel & 0x0F) >> 2);

                di += 1;
                si += 1;
                x -= 1;
            }
        }
        while x > 0 {
            swar8_write(&mut dp[di..], dark_lanes(swar8_read(&sp[si..])));

            di += SW;
            si += SW;
            x -= swi;
        }

        debug_assert_eq!(di % dst_pitch, 264);
        debug_assert_eq!(si % src_pitch, 264 + 24);

        di += dst_pitch - 264;
        si += src_pitch - 264;
    }
}

/// Smoothie #9.
#[doc(alias = "showFlipped")]
pub fn show_flipped(g: &mut Game, dst: ScreenId, src: ScreenId) {
    let (src, dst) = g.video.screen_pair_mut(src, dst);
    assert_vga_surface(src);
    assert_vga_surface(dst);

    let src_pitch = src.pitch as usize;
    let dst_pitch = dst.pitch as usize;

    let mut di = dst_pitch * 184;
    let mut si = 24;

    for _ in 0..184 {
        di -= dst_pitch;

        dst.pixels[di..di + 264].copy_from_slice(&src.pixels[si..si + 264]);

        si += src_pitch;
    }
}

/// Not smoothie #6 xor #9.
#[doc(alias = "showNormal")]
pub fn show_normal(g: &mut Game, dst: ScreenId, src: ScreenId) {
    let (src, dst) = g.video.screen_pair_mut(src, dst);
    assert_vga_surface(src);
    assert_vga_surface(dst);

    let src_pitch = src.pitch as usize;
    let dst_pitch = dst.pitch as usize;

    let mut di = 0;
    let mut si = 24;

    for _ in 0..184 {
        dst.pixels[di..di + 264].copy_from_slice(&src.pixels[si..si + 264]);

        di += dst_pitch;
        si += src_pitch;
    }
}

pub fn initialize_starfield(g: &mut Game) {
    let pitch = g.video.vga().pitch as u64;
    for i in (0..MAX_STARS).rev() {
        let x = mt_rand(g) % 320;
        let y = mt_rand(g) % 200;
        g.backgrnd.starfield_stars[i].position = x.wrapping_add(y.wrapping_mul(pitch)) as u16;
        g.backgrnd.starfield_stars[i].speed = (mt_rand(g) % 3 + 2) as i32;
        g.backgrnd.starfield_stars[i].color = (mt_rand(g) % 16 + u64::from(STARFIELD_HUE)) as u8;
    }
}

pub fn update_and_draw_starfield(g: &mut Game, surface: ScreenId, move_speed: i32) {
    let surface = g.video.screen_mut(surface);
    let pitch = surface.pitch;
    let p = &mut surface.pixels;

    for star in g.backgrnd.starfield_stars.iter_mut().rev() {
        star.position = (i32::from(star.position)
            .wrapping_add(star.speed.wrapping_add(move_speed).wrapping_mul(pitch)))
            as u16;

        let pos = usize::from(star.position);
        if i32::from(star.position) < 177 * pitch {
            if p[pos] == 0 {
                p[pos] = star.color;
            }

            // If star is bright enough, draw surrounding pixels
            if i32::from(star.color) - 4 >= i32::from(STARFIELD_HUE) {
                let c = star.color - 4;
                let pitch = pitch as usize;
                if p[pos + 1] == 0 {
                    p[pos + 1] = c;
                }

                if pos > 0 && p[pos - 1] == 0 {
                    p[pos - 1] = c;
                }

                if p[pos + pitch] == 0 {
                    p[pos + pitch] = c;
                }

                if pos >= pitch && p[pos - pitch] == 0 {
                    p[pos - pitch] = c;
                }
            }
        }
    }
}
