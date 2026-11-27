//! `tyrian.shp`: `JE_loadMainShapeTables` (sprite tables 0-6, sheets 8-12),
//! then every loaded sprite drawn with each `blit_sprite*` and the first
//! sprites of each sheet with `blit_sprite2`. The same bytes as `estsc.shp`
//! go through `load_sprites_file` into the extra-shapes table.
#![no_main]

use libfuzzer_sys::fuzz_target;
use rustyrian_core::sprite::{self as s, EXTRA_SHAPES};
use rustyrian_core::varz::Sprite2Ref;
use rustyrian_core::video::ScreenId;

/// Corners and edges where clipping and wrap-around matter.
const AT: [(i32, i32); 4] = [(0, 0), (300, 190), (-20, -20), (319, 199)];

fn blit_tables(g: &mut rustyrian_core::Game, tables: std::ops::Range<u32>) {
    let v = ScreenId::VgaScreenSeg;
    for table in tables {
        let count = g.sprite.sprite_table[table as usize].count as u32;
        // One past the end too: the blitters check the count.
        for index in 0..=count {
            for (x, y) in AT {
                s::blit_sprite(g, v, x, y, table, index);
                s::blit_sprite_blend(g, v, x, y, table, index);
                s::blit_sprite_hv_unsafe(g, v, x, y, table, index, 0x0f, 7);
                s::blit_sprite_hv(g, v, x, y, table, index, 3, -9);
                s::blit_sprite_hv_blend(g, v, x, y, table, index, 3, 9);
                s::blit_sprite_dark(g, v, x, y, table, index, index % 2 == 0);
            }
        }
    }
}

fuzz_target!(|data: &[u8]| {
    let mut g = rustyrian_fuzz::game_with_data_files(&[("tyrian.shp", data), ("estsc.shp", data)]);
    rustyrian_fuzz::with_screens(&mut g);

    if s::je_load_main_shape_tables(&mut g, "tyrian.shp").is_ok() {
        blit_tables(&mut g, 0..7);
        let sheets = [
            Sprite2Ref::SpriteSheet8,
            Sprite2Ref::SpriteSheet9,
            Sprite2Ref::SpriteSheet10,
            Sprite2Ref::SpriteSheet11,
            Sprite2Ref::SpriteSheet12,
        ];
        for sheet in sheets {
            for index in 0..64 {
                s::blit_sprite2(&mut g, ScreenId::VgaScreenSeg, 100, 100, sheet, index);
            }
        }
    }

    if s::load_sprites_file(&mut g, EXTRA_SHAPES as u32, "estsc.shp").is_ok() {
        blit_tables(&mut g, EXTRA_SHAPES as u32..EXTRA_SHAPES as u32 + 1);
    }
});
