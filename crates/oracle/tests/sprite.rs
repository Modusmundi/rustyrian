//! Differential tests for `sprite.c` / `sprite.h`.

use proptest::prelude::*;
use rustyrian_core::Game;
use rustyrian_core::game::Halt;
use rustyrian_core::sdl::Surface;
use rustyrian_core::sprite::{self as s, Sprite, Sprite2Array, State};
use rustyrian_core::varz::Sprite2Ref;
use rustyrian_core::video::{self, ScreenId};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::video::screen_to_c;
use rustyrian_oracle::mirror::{Mirror, global, global_mut};
use std::ffi::CString;
use std::path::{Path, PathBuf};

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data")
        .canonicalize()
        .expect("data/ (run `cargo xtask fetch-data`)")
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir()
        .join(format!("rustyrian-sprite-{}", std::process::id()))
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("scratch dir");
    d
}

fn bytes(p: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    p.as_os_str().as_bytes().to_vec()
}

/// A surface filled with a deterministic pattern of every color.
fn pattern(salt: u32) -> Surface {
    let mut surf = Surface::new_8bit(320, 200);
    for (i, p) in surf.pixels.iter_mut().enumerate() {
        *p = ((i as u32).wrapping_mul(2_654_435_761).wrapping_add(salt) >> 13) as u8;
    }
    surf
}

/// Both engines with the three surfaces of `init_video` (patterned) and the
/// data directory `dir`.
fn setup(dir: &Path) -> Diff {
    let mut d = Diff::new();
    let v = &mut d.game.video;
    for (i, slot) in v.screens.iter_mut().enumerate() {
        *slot = Some(pattern(i as u32));
    }
    v.vga_screen = Some(ScreenId::VgaScreenSeg);
    v.vga_screen_seg = Some(ScreenId::VgaScreenSeg);
    v.vga_screen2 = Some(ScreenId::VgaScreen2);
    v.game_screen = Some(ScreenId::GameScreen);
    d.game.file.custom_data_dir_path = Some(bytes(dir));
    d.store_to_c();
    d
}

/// Copies the Rust surfaces into C (so both start equal), runs `c` and
/// `rust`, and compares the surfaces.
fn check(d: &mut Diff, label: &str, c: impl FnOnce(), rust: impl FnOnce(&mut Game)) {
    d.game.video.store_to_c();
    c();
    rust(&mut d.game);
    let cv = diff::load::<video::State>();
    assert!(
        cv.screens == d.game.video.screens,
        "{label}: surfaces differ"
    );
}

fn surf(id: ScreenId) -> *mut b::SDL_Surface {
    screen_to_c(Some(id))
}

/// The C sheet a `Sprite2Ref` names, by value as upstream passes it.
fn c_sheet(r: Sprite2Ref) -> b::Sprite2_array {
    // SAFETY: C runs only on this thread, inside calls we make.
    unsafe {
        match r {
            Sprite2Ref::ShopSpriteSheet => *global(&raw const b::shopSpriteSheet),
            Sprite2Ref::ExplosionSpriteSheet => *global(&raw const b::explosionSpriteSheet),
            Sprite2Ref::EnemySpriteSheets(i) => {
                global(&raw const b::enemySpriteSheets)[usize::from(i)]
            }
            Sprite2Ref::DestructSpriteSheet => *global(&raw const b::destructSpriteSheet),
            Sprite2Ref::SpriteSheet8 => *global(&raw const b::spriteSheet8),
            Sprite2Ref::SpriteSheet9 => *global(&raw const b::spriteSheet9),
            Sprite2Ref::SpriteSheet10 => *global(&raw const b::spriteSheet10),
            Sprite2Ref::SpriteSheet11 => *global(&raw const b::spriteSheet11),
            Sprite2Ref::SpriteSheet12 => *global(&raw const b::spriteSheet12),
            Sprite2Ref::ExtraShapes => *global(&raw const b::extraShapes),
        }
    }
}

fn c_sheet_mut(r: Sprite2Ref) -> *mut b::Sprite2_array {
    match r {
        Sprite2Ref::ShopSpriteSheet => &raw mut b::shopSpriteSheet,
        Sprite2Ref::ExplosionSpriteSheet => &raw mut b::explosionSpriteSheet,
        Sprite2Ref::EnemySpriteSheets(i) => {
            // SAFETY: C runs only on this thread, inside calls we make.
            unsafe { &raw mut global_mut(&raw mut b::enemySpriteSheets)[usize::from(i)] }
        }
        Sprite2Ref::DestructSpriteSheet => &raw mut b::destructSpriteSheet,
        Sprite2Ref::SpriteSheet8 => &raw mut b::spriteSheet8,
        Sprite2Ref::SpriteSheet9 => &raw mut b::spriteSheet9,
        Sprite2Ref::SpriteSheet10 => &raw mut b::spriteSheet10,
        Sprite2Ref::SpriteSheet11 => &raw mut b::spriteSheet11,
        Sprite2Ref::SpriteSheet12 => &raw mut b::spriteSheet12,
        Sprite2Ref::ExtraShapes => &raw mut b::extraShapes,
    }
}

/// Positions around and across every edge of a 320×200 surface.
const XS: [i32; 12] = [-300, -40, -11, -1, 0, 1, 150, 300, 309, 319, 330, 700];
const YS: [i32; 12] = [-300, -30, -13, -1, 0, 1, 95, 186, 190, 199, 205, 400];

/// Eight positions for the `k`th sprite; all 144 combinations are used over
/// 18 consecutive sprites.
fn positions(k: usize) -> impl Iterator<Item = (i32, i32)> {
    (0..8).map(move |j| {
        let n = (k * 8 + j) % (XS.len() * YS.len());
        (XS[n % XS.len()], YS[n / XS.len()])
    })
}

/// Loads `filename` with `JE_loadMainShapeTables` in both engines.
fn load_main(d: &mut Diff, filename: &str) -> (Option<i32>, Result<(), Halt>) {
    let name = CString::new(filename).expect("no NUL");
    // SAFETY: a valid C string; exit is caught by the guard.
    let c = unsafe { rustyrian_oracle::guard_call(|| b::JE_loadMainShapeTables(name.as_ptr())) };
    let r = s::je_load_main_shape_tables(&mut d.game, filename);
    (c, r)
}

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

fn sheet(n: usize) -> Sprite2Array {
    Sprite2Array {
        size: n,
        data: Some((0..n).map(|i| (i * 13 + n) as u8).collect()),
    }
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let mut s = State::default();
    for (t, table) in s.sprite_table.iter_mut().enumerate() {
        table.count = 10 + t;
        for (i, sp) in table.sprite.iter_mut().enumerate().take(table.count) {
            if i % 3 == 0 {
                continue;
            }
            let size = (i * 5 + t) as u16;
            *sp = Sprite {
                width: 12,
                height: 14,
                size,
                data: Some((0..size).map(|x| x as u8 ^ 0x5a).collect()),
            };
        }
    }
    s.shop_sprite_sheet = sheet(100);
    s.explosion_sprite_sheet = sheet(0);
    s.enemy_sprite_sheets = [sheet(1), Sprite2Array::default(), sheet(300), sheet(4)];
    s.enemy_sprite_sheet_ids = [1, 2, 3, 255];
    s.destruct_sprite_sheet = sheet(5);
    s.sprite_sheet8 = sheet(6);
    s.sprite_sheet9 = sheet(7);
    s.sprite_sheet10 = sheet(8);
    s.sprite_sheet11 = sheet(9);
    s.sprite_sheet12 = Sprite2Array {
        size: 77,
        data: None,
    };
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
    // Storing again replaces (and frees) the blocks stored above.
    s.shop_sprite_sheet = sheet(3);
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}

/// Both main shape files load identically, and every sprite accessor agrees
/// on every table entry.
#[test]
fn je_load_main_shape_tables_matches_on_real_data() {
    for filename in ["tyrian.shp", "tyrianc.shp"] {
        let mut d = setup(&data_dir());
        let (c, r) = load_main(&mut d, filename);
        assert_eq!((c, r), (None, Ok(())), "{filename}");
        assert_eq!(diff::load::<State>(), d.game.sprite, "{filename}");
        assert_eq!(d.c_calls(), d.rust_calls(), "{filename}");
        assert!(d.game.sprite.sprite_table[0].count > 0);
        let sp = &d.game.sprite;
        for sheet in [
            &sp.sprite_sheet8,
            &sp.sprite_sheet9,
            &sp.sprite_sheet10,
            &sp.sprite_sheet11,
            &sp.sprite_sheet12,
        ] {
            assert!(s::sprite2s_valid(sheet), "{filename}");
        }
        assert!(d.game.sprite.sprite_sheet12.data.is_some());

        for table in 0..s::SPRITE_TABLES_MAX as u32 {
            for index in 0..s::SPRITES_PER_TABLE_MAX as u32 {
                // SAFETY: indices within the table bounds.
                let (cs, ce, cw, ch) = unsafe {
                    let sp = &*b::ot_i_sprite_sprite(table, index);
                    (
                        (sp.width, sp.height, sp.size),
                        b::ot_i_sprite_sprite_exists(table, index),
                        b::ot_i_sprite_get_sprite_width(table, index),
                        b::ot_i_sprite_get_sprite_height(table, index),
                    )
                };
                let sp = s::sprite(&d.game, table, index);
                assert_eq!(cs, (sp.width, sp.height, sp.size));
                assert_eq!(ce, s::sprite_exists(&d.game, table, index));
                assert_eq!(cw, s::get_sprite_width(&d.game, table, index));
                assert_eq!(ch, s::get_sprite_height(&d.game, table, index));
            }
        }

        // SAFETY: plain C call.
        unsafe { b::free_main_shape_tables() };
        s::free_main_shape_tables(&mut d.game);
        assert_eq!(diff::load::<State>(), d.game.sprite, "{filename}");
        assert_eq!(d.game.sprite.sprite_table[0].count, 0);
    }
}

#[test]
fn je_load_main_shape_tables_missing_file_matches() {
    let mut d = setup(&tmp("missing-main"));
    let (c, r) = load_main(&mut d, "tyrian.shp");
    assert_eq!(c, Some(1));
    assert_eq!(r, Err(Halt { code: 1 }));
    assert_eq!(d.c_calls(), d.rust_calls());
}

/// Truncated and short-headered files: C decides where reads fail; state,
/// exit code and log must match.
#[test]
fn je_load_main_shape_tables_truncated_files_match() {
    let full = std::fs::read(data_dir().join("tyrian.shp")).expect("tyrian.shp");
    let mut halted = 0;
    // Position 12 is the file length, so a cut before position 11 makes the
    // last sheet's size negative (C crashes); cuts are before the count or
    // inside the last sheet.
    let pos11 =
        u32::from_le_bytes(full[2 + 11 * 4..2 + 12 * 4].try_into().expect("4 bytes")) as usize;
    let mut cases: Vec<Vec<u8>> = [0, 1, 2, pos11, pos11 + 1]
        .iter()
        .map(|&n| full[..n].to_vec())
        .collect();
    // fewer than 12 positions: the rest default to the file length
    let mut short = full.clone();
    short[0] = 9;
    cases.push(short);
    for (k, contents) in cases.iter().enumerate() {
        let dir = tmp(&format!("trunc{k}"));
        std::fs::write(dir.join("tyrian.shp"), contents).expect("write");
        let mut d = setup(&dir);
        let (c, r) = load_main(&mut d, "tyrian.shp");
        assert_eq!(c.map(|code| Halt { code }), r.err(), "case {k}");
        halted += usize::from(c.is_some());
        assert_eq!(diff::load::<State>(), d.game.sprite, "case {k}");
        assert_eq!(d.c_calls(), d.rust_calls(), "case {k}");
    }
    // Cut inside the last sheet: its sprites run past its end. C loads it
    // (and would read past the block when drawing them); Rust rejects it at
    // load (`D-UB-SPRITE-1`).
    for n in [(pos11 + full.len()) / 2, full.len() - 1] {
        let dir = tmp(&format!("cut{n}"));
        std::fs::write(dir.join("tyrian.shp"), &full[..n]).expect("write");
        let mut d = setup(&dir);
        let (c, r) = load_main(&mut d, "tyrian.shp");
        assert_eq!(c, None, "cut {n}");
        assert_eq!(r, Err(Halt { code: 1 }), "cut {n}");
        let mut c_state = diff::load::<State>();
        assert!(!s::sprite2s_valid(&c_state.sprite_sheet12), "cut {n}");
        // Everything else loaded identically.
        c_state.sprite_sheet12 = d.game.sprite.sprite_sheet12.clone();
        assert_eq!(c_state, d.game.sprite, "cut {n}");
    }
    assert!(halted >= 3, "{halted}");
}

/// Rust only (C allocates a negative size for the next sheet and crashes):
/// a sheet position far beyond the end of the file makes the positions
/// decrease, which fails as a read past the end without loading any sheet
/// (`D-UB-SPRITE-1`).
#[test]
fn je_load_main_shape_tables_huge_sheet_halts() {
    let mut contents = std::fs::read(data_dir().join("tyrian.shp")).expect("tyrian.shp");
    // position[8] (the end of sprite sheet 8) far beyond the file
    contents[2 + 8 * 4..2 + 9 * 4].copy_from_slice(&0x0200_0000u32.to_le_bytes());
    let dir = tmp("huge");
    std::fs::write(dir.join("tyrian.shp"), &contents).expect("write");
    let mut d = setup(&dir);
    let r = s::je_load_main_shape_tables(&mut d.game, "tyrian.shp");
    assert_eq!(r, Err(Halt { code: 1 }));
    assert_eq!(d.game.sprite.sprite_sheet8, Sprite2Array::default());
    assert!(d.game.sprite.sprite_table[0].count > 0);
}

/// Table positions past the end of the file (with valid sheets): C reads
/// past the end and exits; Rust matches, log included.
#[test]
fn je_load_main_shape_tables_table_past_end_matches() {
    let mut contents = std::fs::read(data_dir().join("tyrian.shp")).expect("tyrian.shp");
    contents[2 + 3 * 4..2 + 4 * 4].copy_from_slice(&0x7000_0000u32.to_le_bytes());
    let dir = tmp("table-past-end");
    std::fs::write(dir.join("tyrian.shp"), &contents).expect("write");
    let mut d = setup(&dir);
    let (c, r) = load_main(&mut d, "tyrian.shp");
    assert_eq!(c, Some(1));
    assert_eq!(r, Err(Halt { code: 1 }));
    assert_eq!(diff::load::<State>(), d.game.sprite);
    assert_eq!(d.c_calls(), d.rust_calls());
}

/// The fatal log message of the last call (Rust side).
fn last_log(d: &Diff) -> String {
    format!("{:?}", d.rust_calls())
}

/// Rust only (C reads outside the sheet when such a sprite is drawn):
/// sheets whose offset table or streams run past their end are rejected at
/// load through the fatal read-error path (`D-UB-SPRITE-1`).
#[test]
fn je_load_comp_shapes_rejects_malformed_sheets() {
    let good = std::fs::read(data_dir().join("newsh1.shp")).expect("newsh1.shp");
    let first = usize::from(u16::from_le_bytes([good[0], good[1]]));
    let mut offset_past_end = good.clone();
    offset_past_end[2..4].copy_from_slice(&(good.len() as u16).to_le_bytes());
    let mut table_past_end = good.clone();
    table_past_end[..2].copy_from_slice(&0xfffeu16.to_le_bytes());
    let mut unterminated = good[..first].to_vec();
    unterminated.extend([0x21, 1, 2, 0x10, 5]);
    for (k, contents) in [
        offset_past_end,
        table_past_end,
        unterminated,
        good[..good.len() - 1].to_vec(),
    ]
    .iter()
    .enumerate()
    {
        let sheet = Sprite2Array {
            size: contents.len(),
            data: Some(contents.clone()),
        };
        assert!(!s::sprite2s_valid(&sheet), "case {k}");
        let dir = tmp(&format!("bad{k}"));
        std::fs::write(dir.join("newsh1.shp"), contents).expect("write");
        let mut d = setup(&dir);
        let _ = d.rust_calls();
        let r = s::je_load_comp_shapes(&mut d.game, Sprite2Ref::ShopSpriteSheet, b'1');
        assert_eq!(r, Err(Halt { code: 1 }), "case {k}");
        assert!(last_log(&d).contains("Invalid sprite data"), "case {k}");
    }
    // Sheets without an offset table hold no sprites.
    for data in [None, Some(vec![]), Some(vec![7])] {
        assert!(s::sprite2s_valid(&Sprite2Array { size: 0, data }));
    }
}

/// Rust only: valid sprite tables but a sheet whose stream runs past its end
/// in the main shape file are rejected at load (`D-UB-SPRITE-1`).
#[test]
fn je_load_main_shape_tables_rejects_malformed_sheet() {
    let mut contents = std::fs::read(data_dir().join("tyrian.shp")).expect("tyrian.shp");
    let pos = |i: usize| {
        u32::from_le_bytes(contents[2 + i * 4..6 + i * 4].try_into().expect("4")) as usize
    };
    let (p8, p9) = (pos(7), pos(8));
    // sheet 8's first entry points at its own last byte, which is no terminator
    let last = (p9 - p8 - 1) as u16;
    contents[p8 + 2..p8 + 4].copy_from_slice(&last.to_le_bytes());
    contents[p9 - 1] = 0x10;
    let dir = tmp("bad-main");
    std::fs::write(dir.join("tyrian.shp"), &contents).expect("write");
    let mut d = setup(&dir);
    let _ = d.rust_calls();
    let r = s::je_load_main_shape_tables(&mut d.game, "tyrian.shp");
    assert_eq!(r, Err(Halt { code: 1 }));
    assert!(last_log(&d).contains("Invalid sprite data"));
}

/// Rust only (C's `malloc` of a negative size fails and C then crashes): a
/// sheet whose end precedes its start never panics and exits as a read
/// failure.
#[test]
fn je_load_main_shape_tables_negative_sheet_size_halts() {
    let mut contents = std::fs::read(data_dir().join("tyrian.shp")).expect("tyrian.shp");
    contents[2 + 8 * 4..2 + 9 * 4].copy_from_slice(&1u32.to_le_bytes());
    let dir = tmp("negative");
    std::fs::write(dir.join("tyrian.shp"), &contents).expect("write");
    let mut d = setup(&dir);
    let r = s::je_load_main_shape_tables(&mut d.game, "tyrian.shp");
    assert_eq!(r, Err(Halt { code: 1 }));
}

/// `estsc.shp` through `load_sprites_file`, plus its error paths.
#[test]
fn load_sprites_file_matches() {
    let mut d = setup(&data_dir());
    for (table, name) in [(7, "estsc.shp"), (3, "estsc.shp"), (7, "missing.shp")] {
        let cname = CString::new(name).expect("no NUL");
        // SAFETY: a valid C string; exit is caught by the guard.
        let c =
            unsafe { rustyrian_oracle::guard_call(|| b::load_sprites_file(table, cname.as_ptr())) };
        let r = s::load_sprites_file(&mut d.game, table, name);
        assert_eq!(c.map(|code| Halt { code }), r.err(), "{name}");
        assert_eq!(diff::load::<State>(), d.game.sprite, "{name}");
        assert_eq!(d.c_calls(), d.rust_calls(), "{name}");
    }

    let full = std::fs::read(data_dir().join("estsc.shp")).expect("estsc.shp");
    let mut too_many = full.clone();
    too_many[..2].copy_from_slice(&200u16.to_le_bytes());
    for (k, contents) in [full[..1].to_vec(), full[..500].to_vec(), too_many]
        .iter()
        .enumerate()
    {
        let dir = tmp(&format!("estsc{k}"));
        std::fs::write(dir.join("estsc.shp"), contents).expect("write");
        let mut d = setup(&dir);
        let cname = CString::new("estsc.shp").expect("no NUL");
        // SAFETY: a valid C string; exit is caught by the guard.
        let c = unsafe { rustyrian_oracle::guard_call(|| b::load_sprites_file(7, cname.as_ptr())) };
        let r = s::load_sprites_file(&mut d.game, 7, "estsc.shp");
        assert_eq!(c.map(|code| Halt { code }), r.err(), "case {k}");
        assert_eq!(diff::load::<State>(), d.game.sprite, "case {k}");
        assert_eq!(d.c_calls(), d.rust_calls(), "case {k}");
    }
}

/// Every table blit function over every sprite of every table (both main
/// shape files and `estsc.shp`), at positions across all surface edges, and
/// at indices past the table's end.
#[test]
fn blit_sprite_variants_match_on_all_tables() {
    for filename in ["tyrian.shp", "tyrianc.shp"] {
        let mut d = setup(&data_dir());
        assert_eq!(load_main(&mut d, filename), (None, Ok(())));
        let name = CString::new("estsc.shp").expect("no NUL");
        // SAFETY: a valid C string.
        unsafe { b::load_sprites_file(7, name.as_ptr()) };
        s::load_sprites_file(&mut d.game, 7, "estsc.shp").expect("estsc.shp");
        assert_eq!(diff::load::<State>(), d.game.sprite);

        let tables: Vec<(u32, u32)> = (0..s::SPRITE_TABLES_MAX as u32)
            .map(|t| (t, d.game.sprite.sprite_table[t as usize].count as u32 + 2))
            .collect();
        for &(table, n) in &tables {
            let indices = || (0..n.min(s::SPRITES_PER_TABLE_MAX as u32)).enumerate();
            let calls = || {
                indices().flat_map(|(k, index)| positions(k).map(move |(x, y)| (k, index, x, y)))
            };
            let hv = |k: usize| ((k % 16) as u8, ((k * 7) % 41) as i8 - 20);
            let screen = ScreenId::VgaScreen2;
            let label = format!("{filename} table {table}");
            // SAFETY: (each block) C blit on a valid surface and loaded tables.
            check(
                &mut d,
                &format!("blit_sprite {label}"),
                || {
                    calls().for_each(|(_, i, x, y)| {
                        // SAFETY: as above.
                        unsafe { b::blit_sprite(surf(screen), x, y, table, i) }
                    })
                },
                |g| calls().for_each(|(_, i, x, y)| s::blit_sprite(g, screen, x, y, table, i)),
            );
            check(
                &mut d,
                &format!("blit_sprite_blend {label}"),
                // SAFETY: as above.
                || {
                    calls().for_each(|(_, i, x, y)| {
                        // SAFETY: as above.
                        unsafe { b::blit_sprite_blend(surf(screen), x, y, table, i) }
                    })
                },
                |g| {
                    calls().for_each(|(_, i, x, y)| s::blit_sprite_blend(g, screen, x, y, table, i))
                },
            );
            check(
                &mut d,
                &format!("blit_sprite_hv_unsafe {label}"),
                || {
                    calls().for_each(|(k, i, x, y)| {
                        let (h, v) = hv(k);
                        // SAFETY: as above.
                        unsafe { b::blit_sprite_hv_unsafe(surf(screen), x, y, table, i, h, v) }
                    })
                },
                |g| {
                    calls().for_each(|(k, i, x, y)| {
                        let (h, v) = hv(k);
                        s::blit_sprite_hv_unsafe(g, screen, x, y, table, i, h, v)
                    })
                },
            );
            check(
                &mut d,
                &format!("blit_sprite_hv {label}"),
                || {
                    calls().for_each(|(k, i, x, y)| {
                        let (h, v) = hv(k);
                        // SAFETY: as above.
                        unsafe { b::blit_sprite_hv(surf(screen), x, y, table, i, h, v) }
                    })
                },
                |g| {
                    calls().for_each(|(k, i, x, y)| {
                        let (h, v) = hv(k);
                        s::blit_sprite_hv(g, screen, x, y, table, i, h, v)
                    })
                },
            );
            check(
                &mut d,
                &format!("blit_sprite_hv_blend {label}"),
                || {
                    calls().for_each(|(k, i, x, y)| {
                        let (h, v) = hv(k);
                        // SAFETY: as above.
                        unsafe { b::blit_sprite_hv_blend(surf(screen), x, y, table, i, h, v) }
                    })
                },
                |g| {
                    calls().for_each(|(k, i, x, y)| {
                        let (h, v) = hv(k);
                        s::blit_sprite_hv_blend(g, screen, x, y, table, i, h, v)
                    })
                },
            );
            check(
                &mut d,
                &format!("blit_sprite_dark {label}"),
                || {
                    calls().for_each(|(k, i, x, y)| {
                        // SAFETY: as above.
                        unsafe { b::blit_sprite_dark(surf(screen), x, y, table, i, k % 3 == 0) }
                    })
                },
                |g| {
                    calls().for_each(|(k, i, x, y)| {
                        s::blit_sprite_dark(g, screen, x, y, table, i, k % 3 == 0)
                    })
                },
            );
            // Fresh surface for the next table.
            d.game.video.screens[ScreenId::VgaScreen2 as usize] = Some(pattern(table));
        }
        assert_eq!(d.c_calls(), d.rust_calls());
    }
}

/// The number of entries in a sheet's offset table (its first offset / 2).
fn sheet_entries(sheet: &Sprite2Array) -> u32 {
    let data = sheet.data.as_deref().expect("loaded");
    u32::from(u16::from_le_bytes([data[0], data[1]])) / 2
}

/// Runs every `blit_sprite2*` function over every sprite of sheet `r` (all
/// positions; `x2` = the 2×2 variants where all four quarters exist).
fn check_sheet(d: &mut Diff, label: &str, r: Sprite2Ref) {
    let n = {
        let sheet = s::sprite2s(&d.game.sprite, &d.game.editship, r);
        sheet_entries(sheet)
    };
    let screen = ScreenId::VgaScreenSeg;
    let calls = |n: u32| {
        (1..=n)
            .enumerate()
            .flat_map(|(k, index)| positions(k).map(move |(x, y)| (k, index, x, y)))
    };
    let n2 = n.saturating_sub(20);
    let filter = |k: usize| ((k % 16) as u8) << 4;
    macro_rules! both {
        ($name:literal, $n:expr, |$k:ident, $i:ident, $x:ident, $y:ident| $c:expr, $g:ident => $r:expr) => {
            check(
                d,
                &format!("{} {label}", $name),
                || {
                    calls($n).for_each(|($k, $i, $x, $y)| {
                        let _ = $k;
                        // SAFETY: C blit on a valid surface and a loaded sheet,
                        // at an index inside its offset table.
                        unsafe { $c }
                    })
                },
                |$g| {
                    calls($n).for_each(|($k, $i, $x, $y)| {
                        let _ = $k;
                        $r
                    })
                },
            )
        };
    }
    both!("blit_sprite2", n, |k, i, x, y| b::blit_sprite2(surf(screen), x, y, c_sheet(r), i),
        g => s::blit_sprite2(g, screen, x, y, r, i));
    both!("blit_sprite2_clip", n, |k, i, x, y| b::blit_sprite2_clip(surf(screen), x, y, c_sheet(r), i),
        g => s::blit_sprite2_clip(g, screen, x, y, r, i));
    both!("blit_sprite2_blend", n, |k, i, x, y| b::blit_sprite2_blend(surf(screen), x, y, c_sheet(r), i),
        g => s::blit_sprite2_blend(g, screen, x, y, r, i));
    both!("blit_sprite2_darken", n, |k, i, x, y| b::blit_sprite2_darken(surf(screen), x, y, c_sheet(r), i),
        g => s::blit_sprite2_darken(g, screen, x, y, r, i));
    both!("blit_sprite2_filter", n, |k, i, x, y| b::blit_sprite2_filter(surf(screen), x, y, c_sheet(r), i, filter(k)),
        g => s::blit_sprite2_filter(g, screen, x, y, r, i, filter(k)));
    both!("blit_sprite2_filter_clip", n, |k, i, x, y| b::blit_sprite2_filter_clip(surf(screen), x, y, c_sheet(r), i, filter(k)),
        g => s::blit_sprite2_filter_clip(g, screen, x, y, r, i, filter(k)));
    both!("blit_sprite2x2", n2, |k, i, x, y| b::blit_sprite2x2(surf(screen), x, y, c_sheet(r), i),
        g => s::blit_sprite2x2(g, screen, x, y, r, i));
    both!("blit_sprite2x2_clip", n2, |k, i, x, y| b::blit_sprite2x2_clip(surf(screen), x, y, c_sheet(r), i),
        g => s::blit_sprite2x2_clip(g, screen, x, y, r, i));
    both!("blit_sprite2x2_blend", n2, |k, i, x, y| b::blit_sprite2x2_blend(surf(screen), x, y, c_sheet(r), i),
        g => s::blit_sprite2x2_blend(g, screen, x, y, r, i));
    both!("blit_sprite2x2_darken", n2, |k, i, x, y| b::blit_sprite2x2_darken(surf(screen), x, y, c_sheet(r), i),
        g => s::blit_sprite2x2_darken(g, screen, x, y, r, i));
    both!("blit_sprite2x2_filter", n2, |k, i, x, y| b::blit_sprite2x2_filter(surf(screen), x, y, c_sheet(r), i, filter(k)),
        g => s::blit_sprite2x2_filter(g, screen, x, y, r, i, filter(k)));
    both!("blit_sprite2x2_filter_clip", n2, |k, i, x, y| b::blit_sprite2x2_filter_clip(surf(screen), x, y, c_sheet(r), i, filter(k)),
        g => s::blit_sprite2x2_filter_clip(g, screen, x, y, r, i, filter(k)));
}

/// The five sprite sheets of both main shape files.
#[test]
fn blit_sprite2_variants_match_on_main_sheets() {
    for filename in ["tyrian.shp", "tyrianc.shp"] {
        let mut d = setup(&data_dir());
        assert_eq!(load_main(&mut d, filename), (None, Ok(())));
        for r in [
            Sprite2Ref::SpriteSheet8,
            Sprite2Ref::SpriteSheet9,
            Sprite2Ref::SpriteSheet10,
            Sprite2Ref::SpriteSheet11,
            Sprite2Ref::SpriteSheet12,
        ] {
            check_sheet(&mut d, &format!("{filename} {r:?}"), r);
        }
    }
}

/// The `newsh?.shp` letters present in the freeware data.
fn comp_shape_letters() -> Vec<u8> {
    let mut v: Vec<u8> = std::fs::read_dir(data_dir())
        .expect("data/")
        .filter_map(|e| {
            let name = e.ok()?.file_name().into_string().ok()?;
            let rest = name.strip_prefix("newsh")?.strip_suffix(".shp")?;
            (rest.len() == 1).then(|| rest.as_bytes()[0])
        })
        .collect();
    v.sort_unstable();
    v
}

/// Every `newsh?.shp` loads identically through `JE_loadCompShapes` into
/// every sheet global it is loaded into upstream, and every blit function
/// matches over all of its sprites.
#[test]
fn je_load_comp_shapes_and_blits_match_on_every_sheet_file() {
    let letters = comp_shape_letters();
    assert!(letters.len() >= 34, "{letters:?}");
    let targets = [
        Sprite2Ref::ShopSpriteSheet,
        Sprite2Ref::ExplosionSpriteSheet,
        Sprite2Ref::EnemySpriteSheets(0),
        Sprite2Ref::EnemySpriteSheets(1),
        Sprite2Ref::EnemySpriteSheets(2),
        Sprite2Ref::EnemySpriteSheets(3),
        Sprite2Ref::DestructSpriteSheet,
    ];
    let mut d = setup(&data_dir());
    for (k, &letter) in letters.iter().enumerate() {
        let r = targets[k % targets.len()];
        // Upper case exercises `tolower` (only letters differ).
        let arg = if k % 2 == 0 {
            letter.to_ascii_uppercase()
        } else {
            letter
        };
        // SAFETY: a valid sheet global; exit is caught by the guard.
        let c = unsafe {
            rustyrian_oracle::guard_call(|| {
                b::JE_loadCompShapes(c_sheet_mut(r), arg as std::ffi::c_char)
            })
        };
        let rr = s::je_load_comp_shapes(&mut d.game, r, arg);
        assert_eq!((c, rr), (None, Ok(())), "newsh{}.shp", letter as char);
        assert_eq!(
            diff::load::<State>(),
            d.game.sprite,
            "newsh{}.shp",
            letter as char
        );
        check_sheet(&mut d, &format!("newsh{}.shp", letter as char), r);
    }
    assert_eq!(d.c_calls(), d.rust_calls());
}

#[test]
fn je_load_comp_shapes_error_paths_match() {
    // missing file
    let mut d = setup(&tmp("missing-comp"));
    // SAFETY: a valid sheet global; exit is caught by the guard.
    let c = unsafe {
        rustyrian_oracle::guard_call(|| {
            b::JE_loadCompShapes(&raw mut b::shopSpriteSheet, b'1' as _)
        })
    };
    let r = s::je_load_comp_shapes(&mut d.game, Sprite2Ref::ShopSpriteSheet, b'1');
    assert_eq!(c.map(|code| Halt { code }), r.err());
    assert_eq!(c, Some(1));
    assert_eq!(diff::load::<State>(), d.game.sprite);
    assert_eq!(d.c_calls(), d.rust_calls());

    // A directory named like the file opens but cannot be read. Where its
    // SEEK_END length is huge (ext4 reports LLONG_MAX), C's `malloc` returns
    // NULL and it reads into it (UF-22), so only Rust runs: the allocation
    // guard makes it a read error (`D-UB-SPRITE-2`).
    let dir = tmp("dir-comp");
    std::fs::create_dir(dir.join("newsh1.shp")).expect("mkdir");
    let mut d = setup(&dir);
    let dir_len = {
        use std::io::Seek;
        std::fs::File::open(dir.join("newsh1.shp"))
            .and_then(|mut f| f.seek(std::io::SeekFrom::End(0)))
            .unwrap_or(0)
    };
    let r = s::je_load_comp_shapes(&mut d.game, Sprite2Ref::ShopSpriteSheet, b'1');
    if dir_len < 1 << 32 {
        // SAFETY: as above.
        let c = unsafe {
            rustyrian_oracle::guard_call(|| {
                b::JE_loadCompShapes(&raw mut b::shopSpriteSheet, b'1' as _)
            })
        };
        assert_eq!(c.map(|code| Halt { code }), r.err());
        assert_eq!(d.c_calls(), d.rust_calls());
    } else {
        assert_eq!(r, Err(Halt { code: 1 }));
        let calls = format!("{:?}", d.rust_calls());
        assert!(
            calls.contains("Failed to read from file 'newsh1.shp': Cannot allocate memory"),
            "{calls}"
        );
        assert_eq!(d.game.sprite.shop_sprite_sheet.data, None);
    }
}

#[test]
fn free_sprite2s_matches() {
    let mut d = setup(&data_dir());
    d.game.sprite.explosion_sprite_sheet = sheet(40);
    d.store_to_c();
    // SAFETY: a valid sheet global.
    unsafe { b::free_sprite2s(&raw mut b::explosionSpriteSheet) };
    s::free_sprite2s(&mut d.game.sprite.explosion_sprite_sheet);
    assert_eq!(diff::load::<State>(), d.game.sprite);
    assert_eq!(
        d.game.sprite.explosion_sprite_sheet,
        Sprite2Array::default()
    );
}

/// A well-formed random `Sprite2_array`: an offset table of `streams.len()`
/// entries, then each stream followed by 16 terminators (so no run can skip
/// past the end).
fn sheet_from_streams(streams: &[Vec<u8>]) -> Sprite2Array {
    let mut data = vec![0u8; streams.len() * 2];
    for (i, st) in streams.iter().enumerate() {
        let off = data.len() as u16;
        data[i * 2..i * 2 + 2].copy_from_slice(&off.to_le_bytes());
        data.extend_from_slice(st);
        data.extend_from_slice(&[0x0f; 16]);
    }
    Sprite2Array {
        size: data.len(),
        data: Some(data),
    }
}

/// Whether a sprite stream ends on a transparent-run marker (255) whose
/// count byte is past the data: upstream's blitters then read one byte past
/// the block (UF-3). The plain oracle reads its zeroed allocation slack there;
/// the sanitized oracle has no slack and aborts.
fn has_dangling_run_marker(data: &[u8]) -> bool {
    let mut i = 0;
    while i < data.len() {
        if data[i] == 255 {
            if i + 1 == data.len() {
                return true;
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    false
}

proptest! {
    /// Random sprites (any bytes) blitted with every table function at any
    /// position.
    #[test]
    fn blit_sprite_variants_match_random(
        width in 0u16..40,
        data in prop::collection::vec(any::<u8>(), 0..400),
        x in -400i32..800,
        y in -300i32..500,
        hue in any::<u8>(),
        value in any::<i8>(),
        black in any::<bool>(),
        exists in any::<bool>(),
        salt in any::<u32>(),
    ) {
        prop_assume!(!(cfg!(ot_ref_sanitize) && exists && has_dangling_run_marker(&data)));
        let mut d = setup(&data_dir());
        d.game.video.screens[0] = Some(pattern(salt));
        let t = &mut d.game.sprite.sprite_table[4];
        t.count = 3;
        t.sprite[1] = Sprite {
            width,
            height: 9,
            size: data.len() as u16,
            data: exists.then(|| data.clone()),
        };
        d.game.sprite.store_to_c();
        let sc = ScreenId::VgaScreenSeg;
        // SAFETY: (each) C blit on a valid surface and stored tables.
        check(&mut d, "blit_sprite", || unsafe { b::blit_sprite(surf(sc), x, y, 4, 1) },
            |g| s::blit_sprite(g, sc, x, y, 4, 1));
        // SAFETY: as above.
        check(&mut d, "blend", || unsafe { b::blit_sprite_blend(surf(sc), x, y, 4, 1) },
            |g| s::blit_sprite_blend(g, sc, x, y, 4, 1));
        // SAFETY: as above.
        check(&mut d, "hv_unsafe", || unsafe { b::blit_sprite_hv_unsafe(surf(sc), x, y, 4, 1, hue, value) },
            |g| s::blit_sprite_hv_unsafe(g, sc, x, y, 4, 1, hue, value));
        // SAFETY: as above.
        check(&mut d, "hv", || unsafe { b::blit_sprite_hv(surf(sc), x, y, 4, 1, hue, value) },
            |g| s::blit_sprite_hv(g, sc, x, y, 4, 1, hue, value));
        // SAFETY: as above.
        check(&mut d, "hv_blend", || unsafe { b::blit_sprite_hv_blend(surf(sc), x, y, 4, 1, hue, value) },
            |g| s::blit_sprite_hv_blend(g, sc, x, y, 4, 1, hue, value));
        // SAFETY: as above.
        check(&mut d, "dark", || unsafe { b::blit_sprite_dark(surf(sc), x, y, 4, 1, black) },
            |g| s::blit_sprite_dark(g, sc, x, y, 4, 1, black));
    }

    /// Random well-formed sheets blitted with every sheet function at any
    /// position.
    #[test]
    fn blit_sprite2_variants_match_random(
        streams in prop::collection::vec(prop::collection::vec(any::<u8>(), 0..120), 24..27),
        x in -400i32..800,
        y in -300i32..500,
        index in 1u32..4,
        salt in any::<u32>(),
    ) {
        let mut d = setup(&data_dir());
        d.game.video.screens[1] = Some(pattern(salt));
        d.game.video.vga_screen = Some(ScreenId::VgaScreen2);
        d.game.sprite.sprite_sheet10 = sheet_from_streams(&streams);
        d.store_to_c();
        let r = Sprite2Ref::SpriteSheet10;
        let sc = ScreenId::VgaScreenSeg;
        let f = (salt as u8) & 0xf0;
        macro_rules! both {
            ($c:ident, $r:ident $(, $extra:expr)?) => {
                // SAFETY: C blit on a valid surface and a stored sheet.
                check(&mut d, stringify!($r), || unsafe { b::$c(surf(sc), x, y, c_sheet(r), index $(, $extra)?) },
                    |g| s::$r(g, sc, x, y, r, index $(, $extra)?));
            };
        }
        both!(blit_sprite2, blit_sprite2);
        both!(blit_sprite2_clip, blit_sprite2_clip);
        both!(blit_sprite2_blend, blit_sprite2_blend);
        both!(blit_sprite2_darken, blit_sprite2_darken);
        both!(blit_sprite2_filter, blit_sprite2_filter, f);
        both!(blit_sprite2_filter_clip, blit_sprite2_filter_clip, f);
        both!(blit_sprite2x2, blit_sprite2x2);
        both!(blit_sprite2x2_clip, blit_sprite2x2_clip);
        both!(blit_sprite2x2_blend, blit_sprite2x2_blend);
        both!(blit_sprite2x2_darken, blit_sprite2x2_darken);
        both!(blit_sprite2x2_filter, blit_sprite2x2_filter, f);
        both!(blit_sprite2x2_filter_clip, blit_sprite2x2_filter_clip, f);
    }

    /// Rust only (C reads outside the block here, `D-UB-SPRITE-1`): any
    /// bytes as a sheet, any index, never panic.
    #[test]
    fn blit_sprite2_never_panics_on_malformed_sheets(
        data in prop::collection::vec(any::<u8>(), 0..300),
        none in any::<bool>(),
        x in -400i32..800,
        y in -300i32..500,
        index in any::<u32>(),
    ) {
        let mut g = setup(&data_dir()).game;
        g.sprite.sprite_sheet11 = Sprite2Array { size: data.len(), data: (!none).then_some(data) };
        let r = Sprite2Ref::SpriteSheet11;
        let sc = ScreenId::GameScreen;
        s::blit_sprite2(&mut g, sc, x, y, r, index);
        s::blit_sprite2_clip(&mut g, sc, x, y, r, index);
        s::blit_sprite2_darken(&mut g, sc, x, y, r, index);
        s::blit_sprite2x2_filter_clip(&mut g, sc, x, y, r, index, 0x30);
    }
}
