//! Sprite blitting: every `blit_sprite*` over every sprite of the `tyrian.shp`
//! tables, and every `blit_sprite2*` over every sprite of its five sprite
//! sheets. One iteration blits each sprite once.

mod common;

use criterion::{Criterion, criterion_group, criterion_main};
use rustyrian_core::Game;
use rustyrian_core::sprite::{self as s, SPRITES_PER_TABLE_MAX};
use rustyrian_core::varz::Sprite2Ref;
use rustyrian_core::video::ScreenId;
use std::hint::black_box;

const SCREEN: ScreenId = ScreenId::VgaScreen2;

/// Where sprite `k` goes: spread over the screen, partly over the bottom edge.
fn pos(k: u32) -> (i32, i32) {
    ((k * 37 % 300) as i32, (k * 23 % 210) as i32 - 5)
}

fn loaded() -> Box<Game> {
    let mut g = common::game_with_screens();
    s::je_load_main_shape_tables(&mut g, "tyrian.shp").expect("data/tyrian.shp loads");
    g
}

/// `(table, sprite count)` of every loaded `blit_sprite` table.
fn tables(g: &Game) -> Vec<(u32, u32)> {
    g.sprite
        .sprite_table
        .iter()
        .enumerate()
        .filter(|(_, t)| t.count > 0)
        .map(|(i, t)| (i as u32, t.count.min(SPRITES_PER_TABLE_MAX) as u32))
        .collect()
}

/// Entries in a sheet's offset table (its first offset / 2).
fn sheet_entries(g: &Game, r: Sprite2Ref) -> u32 {
    let data = s::sprite2s(&g.sprite, &g.editship, r)
        .data
        .as_deref()
        .expect("sheet loaded");
    u32::from(u16::from_le_bytes([data[0], data[1]])) / 2
}

fn bench_blit_sprite(c: &mut Criterion) {
    let mut g = loaded();
    let tables = tables(&g);
    type Blit = fn(&mut Game, u32, u32, u32);
    let funcs: [(&str, Blit); 6] = [
        ("blit_sprite", |g, t, i, k| {
            let (x, y) = pos(k);
            s::blit_sprite(g, SCREEN, x, y, t, i)
        }),
        ("blit_sprite_blend", |g, t, i, k| {
            let (x, y) = pos(k);
            s::blit_sprite_blend(g, SCREEN, x, y, t, i)
        }),
        ("blit_sprite_hv_unsafe", |g, t, i, k| {
            let (x, y) = pos(k);
            s::blit_sprite_hv_unsafe(g, SCREEN, x, y, t, i, (k % 16) as u8, 3)
        }),
        ("blit_sprite_hv", |g, t, i, k| {
            let (x, y) = pos(k);
            s::blit_sprite_hv(g, SCREEN, x, y, t, i, (k % 16) as u8, -3)
        }),
        ("blit_sprite_hv_blend", |g, t, i, k| {
            let (x, y) = pos(k);
            s::blit_sprite_hv_blend(g, SCREEN, x, y, t, i, (k % 16) as u8, 2)
        }),
        ("blit_sprite_dark", |g, t, i, k| {
            let (x, y) = pos(k);
            s::blit_sprite_dark(g, SCREEN, x, y, t, i, k % 3 == 0)
        }),
    ];
    let mut group = c.benchmark_group("blit_sprite/tyrian.shp");
    for (name, f) in funcs {
        group.bench_function(name, |b| {
            b.iter(|| {
                let mut k = 0;
                for &(t, n) in &tables {
                    for i in 0..n {
                        f(&mut g, black_box(t), i, k);
                        k += 1;
                    }
                }
            })
        });
    }
    group.finish();
    black_box(&g.video.screens);
}

fn bench_blit_sprite2(c: &mut Criterion) {
    let mut g = loaded();
    let sheets = [
        Sprite2Ref::SpriteSheet8,
        Sprite2Ref::SpriteSheet9,
        Sprite2Ref::SpriteSheet10,
        Sprite2Ref::SpriteSheet11,
        Sprite2Ref::SpriteSheet12,
    ]
    .map(|r| (r, sheet_entries(&g, r)));
    type Blit2 = fn(&mut Game, Sprite2Ref, u32, u32);
    let funcs: [(&str, bool, Blit2); 12] = [
        ("blit_sprite2", false, |g, r, i, k| {
            let (x, y) = pos(k);
            s::blit_sprite2(g, SCREEN, x, y, r, i)
        }),
        ("blit_sprite2_clip", false, |g, r, i, k| {
            let (x, y) = pos(k);
            s::blit_sprite2_clip(g, SCREEN, x, y, r, i)
        }),
        ("blit_sprite2_blend", false, |g, r, i, k| {
            let (x, y) = pos(k);
            s::blit_sprite2_blend(g, SCREEN, x, y, r, i)
        }),
        ("blit_sprite2_darken", false, |g, r, i, k| {
            let (x, y) = pos(k);
            s::blit_sprite2_darken(g, SCREEN, x, y, r, i)
        }),
        ("blit_sprite2_filter", false, |g, r, i, k| {
            let (x, y) = pos(k);
            s::blit_sprite2_filter(g, SCREEN, x, y, r, i, ((k % 16) as u8) << 4)
        }),
        ("blit_sprite2_filter_clip", false, |g, r, i, k| {
            let (x, y) = pos(k);
            s::blit_sprite2_filter_clip(g, SCREEN, x, y, r, i, ((k % 16) as u8) << 4)
        }),
        ("blit_sprite2x2", true, |g, r, i, k| {
            let (x, y) = pos(k);
            s::blit_sprite2x2(g, SCREEN, x, y, r, i)
        }),
        ("blit_sprite2x2_clip", true, |g, r, i, k| {
            let (x, y) = pos(k);
            s::blit_sprite2x2_clip(g, SCREEN, x, y, r, i)
        }),
        ("blit_sprite2x2_blend", true, |g, r, i, k| {
            let (x, y) = pos(k);
            s::blit_sprite2x2_blend(g, SCREEN, x, y, r, i)
        }),
        ("blit_sprite2x2_darken", true, |g, r, i, k| {
            let (x, y) = pos(k);
            s::blit_sprite2x2_darken(g, SCREEN, x, y, r, i)
        }),
        ("blit_sprite2x2_filter", true, |g, r, i, k| {
            let (x, y) = pos(k);
            s::blit_sprite2x2_filter(g, SCREEN, x, y, r, i, ((k % 16) as u8) << 4)
        }),
        ("blit_sprite2x2_filter_clip", true, |g, r, i, k| {
            let (x, y) = pos(k);
            s::blit_sprite2x2_filter_clip(g, SCREEN, x, y, r, i, ((k % 16) as u8) << 4)
        }),
    ];
    let mut group = c.benchmark_group("blit_sprite2/tyrian.shp");
    for (name, x2, f) in funcs {
        group.bench_function(name, |b| {
            b.iter(|| {
                let mut k = 0;
                for &(r, n) in &sheets {
                    // The 2×2 variants also draw index + 1, + 19 and + 20.
                    let n = if x2 { n.saturating_sub(20) } else { n };
                    for i in 1..=n {
                        f(&mut g, black_box(r), i, k);
                        k += 1;
                    }
                }
            })
        });
    }
    group.finish();
    black_box(&g.video.screens);
}

criterion_group!(benches, bench_blit_sprite, bench_blit_sprite2);
criterion_main!(benches);
