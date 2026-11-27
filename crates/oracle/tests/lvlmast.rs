//! Differential tests for `lvlmast.c` / `lvlmast.h` (const table only; the
//! file has no mutable globals).

use rustyrian_core::lvlmast::{SHAPE_FILE, State};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

#[test]
fn shape_file_matches_c() {
    // SAFETY: reading a const C table.
    let c = unsafe { b::shapeFile };
    assert_eq!(SHAPE_FILE, c.map(|x| x as u8));
}
