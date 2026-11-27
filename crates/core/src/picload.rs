// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `picload.c` / `picload.h`.

use crate::file::{self, File};
use crate::game::{EXIT_FAILURE, Flow, Game, Halt};
use crate::logging::{SDL_LOG_CATEGORY_APPLICATION, log_fatal};
use crate::palette::set_palette;
use crate::pcxload::decode_to_screen;
use crate::pcxmast::{PCX_NUM, PCXPAL};
use crate::video::ScreenId;
use serde::{Deserialize, Serialize};

/// Mutable globals of this module (rule R3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// `JE_loadPic`'s local `static bool first = true`.
    pub je_load_pic_first: bool,
}

impl Default for State {
    fn default() -> Self {
        State {
            je_load_pic_first: true,
        }
    }
}

/// A zeroed buffer for reading `size` bytes with `fileReadExactly` (C's
/// `malloc(size)`; the oracle's `malloc` zeroes). Where the read cannot fill
/// `size` bytes, a shorter buffer yields the same picture: past the end of the
/// file (one byte more than the rest, so the read still fails with end of
/// file) or after an earlier error (empty; C only zeroes its buffer), C
/// decodes zero bytes, which write the zero pixels the image starts with. So
/// this is not a divergence; it avoids allocating a corrupt size.
fn read_buffer(file: &mut File, size: usize) -> Vec<u8> {
    if file.error {
        return Vec::new();
    }
    let pos = file::file_get_position(file);
    let len = file::file_get_length(file);
    let rest = usize::try_from(len.saturating_sub(pos)).unwrap_or(0);
    vec![0; size.min(rest + 1)]
}

/// Loads picture `id` (1-based) of `tyrian.pic` into `screen` and its
/// palette into `colors`, and makes that palette current if `storepal`.
#[doc(alias = "JE_loadPic")]
pub fn je_load_pic(g: &mut Game, screen: ScreenId, id: u8, storepal: bool) -> Flow {
    let filename = "tyrian.pic";

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

    if g.picload.je_load_pic_first {
        g.picload.je_load_pic_first = false;

        let count = usize::from(file::file_read_u16(&mut file));

        for i in 0..count.min(PCX_NUM + 1) {
            g.pcxmast.pcxpos[i] = file::file_read_u32(&mut file) as i32;
        }

        for i in count..PCX_NUM + 1 {
            g.pcxmast.pcxpos[i] = file::file_get_length(&mut file) as i32;
        }

        if file.error {
            let message = format!(
                "Failed to read from file '{}': {}",
                filename,
                file::file_get_error(&file)
            );
            g.platform.log_error(SDL_LOG_CATEGORY_APPLICATION, &message);
        }
    }

    if id < 1 || usize::from(id) > PCX_NUM {
        let message = format!("Attempted to load picture {id}, which does not exist.");
        g.platform.log_error(SDL_LOG_CATEGORY_APPLICATION, &message);
        return Ok(());
    }

    let position = i64::from(g.pcxmast.pcxpos[usize::from(id) - 1]);
    let end_position = i64::from(g.pcxmast.pcxpos[usize::from(id)]);
    let size = if end_position > position {
        (end_position - position) as usize
    } else {
        0
    };

    file::file_set_position(&mut file, position);

    let mut data = read_buffer(&mut file, size);
    file::file_read_exactly(&mut file, &mut data);

    if file.error {
        let message = format!(
            "Failed to read from file '{}': {}",
            filename,
            file::file_get_error(&file)
        );
        g.platform.log_error(SDL_LOG_CATEGORY_APPLICATION, &message);
    }

    file::file_close(&mut file);

    decode_to_screen(&data, g.video.screen_mut(screen));

    g.palette.colors = g.palette.palettes[usize::from(PCXPAL[usize::from(id) - 1])];

    if storepal {
        let colors = g.palette.colors;
        set_palette(g, &colors, 0, 255);
    }
    Ok(())
}
