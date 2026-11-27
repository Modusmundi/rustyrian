//! Mirror of `rustyrian_core::mtrand::State` <-> the C globals of `mtrand.c`.

use super::{Mirror, global, global_mut};
use crate::bindings as b;
use rustyrian_core::mtrand::{MtState, State};

/// Converts a C `MtState`.
pub fn mt_from_c(c: &b::MtState) -> MtState {
    let mut r = MtState::default();
    for (d, s) in r.x.iter_mut().zip(c.x.iter()) {
        // `unsigned long` is 64-bit on the LP64 hosts the oracle builds on.
        #[allow(clippy::useless_conversion)]
        let v = u64::from(*s);
        *d = v;
    }
    r.p0 = c.p0;
    r.p1 = c.p1;
    r.pm = c.pm;
    r
}

/// Writes a core `MtState` into a C one.
pub fn mt_to_c(r: &MtState, c: &mut b::MtState) {
    for (d, s) in c.x.iter_mut().zip(r.x.iter()) {
        *d = *s as std::ffi::c_ulong;
    }
    c.p0 = r.p0;
    c.p1 = r.p1;
    c.pm = r.pm;
}

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make.
        self.mt_global = mt_from_c(unsafe { global(&raw const b::mt_global) });
    }

    fn store_to_c(&self) {
        // SAFETY: as above.
        mt_to_c(&self.mt_global, unsafe {
            global_mut(&raw mut b::mt_global)
        });
    }
}
