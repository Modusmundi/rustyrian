//! Shared setup for the `rustyrian-core` benches: a `Game` on the harness's
//! fake platform, the freeware data directory, and the C oracle's fixture
//! states (`cargo xtask fixtures`).

// Each bench target compiles this module and uses part of it.
#![allow(dead_code)]

use rustyrian_core::Game;
use rustyrian_core::game::Snapshot;
use rustyrian_core::sdl::Surface;
use rustyrian_core::video::ScreenId;
use rustyrian_harness::{FakeConfig, FakePlatform};
use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `data/` (run `cargo xtask fetch-data`).
pub fn data_dir() -> PathBuf {
    repo()
        .join("data")
        .canonicalize()
        .expect("data/ (run `cargo xtask fetch-data`)")
}

/// A freshly started game on the fake platform.
pub fn game() -> Box<Game> {
    Game::new(Box::new(FakePlatform::new(FakeConfig::default())))
}

/// A fresh game with the three `init_video` surfaces (filled with a fixed
/// pattern of every color) and `data/` as its data directory.
pub fn game_with_screens() -> Box<Game> {
    let mut g = game();
    let v = &mut g.video;
    for (i, slot) in v.screens.iter_mut().enumerate() {
        let mut s = Surface::new_8bit(320, 200);
        for (k, p) in s.pixels.iter_mut().enumerate() {
            *p = ((k as u32)
                .wrapping_mul(2_654_435_761)
                .wrapping_add(i as u32)
                >> 13) as u8;
        }
        *slot = Some(s);
    }
    v.vga_screen = Some(ScreenId::VgaScreenSeg);
    v.vga_screen_seg = Some(ScreenId::VgaScreenSeg);
    v.vga_screen2 = Some(ScreenId::VgaScreen2);
    v.game_screen = Some(ScreenId::GameScreen);
    g.file.custom_data_dir_path = Some(data_dir().to_string_lossy().into_owned().into_bytes());
    g
}

/// Mid-level frames of `demo-cycle` listed in `porting/fixtures.toml`.
pub const FIXTURE_FRAMES: [u64; 2] = [500, 2000];

/// A game in the C oracle's state at `frame` of `demo-cycle`, from
/// `target/fixtures/` (the format `rustyrian_oracle::diff::fixture` reads).
pub fn fixture_game(frame: u64) -> Box<Game> {
    let path = repo()
        .join("target/fixtures")
        .join(format!("demo-cycle@{frame}.postcard"));
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("{}: {e}; run `cargo xtask fixtures`", path.display()));
    let snap: Snapshot = postcard::from_bytes(&bytes)
        .unwrap_or_else(|e| panic!("{}: {e}; rerun `cargo xtask fixtures`", path.display()));
    let mut g = game();
    g.restore(snap);
    g
}

/// Every song of `data/music.mus`, split as `load_song` does.
pub fn songs() -> Vec<Vec<u8>> {
    let data = std::fs::read(data_dir().join("music.mus")).expect("data/music.mus");
    let count = usize::from(u16::from_le_bytes([data[0], data[1]]));
    let mut pos: Vec<usize> = (0..count)
        .map(|i| {
            let o = 2 + 4 * i;
            u32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]) as usize
        })
        .collect();
    pos.push(data.len());
    (0..count)
        .map(|i| data[pos[i]..pos[i + 1].max(pos[i])].to_vec())
        .collect()
}

/// Output rate of the audio benches (the platform's obtained rate).
pub const RATE: i32 = 44_100;

/// The LDS player and OPL chip with song `n` of `music.mus` loaded.
pub fn loaded_song(
    n: usize,
) -> (
    rustyrian_core::lds_play::LdsState,
    rustyrian_core::opl::OplState,
) {
    use rustyrian_core::lds_play::{self as l, LdsState};
    use rustyrian_core::opl::OplState;
    let (mut lds, mut opl) = (LdsState::default(), OplState::default());
    assert!(
        l::lds_load(&mut lds, &mut opl, RATE, &songs()[n]),
        "song {n} loads"
    );
    (lds, opl)
}
