// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `memreader.c` / `memreader.h`: bounds-checked
//! little-endian reads from a byte buffer.
//!
//! C's `MemReader { const uint8_t *data; size_t size; bool error; }` becomes a
//! slice (`data`, whose length is C's `size`) plus `error`. The `{ NULL, 0,
//! true }` reader C returns on failure is an empty slice with `error` set.
//! The functions hold no global state, so they take no `Game` (like
//! `mtrand::mt_rand_r`).

use serde::{Deserialize, Serialize};

/// `MemReader`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MemReader<'a> {
    /// The unread bytes (`data`, `size`).
    pub data: &'a [u8],
    /// Indicates that an operation failed and no further operations will be
    /// performed.
    pub error: bool,
}

impl<'a> MemReader<'a> {
    /// A reader over `data` with no error.
    pub fn new(data: &'a [u8]) -> MemReader<'a> {
        MemReader { data, error: false }
    }

    /// C's `size`.
    pub fn size(&self) -> usize {
        self.data.len()
    }

    fn advance(&mut self, n: usize) {
        self.data = &self.data[n..];
    }
}

/// Mutable globals of this module (rule R3): none.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {}

#[doc(alias = "memReaderSplit")]
pub fn mem_reader_split<'a>(reader: &mut MemReader<'a>, size: usize) -> MemReader<'a> {
    reader.error |= reader.size() < size;

    if reader.error {
        return MemReader {
            data: &[],
            error: true,
        };
    }

    let data = &reader.data[..size];

    reader.advance(size);

    MemReader { data, error: false }
}

#[doc(alias = "memReaderSkip")]
pub fn mem_reader_skip(reader: &mut MemReader<'_>, size: usize) {
    reader.error |= reader.size() < size;

    if reader.error {
        return;
    }

    reader.advance(size);
}

#[doc(alias = "memReadU8")]
pub fn mem_read_u8(reader: &mut MemReader<'_>) -> u8 {
    reader.error |= reader.size() < 1;

    if reader.error {
        return 0;
    }

    let value = reader.data[0];
    reader.advance(1);
    value
}

/// `memReadU8Array(reader, values, count)` with `count = values.len()`.
#[doc(alias = "memReadU8Array")]
pub fn mem_read_u8_array(reader: &mut MemReader<'_>, values: &mut [u8]) {
    let count = values.len();
    reader.error |= reader.size() < count;

    if reader.error {
        values.fill(0);
        return;
    }

    values.copy_from_slice(&reader.data[..count]);
    reader.advance(count);
}

/// One byte at a time because some platforms do not allow unaligned loads.
#[doc(alias = "loadU16")]
pub fn load_u16(data: &[u8]) -> u16 {
    u16::from(data[0]) | (u16::from(data[1]) << 8)
}

#[doc(alias = "memReadU16")]
pub fn mem_read_u16(reader: &mut MemReader<'_>) -> u16 {
    reader.error |= reader.size() < 2;

    if reader.error {
        return 0;
    }

    let value = load_u16(reader.data);
    reader.advance(2);
    value
}

#[doc(alias = "memReadU16Array")]
pub fn mem_read_u16_array(reader: &mut MemReader<'_>, values: &mut [u16]) {
    let count = values.len();
    reader.error |= reader.size() / 2 < count;

    if reader.error {
        values.fill(0);
        return;
    }

    for v in values.iter_mut() {
        *v = load_u16(reader.data);
        reader.advance(2);
    }
}

/// One byte at a time because some platforms do not allow unaligned loads.
#[doc(alias = "loadU16BE")]
pub fn load_u16be(data: &[u8]) -> u16 {
    (u16::from(data[0]) << 8) | u16::from(data[1])
}

#[doc(alias = "memReadU16BE")]
pub fn mem_read_u16be(reader: &mut MemReader<'_>) -> u16 {
    reader.error |= reader.size() < 2;

    if reader.error {
        return 0;
    }

    let value = load_u16be(reader.data);
    reader.advance(2);
    value
}

/// One byte at a time because some platforms do not allow unaligned loads.
#[doc(alias = "loadU32")]
pub fn load_u32(data: &[u8]) -> u32 {
    u32::from(data[0])
        | (u32::from(data[1]) << 8)
        | (u32::from(data[2]) << 16)
        | (u32::from(data[3]) << 24)
}

#[doc(alias = "memReadU32")]
pub fn mem_read_u32(reader: &mut MemReader<'_>) -> u32 {
    reader.error |= reader.size() < 4;

    if reader.error {
        return 0;
    }

    let value = load_u32(reader.data);
    reader.advance(4);
    value
}

#[doc(alias = "memReadU32Array")]
pub fn mem_read_u32_array(reader: &mut MemReader<'_>, values: &mut [u32]) {
    let count = values.len();
    reader.error |= reader.size() / 4 < count;

    if reader.error {
        values.fill(0);
        return;
    }

    for v in values.iter_mut() {
        *v = load_u32(reader.data);
        reader.advance(4);
    }
}

#[doc(alias = "memReadBool")]
pub fn mem_read_bool(reader: &mut MemReader<'_>) -> bool {
    mem_read_u8(reader) != 0
}

/// `char` is signed in the oracle (rule R4).
#[doc(alias = "memReadChar")]
pub fn mem_read_char(reader: &mut MemReader<'_>) -> i8 {
    mem_read_u8(reader) as i8
}

/// `char` buffers are bytes (rule R4).
#[doc(alias = "memReadCharArray")]
pub fn mem_read_char_array(reader: &mut MemReader<'_>, values: &mut [u8]) {
    mem_read_u8_array(reader, values);
}

#[doc(alias = "memReadS8")]
pub fn mem_read_s8(reader: &mut MemReader<'_>) -> i8 {
    mem_read_u8(reader) as i8
}

#[doc(alias = "memReadS8Array")]
pub fn mem_read_s8_array(reader: &mut MemReader<'_>, values: &mut [i8]) {
    let count = values.len();
    reader.error |= reader.size() < count;

    if reader.error {
        values.fill(0);
        return;
    }

    for (v, &b) in values.iter_mut().zip(&reader.data[..count]) {
        *v = b as i8;
    }
    reader.advance(count);
}

#[doc(alias = "memReadS16")]
pub fn mem_read_s16(reader: &mut MemReader<'_>) -> i16 {
    mem_read_u16(reader) as i16
}

#[doc(alias = "memReadS16Array")]
pub fn mem_read_s16_array(reader: &mut MemReader<'_>, values: &mut [i16]) {
    let count = values.len();
    reader.error |= reader.size() / 2 < count;

    if reader.error {
        values.fill(0);
        return;
    }

    for v in values.iter_mut() {
        *v = load_u16(reader.data) as i16;
        reader.advance(2);
    }
}

#[doc(alias = "memReadS32")]
pub fn mem_read_s32(reader: &mut MemReader<'_>) -> i32 {
    mem_read_u32(reader) as i32
}

#[doc(alias = "memReadS32Array")]
pub fn mem_read_s32_array(reader: &mut MemReader<'_>, values: &mut [i32]) {
    let count = values.len();
    reader.error |= reader.size() / 4 < count;

    if reader.error {
        values.fill(0);
        return;
    }

    for v in values.iter_mut() {
        *v = load_u32(reader.data) as i32;
        reader.advance(4);
    }
}
