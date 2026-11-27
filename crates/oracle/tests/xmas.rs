//! Differential tests for `xmas.c` / `xmas.h`.

use rustyrian_core::xmas::State;
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
    let s = State { xmas: true };
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}
