//! `tyrian.hdt` item data: `JE_loadItemDat` for episodes 1-3 (the offset at
//! the start of the file, then the weapon, port, special, power, ship,
//! option, shield and enemy tables).
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut g = rustyrian_fuzz::game_with_data_file("tyrian.hdt", data);
    g.episodes.episode_num = 1;
    let _ = rustyrian_core::episodes::je_load_item_dat(&mut g);
});
