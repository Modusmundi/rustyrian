//! Differential tests for `swar.h` (no mutable globals).

use rustyrian_core::swar::{SWAR8_LANES, State, Swar8};
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
    let s = State::default();
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}

#[test]
fn lanes_match_swar8_width() {
    assert_eq!(SWAR8_LANES, size_of::<Swar8>());
}
