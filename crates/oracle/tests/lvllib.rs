//! Differential tests for `lvllib.c` / `lvllib.h`.

use rustyrian_core::lvllib::State;
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
    for (i, p) in s.lvl_pos.iter_mut().enumerate() {
        *p = i as i32 * 1000 - 7;
    }
    s.level_filename[..9].copy_from_slice(b"tyrian1.l");
    s.level_filename[12] = 0xff;
    s.lvl_num = 42;
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}
