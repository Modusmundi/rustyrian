// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `backgrnd.c` / `backgrnd.h`.

use serde::{Deserialize, Serialize};

/// `StarfieldStar`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StarfieldStar {
    pub color: u8,
    /// Relies on overflow wrap-around.
    pub position: u16,
    pub speed: i32,
}

pub const MAX_STARS: usize = 100;
pub const STARFIELD_HUE: u8 = 0x90;

/// Mutable globals of this module (rule R3).
///
/// `mapYPos`, `mapY2Pos` and `mapY3Pos` (`JE_byte **`) point into the
/// `mainmap` tile-pointer arrays of `megaData1`, `megaData2` and `megaData3`
/// (`varz`). Per rule R8 they are element indices into the flattened map
/// (`row * width + column`, widths 14, 14 and 15); upstream forms
/// one-before-begin pointers (`tyrian2.c:693-695`), hence `isize`. `None` is
/// NULL.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub back_pos: u16,
    pub back_pos2: u16,
    pub back_pos3: u16,
    pub back_move: u16,
    pub back_move2: u16,
    pub back_move3: u16,
    pub map_x: u16,
    pub map_y: u16,
    pub map_x2: u16,
    pub map_x3: u16,
    pub map_y2: u16,
    pub map_y3: u16,
    pub map_y_pos: Option<isize>,
    pub map_y2_pos: Option<isize>,
    pub map_y3_pos: Option<isize>,
    pub map_x_pos: u16,
    pub old_map_x_ofs: u16,
    pub map_x_ofs: u16,
    pub map_x2_ofs: u16,
    pub map_x2_pos: u16,
    pub map_x3_pos: u16,
    pub old_map_x3_ofs: u16,
    pub map_x3_ofs: u16,
    pub temp_map_x_ofs: u16,
    /// `intptr_t`.
    pub map_xbp_pos: isize,
    pub map_x2bp_pos: isize,
    pub map_x3bp_pos: isize,
    pub map1_y_delay: u8,
    pub map1_y_delay_max: u8,
    pub map2_y_delay: u8,
    pub map2_y_delay_max: u8,
    pub any_smoothies: bool,
    /// `[1..9]`.
    pub smoothie_data: [u8; 9],
    #[serde(with = "crate::serde_big")]
    pub starfield_stars: [StarfieldStar; MAX_STARS],
    pub starfield_speed: i32,
}

impl Default for State {
    fn default() -> Self {
        State {
            back_pos: 0,
            back_pos2: 0,
            back_pos3: 0,
            back_move: 0,
            back_move2: 0,
            back_move3: 0,
            map_x: 0,
            map_y: 0,
            map_x2: 0,
            map_x3: 0,
            map_y2: 0,
            map_y3: 0,
            map_y_pos: None,
            map_y2_pos: None,
            map_y3_pos: None,
            map_x_pos: 0,
            old_map_x_ofs: 0,
            map_x_ofs: 0,
            map_x2_ofs: 0,
            map_x2_pos: 0,
            map_x3_pos: 0,
            old_map_x3_ofs: 0,
            map_x3_ofs: 0,
            temp_map_x_ofs: 0,
            map_xbp_pos: 0,
            map_x2bp_pos: 0,
            map_x3bp_pos: 0,
            map1_y_delay: 0,
            map1_y_delay_max: 0,
            map2_y_delay: 0,
            map2_y_delay_max: 0,
            any_smoothies: false,
            smoothie_data: [0; 9],
            starfield_stars: [StarfieldStar::default(); MAX_STARS],
            starfield_speed: 0,
        }
    }
}
