//! Shared helpers for the fuzz targets. Every target's invariant: no panic and
//! no hang on arbitrary input (plan step 9).

// The differential targets link the C oracle (and through it SDL2), which
// only runs in Linux CI and the amd64 container; on macOS sdl2-compat cannot
// load SDL3 in cargo-fuzz builds and blocks on a dialog. Refuse to build.
#[cfg(all(feature = "differential", not(target_os = "linux")))]
compile_error!(
    "the `differential` feature (the diff_* fuzz targets) builds only on Linux; \
     run it in CI or the amd64 container (see fuzz/src/differential.rs)"
);

#[cfg(all(feature = "differential", target_os = "linux"))]
pub mod differential;

use rustyrian_core::Game;
use rustyrian_core::sdl::Surface;
use rustyrian_core::video::{self, ScreenId};
use rustyrian_core::{config, editship};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

static SCRATCH: LazyLock<PathBuf> = LazyLock::new(|| {
    let d = std::env::temp_dir().join(format!("rustyrian-fuzz-{}", std::process::id()));
    std::fs::create_dir_all(&d).expect("scratch dir");
    d
});

static REAL_DATA: LazyLock<PathBuf> = LazyLock::new(|| {
    let d = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data");
    match d.join("tyrian1.lvl").canonicalize() {
        Ok(p) => p.parent().expect("a file has a parent").to_path_buf(),
        Err(_) => {
            eprintln!("{}: run `cargo xtask fetch-data` first", d.display());
            std::process::exit(2);
        }
    }
});

static DATA_DIRS: LazyLock<Mutex<HashMap<String, PathBuf>>> = LazyLock::new(Default::default);

/// A per-process scratch directory.
pub fn scratch() -> &'static Path {
    &SCRATCH
}

/// The freeware data (`cargo xtask fetch-data`): every target that writes
/// one data file finds the others here, as the game would.
pub fn real_data() -> &'static Path {
    &REAL_DATA
}

/// A data directory holding links to every freeware data file except
/// `files`, which the caller writes (never through a link into `data/`).
fn data_dir_without(files: &[&str]) -> PathBuf {
    let key = files.join("+");
    let mut dirs = DATA_DIRS.lock().expect("unpoisoned");
    dirs.entry(key.clone())
        .or_insert_with(|| {
            let dir = scratch().join(format!("data-{key}"));
            std::fs::create_dir_all(&dir).expect("scratch data dir");
            let real = real_data();
            for entry in std::fs::read_dir(real).expect("data/ is readable") {
                let name = entry.expect("data/ entry").file_name();
                if files.iter().any(|f| name == *f) {
                    continue;
                }
                let link = dir.join(&name);
                if link.symlink_metadata().is_err() {
                    std::os::unix::fs::symlink(real.join(&name), &link).expect("data link");
                }
            }
            dir
        })
        .clone()
}

/// The scratch user directory.
fn user_dir() -> PathBuf {
    let d = scratch().join("user");
    std::fs::create_dir_all(&d).expect("scratch user dir");
    d
}

fn path_bytes(p: &Path) -> Vec<u8> {
    p.display().to_string().into_bytes()
}

/// Points `g` at data directory `data` and the scratch user directory.
pub fn set_dirs(g: &mut Game, data: &Path) {
    g.file.custom_data_dir_path = Some(path_bytes(data));
    let user = path_bytes(&user_dir());
    g.file.user_dir_path_len = user.len();
    g.file.user_dir_path = Some(user);
}

/// A data directory holding `files` with the given bytes in place of the
/// freeware ones, and the other freeware files.
pub fn data_with(files: &[(&str, &[u8])]) -> PathBuf {
    let names: Vec<&str> = files.iter().map(|(n, _)| *n).collect();
    let data = data_dir_without(&names);
    for (name, bytes) in files {
        std::fs::write(data.join(name), bytes).expect("write fuzz input");
    }
    data
}

/// Writes `file` into the scratch user directory.
pub fn user_file(file: &str, bytes: &[u8]) {
    std::fs::write(user_dir().join(file), bytes).expect("write fuzz input");
}

/// A fresh game on the fake platform, with the freeware data directory and
/// the scratch user directory.
pub fn game() -> Box<Game> {
    game_in(real_data())
}

fn game_in(data: &Path) -> Box<Game> {
    let (platform, _) = rustyrian_harness::test_session();
    let mut g = Game::new(Box::new(platform));
    set_dirs(&mut g, data);
    g
}

/// A fresh game (see [`game`]) on [`data_with`]`(files)`.
pub fn game_with_data_files(files: &[(&str, &[u8])]) -> Box<Game> {
    game_in(&data_with(files))
}

/// [`game_with_data_files`] for one file.
pub fn game_with_data_file(file: &str, bytes: &[u8]) -> Box<Game> {
    game_with_data_files(&[(file, bytes)])
}

/// A fresh game (see [`game`]) whose user directory holds `file`.
pub fn game_with_user_file(file: &str, bytes: &[u8]) -> Box<Game> {
    user_file(file, bytes);
    game()
}

/// The three 320×200 surfaces and screen pointers `init_video` sets up,
/// without its window, renderer and texture: enough for loaders and
/// blitters that never present.
pub fn with_screens(g: &mut Game) {
    let v = &mut g.video;
    v.screens =
        std::array::from_fn(|_| Some(Surface::new_8bit(video::VGA_WIDTH, video::VGA_HEIGHT)));
    v.vga_screen = Some(ScreenId::VgaScreenSeg);
    v.vga_screen_seg = Some(ScreenId::VgaScreenSeg);
    v.vga_screen2 = Some(ScreenId::VgaScreen2);
    v.game_screen = Some(ScreenId::GameScreen);
}

/// Writes `bytes` to a scratch file and returns its path.
pub fn scratch_file(name: &str, bytes: &[u8]) -> PathBuf {
    let p = scratch().join(name);
    std::fs::write(&p, bytes).expect("write fuzz input");
    p
}

/// `tyrian.sav` from its plaintext: inputs of at least the file's size get
/// their checksums and encryption (`encryptSaveData`), so the fuzzer reaches
/// the parser instead of the checksum test; shorter ones stay as they are.
pub fn encrypt_save(plain: &[u8]) -> Vec<u8> {
    let mut out = plain.to_vec();
    if let Some(body) = out.get_mut(..config::SAVE_FILE_SIZE + 4) {
        config::encrypt_save_data(body);
    }
    out
}

/// `newsh$.shp` from its plaintext: the last 154 bytes (the custom ship
/// table) get the checksums and encryption `decryptExtraShipsData` undoes.
pub fn encrypt_extra_ships(plain: &[u8]) -> Vec<u8> {
    let mut out = plain.to_vec();
    let n = std::mem::size_of::<editship::JeShipsType>();
    let Some(start) = out.len().checked_sub(n) else {
        return out;
    };
    let ships = &mut out[start..];
    let body_len = editship::EXTRA_SHIPS_FILE_SIZE;
    let body = &ships[..body_len];
    let sum = body.iter().fold(0u8, |y, &d| y.wrapping_add(d));
    let neg = body.iter().fold(0u8, |y, &d| y.wrapping_sub(d));
    let mul = body
        .iter()
        .fold(1u8, |y, &d| y.wrapping_mul(d).wrapping_add(1));
    let xor = body.iter().fold(0u8, |y, &d| y ^ d);
    ships[body_len..].copy_from_slice(&[sum, neg, mul, xor]);
    for i in 0..body_len {
        ships[i] ^= editship::EXTRA_CRYPT_KEY[(i + 1) % 10];
        if i > 0 {
            ships[i] ^= ships[i - 1];
        }
    }
    out
}

/// Little-endian `i16` at `b[i..i + 2]`, zero past the end.
pub fn i16_at(b: &[u8], i: usize) -> i16 {
    match b.get(i..i + 2) {
        Some(&[lo, hi]) => i16::from_le_bytes([lo, hi]),
        _ => 0,
    }
}
