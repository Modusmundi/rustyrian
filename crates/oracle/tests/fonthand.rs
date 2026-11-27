//! Differential tests for `fonthand.c` / `fonthand.h`.

use proptest::prelude::*;
use rustyrian_core::Game;
use rustyrian_core::fonthand::{
    self as fh, DARKEN, FONT_MAP, FULL_SHADE, NO_SHADE, PART_SHADE, State, TRICK,
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
    let mut s = State {
        text_glow_font: 3,
        text_glow_brightness: 200,
        level_warning_display: true,
        level_warning_lines: 9,
        warning_red: true,
        warning_sound_delay: 17,
        armor_ship_delay: 0xbeef,
        warning_col: 0x80,
        warning_col_change: -3,
        ..State::default()
    };
    for (i, l) in s.level_warning_text.iter_mut().enumerate() {
        l.iter_mut()
            .enumerate()
            .for_each(|(j, c)| *c = (i * 61 + j) as u8);
    }
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}

#[test]
fn font_map_matches_c() {
    // SAFETY: reading a const C table.
    assert_eq!(FONT_MAP, unsafe { b::fontMap });
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
    let id = ScreenId::VgaScreen2;
    d.game.video.store_to_c();
    c(screen_to_c(Some(id)));
    rust(&mut d.game, id);
    assert!(
        diff::load::<video::State>().screens == d.game.video.screens,
        "{label}: surfaces differ"
    );
    assert_eq!(d.c_calls(), d.rust_calls());
}

/// Draws `text` with every portable function of `fonthand.c` and compares
/// the text measurements.
#[allow(clippy::too_many_arguments)]
fn check_all(
    d: &mut Diff,
    text: &[u8],
    x: i32,
    y: i32,
    font: u32,
    bank: u32,
    brightness: i32,
    shadow: bool,
) {
    let ct = CString::new(text).expect("no NUL");
    let p = ct.as_ptr();
    // SAFETY: C measures a valid string with loaded fonts.
    let width = unsafe { b::JE_textWidth(p, font) };
    assert_eq!(width, fh::je_text_width(&d.game, text, font));
    // SAFETY: as above.
    let center = unsafe { b::JE_fontCenter(p, font) };
    assert_eq!(center, fh::je_font_center(&d.game, text, font));
    // SAFETY: (each block) C draws on a valid surface with loaded fonts.
    check(
        d,
        "JE_dString",
        |s| unsafe { b::JE_dString(s, x, y, p, font) },
        |g, s| fh::je_d_string(g, s, x, y, text, font),
    );
    // SAFETY: as above.
    check(
        d,
        "JE_outText",
        |s| unsafe { b::JE_outText(s, x, y, p, bank, brightness) },
        |g, s| fh::je_out_text(g, s, x, y, text, bank, brightness),
    );
    // SAFETY: as above.
    check(
        d,
        "JE_outTextModify",
        |s| unsafe { b::JE_outTextModify(s, x, y, p, bank, brightness as u32, font) },
        |g, s| fh::je_out_text_modify(g, s, x, y, text, bank, brightness as u32, font),
    );
    // SAFETY: as above.
    check(
        d,
        "JE_outTextAdjust",
        |s| unsafe { b::JE_outTextAdjust(s, x, y, p, bank, brightness, font, shadow) },
        |g, s| fh::je_out_text_adjust(g, s, x, y, text, bank, brightness, font, shadow),
    );
    // SAFETY: as above.
    check(
        d,
        "JE_outTextAndDarken",
        |s| unsafe { b::JE_outTextAndDarken(s, x, y, p, bank, brightness as u32, font) },
        |g, s| fh::je_out_text_and_darken(g, s, x, y, text, bank, brightness as u32, font),
    );
    for shadetype in [PART_SHADE, FULL_SHADE, DARKEN, TRICK, NO_SHADE, 7] {
        // SAFETY: as above.
        check(
            d,
            "JE_textShade",
            |s| unsafe { b::JE_textShade(s, x, y, p, bank, brightness, shadetype) },
            |g, s| fh::je_text_shade(g, s, x, y, text, bank, brightness, shadetype),
        );
    }
}

/// Every character, `~` toggles, and every font table (the empty ones too),
/// in both main shape files.
#[test]
fn text_functions_match_on_all_glyphs() {
    for file in ["tyrian.shp", "tyrianc.shp"] {
        let mut d = setup(file);
        let all: Vec<u8> = (1..=255u8).collect();
        for font in 0..8 {
            for (k, chunk) in all.chunks(40).enumerate() {
                let brightness = k as i32 * 3 - 4;
                check_all(
                    &mut d,
                    chunk,
                    2,
                    30,
                    font,
                    (k % 16) as u32,
                    brightness,
                    k % 2 == 0,
                );
            }
            check_all(&mut d, b"~Hi~ there ~you~", 150, 100, font, 14, 3, true);
            check_all(&mut d, b"", 0, 0, font, 0, 0, false);
        }
    }
}

proptest! {
    #[test]
    fn text_functions_match_random(
        text in prop::collection::vec(1u8..=255, 0..24),
        x in -100i32..400,
        y in -40i32..230,
        font in 0u32..8,
        bank in 0u32..300,
        brightness in -300i32..300,
        shadow in any::<bool>(),
    ) {
        let mut d = setup("tyrian.shp");
        check_all(&mut d, &text, x, y, font, bank, brightness, shadow);
    }
}

/// `JE_updateWarning` with video and audio initialised and the sounds
/// loaded: every color-bar boundary and direction, the sound countdown, and
/// both an expired and a pending `frameCount2`.
#[test]
fn je_update_warning_matches() {
    use rustyrian_core::loudness::{self as ld, AudioState};
    use rustyrian_core::nortsong;
    use rustyrian_oracle::mirror::video::sv;
    use std::os::unix::ffi::OsStrExt;

    let mut d = Diff::new();
    d.game.file.custom_data_dir_path = Some(data_dir().as_os_str().as_bytes().to_vec());
    d.store_to_c();
    // SAFETY: plain C calls on the test thread; exit is caught by the guard.
    let c = unsafe { rustyrian_oracle::guard_call(|| b::init_video()) };
    assert_eq!(c, None);
    rustyrian_core::video::init_video(&mut d.game).expect("init_video");
    // SAFETY: as above.
    let c = unsafe { b::init_audio() };
    assert_eq!(c, ld::init_audio(&mut d.game));
    // SAFETY: as above.
    let c = unsafe { rustyrian_oracle::guard_call(|| b::loadSndFile(false)) };
    assert_eq!(
        (c, nortsong::load_snd_file(&mut d.game, false)),
        (None, Ok(()))
    );
    // The C session now holds what the Rust game holds; start both from the
    // Rust state.
    let screen = d.game.video.vga_screen_id();
    assert_eq!(d.c_calls(), d.rust_calls());

    let mut k = 0u32;
    for col in [0u8, 100, 227, 228, 229, 233, 234, 235, 255] {
        for change in [-128i8, -3, -1, 0, 1, 3, 127] {
            for delay in [0u8, 1, 14, 255] {
                for pending in [false, true] {
                    k += 1;
                    let now = d.game.platform.get_ticks() << 10;
                    let _ = d.rust_calls();
                    d.game.fonthand.warning_col = col;
                    d.game.fonthand.warning_col_change = change;
                    d.game.fonthand.warning_sound_delay = delay;
                    d.game.nortsong.frame_count2_end = if pending {
                        now.wrapping_add((k % 7 + 1) << 10)
                    } else {
                        now.wrapping_sub(k << 10)
                    };
                    d.game.fonthand.store_to_c();
                    // Only the timer: storing the whole nortsong state would
                    // reallocate the sample buffers playing channels point to.
                    // SAFETY: C runs only on this thread; the static is a u32.
                    unsafe {
                        *sv::<u32>(b::ot_sv_nortsong_frameCount2End) =
                            d.game.nortsong.frame_count2_end
                    };
                    let label =
                        format!("col {col} change {change} delay {delay} pending {pending}");

                    // SAFETY: plain C call on a live surface.
                    unsafe { b::JE_updateWarning(screen_to_c(Some(screen))) };
                    fh::je_update_warning(&mut d.game, screen);

                    assert_eq!(diff::load::<State>(), d.game.fonthand, "{label}");
                    assert_eq!(diff::load::<nortsong::State>(), d.game.nortsong, "{label}");
                    assert_eq!(diff::load::<AudioState>(), *ld::audio(&d.game), "{label}");
                    assert!(
                        diff::load::<video::State>().screens == d.game.video.screens,
                        "{label}: surfaces differ"
                    );
                    assert_eq!(d.c_calls(), d.rust_calls(), "{label}");
                }
            }
        }
    }
}
