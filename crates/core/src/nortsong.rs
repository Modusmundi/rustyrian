// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `nortsong.c` / `nortsong.h`.

use crate::sndmast::SOUND_COUNT;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::sync::Arc;

/// A sample buffer upstream mallocs (`Sint16 *soundSamples[i]`).
///
/// Shared, immutable after loading: the mixer channels of
/// `loudness::AudioState` hold clones of the same `Arc` where C holds pointers
/// into the buffer, so playing a sound never copies it. Equality and serde
/// use the contents.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SampleBuf(pub Arc<[i16]>);

impl Serialize for SampleBuf {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}

impl<'de> Deserialize<'de> for SampleBuf {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(SampleBuf(Vec::<i16>::deserialize(d)?.into()))
    }
}

/// `fxPlayVol`.
pub const FX_PLAY_VOL: u16 = 4;

/// Initial `frameSpeed` (PIT divisor).
pub const FRAME_SPEED_INIT: u16 = 0x4300;

/// Initial `framePeriod`: `((Uint64)0x4300 << 10) * 1000 * 88 * 3 / 315000000`,
/// fixed point UQ6.10 milliseconds. The frequency of the x86 programmable
/// interval timer is (315 / 88 / 3) MHz.
pub const FRAME_PERIOD_INIT: u16 =
    (((FRAME_SPEED_INIT as u64) << 10) * 1000 * 88 * 3 / 315_000_000) as u16;

/// Mutable globals of this module (rule R3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub frame_count_max: u16,
    /// `[1..soundnum + 9]` (FKA `digiFx`). `None` is NULL.
    #[serde(with = "crate::serde_big")]
    pub sound_samples: [Option<SampleBuf>; SOUND_COUNT],
    /// FKA `fxSize`.
    #[serde(with = "crate::serde_big")]
    pub sound_sample_count: [usize; SOUND_COUNT],
    pub tyr_music_volume: u16,
    pub fx_volume: u16,
    pub temp_volume: u16,
    /// File static; PIT divisor.
    pub frame_speed: u16,
    /// File static; fixed point UQ6.10 in milliseconds.
    pub frame_period: u16,
    /// File static; fixed point UQ22.10 in milliseconds.
    pub frame_count_end: u32,
    /// File static; fixed point UQ22.10 in milliseconds.
    pub frame_count2_end: u32,
}

impl Default for State {
    fn default() -> Self {
        State {
            frame_count_max: 0,
            sound_samples: std::array::from_fn(|_| None),
            sound_sample_count: [0; SOUND_COUNT],
            tyr_music_volume: 0,
            fx_volume: 0,
            temp_volume: 0,
            frame_speed: FRAME_SPEED_INIT,
            frame_period: FRAME_PERIOD_INIT,
            frame_count_end: 0,
            frame_count2_end: 0,
        }
    }
}
