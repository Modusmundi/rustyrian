//! Differential `cfg_text`: `config_parse` then, when it succeeds,
//! `config_write` in C and Rust on the same `opentyrian.cfg` text; the parse
//! results and the written files must be equal (see `src/differential.rs`).
#![no_main]

use libfuzzer_sys::fuzz_target;
use rustyrian_core::{config_file, file};
use rustyrian_fuzz::differential::in_child;
use rustyrian_oracle::bindings as b;
use std::ffi::CString;
use std::path::Path;

/// The parse result and, after a successful parse, the written file.
type Outcome = (bool, Option<Vec<u8>>);

fn c_path(p: &Path) -> CString {
    CString::new(p.display().to_string()).expect("no NUL in scratch paths")
}

fn c_side(input: &Path, output: &Path) -> Outcome {
    let (input, output) = (c_path(input), c_path(output));
    let mut config = b::Config::default();
    // SAFETY: valid C strings, a zeroed config, streams from `fopen`.
    unsafe {
        b::config_init(&raw mut config);
        let f = libc::fopen(input.as_ptr(), c"rb".as_ptr());
        assert!(!f.is_null(), "the input file opens");
        let ok = b::config_parse(&raw mut config, f.cast());
        libc::fclose(f);
        let written = ok.then(|| {
            let w = libc::fopen(output.as_ptr(), c"wb".as_ptr());
            assert!(!w.is_null(), "the output file opens");
            b::config_write(&raw const config, w.cast());
            libc::fclose(w);
            std::fs::read(output.to_str().expect("UTF-8 path")).expect("written")
        });
        b::config_deinit(&raw mut config);
        (ok, written)
    }
}

fn rust_side(input: &Path, output: &Path) -> Outcome {
    let mut f = file::file_open(input.display().to_string().as_bytes(), "rb");
    let mut config = config_file::Config::default();
    config_file::config_init(&mut config);
    let stream = f.f.as_mut().expect("the input file opens");
    let ok = config_file::config_parse(&mut config, stream);
    file::file_close(&mut f);
    let written = ok.then(|| {
        let mut w = file::file_open(output.display().to_string().as_bytes(), "wb");
        config_file::config_write(&config, w.f.as_mut().expect("the output file opens"));
        file::file_close(&mut w);
        std::fs::read(output).expect("written")
    });
    config_file::config_deinit(&mut config);
    (ok, written)
}

fuzz_target!(|data: &[u8]| {
    let input = rustyrian_fuzz::scratch_file("in.cfg", data);
    let dir = rustyrian_fuzz::scratch();
    let Ok(c) = in_child(|| c_side(&input, &dir.join("c.cfg"))) else {
        return;
    };
    let r = rust_side(&input, &dir.join("r.cfg"));
    assert_eq!(c.0, r.0, "parse results differ (C, Rust)");
    if c.1 != r.1 {
        panic!(
            "written files differ:\n  C:    {:?}\n  Rust: {:?}",
            c.1.as_deref().map(String::from_utf8_lossy),
            r.1.as_deref().map(String::from_utf8_lossy)
        );
    }
});
