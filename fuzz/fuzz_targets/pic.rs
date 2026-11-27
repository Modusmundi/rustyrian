//! `tyrian.pic`: `JE_loadPic` of every picture number, valid or not (the
//! first call reads the offset table), each into a screen surface.
#![no_main]

use libfuzzer_sys::fuzz_target;
use rustyrian_core::pcxmast::PCX_NUM;
use rustyrian_core::video::ScreenId;

fuzz_target!(|data: &[u8]| {
    let mut g = rustyrian_fuzz::game_with_data_file("tyrian.pic", data);
    rustyrian_fuzz::with_screens(&mut g);
    for id in 0..=PCX_NUM as u8 + 1 {
        // `storepal` only copies a palette table to the display palette.
        let screen = ScreenId::ALL[usize::from(id) % ScreenId::ALL.len()];
        if rustyrian_core::picload::je_load_pic(&mut g, screen, id, false).is_err() {
            return;
        }
    }
});
