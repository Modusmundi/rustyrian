//! Mirror of `rustyrian_core::musmast::State` <-> the C globals of `musmast.c`.

use super::{Mirror, global, global_mut};
use crate::bindings as b;
use rustyrian_core::musmast::State;

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make.
        unsafe {
            self.song_buy = *global(&raw const b::songBuy);
            self.music_fade = *global(&raw const b::musicFade);
        }
    }

    fn store_to_c(&self) {
        // SAFETY: as above.
        unsafe {
            *global_mut(&raw mut b::songBuy) = self.song_buy;
            *global_mut(&raw mut b::musicFade) = self.music_fade;
        }
    }
}
