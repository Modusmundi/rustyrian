//! Mirror of `rustyrian_core::params::State` <-> the C globals of `params.c`.

use super::{Mirror, global, global_mut};
use crate::bindings as b;
use rustyrian_core::params::State;

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make.
        unsafe {
            self.rich_mode = *global(&raw const b::richMode);
            self.constant_play = *global(&raw const b::constantPlay);
            self.constant_die = *global(&raw const b::constantDie);
        }
    }

    fn store_to_c(&self) {
        // SAFETY: as above.
        unsafe {
            *global_mut(&raw mut b::richMode) = self.rich_mode;
            *global_mut(&raw mut b::constantPlay) = self.constant_play;
            *global_mut(&raw mut b::constantDie) = self.constant_die;
        }
    }
}
