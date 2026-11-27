//! Differential tests for `lds_play.c` / `lds_play.h`. The globals live in
//! `LdsState` inside `loudness::AudioState` (see `rustyrian_core::loudness`).

use proptest::prelude::*;
use rustyrian_core::lds_play::{self as l, LdsState, Position, State};
use rustyrian_core::opl::OplState;
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::lds_play::{lds_from_c, lds_to_c};
use rustyrian_oracle::mirror::opl::{opl_from_c, opl_to_c};
use std::path::Path;

const RATE: i32 = 44100;

/// Every song of `music.mus`, split as `load_song` does.
fn songs() -> Vec<Vec<u8>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/music.mus");
    let data = std::fs::read(path).expect("data/music.mus (run `cargo xtask fetch-data`)");
    let count = usize::from(u16::from_le_bytes([data[0], data[1]]));
    let mut pos: Vec<usize> = (0..count)
        .map(|i| {
            let o = 2 + 4 * i;
            u32::from_le_bytes(data[o..o + 4].try_into().unwrap()) as usize
        })
        .collect();
    pos.push(data.len());
    (0..count)
        .map(|i| data[pos[i]..pos[i + 1].max(pos[i])].to_vec())
        .collect()
}

/// Both engines with fresh audio globals and `audioSampleRate = RATE`.
fn fresh() -> (Diff, LdsState, OplState) {
    let d = Diff::new();
    let (lds, opl) = (LdsState::default(), OplState::default());
    lds_to_c(&lds);
    opl_to_c(&opl);
    // SAFETY: C runs only on this thread, inside calls we make.
    unsafe { *rustyrian_oracle::mirror::global_mut(&raw mut b::audioSampleRate) = RATE };
    (d, lds, opl)
}

fn c_load(data: &[u8]) -> bool {
    // SAFETY: `data` is valid for its length.
    unsafe { b::lds_load(data.as_ptr().cast(), data.len()) }
}

fn assert_same(lds: &LdsState, opl: Option<&OplState>) -> Result<(), TestCaseError> {
    prop_assert_eq!(&lds_from_c(), lds);
    if let Some(opl) = opl {
        prop_assert_eq!(&opl_from_c(), opl);
    }
    Ok(())
}

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(lds_from_c(), LdsState::default());
    assert_eq!(diff::load::<State>(), State::default());
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let mut s = LdsState {
        positions: Some(vec![
            Position {
                patnum: 1,
                transpose: 2,
            };
            9
        ]),
        // Zero-length blocks are still non-NULL (`malloc(0)`).
        soundbank: Some(Vec::new()),
        patterns: Some(vec![0xffff, 0, 7]),
        tempo_now: 3,
        playing: true,
        ..LdsState::default()
    };
    s.channel[8].vibcount = 9;
    lds_to_c(&s);
    assert_eq!(lds_from_c(), s);
    // Storing again frees and replaces the blocks.
    s.patterns = None;
    lds_to_c(&s);
    assert_eq!(lds_from_c(), s);
}

/// Every song loads identically, and passes the `D-UB-LDS-1` check.
#[test]
fn lds_load_matches_every_song() {
    let songs = songs();
    assert!(songs.len() > 30);
    for (n, song) in songs.iter().enumerate() {
        let (_d, mut lds, mut opl) = fresh();
        let c = c_load(song);
        let r = l::lds_load(&mut lds, &mut opl, RATE, song);
        assert_eq!(c, r, "song {n}");
        assert!(r, "song {n} is accepted");
        assert!(l::lds_song_is_safe(&lds), "song {n}");
        assert_same(&lds, Some(&opl)).unwrap();
    }
}

/// Every song plays identically for 20000 updates (about 4.8 minutes of
/// music): return values, player state each update, chip state every 64.
#[test]
fn lds_update_matches_every_song() {
    for (n, song) in songs().iter().enumerate() {
        let (_d, mut lds, mut opl) = fresh();
        assert!(c_load(song));
        assert!(l::lds_load(&mut lds, &mut opl, RATE, song));
        for k in 0..20_000 {
            // SAFETY: plain C call on the test thread.
            let c = unsafe { b::lds_update() };
            let r = l::lds_update(&mut lds, &mut opl);
            assert_eq!(c, r, "song {n} update {k}");
            if k % 16 == 0 {
                assert_eq!(lds_from_c(), lds, "song {n} update {k}");
            }
            if k % 64 == 0 {
                assert_eq!(opl_from_c(), opl, "song {n} update {k}");
            }
        }
        assert_same(&lds, Some(&opl)).unwrap();
    }
}

#[test]
fn lds_load_rejects_mode_above_2_like_c() {
    let mut song = songs()[0].clone();
    song[0] = 3;
    let (_d, mut lds, mut opl) = fresh();
    assert!(!c_load(&song));
    assert!(!l::lds_load(&mut lds, &mut opl, RATE, &song));
    assert_same(&lds, Some(&opl)).unwrap();
}

proptest! {
    /// Mutated and truncated songs: a song the `D-UB-LDS-1` check accepts
    /// loads and plays exactly as in C; a rejected one is loaded as C does
    /// but left stopped (C is not run past the load; see `D-UB-LDS-1`).
    #[test]
    fn mutated_songs_are_exact_or_rejected(
        song in 0usize..40,
        flips in prop::collection::vec((any::<prop::sample::Index>(), any::<u8>()), 0..6),
        cut in prop::option::of(any::<prop::sample::Index>()),
    ) {
        let songs = songs();
        let mut data = songs[song % songs.len()].clone();
        for (i, v) in &flips {
            let i = i.index(data.len());
            data[i] = *v;
        }
        if let Some(c) = cut {
            data.truncate(c.index(data.len() + 1));
        }
        let (_d, mut lds, mut opl) = fresh();
        let c = c_load(&data);
        let r = l::lds_load(&mut lds, &mut opl, RATE, &data);
        if data.first().is_some_and(|&m| m > 2) {
            prop_assert!(!c && !r);
            return Ok(());
        }
        prop_assert!(c);
        if !r {
            prop_assert!(!l::lds_song_is_safe(&lds) && !lds.playing);
            let mut c_state = lds_from_c();
            prop_assert!(c_state.playing);
            c_state.playing = false;
            prop_assert_eq!(c_state, lds);
            return Ok(());
        }
        // Give both engines the same instrument table for `D-UB-LDS-1`'s
        // guarded reads.
        let mut padded = lds.soundbank.clone().unwrap();
        padded.resize(64.max(padded.len()), Default::default());
        let mut c_state = lds_from_c();
        c_state.soundbank = Some(padded);
        lds_to_c(&c_state);
        for _ in 0..3000 {
            // SAFETY: plain C call on the test thread.
            let c = unsafe { b::lds_update() };
            prop_assert_eq!(c, l::lds_update(&mut lds, &mut opl));
        }
        let mut r_state = lds.clone();
        r_state.soundbank = c_state.soundbank.clone();
        assert_same(&r_state, Some(&opl))?;
    }

    /// `lds_setregs`/`lds_setregs_adv` from any register cache.
    #[test]
    fn lds_setregs_match(fm in prop::collection::vec(any::<u8>(), 0xff), reg in 0u8..0xff, mask: u8, val: u8, adv: bool) {
        let (_d, mut lds, mut opl) = fresh();
        // SAFETY: plain C call on the test thread.
        unsafe { b::adlib_init(RATE as u32) };
        rustyrian_core::opl::adlib_init(&mut opl, RATE as u32);
        lds.fmchip.copy_from_slice(&fm);
        lds_to_c(&lds);
        // SAFETY: as above.
        unsafe {
            if adv { b::lds_setregs_adv(reg, mask, val) } else { b::lds_setregs(reg, val) }
        };
        if adv { l::lds_setregs_adv(&mut lds, &mut opl, reg, mask, val) } else { l::lds_setregs(&mut lds, &mut opl, reg, val) };
        assert_same(&lds, Some(&opl))?;
    }

    #[test]
    fn lds_fade_rewind_free_match(speed: u8, regbd: u8, song in 0usize..40) {
        let songs = songs();
        let (_d, mut lds, mut opl) = fresh();
        let data = &songs[song % songs.len()];
        prop_assert!(c_load(data));
        prop_assert!(l::lds_load(&mut lds, &mut opl, RATE, data));
        for _ in 0..50 {
            // SAFETY: plain C call on the test thread.
            unsafe { b::lds_update() };
            l::lds_update(&mut lds, &mut opl);
        }
        // SAFETY: as above.
        unsafe { b::lds_fade(speed) };
        l::lds_fade(&mut lds, speed);
        for _ in 0..300 {
            // SAFETY: as above.
            let c = unsafe { b::lds_update() };
            prop_assert_eq!(c, l::lds_update(&mut lds, &mut opl));
        }
        assert_same(&lds, Some(&opl))?;
        lds.regbd = regbd;
        lds_to_c(&lds);
        // SAFETY: as above.
        unsafe { b::lds_rewind() };
        l::lds_rewind(&mut lds, &mut opl, RATE);
        assert_same(&lds, Some(&opl))?;
        // SAFETY: as above.
        unsafe { b::lds_free() };
        l::lds_free(&mut lds);
        assert_same(&lds, None)?;
    }

    /// `lds_update` from perturbed mid-song states: fading in both
    /// directions, hard fades, arpeggios, glides, vibrato, tremolo, and
    /// pattern commands rare in the freeware songs (written over non-flow
    /// words, so every read stays where the loaded song's reads are).
    #[test]
    fn lds_update_from_perturbed_states_matches(
        song in 0usize..40,
        ticks in 0usize..300,
        fade: (u8, u8, u16, u8),
        chans in prop::collection::vec(
            (0u8..=12, any::<u8>(), any::<[u8; 12]>(), any::<(u16, u16, u8)>(), any::<(u8, u8, u8, u8)>(), any::<(u8, u8, u8, u8, u8)>(), any::<(u8, u8, u8, u8)>()),
            9,
        ),
        commands in prop::collection::vec((any::<prop::sample::Index>(), prop::sample::select(&[0xf0u8, 0xf1, 0xf2, 0xf3, 0xf4, 0xf6, 0xf7, 0xf8, 0xfb, 0xfd, 0xfe, 0xff][..]), any::<u8>()), 0..30),
    ) {
        let songs = songs();
        let (_d, mut lds, mut opl) = fresh();
        let data = &songs[song % songs.len()];
        prop_assert!(c_load(data));
        prop_assert!(l::lds_load(&mut lds, &mut opl, RATE, data));
        for _ in 0..ticks {
            // SAFETY: plain C call on the test thread.
            unsafe { b::lds_update() };
            l::lds_update(&mut lds, &mut opl);
        }
        (lds.fadeonoff, lds.allvolume, lds.mainvolume, lds.hardfade) = (fade.0, fade.1, fade.2 & 0xff, fade.3 % 2);
        for (c, (arp_size, arp_pos, arp_tab, tunes, vib, trm, trc)) in lds.channel.iter_mut().zip(&chans) {
            c.arp_size = *arp_size;
            c.arp_pos = if *arp_size == 0 { 0 } else { arp_pos % arp_size };
            c.arp_tab = *arp_tab;
            (c.lasttune, c.gototune, c.portspeed) = *tunes;
            (c.vibwait, c.vibrate, c.vibcount, c.vibspeed) = (vib.0 % 3, vib.1 % 16, vib.2, vib.3);
            (c.trmwait, c.trmrate, c.trmcount, c.trmspeed, c.volmod) = (trm.0 % 3, trm.1 % 16, trm.2, trm.3, trm.4);
            (c.trcwait, c.trcrate, c.trccount, c.volcar) = (trc.0 % 3, trc.1 % 16, trc.2, trc.3);
        }
        let patterns = lds.patterns.as_mut().unwrap();
        let targets: Vec<usize> = (0..patterns.len())
            .filter(|&k| {
                let hi = patterns[k] >> 8;
                patterns[k] != 0 && hi != 0x80 && hi != 0xf9 && hi != 0xfa && hi != 0xfc && hi != 0xf5
            })
            .collect();
        if !targets.is_empty() {
            for (at, hi, lo) in &commands {
                patterns[targets[at.index(targets.len())]] = u16::from(*hi) << 8 | u16::from(*lo);
            }
        }
        // Same instrument table in both engines for `D-UB-LDS-1`'s guarded
        // reads (the store reallocates C's block).
        let bank = lds.soundbank.as_mut().unwrap();
        bank.resize(64.max(bank.len()), Default::default());
        lds_to_c(&lds);
        for _ in 0..400 {
            // SAFETY: as above.
            let c = unsafe { b::lds_update() };
            prop_assert_eq!(c, l::lds_update(&mut lds, &mut opl));
        }
        assert_same(&lds, Some(&opl))?;
    }

    /// `lds_playsound` for any instrument, channel and tune whose frequency
    /// index is in bounds, from mid-song channel states.
    #[test]
    fn lds_playsound_matches(song in 0usize..40, ticks in 0usize..400, chan in 0i32..9, high in 0i32..0x1000, inst: prop::sample::Index, glideto: u8, nextvol: u8, finetune: u8, allvolume: u8, trmstay: u8) {
        let songs = songs();
        let (_d, mut lds, mut opl) = fresh();
        let data = &songs[song % songs.len()];
        prop_assert!(c_load(data));
        prop_assert!(l::lds_load(&mut lds, &mut opl, RATE, data));
        for _ in 0..ticks {
            // SAFETY: plain C call on the test thread.
            unsafe { b::lds_update() };
            l::lds_update(&mut lds, &mut opl);
        }
        let numpatch = lds.soundbank.as_ref().unwrap().len();
        prop_assume!(numpatch > 0);
        let inst = inst.index(numpatch) as i32;
        let c = &mut lds.channel[chan as usize];
        c.glideto = glideto % 4;
        c.nextvol = nextvol;
        c.finetune = finetune;
        c.trmstay = trmstay;
        lds.allvolume = allvolume;
        lds_to_c(&lds);
        // SAFETY: as above; arguments are in the ranges C's indexing
        // expects for valid songs.
        unsafe { b::lds_playsound(inst, chan, high + 0x900) };
        l::lds_playsound(&mut lds, &mut opl, inst, chan, high + 0x900);
        assert_same(&lds, Some(&opl))?;
    }
}
