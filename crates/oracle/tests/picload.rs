//! Differential tests for `picload.c` / `picload.h`.

use rustyrian_core::picload::State;
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
    let s = State {
        je_load_pic_first: false,
    };
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}
