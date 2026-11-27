//! Mirrors of `rustyrian_core::loudness::{State, AudioState}` <-> the C
//! globals of `loudness.c` (and, through `AudioState`, of `lds_play.c` and
//! `opl.c`).
//!
//! `channelSamples[i]` points into a `soundSamples` buffer (`nortsong.c`); it
//! maps to the index of that buffer, a copy of its contents and the element
//! offset. A pointer that lies in no current `soundSamples` buffer while its
//! `channelSampleCount` is 0 (a finished channel whose buffer has since been
//! freed or reloaded) is never dereferenced by C and loads as `None`.

use super::lds_play::{lds_from_c, lds_to_c};
use super::lvllib::{heap_from_c, heap_to_c};
use super::opl::{opl_from_c, opl_to_c};
use super::video::sv;
use super::{Mirror, global, global_mut};
use crate::bindings as b;
use rustyrian_core::loudness::{AudioState, CHANNEL_COUNT, ChannelSamples, State};
use rustyrian_core::nortsong::SampleBuf;
use std::ffi::c_long;

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make; each `sv`
        // type is the C type of that static; `songPositions` is NULL or
        // malloc'd by `load_song`.
        unsafe {
            self.audio_sample_rate = *global(&raw const b::audioSampleRate);
            self.song_playing = *global(&raw const b::song_playing);
            self.audio_disabled = *global(&raw const b::audio_disabled);
            self.audio_device = *sv::<b::SDL_AudioDeviceID>(b::ot_sv_loudness_audioDevice);
            #[allow(clippy::useless_conversion)]
            let song_positions =
                heap_from_c(*sv::<*mut c_long>(b::ot_sv_loudness_songPositions), |&x| {
                    i64::from(x)
                });
            self.song_positions = song_positions;
            self.songs_count = *sv::<u16>(b::ot_sv_loudness_songsCount);
            self.load_song_first = *sv::<bool>(b::ot_sv_loudness_load_song_first);
        }
    }

    fn store_to_c(&self) {
        // SAFETY: as above; `songPositions` is owned by its global.
        unsafe {
            *global_mut(&raw mut b::audioSampleRate) = self.audio_sample_rate;
            *global_mut(&raw mut b::song_playing) = self.song_playing;
            *global_mut(&raw mut b::audio_disabled) = self.audio_disabled;
            *sv::<b::SDL_AudioDeviceID>(b::ot_sv_loudness_audioDevice) = self.audio_device;
            heap_to_c(
                sv::<*mut c_long>(b::ot_sv_loudness_songPositions),
                self.song_positions.as_deref(),
                |&x| x as c_long,
            );
            *sv::<u16>(b::ot_sv_loudness_songsCount) = self.songs_count;
            *sv::<bool>(b::ot_sv_loudness_load_song_first) = self.load_song_first;
        }
    }
}

/// Maps a C `channelSamples[i]` pointer onto the `soundSamples` buffer it
/// points into.
///
/// # Safety
/// C globals are quiescent; `soundSamples` entries are NULL or live blocks.
unsafe fn channel_from_c(p: *const i16, count: usize) -> Option<ChannelSamples> {
    if p.is_null() {
        return None;
    }
    // SAFETY: per the caller.
    let (sounds, counts) = unsafe {
        (
            global(&raw const b::soundSamples),
            global(&raw const b::soundSampleCount),
        )
    };
    // A pointer may equal both the end of one buffer and the start of the next
    // (adjacent blocks). A playing channel (`count > 0`) lies strictly inside
    // its buffer; a finished one (`count == 0`) normally sits one past the end
    // of the buffer it played, so prefer that match for it.
    let at = p as usize;
    let find = |at_end: bool| {
        sounds
            .iter()
            .zip(counts)
            .enumerate()
            .find_map(|(k, (&s, &n))| {
                let base = s as usize;
                let end = base + n * size_of::<i16>();
                let hit = if at_end {
                    at == end
                } else {
                    at >= base && at < end
                };
                (!s.is_null() && hit).then(|| (k, (at - base) / size_of::<i16>()))
            })
    };
    let found = if count == 0 {
        find(true).or_else(|| find(false))
    } else {
        find(false)
    };
    match found {
        Some((sound, pos)) => {
            // SAFETY: `soundSamples[sound]` is a live block.
            let buf = unsafe { heap_from_c(sounds[sound], |&x| x) }.expect("non-NULL");
            Some(ChannelSamples {
                sound,
                buf: SampleBuf(buf.into()),
                pos,
            })
        }
        None => {
            assert_eq!(
                count, 0,
                "playing channel points outside every soundSamples buffer"
            );
            None
        }
    }
}

fn channel_to_c(r: Option<&ChannelSamples>) -> *const i16 {
    match r {
        None => std::ptr::null(),
        Some(c) => {
            // SAFETY: C globals are quiescent; only an address is formed.
            let base = unsafe { global(&raw const b::soundSamples) }[c.sound];
            base.cast_const().wrapping_add(c.pos)
        }
    }
}

impl Mirror for AudioState {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make; each `sv`
        // type is the C type of that static.
        unsafe {
            self.music_stopped = *sv::<bool>(b::ot_sv_loudness_music_stopped);
            self.music_disabled = *global(&raw const b::music_disabled);
            self.samples_disabled = *global(&raw const b::samples_disabled);
            self.music_volume = *sv::<u8>(b::ot_sv_loudness_musicVolume);
            self.sample_volume = *sv::<u8>(b::ot_sv_loudness_sampleVolume);
            self.volume_factor_table = *sv::<[i32; 256]>(b::ot_sv_loudness_volumeFactorTable);
            self.samples_per_lds_update = *sv::<i32>(b::ot_sv_loudness_samplesPerLdsUpdate);
            self.samples_per_lds_update_frac =
                *sv::<i32>(b::ot_sv_loudness_samplesPerLdsUpdateFrac);
            self.samples_until_lds_update = *sv::<i32>(b::ot_sv_loudness_samplesUntilLdsUpdate);
            self.samples_until_lds_update_frac =
                *sv::<i32>(b::ot_sv_loudness_samplesUntilLdsUpdateFrac);
            self.channel_sample_count =
                *sv::<[usize; CHANNEL_COUNT]>(b::ot_sv_loudness_channelSampleCount);
            let samples = *sv::<[*const i16; CHANNEL_COUNT]>(b::ot_sv_loudness_channelSamples);
            for (i, &p) in samples.iter().enumerate() {
                self.channel_samples[i] = channel_from_c(p, self.channel_sample_count[i]);
            }
            self.channel_volume = *sv::<[u8; CHANNEL_COUNT]>(b::ot_sv_loudness_channelVolume);
        }
        self.lds = lds_from_c();
        self.opl = opl_from_c();
    }

    fn store_to_c(&self) {
        // SAFETY: as above. Run after the nortsong mirror, so channel
        // pointers target the current `soundSamples` buffers.
        unsafe {
            *sv::<bool>(b::ot_sv_loudness_music_stopped) = self.music_stopped;
            *global_mut(&raw mut b::music_disabled) = self.music_disabled;
            *global_mut(&raw mut b::samples_disabled) = self.samples_disabled;
            *sv::<u8>(b::ot_sv_loudness_musicVolume) = self.music_volume;
            *sv::<u8>(b::ot_sv_loudness_sampleVolume) = self.sample_volume;
            *sv::<[i32; 256]>(b::ot_sv_loudness_volumeFactorTable) = self.volume_factor_table;
            *sv::<i32>(b::ot_sv_loudness_samplesPerLdsUpdate) = self.samples_per_lds_update;
            *sv::<i32>(b::ot_sv_loudness_samplesPerLdsUpdateFrac) =
                self.samples_per_lds_update_frac;
            *sv::<i32>(b::ot_sv_loudness_samplesUntilLdsUpdate) = self.samples_until_lds_update;
            *sv::<i32>(b::ot_sv_loudness_samplesUntilLdsUpdateFrac) =
                self.samples_until_lds_update_frac;
            *sv::<[*const i16; CHANNEL_COUNT]>(b::ot_sv_loudness_channelSamples) =
                std::array::from_fn(|i| channel_to_c(self.channel_samples[i].as_ref()));
            *sv::<[usize; CHANNEL_COUNT]>(b::ot_sv_loudness_channelSampleCount) =
                self.channel_sample_count;
            *sv::<[u8; CHANNEL_COUNT]>(b::ot_sv_loudness_channelVolume) = self.channel_volume;
        }
        lds_to_c(&self.lds);
        opl_to_c(&self.opl);
    }
}
