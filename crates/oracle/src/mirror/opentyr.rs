//! Mirror of `rustyrian_core::opentyr::State` <-> the C globals of
//! `opentyr.c`.
//!
//! `opentyrian_str`/`opentyrian_version` point to string literals; they are
//! mirrored by contents. Storing keeps C's pointer when the contents already
//! match and otherwise frees a previous mirror-made copy and points the
//! global at a fresh copy from the upstream allocator.
//!
//! `atexit` mirrors the handlers C registered through the `atexit` shim
//! (upstream registers only `SDL_Quit`). They cannot be registered from Rust,
//! so storing checks that both sides already agree.

use super::video::{c_heap_free, c_heap_from};
use super::{Mirror, global, global_mut};
use crate::bindings as b;
use crate::session;
use rustyrian_core::opentyr::{AtExit, State};
use std::ffi::{CStr, c_char};

/// # Safety
/// `p` is a NUL-terminated string.
unsafe fn load(p: *const c_char) -> Vec<u8> {
    // SAFETY: per the caller.
    unsafe { CStr::from_ptr(p) }.to_bytes().to_vec()
}

/// # Safety
/// `*slot` is a NUL-terminated string.
unsafe fn store(r: &[u8], slot: &mut *const c_char) {
    // SAFETY: per the caller.
    if unsafe { load(*slot) } == r {
        return;
    }
    // SAFETY: a literal is not an upstream heap block and is left alone; a
    // previous mirror-made copy is referenced only by this global.
    unsafe { c_heap_free(slot.cast_mut()) };
    let mut z = r.to_vec();
    z.push(0);
    *slot = c_heap_from(&z).cast();
}

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make; both
        // globals always point to strings.
        unsafe {
            self.opentyrian_str = load(*global(&raw const b::opentyrian_str));
            self.opentyrian_version = load(*global(&raw const b::opentyrian_version));
        }
        self.atexit = vec![AtExit::SdlQuit; session::atexit_len()];
    }

    fn store_to_c(&self) {
        // SAFETY: as above.
        unsafe {
            store(&self.opentyrian_str, global_mut(&raw mut b::opentyrian_str));
            store(
                &self.opentyrian_version,
                global_mut(&raw mut b::opentyrian_version),
            );
        }
        assert_eq!(
            self.atexit.len(),
            session::atexit_len(),
            "atexit registrations cannot be stored into C; both sides must already agree"
        );
    }
}
