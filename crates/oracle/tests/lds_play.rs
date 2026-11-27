//! Differential tests for `lds_play.c` / `lds_play.h`. The globals live in
//! `LdsState` inside `loudness::AudioState` (see `rustyrian_core::loudness`).

use rustyrian_core::lds_play::{LdsState, Position, State};
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::lds_play::{lds_from_c, lds_to_c};

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(lds_from_c(), LdsState::default());
    assert_eq!(diff::load::<State>(), State::default());
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let mut s = LdsState {
        positions: Some(vec![
            Position {
                patnum: 1,
                transpose: 2,
            };
            9
        ]),
        // Zero-length blocks are still non-NULL (`malloc(0)`).
        soundbank: Some(Vec::new()),
        patterns: Some(vec![0xffff, 0, 7]),
        tempo_now: 3,
        playing: true,
        ..LdsState::default()
    };
    s.channel[8].vibcount = 9;
    lds_to_c(&s);
    assert_eq!(lds_from_c(), s);
    // Storing again frees and replaces the blocks.
    s.patterns = None;
    lds_to_c(&s);
    assert_eq!(lds_from_c(), s);
}
