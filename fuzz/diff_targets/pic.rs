//! Differential `pic`: `JE_loadPic` of every picture number, valid or not,
//! in C and Rust on the same `tyrian.pic` (see `src/differential.rs`).
#![no_main]

use libfuzzer_sys::fuzz_target;
use rustyrian_core::pcxmast::PCX_NUM;
use rustyrian_core::video::ScreenId;
use rustyrian_fuzz::differential::check_game;
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::mirror::video::screen_to_c;

const IDS: std::ops::RangeInclusive<u8> = 0..=PCX_NUM as u8 + 1;

fn screen(id: u8) -> ScreenId {
    ScreenId::ALL[usize::from(id) % ScreenId::ALL.len()]
}

fuzz_target!(|data: &[u8]| {
    let dir = rustyrian_fuzz::data_with(&[("tyrian.pic", data)]);
    check_game(
        None,
        |g| {
            rustyrian_fuzz::set_dirs(g, &dir);
            rustyrian_fuzz::with_screens(g);
        },
        || {
            for id in IDS {
                // SAFETY: a surface of C's; `exit` is caught by the guard.
                unsafe { b::JE_loadPic(screen_to_c(Some(screen(id))), id, false) };
            }
        },
        |g| {
            for id in IDS {
                rustyrian_core::picload::je_load_pic(g, screen(id), id, false)?;
            }
            Ok(())
        },
    );
});
