//! A `newsh?.shp` sprite sheet through `JE_loadCompShapes` (with its
//! load-time sheet check), then arbitrary `blit_sprite2*` calls on it.
//!
//! Input: `n`, then `n` 7-byte calls (variant, x and y as little-endian
//! `i16`, index, filter), then the sheet. Every sprite of the sheet is also
//! drawn once with `blit_sprite2` and `blit_sprite2_clip`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use rustyrian_core::Game;
use rustyrian_core::sprite as s;
use rustyrian_core::varz::Sprite2Ref;
use rustyrian_core::video::ScreenId;

const SHEET: Sprite2Ref = Sprite2Ref::ShopSpriteSheet;
const CALL_SIZE: usize = 7;

fn blit(g: &mut Game, call: &[u8; CALL_SIZE]) {
    let v = ScreenId::VgaScreenSeg;
    let x = i32::from(rustyrian_fuzz::i16_at(call, 1));
    let y = i32::from(rustyrian_fuzz::i16_at(call, 3));
    let index = u32::from(call[5]);
    let filter = call[6];
    match call[0] % 12 {
        0 => s::blit_sprite2(g, v, x, y, SHEET, index),
        1 => s::blit_sprite2_clip(g, v, x, y, SHEET, index),
        2 => s::blit_sprite2_blend(g, v, x, y, SHEET, index),
        3 => s::blit_sprite2_darken(g, v, x, y, SHEET, index),
        4 => s::blit_sprite2_filter(g, v, x, y, SHEET, index, filter),
        5 => s::blit_sprite2_filter_clip(g, v, x, y, SHEET, index, filter),
        6 => s::blit_sprite2x2(g, v, x, y, SHEET, index),
        7 => s::blit_sprite2x2_clip(g, v, x, y, SHEET, index),
        8 => s::blit_sprite2x2_blend(g, v, x, y, SHEET, index),
        9 => s::blit_sprite2x2_darken(g, v, x, y, SHEET, index),
        10 => s::blit_sprite2x2_filter(g, v, x, y, SHEET, index, filter),
        _ => s::blit_sprite2x2_filter_clip(g, v, x, y, SHEET, index, filter),
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((&n, rest)) = data.split_first() else {
        return;
    };
    let calls_len = (usize::from(n) * CALL_SIZE).min(rest.len());
    let (calls, sheet) = rest.split_at(calls_len);

    let mut g = rustyrian_fuzz::game_with_data_file("newsh1.shp", sheet);
    rustyrian_fuzz::with_screens(&mut g);
    if s::je_load_comp_shapes(&mut g, SHEET, b'1').is_err() {
        return;
    }

    let entries = sheet
        .get(..2)
        .map_or(0, |b| u32::from(u16::from_le_bytes([b[0], b[1]])) / 2);
    for index in 0..=entries.min(1024) + 1 {
        s::blit_sprite2(&mut g, ScreenId::VgaScreenSeg, 150, 90, SHEET, index);
        s::blit_sprite2_clip(&mut g, ScreenId::VgaScreenSeg, -6, 190, SHEET, index);
    }
    for call in calls.as_chunks::<CALL_SIZE>().0 {
        blit(&mut g, call);
    }
});
