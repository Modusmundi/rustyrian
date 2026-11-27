//! `tyrian.hdt`: `JE_loadHelpText` on an arbitrary file (an error `Halt` is fine).
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut g = rustyrian_fuzz::game_with_data_file("tyrian.hdt", data);
    let _ = rustyrian_core::helptext::je_load_help_text(&mut g);
});
