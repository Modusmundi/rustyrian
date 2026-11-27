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

use crate::lds_play::LdsState;
use crate::nortsong::SampleBuf;
use crate::opl::OplState;
use serde::{Deserialize, Serialize};

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
