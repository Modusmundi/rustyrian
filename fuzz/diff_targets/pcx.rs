//! Differential `pcx`: `JE_loadPCX("tshp2.pcx")` in C and Rust on the same
//! file (see `src/differential.rs`).
#![no_main]

use libfuzzer_sys::fuzz_target;
use rustyrian_fuzz::differential::check_game;
use rustyrian_oracle::bindings as b;

fuzz_target!(|data: &[u8]| {
    let dir = rustyrian_fuzz::data_with(&[("tshp2.pcx", data)]);
    let name = c"tshp2.pcx";
    check_game(
        None,
        |g| {
            rustyrian_fuzz::set_dirs(g, &dir);
            rustyrian_fuzz::with_screens(g);
        },
        // SAFETY: a valid C string; `exit` is caught by the guard.
        || unsafe { b::JE_loadPCX(name.as_ptr()) },
        |g| rustyrian_core::pcxload::je_load_pcx(g, "tshp2.pcx"),
    );
});
