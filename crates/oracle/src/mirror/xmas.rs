//! Mirror of `rustyrian_core::xmas::State` <-> the C globals of `xmas.c`.

use super::{Mirror, global, global_mut};
use crate::bindings as b;
use rustyrian_core::xmas::State;

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make.
        self.xmas = unsafe { *global(&raw const b::xmas) };
    }

    fn store_to_c(&self) {
        // SAFETY: as above.
        unsafe { *global_mut(&raw mut b::xmas) = self.xmas };
    }
}
