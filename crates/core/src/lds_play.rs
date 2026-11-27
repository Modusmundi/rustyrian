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

use crate::memreader::{MemReader, mem_read_u8_array, mem_read_u16_array, mem_reader_skip};
use crate::opl::{self, OplState};
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

/// `opl_init()`: `adlib_init(audioSampleRate)`.
fn opl_init(opl: &mut OplState, audio_sample_rate: i32) {
    opl::adlib_init(opl, audio_sample_rate as u32);
}

/// `opl_write(reg, val)`: `adlib_write(reg, val)`.
fn opl_write(opl: &mut OplState, reg: u8, val: u8) {
    opl::adlib_write(opl, opl::Bitu::from(reg), val);
}

/// `lds_load(data, size)`; `audio_sample_rate` is `loudness.c`'s
/// `audioSampleRate`, read by `opl_init` in `lds_rewind`.
///
/// Song validation per divergence `D-UB-LDS-1` ([`lds_song_is_safe`]): a
/// rejected song is loaded as upstream does, then left stopped
/// (`playing = false`) and reported as a failed load.
pub fn lds_load(
    lds: &mut LdsState,
    opl: &mut OplState,
    audio_sample_rate: i32,
    data: &[u8],
) -> bool {
    let mut reader = MemReader::new(data);

    // header
    let mut b1 = [0u8; 1];
    mem_read_u8_array(&mut reader, &mut b1);
    lds.mode = b1[0];
    if lds.mode > 2 {
        return false;
    }
    let mut w1 = [0u16; 1];
    mem_read_u16_array(&mut reader, &mut w1);
    lds.speed = w1[0];
    mem_read_u8_array(&mut reader, &mut b1);
    lds.tempo = b1[0];
    mem_read_u8_array(&mut reader, &mut b1);
    lds.pattlen = b1[0];
    mem_read_u8_array(&mut reader, &mut lds.chandelay);
    mem_read_u8_array(&mut reader, &mut b1);
    lds.regbd = b1[0];

    // patches
    mem_read_u16_array(&mut reader, &mut w1);
    lds.numpatch = w1[0];

    let mut soundbank = vec![SoundBank::default(); usize::from(lds.numpatch)];
    for sb in soundbank.iter_mut() {
        let u8f = |r: &mut MemReader<'_>| {
            let mut b = [0u8; 1];
            mem_read_u8_array(r, &mut b);
            b[0]
        };
        let u16f = |r: &mut MemReader<'_>| {
            let mut w = [0u16; 1];
            mem_read_u16_array(r, &mut w);
            w[0]
        };
        sb.mod_misc = u8f(&mut reader);
        sb.mod_vol = u8f(&mut reader);
        sb.mod_ad = u8f(&mut reader);
        sb.mod_sr = u8f(&mut reader);
        sb.mod_wave = u8f(&mut reader);
        sb.car_misc = u8f(&mut reader);
        sb.car_vol = u8f(&mut reader);
        sb.car_ad = u8f(&mut reader);
        sb.car_sr = u8f(&mut reader);
        sb.car_wave = u8f(&mut reader);
        sb.feedback = u8f(&mut reader);
        sb.keyoff = u8f(&mut reader);
        sb.portamento = u8f(&mut reader);
        sb.glide = u8f(&mut reader);
        sb.finetune = u8f(&mut reader);
        sb.vibrato = u8f(&mut reader);
        sb.vibdelay = u8f(&mut reader);
        sb.mod_trem = u8f(&mut reader);
        sb.car_trem = u8f(&mut reader);
        sb.tremwait = u8f(&mut reader);
        sb.arpeggio = u8f(&mut reader);
        mem_read_u8_array(&mut reader, &mut sb.arp_tab);
        sb.start = u16f(&mut reader);
        sb.size = u16f(&mut reader);
        sb.fms = u8f(&mut reader);
        sb.transp = u16f(&mut reader);
        sb.midinst = u8f(&mut reader);
        sb.midvelo = u8f(&mut reader);
        sb.midkey = u8f(&mut reader);
        sb.midtrans = u8f(&mut reader);
        sb.middum1 = u8f(&mut reader);
        sb.middum2 = u8f(&mut reader);
    }
    lds.soundbank = Some(soundbank);

    // positions
    mem_read_u16_array(&mut reader, &mut w1);
    lds.numposi = w1[0];

    let mut positions = vec![Position::default(); 9 * usize::from(lds.numposi)];
    for p in positions.iter_mut() {
        // `patnum` is a byte offset into the pattern space; halve it into an
        // index of 16-bit words.
        mem_read_u16_array(&mut reader, &mut w1);
        p.patnum = w1[0];
        mem_read_u8_array(&mut reader, &mut b1);
        p.transpose = b1[0];
        p.patnum /= 2;
    }
    lds.positions = Some(positions);

    // patterns
    mem_reader_skip(&mut reader, 2); // number of digital sounds (unused)

    let numpatterns = reader.size() / 2;

    let mut patterns = vec![0u16; numpatterns];
    mem_read_u16_array(&mut reader, &mut patterns);
    lds.patterns = Some(patterns);

    // Upstream asserts `reader.size == 0 && !reader.error` here; it tests
    // the song data, so (R10, never panic on external input) it is not a
    // debug assertion in the port, matching the NDEBUG build.

    lds_rewind(lds, opl, audio_sample_rate);

    if !lds_song_is_safe(lds) {
        lds.playing = false;
        return false;
    }

    true
}

/// The load-time song check of divergence `D-UB-LDS-1`. It follows the
/// song's row sequence from position 0, which depends only on the pattern
/// data, visiting each reachable position once.
pub fn lds_song_is_safe(lds: &LdsState) -> bool {
    let (Some(positions), Some(patterns), Some(soundbank)) =
        (&lds.positions, &lds.patterns, &lds.soundbank)
    else {
        return false;
    };

    let mut chan_finetunes = [false; 256];
    chan_finetunes[0] = true;
    for &w in patterns.iter().filter(|&&w| w >> 8 == 0xf5) {
        chan_finetunes[usize::from(w as u8)] = true;
    }

    let note_ok = |sound: u8, high: u16| -> bool {
        let i = instrument(soundbank, i32::from(sound));
        if usize::from(i.arpeggio & 15) > i.arp_tab.len() {
            return false;
        }
        (0..256).filter(|&f| chan_finetunes[f]).all(|f| {
            // The tune lds_playsound computes with channel fine-tune `f`.
            let mut tunehigh = i32::from(high);
            tunehigh += ((i32::from(i.finetune) + f as i32 + 0x80) & 0xff) - 0x80;
            if i.arpeggio == 0 {
                let arpcalc = u16::from(i.arp_tab[0]) << 4;
                if arpcalc > 0x800 {
                    tunehigh = tunehigh - i32::from(arpcalc ^ 0xff0) - 16;
                } else {
                    tunehigh += i32::from(arpcalc);
                }
            }
            tunehigh >= 0
        })
    };

    let mut visited = [false; MAXPOS as usize + 1];
    let mut posplay: u16 = 0;
    loop {
        if visited[usize::from(posplay)] {
            return true;
        }
        visited[usize::from(posplay)] = true;

        let mut packpos = [0u16; 9];
        let mut packwait = [0u8; 9];
        let mut pattplay: u8 = 0;
        let next = loop {
            let mut vbreak = false;
            let mut jumppos = 0u16;
            let mut stopped = false;
            for chan in 0..9 {
                if packwait[chan] != 0 {
                    packwait[chan] -= 1;
                    continue;
                }
                let Some(p) = positions.get(usize::from(posplay) * 9 + chan) else {
                    return false;
                };
                let Some(&comword) =
                    patterns.get(usize::from(p.patnum) + usize::from(packpos[chan]))
                else {
                    return false;
                };
                let comhi = (comword >> 8) as u8;
                let comlo = comword as u8;
                if comword != 0 {
                    match comhi {
                        0x80 => packwait[chan] = comlo,
                        0xfc => stopped = true,
                        0xfa => {
                            vbreak = true;
                            jumppos = (posplay + 1) & MAXPOS;
                        }
                        0xf9 => {
                            vbreak = true;
                            jumppos = u16::from(comlo) & MAXPOS;
                        }
                        0x81..=0xff => {}
                        _ => {
                            let (sound, high) = note_of(comhi, comlo, p.transpose);
                            if !note_ok(sound, high) {
                                return false;
                            }
                        }
                    }
                }
                packpos[chan] = packpos[chan].wrapping_add(1);
            }
            if stopped {
                return true;
            }
            pattplay = pattplay.wrapping_add(1);
            if vbreak {
                break jumppos;
            }
            if pattplay >= lds.pattlen {
                break (posplay + 1) & MAXPOS;
            }
        };
        posplay = next;
    }
}

/// The instrument and tune of a note command (`comhi < 0x80`) under the
/// position's `transpose`.
fn note_of(comhi: u8, comlo: u8, transpose: u8) -> (u8, u16) {
    // The original player sign-extended bit 6 of the transpose byte.
    let mut transp = (transpose & 127) as i8;
    if (transpose & 64) != 0 {
        transp = (transp as u8 | 128) as i8;
    }

    if (transpose & 128) != 0 {
        (
            ((i32::from(comlo) + i32::from(transp)) & i32::from(MAXSOUND)) as u8,
            u16::from(comhi) << 4,
        )
    } else {
        (
            comlo & MAXSOUND as u8,
            ((i32::from(comhi) + i32::from(transp)) << 4) as u16,
        )
    }
}

pub fn lds_free(lds: &mut LdsState) {
    lds.soundbank = None;
    lds.positions = None;
    lds.patterns = None;
}

/// `lds_rewind()`; `audio_sample_rate` is read by `opl_init`.
pub fn lds_rewind(lds: &mut LdsState, opl: &mut OplState, audio_sample_rate: i32) {
    lds.tempo_now = 3;
    lds.playing = true;
    lds.songlooped = false;
    lds.jumping = 0;
    lds.fadeonoff = 0;
    lds.allvolume = 0;
    lds.hardfade = 0;
    lds.pattplay = 0;
    lds.posplay = 0;
    lds.jumppos = 0;
    lds.mainvolume = 0;
    lds.channel = [Channel::default(); 9];
    lds.fmchip = [0; 0xff];

    // OPL2 init
    opl_init(opl, audio_sample_rate);
    opl_write(opl, 1, 0x20);
    opl_write(opl, 8, 0);
    opl_write(opl, 0xbd, lds.regbd);

    for i in 0..9u8 {
        let o = OP_TABLE[usize::from(i)];
        opl_write(opl, 0x20 + o, 0);
        opl_write(opl, 0x23 + o, 0);
        opl_write(opl, 0x40 + o, 0x3f);
        opl_write(opl, 0x43 + o, 0x3f);
        opl_write(opl, 0x60 + o, 0xff);
        opl_write(opl, 0x63 + o, 0xff);
        opl_write(opl, 0x80 + o, 0xff);
        opl_write(opl, 0x83 + o, 0xff);
        opl_write(opl, 0xe0 + o, 0);
        opl_write(opl, 0xe3 + o, 0);
        opl_write(opl, 0xa0 + i, 0);
        opl_write(opl, 0xb0 + i, 0);
        opl_write(opl, 0xc0 + i, 0);
    }
}

pub fn lds_fade(lds: &mut LdsState, speed: u8) {
    lds.fadeonoff = speed;
}

pub fn lds_setregs(lds: &mut LdsState, opl: &mut OplState, reg: u8, val: u8) {
    if lds.fmchip[usize::from(reg)] == val {
        return;
    }

    lds.fmchip[usize::from(reg)] = val;
    opl_write(opl, reg, val);
}

pub fn lds_setregs_adv(lds: &mut LdsState, opl: &mut OplState, reg: u8, mask: u8, val: u8) {
    let v = (lds.fmchip[usize::from(reg)] & mask) | val;
    lds_setregs(lds, opl, reg, v);
}

/// `frequency[t % (12 * 16)]` and `t / (12 * 16) - 1` for a `Uint16` tune.
fn freq_octave(t: u16) -> (u16, u16) {
    (
        FREQUENCY[usize::from(t % (12 * 16))],
        (i32::from(t) / (12 * 16) - 1) as u16,
    )
}

/// Writes a channel's frequency registers from `tune`.
fn set_tune(lds: &mut LdsState, opl: &mut OplState, chan: u16, tune: u16) {
    let (freq, octave) = freq_octave(tune);
    lds_setregs(lds, opl, (0xa0 + chan) as u8, (freq & 0xff) as u8);
    lds_setregs_adv(
        lds,
        opl,
        (0xb0 + chan) as u8,
        0x20,
        (((i32::from(octave) << 2) + i32::from(freq >> 8)) & 0xdf) as u8,
    );
}

/// `tune - (arpreg ^ 0xff0) - 16` (negative arpeggio) or `tune + arpreg`.
fn apply_arp(tune: u16, arpreg: u16) -> u16 {
    if arpreg >= 0x800 {
        (i32::from(tune) - i32::from(arpreg ^ 0xff0) - 16) as u16
    } else {
        tune.wrapping_add(arpreg)
    }
}

pub fn lds_update(lds: &mut LdsState, opl: &mut OplState) -> i32 {
    if !lds.playing {
        return 0;
    }

    // fading
    if lds.fadeonoff != 0 {
        if lds.fadeonoff <= 128 {
            if lds.allvolume > lds.fadeonoff || lds.allvolume == 0 {
                lds.allvolume = lds.allvolume.wrapping_sub(lds.fadeonoff);
            } else {
                lds.allvolume = 1;
                lds.fadeonoff = 0;
                if lds.hardfade != 0 {
                    lds.playing = false;
                    lds.hardfade = 0;
                    for c in lds.channel.iter_mut() {
                        c.keycount = 1;
                    }
                }
            }
        } else if u16::from(lds.allvolume.wrapping_sub(lds.fadeonoff)) <= lds.mainvolume {
            lds.allvolume = lds.allvolume.wrapping_sub(lds.fadeonoff);
        } else {
            lds.allvolume = lds.mainvolume as u8;
            lds.fadeonoff = 0;
        }
    }

    // channel delay
    for chan in 0..9u16 {
        let c = &mut lds.channel[usize::from(chan)];
        if c.chancheat.chandelay != 0 {
            c.chancheat.chandelay -= 1;
            if c.chancheat.chandelay == 0 {
                let (sound, high) = (c.chancheat.sound, c.chancheat.high);
                lds_playsound(lds, opl, i32::from(sound), i32::from(chan), i32::from(high));
            }
        }
    }

    // notes
    if lds.tempo_now == 0 && lds.positions.is_some() {
        let mut vbreak = false;
        for chan in 0..9u16 {
            let ch = usize::from(chan);
            if lds.channel[ch].packwait == 0 {
                let pos = lds.positions.as_ref().expect("checked above")
                    [usize::from(lds.posplay) * 9 + ch];
                let (patnum, transpose) = (pos.patnum, pos.transpose);

                let comword = lds.patterns.as_ref().expect("loaded with positions")
                    [usize::from(patnum) + usize::from(lds.channel[ch].packpos)];
                let comhi = (comword >> 8) as u8;
                let comlo = (comword & 0xff) as u8;
                if comword != 0 {
                    if comhi == 0x80 {
                        lds.channel[ch].packwait = comlo;
                    } else if comhi >= 0x80 {
                        let fm_am = (lds.fmchip[0xc0 + ch] & 1) != 0;
                        let c = &mut lds.channel[ch];
                        match comhi {
                            0xff => {
                                c.volcar = ((((c.volcar & 0x3f) as u32 * u32::from(comlo)) >> 6)
                                    & 0x3f) as u8;
                                if fm_am {
                                    c.volmod =
                                        ((((c.volmod & 0x3f) as u32 * u32::from(comlo)) >> 6)
                                            & 0x3f) as u8;
                                }
                            }
                            0xfe => lds.tempo = (comword & 0x3f) as u8,
                            0xfd => c.nextvol = comlo,
                            // The original player also keys everything off here.
                            0xfc => lds.playing = false,
                            0xfb => c.keycount = 1,
                            0xfa => {
                                vbreak = true;
                                lds.jumppos = (lds.posplay + 1) & MAXPOS;
                            }
                            0xf9 => {
                                vbreak = true;
                                lds.jumppos = u16::from(comlo) & MAXPOS;
                                lds.jumping = 1;
                                if lds.jumppos < lds.posplay {
                                    lds.songlooped = true;
                                }
                            }
                            0xf8 => c.lasttune = 0,
                            0xf7 => {
                                c.vibwait = 0;
                                c.vibspeed = (comlo >> 4) + 2;
                                c.vibrate = (comlo & 15) + 1;
                            }
                            0xf6 => c.glideto = comlo,
                            0xf5 => c.finetune = comlo,
                            0xf4 => {
                                if lds.hardfade == 0 {
                                    lds.mainvolume = u16::from(comlo);
                                    lds.allvolume = comlo;
                                    lds.fadeonoff = 0;
                                }
                            }
                            0xf3 => {
                                if lds.hardfade == 0 {
                                    lds.fadeonoff = comlo;
                                }
                            }
                            0xf2 => c.trmstay = comlo,
                            // 0xf1 panorama, 0xf0 progch: MIDI, unhandled.
                            0xf1 | 0xf0 => {}
                            _ => {
                                if comhi < 0xa0 {
                                    c.glideto = comhi & 0x1f;
                                }
                            }
                        }
                    } else {
                        let (sound, high) = note_of(comhi, comlo, transpose);

                        if lds.chandelay[ch] == 0 {
                            lds_playsound(
                                lds,
                                opl,
                                i32::from(sound),
                                i32::from(chan),
                                i32::from(high),
                            );
                        } else {
                            let c = &mut lds.channel[ch];
                            c.chancheat.chandelay = lds.chandelay[ch];
                            c.chancheat.sound = sound;
                            c.chancheat.high = high;
                        }
                    }
                }

                let c = &mut lds.channel[ch];
                c.packpos = c.packpos.wrapping_add(1);
            } else {
                lds.channel[ch].packwait -= 1;
            }
        }

        lds.tempo_now = lds.tempo;
        lds.pattplay = lds.pattplay.wrapping_add(1);
        if vbreak {
            lds.pattplay = 0;
            for c in lds.channel.iter_mut() {
                c.packwait = 0;
                c.packpos = 0;
            }
            lds.posplay = lds.jumppos;
        } else if lds.pattplay >= lds.pattlen {
            lds.pattplay = 0;
            for c in lds.channel.iter_mut() {
                c.packwait = 0;
                c.packpos = 0;
            }
            lds.posplay = (lds.posplay + 1) & MAXPOS;
        }
    } else {
        lds.tempo_now = lds.tempo_now.wrapping_sub(1);
    }

    // effects
    for chan in 0..9u16 {
        let ch = usize::from(chan);
        let regnum = OP_TABLE[ch];
        if lds.channel[ch].keycount > 0 {
            if lds.channel[ch].keycount == 1 {
                lds_setregs_adv(lds, opl, (0xb0 + chan) as u8, 0xdf, 0);
            }
            lds.channel[ch].keycount -= 1;
        }

        // arpeggio
        let c = &mut lds.channel[ch];
        let mut arpreg: u16;
        if c.arp_size == 0 {
            arpreg = 0;
        } else {
            arpreg = u16::from(c.arp_tab[usize::from(c.arp_pos)]) << 4;
            if arpreg == 0x800 {
                if c.arp_pos > 0 {
                    c.arp_tab[0] = c.arp_tab[usize::from(c.arp_pos) - 1];
                }
                c.arp_size = 1;
                c.arp_pos = 0;
                arpreg = u16::from(c.arp_tab[0]) << 4;
            }

            if c.arp_count == c.arp_speed {
                c.arp_pos = c.arp_pos.wrapping_add(1);
                if c.arp_pos >= c.arp_size {
                    c.arp_pos = 0;
                }
                c.arp_count = 0;
            } else {
                c.arp_count = c.arp_count.wrapping_add(1);
            }
        }

        // glide & portamento
        if c.lasttune != 0 && c.lasttune != c.gototune {
            if c.lasttune > c.gototune {
                if c.lasttune - c.gototune < u16::from(c.portspeed) {
                    c.lasttune = c.gototune;
                } else {
                    c.lasttune -= u16::from(c.portspeed);
                }
            } else if c.gototune - c.lasttune < u16::from(c.portspeed) {
                c.lasttune = c.gototune;
            } else {
                c.lasttune = c.lasttune.wrapping_add(u16::from(c.portspeed));
            }

            let arpreg = apply_arp(c.lasttune, arpreg);
            set_tune(lds, opl, chan, arpreg);
        } else if c.vibwait == 0 {
            // vibrato
            if c.vibrate != 0 {
                let wibc = (u32::from(VIBTAB[usize::from(c.vibcount & 0x3f)])
                    * u32::from(c.vibrate)) as u16;

                let tune = if (c.vibcount & 0x40) == 0 {
                    c.lasttune.wrapping_add(wibc >> 8)
                } else {
                    c.lasttune.wrapping_sub(wibc >> 8)
                };
                let tune = apply_arp(tune, arpreg);

                set_tune(lds, opl, chan, tune);
                let c = &mut lds.channel[ch];
                c.vibcount = c.vibcount.wrapping_add(c.vibspeed);
            } else if c.arp_size != 0 {
                // no vibrato, just arpeggio
                let tune = apply_arp(c.lasttune, arpreg);
                set_tune(lds, opl, chan, tune);
            }
        } else {
            // no vibrato, just arpeggio
            c.vibwait -= 1;

            if c.arp_size != 0 {
                let tune = apply_arp(c.lasttune, arpreg);
                set_tune(lds, opl, chan, tune);
            }
        }

        let allvolume = u32::from(lds.allvolume);
        let fm_am = (lds.fmchip[0xc0 + ch] & 1) != 0;

        // tremolo (modulator)
        let c = &mut lds.channel[ch];
        let volmod = u32::from(c.volmod);
        let reg = 0x40 + regnum;
        if c.trmwait == 0 {
            if c.trmrate != 0 {
                let tremc = (u32::from(TREMTAB[usize::from(c.trmcount & 0x7f)])
                    * u32::from(c.trmrate)) as u16;
                let level: u8 = if u32::from(tremc >> 8) <= (volmod & 0x3f) {
                    ((volmod & 0x3f) - u32::from(tremc >> 8)) as u8
                } else {
                    0
                };

                c.trmcount = c.trmcount.wrapping_add(c.trmspeed);
                if allvolume != 0 && fm_am {
                    let v = (((u32::from(level) * allvolume) >> 8) ^ 0x3f) as u8;
                    lds_setregs_adv(lds, opl, reg, 0xc0, v);
                } else {
                    lds_setregs_adv(lds, opl, reg, 0xc0, level ^ 0x3f);
                }
            } else if allvolume != 0 && fm_am {
                let v = ((((volmod & 0x3f) * allvolume) >> 8) ^ 0x3f) & 0x3f;
                lds_setregs_adv(lds, opl, reg, 0xc0, v as u8);
            } else {
                lds_setregs_adv(lds, opl, reg, 0xc0, ((volmod ^ 0x3f) & 0x3f) as u8);
            }
        } else {
            c.trmwait -= 1;
            if allvolume != 0 && fm_am {
                let v = ((((volmod & 0x3f) * allvolume) >> 8) ^ 0x3f) & 0x3f;
                lds_setregs_adv(lds, opl, reg, 0xc0, v as u8);
            }
        }

        // tremolo (carrier)
        let c = &mut lds.channel[ch];
        let volcar = u32::from(c.volcar);
        let reg = 0x43 + regnum;
        if c.trcwait == 0 {
            if c.trcrate != 0 {
                let tremc = (u32::from(TREMTAB[usize::from(c.trccount & 0x7f)])
                    * u32::from(c.trcrate)) as u16;
                let level: u8 = if u32::from(tremc >> 8) <= (volcar & 0x3f) {
                    ((volcar & 0x3f) - u32::from(tremc >> 8)) as u8
                } else {
                    0
                };

                c.trccount = c.trccount.wrapping_add(c.trcspeed);
                if allvolume != 0 {
                    let v = (((u32::from(level) * allvolume) >> 8) ^ 0x3f) as u8;
                    lds_setregs_adv(lds, opl, reg, 0xc0, v);
                } else {
                    lds_setregs_adv(lds, opl, reg, 0xc0, level ^ 0x3f);
                }
            } else if allvolume != 0 {
                let v = ((((volcar & 0x3f) * allvolume) >> 8) ^ 0x3f) & 0x3f;
                lds_setregs_adv(lds, opl, reg, 0xc0, v as u8);
            } else {
                lds_setregs_adv(lds, opl, reg, 0xc0, ((volcar ^ 0x3f) & 0x3f) as u8);
            }
        } else {
            c.trcwait -= 1;
            if allvolume != 0 {
                let v = ((((volcar & 0x3f) * allvolume) >> 8) ^ 0x3f) & 0x3f;
                lds_setregs_adv(lds, opl, reg, 0xc0, v as u8);
            }
        }
    }

    if !lds.playing || lds.songlooped { 0 } else { 1 }
}

/// `soundbank[inst_number]`, guarded per divergence `D-UB-LDS-1`.
fn instrument(soundbank: &[SoundBank], inst_number: i32) -> SoundBank {
    usize::try_from(inst_number)
        .ok()
        .and_then(|n| soundbank.get(n))
        .copied()
        .unwrap_or_default()
}

pub fn lds_playsound(
    lds: &mut LdsState,
    opl: &mut OplState,
    inst_number: i32,
    channel_number: i32,
    tunehigh: i32,
) {
    let ch = channel_number as usize;
    let i = instrument(
        lds.soundbank.as_ref().expect("a song is loaded"),
        inst_number,
    );
    let regnum = OP_TABLE[ch];
    let mut tunehigh = tunehigh;

    // fine tune
    tunehigh +=
        ((i32::from(i.finetune) + i32::from(lds.channel[ch].finetune) + 0x80) & 0xff) - 0x80;

    // arpeggio
    if i.arpeggio == 0 {
        let arpcalc = u16::from(i.arp_tab[0]) << 4;

        if arpcalc > 0x800 {
            tunehigh = tunehigh - i32::from(arpcalc ^ 0xff0) - 16;
        } else {
            tunehigh += i32::from(arpcalc);
        }
    }

    // glide
    let c = &mut lds.channel[ch];
    if c.glideto != 0 {
        c.gototune = tunehigh as u16;
        c.portspeed = c.glideto;
        c.glideto = 0;
        c.finetune = 0;
        return;
    }

    // modulator registers
    lds_setregs(lds, opl, 0x20 + regnum, i.mod_misc);
    let volcalc = i.mod_vol;
    let c = &mut lds.channel[ch];
    if c.nextvol == 0 || (i.feedback & 1) == 0 {
        c.volmod = volcalc;
    } else {
        c.volmod =
            (volcalc & 0xc0) | (((u32::from(volcalc & 0x3f) * u32::from(c.nextvol)) >> 6) as u8);
    }

    let allvolume = u32::from(lds.allvolume);
    let volmod = c.volmod;
    if (i.feedback & 1) == 1 && allvolume != 0 {
        let v = (u32::from(volmod & 0xc0) | ((u32::from(volmod & 0x3f) * allvolume) >> 8)) ^ 0x3f;
        lds_setregs(lds, opl, 0x40 + regnum, v as u8);
    } else {
        lds_setregs(lds, opl, 0x40 + regnum, volmod ^ 0x3f);
    }
    lds_setregs(lds, opl, 0x60 + regnum, i.mod_ad);
    lds_setregs(lds, opl, 0x80 + regnum, i.mod_sr);
    lds_setregs(lds, opl, 0xe0 + regnum, i.mod_wave);

    // carrier registers
    lds_setregs(lds, opl, 0x23 + regnum, i.car_misc);
    let volcalc = i.car_vol;
    let c = &mut lds.channel[ch];
    if c.nextvol == 0 {
        c.volcar = volcalc;
    } else {
        c.volcar =
            (volcalc & 0xc0) | (((u32::from(volcalc & 0x3f) * u32::from(c.nextvol)) >> 6) as u8);
    }

    let volcar = c.volcar;
    if allvolume != 0 {
        let v = (u32::from(volcar & 0xc0) | ((u32::from(volcar & 0x3f) * allvolume) >> 8)) ^ 0x3f;
        lds_setregs(lds, opl, 0x43 + regnum, v as u8);
    } else {
        lds_setregs(lds, opl, 0x43 + regnum, volcar ^ 0x3f);
    }
    lds_setregs(lds, opl, 0x63 + regnum, i.car_ad);
    lds_setregs(lds, opl, 0x83 + regnum, i.car_sr);
    lds_setregs(lds, opl, 0xe3 + regnum, i.car_wave);
    lds_setregs(lds, opl, 0xc0 + ch as u8, i.feedback);
    lds_setregs_adv(lds, opl, 0xb0 + ch as u8, 0xdf, 0); // key off

    // Index guarded at load (D-UB-LDS-1).
    let freq = FREQUENCY[(tunehigh % (12 * 16)) as usize];
    let octave = (tunehigh / (12 * 16) - 1) as u8;
    let key_on = ((i32::from(octave) << 2) + 0x20 + i32::from(freq >> 8)) as u8;
    if i.glide == 0 {
        if i.portamento == 0 || lds.channel[ch].lasttune == 0 {
            lds_setregs(lds, opl, 0xa0 + ch as u8, (freq & 0xff) as u8);
            lds_setregs(lds, opl, 0xb0 + ch as u8, key_on);
            let c = &mut lds.channel[ch];
            c.lasttune = tunehigh as u16;
            c.gototune = tunehigh as u16;
        } else {
            let c = &mut lds.channel[ch];
            c.gototune = tunehigh as u16;
            c.portspeed = i.portamento;
            lds_setregs_adv(lds, opl, 0xb0 + ch as u8, 0xdf, 0x20); // key on
        }
    } else {
        lds_setregs(lds, opl, 0xa0 + ch as u8, (freq & 0xff) as u8);
        lds_setregs(lds, opl, 0xb0 + ch as u8, key_on);
        let c = &mut lds.channel[ch];
        c.lasttune = tunehigh as u16;
        c.gototune = (tunehigh + ((i32::from(i.glide) + 0x80) & 0xff) - 0x80) as u16; // destination
        c.portspeed = i.portamento;
    }

    let c = &mut lds.channel[ch];
    if i.vibrato == 0 {
        c.vibwait = 0;
        c.vibspeed = 0;
        c.vibrate = 0;
    } else {
        c.vibwait = i.vibdelay;
        c.vibspeed = (i.vibrato >> 4) + 2;
        c.vibrate = (i.vibrato & 15) + 1;
    }

    if (c.trmstay & 0xf0) == 0 {
        c.trmwait = (i.tremwait & 0xf0) >> 3;
        c.trmspeed = i.mod_trem >> 4;
        c.trmrate = i.mod_trem & 15;
        c.trmcount = 0;
    }

    if (c.trmstay & 0x0f) == 0 {
        c.trcwait = (i.tremwait & 15) << 1;
        c.trcspeed = i.car_trem >> 4;
        c.trcrate = i.car_trem & 15;
        c.trccount = 0;
    }

    c.arp_size = i.arpeggio & 15;
    c.arp_speed = i.arpeggio >> 4;
    c.arp_tab = i.arp_tab;
    c.keycount = i.keyoff;
    c.nextvol = 0;
    c.glideto = 0;
    c.finetune = 0;
    c.vibcount = 0;
    c.arp_pos = 0;
    c.arp_count = 0;
}
