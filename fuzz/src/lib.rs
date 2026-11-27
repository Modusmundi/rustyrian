//! Shared helpers for the fuzz targets. Every target's invariant: no panic and
//! no hang on arbitrary input (plan step 9).

use rustyrian_core::Game;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// A per-process scratch directory.
pub fn scratch() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let d = std::env::temp_dir().join(format!("rustyrian-fuzz-{}", std::process::id()));
        std::fs::create_dir_all(d.join("data")).expect("scratch dir");
        d
    })
}

/// A fresh game on the fake platform whose data directory holds only
/// `file` with `bytes`.
pub fn game_with_data_file(file: &str, bytes: &[u8]) -> Box<Game> {
    let data = scratch().join("data");
    std::fs::write(data.join(file), bytes).expect("write fuzz input");
    let (platform, _) = rustyrian_harness::test_session();
    let mut g = Game::new(Box::new(platform));
    g.file.custom_data_dir_path = Some(data.display().to_string().into_bytes());
    g
}

/// Writes `bytes` to a scratch file and returns its path.
pub fn scratch_file(name: &str, bytes: &[u8]) -> PathBuf {
    let p = scratch().join(name);
    std::fs::write(&p, bytes).expect("write fuzz input");
    p
}
