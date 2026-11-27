//! Differential tests for `font.c` / `font.h` (no mutable globals).

use rustyrian_core::font::{
    ALIGN_CENTER, ALIGN_LEFT, ALIGN_RIGHT, FONT_LARGE, FONT_NORMAL, FONT_SMALL, State,
};
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
    let s = State::default();
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}

#[test]
fn enum_values_match_c() {
    assert_eq!(FONT_LARGE as u32, b::Font_FONT_LARGE);
    assert_eq!(FONT_NORMAL as u32, b::Font_FONT_NORMAL);
    assert_eq!(FONT_SMALL as u32, b::Font_FONT_SMALL);
    assert_eq!(ALIGN_LEFT as u32, b::FontAlignment_ALIGN_LEFT);
    assert_eq!(ALIGN_CENTER as u32, b::FontAlignment_ALIGN_CENTER);
    assert_eq!(ALIGN_RIGHT as u32, b::FontAlignment_ALIGN_RIGHT);
}
