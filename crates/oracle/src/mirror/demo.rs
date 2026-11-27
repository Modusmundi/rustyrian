//! Mirror of `rustyrian_core::demo::State` <-> the C globals of `demo.c`.
//!
//! `demoFile` goes through `mirror::file`, which mirrors the `FILE *` by
//! the file it reads and its position.

use super::file::{file_from_c, file_to_c};
use super::video::sv;
use super::{Mirror, global, global_mut};
use crate::bindings as b;
use rustyrian_core::demo::State;

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make; each
        // static is read through its own C type.
        unsafe {
            self.play_demo = *global(&raw const b::playDemo);
            self.record_demo = *global(&raw const b::recordDemo);
            self.stopped_demo = *global(&raw const b::stoppedDemo);
            self.demo_num = *sv::<u8>(b::ot_sv_demo_demoNum);
            self.demo_file = file_from_c(sv::<b::File>(b::ot_sv_demo_demoFile));
            self.demo_keys = *sv::<u8>(b::ot_sv_demo_demoKeys);
            self.demo_keys_wait = *sv::<u16>(b::ot_sv_demo_demoKeysWait);
        }
    }

    fn store_to_c(&self) {
        // SAFETY: as in `load_from_c`.
        unsafe {
            *global_mut(&raw mut b::playDemo) = self.play_demo;
            *global_mut(&raw mut b::recordDemo) = self.record_demo;
            *global_mut(&raw mut b::stoppedDemo) = self.stopped_demo;
            *sv::<u8>(b::ot_sv_demo_demoNum) = self.demo_num;
            file_to_c(&self.demo_file, sv::<b::File>(b::ot_sv_demo_demoFile));
            *sv::<u8>(b::ot_sv_demo_demoKeys) = self.demo_keys;
            *sv::<u16>(b::ot_sv_demo_demoKeysWait) = self.demo_keys_wait;
        }
    }
}
