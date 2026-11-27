//! Differential tests for `musmast.c` / `musmast.h`.

use rustyrian_core::musmast::{MUSIC_NUM, MUSIC_TITLE, State};
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
    let s = State {
        song_buy: 31,
        music_fade: true,
    };
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}

#[test]
fn music_title_matches_c() {
    // SAFETY: reading a const C table.
    let c = unsafe { b::musicTitle };
    assert_eq!(c.len(), MUSIC_NUM);
    for (r, c) in MUSIC_TITLE.iter().zip(c.iter()) {
        assert_eq!(r, &c.map(|x| x as u8));
    }
}
