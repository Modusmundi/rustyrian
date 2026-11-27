//! `tyrian.snd` and `voices.snd`: `loadSndFile` with the input as both
//! files (the sound effects untrimmed, the voices trimmed), converted to the
//! fake audio device's format; then every sound number is played and mixed
//! by the audio callback.
#![no_main]

use libfuzzer_sys::fuzz_target;
use rustyrian_core::sndmast::SOUND_COUNT;
use rustyrian_core::{loudness, nortsong};
use std::sync::Arc;

/// Samples mixed after each sound starts.
const MIX: usize = 1024;

fuzz_target!(|data: &[u8]| {
    let mut g = rustyrian_fuzz::game_with_data_files(&[("tyrian.snd", data), ("voices.snd", data)]);
    if !loudness::init_audio(&mut g) || nortsong::load_snd_file(&mut g, false).is_err() {
        return;
    }
    let crand = Arc::clone(&g.crand);
    let mut samples = [0i16; MIX];
    for n in 0..=SOUND_COUNT as u8 + 1 {
        nortsong::je_play_sample_num(&mut g, n);
        loudness::audio_callback(&mut loudness::audio(&g), &crand, &mut samples);
    }
});
