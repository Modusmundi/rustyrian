//! Mirror of `rustyrian_core::mainint::State` <-> the C globals of
//! `mainint.c`.

use super::{Mirror, global, global_mut};
use crate::bindings as b;
use rustyrian_core::mainint::State;

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make.
        unsafe {
            self.button = *global(&raw const b::button);
            self.constant_last_x = *global(&raw const b::constantLastX);
            self.text_erase = *global(&raw const b::textErase);
            self.upgrade_cost = *global(&raw const b::upgradeCost);
            self.downgrade_cost = *global(&raw const b::downgradeCost);
            self.perform_save = *global(&raw const b::performSave);
            self.jump_section = *global(&raw const b::jumpSection);
            self.use_last_bank = *global(&raw const b::useLastBank);
            self.pause_pressed = *global(&raw const b::pause_pressed);
            self.ingamemenu_pressed = *global(&raw const b::ingamemenu_pressed);
        }
    }

    fn store_to_c(&self) {
        // SAFETY: as above.
        unsafe {
            *global_mut(&raw mut b::button) = self.button;
            *global_mut(&raw mut b::constantLastX) = self.constant_last_x;
            *global_mut(&raw mut b::textErase) = self.text_erase;
            *global_mut(&raw mut b::upgradeCost) = self.upgrade_cost;
            *global_mut(&raw mut b::downgradeCost) = self.downgrade_cost;
            *global_mut(&raw mut b::performSave) = self.perform_save;
            *global_mut(&raw mut b::jumpSection) = self.jump_section;
            *global_mut(&raw mut b::useLastBank) = self.use_last_bank;
            *global_mut(&raw mut b::pause_pressed) = self.pause_pressed;
            *global_mut(&raw mut b::ingamemenu_pressed) = self.ingamemenu_pressed;
        }
    }
}
