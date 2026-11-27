//! Mirror of `rustyrian_core::video_scale::State` <-> the C globals of
//! `video_scale.c`.

use super::{Mirror, global, global_mut};
use crate::bindings as b;
use rustyrian_core::video_scale::State;

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make.
        self.scaler = unsafe { *global(&raw const b::scaler) };
    }

    fn store_to_c(&self) {
        // SAFETY: as above.
        unsafe { *global_mut(&raw mut b::scaler) = self.scaler };
    }
}
