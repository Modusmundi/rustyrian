// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `nortsong.c` / `nortsong.h`.

use crate::Game;
use crate::file;
use crate::game::{EXIT_FAILURE, Flow, Halt};
use crate::logging::log_fatal;
use crate::loudness::{self, AUDIO_S8, AUDIO_S16SYS, ChannelSamples};
use crate::platform::AudioCvt;
use crate::sndmast::{S_CLINK, SFX_COUNT, SOUND_COUNT, VOICE_COUNT};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::sync::Arc;

/// `SDL_LOG_CATEGORY_APPLICATION`, the category of `logError`.
const LOG_CATEGORY: i32 = 0;

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

/// `((Uint64)speed << 10) * 1000 * 88 * 3 / 315000000`.
fn frame_period_of(speed: u16) -> u16 {
    ((u64::from(speed) << 10) * 1000 * 88 * 3 / 315_000_000) as u16
}

/// `SDL_GetTicks() << 10`: now in UQ22.10 milliseconds.
fn now(g: &mut Game) -> u32 {
    g.platform.get_ticks() << 10
}

/// `setFrameSpeed(speed)` (FKA NortSong.speed and NortSong.setTimerInt).
#[doc(alias = "setFrameSpeed")]
pub fn set_frame_speed(g: &mut Game, speed: u16) {
    g.nortsong.frame_speed = speed;
    g.nortsong.frame_period = frame_period_of(speed);

    let now = now(g);
    g.nortsong.frame_count_end = now;
}

/// The body shared by `setFrameCount` and `setFrameCount2`: keeps the
/// partial timer period that has already elapsed, then adds `frame_count`
/// periods.
fn advance_end(now: u32, end: u32, frame_period: u16, frame_count: u16) -> u32 {
    let period = i32::from(frame_period);
    // Guarded per divergence D-UB-TIMER-1: with a zero period the end is
    // kept, as the oracle computes it.
    if period == 0 {
        return end;
    }
    let diff = now.wrapping_sub(end) as i32;
    let mut end = end;
    if diff >= period {
        end = now.wrapping_sub((diff as u32) % period as u32);
    } else if diff.wrapping_neg() >= period {
        end = now.wrapping_add((diff.wrapping_neg() as u32) % period as u32);
    }

    end.wrapping_add(i32::from(frame_count).wrapping_mul(period) as u32)
}

/// `setFrameCount(frameCount)` (FKA NortSong.frameCount).
#[doc(alias = "setFrameCount")]
pub fn set_frame_count(g: &mut Game, frame_count: u16) {
    let now = now(g);
    let n = &mut g.nortsong;
    n.frame_count_end = advance_end(now, n.frame_count_end, n.frame_period, frame_count);
}

/// `setFrameCount2(frameCount2)` (FKA NortSong.frameCount2).
#[doc(alias = "setFrameCount2")]
pub fn set_frame_count2(g: &mut Game, frame_count2: u16) {
    let now = now(g);
    let n = &mut g.nortsong;
    n.frame_count2_end = advance_end(now, n.frame_count2_end, n.frame_period, frame_count2);
}

/// Whole milliseconds (rounded) from `now` until `end`, or `None` when it
/// has passed.
fn ms_until(now: u32, end: u32) -> Option<u32> {
    const HALF: u32 = 1 << 9;
    let diff = end.wrapping_sub(now) as i32;
    (diff >= 0).then(|| (diff as u32).wrapping_add(HALF) >> 10)
}

#[doc(alias = "getFrameCountTicks")]
pub fn get_frame_count_ticks(g: &mut Game) -> u32 {
    let now = now(g);
    ms_until(now, g.nortsong.frame_count_end).unwrap_or(0)
}

#[doc(alias = "getFrameCount2Ticks")]
pub fn get_frame_count2_ticks(g: &mut Game) -> u32 {
    let now = now(g);
    ms_until(now, g.nortsong.frame_count2_end).unwrap_or(0)
}

#[doc(alias = "delayUntilElapsed")]
pub fn delay_until_elapsed(g: &mut Game) {
    let now = now(g);
    if let Some(ms) = ms_until(now, g.nortsong.frame_count_end) {
        g.platform.delay(ms);
    }
}

/// `loadSounds(soundsOffset, soundsCount, filename, trim, cvt)`; the
/// converter's buffer (`cvt->buf`) is allocated and freed here, as upstream
/// does.
#[doc(alias = "loadSounds")]
pub fn load_sounds(
    g: &mut Game,
    sounds_offset: usize,
    sounds_count: usize,
    filename: &str,
    trim: bool,
    cvt: &mut AudioCvt,
) -> Flow {
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

    let mut max_size = 0usize;

    // number of sounds
    let count = file::file_read_u16(&mut f);
    // Upstream asserts `count == soundsCount`; it tests the file contents,
    // so (R10) it is not a debug assertion here, matching the NDEBUG build.
    let count = usize::from(count).min(sounds_count);

    let mut positions = vec![0i64; count + 1];

    for p in positions.iter_mut().take(count) {
        *p = i64::from(file::file_read_u32(&mut f));
    }

    positions[count] = file::file_get_length(&mut f);

    let size_of_sound = |i: usize| {
        let (position, end_position) = (positions[i], positions[i + 1]);
        let size = if end_position > position {
            (end_position - position) as usize
        } else {
            0
        };

        // Voice sounds end with some bad data.
        if trim { size.saturating_sub(100) } else { size }
    };

    for i in 0..count {
        max_size = max_size.max(size_of_sound(i));
    }

    let len_mult = cvt.len_mult as usize;
    let mut buf = vec![0u8; max_size * len_mult];

    #[allow(clippy::needless_range_loop)] // `i` also indexes `soundSamples`
    for i in 0..count {
        let size = size_of_sound(i);

        debug_assert!(size <= max_size);

        file::file_set_position(&mut f, positions[i]);

        file::file_read_exactly(&mut f, &mut buf[..size]);
        cvt.len = size as i32;

        if g.platform.convert_audio(cvt, &mut buf[..size * len_mult]) != 0 {
            let message = format!("Failed to convert audio: {}", g.platform.get_error());
            g.platform.log_error(LOG_CATEGORY, &message);
            continue;
        }

        let len_cvt = cvt.len_cvt as usize;
        let samples: Arc<[i16]> = buf[..len_cvt]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&b| i16::from_ne_bytes(b))
            .collect();
        g.nortsong.sound_samples[sounds_offset + i] = Some(SampleBuf(samples));
        g.nortsong.sound_sample_count[sounds_offset + i] = len_cvt / size_of::<i16>();
    }

    if f.error {
        let message = format!(
            "Failed to read from file '{}': {}",
            filename,
            file::file_get_error(&f)
        );
        g.platform.log_error(LOG_CATEGORY, &message);
    }

    file::file_close(&mut f);

    Ok(())
}

#[doc(alias = "loadSndFile")]
pub fn load_snd_file(g: &mut Game, xmas: bool) -> Flow {
    g.nortsong.sound_samples = std::array::from_fn(|_| None);
    g.nortsong.sound_sample_count = [0; SOUND_COUNT];

    // A converter to the output sample format and rate.
    let mut cvt = AudioCvt::default();
    let rate = g.loudness.audio_sample_rate;
    if g.platform
        .build_audio_cvt(&mut cvt, AUDIO_S8, 1, 11025, AUDIO_S16SYS, 1, rate)
        < 0
    {
        let message = format!(
            "Failed to build audio converter: {}",
            g.platform.get_error()
        );
        g.platform.log_error(LOG_CATEGORY, &message);
        return Ok(());
    }

    let sfx_filename = "tyrian.snd";
    load_sounds(g, 0, SFX_COUNT, sfx_filename, false, &mut cvt)?;

    let voice_filename = if xmas { "voicesc.snd" } else { "voices.snd" };
    load_sounds(g, SFX_COUNT, VOICE_COUNT, voice_filename, true, &mut cvt)
}

/// `JE_playSampleNum(samplenum)`; `samplenum` is 1-based. A number outside
/// `1..=SOUND_COUNT` queues silence (divergence D-UB-SND-1).
#[doc(alias = "JE_playSampleNum")]
pub fn je_play_sample_num(g: &mut Game, samplenum: u8) {
    let i = usize::from(samplenum).wrapping_sub(1);
    let samples = g
        .nortsong
        .sound_samples
        .get(i)
        .cloned()
        .flatten()
        .map(|buf| ChannelSamples {
            sound: i,
            buf,
            pos: 0,
        });
    let count = g.nortsong.sound_sample_count.get(i).copied().unwrap_or(0);
    loudness::multi_sample_play(g, samples, count, 0, FX_PLAY_VOL as u8);
}

/// `JE_changeVolume(&music, music_delta, &sample, sample_delta)`.
#[doc(alias = "JE_changeVolume")]
pub fn je_change_volume(
    g: &mut Game,
    music: &mut u16,
    music_delta: i32,
    sample: &mut u16,
    sample_delta: i32,
) {
    let mut music_temp = i32::from(*music) + music_delta;
    let mut sample_temp = i32::from(*sample) + sample_delta;

    if music_delta != 0 {
        if music_temp > 255 {
            music_temp = 255;
            je_play_sample_num(g, S_CLINK);
        } else if music_temp < 0 {
            music_temp = 0;
            je_play_sample_num(g, S_CLINK);
        }
    }

    if sample_delta != 0 {
        if sample_temp > 255 {
            sample_temp = 255;
            je_play_sample_num(g, S_CLINK);
        } else if sample_temp < 0 {
            sample_temp = 0;
            je_play_sample_num(g, S_CLINK);
        }
    }

    *music = music_temp as u16;
    *sample = sample_temp as u16;

    loudness::set_volume(g, *music as u8, *sample as u8);
}
