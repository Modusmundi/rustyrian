//! Differential `level_index`: `JE_initEpisode(4)` (`analyzeLevel`, then
//! episode 4's item data from the level file) in C and Rust on the same
//! `tyrian4.lvl` (see `src/differential.rs`).
#![no_main]

use libfuzzer_sys::fuzz_target;
use rustyrian_fuzz::differential::check_game;
use rustyrian_oracle::bindings as b;

fuzz_target!(|data: &[u8]| {
    let dir = rustyrian_fuzz::data_with(&[("tyrian4.lvl", data)]);
    check_game(
        None,
        |g| rustyrian_fuzz::set_dirs(g, &dir),
        // SAFETY: a plain C call; `exit` is caught by the guard.
        || unsafe { b::JE_initEpisode(4) },
        |g| rustyrian_core::episodes::je_init_episode(g, 4),
    );
});
