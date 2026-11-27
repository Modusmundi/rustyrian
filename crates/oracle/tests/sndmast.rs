//! Differential tests for `sndmast.c` / `sndmast.h` (const tables only; the
//! file has no mutable globals).

use rustyrian_core::sndmast::{SOUND_TITLE, State, WINDOW_TEXT_SAMPLES};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

#[test]
fn tables_match_c() {
    // SAFETY: reading const C tables.
    let (titles, window) = unsafe { (b::soundTitle, b::windowTextSamples) };
    for (r, c) in SOUND_TITLE.iter().zip(titles.iter()) {
        assert_eq!(r, &c.map(|x| x as u8));
    }
    assert_eq!(WINDOW_TEXT_SAMPLES, window);
}
