//! `demo.1`: `beginPlayDemo` (header, episode load) then `playDemoKeys` over
//! the whole recording.
#![no_main]

use libfuzzer_sys::fuzz_target;
use rustyrian_core::demo;

fuzz_target!(|data: &[u8]| {
    let mut g = rustyrian_fuzz::game_with_data_file("demo.1", data);
    if demo::begin_play_demo(&mut g).is_err() {
        return;
    }
    while let Ok(true) = demo::play_demo_keys(&mut g) {
        // A key held for `wait` more frames only repeats the same movement;
        // skip to its last frame to keep runs short.
        g.demo.demo_keys_wait = g.demo.demo_keys_wait.min(1);
    }
    demo::end_play_demo(&mut g);
});
