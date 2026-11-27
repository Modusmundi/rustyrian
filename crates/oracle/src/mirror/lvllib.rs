//! Mirror of `rustyrian_core::lvllib::State` <-> the C globals of `lvllib.c`.
//!
//! Also holds the conversion helpers the other M1/M2 mirrors share: `char`
//! arrays and malloc'd heap blocks.

use super::{Mirror, global, global_mut};
use crate::bindings as b;
use crate::shim::alloc_size;
use rustyrian_core::lvllib::State;
use std::ffi::{c_char, c_void};

unsafe extern "C" {
    // Upstream's allocator shims (`shim/ot_guard.c`): blocks from these are
    // tracked by `alloc_size`, exactly like the ones C allocates itself.
    fn ot_shim_malloc(size: usize) -> *mut c_void;
    fn ot_shim_free(ptr: *mut c_void);
}

/// A C `char[N]` as bytes.
pub fn chars_from_c<const N: usize>(c: &[c_char; N]) -> [u8; N] {
    c.map(|x| x as u8)
}

/// Bytes as a C `char[N]`.
pub fn chars_to_c<const N: usize>(r: &[u8; N]) -> [c_char; N] {
    r.map(|x| x as c_char)
}

/// The contents of a heap block upstream malloc'd (`None` for NULL), with as
/// many elements as were allocated.
///
/// # Safety
/// `p` is NULL or a live block of `C` elements from upstream's allocator.
pub unsafe fn heap_from_c<C, R>(p: *const C, f: impl Fn(&C) -> R) -> Option<Vec<R>> {
    if p.is_null() {
        return None;
    }
    let bytes = alloc_size(p.cast()).expect("pointer is a live malloc'd block");
    let n = bytes / size_of::<C>();
    // SAFETY: per the caller, the block holds `n` initialised elements
    // (upstream's malloc zero-fills).
    let s = unsafe { std::slice::from_raw_parts(p, n) };
    Some(s.iter().map(f).collect())
}

/// Replaces the heap block `*slot` with a new block holding `v` (NULL for
/// `None`), freeing the old one.
///
/// # Safety
/// `*slot` is NULL or a live block from upstream's allocator that nothing else
/// will use afterwards.
pub unsafe fn heap_to_c<C, R>(slot: &mut *mut C, v: Option<&[R]>, f: impl Fn(&R) -> C) {
    // SAFETY: per the caller.
    unsafe { ot_shim_free((*slot).cast()) };
    *slot = match v {
        None => std::ptr::null_mut(),
        Some(v) => {
            // SAFETY: allocation of `v.len()` elements; every element is
            // written below before anyone reads it.
            let p = unsafe { ot_shim_malloc(v.len() * size_of::<C>()) }.cast::<C>();
            for (i, x) in v.iter().enumerate() {
                // SAFETY: `i < v.len()`, inside the block.
                unsafe { p.add(i).write(f(x)) };
            }
            p
        }
    };
}

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make.
        unsafe {
            self.lvl_pos = *global(&raw const b::lvlPos);
            self.level_filename = chars_from_c(global(&raw const b::levelFilename));
            self.lvl_num = *global(&raw const b::lvlNum);
        }
    }

    fn store_to_c(&self) {
        // SAFETY: as above.
        unsafe {
            *global_mut(&raw mut b::lvlPos) = self.lvl_pos;
            *global_mut(&raw mut b::levelFilename) = chars_to_c(&self.level_filename);
            *global_mut(&raw mut b::lvlNum) = self.lvl_num;
        }
    }
}
