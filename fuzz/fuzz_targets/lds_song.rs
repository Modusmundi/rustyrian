//! One song of `music.mus`: `lds_load` (with its load-time song check), then
//! playback as the audio callback drives it: `lds_update` ticks, each
//! followed by OPL samples.
#![no_main]

use libfuzzer_sys::fuzz_target;
use rustyrian_core::crand;
use rustyrian_core::lds_play::{self as l, LdsState};
use rustyrian_core::opl::{self, OplState};
use std::sync::Mutex;

/// The fake platform's output rate.
const RATE: i32 = 44_100;
/// About 7 s of music at 70 ticks per second.
const TICKS: usize = 512;
/// Fewer samples per tick than the callback renders, to keep runs short.
const SAMPLES_PER_TICK: usize = 32;

fuzz_target!(|data: &[u8]| {
    let (mut lds, mut opl) = (LdsState::default(), OplState::default());
    if !l::lds_load(&mut lds, &mut opl, RATE, data) {
        return;
    }
    let crand = Mutex::new(crand::State::default());
    let mut samples = [0i16; SAMPLES_PER_TICK];
    for _ in 0..TICKS {
        l::lds_update(&mut lds, &mut opl);
        opl::adlib_getsample(
            &mut opl,
            &crand,
            &mut samples,
            SAMPLES_PER_TICK as opl::Bits,
        );
    }
});
