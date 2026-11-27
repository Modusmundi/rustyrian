//! Mirror of `rustyrian_core::vga_palette::State` <-> the C globals of
//! `vga_palette.c`.

use super::palette::{palette_from_c, palette_to_c};
use super::{Mirror, global, global_mut};
use crate::bindings as b;
use rustyrian_core::vga_palette::State;

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make.
        self.vga_palette = palette_from_c(unsafe { global(&raw const b::vga_palette) });
    }

    fn store_to_c(&self) {
        // SAFETY: as above.
        unsafe { *global_mut(&raw mut b::vga_palette) = palette_to_c(&self.vga_palette) };
    }
}
