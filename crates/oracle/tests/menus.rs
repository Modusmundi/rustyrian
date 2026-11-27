//! Differential tests for `menus.c` / `menus.h`.

use rustyrian_core::menus::State;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let mut s = State::default();
    for (i, row) in s.episode_name.iter_mut().enumerate() {
        row[..5].copy_from_slice(b"Ep #\0");
        row[3] = b'0' + i as u8;
        row[30] = 0xff;
    }
    for (i, row) in s.difficulty_name.iter_mut().enumerate() {
        row[0] = b'a' + i as u8;
        row[20] = 0x80;
    }
    for (i, row) in s.gameplay_name.iter_mut().enumerate() {
        row[0] = b'A' + i as u8;
        row[25] = 1;
    }
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}
