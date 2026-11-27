//! Mirror of `rustyrian_core::editship::State` <-> the C globals of
//! `editship.c`.

use super::sprite::{sprite2s_from_c, sprite2s_to_c};
use super::{Mirror, global, global_mut};
use crate::bindings as b;
use rustyrian_core::editship::State;

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make;
        // `extraShapes.data` is NULL or an upstream allocation.
        unsafe {
            self.extra_avail = *global(&raw const b::extraAvail);
            self.extra_ships = *global(&raw const b::extraShips);
            self.extra_shapes = sprite2s_from_c(global(&raw const b::extraShapes));
        }
    }

    fn store_to_c(&self) {
        // SAFETY: as above.
        unsafe {
            *global_mut(&raw mut b::extraAvail) = self.extra_avail;
            *global_mut(&raw mut b::extraShips) = self.extra_ships;
            sprite2s_to_c(&self.extra_shapes, global_mut(&raw mut b::extraShapes));
        }
    }
}
