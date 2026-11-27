// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `sprite.c` / `sprite.h`.
//!
//! `SDL_Surface *` parameters are [`ScreenId`]s; `Sprite2_array` arguments
//! (always one of the global sheets) are [`Sprite2Ref`]s (rule R8).
//!
//! Shape data is untrusted (UF-3, divergence `D-UB-SPRITE-1`): sheets are
//! validated where they are loaded (decreasing sheet positions in the main
//! shape file, sprite streams running past the end of a sheet), and every
//! blit read is bounds-checked, stopping where C would read outside the data
//! block (an index outside a sheet's offset table). Where C stays inside the
//! block the result is identical.

use crate::file::{self, File};
use crate::game::{EXIT_FAILURE, Flow, Game, Halt};
use crate::logging::log_fatal;
use crate::sdl::Surface;
use crate::varz::Sprite2Ref;
use crate::video::ScreenId;
use serde::{Deserialize, Serialize};

pub const FONT_SHAPES: usize = 0;
pub const SMALL_FONT_SHAPES: usize = 1;
pub const TINY_FONT: usize = 2;
pub const PLANET_SHAPES: usize = 3;
pub const FACE_SHAPES: usize = 4;
/// Also contains help shapes.
pub const OPTION_SHAPES: usize = 5;
pub const WEAPON_SHAPES: usize = 6;
/// Used for ending pics.
pub const EXTRA_SHAPES: usize = 7;

pub const SPRITE_TABLES_MAX: usize = 8;
pub const SPRITES_PER_TABLE_MAX: usize = 151;

/// `Sprite`. `data` is the heap block upstream mallocs with `size` bytes
/// (`None` is NULL).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sprite {
    pub width: u16,
    pub height: u16,
    pub size: u16,
    pub data: Option<Vec<u8>>,
}

/// `Sprite_array`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpriteArray {
    pub count: usize,
    #[serde(with = "crate::serde_big")]
    pub sprite: [Sprite; SPRITES_PER_TABLE_MAX],
}

impl Default for SpriteArray {
    fn default() -> Self {
        SpriteArray {
            count: 0,
            sprite: std::array::from_fn(|_| Sprite::default()),
        }
    }
}

/// `Sprite2_array`. `data` is the heap block upstream mallocs with `size`
/// bytes (`None` is NULL).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sprite2Array {
    pub size: usize,
    pub data: Option<Vec<u8>>,
}

/// Mutable globals of this module (rule R3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// FKA shapearray, shapex, shapey, shapesize, shapexist, maxshape.
    pub sprite_table: Box<[SpriteArray; SPRITE_TABLES_MAX]>,
    /// Shop icons and arrows sprite sheet (FKA shapes6).
    pub shop_sprite_sheet: Sprite2Array,
    /// Explosions sprite sheet (FKA shapes6).
    pub explosion_sprite_sheet: Sprite2Array,
    /// Enemy sprite sheet banks (FKA eShapes1..4).
    pub enemy_sprite_sheets: [Sprite2Array; 4],
    /// FKA enemyShapeTables.
    pub enemy_sprite_sheet_ids: [u8; 4],
    /// Destruct sprite sheet (FKA shapes6).
    pub destruct_sprite_sheet: Sprite2Array,
    /// FKA shapesC1.
    pub sprite_sheet8: Sprite2Array,
    /// FKA shapes9.
    pub sprite_sheet9: Sprite2Array,
    /// FKA eShapes6.
    pub sprite_sheet10: Sprite2Array,
    /// FKA eShapes5.
    pub sprite_sheet11: Sprite2Array,
    /// FKA shapesW2.
    pub sprite_sheet12: Sprite2Array,
}

impl Default for State {
    fn default() -> Self {
        State {
            sprite_table: vec![SpriteArray::default(); SPRITE_TABLES_MAX]
                .into_boxed_slice()
                .try_into()
                .unwrap_or_else(|_| unreachable!("SPRITE_TABLES_MAX entries")),
            shop_sprite_sheet: Sprite2Array::default(),
            explosion_sprite_sheet: Sprite2Array::default(),
            enemy_sprite_sheets: Default::default(),
            enemy_sprite_sheet_ids: [0; 4],
            destruct_sprite_sheet: Sprite2Array::default(),
            sprite_sheet8: Sprite2Array::default(),
            sprite_sheet9: Sprite2Array::default(),
            sprite_sheet10: Sprite2Array::default(),
            sprite_sheet11: Sprite2Array::default(),
            sprite_sheet12: Sprite2Array::default(),
        }
    }
}

/// `sprite(table, index)`.
pub fn sprite(g: &Game, table: u32, index: u32) -> &Sprite {
    &g.sprite.sprite_table[table as usize].sprite[index as usize]
}

pub fn sprite_exists(g: &Game, table: u32, index: u32) -> bool {
    sprite(g, table, index).data.is_some()
}

pub fn get_sprite_width(g: &Game, table: u32, index: u32) -> u16 {
    if sprite_exists(g, table, index) {
        sprite(g, table, index).width
    } else {
        0
    }
}

pub fn get_sprite_height(g: &Game, table: u32, index: u32) -> u16 {
    if sprite_exists(g, table, index) {
        sprite(g, table, index).height
    } else {
        0
    }
}

/// Whether every sprite of `sprite2s` can be drawn without reading outside
/// its data block: the offset table (first offset / 2 entries) and each
/// stream it points to, through the stream's `0x0f` terminator, lie inside
/// the data. A block shorter than one entry holds no sprites. This is the R10
/// load-boundary check of `D-UB-SPRITE-1`; every sheet in the freeware data
/// passes it.
pub fn sprite2s_valid(sprite2s: &Sprite2Array) -> bool {
    let Some(data) = sprite2s.data.as_deref() else {
        return true;
    };
    if data.len() < 2 {
        return true;
    }
    let entries = usize::from(u16::from_le_bytes([data[0], data[1]])) / 2;
    if entries * 2 > data.len() {
        return false;
    }
    data[..entries * 2].as_chunks::<2>().0.iter().all(|&off| {
        let mut i = usize::from(u16::from_le_bytes(off));
        loop {
            let Some(&d) = data.get(i) else { return false };
            if d == 0x0f {
                return true;
            }
            // the command byte plus its opaque pixels
            i += usize::from(d >> 4) + 1;
        }
    })
}

/// Logs and exits for a sheet that fails [`sprite2s_valid`] (`D-UB-SPRITE-1`).
fn invalid_sheet(g: &mut Game, filename: &str) -> Flow {
    let message = format!("Failed to read from file '{filename}': Invalid sprite data");
    log_fatal(g, &message);
    Err(Halt { code: EXIT_FAILURE })
}

pub fn load_sprites_file(g: &mut Game, table: u32, filename: &str) -> Flow {
    free_sprites(g, table);

    let mut file = file::data_file_open(g, filename, "rb");
    if file.error {
        let message = format!(
            "Failed to open file '{}': {}",
            filename,
            file::file_get_error(&file)
        );
        log_fatal(g, &message);
        return Err(Halt { code: EXIT_FAILURE });
    }

    load_sprites(g, table, &mut file);

    if file.error {
        let message = format!(
            "Failed to read from file '{}': {}",
            filename,
            file::file_get_error(&file)
        );
        log_fatal(g, &message);
        return Err(Halt { code: EXIT_FAILURE });
    }

    file::file_close(&mut file);
    Ok(())
}

pub fn load_sprites(g: &mut Game, table: u32, file: &mut File) {
    free_sprites(g, table);

    let count = file::file_read_u16(file).min(SPRITES_PER_TABLE_MAX as u16);

    let t = &mut g.sprite.sprite_table[table as usize];
    t.count = usize::from(count);

    for sprite in t.sprite.iter_mut().take(t.count) {
        let populated = file::file_read_bool(file);
        if !populated {
            *sprite = Sprite::default();
        } else {
            sprite.width = file::file_read_u16(file);
            sprite.height = file::file_read_u16(file);
            sprite.size = file::file_read_u16(file);
            let mut data = vec![0; usize::from(sprite.size)];
            file::file_read_exactly(file, &mut data);
            sprite.data = Some(data);
        }
    }
}

pub fn free_sprites(g: &mut Game, table: u32) {
    let t = &mut g.sprite.sprite_table[table as usize];
    for sprite in t.sprite.iter_mut().take(t.count) {
        *sprite = Sprite::default();
    }

    t.count = 0;
}

/// The byte offset of `surface->pixels + y * pitch + x` (C `int` math).
fn pixel_offset(s: &Surface, x: i32, y: i32) -> i64 {
    i64::from(y.wrapping_mul(s.pitch).wrapping_add(x))
}

/// The shared body of the `blit_sprite*` functions: walks the sprite's
/// run-length data and sets every opaque pixel inside the surface to
/// `op(sprite pixel, destination pixel)`. No clipping at the left or right
/// edge.
fn blit_sprite_with(
    g: &mut Game,
    surface: ScreenId,
    x: i32,
    y: i32,
    table: u32,
    index: u32,
    op: impl Fn(u8, u8) -> u8,
) {
    let t = &g.sprite.sprite_table[table as usize];
    if index as usize >= t.count {
        return;
    }
    let cur_sprite = &t.sprite[index as usize];
    let Some(data) = cur_sprite.data.as_deref() else {
        return;
    };
    let data_ul = usize::from(cur_sprite.size);

    let width = u32::from(cur_sprite.width);
    let mut x_offset: u32 = 0;

    let s = g.video.screen_mut(surface);
    let mut pixels = pixel_offset(s, x, y);
    let pixels_ul = i64::from(s.h.wrapping_mul(s.pitch));

    let mut i = 0;
    while i < data_ul {
        let Some(&d) = data.get(i) else { return };
        match d {
            255 => {
                // a run of transparent pixels; the next byte is its length
                i += 1;
                let Some(&n) = data.get(i) else { return };
                pixels += i64::from(n);
                x_offset = x_offset.wrapping_add(u32::from(n));
            }
            254 => {
                // end of row
                pixels += i64::from(width.wrapping_sub(x_offset));
                x_offset = width;
            }
            253 => {
                // a single transparent pixel
                pixels += 1;
                x_offset = x_offset.wrapping_add(1);
            }
            _ => {
                if pixels >= pixels_ul {
                    return;
                }
                if pixels >= 0 {
                    let p = &mut s.pixels[pixels as usize];
                    *p = op(d, *p);
                }
                pixels += 1;
                x_offset = x_offset.wrapping_add(1);
            }
        }
        if x_offset >= width {
            pixels += i64::from((s.pitch as u32).wrapping_sub(x_offset));
            x_offset = 0;
        }
        i += 1;
    }
}

/// No clipping at the left or right edge.
pub fn blit_sprite(g: &mut Game, surface: ScreenId, x: i32, y: i32, table: u32, index: u32) {
    blit_sprite_with(g, surface, x, y, table, index, |d, _| d);
}

/// No clipping at the left or right edge.
pub fn blit_sprite_blend(g: &mut Game, surface: ScreenId, x: i32, y: i32, table: u32, index: u32) {
    blit_sprite_with(g, surface, x, y, table, index, |d, p| {
        (d & 0xf0) | (((p & 0x0f) + (d & 0x0f)) / 2)
    });
}

/// No clipping at the left or right edge; `value` may carry into the hue.
#[allow(clippy::too_many_arguments)]
pub fn blit_sprite_hv_unsafe(
    g: &mut Game,
    surface: ScreenId,
    x: i32,
    y: i32,
    table: u32,
    index: u32,
    hue: u8,
    value: i8,
) {
    let hue = hue.wrapping_shl(4);
    blit_sprite_with(g, surface, x, y, table, index, |d, _| {
        (i32::from(hue) | (i32::from(d & 0x0f) + i32::from(value))) as u8
    });
}

/// The sprite pixel's value plus `value`, limited to 0..=15 as upstream
/// limits it.
fn hv_value(d: u8, value: i8) -> u8 {
    let temp_value = (i32::from(d & 0x0f) + i32::from(value)) as u8;
    if temp_value > 0xf {
        if temp_value >= 0x1f { 0x0 } else { 0xf }
    } else {
        temp_value
    }
}

/// No clipping at the left or right edge.
#[allow(clippy::too_many_arguments)]
pub fn blit_sprite_hv(
    g: &mut Game,
    surface: ScreenId,
    x: i32,
    y: i32,
    table: u32,
    index: u32,
    hue: u8,
    value: i8,
) {
    let hue = hue.wrapping_shl(4);
    blit_sprite_with(g, surface, x, y, table, index, |d, _| {
        hue | hv_value(d, value)
    });
}

/// No clipping at the left or right edge.
#[allow(clippy::too_many_arguments)]
pub fn blit_sprite_hv_blend(
    g: &mut Game,
    surface: ScreenId,
    x: i32,
    y: i32,
    table: u32,
    index: u32,
    hue: u8,
    value: i8,
) {
    let hue = hue.wrapping_shl(4);
    blit_sprite_with(g, surface, x, y, table, index, |d, p| {
        let temp_value = hv_value(d, value);
        (i32::from(hue) | ((i32::from(p & 0x0f) + i32::from(temp_value)) / 2)) as u8
    });
}

/// No clipping at the left or right edge.
pub fn blit_sprite_dark(
    g: &mut Game,
    surface: ScreenId,
    x: i32,
    y: i32,
    table: u32,
    index: u32,
    black: bool,
) {
    blit_sprite_with(g, surface, x, y, table, index, |_, p| {
        if black {
            0x00
        } else {
            (p & 0xf0) | ((p & 0x0f) / 2)
        }
    });
}

/// The sheet a [`Sprite2Ref`] names.
pub fn sprite2s<'a>(
    sprite: &'a State,
    editship: &'a crate::editship::State,
    r: Sprite2Ref,
) -> &'a Sprite2Array {
    match r {
        Sprite2Ref::ShopSpriteSheet => &sprite.shop_sprite_sheet,
        Sprite2Ref::ExplosionSpriteSheet => &sprite.explosion_sprite_sheet,
        Sprite2Ref::EnemySpriteSheets(i) => &sprite.enemy_sprite_sheets[usize::from(i)],
        Sprite2Ref::DestructSpriteSheet => &sprite.destruct_sprite_sheet,
        Sprite2Ref::SpriteSheet8 => &sprite.sprite_sheet8,
        Sprite2Ref::SpriteSheet9 => &sprite.sprite_sheet9,
        Sprite2Ref::SpriteSheet10 => &sprite.sprite_sheet10,
        Sprite2Ref::SpriteSheet11 => &sprite.sprite_sheet11,
        Sprite2Ref::SpriteSheet12 => &sprite.sprite_sheet12,
        Sprite2Ref::ExtraShapes => &editship.extra_shapes,
    }
}

/// Mutable [`sprite2s`].
pub fn sprite2s_mut(g: &mut Game, r: Sprite2Ref) -> &mut Sprite2Array {
    let sprite = &mut g.sprite;
    match r {
        Sprite2Ref::ShopSpriteSheet => &mut sprite.shop_sprite_sheet,
        Sprite2Ref::ExplosionSpriteSheet => &mut sprite.explosion_sprite_sheet,
        Sprite2Ref::EnemySpriteSheets(i) => &mut sprite.enemy_sprite_sheets[usize::from(i)],
        Sprite2Ref::DestructSpriteSheet => &mut sprite.destruct_sprite_sheet,
        Sprite2Ref::SpriteSheet8 => &mut sprite.sprite_sheet8,
        Sprite2Ref::SpriteSheet9 => &mut sprite.sprite_sheet9,
        Sprite2Ref::SpriteSheet10 => &mut sprite.sprite_sheet10,
        Sprite2Ref::SpriteSheet11 => &mut sprite.sprite_sheet11,
        Sprite2Ref::SpriteSheet12 => &mut sprite.sprite_sheet12,
        Sprite2Ref::ExtraShapes => &mut g.editship.extra_shapes,
    }
}

#[doc(alias = "JE_loadCompShapes")]
pub fn je_load_comp_shapes(g: &mut Game, sprite2s: Sprite2Ref, s: u8) -> Flow {
    free_sprite2s(sprite2s_mut(g, sprite2s));

    let filename = format!("newsh{}.shp", char::from(s.to_ascii_lowercase()));

    let mut file = file::data_file_open(g, &filename, "rb");
    if file.error {
        let message = format!(
            "Failed to open file '{}': {}",
            filename,
            file::file_get_error(&file)
        );
        log_fatal(g, &message);
        return Err(Halt { code: EXIT_FAILURE });
    }

    let size = file::file_get_length(&mut file) as u64 as usize;
    let sheet = sprite2s_mut(g, sprite2s);
    sheet.size = size;

    je_load_comp_shapes_b(sheet, &mut file);

    if file.error {
        let message = format!(
            "Failed to read from file '{}': {}",
            filename,
            file::file_get_error(&file)
        );
        log_fatal(g, &message);
        return Err(Halt { code: EXIT_FAILURE });
    }

    if !sprite2s_valid(sprite2s_mut(g, sprite2s)) {
        return invalid_sheet(g, &filename);
    }

    file::file_close(&mut file);
    Ok(())
}

/// Reads `sprite2s.size` bytes into a new block. Upstream asserts the sheet
/// holds no block (and would leak it); here it is replaced. Callers bound
/// `size` by the file length.
#[doc(alias = "JE_loadCompShapesB")]
pub fn je_load_comp_shapes_b(sprite2s: &mut Sprite2Array, file: &mut File) {
    let mut data = vec![0; sprite2s.size];
    file::file_read_exactly(file, &mut data);
    sprite2s.data = Some(data);
}

pub fn free_sprite2s(sprite2s: &mut Sprite2Array) {
    sprite2s.data = None;

    sprite2s.size = 0;
}

/// The data block of `sheet` and the start of sprite `index`'s stream in it
/// (the little-endian `u16` at entry `index - 1` of the offset table); `None`
/// where C would read outside the block.
fn sprite2_start(sheet: &Sprite2Array, index: u32) -> Option<(&[u8], usize)> {
    let data = sheet.data.as_deref()?;
    let t = (index.wrapping_sub(1) as usize).checked_mul(2)?;
    let off = data.get(t..t.checked_add(2)?)?;
    Some((data, usize::from(u16::from_le_bytes([off[0], off[1]]))))
}

/// The shared body of the unclipped `blit_sprite2*` functions: sets every
/// opaque pixel inside the surface to `op(sprite pixel, destination pixel)`.
/// `reads_data` is false for an `op` that ignores the sprite pixel (C does
/// not read it then). No clipping at the left or right edge.
#[allow(clippy::too_many_arguments)]
fn blit_sprite2_with(
    g: &mut Game,
    surface: ScreenId,
    x: i32,
    y: i32,
    sprite2s: Sprite2Ref,
    index: u32,
    reads_data: bool,
    op: impl Fn(u8, u8) -> u8,
) {
    let vga_pitch = g.video.vga().pitch;
    let sheet = self::sprite2s(&g.sprite, &g.editship, sprite2s);
    let s = g.video.screen_mut(surface);
    let mut pixels = pixel_offset(s, x, y);
    let pixels_ul = i64::from(s.h.wrapping_mul(s.pitch));

    let Some((data, mut i)) = sprite2_start(sheet, index) else {
        return;
    };

    loop {
        let Some(&d) = data.get(i) else { return };
        if d == 0x0f {
            return;
        }
        pixels += i64::from(d & 0x0f); // low nibble: transparent pixels
        let mut count = (d & 0xf0) >> 4; // high nibble: opaque pixels

        if count == 0 {
            // end of row
            pixels += i64::from(vga_pitch.wrapping_sub(12));
        } else {
            while count > 0 {
                count -= 1;
                i += 1;

                if pixels >= pixels_ul {
                    return;
                }
                if pixels >= 0 {
                    let v = if reads_data {
                        let Some(&v) = data.get(i) else { return };
                        v
                    } else {
                        0
                    };
                    let p = &mut s.pixels[pixels as usize];
                    *p = op(v, *p);
                }

                pixels += 1;
            }
        }
        i += 1;
    }
}

/// The shared body of the `blit_sprite2*_clip` functions.
fn blit_sprite2_clip_with(
    g: &mut Game,
    surface: ScreenId,
    mut x: i32,
    mut y: i32,
    sprite2s: Sprite2Ref,
    index: u32,
    op: impl Fn(u8) -> u8,
) {
    let sheet = self::sprite2s(&g.sprite, &g.editship, sprite2s);
    let s = g.video.screen_mut(surface);

    let Some((data, mut i)) = sprite2_start(sheet, index) else {
        return;
    };

    loop {
        let Some(&d) = data.get(i) else { return };
        if d == 0x0f {
            return;
        }
        if y >= s.h {
            return;
        }

        let skip_count = d & 0x0f;
        let mut fill_count = (d >> 4) & 0x0f;

        x = x.wrapping_add(i32::from(skip_count));

        if fill_count == 0 {
            // end of row
            y = y.wrapping_add(1);
            x = x.wrapping_sub(12);
        } else if y >= 0 {
            let pixel_row = y.wrapping_mul(s.pitch);
            loop {
                i += 1;

                if x >= 0 && x < s.pitch {
                    let Some(&v) = data.get(i) else { return };
                    s.pixels[(pixel_row + x) as usize] = op(v);
                }
                x = x.wrapping_add(1);
                fill_count -= 1;
                if fill_count == 0 {
                    break;
                }
            }
        } else {
            i += usize::from(fill_count);
            x = x.wrapping_add(i32::from(fill_count));
        }
        i += 1;
    }
}

/// No clipping at the left or right edge.
pub fn blit_sprite2(
    g: &mut Game,
    surface: ScreenId,
    x: i32,
    y: i32,
    sprite2s: Sprite2Ref,
    index: u32,
) {
    blit_sprite2_with(g, surface, x, y, sprite2s, index, true, |d, _| d);
}

pub fn blit_sprite2_clip(
    g: &mut Game,
    surface: ScreenId,
    x: i32,
    y: i32,
    sprite2s: Sprite2Ref,
    index: u32,
) {
    blit_sprite2_clip_with(g, surface, x, y, sprite2s, index, |d| d);
}

/// No clipping at the left or right edge.
pub fn blit_sprite2_blend(
    g: &mut Game,
    surface: ScreenId,
    x: i32,
    y: i32,
    sprite2s: Sprite2Ref,
    index: u32,
) {
    blit_sprite2_with(g, surface, x, y, sprite2s, index, true, |d, p| {
        (((d & 0x0f) + (p & 0x0f)) / 2) | (d & 0xf0)
    });
}

/// No clipping at the left or right edge.
pub fn blit_sprite2_darken(
    g: &mut Game,
    surface: ScreenId,
    x: i32,
    y: i32,
    sprite2s: Sprite2Ref,
    index: u32,
) {
    blit_sprite2_with(g, surface, x, y, sprite2s, index, false, |_, p| {
        ((p & 0x0f) / 2) + (p & 0xf0)
    });
}

/// No clipping at the left or right edge.
pub fn blit_sprite2_filter(
    g: &mut Game,
    surface: ScreenId,
    x: i32,
    y: i32,
    sprite2s: Sprite2Ref,
    index: u32,
    filter: u8,
) {
    blit_sprite2_with(g, surface, x, y, sprite2s, index, true, |d, _| {
        filter | (d & 0x0f)
    });
}

pub fn blit_sprite2_filter_clip(
    g: &mut Game,
    surface: ScreenId,
    x: i32,
    y: i32,
    sprite2s: Sprite2Ref,
    index: u32,
    filter: u8,
) {
    blit_sprite2_clip_with(g, surface, x, y, sprite2s, index, |d| filter | (d & 0x0f));
}

/// The four quarters of a 2×2 sprite: offsets and index increments.
const QUARTERS: [(i32, i32, u32); 4] = [(0, 0, 0), (12, 0, 1), (0, 14, 19), (12, 14, 20)];

/// No clipping at the left or right edge.
pub fn blit_sprite2x2(
    g: &mut Game,
    surface: ScreenId,
    x: i32,
    y: i32,
    sprite2s: Sprite2Ref,
    index: u32,
) {
    for (dx, dy, di) in QUARTERS {
        let (x, y, index) = (
            x.wrapping_add(dx),
            y.wrapping_add(dy),
            index.wrapping_add(di),
        );
        blit_sprite2(g, surface, x, y, sprite2s, index);
    }
}

pub fn blit_sprite2x2_clip(
    g: &mut Game,
    surface: ScreenId,
    x: i32,
    y: i32,
    sprite2s: Sprite2Ref,
    index: u32,
) {
    for (dx, dy, di) in QUARTERS {
        let (x, y, index) = (
            x.wrapping_add(dx),
            y.wrapping_add(dy),
            index.wrapping_add(di),
        );
        blit_sprite2_clip(g, surface, x, y, sprite2s, index);
    }
}

/// No clipping at the left or right edge.
pub fn blit_sprite2x2_blend(
    g: &mut Game,
    surface: ScreenId,
    x: i32,
    y: i32,
    sprite2s: Sprite2Ref,
    index: u32,
) {
    for (dx, dy, di) in QUARTERS {
        let (x, y, index) = (
            x.wrapping_add(dx),
            y.wrapping_add(dy),
            index.wrapping_add(di),
        );
        blit_sprite2_blend(g, surface, x, y, sprite2s, index);
    }
}

/// No clipping at the left or right edge.
pub fn blit_sprite2x2_darken(
    g: &mut Game,
    surface: ScreenId,
    x: i32,
    y: i32,
    sprite2s: Sprite2Ref,
    index: u32,
) {
    for (dx, dy, di) in QUARTERS {
        let (x, y, index) = (
            x.wrapping_add(dx),
            y.wrapping_add(dy),
            index.wrapping_add(di),
        );
        blit_sprite2_darken(g, surface, x, y, sprite2s, index);
    }
}

/// No clipping at the left or right edge.
pub fn blit_sprite2x2_filter(
    g: &mut Game,
    surface: ScreenId,
    x: i32,
    y: i32,
    sprite2s: Sprite2Ref,
    index: u32,
    filter: u8,
) {
    for (dx, dy, di) in QUARTERS {
        let (x, y, index) = (
            x.wrapping_add(dx),
            y.wrapping_add(dy),
            index.wrapping_add(di),
        );
        blit_sprite2_filter(g, surface, x, y, sprite2s, index, filter);
    }
}

pub fn blit_sprite2x2_filter_clip(
    g: &mut Game,
    surface: ScreenId,
    x: i32,
    y: i32,
    sprite2s: Sprite2Ref,
    index: u32,
    filter: u8,
) {
    for (dx, dy, di) in QUARTERS {
        let (x, y, index) = (
            x.wrapping_add(dx),
            y.wrapping_add(dy),
            index.wrapping_add(di),
        );
        blit_sprite2_filter_clip(g, surface, x, y, sprite2s, index, filter);
    }
}

#[doc(alias = "JE_loadMainShapeTables")]
pub fn je_load_main_shape_tables(g: &mut Game, filename: &str) -> Flow {
    const SHP_NUM: usize = 12;

    let mut file = file::data_file_open(g, filename, "rb");
    if file.error {
        let message = format!(
            "Failed to open file '{}': {}",
            filename,
            file::file_get_error(&file)
        );
        log_fatal(g, &message);
        return Err(Halt { code: EXIT_FAILURE });
    }

    let mut positions = [0i64; SHP_NUM + 1];

    let count = usize::from(file::file_read_u16(&mut file)).min(SHP_NUM);

    for p in positions.iter_mut().take(count) {
        *p = i64::from(file::file_read_u32(&mut file));
    }

    let file_length = file::file_get_length(&mut file);
    for p in positions.iter_mut().skip(count) {
        *p = file_length;
    }

    // tables 0..7: fonts, interface and option sprites
    for (i, &position) in positions.iter().enumerate().take(7) {
        file::file_set_position(&mut file, position);

        load_sprites(g, i as u32, &mut file);
    }

    // Sheet sizes are differences of consecutive positions; upstream
    // allocates a negative size when they decrease (`D-UB-SPRITE-1`). The
    // last position is the file length, so increasing positions also bound
    // every sheet by the file. Decreasing ones fail as a read past the end.
    if positions[7..].is_sorted() {
        // player shots, player ships, power-ups, coins and datacubes, more
        // player shots
        let sheets = [
            &mut g.sprite.sprite_sheet8,
            &mut g.sprite.sprite_sheet9,
            &mut g.sprite.sprite_sheet10,
            &mut g.sprite.sprite_sheet11,
            &mut g.sprite.sprite_sheet12,
        ];
        for (i, sheet) in (7..).zip(sheets) {
            sheet.size = (positions[i + 1] - positions[i]) as usize;
            je_load_comp_shapes_b(sheet, &mut file);
        }
    } else if !file.error {
        file.error = true;
        file.errnum = file::ERRNUM_EOF;
    }

    if file.error {
        let message = format!(
            "Failed to read from file '{}': {}",
            filename,
            file::file_get_error(&file)
        );
        log_fatal(g, &message);
        return Err(Halt { code: EXIT_FAILURE });
    }

    let s = &g.sprite;
    let sheets = [
        &s.sprite_sheet8,
        &s.sprite_sheet9,
        &s.sprite_sheet10,
        &s.sprite_sheet11,
        &s.sprite_sheet12,
    ];
    if !sheets.into_iter().all(sprite2s_valid) {
        return invalid_sheet(g, filename);
    }

    file::file_close(&mut file);
    Ok(())
}

pub fn free_main_shape_tables(g: &mut Game) {
    for i in 0..SPRITE_TABLES_MAX as u32 {
        free_sprites(g, i);
    }

    free_sprite2s(&mut g.sprite.sprite_sheet8);
    free_sprite2s(&mut g.sprite.sprite_sheet9);
    free_sprite2s(&mut g.sprite.sprite_sheet10);
    free_sprite2s(&mut g.sprite.sprite_sheet11);
    free_sprite2s(&mut g.sprite.sprite_sheet12);
}
