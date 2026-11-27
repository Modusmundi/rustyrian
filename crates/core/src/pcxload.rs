// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `pcxload.c` / `pcxload.h`.

use crate::file;
use crate::game::{EXIT_FAILURE, Flow, Game, Halt};
use crate::logging::{SDL_LOG_CATEGORY_APPLICATION, log_fatal};
use crate::memreader::{MemReader, mem_read_u8};
use crate::memwriter::{MemWriter, mem_write_fill, mem_write_u8};
use crate::sdl::Surface;
use serde::{Deserialize, Serialize};

/// Mutable globals of this module (rule R3).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {}

/// `ENOMEM` (12 on every platform the port supports).
const ENOMEM: i32 = 12;

/// Width × height of the full-screen pictures.
pub const IMAGE_SIZE: usize = 320 * 200;

/// Decodes PCX run-length data into a 320×200 image and copies it into
/// `screen` row by row (the loop shared by `JE_loadPCX` and `JE_loadPic`).
pub fn decode_to_screen(data: &[u8], screen: &mut Surface) {
    let mut image = vec![0u8; IMAGE_SIZE];

    let mut reader = MemReader::new(data);
    let mut writer = MemWriter::new(&mut image);

    while !reader.error && writer.size() > 0 {
        let b = mem_read_u8(&mut reader);

        if (b & 0xC0) == 0xC0 {
            let size = b & 0x3F;
            let value = mem_read_u8(&mut reader);
            mem_write_fill(&mut writer, value, usize::from(size));
        } else {
            mem_write_u8(&mut writer, b);
        }
    }

    let pitch = screen.pitch as usize;
    for (y, row) in image.as_chunks::<320>().0.iter().enumerate() {
        screen.pixels[y * pitch..y * pitch + 320].copy_from_slice(row);
    }
}

/// Loads a PCX picture into `VGAScreen` and its palette into `colors`; only
/// meant for `tshp2.pcx`.
#[doc(alias = "JE_loadPCX")]
pub fn je_load_pcx(g: &mut Game, filename: &str) -> Flow {
    let mut file = file::data_file_open(g, filename, "rb");
    if file.error {
        let message = format!(
            "Failed to open file '{}': {}",
            filename,
            file::file_get_error(&file)
        );
        log_fatal(g, &message);
        return Err(Halt { code: EXIT_FAILURE });
    }

    let file_length = file::file_get_length(&mut file);
    if file_length < 128 + 3 * 256 {
        let message = format!("Failed to load file '{filename}'.");
        g.platform.log_error(SDL_LOG_CATEGORY_APPLICATION, &message);
        return Ok(());
    }

    file::file_set_position(&mut file, file_length - 3 * 256);

    let mut palette_data = [0u8; 3 * 256];

    file::file_read_exactly(&mut file, &mut palette_data);

    for (color, rgb) in g
        .palette
        .colors
        .iter_mut()
        .zip(palette_data.as_chunks::<3>().0)
    {
        color.r = rgb[0];
        color.g = rgb[1];
        color.b = rgb[2];
    }

    file::file_set_position(&mut file, 128);

    // Upstream does not check `malloc` for NULL (a directory's length can be
    // huge); an allocation failure becomes a read error with ENOMEM and the
    // picture decodes from no data (divergence `D-UB-PCX-1`).
    let size = (file_length - 128 - 3 * 256) as usize;
    let mut data = Vec::new();
    if data.try_reserve_exact(size).is_ok() {
        data.resize(size, 0);
        file::file_read_exactly(&mut file, &mut data);
    } else {
        file.error = true;
        file.errnum = ENOMEM;
    }

    if file.error {
        let message = format!(
            "Failed to read from file '{}': {}",
            filename,
            file::file_get_error(&file)
        );
        g.platform.log_error(SDL_LOG_CATEGORY_APPLICATION, &message);
    }

    file::file_close(&mut file);

    let screen = g.video.vga_screen_id();
    decode_to_screen(&data, g.video.screen_mut(screen));
    Ok(())
}
