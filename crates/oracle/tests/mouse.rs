//! Differential tests for `mouse.c` / `mouse.h`.

use rustyrian_core::mouse::State;
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
        has_mouse: false,
        mouse_has_three_buttons: false,
        mouse_inactive: false,
        mouse_cursor: 3,
        mouse_grab_x: 301,
        mouse_grab_y: 187,
        ..State::default()
    };
    for (i, p) in s.mouse_grab_shape.iter_mut().enumerate() {
        *p = (i * 31) as u8;
    }
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}
