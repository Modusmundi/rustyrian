// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `loudness.c` / `loudness.h`.
//!
//! # Audio-state split
//!
//! Upstream's `audioCallback` runs on SDL's audio thread and touches, directly
//! or through `lds_update` (`lds_play.c`) and `adlib_getsample` (`opl.c`):
//! - in `loudness.c`: `music_stopped`, `music_disabled`, `samples_disabled`,
//!   `musicVolume`, `sampleVolume`, `volumeFactorTable`,
//!   `samplesPerLdsUpdate`(`Frac`), `samplesUntilLdsUpdate`(`Frac`),
//!   `channelSamples`, `channelSampleCount`, `channelVolume`;
//! - every global of `lds_play.c` ([`LdsState`]) and of `opl.c` ([`OplState`]).
//!
//! All of these live in [`AudioState`], held by `Game::audio`
//! (`Arc<Mutex<AudioState>>`), so the callback closure reaches them under one
//! lock. `lds_play::State` and `opl::State` are therefore empty. The remaining
//! `loudness.c` globals (`audioSampleRate`, `song_playing`, `audio_disabled`,
//! `audioDevice`, `songPositions`, `songsCount`, `load_song`'s `first`) are
//! main-thread only and live in [`State`].
//!
//! Upstream reads and writes `music_disabled`/`samples_disabled` (and
//! `playing`/`songlooped`) from the main thread without locking the audio
//! device. The port keeps them as plain fields of `AudioState`: the main thread
//! takes the Rust mutex to access them, which needs no platform call, so the
//! platform call sequence (`lock_audio_device`/`unlock_audio_device` exactly
//! where C calls them) is unchanged.
//!
//! `channelSamples[i]` points into a buffer of `nortsong`'s `soundSamples`;
//! [`ChannelSamples`] holds a clone of that buffer's `Arc` plus the pointer's
//! element offset, so the callback can read the samples without touching
//! `nortsong::State`.

use crate::Game;
use crate::crand;
use crate::file;
use crate::game::{EXIT_FAILURE, Flow, Halt};
use crate::lds_play::{self, LdsState};
use crate::logging::log_fatal;
use crate::nortsong::SampleBuf;
use crate::opl::{self, OplState};
use crate::platform::AudioSpec;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, MutexGuard};

/// `SDL_INIT_AUDIO`.
pub const SDL_INIT_AUDIO: u32 = 0x0000_0010;
/// `SDL_AUDIO_ALLOW_FREQUENCY_CHANGE`.
pub const SDL_AUDIO_ALLOW_FREQUENCY_CHANGE: i32 = 0x0000_0001;
/// `SDL_AUDIO_ALLOW_SAMPLES_CHANGE`.
pub const SDL_AUDIO_ALLOW_SAMPLES_CHANGE: i32 = 0x0000_0008;
/// `AUDIO_S8`.
pub const AUDIO_S8: u16 = 0x8008;
/// `AUDIO_S16SYS`: `AUDIO_S16LSB` or `AUDIO_S16MSB` by host byte order.
pub const AUDIO_S16SYS: u16 = if cfg!(target_endian = "big") {
    0x9010
} else {
    0x8010
};
/// `SDL_LOG_CATEGORY_APPLICATION`, the category of `logError`.
const LOG_CATEGORY: i32 = 0;

/// 44.1 kHz.
pub const OUTPUT_QUALITY: i32 = 4;

/// `volumeRange`, in dB.
pub const VOLUME_RANGE: f32 = 30.0;

/// `ldsUpdate2Rate`: twice the Loudness update rate (69.5 Hz * 2).
pub const LDS_UPDATE2_RATE: i32 = 139;

pub const CHANNEL_COUNT: usize = 8;
pub const CHANNEL_VOLUME_LEVELS: usize = 8;

/// `const Sint16 *channelSamples[i]` (rule R8): the sound buffer it points
/// into and the element offset of the pointer within it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelSamples {
    /// The `nortsong` `soundSamples` index whose buffer this points into.
    pub sound: usize,
    /// That buffer (shared, not copied).
    pub buf: SampleBuf,
    /// Element offset of the pointer from the start of `buf`.
    pub pos: usize,
}

/// Main-thread-only globals of `loudness.c` (rule R3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub audio_sample_rate: i32,
    pub song_playing: u32,
    pub audio_disabled: bool,
    /// `SDL_AudioDeviceID audioDevice`.
    pub audio_device: crate::platform::AudioDeviceId,
    /// `long *songPositions`: `songsCount + 1` entries once loaded; `None` is
    /// NULL.
    pub song_positions: Option<Vec<i64>>,
    pub songs_count: u16,
    /// `load_song`'s local `static bool first = true`.
    pub load_song_first: bool,
}

impl Default for State {
    fn default() -> Self {
        State {
            audio_sample_rate: 0,
            song_playing: 0,
            audio_disabled: false,
            audio_device: 0,
            song_positions: None,
            songs_count: 0,
            load_song_first: true,
        }
    }
}

/// Globals touched by `audioCallback`; shared with the platform's audio
/// thread behind `Game::audio`. See the module docs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioState {
    pub music_stopped: bool,
    pub music_disabled: bool,
    pub samples_disabled: bool,
    pub music_volume: u8,
    pub sample_volume: u8,
    /// Fixed point Q20.12; needs to store `10 * INT16_MIN/MAX`.
    #[serde(with = "crate::serde_big")]
    pub volume_factor_table: [i32; 256],
    pub samples_per_lds_update: i32,
    pub samples_per_lds_update_frac: i32,
    pub samples_until_lds_update: i32,
    pub samples_until_lds_update_frac: i32,
    /// `None` is NULL.
    pub channel_samples: [Option<ChannelSamples>; CHANNEL_COUNT],
    pub channel_sample_count: [usize; CHANNEL_COUNT],
    pub channel_volume: [u8; CHANNEL_COUNT],
    /// The globals of `lds_play.c`.
    pub lds: LdsState,
    /// The globals of `opl.c`.
    pub opl: OplState,
}

impl Default for AudioState {
    fn default() -> Self {
        AudioState {
            music_stopped: true,
            music_disabled: false,
            samples_disabled: false,
            music_volume: 255,
            sample_volume: 255,
            volume_factor_table: [0; 256],
            samples_per_lds_update: 0,
            samples_per_lds_update_frac: 0,
            samples_until_lds_update: 0,
            samples_until_lds_update_frac: 0,
            channel_samples: Default::default(),
            channel_sample_count: [0; CHANNEL_COUNT],
            channel_volume: [0; CHANNEL_COUNT],
            lds: LdsState::default(),
            opl: OplState::default(),
        }
    }
}

/// Locks `Game::audio` (the Rust side of the audio-device lock; C's
/// `SDL_LockAudioDevice` calls stay where upstream makes them).
pub fn audio(g: &Game) -> MutexGuard<'_, AudioState> {
    g.audio.lock().unwrap_or_else(|e| e.into_inner())
}

/// `TO_FIXED(x)`: Q20.12.
fn to_fixed(x: f32) -> i32 {
    (x * (1 << 12) as f32) as i32
}

/// `FIXED_TO_INT(x)`.
fn fixed_to_int(x: i32) -> i32 {
    x >> 12
}

/// `MIN(MAX(INT16_MIN, sample), INT16_MAX)`.
fn clamp16(sample: i32) -> i16 {
    sample.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}

/// The callback `init_audio` registers: [`audio_callback`] over `stream` as
/// `AUDIO_S16SYS` samples, under the `Game::audio` lock. `samples` is a
/// scratch buffer reused across calls.
pub fn stream_callback(
    audio: &Mutex<AudioState>,
    crand: &Mutex<crand::State>,
    samples: &mut Vec<i16>,
    stream: &mut [u8],
) {
    samples.resize(stream.len() / size_of::<i16>(), 0);
    {
        let mut a = audio.lock().unwrap_or_else(|e| e.into_inner());
        audio_callback(&mut a, crand, samples);
    }
    for (b, s) in stream.as_chunks_mut::<2>().0.iter_mut().zip(samples.iter()) {
        b.copy_from_slice(&s.to_ne_bytes());
    }
}

pub fn init_audio(g: &mut Game) -> bool {
    if g.loudness.audio_disabled {
        return false;
    }

    // `ask`'s unset fields are zero (the oracle zero-initialises locals).
    let ask = AudioSpec {
        freq: 11025 * OUTPUT_QUALITY,
        format: AUDIO_S16SYS,
        channels: 1,
        silence: 0,
        samples: (256 * OUTPUT_QUALITY) as u16, // ~23 ms
        size: 0,
    };
    let mut got = AudioSpec::default();

    if g.platform.init_sub_system(SDL_INIT_AUDIO) != 0 {
        let message = format!("Failed to initialize SDL audio: {}", g.platform.get_error());
        g.platform.log_error(LOG_CATEGORY, &message);

        g.loudness.audio_disabled = true;
        return false;
    }

    let allowed_changes = SDL_AUDIO_ALLOW_FREQUENCY_CHANGE | SDL_AUDIO_ALLOW_SAMPLES_CHANGE;
    let callback = {
        let audio = Arc::clone(&g.audio);
        let crand = Arc::clone(&g.crand);
        // Reused across calls: the callback's samples, as `Sint16`.
        let mut samples: Vec<i16> = Vec::new();
        Box::new(move |stream: &mut [u8]| {
            stream_callback(&audio, &crand, &mut samples, stream);
        })
    };
    g.loudness.audio_device =
        g.platform
            .open_audio_device(None, 0, &ask, callback, &mut got, allowed_changes);

    if g.loudness.audio_device == 0 {
        let message = format!("Failed to open audio device: {}", g.platform.get_error());
        g.platform.log_error(LOG_CATEGORY, &message);

        g.platform.quit_sub_system(SDL_INIT_AUDIO);

        g.loudness.audio_disabled = true;
        return false;
    }

    g.loudness.audio_sample_rate = got.freq;
    let rate = g.loudness.audio_sample_rate;

    let mut a = audio(g);
    a.samples_per_lds_update = 2 * (rate / LDS_UPDATE2_RATE);
    a.samples_per_lds_update_frac = 2 * (rate % LDS_UPDATE2_RATE);

    a.volume_factor_table[0] = 0;
    for i in 1..256usize {
        a.volume_factor_table[i] = to_fixed(libm::powf(
            10.0,
            (255 - i) as f32 * (-VOLUME_RANGE / (20.0f32 * 255.0)),
        ));
    }

    opl::adlib_init(&mut a.opl, rate as u32); // opl_init()
    drop(a);

    let device = g.loudness.audio_device;
    g.platform.pause_audio_device(device, 0); // unpause

    true
}

/// `audioCallback(userdata, stream, size)` over the stream's
/// `size / sizeof (Sint16)` samples; `crand` is the libc `rand()` stream
/// the OPL percussion draws from.
#[doc(alias = "audioCallback")]
pub fn audio_callback(a: &mut AudioState, crand: &Mutex<crand::State>, samples: &mut [i16]) {
    let samples_count = samples.len();

    if !a.music_disabled && !a.music_stopped {
        let mut remaining = 0usize;
        let mut remaining_count = samples_count as i32;
        while remaining_count > 0 {
            if a.samples_until_lds_update == 0 {
                lds_play::lds_update(&mut a.lds, &mut a.opl);

                // Accumulate the fractional samples per update until they
                // amount to a whole sample.
                a.samples_until_lds_update += a.samples_per_lds_update;
                a.samples_until_lds_update_frac += a.samples_per_lds_update_frac;
                if a.samples_until_lds_update_frac >= LDS_UPDATE2_RATE {
                    a.samples_until_lds_update += 1;
                    a.samples_until_lds_update_frac -= LDS_UPDATE2_RATE;
                }
            }

            let count = a.samples_until_lds_update.min(remaining_count);

            // opl_update(remaining, count)
            opl::adlib_getsample(
                &mut a.opl,
                crand,
                &mut samples[remaining..],
                count as opl::Bits,
            );

            remaining += count as usize;
            remaining_count -= count;

            a.samples_until_lds_update -= count;
        }
    } else {
        samples.fill(0);
    }

    // The OPL emulator is too quiet: double the music.
    let music_volume_factor = a.volume_factor_table[usize::from(a.music_volume)].wrapping_mul(2);

    if a.samples_disabled && !a.music_disabled {
        // Mix music
        for s in samples.iter_mut() {
            let sample = fixed_to_int(i32::from(*s).wrapping_mul(music_volume_factor));
            *s = clamp16(sample);
        }
    } else if !a.samples_disabled {
        let sample_volume_factor = a.volume_factor_table[usize::from(a.sample_volume)];
        let sample_volume_factors: [i32; CHANNEL_VOLUME_LEVELS] = std::array::from_fn(|i| {
            sample_volume_factor.wrapping_mul(i as i32 + 1) / CHANNEL_VOLUME_LEVELS as i32
        });

        // Mix music and channels. C walks the samples and, for each, adds
        // the playing channels in order; the same sums, channel by channel
        // over a block of samples.
        const MIX_BLOCK: usize = 1024;
        for block in samples.chunks_mut(MIX_BLOCK) {
            let mut mix = [0i32; MIX_BLOCK];
            let mix = &mut mix[..block.len()];
            for (m, &s) in mix.iter_mut().zip(block.iter()) {
                *m = i32::from(s).wrapping_mul(music_volume_factor);
            }

            for i in 0..CHANNEL_COUNT {
                let n = a.channel_sample_count[i].min(block.len());
                if n == 0 {
                    continue;
                }
                let factor = sample_volume_factors[usize::from(a.channel_volume[i])];
                let ch = a.channel_samples[i]
                    .as_mut()
                    .expect("a playing channel has samples");
                for (m, &v) in mix[..n].iter_mut().zip(&ch.buf.0[ch.pos..ch.pos + n]) {
                    *m = m.wrapping_add(i32::from(v).wrapping_mul(factor));
                }

                ch.pos += n;
                a.channel_sample_count[i] -= n;
            }

            for (s, &m) in block.iter_mut().zip(mix.iter()) {
                *s = clamp16(fixed_to_int(m));
            }
        }
    }
}

pub fn deinit_audio(g: &mut Game) {
    if g.loudness.audio_disabled {
        return;
    }

    if g.loudness.audio_device != 0 {
        let device = g.loudness.audio_device;
        g.platform.pause_audio_device(device, 1); // pause
        g.platform.close_audio_device(device);
        g.loudness.audio_device = 0;
    }

    g.platform.quit_sub_system(SDL_INIT_AUDIO);

    let mut a = audio(g);
    a.channel_sample_count = [0; CHANNEL_COUNT];

    lds_play::lds_free(&mut a.lds);
}

/// `load_song(song_num)` (FKA NortSong.loadSong).
pub fn load_song(g: &mut Game, song_num: u32) -> Flow {
    let filename = "music.mus";

    let mut f = file::data_file_open(g, filename, "rb");
    if f.error {
        let message = format!(
            "Failed to open file '{}': {}",
            filename,
            file::file_get_error(&f)
        );
        log_fatal(g, &message);
        return Err(Halt { code: EXIT_FAILURE });
    }

    if g.loudness.load_song_first {
        g.loudness.load_song_first = false;

        g.loudness.songs_count = file::file_read_u16(&mut f);

        let songs_count = usize::from(g.loudness.songs_count);
        let mut song_positions = vec![0i64; songs_count + 1];

        for p in song_positions.iter_mut().take(songs_count) {
            *p = i64::from(file::file_read_u32(&mut f));
        }

        song_positions[songs_count] = file::file_get_length(&mut f);
        g.loudness.song_positions = Some(song_positions);

        if f.error {
            let message = format!(
                "Failed to read from file '{}': {}",
                filename,
                file::file_get_error(&f)
            );
            g.platform.log_error(LOG_CATEGORY, &message);
        }
    }

    if song_num >= u32::from(g.loudness.songs_count) {
        let message = format!("Attempted to load song {song_num}, which does not exist.");
        g.platform.log_error(LOG_CATEGORY, &message);
        return Ok(());
    }

    let positions = g
        .loudness
        .song_positions
        .as_ref()
        .expect("read on the first call");
    let position = positions[song_num as usize];
    let end_position = positions[song_num as usize + 1];
    let size = if end_position > position {
        (end_position - position) as usize
    } else {
        0
    };

    file::file_set_position(&mut f, position);

    let mut data = vec![0u8; size];
    file::file_read_exactly(&mut f, &mut data);

    if f.error {
        let message = format!(
            "Failed to read from file '{}': {}",
            filename,
            file::file_get_error(&f)
        );
        g.platform.log_error(LOG_CATEGORY, &message);
    }

    file::file_close(&mut f);

    let rate = g.loudness.audio_sample_rate;
    let loaded = {
        let a = &mut *audio(g);
        lds_play::lds_load(&mut a.lds, &mut a.opl, rate, &data)
    };
    if !loaded {
        let message = format!("Failed to load song {song_num}.");
        g.platform.log_error(LOG_CATEGORY, &message);
    }

    Ok(())
}

/// `play_song(song_num)` (FKA NortSong.playSong).
pub fn play_song(g: &mut Game, song_num: u32) -> Flow {
    if g.loudness.audio_disabled {
        return Ok(());
    }

    let device = g.loudness.audio_device;
    if song_num != g.loudness.song_playing {
        g.platform.lock_audio_device(device);

        audio(g).music_stopped = true;

        g.platform.unlock_audio_device(device);

        load_song(g, song_num)?;

        g.loudness.song_playing = song_num;
    }

    g.platform.lock_audio_device(device);

    audio(g).music_stopped = false;

    g.platform.unlock_audio_device(device);

    Ok(())
}

/// `restart_song()` (FKA Player.selectSong(1)).
pub fn restart_song(g: &mut Game) {
    if g.loudness.audio_disabled {
        return;
    }

    let device = g.loudness.audio_device;
    g.platform.lock_audio_device(device);

    let rate = g.loudness.audio_sample_rate;
    {
        let a = &mut *audio(g);
        lds_play::lds_rewind(&mut a.lds, &mut a.opl, rate);

        a.music_stopped = false;
    }

    g.platform.unlock_audio_device(device);
}

/// `stop_song()` (FKA Player.selectSong(0)).
pub fn stop_song(g: &mut Game) {
    if g.loudness.audio_disabled {
        return;
    }

    let device = g.loudness.audio_device;
    g.platform.lock_audio_device(device);

    audio(g).music_stopped = true;

    g.platform.unlock_audio_device(device);
}

/// `fade_song()` (FKA Player.selectSong($C001)).
pub fn fade_song(g: &mut Game) {
    if g.loudness.audio_disabled {
        return;
    }

    let device = g.loudness.audio_device;
    g.platform.lock_audio_device(device);

    lds_play::lds_fade(&mut audio(g).lds, 1);

    g.platform.unlock_audio_device(device);
}

/// `set_volume(musicVolume, sampleVolume)` (FKA NortSong.setVol and
/// Player.setVol).
pub fn set_volume(g: &mut Game, music_volume: u8, sample_volume: u8) {
    if g.loudness.audio_disabled {
        return;
    }

    let device = g.loudness.audio_device;
    g.platform.lock_audio_device(device);

    {
        let mut a = audio(g);
        a.music_volume = music_volume;
        a.sample_volume = sample_volume;
    }

    g.platform.unlock_audio_device(device);
}

/// `multiSamplePlay(samples, sampleCount, chan, vol)` (FKA
/// Player.multiSamplePlay); `samples` is the `soundSamples` buffer the
/// pointer starts (`None` for NULL).
#[doc(alias = "multiSamplePlay")]
pub fn multi_sample_play(
    g: &mut Game,
    samples: Option<ChannelSamples>,
    sample_count: usize,
    chan: u8,
    vol: u8,
) {
    debug_assert!(usize::from(chan) < CHANNEL_COUNT);
    debug_assert!(usize::from(vol) < CHANNEL_VOLUME_LEVELS);

    if g.loudness.audio_disabled || audio(g).samples_disabled {
        return;
    }

    let device = g.loudness.audio_device;
    g.platform.lock_audio_device(device);

    {
        let mut a = audio(g);
        let chan = usize::from(chan);
        a.channel_samples[chan] = samples;
        a.channel_sample_count[chan] = sample_count;
        a.channel_volume[chan] = vol;
    }

    g.platform.unlock_audio_device(device);
}
