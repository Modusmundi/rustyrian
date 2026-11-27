//! Differential tests for `helptext.c` / `helptext.h`.

use proptest::prelude::*;
use rustyrian_core::file::{self as f, File};
use rustyrian_core::game::Halt;
use rustyrian_core::helptext::{self as h, State};
use rustyrian_core::sdl::Surface;
use rustyrian_core::sprite;
use rustyrian_core::video::{self, ScreenId};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;
use rustyrian_oracle::mirror::video::screen_to_c;
use std::ffi::{CString, c_char};
use std::path::{Path, PathBuf};

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data")
        .canonicalize()
        .expect("data/ (run `cargo xtask fetch-data`)")
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir()
        .join(format!("rustyrian-helptext-{}", std::process::id()))
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("scratch dir");
    d
}

fn bytes(p: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    p.as_os_str().as_bytes().to_vec()
}

/// `menus.c`'s name tables as C holds them.
fn c_menu_names() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    // SAFETY: C runs only on this thread, inside calls we make.
    unsafe {
        let flat = |p: *const u8, n: usize| std::slice::from_raw_parts(p, n).to_vec();
        (
            flat((&raw const b::episode_name).cast(), 6 * 31),
            flat((&raw const b::difficulty_name).cast(), 7 * 21),
            flat((&raw const b::gameplay_name).cast(), 5 * 26),
        )
    }
}

fn rust_menu_names(g: &rustyrian_core::Game) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    (
        g.menus.episode_name.concat(),
        g.menus.difficulty_name.concat(),
        g.menus.gameplay_name.concat(),
    )
}

/// Runs `JE_loadHelpText` in both engines with the data directory `dir`.
fn load_both(dir: &Path) -> (Diff, Option<i32>, Result<(), Halt>) {
    let mut d = Diff::new();
    d.game.file.custom_data_dir_path = Some(bytes(dir));
    d.game.file.store_to_c();
    // SAFETY: JE_loadHelpText may exit, which the guard catches.
    let c = unsafe { rustyrian_oracle::guard_call(|| b::JE_loadHelpText()) };
    let r = h::je_load_help_text(&mut d.game);
    (d, c, r)
}

/// Every string of the freeware `tyrian.hdt` decodes byte-identically (plan
/// verification step 3).
#[test]
fn je_load_help_text_matches_on_real_data() {
    let (d, c, r) = load_both(&data_dir());
    assert_eq!(c, None);
    assert_eq!(r, Ok(()));
    assert_eq!(diff::load::<State>(), d.game.helptext);
    assert_eq!(c_menu_names(), rust_menu_names(&d.game));
    assert_ne!(d.game.helptext, State::default());
    assert_eq!(d.c_calls(), d.rust_calls());
}

/// Every length-prefixed string in `tyrian.hdt`, including the headers and
/// trailers the loader skips and everything after the last block, decodes
/// byte-identically, up to end of file.
#[test]
fn read_encrypted_string_walks_whole_hdt() {
    let p = data_dir().join("tyrian.hdt");
    let (cp, cm) = (
        CString::new(bytes(&p)).expect("no NUL"),
        CString::new("rb").expect("no NUL"),
    );
    // SAFETY: valid C strings.
    let mut c = unsafe { b::ot_s_file_fileOpen(cp.as_ptr(), cm.as_ptr()) };
    let mut r = f::file_open(&bytes(&p), "rb");
    // SAFETY: an open File.
    let cu = unsafe { b::ot_i_file_fileReadU32(&raw mut c) };
    assert_eq!(cu, f::file_read_u32(&mut r));
    let mut strings = 0;
    while !c.error {
        let (mut cd, mut rd) = ([0u8; 256], [0u8; 256]);
        // SAFETY: `cd` holds 256 bytes.
        unsafe { b::readEncryptedString(&raw mut c, cd.as_mut_ptr().cast::<c_char>(), cd.len()) };
        h::read_encrypted_string(&mut r, Some(&mut rd));
        assert_eq!(cd, rd, "string {strings}");
        assert_eq!((c.errnum, c.error), (r.errnum, r.error), "string {strings}");
        strings += 1;
    }
    assert!(strings > 600, "{strings} strings");
    // SAFETY: from fileOpen.
    unsafe { b::fileClose(&raw mut c) };
}

#[test]
fn je_load_help_text_missing_file_matches() {
    let (d, c, r) = load_both(&tmp("missing"));
    assert_eq!(c, Some(1));
    assert_eq!(r, Err(Halt { code: 1 }));
    let (cc, rc) = (d.c_calls(), d.rust_calls());
    assert!(!cc.is_empty());
    assert_eq!(cc, rc);
}

#[test]
fn je_load_help_text_truncated_file_matches() {
    let full = std::fs::read(data_dir().join("tyrian.hdt")).expect("tyrian.hdt");
    // Only the first ~16 KiB are text; shorter files fail to read, longer
    // ones load (C decides which).
    let mut failed = 0;
    for len in [0, 3, 4, 100, 5000, 16000, full.len() / 2, full.len() - 1] {
        let dir = tmp(&format!("trunc{len}"));
        std::fs::write(dir.join("tyrian.hdt"), &full[..len]).expect("write");
        std::fs::write(dir.join("tyrian1.lvl"), b"").expect("write");
        let (d, c, r) = load_both(&dir);
        assert_eq!(c.is_some(), r.is_err(), "{len}");
        if let Some(code) = c {
            assert_eq!(r, Err(Halt { code }), "{len}");
            failed += 1;
        }
        assert_eq!(diff::load::<State>(), d.game.helptext, "{len}");
        assert_eq!(c_menu_names(), rust_menu_names(&d.game));
        assert_eq!(d.c_calls(), d.rust_calls(), "{len}");
    }
    assert!(failed >= 4, "short files must hit the read-error path");
}

proptest! {
    #[test]
    fn decrypt_string_matches(s in prop::collection::vec(any::<u8>(), 0..300)) {
        let mut c = s.clone();
        // SAFETY: `c` holds `len` bytes.
        unsafe { b::ot_s_helptext_decrypt_string(c.as_mut_ptr().cast::<c_char>(), c.len() as _) };
        let mut r = s;
        h::decrypt_string(&mut r);
        prop_assert_eq!(c, r);
    }

    /// Strings longer than, equal to and shorter than the destination; short
    /// files; skipped strings.
    #[test]
    fn read_encrypted_string_matches(
        content in prop::collection::vec(any::<u8>(), 0..80),
        sizes in prop::collection::vec(prop::option::of(0usize..40), 1..4),
    ) {
        let dir = tmp("res");
        let p = dir.join("s.bin");
        std::fs::write(&p, &content).expect("write");
        let (cp, cm) = (CString::new(bytes(&p)).expect("no NUL"), CString::new("rb").expect("no NUL"));
        // SAFETY: valid C strings.
        let mut c = unsafe { b::ot_s_file_fileOpen(cp.as_ptr(), cm.as_ptr()) };
        let mut r: File = f::file_open(&bytes(&p), "rb");
        for size in sizes {
            let (mut cd, mut rd) = (vec![0x55u8; size.unwrap_or(0)], vec![0x55u8; size.unwrap_or(0)]);
            let cdst = if size.is_some() { cd.as_mut_ptr().cast::<c_char>() } else { std::ptr::null_mut() };
            // SAFETY: `cd` holds `size` bytes (or NULL, 0).
            unsafe { b::readEncryptedString(&raw mut c, cdst, size.unwrap_or(0)) };
            h::read_encrypted_string(&mut r, size.map(|_| &mut rd[..]));
            prop_assert_eq!(cd, rd);
            prop_assert_eq!((c.errnum, c.error), (r.errnum, r.error));
        }
        // SAFETY: from fileOpen.
        unsafe { b::fileClose(&raw mut c) };
    }
}

#[test]
fn initial_state_matches_c() {
    let d = Diff::new();
    assert_eq!(diff::load::<State>(), d.game.helptext);
}

#[test]
fn mirror_round_trip() {
    let mut d = Diff::new();
    let s = &mut d.game.helptext;
    s.help_txt[38][230] = 1;
    s.p_name[0][0] = 2;
    s.misc_text[67][41] = 3;
    s.misc_text_b[4][10] = 4;
    s.key_name[7][17] = 5;
    s.menu_text[6][20] = 6;
    s.outputs[8][30] = 7;
    s.topic_name[5][20] = 8;
    s.main_menu_help[33][65] = 9;
    s.in_game_text[5][20] = 10;
    s.detail_level[5][12] = 11;
    s.game_speed_text[4][12] = 12;
    s.input_devices[2][12] = 13;
    s.network_text[3][21] = 14;
    s.difficulty_name_b[10][20] = 15;
    s.joy_button_names[4][20] = 16;
    s.super_ships[10][25] = 17;
    s.special_name[8][9] = 18;
    s.destruct_help[24][21] = 19;
    s.weapon_names[16][16] = 20;
    s.destruct_mode_name[4][12] = 21;
    s.ship_info[12][1][255] = 22;
    s.menu_int[14][10][17] = 23;
    d.game.helptext.store_to_c();
    assert_eq!(diff::load::<State>(), d.game.helptext);
}

/// Both engines with the surfaces of `init_video`, the main shape tables and
/// the real help text loaded.
fn drawing_setup() -> Diff {
    let (mut d, c, r) = load_both(&data_dir());
    assert_eq!((c, r), (None, Ok(())));
    let vs = &mut d.game.video;
    for (i, slot) in vs.screens.iter_mut().enumerate() {
        let mut surf = Surface::new_8bit(320, 200);
        surf.pixels
            .iter_mut()
            .enumerate()
            .for_each(|(k, p)| *p = (k * 13 + i) as u8);
        *slot = Some(surf);
    }
    vs.vga_screen = Some(ScreenId::VgaScreenSeg);
    vs.vga_screen_seg = Some(ScreenId::VgaScreenSeg);
    vs.vga_screen2 = Some(ScreenId::VgaScreen2);
    vs.game_screen = Some(ScreenId::GameScreen);
    d.game.video.store_to_c();
    let name = CString::new("tyrian.shp").expect("no NUL");
    // SAFETY: a valid C string.
    unsafe { b::JE_loadMainShapeTables(name.as_ptr()) };
    sprite::je_load_main_shape_tables(&mut d.game, "tyrian.shp").expect("shape tables");
    let _ = (d.c_calls(), d.rust_calls());
    d
}

fn check_surfaces(d: &mut Diff, label: &str) {
    assert!(
        diff::load::<video::State>().screens == d.game.video.screens,
        "{label}: surfaces differ"
    );
    assert_eq!(d.c_calls(), d.rust_calls(), "{label}");
}

/// Every help text with the box sizes upstream uses and many others.
#[test]
fn je_h_box_matches_on_every_help_text() {
    let mut d = drawing_setup();
    let s = ScreenId::VgaScreen2;
    for n in 1..=39u8 {
        for (k, (w, v, color, bright)) in [
            (60, 8, 12, 3),
            (40, 7, 5, 3),
            (44, 7, 5, 3),
            (55, 7, 5, 3),
            (1, 7, 1, 0),
            (10, 9, 15, 4),
            (25, 6, 3, 200),
            (0, 7, 5, 3),
            (255, 7, 5, 3),
        ]
        .into_iter()
        .enumerate()
        {
            let (x, y) = (5 + k as i32 * 7, 4 + n as i32 * 3);
            d.game.video.store_to_c();
            // SAFETY: a live surface and loaded help text.
            unsafe { b::JE_HBox(screen_to_c(Some(s)), x, y, n, w, v, color, bright) };
            h::je_h_box(&mut d.game, s, x, y, n, w, v, color, bright);
            check_surfaces(&mut d, &format!("help {n} width {w}"));
        }
    }
}

/// Rust only (C reads outside `helpTxt`): out-of-range help numbers draw
/// nothing (`D-UB-HELP-1`).
#[test]
fn je_h_box_ignores_out_of_range_numbers() {
    let mut d = drawing_setup();
    let before = d.game.video.clone();
    for n in [0u8, 40, 255] {
        h::je_h_box(&mut d.game, ScreenId::VgaScreenSeg, 10, 10, n, 60, 8, 12, 3);
    }
    assert_eq!(d.game.video, before);
}

/// A 258-byte C buffer holding `text` at offset 1, its first byte NUL (C
/// reads it for one-character messages).
fn buffer(text: &[u8]) -> Vec<u8> {
    let mut buf = vec![0u8; 258];
    buf[1..1 + text.len()].copy_from_slice(text);
    buf
}

fn help_box_both(d: &mut Diff, buf: &[u8], args: (i32, i32, u8, u8, u8, u8, u8)) {
    let (x, y, w, v, color, bright, shade) = args;
    let s = ScreenId::VgaScreenSeg;
    d.game.video.store_to_c();
    // SAFETY: a live surface; the message is NUL-terminated inside `buf`,
    // and every byte C scans lies inside `buf`.
    unsafe {
        b::JE_helpBox(
            screen_to_c(Some(s)),
            x,
            y,
            buf.as_ptr().add(1).cast(),
            w,
            v,
            color,
            bright,
            shade,
        )
    };
    h::je_help_box(&mut d.game, s, x, y, &buf[1..], w, v, color, bright, shade);
}

/// The real strings upstream passes besides help text (misc text), every
/// shade type.
#[test]
fn je_help_box_matches_on_misc_text() {
    let mut d = drawing_setup();
    let texts: Vec<Vec<u8>> = d
        .game
        .helptext
        .misc_text
        .iter()
        .map(|t| h_c_str(t).to_vec())
        .collect();
    for (k, t) in texts.iter().enumerate() {
        for shade in 0..5u8 {
            let buf = buffer(t);
            help_box_both(
                &mut d,
                &buf,
                (k as i32 % 100, 3 + shade as i32 * 30, 30, 7, 12, 1, shade),
            );
            check_surfaces(&mut d, &format!("misc {k} shade {shade}"));
        }
    }
}

fn h_c_str(s: &[u8]) -> &[u8] {
    s.iter().position(|&c| c == 0).map_or(s, |n| &s[..n])
}

proptest! {
    /// Random words (including one-character and empty messages) at any box
    /// width.
    #[test]
    fn je_help_box_matches_random(
        words in prop::collection::vec(prop::collection::vec(prop::sample::select(b"aZ09~! .-".to_vec()), 0..14), 0..20),
        w in any::<u8>(),
        v in 0u8..12,
        color in 0u8..16,
        bright in any::<u8>(),
        shade in 0u8..5,
        x in -20i32..300,
        y in -20i32..190,
    ) {
        let mut d = drawing_setup();
        let mut text: Vec<u8> = words.join(&b' ');
        text.truncate(200);
        let buf = buffer(&text);
        help_box_both(&mut d, &buf, (x, y, w, v, color, bright, shade));
        check_surfaces(&mut d, "random");
    }
}

/// Rust only (upstream never terminates on strings of 256 bytes or more):
/// such a message draws nothing (`D-UB-HELP-1`).
#[test]
fn je_help_box_ignores_overlong_messages() {
    let mut d = drawing_setup();
    let before = d.game.video.clone();
    let long = vec![b'a'; 300];
    h::je_help_box(
        &mut d.game,
        ScreenId::VgaScreenSeg,
        0,
        0,
        &long,
        30,
        7,
        12,
        1,
        1,
    );
    assert_eq!(d.game.video, before);
}
