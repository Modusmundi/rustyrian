// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `xmas.c` / `xmas.h`.

use crate::Game;
use crate::crand::rand_shared;
use crate::font::{ALIGN_CENTER, FONT_NORMAL, draw_font_hv_full_shadow_aligned};
use crate::game::Flow;
use crate::keyboard::{
    INPUT_ANY, KeyboardInput, MouseInput, has_input, keyboard_get_input, mouse_get_input,
    wait_until_elapsed,
};
use crate::mouse::{MOUSE_POINTER_NORMAL, je_mouse_replace, je_mouse_start};
use crate::nortsong::set_frame_count;
use crate::palette::{fade_black, fade_palette};
use crate::sprite::{blit_sprite2_blend, je_load_comp_shapes};
use crate::varz::Sprite2Ref;
use crate::vga256d::fill_rectangle_wh;
use crate::video::je_show_vga;
use serde::{Deserialize, Serialize};

/// Mutable globals of `xmas.c` (rule R3).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub xmas: bool,
}

/// Whether the local date is in December.
pub fn xmas_time(g: &mut Game) -> bool {
    let now = g.platform.time();
    g.platform.localtime(now).mon == 11
}

/// A falling snowflake of the prompt's backdrop.
#[derive(Clone, Copy, Default)]
struct Flake {
    x: i16,
    y: i16,
    dy_acc: u8,
    dy_add: u8,
}

const SDL_BUTTON_LEFT: u8 = 1;
const SDL_BUTTON_RIGHT: u8 = 3;
const SDL_SCANCODE_RETURN: u16 = 40;
const SDL_SCANCODE_ESCAPE: u16 = 41;
const SDL_SCANCODE_SPACE: u16 = 44;
const SDL_SCANCODE_RIGHT: u16 = 79;
const SDL_SCANCODE_LEFT: u16 = 80;

/// Asks whether to turn on Christmas mode; returns true for "Yes".
pub fn xmas_prompt(g: &mut Game) -> Flow<bool> {
    const PROMPT: [&[u8]; 2] = [b"Christmas has been detected.", b"Activate Christmas?"];
    const CHOICES: [&[u8]; 2] = [b"Yes", b"No"];

    if g.sprite.shop_sprite_sheet.data.is_none() {
        // The mouse pointer sprites.
        je_load_comp_shapes(g, Sprite2Ref::ShopSpriteSheet, b'1')?;
    }

    let mut restart = true;

    let mut flakes = [Flake::default(); 80];
    let n = flakes.len();

    const DY_DEN: u8 = 128;

    const X_CENTER: i32 = 320 / 2;
    const Y_PROMPT: i32 = 85;
    const DY_PROMPT: i32 = 15;
    const W_CHOICE: i32 = 40;
    const Y_CHOICE: i32 = 120;
    const H_CHOICE: i32 = 13;

    let mut selected_index: usize = 0;

    set_frame_count(g, 1);

    loop {
        loop {
            if restart {
                for (i, f) in flakes.iter_mut().enumerate() {
                    f.y = (200 + rand_shared(&g.crand) % 200) as i16;
                    f.dy_add = (usize::from(DY_DEN / 2) + i * usize::from(DY_DEN) / n) as u8;
                }
            }

            for f in &mut flakes {
                if f.y >= 200 {
                    f.x = (rand_shared(&g.crand) % 320) as i16;
                    f.y = (200 - 14 - i32::from(f.y)) as i16;
                    f.dy_acc = f.dy_add;
                }

                let temp = rand_shared(&g.crand) & 0xF;
                if temp & 0xE == 0 {
                    f.x = (i32::from(f.x) + (temp & 1) * 2 - 1) as i16;
                }

                let dy_num = u16::from(f.dy_acc) + u16::from(f.dy_add);
                let dy = (dy_num / u16::from(DY_DEN)) as u8;
                f.dy_acc = (dy_num % u16::from(DY_DEN)) as u8;
                f.y = (i32::from(f.y) + i32::from(dy)) as i16;
            }

            let screen = g.video.vga_screen_id();
            fill_rectangle_wh(g, screen, 0, 0, 320, 200, 0x8F);

            // Background snowflakes.
            for f in &flakes[..n * 2 / 3] {
                let (x, y) = (i32::from(f.x), i32::from(f.y));
                blit_sprite2_blend(g, screen, x, y, Sprite2Ref::SpriteSheet8, 225);
            }

            // The prompt.
            for (i, line) in PROMPT.iter().enumerate() {
                let y = Y_PROMPT + DY_PROMPT * i as i32;
                let hue = if i % 2 != 0 { 2 } else { 4 };
                draw_font_hv_full_shadow_aligned(
                    g,
                    screen,
                    X_CENTER,
                    y,
                    line,
                    FONT_NORMAL,
                    ALIGN_CENTER,
                    hue,
                    -2,
                    true,
                    1,
                );
            }

            // The choices.
            for (i, choice) in CHOICES.iter().enumerate() {
                let x = X_CENTER - W_CHOICE / 2 + W_CHOICE * i as i32;

                let selected = selected_index == i;

                let value = if selected { -2 } else { -4 };
                draw_font_hv_full_shadow_aligned(
                    g,
                    screen,
                    x,
                    Y_CHOICE,
                    choice,
                    FONT_NORMAL,
                    ALIGN_CENTER,
                    15,
                    value,
                    true,
                    1,
                );
            }

            // Foreground snowflakes.
            for f in &flakes[n * 2 / 3..] {
                let (x, y) = (i32::from(f.x), i32::from(f.y));
                blit_sprite2_blend(g, screen, x, y, Sprite2Ref::SpriteSheet8, 226);
            }

            if restart {
                g.mouse.mouse_cursor = MOUSE_POINTER_NORMAL;

                let colors = g.palette.palettes[0];
                fade_palette(g, &colors, 10, 0, 255)?;

                restart = false;
            }

            je_mouse_start(g);
            je_show_vga(g);
            je_mouse_replace(g);

            wait_until_elapsed(g)?;

            set_frame_count(g, 1);

            if has_input(g, INPUT_ANY) {
                break;
            }
        }

        // Interaction.

        let mut action = false;
        let mut cancel = false;

        let mut mouse_input = MouseInput::default();
        let mut keyboard_input = KeyboardInput::default();

        if mouse_get_input(g, INPUT_ANY, Some(&mut mouse_input)) {
            // The choice hovered or clicked.
            let (mx, my) = (mouse_input.x, mouse_input.y);
            let rows = Y_CHOICE..Y_CHOICE + H_CHOICE;
            if rows.contains(&my) {
                for i in 0..CHOICES.len() {
                    let x_choice = X_CENTER - W_CHOICE + W_CHOICE * i as i32;
                    let columns = x_choice..x_choice + W_CHOICE;
                    if columns.contains(&mx) {
                        selected_index = i;

                        if mouse_input.button == SDL_BUTTON_LEFT
                            && columns.contains(&mx)
                            && rows.contains(&my)
                        {
                            action = true;
                        }

                        break;
                    }
                }
            }

            if mouse_input.button == SDL_BUTTON_RIGHT {
                cancel = true;
            }
        } else if keyboard_get_input(g, Some(&mut keyboard_input)) {
            match keyboard_input.scancode {
                SDL_SCANCODE_LEFT => {
                    selected_index = if selected_index == 0 {
                        CHOICES.len() - 1
                    } else {
                        selected_index - 1
                    };
                }
                SDL_SCANCODE_RIGHT => {
                    selected_index = if selected_index == CHOICES.len() - 1 {
                        0
                    } else {
                        selected_index + 1
                    };
                }
                SDL_SCANCODE_SPACE | SDL_SCANCODE_RETURN => action = true,
                SDL_SCANCODE_ESCAPE => cancel = true,
                _ => {}
            }
        }

        if action || cancel {
            fade_black(g, 10)?;

            return Ok(!cancel && selected_index == 0);
        }
    }
}
