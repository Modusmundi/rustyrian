//! Differential tests for `pcxmast.c` / `pcxmast.h`.

use rustyrian_core::pcxmast::{FACEPAL, PCXPAL, State};
use rustyrian_oracle::bindings as b;
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
    for (i, p) in s.pcxpos.iter_mut().enumerate() {
        *p = (i as i32 + 1) * 12345;
    }
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}

#[test]
fn tables_match_c() {
    // SAFETY: reading const C tables.
    unsafe {
        assert_eq!(PCXPAL, b::pcxpal);
        assert_eq!(FACEPAL, b::facepal);
    }
}
