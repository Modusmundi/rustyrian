//! Differential tests for `demo.c` / `demo.h`.

use rustyrian_core::demo::State;
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
        play_demo: true,
        record_demo: true,
        stopped_demo: true,
        demo_num: 4,
        demo_keys: 0x1f,
        demo_keys_wait: 600,
        ..State::default()
    };
    s.demo_file.errnum = 2;
    s.demo_file.error = true;
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}
