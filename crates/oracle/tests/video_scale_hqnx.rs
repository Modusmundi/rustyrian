//! Differential tests for `video_scale_hqNx.c`.

use rustyrian_core::video_scale_hqnx::State;
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
        yuv1: 0x0012_3456,
        yuv2: -7,
    };
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}
