//! Mirror of `rustyrian_core::fonthand::State` <-> the C globals of `fonthand.c`.

use super::lvllib::{chars_from_c, chars_to_c};
use super::{Mirror, global, global_mut};
use crate::bindings as b;
use rustyrian_core::fonthand::State;

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make.
        unsafe {
            self.text_glow_font = *global(&raw const b::textGlowFont);
            self.text_glow_brightness = *global(&raw const b::textGlowBrightness);
            self.level_warning_display = *global(&raw const b::levelWarningDisplay);
            self.level_warning_lines = *global(&raw const b::levelWarningLines);
            self.level_warning_text =
                global(&raw const b::levelWarningText).map(|l| chars_from_c(&l));
            self.warning_red = *global(&raw const b::warningRed);
            self.warning_sound_delay = *global(&raw const b::warningSoundDelay);
            self.armor_ship_delay = *global(&raw const b::armorShipDelay);
            self.warning_col = *global(&raw const b::warningCol);
            self.warning_col_change = *global(&raw const b::warningColChange);
        }
    }

    fn store_to_c(&self) {
        // SAFETY: as above.
        unsafe {
            *global_mut(&raw mut b::textGlowFont) = self.text_glow_font;
            *global_mut(&raw mut b::textGlowBrightness) = self.text_glow_brightness;
            *global_mut(&raw mut b::levelWarningDisplay) = self.level_warning_display;
            *global_mut(&raw mut b::levelWarningLines) = self.level_warning_lines;
            *global_mut(&raw mut b::levelWarningText) =
                self.level_warning_text.map(|l| chars_to_c(&l));
            *global_mut(&raw mut b::warningRed) = self.warning_red;
            *global_mut(&raw mut b::warningSoundDelay) = self.warning_sound_delay;
            *global_mut(&raw mut b::armorShipDelay) = self.armor_ship_delay;
            *global_mut(&raw mut b::warningCol) = self.warning_col;
            *global_mut(&raw mut b::warningColChange) = self.warning_col_change;
        }
    }
}
