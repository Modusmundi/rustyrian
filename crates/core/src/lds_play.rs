// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `lds_play.c` / `lds_play.h` (Loudness sound system
//! player, adapted from AdPlug).
//!
//! Every global of `lds_play.c` is read or written by `lds_update`, which the
//! audio callback calls, so they all live in [`LdsState`], owned by
//! `loudness::AudioState` behind `Game::audio` (see `loudness.rs`). `State`
//! is empty.

use serde::{Deserialize, Serialize};

/// `SoundBank`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SoundBank {
    pub mod_misc: u8,
    pub mod_vol: u8,
    pub mod_ad: u8,
    pub mod_sr: u8,
    pub mod_wave: u8,
    pub car_misc: u8,
    pub car_vol: u8,
    pub car_ad: u8,
    pub car_sr: u8,
    pub car_wave: u8,
    pub feedback: u8,
    pub keyoff: u8,
    pub portamento: u8,
    pub glide: u8,
    pub finetune: u8,
    pub vibrato: u8,
    pub vibdelay: u8,
    pub mod_trem: u8,
    pub car_trem: u8,
    pub tremwait: u8,
    pub arpeggio: u8,
    pub arp_tab: [u8; 12],
    pub start: u16,
    pub size: u16,
    pub fms: u8,
    pub transp: u16,
    pub midinst: u8,
    pub midvelo: u8,
    pub midkey: u8,
    pub midtrans: u8,
    pub middum1: u8,
    pub middum2: u8,
}

/// `Channel::chancheat` (anonymous struct in C).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelChancheat {
    pub chandelay: u8,
    pub sound: u8,
    pub high: u16,
}

/// `Channel`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Channel {
    pub gototune: u16,
    pub lasttune: u16,
    pub packpos: u16,
    pub finetune: u8,
    pub glideto: u8,
    pub portspeed: u8,
    pub nextvol: u8,
    pub volmod: u8,
    pub volcar: u8,
    pub vibwait: u8,
    pub vibspeed: u8,
    pub vibrate: u8,
    pub trmstay: u8,
    pub trmwait: u8,
    pub trmspeed: u8,
    pub trmrate: u8,
    pub trmcount: u8,
    pub trcwait: u8,
    pub trcspeed: u8,
    pub trcrate: u8,
    pub trccount: u8,
    pub arp_size: u8,
    pub arp_speed: u8,
    pub keycount: u8,
    pub vibcount: u8,
    pub arp_pos: u8,
    pub arp_count: u8,
    pub packwait: u8,
    pub arp_tab: [u8; 12],
    pub chancheat: ChannelChancheat,
}

/// `Position`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub patnum: u16,
    pub transpose: u8,
}

/// `op_table`.
pub const OP_TABLE: [u8; 9] = [0x00, 0x01, 0x02, 0x08, 0x09, 0x0a, 0x10, 0x11, 0x12];

/// Note frequency table (16 notes / octave).
#[rustfmt::skip]
pub const FREQUENCY: [u16; (13 * 15) - 3] = [
    343, 344, 345, 347, 348, 349, 350, 352, 353, 354, 356, 357, 358, 359, 361, 362,
    363, 365, 366, 367, 369, 370, 371, 373, 374, 375, 377, 378, 379, 381, 382, 384,
    385, 386, 388, 389, 391, 392, 393, 395, 396, 398, 399, 401, 402, 403, 405, 406,
    408, 409, 411, 412, 414, 415, 417, 418, 420, 421, 423, 424, 426, 427, 429, 430,
    432, 434, 435, 437, 438, 440, 442, 443, 445, 446, 448, 450, 451, 453, 454, 456,
    458, 459, 461, 463, 464, 466, 468, 469, 471, 473, 475, 476, 478, 480, 481, 483,
    485, 487, 488, 490, 492, 494, 496, 497, 499, 501, 503, 505, 506, 508, 510, 512,
    514, 516, 518, 519, 521, 523, 525, 527, 529, 531, 533, 535, 537, 538, 540, 542,
    544, 546, 548, 550, 552, 554, 556, 558, 560, 562, 564, 566, 568, 571, 573, 575,
    577, 579, 581, 583, 585, 587, 589, 591, 594, 596, 598, 600, 602, 604, 607, 609,
    611, 613, 615, 618, 620, 622, 624, 627, 629, 631, 633, 636, 638, 640, 643, 645,
    647, 650, 652, 654, 657, 659, 662, 664, 666, 669, 671, 674, 676, 678, 681, 683,
];

/// Vibrato (sine) table.
#[rustfmt::skip]
pub const VIBTAB: [u8; 25 + (13 * 3)] = [
    0, 13, 25, 37, 50, 62, 74, 86, 98, 109, 120, 131, 142, 152, 162, 171,
    180, 189, 197, 205, 212, 219, 225, 231, 236, 240, 244, 247, 250, 252, 254, 255,
    255, 255, 254, 252, 250, 247, 244, 240, 236, 231, 225, 219, 212, 205, 197, 189,
    180, 171, 162, 152, 142, 131, 120, 109, 98, 86, 74, 62, 50, 37, 25, 13,
];

/// Tremolo (sine * sine) table.
#[rustfmt::skip]
pub const TREMTAB: [u8; 128] = [
    0, 0, 1, 1, 2, 4, 5, 7, 10, 12, 15, 18, 21, 25, 29, 33,
    37, 42, 47, 52, 57, 62, 67, 73, 79, 85, 90, 97, 103, 109, 115, 121,
    128, 134, 140, 146, 152, 158, 165, 170, 176, 182, 188, 193, 198, 203, 208, 213,
    218, 222, 226, 230, 234, 237, 240, 243, 245, 248, 250, 251, 253, 254, 254, 255,
    255, 255, 254, 254, 253, 251, 250, 248, 245, 243, 240, 237, 234, 230, 226, 222,
    218, 213, 208, 203, 198, 193, 188, 182, 176, 170, 165, 158, 152, 146, 140, 134,
    127, 121, 115, 109, 103, 97, 90, 85, 79, 73, 67, 62, 57, 52, 47, 42,
    37, 33, 29, 25, 21, 18, 15, 12, 10, 7, 5, 4, 2, 1, 1, 0,
];

pub const MAXSOUND: u16 = 0x3f;
pub const MAXPOS: u16 = 0xff;

/// Every global of `lds_play.c` (all file statics except `playing` and
/// `songlooped`). The heap blocks `soundbank`, `positions` and `patterns` are
/// `Some(contents)` with exactly the element count upstream mallocs
/// (`numpatch`, `9 * numposi`, remaining-bytes / 2); `None` is NULL.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LdsState {
    pub soundbank: Option<Vec<SoundBank>>,
    pub channel: [Channel; 9],
    pub positions: Option<Vec<Position>>,
    #[serde(with = "crate::serde_big")]
    pub fmchip: [u8; 0xff],
    pub jumping: u8,
    pub fadeonoff: u8,
    pub allvolume: u8,
    pub hardfade: u8,
    pub tempo_now: u8,
    pub pattplay: u8,
    pub tempo: u8,
    pub regbd: u8,
    pub chandelay: [u8; 9],
    pub mode: u8,
    pub pattlen: u8,
    pub posplay: u16,
    pub jumppos: u16,
    pub speed: u16,
    pub patterns: Option<Vec<u16>>,
    pub numpatch: u16,
    pub numposi: u16,
    pub mainvolume: u16,
    pub playing: bool,
    pub songlooped: bool,
}

impl Default for LdsState {
    fn default() -> Self {
        LdsState {
            soundbank: None,
            channel: [Channel::default(); 9],
            positions: None,
            fmchip: [0; 0xff],
            jumping: 0,
            fadeonoff: 0,
            allvolume: 0,
            hardfade: 0,
            tempo_now: 0,
            pattplay: 0,
            tempo: 0,
            regbd: 0,
            chandelay: [0; 9],
            mode: 0,
            pattlen: 0,
            posplay: 0,
            jumppos: 0,
            speed: 0,
            patterns: None,
            numpatch: 0,
            numposi: 0,
            mainvolume: 0,
            playing: false,
            songlooped: false,
        }
    }
}

/// Main-thread-only globals of `lds_play.c`: none (see the module docs).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {}
