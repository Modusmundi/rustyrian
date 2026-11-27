//! Differential tests for `pcxload.c` / `pcxload.h` (no mutable globals).

use rustyrian_core::game::Halt;
use rustyrian_core::palette;
use rustyrian_core::pcxload::{self as pcx, State};
use rustyrian_core::sdl::Surface;
use rustyrian_core::video::{self, ScreenId};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;
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

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data")
        .canonicalize()
        .expect("data/ (run `cargo xtask fetch-data`)")
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir()
        .join(format!("rustyrian-pcxload-{}", std::process::id()))
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("scratch dir");
    d
}

/// Loads `name` from `dir` into `VGAScreen` (`vga`) in both engines and
/// compares surfaces, `colors` and the log.
fn check(dir: &Path, name: &str, vga: ScreenId) -> (Option<i32>, Result<(), Halt>) {
    use std::os::unix::ffi::OsStrExt;
    let mut d = Diff::new();
    let vs = &mut d.game.video;
    for (i, slot) in vs.screens.iter_mut().enumerate() {
        let mut s = Surface::new_8bit(320, 200);
        s.pixels
            .iter_mut()
            .enumerate()
            .for_each(|(k, p)| *p = (k * 3 + i) as u8);
        *slot = Some(s);
    }
    vs.vga_screen = Some(vga);
    vs.vga_screen_seg = Some(ScreenId::VgaScreenSeg);
    vs.vga_screen2 = Some(ScreenId::VgaScreen2);
    vs.game_screen = Some(ScreenId::GameScreen);
    d.game.file.custom_data_dir_path = Some(dir.as_os_str().as_bytes().to_vec());
    d.store_to_c();
    let cname = CString::new(name).expect("no NUL");
    // SAFETY: a valid C string; exit is caught by the guard.
    let c = unsafe { rustyrian_oracle::guard_call(|| b::JE_loadPCX(cname.as_ptr())) };
    let r = pcx::je_load_pcx(&mut d.game, name);
    assert_eq!(diff::load::<palette::State>(), d.game.palette, "{name}");
    assert!(
        diff::load::<video::State>().screens == d.game.video.screens,
        "{name}: surfaces differ"
    );
    assert_eq!(d.c_calls(), d.rust_calls(), "{name}");
    (c, r)
}

/// Every PCX picture in the freeware data.
#[test]
fn je_load_pcx_matches_on_every_pcx() {
    let mut names: Vec<String> = std::fs::read_dir(data_dir())
        .expect("data/")
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| n.ends_with(".pcx"))
        .collect();
    names.sort();
    assert!(names.len() >= 8, "{names:?}");
    for (k, name) in names.iter().enumerate() {
        let vga = [
            ScreenId::VgaScreenSeg,
            ScreenId::VgaScreen2,
            ScreenId::GameScreen,
        ][k % 3];
        assert_eq!(check(&data_dir(), name, vga), (None, Ok(())), "{name}");
    }
}

/// Missing, too short (logged, nothing loaded), minimal and truncated files.
#[test]
fn je_load_pcx_other_files_match() {
    let (c, r) = check(&tmp("missing"), "tshp2.pcx", ScreenId::VgaScreenSeg);
    assert_eq!(c, Some(1));
    assert_eq!(r, Err(Halt { code: 1 }));

    let full = std::fs::read(data_dir().join("tshp2.pcx")).expect("tshp2.pcx");
    let mut runs = vec![0u8; 128];
    runs.extend((0..2000).map(|i| {
        if i % 2 == 0 {
            0xC0 | (i % 64) as u8
        } else {
            i as u8
        }
    }));
    runs.extend((0..768).map(|i| i as u8));
    for (k, contents) in [
        full[..0].to_vec(),
        full[..895].to_vec(),
        full[..896].to_vec(),
        full[..5000].to_vec(),
        full[..full.len() - 1].to_vec(),
        runs,
    ]
    .iter()
    .enumerate()
    {
        let dir = tmp(&format!("case{k}"));
        std::fs::write(dir.join("tshp2.pcx"), contents).expect("write");
        assert_eq!(
            check(&dir, "tshp2.pcx", ScreenId::VgaScreenSeg),
            (None, Ok(())),
            "case {k}"
        );
    }

    // A directory named like the file opens but cannot be read. Its length
    // (SEEK_END) is small on macOS but huge on ext4, where C's `malloc` fails
    // and C then crashes; there only Rust runs (`D-UB-PCX-1`).
    let dir = tmp("dir");
    std::fs::create_dir(dir.join("tshp2.pcx")).expect("mkdir");
    if seek_end(&dir.join("tshp2.pcx")) < 1 << 30 {
        let (c, r) = check(&dir, "tshp2.pcx", ScreenId::VgaScreenSeg);
        assert_eq!(c.map(|code| Halt { code }), r.err());
    } else {
        rust_only_allocation_failure(&dir);
    }
}

/// `lseek(fd, 0, SEEK_END)` of `p`, as `fileGetLength` measures it.
fn seek_end(p: &Path) -> u64 {
    use std::io::{Seek, SeekFrom};
    let mut f = std::fs::File::open(p).expect("open");
    f.seek(SeekFrom::End(0)).unwrap_or(0)
}

/// Rust only: `tshp2.pcx` in `dir` has a length no allocation can hold. The
/// load logs a read error (ENOMEM), leaves the screen blank and returns
/// (`D-UB-PCX-1`); upstream's `malloc` returns NULL there and C crashes.
fn rust_only_allocation_failure(dir: &Path) {
    use std::os::unix::ffi::OsStrExt;
    let mut d = Diff::new();
    let vs = &mut d.game.video;
    for slot in vs.screens.iter_mut() {
        let mut s = Surface::new_8bit(320, 200);
        s.pixels.fill(7);
        *slot = Some(s);
    }
    vs.vga_screen = Some(ScreenId::VgaScreen2);
    d.game.file.custom_data_dir_path = Some(dir.as_os_str().as_bytes().to_vec());
    let r = pcx::je_load_pcx(&mut d.game, "tshp2.pcx");
    assert_eq!(r, Ok(()));
    let calls = format!("{:?}", d.rust_calls());
    assert!(
        calls.contains("Failed to read from file 'tshp2.pcx': Cannot allocate memory"),
        "{calls}"
    );
    assert!(
        d.game
            .video
            .screen(ScreenId::VgaScreen2)
            .pixels
            .iter()
            .all(|&p| p == 0)
    );
}

/// A sparse `tshp2.pcx` larger than any address space (2^60 bytes, or 2^52
/// where the file system caps file sizes lower, as APFS does) exercises the
/// allocation guard on every platform.
#[test]
fn je_load_pcx_allocation_failure_is_a_read_error() {
    let dir = tmp("sparse");
    let f = std::fs::File::create(dir.join("tshp2.pcx")).expect("create");
    let sized = [1u64 << 60, 1 << 52]
        .into_iter()
        .any(|n| f.set_len(n).is_ok());
    assert!(sized, "the file system refuses a 2^52-byte sparse file");
    drop(f);
    rust_only_allocation_failure(&dir);
}
