//! Differential tests for `editship.c` / `editship.h`: `loadExtraShapes`
//! (and through it `decryptExtraShipsData`) on generated `newsh$.shp` files.

use proptest::prelude::*;
use rustyrian_core::editship::{self, EXTRA_CRYPT_KEY, EXTRA_SHIPS_FILE_SIZE, State};
use rustyrian_core::sprite::Sprite2Array;
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let mut s = State {
        extra_avail: true,
        extra_shapes: Sprite2Array {
            size: 5,
            data: Some(vec![1, 2, 3, 4, 5]),
        },
        ..State::default()
    };
    for (i, b) in s.extra_ships.iter_mut().enumerate() {
        *b = i as u8;
    }
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
    State::default().store_to_c();
    assert_eq!(diff::load::<State>(), State::default());
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir()
        .join(format!("rustyrian-editship-{}", std::process::id()))
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("scratch dir");
    d
}

/// The ship table as stored: `plain` (150 bytes) followed by its four
/// checksums, the first 150 bytes encrypted as `decryptExtraShipsData`
/// expects. `corrupt` (0-3) breaks one checksum.
fn encrypted_ships(plain: &[u8], corrupt: Option<usize>) -> Vec<u8> {
    let mut sums = [
        plain.iter().fold(0u8, |y, &d| y.wrapping_add(d)),
        plain.iter().fold(0u8, |y, &d| y.wrapping_sub(d)),
        plain
            .iter()
            .fold(1u8, |y, &d| y.wrapping_mul(d).wrapping_add(1)),
        plain.iter().fold(0u8, |y, &d| y ^ d),
    ];
    if let Some(i) = corrupt {
        sums[i] = sums[i].wrapping_add(1);
    }
    let mut e = vec![0u8; EXTRA_SHIPS_FILE_SIZE];
    for i in 0..EXTRA_SHIPS_FILE_SIZE {
        e[i] = plain[i] ^ EXTRA_CRYPT_KEY[(i + 1) % 10];
        if i > 0 {
            e[i] ^= e[i - 1];
        }
    }
    e.extend_from_slice(&sums);
    e
}

/// A sprite sheet every sprite of which ends inside it (`sprite2s_valid`).
fn any_valid_sheet() -> impl Strategy<Value = Vec<u8>> {
    let stream = prop::collection::vec((0u8..16, 0u8..15, any::<u8>()), 0..6);
    prop_oneof![
        prop::collection::vec(any::<u8>(), 0..2),
        prop::collection::vec(stream, 1..5).prop_map(|sprites| {
            let mut bodies = Vec::new();
            for cmds in &sprites {
                let mut s = Vec::new();
                for &(count, skip, px) in cmds {
                    // A command byte other than the 0x0f terminator, then its
                    // opaque pixels.
                    let skip = if count == 0 && skip == 0x0f { 0 } else { skip };
                    s.push((count << 4) | skip);
                    s.extend(std::iter::repeat_n(px, usize::from(count)));
                }
                s.push(0x0f);
                bodies.push(s);
            }
            let mut off = 2 * bodies.len();
            let mut sheet = Vec::new();
            for s in &bodies {
                sheet.extend_from_slice(&(off as u16).to_le_bytes());
                off += s.len();
            }
            for s in bodies {
                sheet.extend(s);
            }
            sheet
        }),
    ]
}

/// Runs `loadExtraShapes` on both engines with `file` (if any) as
/// `newsh$.shp` in the user directory; compares states and calls.
fn load_both(file: Option<&[u8]>) -> State {
    let dir = tmp("user");
    if let Some(f) = file {
        std::fs::write(dir.join("newsh$.shp"), f).expect("write");
    }
    let mut d = Diff::new();
    let p = dir.as_os_str().as_bytes().to_vec();
    d.game.file.user_dir_path_len = p.len();
    d.game.file.user_dir_path = Some(p);
    d.store_to_c();
    // SAFETY: plain C call.
    unsafe { b::loadExtraShapes() };
    editship::load_extra_shapes(&mut d.game);
    assert_eq!(diff::load::<State>(), d.game.editship);
    assert_eq!(d.c_calls(), d.rust_calls());
    d.game.editship.clone()
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

    /// Valid sheets with ship tables whose checksums match or one of which
    /// does not.
    #[test]
    fn load_extra_shapes_matches(
        sheet in any_valid_sheet(),
        plain in prop::collection::vec(any::<u8>(), EXTRA_SHIPS_FILE_SIZE),
        corrupt in prop::option::of(0usize..4),
    ) {
        let mut file = sheet.clone();
        file.extend(encrypted_ships(&plain, corrupt));
        let s = load_both(Some(&file));
        prop_assert_eq!(s.extra_avail, corrupt.is_none());
        prop_assert_eq!(s.extra_shapes.size, sheet.len());
        if corrupt.is_none() {
            prop_assert_eq!(&s.extra_ships[..EXTRA_SHIPS_FILE_SIZE], &plain[..]);
        }
    }

    /// Files shorter than the ship table are ignored.
    #[test]
    fn load_extra_shapes_short_files_match(len in 0usize..154, fill: u8) {
        let s = load_both(Some(&vec![fill; len]));
        prop_assert!(!s.extra_avail);
    }
}

/// No `newsh$.shp`: nothing is loaded.
#[test]
fn load_extra_shapes_missing_file_matches() {
    assert_eq!(load_both(None), State::default());
}

/// D-UB-EDITSHIP-1 (Rust only): a sheet whose sprites run past its end loads
/// but leaves the custom ships unavailable.
#[test]
fn load_extra_shapes_rejects_unsafe_sheet() {
    let dir = tmp("bad");
    let plain = vec![7u8; EXTRA_SHIPS_FILE_SIZE];
    // One sprite whose stream has no terminator inside the sheet.
    let mut file = vec![2, 0, 0x30, 1, 2, 3];
    file.extend(encrypted_ships(&plain, None));
    std::fs::write(dir.join("newsh$.shp"), &file).expect("write");
    let mut d = Diff::new();
    let p = dir.as_os_str().as_bytes().to_vec();
    d.game.file.user_dir_path_len = p.len();
    d.game.file.user_dir_path = Some(p);
    editship::load_extra_shapes(&mut d.game);
    assert!(!d.game.editship.extra_avail);
    assert_eq!(d.game.editship.extra_shapes.size, 6);
}

/// D-UB-EDITSHIP-1 (Rust only): a `newsh$.shp` whose length no allocation can
/// hold (on Linux a directory: ext4 reports LLONG_MAX as its SEEK_END length;
/// elsewhere a sparse file of 2^60 bytes, or 2^52 where the file system caps
/// file sizes lower, as APFS does) loads no sheet and leaves the custom ships
/// unavailable.
#[test]
fn load_extra_shapes_allocation_failure_is_a_read_error() {
    let dir = tmp("huge");
    let path = dir.join("newsh$.shp");
    if cfg!(target_os = "linux") {
        std::fs::create_dir(&path).expect("mkdir");
    } else {
        let f = std::fs::File::create(&path).expect("create");
        let sized = [1u64 << 60, 1 << 52]
            .into_iter()
            .any(|n| f.set_len(n).is_ok());
        assert!(sized, "the file system refuses a 2^52-byte sparse file");
    }
    let mut d = Diff::new();
    let p = dir.as_os_str().as_bytes().to_vec();
    d.game.file.user_dir_path_len = p.len();
    d.game.file.user_dir_path = Some(p);
    editship::load_extra_shapes(&mut d.game);
    assert!(!d.game.editship.extra_avail);
    assert_eq!(d.game.editship.extra_shapes.data, None);
}
