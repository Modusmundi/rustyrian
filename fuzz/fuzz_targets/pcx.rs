//! `tshp2.pcx`: `JE_loadPCX` (palette from the end of the file, then the
//! run-length picture into `VGAScreen`).
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut g = rustyrian_fuzz::game_with_data_file("tshp2.pcx", data);
    rustyrian_fuzz::with_screens(&mut g);
    let _ = rustyrian_core::pcxload::je_load_pcx(&mut g, "tshp2.pcx");
});
