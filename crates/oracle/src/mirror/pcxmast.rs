//! Mirror of `rustyrian_core::pcxmast::State` <-> the C globals of `pcxmast.c`.

use super::{Mirror, global, global_mut};
use crate::bindings as b;
use rustyrian_core::pcxmast::State;

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make.
        self.pcxpos = unsafe { *global(&raw const b::pcxpos) };
    }

    fn store_to_c(&self) {
        // SAFETY: as above.
        unsafe { *global_mut(&raw mut b::pcxpos) = self.pcxpos };
    }
}
