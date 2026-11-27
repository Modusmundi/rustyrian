//! Differential tests for `memreader.c` / `memreader.h`.

use proptest::prelude::*;
use rustyrian_core::memreader::{self as m, MemReader};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use std::ffi::c_char;

/// A reader state: the buffer, the offset of the unread part and `error`.
fn any_reader() -> impl Strategy<Value = (Vec<u8>, usize, bool)> {
    (
        prop::collection::vec(any::<u8>(), 0..24),
        any::<prop::sample::Index>(),
        any::<bool>(),
    )
        .prop_map(|(buf, i, error)| {
            let off = i.index(buf.len() + 1);
            (buf, off, error)
        })
}

/// The C reader over `buf[off..]`.
fn c_reader(buf: &[u8], off: usize, error: bool) -> b::MemReader {
    b::MemReader {
        data: buf[off..].as_ptr(),
        size: buf.len() - off,
        error,
    }
}

/// `(offset, size, error)` of a C reader over `buf` (`NULL` data → offset
/// `usize::MAX`).
fn c_view(buf: &[u8], r: &b::MemReader) -> (usize, usize, bool) {
    let off = if r.data.is_null() {
        usize::MAX
    } else {
        r.data as usize - buf.as_ptr() as usize
    };
    (off, r.size, r.error)
}

/// The same view of a Rust reader (a slice outside `buf`, as returned on
/// failure, stands for `NULL`).
fn rust_view(buf: &[u8], r: &MemReader<'_>) -> (usize, usize, bool) {
    let p = r.data.as_ptr() as usize;
    let base = buf.as_ptr() as usize;
    let off = if p < base || p > base + buf.len() {
        usize::MAX
    } else {
        p - base
    };
    (off, r.size(), r.error)
}

/// Runs a scalar read on both sides and compares result and reader state.
macro_rules! scalar {
    ($name:ident, $c:expr, $r:path) => {
        proptest! {
            #[test]
            fn $name((buf, off, error) in any_reader()) {
                let mut c = c_reader(&buf, off, error);
                // SAFETY: `c` reads within `buf`.
                let cv = unsafe { $c(&raw mut c) };
                let mut r = MemReader { data: &buf[off..], error };
                let rv = $r(&mut r);
                prop_assert_eq!(cv, rv);
                prop_assert_eq!((c.size, c.error), (r.size(), r.error));
                prop_assert_eq!(c_view(&buf, &c).0, rust_view(&buf, &r).0);
            }
        }
    };
}

/// Runs an array read on both sides and compares output and reader state.
macro_rules! array {
    ($name:ident, $t:ty, $ct:ty, $c:expr, $r:path) => {
        proptest! {
            #[test]
            fn $name((buf, off, error) in any_reader(), count in 0usize..16, junk: $t) {
                let mut c = c_reader(&buf, off, error);
                let mut cv = vec![junk; count];
                // SAFETY: `cv` holds `count` elements; `c` reads within `buf`.
                unsafe { $c(&raw mut c, cv.as_mut_ptr().cast::<$ct>(), count as _) };
                let mut r = MemReader { data: &buf[off..], error };
                let mut rv = vec![junk; count];
                $r(&mut r, &mut rv);
                prop_assert_eq!(cv, rv);
                prop_assert_eq!((c.size, c.error), (r.size(), r.error));
                prop_assert_eq!(c_view(&buf, &c).0, rust_view(&buf, &r).0);
            }
        }
    };
}

scalar!(mem_read_u8_matches, b::memReadU8, m::mem_read_u8);
scalar!(mem_read_u16_matches, b::memReadU16, m::mem_read_u16);
scalar!(mem_read_u16_be_matches, b::memReadU16BE, m::mem_read_u16be);
scalar!(mem_read_u32_matches, b::memReadU32, m::mem_read_u32);
scalar!(
    mem_read_bool_matches,
    b::ot_i_memreader_memReadBool,
    m::mem_read_bool
);
scalar!(
    mem_read_char_matches,
    b::ot_i_memreader_memReadChar,
    m::mem_read_char
);
scalar!(
    mem_read_s8_matches,
    b::ot_i_memreader_memReadS8,
    m::mem_read_s8
);
scalar!(
    mem_read_s16_matches,
    b::ot_i_memreader_memReadS16,
    m::mem_read_s16
);
scalar!(
    mem_read_s32_matches,
    b::ot_i_memreader_memReadS32,
    m::mem_read_s32
);

array!(
    mem_read_u8_array_matches,
    u8,
    u8,
    b::memReadU8Array,
    m::mem_read_u8_array
);
array!(
    mem_read_u16_array_matches,
    u16,
    u16,
    b::memReadU16Array,
    m::mem_read_u16_array
);
array!(
    mem_read_u32_array_matches,
    u32,
    u32,
    b::memReadU32Array,
    m::mem_read_u32_array
);
array!(
    mem_read_char_array_matches,
    u8,
    c_char,
    b::ot_i_memreader_memReadCharArray,
    m::mem_read_char_array
);
array!(
    mem_read_s8_array_matches,
    i8,
    i8,
    b::ot_i_memreader_memReadS8Array,
    m::mem_read_s8_array
);
array!(
    mem_read_s16_array_matches,
    i16,
    i16,
    b::ot_i_memreader_memReadS16Array,
    m::mem_read_s16_array
);
array!(
    mem_read_s32_array_matches,
    i32,
    i32,
    b::ot_i_memreader_memReadS32Array,
    m::mem_read_s32_array
);

proptest! {
    #[test]
    fn mem_reader_split_matches((buf, off, error) in any_reader(), size in 0usize..30) {
        let mut c = c_reader(&buf, off, error);
        // SAFETY: `c` reads within `buf`.
        let cs = unsafe { b::memReaderSplit(&raw mut c, size) };
        let mut r = MemReader { data: &buf[off..], error };
        let rs = m::mem_reader_split(&mut r, size);
        prop_assert_eq!(c_view(&buf, &c), rust_view(&buf, &r));
        if cs.data.is_null() {
            // The failure reader `{ NULL, 0, true }`.
            prop_assert_eq!((cs.size, cs.error), (rs.size(), rs.error));
            prop_assert!(rs.error && rs.data.is_empty());
        } else {
            prop_assert_eq!(c_view(&buf, &cs), rust_view(&buf, &rs));
        }
    }

    #[test]
    fn mem_reader_skip_matches((buf, off, error) in any_reader(), size in 0usize..30) {
        let mut c = c_reader(&buf, off, error);
        // SAFETY: `c` reads within `buf`.
        unsafe { b::memReaderSkip(&raw mut c, size) };
        let mut r = MemReader { data: &buf[off..], error };
        m::mem_reader_skip(&mut r, size);
        prop_assert_eq!(c_view(&buf, &c), rust_view(&buf, &r));
    }

    #[test]
    fn load_helpers_match(x: [u8; 4]) {
        // SAFETY: `x` has 4 readable bytes.
        unsafe {
            prop_assert_eq!(b::ot_s_memreader_loadU16(x.as_ptr()), m::load_u16(&x));
            prop_assert_eq!(b::ot_s_memreader_loadU16BE(x.as_ptr()), m::load_u16be(&x));
            prop_assert_eq!(b::ot_s_memreader_loadU32(x.as_ptr()), m::load_u32(&x));
        }
    }
}

#[test]
fn initial_state_matches_c() {
    let d = Diff::new();
    assert_eq!(diff::load::<m::State>(), d.game.memreader);
}

#[test]
fn mirror_round_trip() {
    let d = Diff::new();
    d.store_to_c();
    assert_eq!(diff::load::<m::State>(), d.game.memreader);
    assert!(d.c_calls().is_empty() && d.rust_calls().is_empty());
}
