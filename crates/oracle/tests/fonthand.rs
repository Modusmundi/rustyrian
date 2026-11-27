//! Differential tests for `fonthand.c` / `fonthand.h`.

use rustyrian_core::fonthand::{FONT_MAP, State};
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
    let mut s = State {
        text_glow_font: 3,
        text_glow_brightness: 200,
        level_warning_display: true,
        level_warning_lines: 9,
        warning_red: true,
        warning_sound_delay: 17,
        armor_ship_delay: 0xbeef,
        warning_col: 0x80,
        warning_col_change: -3,
        ..State::default()
    };
    for (i, l) in s.level_warning_text.iter_mut().enumerate() {
        l.iter_mut()
            .enumerate()
            .for_each(|(j, c)| *c = (i * 61 + j) as u8);
    }
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}

#[test]
fn font_map_matches_c() {
    // SAFETY: reading a const C table.
    assert_eq!(FONT_MAP, unsafe { b::fontMap });
}
