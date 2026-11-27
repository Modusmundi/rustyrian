// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `menus.c` / `menus.h`.

use crate::Game;
use crate::config::{
    DIFFICULTY_EASY, DIFFICULTY_HARD, DIFFICULTY_IMPOSSIBLE, DIFFICULTY_NORMAL, DIFFICULTY_SUICIDE,
    DIFFICULTY_ZINGLON,
};
use crate::episodes::{EPISODE_AVAILABLE, je_init_episode};
use crate::font::{
    ALIGN_CENTER, FONT_LARGE, FONT_NORMAL, Font, draw_font_hv_shadow, draw_font_hv_shadow_aligned,
};
use crate::fonthand::je_text_width;
use crate::game::Flow;
use crate::keyboard::{
    INPUT_ANY, KeyboardInput, LORD_KEY_SYMS, MouseInput, keyboard_get_input, mouse_get_input,
    wait_until_elapsed, wait_until_has_input,
};
use crate::mouse::{MOUSE_POINTER_NORMAL, je_mouse_replace, je_mouse_start};
use crate::nortsong::{je_play_sample_num, set_frame_count};
use crate::palette::{fade_black, fade_palette};
use crate::picload::je_load_pic;
use crate::sndmast::{S_CURSOR, S_SELECT, S_SPRING};
use crate::sprite::je_load_comp_shapes;
use crate::varz::Sprite2Ref;
use crate::video::je_show_vga;
use serde::{Deserialize, Serialize};

/// Mutable globals of `menus.c` (rule R3). The name tables are NUL-terminated
/// `char` buffers filled from the episode data.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    #[serde(with = "crate::serde_big::d2")]
    pub episode_name: [[u8; 31]; 6],
    pub difficulty_name: [[u8; 21]; 7],
    pub gameplay_name: [[u8; 26]; 5],
}

const SDL_BUTTON_LEFT: u8 = 1;
const SDL_BUTTON_RIGHT: u8 = 3;
const SDL_SCANCODE_RETURN: u16 = 40;
const SDL_SCANCODE_ESCAPE: u16 = 41;
const SDL_SCANCODE_SPACE: u16 = 44;
const SDL_SCANCODE_DOWN: u16 = 81;
const SDL_SCANCODE_UP: u16 = 82;
const KMOD_SHIFT: u16 = 0x0003;
const SDLK_RIGHTBRACKET: i32 = b']' as i32;
const SDLK_G: i32 = b'g' as i32;

const X_CENTER: i32 = 320 / 2;
const Y_MENU_HEADER: i32 = 20;
const H_MENU_ITEM: i32 = 13;

/// The text of a NUL-terminated name buffer.
fn c_str(s: &[u8]) -> &[u8] {
    &s[..s.iter().position(|&c| c == 0).unwrap_or(s.len())]
}

/// `JE_textWidth(text, FONT_NORMAL)`.
fn text_width(g: &Game, text: &[u8]) -> i32 {
    je_text_width(g, text, Font::Normal as u32)
}

/// Loads the shop sprites if missing (the mouse pointer is among them).
fn load_pointer_sprites(g: &mut Game) -> Flow {
    if g.sprite.shop_sprite_sheet.data.is_none() {
        je_load_comp_shapes(g, Sprite2Ref::ShopSpriteSheet, b'1')?;
    }
    Ok(())
}

/// Draws the menu background (picture 2) and `header` on `VGAScreen2`.
fn draw_background(g: &mut Game, header: &[u8]) -> Flow {
    let screen2 = g
        .video
        .vga_screen2
        .expect("VGAScreen2 used before init_video");
    je_load_pic(g, screen2, 2, false)?;

    draw_font_hv_shadow_aligned(
        g,
        screen2,
        X_CENTER,
        Y_MENU_HEADER,
        header,
        FONT_LARGE,
        ALIGN_CENTER,
        15,
        -3,
        false,
        2,
    );
    Ok(())
}

/// Copies `VGAScreen2` (background and header) over `VGAScreen`.
fn restore_background(g: &mut Game) {
    let dst = g.video.vga_screen_id();
    let src = g
        .video
        .vga_screen2
        .expect("VGAScreen2 used before init_video");
    let (d, s) = g.video.screen_pair_mut(dst, src);
    let n = (d.pitch * d.h) as usize;
    d.pixels[..n].copy_from_slice(&s.pixels[..n]);
}

/// The first frame's fade-in from black.
fn fade_in(g: &mut Game) -> Flow {
    g.mouse.mouse_cursor = MOUSE_POINTER_NORMAL;

    let colors = g.palette.colors;
    fade_palette(g, &colors, 10, 0, 255)
}

/// Shows the frame with the mouse pointer and waits for input.
fn show_and_wait(g: &mut Game) -> Flow {
    je_mouse_start(g);
    je_show_vga(g);
    je_mouse_replace(g);

    wait_until_elapsed(g)?;
    wait_until_has_input(g, INPUT_ANY)
}

/// Whether (`x`, `y`) lies in the `w`×`h` box at (`left`, `top`).
fn inside(x: i32, y: i32, left: i32, w: i32, top: i32, h: i32) -> bool {
    (left..left + w).contains(&x) && (top..top + h).contains(&y)
}

/// Mouse interaction with a vertical menu: hovering selects an item (with a
/// cursor sound), a left click on it acts, a right click cancels. Returns
/// `(action, cancel)`.
fn menu_mouse(
    g: &mut Game,
    mouse_input: &MouseInput,
    selected_index: &mut usize,
    items: impl Iterator<Item = (i32, i32, i32)>,
) -> (bool, bool) {
    let mut action = false;
    let mut cancel = false;

    let (mx, my) = (mouse_input.x, mouse_input.y);
    for (i, (x_item, w_item, y_item)) in items.enumerate() {
        if (x_item..x_item + w_item).contains(&mx) && (y_item..y_item + H_MENU_ITEM).contains(&my) {
            if *selected_index != i {
                je_play_sample_num(g, S_CURSOR);

                *selected_index = i;
            }

            if mouse_input.button == SDL_BUTTON_LEFT
                && inside(mx, my, x_item, w_item, y_item, H_MENU_ITEM)
            {
                action = true;
            }

            break;
        }
    }

    if mouse_input.button == SDL_BUTTON_RIGHT {
        je_play_sample_num(g, S_SPRING);

        cancel = true;
    }

    (action, cancel)
}

/// Keyboard interaction with a vertical menu of `count` items: up/down move
/// (wrapping), space/return act, escape cancels. Returns `(action, cancel)`.
fn menu_keys(
    g: &mut Game,
    keyboard_input: &KeyboardInput,
    selected_index: &mut usize,
    count: usize,
) -> (bool, bool) {
    let mut action = false;
    let mut cancel = false;

    match keyboard_input.scancode {
        SDL_SCANCODE_UP => {
            je_play_sample_num(g, S_CURSOR);

            *selected_index = if *selected_index == 0 {
                count.wrapping_sub(1)
            } else {
                *selected_index - 1
            };
        }
        SDL_SCANCODE_DOWN => {
            je_play_sample_num(g, S_CURSOR);

            *selected_index = if *selected_index == count.wrapping_sub(1) {
                0
            } else {
                *selected_index + 1
            };
        }
        SDL_SCANCODE_SPACE | SDL_SCANCODE_RETURN => action = true,
        SDL_SCANCODE_ESCAPE => {
            je_play_sample_num(g, S_SPRING);

            cancel = true;
        }
        _ => {}
    }

    (action, cancel)
}

/// The game-mode menu (full game, arcade, two-player arcade; network is shown
/// disabled). Returns false when cancelled.
#[doc(alias = "gameplaySelect")]
pub fn gameplay_select(g: &mut Game) -> Flow<bool> {
    const MENU_ITEM_1_PLAYER_FULL_GAME: usize = 0;
    const MENU_ITEM_1_PLAYER_ARCADE: usize = 1;
    const MENU_ITEM_2_PLAYER_ARCADE: usize = 2;
    const MENU_ITEM_NETWORK: usize = 3;

    load_pointer_sprites(g)?;

    let mut restart = true;

    const MENU_ITEMS_COUNT: usize = 5 - 1;
    let mut selected_index = MENU_ITEM_1_PLAYER_FULL_GAME;

    const Y_MENU_ITEMS: i32 = 54;
    const DY_MENU_ITEMS: i32 = 24;
    let mut w_menu_item = [0i32; MENU_ITEMS_COUNT];

    loop {
        set_frame_count(g, 1);

        if restart {
            let header = g.menus.gameplay_name[0];
            draw_background(g, c_str(&header))?;
        }

        restore_background(g);

        let screen = g.video.vga_screen_id();
        for (i, w_item) in w_menu_item.iter_mut().enumerate() {
            let text = g.menus.gameplay_name[i + 1];
            let text = c_str(&text);

            *w_item = text_width(g, text);
            let x = X_CENTER - *w_item / 2;
            let y = Y_MENU_ITEMS + DY_MENU_ITEMS * i as i32;

            let selected = i == selected_index;
            let disabled = i == MENU_ITEM_NETWORK;

            let value = -4 + if selected { 2 } else { 0 } + if disabled { -4 } else { 0 };
            draw_font_hv_shadow(g, screen, x, y, text, FONT_NORMAL, 15, value, false, 2);
        }

        if restart {
            fade_in(g)?;

            restart = false;
        }

        show_and_wait(g)?;

        // Interaction.

        let mut mouse_input = MouseInput::default();
        let mut keyboard_input = KeyboardInput::default();

        let (action, cancel) = if mouse_get_input(g, INPUT_ANY, Some(&mut mouse_input)) {
            let items = (0..MENU_ITEMS_COUNT).map(|i| {
                let w = w_menu_item[i];
                (X_CENTER - w / 2, w, Y_MENU_ITEMS + DY_MENU_ITEMS * i as i32)
            });
            menu_mouse(g, &mouse_input, &mut selected_index, items)
        } else if keyboard_get_input(g, Some(&mut keyboard_input)) {
            menu_keys(g, &keyboard_input, &mut selected_index, MENU_ITEMS_COUNT)
        } else {
            (false, false)
        };

        if action {
            match selected_index {
                MENU_ITEM_1_PLAYER_FULL_GAME
                | MENU_ITEM_1_PLAYER_ARCADE
                | MENU_ITEM_2_PLAYER_ARCADE => {
                    je_play_sample_num(g, S_SELECT);

                    fade_black(g, 10)?;

                    g.config.one_player_action = selected_index == MENU_ITEM_1_PLAYER_ARCADE;
                    g.config.two_player_mode = selected_index == MENU_ITEM_2_PLAYER_ARCADE;
                    return Ok(true);
                }
                MENU_ITEM_NETWORK => je_play_sample_num(g, S_SPRING),
                _ => {}
            }
        }

        if cancel {
            fade_black(g, 15)?;

            return Ok(false);
        }
    }
}

/// The episode menu (unavailable episodes are shown dimmed and refuse
/// selection). Starts the chosen episode; returns false when cancelled.
#[doc(alias = "episodeSelect")]
pub fn episode_select(g: &mut Game) -> Flow<bool> {
    load_pointer_sprites(g)?;

    let mut restart = true;

    const MENU_ITEMS_COUNT: usize = EPISODE_AVAILABLE;
    let mut selected_index: usize = 0;

    const X_MENU_ITEM: i32 = 20;
    const Y_MENU_ITEMS: i32 = 50;
    const DY_MENU_ITEMS: i32 = 30;
    let mut w_menu_item = [0i32; EPISODE_AVAILABLE];

    loop {
        set_frame_count(g, 1);

        if restart {
            let header = g.menus.episode_name[0];
            draw_background(g, c_str(&header))?;
        }

        restore_background(g);

        let screen = g.video.vga_screen_id();
        for (i, w_item) in w_menu_item.iter_mut().enumerate() {
            let text = g.menus.episode_name[i + 1];
            let text = c_str(&text);

            *w_item = text_width(g, text);
            let y = Y_MENU_ITEMS + DY_MENU_ITEMS * i as i32;

            let selected = i == selected_index;
            let disabled = !g.episodes.episode_avail[i];

            let value = -4 + if selected { 2 } else { 0 } + if disabled { -4 } else { 0 };
            draw_font_hv_shadow(
                g,
                screen,
                X_MENU_ITEM,
                y,
                text,
                FONT_NORMAL,
                15,
                value,
                false,
                2,
            );
        }

        if restart {
            fade_in(g)?;

            restart = false;
        }

        show_and_wait(g)?;

        // Interaction.

        let mut mouse_input = MouseInput::default();
        let mut keyboard_input = KeyboardInput::default();

        let (action, cancel) = if mouse_get_input(g, INPUT_ANY, Some(&mut mouse_input)) {
            let items = (0..MENU_ITEMS_COUNT).map(|i| {
                (
                    X_MENU_ITEM,
                    w_menu_item[i],
                    Y_MENU_ITEMS + DY_MENU_ITEMS * i as i32,
                )
            });
            menu_mouse(g, &mouse_input, &mut selected_index, items)
        } else if keyboard_get_input(g, Some(&mut keyboard_input)) {
            menu_keys(g, &keyboard_input, &mut selected_index, MENU_ITEMS_COUNT)
        } else {
            (false, false)
        };

        if action {
            if g.episodes.episode_avail[selected_index] {
                je_play_sample_num(g, S_SELECT);

                fade_black(g, 10)?;

                je_init_episode(g, (selected_index + 1) as u8)?;
                g.episodes.initial_episode_num = g.episodes.episode_num;
                return Ok(true);
            } else {
                je_play_sample_num(g, S_SPRING);
            }
        }

        if cancel {
            fade_black(g, 15)?;

            return Ok(false);
        }
    }
}

/// The difficulty menu. Three items show at first; shift+G, then shift+],
/// then the four "lord" keys (held together or typed in order) reveal one
/// more each. Sets `difficultyLevel`; returns false when cancelled.
#[doc(alias = "difficultySelect")]
pub fn difficulty_select(g: &mut Game) -> Flow<bool> {
    load_pointer_sprites(g)?;

    let mut restart = true;

    const MENU_ITEMS_COUNT: usize = 7 - 1;
    let mut menu_items_visible_count = MENU_ITEMS_COUNT - 3;
    let mut selected_index: usize = 1;
    let mut lord_progress: usize = 0;

    const Y_MENU_ITEMS: i32 = 54;
    const DY_MENU_ITEMS: i32 = 24;
    let mut w_menu_item = [0i32; MENU_ITEMS_COUNT];

    loop {
        set_frame_count(g, 1);

        if restart {
            let header = g.menus.difficulty_name[0];
            draw_background(g, c_str(&header))?;
        }

        restore_background(g);

        let screen = g.video.vga_screen_id();
        for (i, w_item) in w_menu_item
            .iter_mut()
            .enumerate()
            .take(menu_items_visible_count)
        {
            let text = g.menus.difficulty_name[i + 1];
            let text = c_str(&text);

            *w_item = text_width(g, text);
            let x = X_CENTER - *w_item / 2;
            let y = Y_MENU_ITEMS + DY_MENU_ITEMS * i as i32;

            let selected = i == selected_index;

            let value = -4 + if selected { 2 } else { 0 };
            draw_font_hv_shadow(g, screen, x, y, text, FONT_NORMAL, 15, value, false, 2);
        }

        if restart {
            fade_in(g)?;

            restart = false;
        }

        show_and_wait(g)?;

        // Interaction.

        let mut mouse_input = MouseInput::default();
        let mut keyboard_input = KeyboardInput::default();

        let (action, cancel) = if mouse_get_input(g, INPUT_ANY, Some(&mut mouse_input)) {
            let items = (0..menu_items_visible_count).map(|i| {
                let w = w_menu_item[i];
                (X_CENTER - w / 2, w, Y_MENU_ITEMS + DY_MENU_ITEMS * i as i32)
            });
            menu_mouse(g, &mouse_input, &mut selected_index, items)
        } else if keyboard_get_input(g, Some(&mut keyboard_input)) {
            let r = menu_keys(
                g,
                &keyboard_input,
                &mut selected_index,
                menu_items_visible_count,
            );

            let shift = keyboard_input.r#mod & KMOD_SHIFT != 0;
            match menu_items_visible_count {
                3 => {
                    if shift && keyboard_input.sym == SDLK_G {
                        menu_items_visible_count = 4;
                    }
                }
                4 => {
                    if shift && keyboard_input.sym == SDLK_RIGHTBRACKET {
                        menu_items_visible_count = 5;
                    }
                }
                5 => {
                    if g.keyboard.lord_key_syms_down.iter().all(|&down| down) {
                        menu_items_visible_count = 6;
                    }

                    // Key rollover may prevent holding all four keys at once,
                    // so typing them in order counts too.
                    if lord_progress < LORD_KEY_SYMS.len()
                        && keyboard_input.sym == LORD_KEY_SYMS[lord_progress]
                    {
                        lord_progress += 1;

                        if lord_progress == LORD_KEY_SYMS.len() {
                            menu_items_visible_count = 6;
                        }
                    } else {
                        lord_progress = 0;
                    }
                }
                _ => {}
            }
            r
        } else {
            (false, false)
        };

        if action {
            je_play_sample_num(g, S_SELECT);

            match selected_index {
                0 => g.config.difficulty_level = DIFFICULTY_EASY,
                1 => g.config.difficulty_level = DIFFICULTY_NORMAL,
                2 => g.config.difficulty_level = DIFFICULTY_HARD,
                3 => g.config.difficulty_level = DIFFICULTY_IMPOSSIBLE,
                4 => g.config.difficulty_level = DIFFICULTY_SUICIDE,
                5 => g.config.difficulty_level = DIFFICULTY_ZINGLON,
                _ => {}
            }

            fade_black(g, 10)?;

            return Ok(true);
        }

        if cancel {
            fade_black(g, 15)?;

            return Ok(false);
        }
    }
}
