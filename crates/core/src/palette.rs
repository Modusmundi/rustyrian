// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `palette.c` / `palette.h`.

use serde::{Deserialize, Serialize};

/// `SDL_Color`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// `Palette`: `SDL_Color[256]`.
pub type Palette = [Color; 256];

/// Number of entries of `palettes`.
pub const PALETTES_MAX: usize = 23;

/// Mutable globals of this module (rule R3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    #[serde(with = "crate::serde_big::d2_boxed")]
    pub palettes: Box<[Palette; PALETTES_MAX]>,
    pub palettes_count: usize,
    /// File `static palette`.
    #[serde(with = "crate::serde_big")]
    pub palette: Palette,
    #[serde(with = "crate::serde_big")]
    pub rgb_palette: [u32; 256],
    #[serde(with = "crate::serde_big")]
    pub yuv_palette: [u32; 256],
    #[serde(with = "crate::serde_big")]
    pub colors: Palette,
    /// `fade_palette`'s local `static int diff[256][3]`.
    #[serde(with = "crate::serde_big")]
    pub fade_palette_diff: [[i32; 3]; 256],
    /// `fade_solid`'s local `static int diff[256][3]`.
    #[serde(with = "crate::serde_big")]
    pub fade_solid_diff: [[i32; 3]; 256],
}

impl Default for State {
    fn default() -> Self {
        State {
            palettes: vec![[Color::default(); 256]; PALETTES_MAX]
                .into_boxed_slice()
                .try_into()
                .expect("PALETTES_MAX entries"),
            palettes_count: 0,
            palette: [Color::default(); 256],
            rgb_palette: [0; 256],
            yuv_palette: [0; 256],
            colors: [Color::default(); 256],
            fade_palette_diff: [[0; 3]; 256],
            fade_solid_diff: [[0; 3]; 256],
        }
    }
}
