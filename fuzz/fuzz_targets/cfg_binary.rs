//! `tyrian.cfg`: `loadConfiguration` on an arbitrary file in the user
//! directory (it also reads the absent `opentyrian.cfg`).
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut g = rustyrian_fuzz::game_with_user_file("tyrian.cfg", data);
    rustyrian_core::config::load_configuration(&mut g);
});
