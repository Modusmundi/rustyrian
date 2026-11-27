//! Differential tests for `lvllib.c` / `lvllib.h`.

use rustyrian_core::game::Halt;
use rustyrian_core::lvllib::{self as l, State};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;
use std::path::{Path, PathBuf};

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data")
        .canonicalize()
        .expect("data/ (run `cargo xtask fetch-data`)")
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir()
        .join(format!("rustyrian-lvllib-{}", std::process::id()))
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("scratch dir");
    d
}

/// Runs `analyzeLevel` in both engines on `filename` in `dir`, starting from
/// a non-default `lvlPos`, and compares state and platform calls.
fn analyze_both(dir: &Path, filename: &str) -> (Option<i32>, Result<(), Halt>, State) {
    use std::os::unix::ffi::OsStrExt;
    let mut d = Diff::new();
    d.game.file.custom_data_dir_path = Some(dir.as_os_str().as_bytes().to_vec());
    let mut level_filename = [0u8; 13];
    level_filename[..filename.len()].copy_from_slice(filename.as_bytes());
    d.game.lvllib = State {
        lvl_pos: std::array::from_fn(|i| i as i32 * -3),
        level_filename,
        lvl_num: 7,
    };
    d.store_to_c();
    // SAFETY: analyzeLevel may exit, which the guard catches.
    let c = unsafe { rustyrian_oracle::guard_call(|| b::analyzeLevel()) };
    let r = l::analyze_level(&mut d.game);
    assert_eq!(c.is_some(), r.is_err());
    if let Some(code) = c {
        assert_eq!(r, Err(Halt { code }));
    }
    assert_eq!(diff::load::<State>(), d.game.lvllib);
    assert_eq!(d.c_calls(), d.rust_calls());
    (c, r, d.game.lvllib.clone())
}

/// Every episode's level file is indexed identically.
#[test]
fn analyze_level_matches_on_every_episode() {
    for ep in 1..=4 {
        let (c, _, s) = analyze_both(&data_dir(), &format!("tyrian{ep}.lvl"));
        assert_eq!(c, None, "episode {ep}");
        assert!(s.lvl_num > 0, "episode {ep}");
    }
}

#[test]
fn analyze_level_missing_file_matches() {
    let (c, _, _) = analyze_both(&tmp("missing"), "tyrian1.lvl");
    assert_eq!(c, Some(1));
}

/// Short files (read error) and counts above the 42-entry table (clamped).
#[test]
fn analyze_level_crafted_files_match() {
    let full = std::fs::read(data_dir().join("tyrian1.lvl")).expect("tyrian1.lvl");
    let mut many = vec![0xFF, 0xFF];
    many.extend((0u32..60).flat_map(|i| (i * 1000).to_le_bytes()));
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("empty", Vec::new()),
        ("one", vec![3]),
        ("short", full[..20].to_vec()),
        ("zero", vec![0, 0, 1, 2]),
        ("many", many),
        (
            "big",
            [&[1u8, 0][..], &0xFFFF_FFF0u32.to_le_bytes()].concat(),
        ),
    ];
    let mut failed = 0;
    for (name, bytes) in cases {
        let dir = tmp(name);
        std::fs::write(dir.join("tyrian1.lvl"), bytes).expect("write");
        let (c, _, _) = analyze_both(&dir, "tyrian1.lvl");
        failed += usize::from(c.is_some());
    }
    assert_eq!(failed, 3);
}

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let mut s = State::default();
    for (i, p) in s.lvl_pos.iter_mut().enumerate() {
        *p = i as i32 * 1000 - 7;
    }
    s.level_filename[..9].copy_from_slice(b"tyrian1.l");
    s.level_filename[12] = 0xff;
    s.lvl_num = 42;
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}
