// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `lvllib.c` / `lvllib.h`.

use crate::Game;
use crate::file;
use crate::game::{EXIT_FAILURE, Flow, Halt};
use crate::logging::log_fatal;
use serde::{Deserialize, Serialize};

/// `JE_LvlPosType` `[1..42 + 1]`.
pub type JeLvlPosType = [i32; 43];

/// Mutable globals of this module (rule R3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    #[serde(with = "crate::serde_big")]
    pub lvl_pos: JeLvlPosType,
    /// `string [12]` (FKA `LvlLib.levelFile`).
    pub level_filename: [u8; 13],
    pub lvl_num: u16,
}

impl Default for State {
    fn default() -> Self {
        State {
            lvl_pos: [0; 43],
            level_filename: [0; 13],
            lvl_num: 0,
        }
    }
}

/// The C string in `b` (up to its first NUL) as text for paths and messages.
/// Upstream only ever stores ASCII file names here.
pub fn c_string(b: &[u8]) -> String {
    let len = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..len]).into_owned()
}

#[doc(alias = "analyzeLevel")]
pub fn analyze_level(g: &mut Game) -> Flow {
    let level_filename = c_string(&g.lvllib.level_filename);
    let mut file = file::data_file_open(g, &level_filename, "rb");
    if file.error {
        let message = format!(
            "Failed to open file '{}': {}",
            level_filename,
            file::file_get_error(&file)
        );
        log_fatal(g, &message);
        return Err(Halt { code: EXIT_FAILURE });
    }

    let lvl_pos_len = g.lvllib.lvl_pos.len();
    g.lvllib.lvl_num = file::file_read_u16(&mut file);
    // Upstream asserts this bound on data; NDEBUG builds only clamp.
    g.lvllib.lvl_num = g.lvllib.lvl_num.min((lvl_pos_len - 1) as u16);

    for i in 0..usize::from(g.lvllib.lvl_num) {
        g.lvllib.lvl_pos[i] = file::file_read_u32(&mut file) as i32;
    }

    let file_length = file::file_get_length(&mut file);
    for i in usize::from(g.lvllib.lvl_num)..lvl_pos_len {
        g.lvllib.lvl_pos[i] = file_length as i32;
    }

    if file.error {
        let message = format!(
            "Failed to read from file '{}': {}",
            level_filename,
            file::file_get_error(&file)
        );
        log_fatal(g, &message);
        return Err(Halt { code: EXIT_FAILURE });
    }

    file::file_close(&mut file);
    Ok(())
}
