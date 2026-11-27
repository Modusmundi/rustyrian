// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `memwriter.c` / `memwriter.h`: bounds-checked
//! little-endian writes into a byte buffer.
//!
//! C's `MemWriter { uint8_t *data; size_t size; bool error; }` becomes a
//! mutable slice (`data`, whose length is C's `size`) plus `error`; the
//! `{ NULL, 0, true }` writer C returns on failure is an empty slice with
//! `error` set. No global state, so no `Game` parameter.

use crate::memreader::MemReader;
use serde::{Deserialize, Serialize};

/// `MemWriter`.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct MemWriter<'a> {
    /// The unwritten bytes (`data`, `size`).
    pub data: &'a mut [u8],
    /// Indicates that an operation failed and no further operations will be
    /// performed.
    pub error: bool,
}

impl<'a> MemWriter<'a> {
    /// A writer over `data` with no error.
    pub fn new(data: &'a mut [u8]) -> MemWriter<'a> {
        MemWriter { data, error: false }
    }

    /// C's `size`.
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// `data += n; size -= n;`, returning the skipped bytes.
    fn advance(&mut self, n: usize) -> &'a mut [u8] {
        let (head, tail) = std::mem::take(&mut self.data).split_at_mut(n);
        self.data = tail;
        head
    }
}

/// Mutable globals of this module (rule R3): none.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {}

#[doc(alias = "memWriterSplit")]
pub fn mem_writer_split<'a>(writer: &mut MemWriter<'a>, size: usize) -> MemWriter<'a> {
    writer.error |= writer.size() < size;

    if writer.error {
        return MemWriter {
            data: &mut [],
            error: true,
        };
    }

    let data = writer.advance(size);

    MemWriter { data, error: false }
}

#[doc(alias = "memWriterSkip")]
pub fn mem_writer_skip(writer: &mut MemWriter<'_>, size: usize) {
    writer.error |= writer.size() < size;

    if writer.error {
        return;
    }

    writer.advance(size);
}

#[doc(alias = "memWriteFill")]
pub fn mem_write_fill(writer: &mut MemWriter<'_>, value: u8, size: usize) {
    writer.error |= writer.size() < size;

    if writer.error {
        return;
    }

    writer.advance(size).fill(value);
}

#[doc(alias = "memWriteRead")]
pub fn mem_write_read(writer: &mut MemWriter<'_>, reader: &mut MemReader<'_>, size: usize) {
    writer.error |= writer.size() < size;
    reader.error |= reader.size() < size;
    writer.error |= reader.error;
    reader.error = writer.error;

    if writer.error {
        return;
    }

    writer.advance(size).copy_from_slice(&reader.data[..size]);
    reader.data = &reader.data[size..];
}

#[doc(alias = "memWriteU8")]
pub fn mem_write_u8(writer: &mut MemWriter<'_>, value: u8) {
    writer.error |= writer.size() < 1;

    if writer.error {
        return;
    }

    writer.advance(1)[0] = value;
}

/// `memWriteU8Array(writer, values, count)` with `count = values.len()`.
#[doc(alias = "memWriteU8Array")]
pub fn mem_write_u8_array(writer: &mut MemWriter<'_>, values: &[u8]) {
    let count = values.len();
    writer.error |= writer.size() < count;

    if writer.error {
        return;
    }

    writer.advance(count).copy_from_slice(values);
}

/// One byte at a time because some platforms do not allow unaligned stores.
#[doc(alias = "storeU16")]
pub fn store_u16(data: &mut [u8], value: u16) {
    data[0] = value as u8;
    data[1] = (value >> 8) as u8;
}

#[doc(alias = "memWriteU16")]
pub fn mem_write_u16(writer: &mut MemWriter<'_>, value: u16) {
    writer.error |= writer.size() < 2;

    if writer.error {
        return;
    }

    store_u16(writer.advance(2), value);
}

#[doc(alias = "memWriteU16Array")]
pub fn mem_write_u16_array(writer: &mut MemWriter<'_>, values: &[u16]) {
    let count = values.len();
    writer.error |= writer.size() / 2 < count;

    if writer.error {
        return;
    }

    for &v in values {
        store_u16(writer.advance(2), v);
    }
}

/// One byte at a time because some platforms do not allow unaligned stores.
#[doc(alias = "storeU16BE")]
pub fn store_u16be(data: &mut [u8], value: u16) {
    data[0] = (value >> 8) as u8;
    data[1] = value as u8;
}

#[doc(alias = "memWriteU16BE")]
pub fn mem_write_u16be(writer: &mut MemWriter<'_>, value: u16) {
    writer.error |= writer.size() < 2;

    if writer.error {
        return;
    }

    store_u16be(writer.advance(2), value);
}

/// One byte at a time because some platforms do not allow unaligned stores.
#[doc(alias = "storeU32")]
pub fn store_u32(data: &mut [u8], value: u32) {
    data[0] = value as u8;
    data[1] = (value >> 8) as u8;
    data[2] = (value >> 16) as u8;
    data[3] = (value >> 24) as u8;
}

#[doc(alias = "memWriteU32")]
pub fn mem_write_u32(writer: &mut MemWriter<'_>, value: u32) {
    writer.error |= writer.size() < 4;

    if writer.error {
        return;
    }

    store_u32(writer.advance(4), value);
}

#[doc(alias = "memWriteU32Array")]
pub fn mem_write_u32_array(writer: &mut MemWriter<'_>, values: &[u32]) {
    let count = values.len();
    writer.error |= writer.size() / 4 < count;

    if writer.error {
        return;
    }

    for &v in values {
        store_u32(writer.advance(4), v);
    }
}

#[doc(alias = "memWriteBool")]
pub fn mem_write_bool(writer: &mut MemWriter<'_>, value: bool) {
    mem_write_u8(writer, if value { 1 } else { 0 });
}

/// `char` is signed in the oracle (rule R4).
#[doc(alias = "memWriteChar")]
pub fn mem_write_char(writer: &mut MemWriter<'_>, value: i8) {
    mem_write_u8(writer, value as u8);
}

/// `char` buffers are bytes (rule R4).
#[doc(alias = "memWriteCharArray")]
pub fn mem_write_char_array(writer: &mut MemWriter<'_>, values: &[u8]) {
    mem_write_u8_array(writer, values);
}

#[doc(alias = "memWriteS8")]
pub fn mem_write_s8(writer: &mut MemWriter<'_>, value: i8) {
    mem_write_u8(writer, value as u8);
}

#[doc(alias = "memWriteS8Array")]
pub fn mem_write_s8_array(writer: &mut MemWriter<'_>, values: &[i8]) {
    let count = values.len();
    writer.error |= writer.size() < count;

    if writer.error {
        return;
    }

    for (d, &v) in writer.advance(count).iter_mut().zip(values) {
        *d = v as u8;
    }
}

#[doc(alias = "memWriteS16")]
pub fn mem_write_s16(writer: &mut MemWriter<'_>, value: i16) {
    mem_write_u16(writer, value as u16);
}

#[doc(alias = "memWriteS16Array")]
pub fn mem_write_s16_array(writer: &mut MemWriter<'_>, values: &[i16]) {
    let count = values.len();
    writer.error |= writer.size() / 2 < count;

    if writer.error {
        return;
    }

    for &v in values {
        store_u16(writer.advance(2), v as u16);
    }
}

#[doc(alias = "memWriteS32")]
pub fn mem_write_s32(writer: &mut MemWriter<'_>, value: i32) {
    mem_write_u32(writer, value as u32);
}

#[doc(alias = "memWriteS32Array")]
pub fn mem_write_s32_array(writer: &mut MemWriter<'_>, values: &[i32]) {
    let count = values.len();
    writer.error |= writer.size() / 4 < count;

    if writer.error {
        return;
    }

    for &v in values {
        store_u32(writer.advance(4), v as u32);
    }
}
