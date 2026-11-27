//! Mirror of the C globals of `lds_play.c`.
//!
//! They all live in `rustyrian_core::lds_play::LdsState`, owned by
//! `loudness::AudioState` (see `rustyrian_core::loudness`), so the loudness
//! `AudioState` mirror calls [`lds_from_c`]/[`lds_to_c`]; `lds_play::State` is
//! empty. Heap blocks are mirrored as their contents, with the length
//! upstream allocated (`shim::alloc_size`).

use super::lvllib::{heap_from_c, heap_to_c};
use super::video::sv;
use super::{Mirror, global, global_mut};
use crate::bindings as b;
use rustyrian_core::lds_play::{Channel, ChannelChancheat, LdsState, Position, SoundBank, State};

fn soundbank_from_c(c: &b::SoundBank) -> SoundBank {
    SoundBank {
        mod_misc: c.mod_misc,
        mod_vol: c.mod_vol,
        mod_ad: c.mod_ad,
        mod_sr: c.mod_sr,
        mod_wave: c.mod_wave,
        car_misc: c.car_misc,
        car_vol: c.car_vol,
        car_ad: c.car_ad,
        car_sr: c.car_sr,
        car_wave: c.car_wave,
        feedback: c.feedback,
        keyoff: c.keyoff,
        portamento: c.portamento,
        glide: c.glide,
        finetune: c.finetune,
        vibrato: c.vibrato,
        vibdelay: c.vibdelay,
        mod_trem: c.mod_trem,
        car_trem: c.car_trem,
        tremwait: c.tremwait,
        arpeggio: c.arpeggio,
        arp_tab: c.arp_tab,
        start: c.start,
        size: c.size,
        fms: c.fms,
        transp: c.transp,
        midinst: c.midinst,
        midvelo: c.midvelo,
        midkey: c.midkey,
        midtrans: c.midtrans,
        middum1: c.middum1,
        middum2: c.middum2,
    }
}

fn soundbank_to_c(r: &SoundBank) -> b::SoundBank {
    b::SoundBank {
        mod_misc: r.mod_misc,
        mod_vol: r.mod_vol,
        mod_ad: r.mod_ad,
        mod_sr: r.mod_sr,
        mod_wave: r.mod_wave,
        car_misc: r.car_misc,
        car_vol: r.car_vol,
        car_ad: r.car_ad,
        car_sr: r.car_sr,
        car_wave: r.car_wave,
        feedback: r.feedback,
        keyoff: r.keyoff,
        portamento: r.portamento,
        glide: r.glide,
        finetune: r.finetune,
        vibrato: r.vibrato,
        vibdelay: r.vibdelay,
        mod_trem: r.mod_trem,
        car_trem: r.car_trem,
        tremwait: r.tremwait,
        arpeggio: r.arpeggio,
        arp_tab: r.arp_tab,
        start: r.start,
        size: r.size,
        fms: r.fms,
        transp: r.transp,
        midinst: r.midinst,
        midvelo: r.midvelo,
        midkey: r.midkey,
        midtrans: r.midtrans,
        middum1: r.middum1,
        middum2: r.middum2,
    }
}

fn channel_from_c(c: &b::Channel) -> Channel {
    Channel {
        gototune: c.gototune,
        lasttune: c.lasttune,
        packpos: c.packpos,
        finetune: c.finetune,
        glideto: c.glideto,
        portspeed: c.portspeed,
        nextvol: c.nextvol,
        volmod: c.volmod,
        volcar: c.volcar,
        vibwait: c.vibwait,
        vibspeed: c.vibspeed,
        vibrate: c.vibrate,
        trmstay: c.trmstay,
        trmwait: c.trmwait,
        trmspeed: c.trmspeed,
        trmrate: c.trmrate,
        trmcount: c.trmcount,
        trcwait: c.trcwait,
        trcspeed: c.trcspeed,
        trcrate: c.trcrate,
        trccount: c.trccount,
        arp_size: c.arp_size,
        arp_speed: c.arp_speed,
        keycount: c.keycount,
        vibcount: c.vibcount,
        arp_pos: c.arp_pos,
        arp_count: c.arp_count,
        packwait: c.packwait,
        arp_tab: c.arp_tab,
        chancheat: ChannelChancheat {
            chandelay: c.chancheat.chandelay,
            sound: c.chancheat.sound,
            high: c.chancheat.high,
        },
    }
}

fn channel_to_c(r: &Channel) -> b::Channel {
    b::Channel {
        gototune: r.gototune,
        lasttune: r.lasttune,
        packpos: r.packpos,
        finetune: r.finetune,
        glideto: r.glideto,
        portspeed: r.portspeed,
        nextvol: r.nextvol,
        volmod: r.volmod,
        volcar: r.volcar,
        vibwait: r.vibwait,
        vibspeed: r.vibspeed,
        vibrate: r.vibrate,
        trmstay: r.trmstay,
        trmwait: r.trmwait,
        trmspeed: r.trmspeed,
        trmrate: r.trmrate,
        trmcount: r.trmcount,
        trcwait: r.trcwait,
        trcspeed: r.trcspeed,
        trcrate: r.trcrate,
        trccount: r.trccount,
        arp_size: r.arp_size,
        arp_speed: r.arp_speed,
        keycount: r.keycount,
        vibcount: r.vibcount,
        arp_pos: r.arp_pos,
        arp_count: r.arp_count,
        packwait: r.packwait,
        arp_tab: r.arp_tab,
        chancheat: b::Channel__bindgen_ty_1 {
            chandelay: r.chancheat.chandelay,
            sound: r.chancheat.sound,
            high: r.chancheat.high,
        },
    }
}

/// Every global of `lds_play.c`, from C.
pub fn lds_from_c() -> LdsState {
    // SAFETY: C runs only on this thread, inside calls we make; each `sv`
    // type is the C type of that static; the heap pointers are NULL or blocks
    // malloc'd by `lds_load`.
    unsafe {
        let channel = sv::<[b::Channel; 9]>(b::ot_sv_lds_play_channel);
        LdsState {
            soundbank: heap_from_c(
                *sv::<*mut b::SoundBank>(b::ot_sv_lds_play_soundbank),
                soundbank_from_c,
            ),
            channel: channel.map(|c| channel_from_c(&c)),
            positions: heap_from_c(*sv::<*mut b::Position>(b::ot_sv_lds_play_positions), |p| {
                Position {
                    patnum: p.patnum,
                    transpose: p.transpose,
                }
            }),
            fmchip: *sv::<[u8; 0xff]>(b::ot_sv_lds_play_fmchip),
            jumping: *sv::<u8>(b::ot_sv_lds_play_jumping),
            fadeonoff: *sv::<u8>(b::ot_sv_lds_play_fadeonoff),
            allvolume: *sv::<u8>(b::ot_sv_lds_play_allvolume),
            hardfade: *sv::<u8>(b::ot_sv_lds_play_hardfade),
            tempo_now: *sv::<u8>(b::ot_sv_lds_play_tempo_now),
            pattplay: *sv::<u8>(b::ot_sv_lds_play_pattplay),
            tempo: *sv::<u8>(b::ot_sv_lds_play_tempo),
            regbd: *sv::<u8>(b::ot_sv_lds_play_regbd),
            chandelay: *sv::<[u8; 9]>(b::ot_sv_lds_play_chandelay),
            mode: *sv::<u8>(b::ot_sv_lds_play_mode),
            pattlen: *sv::<u8>(b::ot_sv_lds_play_pattlen),
            posplay: *sv::<u16>(b::ot_sv_lds_play_posplay),
            jumppos: *sv::<u16>(b::ot_sv_lds_play_jumppos),
            speed: *sv::<u16>(b::ot_sv_lds_play_speed),
            patterns: heap_from_c(*sv::<*mut u16>(b::ot_sv_lds_play_patterns), |&x| x),
            numpatch: *sv::<u16>(b::ot_sv_lds_play_numpatch),
            numposi: *sv::<u16>(b::ot_sv_lds_play_numposi),
            mainvolume: *sv::<u16>(b::ot_sv_lds_play_mainvolume),
            playing: *global(&raw const b::playing),
            songlooped: *global(&raw const b::songlooped),
        }
    }
}

/// Writes every global of `lds_play.c` (replacing and freeing its heap
/// blocks).
pub fn lds_to_c(r: &LdsState) {
    // SAFETY: as in `lds_from_c`; the heap blocks are owned by their globals.
    unsafe {
        heap_to_c(
            sv::<*mut b::SoundBank>(b::ot_sv_lds_play_soundbank),
            r.soundbank.as_deref(),
            soundbank_to_c,
        );
        *sv::<[b::Channel; 9]>(b::ot_sv_lds_play_channel) = r.channel.map(|c| channel_to_c(&c));
        heap_to_c(
            sv::<*mut b::Position>(b::ot_sv_lds_play_positions),
            r.positions.as_deref(),
            |p| b::Position {
                patnum: p.patnum,
                transpose: p.transpose,
            },
        );
        *sv::<[u8; 0xff]>(b::ot_sv_lds_play_fmchip) = r.fmchip;
        *sv::<u8>(b::ot_sv_lds_play_jumping) = r.jumping;
        *sv::<u8>(b::ot_sv_lds_play_fadeonoff) = r.fadeonoff;
        *sv::<u8>(b::ot_sv_lds_play_allvolume) = r.allvolume;
        *sv::<u8>(b::ot_sv_lds_play_hardfade) = r.hardfade;
        *sv::<u8>(b::ot_sv_lds_play_tempo_now) = r.tempo_now;
        *sv::<u8>(b::ot_sv_lds_play_pattplay) = r.pattplay;
        *sv::<u8>(b::ot_sv_lds_play_tempo) = r.tempo;
        *sv::<u8>(b::ot_sv_lds_play_regbd) = r.regbd;
        *sv::<[u8; 9]>(b::ot_sv_lds_play_chandelay) = r.chandelay;
        *sv::<u8>(b::ot_sv_lds_play_mode) = r.mode;
        *sv::<u8>(b::ot_sv_lds_play_pattlen) = r.pattlen;
        *sv::<u16>(b::ot_sv_lds_play_posplay) = r.posplay;
        *sv::<u16>(b::ot_sv_lds_play_jumppos) = r.jumppos;
        *sv::<u16>(b::ot_sv_lds_play_speed) = r.speed;
        heap_to_c(
            sv::<*mut u16>(b::ot_sv_lds_play_patterns),
            r.patterns.as_deref(),
            |&x| x,
        );
        *sv::<u16>(b::ot_sv_lds_play_numpatch) = r.numpatch;
        *sv::<u16>(b::ot_sv_lds_play_numposi) = r.numposi;
        *sv::<u16>(b::ot_sv_lds_play_mainvolume) = r.mainvolume;
        *global_mut(&raw mut b::playing) = r.playing;
        *global_mut(&raw mut b::songlooped) = r.songlooped;
    }
}

/// `lds_play::State` is empty; its globals are mirrored with `AudioState`.
impl Mirror for State {
    fn load_from_c(&mut self) {}

    fn store_to_c(&self) {}
}
