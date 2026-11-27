//! Differential `save`: `loadSaves` on the same `tyrian.sav` (plaintext,
//! encrypted here), then `JE_loadGame` of every used slot until one exits,
//! in C and Rust (see `src/differential.rs`).
#![no_main]

use libfuzzer_sys::fuzz_target;
use rustyrian_core::config::{self, SAVE_FILES_NUM};
use rustyrian_fuzz::differential::check_game;
use rustyrian_oracle::bindings as b;

fuzz_target!(|data: &[u8]| {
    rustyrian_fuzz::user_file("tyrian.sav", &rustyrian_fuzz::encrypt_save(data));
    let dir = rustyrian_fuzz::real_data();
    check_game(
        None,
        |g| rustyrian_fuzz::set_dirs(g, dir),
        || {
            // SAFETY: plain C calls and reads of C's save slots; `exit` is
            // caught by the guard.
            unsafe {
                b::loadSaves();
                for slot in 1..=SAVE_FILES_NUM as u8 {
                    let level = (&raw const b::saveFiles).read()[usize::from(slot) - 1].level;
                    if level != 0 {
                        b::JE_loadGame(slot);
                    }
                }
            }
        },
        |g| {
            config::load_saves(g);
            for slot in 1..=SAVE_FILES_NUM as u8 {
                if g.config.save_files[usize::from(slot) - 1].level != 0 {
                    config::je_load_game(g, slot)?;
                }
            }
            Ok(())
        },
    );
});
