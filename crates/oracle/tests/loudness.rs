//! Differential tests for `loudness.c` / `loudness.h` (`State` and the
//! audio-thread `AudioState`, which also carries the `lds_play.c` and `opl.c`
//! globals).

use proptest::prelude::*;
use rustyrian_core::Game;
use rustyrian_core::game::Halt;
use rustyrian_core::loudness::{self as ld, AudioState, ChannelSamples, State, audio};
use rustyrian_core::nortsong::{self, SampleBuf};
use rustyrian_core::platform::{FrameView, Platform, PlatformProbe, RendererId};
use rustyrian_harness::{CallLog, test_session};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;
use rustyrian_oracle::session;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data")
        .canonicalize()
        .expect("data/ (run `cargo xtask fetch-data`)")
}

fn bytes(p: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    p.as_os_str().as_bytes().to_vec()
}

/// Both engines in a fresh-process state (C's globals reset from the Rust
/// defaults, as proptest reuses the process), finding data in `dir`.
fn fresh(dir: &Path) -> Diff {
    let mut d = Diff::new();
    d.game.file.custom_data_dir_path = Some(bytes(dir));
    d.store_to_c();
    d
}

/// Replaces the Rust platform with a session whose next calls to `fails`
/// fail; C's session gets the same failures. Returns the new Rust call log.
fn inject(d: &mut Diff, fails: &[&str]) -> CallLog {
    let (mut r, log) = test_session();
    for f in fails {
        r.inner_mut().fail_next(f);
        session::with(|s| s.inner_mut().fail_next(f));
    }
    d.game.platform = Box::new(r);
    log
}

fn c_audio_state() -> AudioState {
    diff::load::<AudioState>()
}

/// Compares every audio-related module state of both engines.
fn assert_states(d: &Diff) {
    assert_eq!(diff::load::<State>(), d.game.loudness);
    assert_eq!(diff::load::<nortsong::State>(), d.game.nortsong);
    assert_eq!(c_audio_state(), *audio(&d.game));
    assert_eq!(session::crand_state(), *d.game.crand.lock().unwrap());
}

/// `init_audio` in both engines; returns both results.
fn init_both(d: &mut Diff) -> (bool, bool) {
    // SAFETY: plain C call on the test thread.
    let c = unsafe { b::init_audio() };
    (c, ld::init_audio(&mut d.game))
}

/// The audio callback over `n` samples in both engines; returns C's and
/// Rust's samples.
fn callback_both(g: &Game, n: usize) -> (Vec<i16>, Vec<i16>) {
    let mut c = vec![0i16; n];
    // SAFETY: `c` holds `n` samples, `2 * n` bytes.
    unsafe {
        b::ot_s_loudness_audioCallback(std::ptr::null_mut(), c.as_mut_ptr().cast(), (2 * n) as i32)
    };
    let mut r = vec![0i16; n];
    ld::audio_callback(&mut audio(g), &g.crand, &mut r);
    (c, r)
}

fn play_both(d: &mut Diff, song: u32) -> (Option<i32>, Result<(), Halt>) {
    // SAFETY: play_song may exit, which the guard catches.
    let c = unsafe { rustyrian_oracle::guard_call(|| b::play_song(song)) };
    (c, ld::play_song(&mut d.game, song))
}

fn load_sounds_both(d: &mut Diff) {
    // SAFETY: plain C call on the test thread.
    let c = unsafe { rustyrian_oracle::guard_call(|| b::loadSndFile(false)) };
    let r = nortsong::load_snd_file(&mut d.game, false);
    assert_eq!((c, r), (None, Ok(())));
}

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
    assert_eq!(diff::load::<AudioState>(), AudioState::default());
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let s = State {
        audio_sample_rate: 44100,
        song_playing: 29,
        audio_disabled: true,
        audio_device: 3,
        song_positions: Some(vec![10, 2000, -5, 1 << 40]),
        songs_count: 3,
        load_song_first: false,
    };
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}

#[test]
fn audio_state_mirror_round_trip() {
    let _d = Diff::new();
    // Channels point into `soundSamples`, so store those first (as
    // `store_game` does).
    let mut sounds = nortsong::State::default();
    for i in [0, 4, 7] {
        let v: Vec<i16> = (0..100 + i).map(|x| x as i16 * 3 - 50).collect();
        sounds.sound_sample_count[i] = v.len();
        sounds.sound_samples[i] = Some(SampleBuf(v.into()));
    }
    sounds.store_to_c();
    let buf = |i: usize| sounds.sound_samples[i].clone().expect("stored above");

    let mut s = AudioState {
        music_stopped: false,
        music_disabled: true,
        samples_disabled: true,
        music_volume: 10,
        sample_volume: 20,
        samples_per_lds_update: 634,
        samples_per_lds_update_frac: 122,
        samples_until_lds_update: 5,
        samples_until_lds_update_frac: 6,
        channel_sample_count: [50, 0, 3, 0, 0, 0, 0, 1],
        channel_volume: [1, 2, 3, 4, 5, 6, 7, 0],
        ..AudioState::default()
    };
    for (i, v) in s.volume_factor_table.iter_mut().enumerate() {
        *v = i as i32 * 4096 - 7;
    }
    s.channel_samples[0] = Some(ChannelSamples {
        sound: 0,
        buf: buf(0),
        pos: 50,
    });
    s.channel_samples[2] = Some(ChannelSamples {
        sound: 7,
        buf: buf(7),
        pos: 104,
    });
    // A finished channel: pointer at the end of its buffer.
    s.channel_samples[3] = Some(ChannelSamples {
        sound: 4,
        buf: buf(4),
        pos: 104,
    });
    s.channel_samples[7] = Some(ChannelSamples {
        sound: 4,
        buf: buf(4),
        pos: 0,
    });
    s.lds.soundbank = Some(vec![Default::default(); 5]);
    s.lds.positions = Some(vec![Default::default(); 18]);
    s.lds.patterns = Some((0..333).map(|x| x * 3).collect());
    s.lds.playing = true;
    s.opl.op[3].cur_wform = Some(300);
    s.opl.vibval1 = rustyrian_core::opl::VibvalTable::Var1;
    s.opl.adlib_init_initfirstime = 1;

    s.store_to_c();
    assert_eq!(diff::load::<AudioState>(), s);
}

#[test]
fn init_audio_matches() {
    let mut d = fresh(&data_dir());
    assert_eq!(init_both(&mut d), (true, true));
    assert_states(&d);
    assert_eq!(d.c_calls(), d.rust_calls());
    // Already initialised: init again (as upstream never does, but the
    // function allows it).
    assert_eq!(init_both(&mut d), (true, true));
    assert_states(&d);
    assert_eq!(d.c_calls(), d.rust_calls());
}

#[test]
fn init_audio_failures_match() {
    for fail in ["SDL_InitSubSystem", "SDL_OpenAudioDevice"] {
        let mut d = fresh(&data_dir());
        let log = inject(&mut d, &[fail]);
        assert_eq!(init_both(&mut d), (false, false), "{fail}");
        assert_states(&d);
        assert_eq!(d.c_calls(), log.take(), "{fail}");
        // audio_disabled is now set: every entry point returns early.
        assert_eq!(init_both(&mut d), (false, false));
        assert_eq!(play_both(&mut d, 3), (None, Ok(())));
        // SAFETY: plain C calls on the test thread.
        unsafe {
            b::restart_song();
            b::stop_song();
            b::fade_song();
            b::set_volume(1, 2);
            b::multiSamplePlay(std::ptr::null(), 0, 0, 0);
            b::deinit_audio();
        }
        ld::restart_song(&mut d.game);
        ld::stop_song(&mut d.game);
        ld::fade_song(&mut d.game);
        ld::set_volume(&mut d.game, 1, 2);
        ld::multi_sample_play(&mut d.game, None, 0, 0, 0);
        ld::deinit_audio(&mut d.game);
        assert_states(&d);
        assert_eq!(d.c_calls(), log.take(), "{fail}");
    }
}

#[test]
fn deinit_audio_matches() {
    let mut d = fresh(&data_dir());
    init_both(&mut d);
    assert_eq!(play_both(&mut d, 5), (None, Ok(())));
    // SAFETY: plain C call on the test thread.
    unsafe { b::deinit_audio() };
    ld::deinit_audio(&mut d.game);
    assert_states(&d);
    assert_eq!(d.c_calls(), d.rust_calls());
    // A second deinit with the device already closed.
    // SAFETY: as above.
    unsafe { b::deinit_audio() };
    ld::deinit_audio(&mut d.game);
    assert_states(&d);
    assert_eq!(d.c_calls(), d.rust_calls());
}

/// As [`assert_states`] plus equal platform calls, except where Rust rejected
/// the loaded song (divergence `D-UB-LDS-1`): then C's player state differs
/// only in `playing`, and Rust's calls only add its "Failed to load song"
/// error.
fn assert_states_or_rejected(d: &Diff) {
    let rejected = {
        let a = audio(&d.game);
        a.lds.positions.is_some()
            && !a.lds.playing
            && !rustyrian_core::lds_play::lds_song_is_safe(&a.lds)
    };
    let c_calls = d.c_calls();
    let mut r_calls = d.rust_calls();
    if rejected {
        let mut c = c_audio_state();
        assert!(c.lds.playing);
        c.lds.playing = false;
        assert_eq!(c, *audio(&d.game));
        let extra = r_calls
            .iter()
            .position(|call| format!("{call:?}").contains("Failed to load song"))
            .expect("Rust reports the rejected song");
        r_calls.remove(extra);
    } else {
        assert_eq!(c_audio_state(), *audio(&d.game));
    }
    assert_eq!(diff::load::<State>(), d.game.loudness);
    assert_eq!(c_calls, r_calls);
}

#[test]
fn load_song_out_of_range_and_failures_match() {
    let mut d = fresh(&data_dir());
    init_both(&mut d);
    // Out of range (logs, loads nothing), then a real song.
    for song in [500, 41, 7] {
        assert_eq!(play_both(&mut d, song), (None, Ok(())));
        assert_states(&d);
        assert_eq!(d.c_calls(), d.rust_calls());
    }
    // A truncated music.mus: reads fail after the header.
    let dir = std::env::temp_dir().join(format!("rustyrian-loudness-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let music = std::fs::read(data_dir().join("music.mus")).unwrap();
    std::fs::write(dir.join("music.mus"), &music[..60]).unwrap();
    for len in [60usize, 1] {
        std::fs::write(dir.join("music.mus"), &music[..len]).unwrap();
        let mut d = fresh(&dir);
        init_both(&mut d);
        for song in [3, 0] {
            assert_eq!(play_both(&mut d, song), (None, Ok(())));
            assert_states_or_rejected(&d);
        }
    }
    // A song whose data is cut short by the end of the file.
    let mut cut = music.clone();
    cut.truncate(music.len() - 500);
    std::fs::write(dir.join("music.mus"), &cut).unwrap();
    let mut d = fresh(&dir);
    init_both(&mut d);
    assert_eq!(play_both(&mut d, 40), (None, Ok(())));
    assert_states_or_rejected(&d);
    // A song with an unknown format mode: lds_load fails, which is logged.
    let mut bad = music.clone();
    let song3 = u32::from_le_bytes(bad[2 + 4 * 3..6 + 4 * 3].try_into().unwrap()) as usize;
    bad[song3] = 3;
    std::fs::write(dir.join("music.mus"), &bad).unwrap();
    let mut d = fresh(&dir);
    init_both(&mut d);
    assert_eq!(play_both(&mut d, 3), (None, Ok(())));
    assert_states(&d);
    assert_eq!(d.c_calls(), d.rust_calls());
    // No music.mus at all: fatal.
    std::fs::remove_file(dir.join("music.mus")).unwrap();
    let mut d = fresh(&dir);
    init_both(&mut d);
    let (c, r) = play_both(&mut d, 3);
    assert_eq!(c, Some(1));
    assert_eq!(r, Err(Halt { code: 1 }));
    assert_eq!(d.c_calls(), d.rust_calls());
    let _ = std::fs::remove_dir_all(&dir);
}

/// One step of a random audio session.
#[derive(Clone, Debug)]
enum Op {
    Play(u32),
    Stop,
    Restart,
    Fade,
    Volume(u8, u8),
    Sample(usize, u8, u8),
    Callback(usize),
    MusicDisabled(bool),
    SamplesDisabled(bool),
}

fn any_op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0u32..42).prop_map(Op::Play),
        Just(Op::Stop),
        Just(Op::Restart),
        Just(Op::Fade),
        (any::<u8>(), any::<u8>()).prop_map(|(m, s)| Op::Volume(m, s)),
        (0usize..38, 0u8..8, 0u8..8).prop_map(|(i, c, v)| Op::Sample(i, c, v)),
        (0usize..9000).prop_map(Op::Callback),
        (0usize..9000).prop_map(Op::Callback),
        any::<bool>().prop_map(Op::MusicDisabled),
        any::<bool>().prop_map(Op::SamplesDisabled),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 48, ..ProptestConfig::default() })]

    /// Random sessions of every entry point and the mixer: samples, states
    /// and platform calls match after each step.
    #[test]
    fn random_sessions_match(ops in prop::collection::vec(any_op(), 1..40)) {
        let mut d = fresh(&data_dir());
        init_both(&mut d);
        load_sounds_both(&mut d);
        prop_assert_eq!(d.c_calls(), d.rust_calls());
        for op in &ops {
            match *op {
                Op::Play(n) => prop_assert_eq!(play_both(&mut d, n), (None, Ok(()))),
                Op::Stop => {
                    // SAFETY: plain C call on the test thread.
                    unsafe { b::stop_song() };
                    ld::stop_song(&mut d.game);
                }
                Op::Restart => {
                    // SAFETY: as above.
                    unsafe { b::restart_song() };
                    ld::restart_song(&mut d.game);
                }
                Op::Fade => {
                    // SAFETY: as above.
                    unsafe { b::fade_song() };
                    ld::fade_song(&mut d.game);
                }
                Op::Volume(m, s) => {
                    // SAFETY: as above.
                    unsafe { b::set_volume(m, s) };
                    ld::set_volume(&mut d.game, m, s);
                }
                Op::Sample(i, chan, vol) => {
                    // SAFETY: `soundSamples[i]` is NULL or a live block of
                    // `soundSampleCount[i]` samples.
                    unsafe {
                        let s = rustyrian_oracle::mirror::global(&raw const b::soundSamples)[i];
                        let n = rustyrian_oracle::mirror::global(&raw const b::soundSampleCount)[i];
                        b::multiSamplePlay(s, n, chan, vol);
                    }
                    let n = d.game.nortsong.sound_sample_count[i];
                    let s = d.game.nortsong.sound_samples[i]
                        .clone()
                        .map(|buf| ChannelSamples { sound: i, buf, pos: 0 });
                    ld::multi_sample_play(&mut d.game, s, n, chan, vol);
                }
                Op::Callback(n) => {
                    let (c, r) = callback_both(&d.game, n);
                    prop_assert_eq!(c, r);
                }
                Op::MusicDisabled(v) | Op::SamplesDisabled(v) => {
                    let mut a = audio(&d.game);
                    if matches!(op, Op::MusicDisabled(_)) {
                        a.music_disabled = v;
                    } else {
                        a.samples_disabled = v;
                    }
                    // SAFETY: C runs only on this thread.
                    unsafe {
                        *rustyrian_oracle::mirror::global_mut(&raw mut b::music_disabled) = a.music_disabled;
                        *rustyrian_oracle::mirror::global_mut(&raw mut b::samples_disabled) = a.samples_disabled;
                    }
                }
            }
            prop_assert_eq!(d.c_calls(), d.rust_calls());
        }
        assert_states(&d);
    }
}

/// Seconds of samples per song for the M2 all-songs test:
/// `RUSTYRIAN_AUDIO_SECONDS` (the nightly job sets 120), else a default that
/// keeps this test binary well under a minute in the test profile.
fn audio_seconds() -> usize {
    std::env::var("RUSTYRIAN_AUDIO_SECONDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8)
}

/// Plays sound `i` on `chan` at `vol` in both engines.
fn sample_both(d: &mut Diff, i: usize, chan: u8, vol: u8) {
    // SAFETY: `soundSamples[i]` is NULL or a live block of
    // `soundSampleCount[i]` samples.
    unsafe {
        let s = rustyrian_oracle::mirror::global(&raw const b::soundSamples)[i];
        let n = rustyrian_oracle::mirror::global(&raw const b::soundSampleCount)[i];
        b::multiSamplePlay(s, n, chan, vol);
    }
    let n = d.game.nortsong.sound_sample_count[i];
    let s = d.game.nortsong.sound_samples[i]
        .clone()
        .map(|buf| ChannelSamples {
            sound: i,
            buf,
            pos: 0,
        });
    ld::multi_sample_play(&mut d.game, s, n, chan, vol);
}

/// Advances C's virtual clock by `ms` and presents, so the fake platform
/// runs the audio callback C registered over the stream bytes due (the
/// real output path, `AUDIO_S16SYS`); then runs the callback the Rust
/// `init_audio` registers over as many bytes. Returns both streams.
fn present_both(d: &Diff, scratch: &mut Vec<i16>, ms: u32) -> (Vec<u8>, Vec<u8>) {
    let screen = [0u8; 320 * 200];
    let palette = [0u32; 256];
    let view = FrameView {
        screen: &screen,
        palette: &palette,
        sim: None,
        snapshot: None,
    };
    let c = session::with(|s| {
        let f = s.inner_mut();
        f.delay(ms);
        f.render_present(RendererId(0), &view);
        f.probe_take_audio()
    })
    .expect("audio device open");
    let mut r = vec![0u8; c.len()];
    ld::stream_callback(&d.game.audio, &d.game.crand, scratch, &mut r);
    (c, r)
}

/// Panics with the song and the first differing sample when the streams
/// differ.
fn assert_streams(c: &[u8], r: &[u8], song: u32, first_sample: usize) {
    if c != r {
        let k = c
            .chunks(2)
            .zip(r.chunks(2))
            .position(|(a, b)| a != b)
            .expect("streams differ");
        panic!(
            "song {song}: first differing sample {} (C {:?}, Rust {:?})",
            first_sample + k,
            &c[2 * k..2 * k + 2],
            &r[2 * k..2 * k + 2]
        );
    }
}

/// Milestone M2: every song of `music.mus`, selected with `play_song` and
/// played through the registered audio callbacks for
/// `RUSTYRIAN_AUDIO_SECONDS` of 44.1 kHz output, is bit-identical in C and
/// Rust, with every sound effect and voice mixed in over it on rotating
/// channels and volumes.
#[test]
fn every_song_is_bit_identical() {
    const CHUNK_MS: u32 = 250;
    let seconds = audio_seconds();
    let start = Instant::now();
    let mut d = fresh(&data_dir());
    assert_eq!(init_both(&mut d), (true, true));
    load_sounds_both(&mut d);
    d.c_calls();
    d.rust_calls();
    let mut scratch = Vec::new();
    // `song_playing` starts at 0, so song 0 goes last (loaded on change).
    let order: Vec<u32> = (1..41).chain([0]).collect();
    let chunks = seconds as u32 * 1000 / CHUNK_MS;
    let mut sound = 0usize;
    for &song in &order {
        assert_eq!(play_both(&mut d, song), (None, Ok(())), "song {song}");
        let mut samples = 0;
        for chunk in 0..chunks {
            if chunk % 2 == 1 {
                let chan = (sound % 8) as u8;
                sample_both(&mut d, sound % 38, chan, (sound / 8 % 8) as u8);
                sound += 1;
            }
            let (c, r) = present_both(&d, &mut scratch, CHUNK_MS);
            assert_streams(&c, &r, song, samples);
            samples += c.len() / 2;
        }
        assert_eq!(samples, seconds * 44100, "song {song}");
        assert_states(&d);
        assert_eq!(d.c_calls(), d.rust_calls(), "song {song}");
    }
    assert!(sound >= 38 || seconds < 4, "every sound was mixed");
    eprintln!(
        "{} songs x {seconds} s: {:.1} s",
        order.len(),
        start.elapsed().as_secs_f64()
    );
}

/// Every sound effect and voice, at every channel volume, alone and over
/// music, mixed to the end through the registered callbacks.
#[test]
fn every_sound_mixes_identically() {
    let mut d = fresh(&data_dir());
    assert_eq!(init_both(&mut d), (true, true));
    load_sounds_both(&mut d);
    let mut scratch = Vec::new();
    for (song, music_volume) in [(0u32, 0u8), (12, 255)] {
        if song != 0 {
            assert_eq!(play_both(&mut d, song), (None, Ok(())));
        }
        for vol in 0..8u8 {
            // SAFETY: plain C call on the test thread.
            unsafe { b::set_volume(music_volume, 255 - vol * 20) };
            ld::set_volume(&mut d.game, music_volume, 255 - vol * 20);
            for i in 0..38 {
                sample_both(&mut d, i, (i % 8) as u8, vol);
                // Mix until the sound has played to its end.
                while audio(&d.game).channel_sample_count.iter().any(|&n| n > 0) {
                    let (c, r) = present_both(&d, &mut scratch, 250);
                    assert_streams(&c, &r, song, 0);
                }
            }
            assert_states(&d);
        }
    }
    assert_eq!(d.c_calls(), d.rust_calls());
}
