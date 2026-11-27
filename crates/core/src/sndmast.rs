// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `sndmast.c` / `sndmast.h`.

use serde::{Deserialize, Serialize};

pub const SFX_COUNT: usize = 29;
pub const VOICE_COUNT: usize = 9;
pub const SOUND_COUNT: usize = SFX_COUNT + VOICE_COUNT;

pub const S_NONE: u8 = 0;
pub const S_WEAPON_1: u8 = 1;
pub const S_WEAPON_2: u8 = 2;
pub const S_ENEMY_HIT: u8 = 3;
pub const S_EXPLOSION_4: u8 = 4;
pub const S_WEAPON_5: u8 = 5;
pub const S_WEAPON_6: u8 = 6;
pub const S_WEAPON_7: u8 = 7;
pub const S_SELECT: u8 = 8;
pub const S_EXPLOSION_8: u8 = 8;
pub const S_EXPLOSION_9: u8 = 9;
pub const S_WEAPON_10: u8 = 10;
pub const S_EXPLOSION_11: u8 = 11;
pub const S_EXPLOSION_12: u8 = 12;
pub const S_WEAPON_13: u8 = 13;
pub const S_WEAPON_14: u8 = 14;
pub const S_WEAPON_15: u8 = 15;
pub const S_SPRING: u8 = 16;
pub const S_WARNING: u8 = 17;
pub const S_ITEM: u8 = 18;
pub const S_HULL_HIT: u8 = 19;
pub const S_MACHINE_GUN: u8 = 20;
pub const S_SOUL_OF_ZINGLON: u8 = 21;
pub const S_EXPLOSION_22: u8 = 22;
pub const S_CLINK: u8 = 23;
pub const S_CLICK: u8 = 24;
pub const S_WEAPON_25: u8 = 25;
pub const S_WEAPON_26: u8 = 26;
pub const S_SHIELD_HIT: u8 = 27;
pub const S_CURSOR: u8 = 28;
pub const S_POWERUP: u8 = 29;
/// "Cleared enemy platform."
pub const V_CLEARED_PLATFORM: u8 = 30;
/// "Large enemy approaching."
pub const V_BOSS: u8 = 31;
/// "Enemies ahead."
pub const V_ENEMIES: u8 = 32;
/// "Good luck."
pub const V_GOOD_LUCK: u8 = 33;
/// "Level completed."
pub const V_LEVEL_END: u8 = 34;
/// "Danger."
pub const V_DANGER: u8 = 35;
/// "Warning: spikes ahead."
pub const V_SPIKES: u8 = 36;
/// "Data acquired."
pub const V_DATA_CUBE: u8 = 37;
/// "Unexplained speed increase."
pub const V_ACCELERATE: u8 = 38;

const fn t(s: &[u8]) -> [u8; 9] {
    let mut out = [0u8; 9];
    let mut i = 0;
    while i < s.len() {
        out[i] = s[i];
        i += 1;
    }
    out
}

/// `soundTitle` `[1..soundnum + 9] of string [8]`.
pub const SOUND_TITLE: [[u8; 9]; SOUND_COUNT] = [
    t(b"SCALEDN2"),
    t(b"F2"),
    t(b"TEMP10"),
    t(b"EXPLSM"),
    t(b"PASS3"),
    t(b"TEMP2"),
    t(b"BYPASS1"),
    t(b"EXP1RT"),
    t(b"EXPLLOW"),
    t(b"TEMP13"),
    t(b"EXPRETAP"),
    t(b"MT2BOOM"),
    t(b"TEMP3"),
    t(b"LAZB"),
    t(b"LAZGUN2"),
    t(b"SPRING"),
    t(b"WARNING"),
    t(b"ITEM"),
    t(b"HIT2"),
    t(b"MACHNGUN"),
    t(b"HYPERD2"),
    t(b"EXPLHUG"),
    t(b"CLINK1"),
    t(b"CLICK"),
    t(b"SCALEDN1"),
    t(b"TEMP11"),
    t(b"TEMP16"),
    t(b"SMALL1"),
    t(b"POWERUP"),
    t(b"VOICE1"),
    t(b"VOICE2"),
    t(b"VOICE3"),
    t(b"VOICE4"),
    t(b"VOICE5"),
    t(b"VOICE6"),
    t(b"VOICE7"),
    t(b"VOICE8"),
    t(b"VOICE9"),
];

/// `windowTextSamples` `[1..9]`.
pub const WINDOW_TEXT_SAMPLES: [u8; 9] = [
    V_DANGER,
    V_BOSS,
    V_ENEMIES,
    V_CLEARED_PLATFORM,
    V_DANGER,
    V_SPIKES,
    V_ACCELERATE,
    V_DANGER,
    V_ENEMIES,
];

/// `sndmast.c` has no mutable globals.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {}
