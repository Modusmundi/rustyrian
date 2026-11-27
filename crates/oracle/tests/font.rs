//! Differential tests for `font.c` / `font.h` (no mutable globals).

use proptest::prelude::*;
use rustyrian_core::Game;
use rustyrian_core::font::{
    self as f, ALIGN_CENTER, ALIGN_LEFT, ALIGN_RIGHT, FONT_LARGE, FONT_NORMAL, FONT_SMALL, Font,
    FontAlignment, State,
};
use rustyrian_core::sdl::Surface;
use rustyrian_core::sprite;
use rustyrian_core::video::{self, ScreenId};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;
use rustyrian_oracle::mirror::video::screen_to_c;
use std::ffi::CString;
use std::path::{Path, PathBuf};

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let s = State::default();
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}

#[test]
fn enum_values_match_c() {
    assert_eq!(FONT_LARGE as u32, b::Font_FONT_LARGE);
    assert_eq!(FONT_NORMAL as u32, b::Font_FONT_NORMAL);
    assert_eq!(FONT_SMALL as u32, b::Font_FONT_SMALL);
    assert_eq!(ALIGN_LEFT as u32, b::FontAlignment_ALIGN_LEFT);
    assert_eq!(ALIGN_CENTER as u32, b::FontAlignment_ALIGN_CENTER);
    assert_eq!(ALIGN_RIGHT as u32, b::FontAlignment_ALIGN_RIGHT);
}

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data")
        .canonicalize()
        .expect("data/ (run `cargo xtask fetch-data`)")
}

fn pattern(salt: u32) -> Surface {
    let mut surf = Surface::new_8bit(320, 200);
    for (i, p) in surf.pixels.iter_mut().enumerate() {
        *p = ((i as u32).wrapping_mul(2_654_435_761).wrapping_add(salt) >> 13) as u8;
    }
    surf
}

/// Both engines with the surfaces of `init_video` and the main shape tables
/// (`file`) loaded.
fn setup(file: &str) -> Diff {
    use std::os::unix::ffi::OsStrExt;
    let mut d = Diff::new();
    let vs = &mut d.game.video;
    for (i, slot) in vs.screens.iter_mut().enumerate() {
        *slot = Some(pattern(i as u32));
    }
    vs.vga_screen = Some(ScreenId::VgaScreenSeg);
    vs.vga_screen_seg = Some(ScreenId::VgaScreenSeg);
    vs.vga_screen2 = Some(ScreenId::VgaScreen2);
    vs.game_screen = Some(ScreenId::GameScreen);
    d.game.file.custom_data_dir_path = Some(data_dir().as_os_str().as_bytes().to_vec());
    d.store_to_c();
    let name = CString::new(file).expect("no NUL");
    // SAFETY: a valid C string.
    unsafe { b::JE_loadMainShapeTables(name.as_ptr()) };
    sprite::je_load_main_shape_tables(&mut d.game, file).expect("shape tables");
    d
}

fn check(
    d: &mut Diff,
    label: &str,
    c: impl FnOnce(*mut b::SDL_Surface),
    rust: impl FnOnce(&mut Game, ScreenId),
) {
    let id = ScreenId::VgaScreenSeg;
    d.game.video.store_to_c();
    c(screen_to_c(Some(id)));
    rust(&mut d.game, id);
    assert!(
        diff::load::<video::State>().screens == d.game.video.screens,
        "{label}: surfaces differ"
    );
    assert_eq!(d.c_calls(), d.rust_calls());
}

const FONTS: [Font; 3] = [Font::Large, Font::Normal, Font::Small];
const ALIGNMENTS: [FontAlignment; 3] = [
    FontAlignment::Left,
    FontAlignment::Center,
    FontAlignment::Right,
];

/// Draws `text` with every function of `font.c`, every font and alignment.
#[allow(clippy::too_many_arguments)]
fn check_all(
    d: &mut Diff,
    text: &[u8],
    x: i32,
    y: i32,
    hue: u8,
    value: i8,
    black: bool,
    dist: i32,
) {
    let ct = CString::new(text).expect("no NUL");
    let p = ct.as_ptr();
    for font in FONTS {
        let cf = font as b::Font;
        // SAFETY: (each block) C draws on a valid surface with loaded fonts.
        check(
            d,
            "drawFontHv",
            |s| unsafe { b::drawFontHv(s, x, y, p, cf, hue, value) },
            |g, s| f::draw_font_hv(g, s, x, y, text, font, hue, value),
        );
        // SAFETY: as above.
        check(
            d,
            "drawFontHvBlend",
            |s| unsafe { b::drawFontHvBlend(s, x, y, p, cf, hue, value) },
            |g, s| f::draw_font_hv_blend(g, s, x, y, text, font, hue, value),
        );
        // SAFETY: as above.
        check(
            d,
            "drawFontDark",
            |s| unsafe { b::drawFontDark(s, x, y, p, cf, black) },
            |g, s| f::draw_font_dark(g, s, x, y, text, font, black),
        );
        // SAFETY: as above.
        check(
            d,
            "drawFontHvShadow",
            |s| unsafe { b::drawFontHvShadow(s, x, y, p, cf, hue, value, black, dist) },
            |g, s| f::draw_font_hv_shadow(g, s, x, y, text, font, hue, value, black, dist),
        );
        // SAFETY: as above.
        check(
            d,
            "drawFontHvFullShadow",
            |s| unsafe { b::drawFontHvFullShadow(s, x, y, p, cf, hue, value, black, dist) },
            |g, s| f::draw_font_hv_full_shadow(g, s, x, y, text, font, hue, value, black, dist),
        );
        for al in ALIGNMENTS {
            let ca = al as b::FontAlignment;
            // SAFETY: as above.
            check(
                d,
                "drawFontHvAligned",
                |s| unsafe { b::drawFontHvAligned(s, x, y, p, cf, ca, hue, value) },
                |g, s| f::draw_font_hv_aligned(g, s, x, y, text, font, al, hue, value),
            );
            // SAFETY: as above.
            check(
                d,
                "drawFontHvBlendAligned",
                |s| unsafe { b::drawFontHvBlendAligned(s, x, y, p, cf, ca, hue, value) },
                |g, s| f::draw_font_hv_blend_aligned(g, s, x, y, text, font, al, hue, value),
            );
            // SAFETY: as above.
            check(
                d,
                "drawFontDarkAligned",
                |s| unsafe { b::drawFontDarkAligned(s, x, y, p, cf, ca, black) },
                |g, s| f::draw_font_dark_aligned(g, s, x, y, text, font, al, black),
            );
            // SAFETY: as above.
            check(
                d,
                "drawFontHvShadowAligned",
                |s| unsafe {
                    b::drawFontHvShadowAligned(s, x, y, p, cf, ca, hue, value, black, dist)
                },
                |g, s| {
                    f::draw_font_hv_shadow_aligned(
                        g, s, x, y, text, font, al, hue, value, black, dist,
                    )
                },
            );
            // SAFETY: as above.
            check(
                d,
                "drawFontHvFullShadowAligned",
                |s| unsafe {
                    b::drawFontHvFullShadowAligned(s, x, y, p, cf, ca, hue, value, black, dist)
                },
                |g, s| {
                    f::draw_font_hv_full_shadow_aligned(
                        g, s, x, y, text, font, al, hue, value, black, dist,
                    )
                },
            );
        }
    }
}

/// Every character with a glyph, `~` highlighting and characters without
/// glyphs, in both main shape files.
#[test]
fn draw_font_functions_match_on_all_glyphs() {
    for file in ["tyrian.shp", "tyrianc.shp"] {
        let mut d = setup(file);
        let all: Vec<u8> = (1..=255u8).collect();
        for chunk in all.chunks(40) {
            check_all(&mut d, chunk, 3, 20, 4, 2, false, 1);
        }
        check_all(&mut d, b"~Hi~ there ~you~", 150, 100, 15, -3, true, 2);
        check_all(&mut d, b"", 0, 0, 0, 0, false, 0);
        check_all(
            &mut d,
            b"~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ wrap",
            280,
            190,
            9,
            126,
            true,
            3,
        );
    }
}

proptest! {
    #[test]
    fn draw_font_functions_match_random(
        text in prop::collection::vec(1u8..=255, 0..24),
        x in -100i32..400,
        y in -40i32..230,
        hue in any::<u8>(),
        value in any::<i8>(),
        black in any::<bool>(),
        dist in -3i32..4,
    ) {
        let mut d = setup("tyrian.shp");
        check_all(&mut d, &text, x, y, hue, value, black, dist);
    }
}
