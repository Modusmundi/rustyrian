//! Differential tests for `memwriter.c` / `memwriter.h`.

use proptest::prelude::*;
use rustyrian_core::memreader::MemReader;
use rustyrian_core::memwriter::{self as m, MemWriter};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use std::ffi::c_char;

/// A writer state: the buffer, the offset of the unwritten part and `error`.
fn any_writer() -> impl Strategy<Value = (Vec<u8>, usize, bool)> {
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

/// Runs `c_op` on a C writer over a copy of `buf[off..]` and `r_op` on a Rust
/// writer over another copy; compares the buffers and writer states.
fn check(
    (buf, off, error): &(Vec<u8>, usize, bool),
    c_op: impl FnOnce(&mut b::MemWriter),
    r_op: impl FnOnce(&mut MemWriter<'_>),
) -> Result<(), TestCaseError> {
    let mut cbuf = buf.clone();
    let mut c = b::MemWriter {
        data: cbuf[*off..].as_mut_ptr(),
        size: buf.len() - off,
        error: *error,
    };
    c_op(&mut c);
    // `data` and `size` move independently in C; Rust's unwritten slice is
    // always the buffer's tail, so its offset is `len - size`.
    let c_state = (c.data as usize - cbuf.as_ptr() as usize, c.size, c.error);

    let mut rbuf = buf.clone();
    let len = rbuf.len();
    let mut r = MemWriter {
        data: &mut rbuf[*off..],
        error: *error,
    };
    r_op(&mut r);
    let r_state = (len - r.size(), r.size(), r.error);

    prop_assert_eq!(cbuf, rbuf);
    prop_assert_eq!(c_state, r_state);
    Ok(())
}

macro_rules! scalar {
    ($name:ident, $t:ty, $c:expr, $r:path) => {
        proptest! {
            #[test]
            fn $name(w in any_writer(), v: $t) {
                check(
                    &w,
                    // SAFETY: the writer covers its buffer.
                    |c| unsafe { $c(c, v as _) },
                    |r| $r(r, v),
                )?;
            }
        }
    };
}

macro_rules! array {
    ($name:ident, $t:ty, $ct:ty, $c:expr, $r:path) => {
        proptest! {
            #[test]
            fn $name(w in any_writer(), v in prop::collection::vec(any::<$t>(), 0..10)) {
                let mut cv = v.clone();
                check(
                    &w,
                    // SAFETY: `cv` holds `len` elements; the writer covers
                    // its buffer.
                    |c| unsafe { $c(c, cv.as_mut_ptr().cast::<$ct>(), v.len() as _) },
                    |r| $r(r, &v),
                )?;
            }
        }
    };
}

scalar!(mem_write_u8_matches, u8, b::memWriteU8, m::mem_write_u8);
scalar!(mem_write_u16_matches, u16, b::memWriteU16, m::mem_write_u16);
scalar!(
    mem_write_u16_be_matches,
    u16,
    b::memWriteU16BE,
    m::mem_write_u16be
);
scalar!(mem_write_u32_matches, u32, b::memWriteU32, m::mem_write_u32);
scalar!(
    mem_write_bool_matches,
    bool,
    b::ot_i_memwriter_memWriteBool,
    m::mem_write_bool
);
scalar!(
    mem_write_char_matches,
    i8,
    b::ot_i_memwriter_memWriteChar,
    m::mem_write_char
);
scalar!(
    mem_write_s8_matches,
    i8,
    b::ot_i_memwriter_memWriteS8,
    m::mem_write_s8
);
scalar!(
    mem_write_s16_matches,
    i16,
    b::ot_i_memwriter_memWriteS16,
    m::mem_write_s16
);
scalar!(
    mem_write_s32_matches,
    i32,
    b::ot_i_memwriter_memWriteS32,
    m::mem_write_s32
);

array!(
    mem_write_u8_array_matches,
    u8,
    u8,
    b::memWriteU8Array,
    m::mem_write_u8_array
);
array!(
    mem_write_u16_array_matches,
    u16,
    u16,
    b::memWriteU16Array,
    m::mem_write_u16_array
);
array!(
    mem_write_u32_array_matches,
    u32,
    u32,
    b::memWriteU32Array,
    m::mem_write_u32_array
);
array!(
    mem_write_char_array_matches,
    u8,
    c_char,
    b::ot_i_memwriter_memWriteCharArray,
    m::mem_write_char_array
);
array!(
    mem_write_s8_array_matches,
    i8,
    i8,
    b::ot_i_memwriter_memWriteS8Array,
    m::mem_write_s8_array
);
array!(
    mem_write_s16_array_matches,
    i16,
    i16,
    b::ot_i_memwriter_memWriteS16Array,
    m::mem_write_s16_array
);
array!(
    mem_write_s32_array_matches,
    i32,
    i32,
    b::ot_i_memwriter_memWriteS32Array,
    m::mem_write_s32_array
);

proptest! {
    #[test]
    fn mem_writer_split_matches(w in any_writer(), size in 0usize..30, fill: u8) {
        // Writes `fill` through the split writer so both halves are checked.
        let mut c_split = (0usize, false, false);
        let mut r_split = (0usize, false, false);
        check(
            &w,
            |c| {
                // SAFETY: the writer covers its buffer.
                let mut s = unsafe { b::memWriterSplit(c, size) };
                c_split = (s.size, s.error, s.data.is_null());
                // SAFETY: as above.
                unsafe { b::memWriteFill(&raw mut s, fill, s.size) };
            },
            |r| {
                let mut s = m::mem_writer_split(r, size);
                r_split = (s.size(), s.error, s.error && s.data.is_empty());
                let n = s.size();
                m::mem_write_fill(&mut s, fill, n);
            },
        )?;
        prop_assert_eq!(c_split, r_split);
    }

    #[test]
    fn mem_writer_skip_matches(w in any_writer(), size in 0usize..30) {
        // SAFETY: the writer covers its buffer.
        check(&w, |c| unsafe { b::memWriterSkip(c, size) }, |r| m::mem_writer_skip(r, size))?;
    }

    #[test]
    fn mem_write_fill_matches(w in any_writer(), value: u8, size in 0usize..30) {
        // SAFETY: the writer covers its buffer.
        check(&w, |c| unsafe { b::memWriteFill(c, value, size) }, |r| m::mem_write_fill(r, value, size))?;
    }

    #[test]
    fn mem_write_read_matches(
        w in any_writer(),
        src in prop::collection::vec(any::<u8>(), 0..24),
        roff in any::<prop::sample::Index>(),
        rerror: bool,
        size in 0usize..30,
    ) {
        let roff = roff.index(src.len() + 1);
        let mut c_reader = (0usize, false);
        let mut r_reader = (0usize, false);
        check(
            &w,
            |c| {
                let mut cr = b::MemReader { data: src[roff..].as_ptr(), size: src.len() - roff, error: rerror };
                // SAFETY: both cover their buffers.
                unsafe { b::memWriteRead(c, &raw mut cr, size) };
                c_reader = (cr.size, cr.error);
            },
            |r| {
                let mut rr = MemReader { data: &src[roff..], error: rerror };
                m::mem_write_read(r, &mut rr, size);
                r_reader = (rr.size(), rr.error);
            },
        )?;
        prop_assert_eq!(c_reader, r_reader);
    }

    #[test]
    fn store_helpers_match(v16: u16, v32: u32) {
        let (mut c, mut r) = ([0u8; 4], [0u8; 4]);
        // SAFETY: 4 writable bytes.
        unsafe { b::ot_s_memwriter_storeU16(c.as_mut_ptr(), v16) };
        m::store_u16(&mut r, v16);
        prop_assert_eq!(c, r);
        // SAFETY: as above.
        unsafe { b::ot_s_memwriter_storeU16BE(c.as_mut_ptr(), v16) };
        m::store_u16be(&mut r, v16);
        prop_assert_eq!(c, r);
        // SAFETY: as above.
        unsafe { b::ot_s_memwriter_storeU32(c.as_mut_ptr(), v32) };
        m::store_u32(&mut r, v32);
        prop_assert_eq!(c, r);
    }
}

#[test]
fn initial_state_matches_c() {
    let d = Diff::new();
    assert_eq!(diff::load::<m::State>(), d.game.memwriter);
}

#[test]
fn mirror_round_trip() {
    let d = Diff::new();
    d.store_to_c();
    assert_eq!(diff::load::<m::State>(), d.game.memwriter);
    assert!(d.c_calls().is_empty() && d.rust_calls().is_empty());
}
