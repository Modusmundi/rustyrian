//! Differential tests for `nortsong.c` / `nortsong.h`.

use rustyrian_core::nortsong::{SampleBuf, State};
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
        frame_count_max: 2,
        tyr_music_volume: 255,
        fx_volume: 128,
        temp_volume: 64,
        frame_speed: 0x1234,
        frame_period: 4321,
        frame_count_end: 0xdead_beef,
        frame_count2_end: 7,
        ..State::default()
    };
    for i in 0..s.sound_samples.len() {
        if i % 4 == 3 {
            continue;
        }
        let n = i * 37;
        let v: Vec<i16> = (0..n)
            .map(|x| (x as i16).wrapping_mul(97).wrapping_sub(1000))
            .collect();
        s.sound_sample_count[i] = n;
        s.sound_samples[i] = Some(SampleBuf(v.into()));
    }
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}
