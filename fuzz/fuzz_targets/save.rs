//! `tyrian.sav`: `loadSaves` on an arbitrary save (given as plaintext and
//! encrypted here, so the checksums hold), then `JE_loadGame` of every slot
//! the load screen offers (a used one: level not 0) until one halts.
#![no_main]

use libfuzzer_sys::fuzz_target;
use rustyrian_core::config::{self, SAVE_FILES_NUM};

fuzz_target!(|data: &[u8]| {
    let file = rustyrian_fuzz::encrypt_save(data);
    let mut g = rustyrian_fuzz::game_with_user_file("tyrian.sav", &file);
    config::load_saves(&mut g);
    for slot in 1..=SAVE_FILES_NUM as u8 {
        if g.config.save_files[usize::from(slot) - 1].level != 0
            && config::je_load_game(&mut g, slot).is_err()
        {
            break;
        }
    }
});
