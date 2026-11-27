// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `animlib.c` / `animlib.h`: `.anm` animation playback.

use crate::Game;
use crate::file::{self, File};
use crate::game::Flow;
use crate::keyboard::wait_until_get_input_or_elapsed;
use crate::logging::SDL_LOG_CATEGORY_APPLICATION;
use crate::memreader::{
    MemReader, mem_read_u8, mem_read_u16, mem_read_u32, mem_reader_skip, mem_reader_split,
};
use crate::memwriter::{MemWriter, mem_write_fill, mem_write_read, mem_writer_skip};
use crate::nortsong::set_frame_count;
use crate::palette::{Color, set_palette};
use crate::video::{je_clr256, je_show_vga};
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

/// A `MemReader` over the page buffer, kept as offsets so the buffer can be
/// refilled while the reader is idle.
#[derive(Clone, Copy, Default)]
struct Span {
    start: usize,
    end: usize,
    error: bool,
}

impl Span {
    /// The reader over `data[start..end]`.
    fn reader(self, data: &[u8]) -> MemReader<'_> {
        MemReader {
            data: &data[self.start..self.end],
            error: self.error,
        }
    }

    /// Records how far `reader` (from [`Span::reader`]) has read.
    fn update(&mut self, reader: &MemReader<'_>) {
        self.start = self.end - reader.size();
        self.error = reader.error;
    }
}

/// Plays the `.anm` animation `filename` on `VGAScreen` from frame
/// `starting_frame`, `speed` frame periods per frame, until it ends or input
/// arrives.
#[doc(alias = "playAnim")]
pub fn play_anim(g: &mut Game, filename: &str, starting_frame: u8, speed: u8) -> Flow {
    let screen = g.video.vga_screen_id();
    je_clr256(g, screen);
    je_show_vga(g);

    let mut file = file::data_file_open(g, filename, "rb");
    if file.error {
        let e = file::file_get_error(&file);
        g.platform.log_warn(
            SDL_LOG_CATEGORY_APPLICATION,
            &format!("Failed to open file '{filename}': {e}"),
        );
        return Ok(());
    }

    // Leaving this block is upstream's `goto fail`.
    'fail: {
        let mut file_header = FileHeader::default();
        read_file_header(&mut file_header, &mut file);
        if file.error {
            break 'fail;
        }

        let mut palette = [Color::default(); 256];
        read_palette(&mut palette, &mut file);
        if file.error {
            break 'fail;
        }

        // Fixed-size pages, each holding a run of consecutive records (one
        // per frame); the descriptors say which records each page holds.
        let mut page_descriptors = [PageDescriptor::default(); 256];
        read_page_descriptors(&mut page_descriptors, &mut file);
        if file.error {
            break 'fail;
        }

        palette[0].r = 0;
        palette[0].g = 0;
        palette[0].b = 0;
        set_palette(g, &palette, 0, 255);

        let mut first_record: u16 = 0;
        let mut record_count: u16 = 0;

        const DATA_SIZE: usize = 1 << 16;
        let mut data = vec![0u8; DATA_SIZE];

        let mut record_sizes = Span::default();
        let mut records = Span::default();

        const IMAGE_SIZE: usize = 320 * 200;
        let mut image = vec![0u8; IMAGE_SIZE];

        let mut record = u16::from(starting_frame);
        while u32::from(record) < file_header.record_count.wrapping_sub(1) {
            set_frame_count(g, u16::from(speed));

            if record < first_record
                || i32::from(record) - i32::from(first_record) >= i32::from(record_count)
            {
                // Load the page holding the record.
                let page_count = usize::from(file_header.page_count);
                for (i, pd) in page_descriptors.iter().enumerate().take(page_count) {
                    first_record = pd.first_record;
                    record_count = pd.record_count;
                    let records_size = pd.records_size;

                    if record >= first_record
                        && i32::from(record) - i32::from(first_record) < i32::from(record_count)
                    {
                        file::file_set_position(&mut file, 0xB00 + ((i as i64) << 16));

                        let page_size =
                            8 + 2 * usize::from(record_count) + usize::from(records_size);
                        let size = file::file_read_at_most(
                            &mut file,
                            &mut data[..page_size.min(DATA_SIZE)],
                        );

                        let mut page_reader = MemReader {
                            data: &data[..size],
                            error: size != page_size,
                        };

                        mem_reader_skip(&mut page_reader, 8);

                        // Record sizes, then the records.
                        let sizes =
                            mem_reader_split(&mut page_reader, 2 * usize::from(record_count));
                        record_sizes = if sizes.error {
                            Span {
                                error: true,
                                ..Span::default()
                            }
                        } else {
                            Span {
                                start: 8,
                                end: 8 + 2 * usize::from(record_count),
                                error: false,
                            }
                        };
                        records = if page_reader.error {
                            Span {
                                error: true,
                                ..Span::default()
                            }
                        } else {
                            Span {
                                start: size - page_reader.size(),
                                end: size,
                                error: false,
                            }
                        };
                        break;
                    }
                }
            }

            while record >= first_record && record_count > 0 {
                first_record = first_record.wrapping_add(1);
                record_count -= 1;

                let mut sizes_reader = record_sizes.reader(&data);
                let record_size = mem_read_u16(&mut sizes_reader);
                record_sizes.update(&sizes_reader);

                if sizes_reader.error {
                    continue;
                }

                let mut records_reader = records.reader(&data);
                let mut record_reader =
                    mem_reader_split(&mut records_reader, usize::from(record_size));
                records.update(&records_reader);

                // Records before the wanted one are skipped.
                if record > first_record {
                    continue;
                }

                // Record header: bitmap id, flags, body type.
                mem_read_u8(&mut record_reader);
                mem_read_u8(&mut record_reader);
                mem_read_u16(&mut record_reader);

                let mut image_writer = MemWriter::new(&mut image);
                decode_run_skip_dump(&mut image_writer, &mut record_reader);

                let vga = g.video.vga_mut();
                debug_assert!(vga.w == 320 && vga.h == 200);
                let pitch = vga.pitch as usize;
                for y in 0..200 {
                    vga.pixels[y * pitch..y * pitch + 320]
                        .copy_from_slice(&image[y * 320..y * 320 + 320]);
                }

                je_show_vga(g);
            }

            if wait_until_get_input_or_elapsed(g)? {
                break;
            }
            record = record.wrapping_add(1);
        }
    }

    if file.error {
        let e = file::file_get_error(&file);
        g.platform.log_error(
            SDL_LOG_CATEGORY_APPLICATION,
            &format!("Failed to read from file '{filename}': {e}"),
        );
    }

    file::file_close(&mut file);
    Ok(())
}
