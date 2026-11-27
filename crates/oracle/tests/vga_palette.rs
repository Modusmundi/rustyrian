//! Differential tests for `vga_palette.c` / `vga_palette.h`.

use rustyrian_core::palette::Color;
use rustyrian_core::vga_palette::State;
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
    for (i, c) in s.vga_palette.iter_mut().enumerate() {
        *c = Color {
            r: 255 - i as u8,
            g: i as u8,
            b: (i * 7) as u8,
            a: 1,
        };
    }
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}
