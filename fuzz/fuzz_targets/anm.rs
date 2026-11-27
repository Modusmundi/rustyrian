//! `tyrend.anm`: `playAnim` as the ending plays it (from frame 0, speed 7)
//! on a full headless video setup. A key press scripted at 1.5 s of
//! virtual time ends animations that would play for minutes, as a player's
//! key press does.
#![no_main]

use libfuzzer_sys::fuzz_target;
use rustyrian_core::Game;
use rustyrian_harness::{FakeConfig, FakePlatform, InstrumentConfig, Instrumented, script};
use std::sync::LazyLock;

static SCRIPT: LazyLock<script::Script> = LazyLock::new(|| {
    script::parse("ots 1\n@1500 key-down SPACE\n@1600 key-up SPACE\n").expect("a valid script")
});

fuzz_target!(|data: &[u8]| {
    let mut g = rustyrian_fuzz::game_with_data_file("tyrend.anm", data);
    let platform = Instrumented::new(
        FakePlatform::new(FakeConfig::default()),
        InstrumentConfig {
            script: Some(SCRIPT.clone()),
            ..InstrumentConfig::default()
        },
    )
    .expect("a session without outputs cannot fail to open");
    g.platform = Box::new(platform);
    play(&mut g);
});

fn play(g: &mut Game) {
    if rustyrian_core::video::init_video(g).is_err() {
        return;
    }
    let _ = rustyrian_core::animlib::play_anim(g, "tyrend.anm", 0, 7);
}
