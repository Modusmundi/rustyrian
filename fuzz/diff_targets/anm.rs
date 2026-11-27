//! Differential `anm`: `init_video` then `playAnim("tyrend.anm", 0, 7)` in C
//! and Rust on the same file and input script (a key press at 1.5 s of
//! virtual time), comparing every platform call, presented frames included
//! (see `src/differential.rs`).
#![no_main]

use libfuzzer_sys::fuzz_target;
use rustyrian_fuzz::differential::{check_game, parse_script};
use rustyrian_harness::script::Script;
use rustyrian_oracle::bindings as b;
use std::sync::LazyLock;

static SCRIPT: LazyLock<Script> =
    LazyLock::new(|| parse_script("ots 1\n@1500 key-down SPACE\n@1600 key-up SPACE\n"));

fuzz_target!(|data: &[u8]| {
    let dir = rustyrian_fuzz::data_with(&[("tyrend.anm", data)]);
    let name = c"tyrend.anm";
    check_game(
        Some(&SCRIPT),
        |g| rustyrian_fuzz::set_dirs(g, &dir),
        // SAFETY: plain C calls with a valid C string; `exit` is caught by
        // the guard.
        || unsafe {
            b::init_video();
            b::playAnim(name.as_ptr(), 0, 7);
        },
        |g| {
            rustyrian_core::video::init_video(g)?;
            rustyrian_core::animlib::play_anim(g, "tyrend.anm", 0, 7)
        },
    );
});
