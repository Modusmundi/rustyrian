// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `musmast.c` / `musmast.h`.

use serde::{Deserialize, Serialize};

pub const DEFAULT_SONG_BUY: u8 = 2;
pub const SONG_LEVELEND: u8 = 9;
pub const SONG_GAMEOVER: u8 = 10;
pub const SONG_MAPVIEW: u8 = 19;
pub const SONG_ENDGAME1: u8 = 7;
pub const SONG_ZANAC: u8 = 31;
pub const SONG_TITLE: u8 = 29;

pub const MUSIC_NUM: usize = 41;

/// A NUL-padded `char[N]` initialised from a string literal.
const fn cstr<const N: usize>(s: &str) -> [u8; N] {
    let b = s.as_bytes();
    assert!(b.len() < N);
    let mut out = [0u8; N];
    let mut i = 0;
    while i < b.len() {
        out[i] = b[i];
        i += 1;
    }
    out
}

/// `musicTitle`: `const char[MUSIC_NUM][48]`.
pub const MUSIC_TITLE: [[u8; 48]; MUSIC_NUM] = [
    cstr("Asteroid Dance Part 2"),
    cstr("Asteroid Dance Part 1"),
    cstr("Buy/Sell Music"),
    cstr("CAMANIS"),
    cstr("CAMANISE"),
    cstr("Deli Shop Quartet"),
    cstr("Deli Shop Quartet No. 2"),
    cstr("Ending Number 1"),
    cstr("Ending Number 2"),
    cstr("End of Level"),
    cstr("Game Over Solo"),
    cstr("Gryphons of the West"),
    cstr("Somebody pick up the Gryphone"),
    cstr("Gyges, Will You Please Help Me?"),
    cstr("I speak Gygese"),
    cstr("Halloween Ramble"),
    cstr("Tunneling Trolls"),
    cstr("Tyrian, The Level"),
    cstr("The MusicMan"),
    cstr("The Navigator"),
    cstr("Come Back to Me, Savara"),
    cstr("Come Back again to Savara"),
    cstr("Space Journey 1"),
    cstr("Space Journey 2"),
    cstr("The final edge"),
    cstr("START5"),
    cstr("Parlance"),
    cstr("Torm - The Gathering"),
    cstr("TRANSON"),
    cstr("Tyrian: The Song"),
    cstr("ZANAC3"),
    cstr("ZANACS"),
    cstr("Return me to Savara"),
    cstr("High Score Table"),
    cstr("One Mustn't Fall"),
    cstr("Sarah's Song"),
    cstr("A Field for Mag"),
    cstr("Rock Garden"),
    cstr("Quest for Peace"),
    cstr("Composition in Q"),
    cstr("BEER"),
];

/// Mutable globals of this module (rule R3).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub song_buy: u8,
    pub music_fade: bool,
}
