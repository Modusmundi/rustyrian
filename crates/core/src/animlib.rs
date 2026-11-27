// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `animlib.c` / `animlib.h`: `.anm` animation playback.

use crate::file::{self, File};
use crate::memreader::{MemReader, mem_read_u8, mem_read_u16, mem_read_u32, mem_reader_skip};
use crate::memwriter::{MemWriter, mem_write_fill, mem_write_read, mem_writer_skip};
use crate::palette::Color;
use serde::{Deserialize, Serialize};

/// `FileHeader` (file-local type of `animlib.c`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FileHeader {
    pub page_count: u16,
    pub record_count: u32,
}

/// `PageDescriptor` (file-local type of `animlib.c`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PageDescriptor {
    pub first_record: u16,
    pub record_count: u16,
    pub records_size: u16,
}

/// `animlib.c` has no mutable globals.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {}

/// Reads the 256-byte file header.
#[doc(alias = "readFileHeader")]
pub fn read_file_header(file_header: &mut FileHeader, file: &mut File) {
    let mut data = [0u8; 256];
    file::file_read_exactly(file, &mut data);

    let mut reader = MemReader::new(&data);

    mem_reader_skip(&mut reader, 6);
    file_header.page_count = mem_read_u16(&mut reader);
    file_header.record_count = mem_read_u32(&mut reader);
    mem_reader_skip(&mut reader, 244);

    debug_assert!(reader.size() == 0 && !reader.error);
}

/// Reads the 256 BGRx palette entries.
#[doc(alias = "readPalette")]
pub fn read_palette(palette: &mut [Color; 256], file: &mut File) {
    let mut data = [0u8; 4 * 256];
    file::file_read_exactly(file, &mut data);

    let mut reader = MemReader::new(&data);

    for color in palette.iter_mut() {
        color.b = mem_read_u8(&mut reader);
        color.g = mem_read_u8(&mut reader);
        color.r = mem_read_u8(&mut reader);
        let _ = mem_read_u8(&mut reader);
    }

    debug_assert!(reader.size() == 0 && !reader.error);
}

/// Reads the 256 page descriptors.
#[doc(alias = "readPageDescriptors")]
pub fn read_page_descriptors(page_descriptors: &mut [PageDescriptor; 256], file: &mut File) {
    let mut data = [0u8; 6 * 256];
    file::file_read_exactly(file, &mut data);

    let mut reader = MemReader::new(&data);

    for d in page_descriptors.iter_mut() {
        d.first_record = mem_read_u16(&mut reader);
        d.record_count = mem_read_u16(&mut reader);
        d.records_size = mem_read_u16(&mut reader);
    }

    debug_assert!(reader.size() == 0 && !reader.error);
}

/// Decodes one run/skip/dump-compressed record body into `writer`.
#[doc(alias = "decodeRunSkipDump")]
pub fn decode_run_skip_dump(writer: &mut MemWriter<'_>, reader: &mut MemReader<'_>) {
    while !reader.error {
        let op_code = mem_read_u8(reader);

        if op_code == 0 {
            // 00: short run
            let size = mem_read_u8(reader);
            let value = mem_read_u8(reader);
            mem_write_fill(writer, value, usize::from(size));
        } else if op_code > 0x80 {
            // 81..FF: short skip
            mem_writer_skip(writer, usize::from(op_code - 0x80));
        } else if op_code < 0x80 {
            // 01..7F: short dump
            mem_write_read(writer, reader, usize::from(op_code));
        } else {
            // 80: long op
            let op_code = mem_read_u16(reader);

            if op_code == 0 {
                // 0000: stop
                return;
            } else if op_code >= 0xC000 {
                // C000..FFFF: long run
                let value = mem_read_u8(reader);
                mem_write_fill(writer, value, usize::from(op_code - 0xC000));
            } else if op_code < 0x8000 {
                // 0001..7FFF: long skip
                mem_writer_skip(writer, usize::from(op_code));
            } else {
                // 8000..BFFF: long dump
                mem_write_read(writer, reader, usize::from(op_code - 0x8000));
            }
        }
    }
}
