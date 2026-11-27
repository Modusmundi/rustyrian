// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `palette.c` / `palette.h`.

use crate::file;
use crate::game::{EXIT_FAILURE, Flow, Game, Halt};
use crate::keyboard::{keyboard_clear_input, mouse_clear_input, wait_until_elapsed};
use crate::logging::log_fatal;
use crate::nortsong::set_frame_count;
use crate::sdl::map_rgb;
use crate::video::je_show_vga;
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

#[doc(alias = "loadPals")]
pub fn load_pals(g: &mut Game) -> Flow {
    let filename = "palette.dat";

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

    let count = (file::file_get_length(&mut file) / (256 * 3)) as u64 as usize;
    g.palette.palettes_count = count.min(PALETTES_MAX);

    for p in 0..g.palette.palettes_count {
        let mut data = [0u8; 3 * 256];
        file::file_read_exactly(&mut file, &mut data);

        for (color, rgb) in g.palette.palettes[p]
            .iter_mut()
            .zip(data.as_chunks::<3>().0)
        {
            // 6-bit VGA components scaled to 8 bits, with the top bits
            // repeated in the low bits so that 63 becomes 255.
            color.r = (rgb[0] << 2) | (rgb[0] >> 4);
            color.g = (rgb[1] << 2) | (rgb[1] >> 4);
            color.b = (rgb[2] << 2) | (rgb[2] >> 4);
        }
    }

    if file.error {
        let message = format!(
            "Failed to read from file '{}': {}",
            filename,
            file::file_get_error(&file)
        );
        log_fatal(g, &message);
        return Err(Halt { code: EXIT_FAILURE });
    }

    file::file_close(&mut file);
    Ok(())
}

/// Recomputes `rgb_palette[i]` and `yuv_palette[i]` from `palette[i]`.
fn update_color(g: &mut Game, i: usize) {
    let format = g
        .video
        .main_window_tex_format
        .as_ref()
        .expect("main_window_tex_format used before init_video");
    let c = g.palette.palette[i];
    g.palette.rgb_palette[i] = map_rgb(format, c.r, c.g, c.b);
    g.palette.yuv_palette[i] = rgb_to_yuv(i32::from(c.r), i32::from(c.g), i32::from(c.b));
}

pub fn set_palette(g: &mut Game, colors: &Palette, first_color: u32, last_color: u32) {
    for i in first_color..=last_color {
        let i = i as usize;
        g.palette.palette[i] = colors[i];
        update_color(g, i);
    }
}

pub fn set_colors(g: &mut Game, color: Color, first_color: u32, last_color: u32) {
    for i in first_color..=last_color {
        let i = i as usize;
        g.palette.palette[i] = color;
        update_color(g, i);
    }
}

pub fn init_step_fade_palette(
    g: &Game,
    diff: &mut [[i32; 3]; 256],
    colors: &Palette,
    first_color: u32,
    last_color: u32,
) {
    for i in first_color..=last_color {
        let i = i as usize;
        let p = g.palette.palette[i];
        diff[i][0] = i32::from(colors[i].r) - i32::from(p.r);
        diff[i][1] = i32::from(colors[i].g) - i32::from(p.g);
        diff[i][2] = i32::from(colors[i].b) - i32::from(p.b);
    }
}

pub fn init_step_fade_solid(
    g: &Game,
    diff: &mut [[i32; 3]; 256],
    color: Color,
    first_color: u32,
    last_color: u32,
) {
    for i in first_color..=last_color {
        let i = i as usize;
        let p = g.palette.palette[i];
        diff[i][0] = i32::from(color.r) - i32::from(p.r);
        diff[i][1] = i32::from(color.g) - i32::from(p.g);
        diff[i][2] = i32::from(color.b) - i32::from(p.b);
    }
}

/// Moves `palette[first_color..=last_color]` one of `steps` steps along
/// `diff`. `steps` must be positive.
pub fn step_fade_palette(
    g: &mut Game,
    diff: &mut [[i32; 3]; 256],
    steps: i32,
    first_color: u32,
    last_color: u32,
) {
    for i in first_color..=last_color {
        let i = i as usize;
        let delta = [diff[i][0] / steps, diff[i][1] / steps, diff[i][2] / steps];

        diff[i][0] = diff[i][0].wrapping_sub(delta[0]);
        diff[i][1] = diff[i][1].wrapping_sub(delta[1]);
        diff[i][2] = diff[i][2].wrapping_sub(delta[2]);

        let p = &mut g.palette.palette[i];
        p.r = i32::from(p.r).wrapping_add(delta[0]) as u8;
        p.g = i32::from(p.g).wrapping_add(delta[1]) as u8;
        p.b = i32::from(p.b).wrapping_add(delta[2]) as u8;

        update_color(g, i);
    }
}

/// Fades `palette[first_color..=last_color]` towards `colors` over `steps`
/// frames, then discards the input received meanwhile.
pub fn fade_palette(
    g: &mut Game,
    colors: &Palette,
    mut steps: i32,
    first_color: u32,
    last_color: u32,
) -> Flow {
    debug_assert!(steps > 0);

    let mut diff = g.palette.fade_palette_diff;
    init_step_fade_palette(g, &mut diff, colors, first_color, last_color);
    g.palette.fade_palette_diff = diff;

    while steps > 0 {
        set_frame_count(g, 1);

        let mut diff = g.palette.fade_palette_diff;
        step_fade_palette(g, &mut diff, steps, first_color, last_color);
        g.palette.fade_palette_diff = diff;

        je_show_vga(g);

        wait_until_elapsed(g)?;
        steps -= 1;
    }

    keyboard_clear_input(g);
    mouse_clear_input(g);
    Ok(())
}

/// Fades `palette[first_color..=last_color]` towards the single `color` over
/// `steps` frames, then discards the input received meanwhile.
pub fn fade_solid(
    g: &mut Game,
    color: Color,
    mut steps: i32,
    first_color: u32,
    last_color: u32,
) -> Flow {
    debug_assert!(steps > 0);

    let mut diff = g.palette.fade_solid_diff;
    init_step_fade_solid(g, &mut diff, color, first_color, last_color);
    g.palette.fade_solid_diff = diff;

    while steps > 0 {
        set_frame_count(g, 1);

        let mut diff = g.palette.fade_solid_diff;
        step_fade_palette(g, &mut diff, steps, first_color, last_color);
        g.palette.fade_solid_diff = diff;

        je_show_vga(g);

        wait_until_elapsed(g)?;
        steps -= 1;
    }

    keyboard_clear_input(g);
    mouse_clear_input(g);
    Ok(())
}

pub fn fade_black(g: &mut Game, steps: i32) -> Flow {
    let black = Color {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };
    fade_solid(g, black, steps, 0, 255)
}

pub fn fade_white(g: &mut Game, steps: i32) -> Flow {
    let white = Color {
        r: 255,
        g: 255,
        b: 255,
        a: 0,
    };
    fade_solid(g, white, steps, 0, 255)
}

pub fn rgb_to_yuv(r: i32, g: i32, b: i32) -> u32 {
    let y = r.wrapping_add(g).wrapping_add(b) >> 2;
    let u = 128i32.wrapping_add(r.wrapping_sub(b) >> 2);
    let v = 128i32.wrapping_add(
        r.wrapping_neg()
            .wrapping_add(g.wrapping_mul(2))
            .wrapping_sub(b)
            >> 3,
    );
    (y << 16).wrapping_add(u << 8).wrapping_add(v) as u32
}
