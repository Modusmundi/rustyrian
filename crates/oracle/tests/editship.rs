//! Differential tests for `editship.c` / `editship.h`.

use rustyrian_core::editship::State;
use rustyrian_core::sprite::Sprite2Array;
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
        extra_avail: true,
        extra_shapes: Sprite2Array {
            size: 5,
            data: Some(vec![1, 2, 3, 4, 5]),
        },
        ..State::default()
    };
    for (i, b) in s.extra_ships.iter_mut().enumerate() {
        *b = i as u8;
    }
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
    State::default().store_to_c();
    assert_eq!(diff::load::<State>(), State::default());
}
