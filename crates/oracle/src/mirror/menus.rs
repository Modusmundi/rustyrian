//! Mirror of `rustyrian_core::menus::State` <-> the C globals of `menus.c`.

use super::{Mirror, global, global_mut};
use crate::bindings as b;
use rustyrian_core::menus::State;

/// Copies a C `char` matrix into bytes.
fn from_c<const N: usize, const M: usize>(c: &[[std::ffi::c_char; N]; M]) -> [[u8; N]; M] {
    c.map(|row| row.map(|ch| ch as u8))
}

/// Copies bytes into a C `char` matrix.
fn to_c<const N: usize, const M: usize>(r: &[[u8; N]; M]) -> [[std::ffi::c_char; N]; M] {
    r.map(|row| row.map(|ch| ch as std::ffi::c_char))
}

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make.
        unsafe {
            self.episode_name = from_c(global(&raw const b::episode_name));
            self.difficulty_name = from_c(global(&raw const b::difficulty_name));
            self.gameplay_name = from_c(global(&raw const b::gameplay_name));
        }
    }

    fn store_to_c(&self) {
        // SAFETY: as above.
        unsafe {
            *global_mut(&raw mut b::episode_name) = to_c(&self.episode_name);
            *global_mut(&raw mut b::difficulty_name) = to_c(&self.difficulty_name);
            *global_mut(&raw mut b::gameplay_name) = to_c(&self.gameplay_name);
        }
    }
}
