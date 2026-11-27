//! Mirror of `rustyrian_core::picload::State` <-> the C globals of `picload.c`.

use super::Mirror;
use super::video::sv;
use crate::bindings as b;
use rustyrian_core::picload::State;

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread; `first` is a `bool`.
        self.je_load_pic_first = unsafe { *sv::<bool>(b::ot_sv_picload_JE_loadPic_first) };
    }

    fn store_to_c(&self) {
        // SAFETY: as above.
        unsafe { *sv::<bool>(b::ot_sv_picload_JE_loadPic_first) = self.je_load_pic_first };
    }
}
