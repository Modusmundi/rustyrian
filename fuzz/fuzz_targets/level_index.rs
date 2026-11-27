//! `tyrian4.lvl`'s index and item data: `JE_initEpisode(4)`, i.e.
//! `analyzeLevel` (the level offset table) then `JE_loadItemDat`, which reads
//! episode 4's items from the level file at its last level offset. (The
//! episode script `levelsN.dat` and the level maps are read by `JE_loadMap`.)
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut g = rustyrian_fuzz::game_with_data_file("tyrian4.lvl", data);
    let _ = rustyrian_core::episodes::je_init_episode(&mut g, 4);
});
