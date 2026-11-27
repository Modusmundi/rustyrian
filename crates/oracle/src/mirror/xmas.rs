//! Mirror of `rustyrian_core::xmas::State` <-> the C globals of the same file.

use super::Mirror;
use rustyrian_core::xmas::State;

impl Mirror for State {
    fn load_from_c(&mut self) {}

    fn store_to_c(&self) {}
}
