//! `opentyrian.cfg` text: `config_parse` then `config_write` on arbitrary bytes.
#![no_main]

use libfuzzer_sys::fuzz_target;
use rustyrian_core::{config_file, file};

fuzz_target!(|data: &[u8]| {
    let input = rustyrian_fuzz::scratch_file("in.cfg", data);
    let mut f = file::file_open(input.display().to_string().as_bytes(), "rb");
    let mut config = config_file::Config::default();
    config_file::config_init(&mut config);
    if let Some(stream) = f.f.as_mut()
        && config_file::config_parse(&mut config, stream)
    {
        let out = rustyrian_fuzz::scratch().join("out.cfg");
        let mut w = file::file_open(out.display().to_string().as_bytes(), "wb");
        if let Some(ws) = w.f.as_mut() {
            config_file::config_write(&config, ws);
        }
        file::file_close(&mut w);
    }
    config_file::config_deinit(&mut config);
    file::file_close(&mut f);
});
