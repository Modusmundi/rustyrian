//! Differential tests for `picload.c` / `picload.h`.

use rustyrian_core::game::Halt;
use rustyrian_core::picload::{self as pic, State};
use rustyrian_core::sdl::{PIXELFORMAT_RGB888, Surface, alloc_format};
use rustyrian_core::video::{self, ScreenId};
use rustyrian_core::{palette, pcxmast};

use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;
use rustyrian_oracle::mirror::video::screen_to_c;
use std::path::{Path, PathBuf};

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let s = State {
        je_load_pic_first: false,
    };
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data")
        .canonicalize()
        .expect("data/ (run `cargo xtask fetch-data`)")
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir()
        .join(format!("rustyrian-picload-{}", std::process::id()))
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("scratch dir");
    d
}

/// Both engines after `init_video` (patterned surfaces, a texture format),
/// with the palettes of the real `palette.dat` and data directory `dir`.
fn setup(dir: &Path) -> Diff {
    use std::os::unix::ffi::OsStrExt;
    let mut d = Diff::new();
    let vs = &mut d.game.video;
    for (i, slot) in vs.screens.iter_mut().enumerate() {
        let mut s = Surface::new_8bit(320, 200);
        s.pixels
            .iter_mut()
            .enumerate()
            .for_each(|(k, p)| *p = (k * 7 + i) as u8);
        *slot = Some(s);
    }
    vs.vga_screen = Some(ScreenId::VgaScreenSeg);
    vs.vga_screen_seg = Some(ScreenId::VgaScreenSeg);
    vs.vga_screen2 = Some(ScreenId::VgaScreen2);
    vs.game_screen = Some(ScreenId::GameScreen);
    vs.main_window_tex_format = alloc_format(PIXELFORMAT_RGB888);
    d.game.file.custom_data_dir_path = Some(data_dir().as_os_str().as_bytes().to_vec());
    d.store_to_c();
    // SAFETY: the real palette.dat loads without exiting.
    unsafe { b::loadPals() };
    palette::load_pals(&mut d.game).expect("palette.dat");
    // A fresh data directory search for `dir`.
    d.game.file = diff::load();
    d.game.file.custom_data_dir_path = Some(dir.as_os_str().as_bytes().to_vec());
    d.game.file.data_dir_path = None;
    d.game.file.data_dir_path_len = 0;
    d.game.file.store_to_c();
    d
}

/// Loads picture `id` into `screen` in both engines and compares everything
/// the call touches.
fn check(
    d: &mut Diff,
    screen: ScreenId,
    id: u8,
    storepal: bool,
) -> (Option<i32>, Result<(), Halt>) {
    let s = screen_to_c(Some(screen));
    // SAFETY: a valid surface; exit is caught by the guard.
    let c = unsafe { rustyrian_oracle::guard_call(|| b::JE_loadPic(s, id, storepal)) };
    let r = pic::je_load_pic(&mut d.game, screen, id, storepal);
    let label = format!("picture {id} storepal {storepal}");
    assert_eq!(diff::load::<State>(), d.game.picload, "{label}");
    assert_eq!(diff::load::<pcxmast::State>(), d.game.pcxmast, "{label}");
    assert_eq!(diff::load::<palette::State>(), d.game.palette, "{label}");
    assert!(
        diff::load::<video::State>().screens == d.game.video.screens,
        "{label}: surfaces differ"
    );
    assert_eq!(d.c_calls(), d.rust_calls(), "{label}");
    (c, r)
}

/// Every picture of `tyrian.pic` (and invalid ids) into both screens, with
/// and without making its palette current.
#[test]
fn je_load_pic_matches_on_real_data() {
    let mut d = setup(&data_dir());
    for id in 0..=15u8 {
        for (screen, storepal) in [
            (ScreenId::VgaScreenSeg, false),
            (ScreenId::VgaScreen2, true),
        ] {
            assert_eq!(check(&mut d, screen, id, storepal), (None, Ok(())));
        }
    }
    assert_eq!(
        check(&mut d, ScreenId::GameScreen, 255, true),
        (None, Ok(()))
    );
    assert!(!d.game.picload.je_load_pic_first);
}

#[test]
fn je_load_pic_missing_file_matches() {
    let mut d = setup(&tmp("missing"));
    let (c, r) = check(&mut d, ScreenId::VgaScreenSeg, 1, false);
    assert_eq!(c, Some(1));
    assert_eq!(r, Err(Halt { code: 1 }));
}

/// Truncated files, a short header count, and positions past the end of
/// the file (one above the large-read threshold, `D-UB-SPRITE-1`).
#[test]
fn je_load_pic_damaged_files_match() {
    let full = std::fs::read(data_dir().join("tyrian.pic")).expect("tyrian.pic");
    let mut short = full.clone();
    short[..2].copy_from_slice(&5u16.to_le_bytes());
    let mut huge = full.clone();
    huge[2 + 4 * 4..2 + 5 * 4].copy_from_slice(&0x0200_0000u32.to_le_bytes());
    let mut descending = full.clone();
    descending[2 + 2 * 4..2 + 3 * 4].copy_from_slice(&3u32.to_le_bytes());
    let cases = [
        full[..0].to_vec(),
        full[..1].to_vec(),
        full[..20].to_vec(),
        full[..5000].to_vec(),
        full[..full.len() - 100].to_vec(),
        short,
        huge,
        descending,
    ];
    for (k, contents) in cases.iter().enumerate() {
        let dir = tmp(&format!("case{k}"));
        std::fs::write(dir.join("tyrian.pic"), contents).expect("write");
        let mut d = setup(&dir);
        for id in [1, 3, 4, 5, 6, 13] {
            assert_eq!(
                check(&mut d, ScreenId::VgaScreenSeg, id, true),
                (None, Ok(())),
                "case {k}"
            );
        }
    }
}
