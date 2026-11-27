//! Mirror of `rustyrian_core::nortsong::State` <-> the C globals of
//! `nortsong.c`. Sample buffers are mirrored as their contents.

use super::lvllib::{heap_from_c, heap_to_c};
use super::video::sv;
use super::{Mirror, global, global_mut};
use crate::bindings as b;
use rustyrian_core::nortsong::{SampleBuf, State};

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make; every
        // `soundSamples` entry is NULL or a block malloc'd by `loadSounds`.
        unsafe {
            self.frame_count_max = *global(&raw const b::frameCountMax);
            let s = global(&raw const b::soundSamples);
            for (d, &p) in self.sound_samples.iter_mut().zip(s.iter()) {
                *d = heap_from_c(p, |&x| x).map(|v| SampleBuf(v.into()));
            }
            self.sound_sample_count = *global(&raw const b::soundSampleCount);
            self.tyr_music_volume = *global(&raw const b::tyrMusicVolume);
            self.fx_volume = *global(&raw const b::fxVolume);
            self.temp_volume = *global(&raw const b::tempVolume);
            self.frame_speed = *sv::<u16>(b::ot_sv_nortsong_frameSpeed);
            self.frame_period = *sv::<u16>(b::ot_sv_nortsong_framePeriod);
            self.frame_count_end = *sv::<u32>(b::ot_sv_nortsong_frameCountEnd);
            self.frame_count2_end = *sv::<u32>(b::ot_sv_nortsong_frameCount2End);
        }
    }

    fn store_to_c(&self) {
        // SAFETY: as above; each buffer is owned by its `soundSamples` slot
        // (mixer channels pointing into it are re-stored by the loudness
        // `AudioState` mirror, which runs after this one).
        unsafe {
            *global_mut(&raw mut b::frameCountMax) = self.frame_count_max;
            let s = global_mut(&raw mut b::soundSamples);
            for (slot, r) in s.iter_mut().zip(self.sound_samples.iter()) {
                heap_to_c(slot, r.as_ref().map(|r| &*r.0), |&x| x);
            }
            *global_mut(&raw mut b::soundSampleCount) = self.sound_sample_count;
            *global_mut(&raw mut b::tyrMusicVolume) = self.tyr_music_volume;
            *global_mut(&raw mut b::fxVolume) = self.fx_volume;
            *global_mut(&raw mut b::tempVolume) = self.temp_volume;
            *sv::<u16>(b::ot_sv_nortsong_frameSpeed) = self.frame_speed;
            *sv::<u16>(b::ot_sv_nortsong_framePeriod) = self.frame_period;
            *sv::<u32>(b::ot_sv_nortsong_frameCountEnd) = self.frame_count_end;
            *sv::<u32>(b::ot_sv_nortsong_frameCount2End) = self.frame_count2_end;
        }
    }
}
