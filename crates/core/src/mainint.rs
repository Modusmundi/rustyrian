// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `mainint.c` / `mainint.h`.

use serde::{Deserialize, Serialize};

pub const MAX_PAGE: u8 = 8;
pub const TOPICS: usize = 6;
/// `topicStart`.
pub const TOPIC_START: [u8; TOPICS] = [0, 1, 2, 3, 7, 255];

/// Mutable globals of `mainint.c` (rule R3).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// Fire, left fire, right fire, mode swap.
    pub button: [bool; 4],
    pub constant_last_x: i8,
    pub text_erase: u16,
    pub upgrade_cost: u16,
    pub downgrade_cost: u16,
    pub perform_save: bool,
    pub jump_section: bool,
    /// Whether DisplayText uses the last 16 colors.
    pub use_last_bank: bool,
    pub pause_pressed: bool,
    pub ingamemenu_pressed: bool,
}

use crate::Game;
use crate::config::{
    DIFFICULTY_EASY, DIFFICULTY_HARD, DIFFICULTY_IMPOSSIBLE, DIFFICULTY_INSANITY,
    DIFFICULTY_MANIACAL, DIFFICULTY_NORMAL, DIFFICULTY_NORTANEOUS, DIFFICULTY_SUICIDE,
    DIFFICULTY_ZINGLON, je_init_processor_type, je_load_game, je_save_game, je_set_new_game_speed,
};
use crate::episodes::{EPISODE_AVAILABLE, FIRST_LEVEL, je_find_next_episode, je_init_episode};
use crate::file::{data_file_open, file_close, file_get_error};
use crate::font::{
    ALIGN_CENTER, ALIGN_LEFT, ALIGN_RIGHT, FONT_LARGE, FONT_NORMAL, FONT_SMALL,
    draw_font_hv_aligned, draw_font_hv_shadow, draw_font_hv_shadow_aligned,
};
use crate::fonthand::{
    DARKEN, FONT_MAP, FULL_SHADE, PART_SHADE, je_d_string, je_font_center, je_out_text,
    je_out_text_adjust, je_out_text_glow, je_text_shade, je_text_width, je_update_warning,
};
use crate::game::{EXIT_FAILURE, Flow, Halt};
use crate::helptext::{je_h_box, je_help_box, read_encrypted_string};
use crate::keyboard::{
    INPUT_ANY, INPUT_NO_MOTION, KeyboardInput, MouseInput, get_input, has_input,
    keyboard_get_input, mouse_get_input, mouse_set_relative, wait_until_elapsed,
    wait_until_get_input, wait_until_get_input_or_elapsed, wait_until_has_input,
    wait_until_has_input_or_elapsed,
};
use crate::logging::log_fatal;
use crate::loudness::{audio, fade_song, play_song, set_volume};
use crate::mouse::{
    MOUSE_POINTER_LEFT, MOUSE_POINTER_NORMAL, MOUSE_POINTER_RIGHT, je_mouse_replace, je_mouse_start,
};
use crate::mtrand::mt_rand;
use crate::musmast::SONG_MAPVIEW;
use crate::network::{
    PACKET_GAME_MENU, PACKET_GAME_PAUSE, PACKET_GAME_QUIT, PACKET_WAITING, network_check,
    network_is_sync, network_prepare, network_send, network_update,
};
use crate::nortsong::{
    get_frame_count2_ticks, je_change_volume, je_play_sample_num, set_frame_count, set_frame_count2,
};
use crate::nortvars::je_bar_draw_shadow;
use crate::palette::{Color, Palette, fade_black, fade_palette, set_colors, set_palette};
use crate::pcxmast::PCXPAL;
use crate::picload::je_load_pic;
use crate::player::{FRONT_WEAPON, LEFT_SIDEKICK, REAR_WEAPON, RIGHT_SIDEKICK, all_players_alive};
use crate::sndmast::{S_CLICK, S_CLINK, S_CURSOR, S_ITEM, S_SELECT, S_SPRING, V_LEVEL_END};
use crate::sprite::{
    EXTRA_SHAPES, FONT_SHAPES, OPTION_SHAPES, SMALL_FONT_SHAPES, TINY_FONT, blit_sprite,
    blit_sprite_dark, blit_sprite_hv, blit_sprite2, blit_sprite2x2, free_sprites,
    je_load_comp_shapes, load_sprites_file, sprite,
};
use crate::varz::{
    KEYBOARD_COMBOS, SA_NONE, SHIP_COMBOS, SHIP_COMBOS_B, Sprite2Ref, je_tyrian_halt,
};
use crate::vga256d::{fill_rectangle_wh, fill_rectangle_xy, je_bar_shade};
use crate::video::{ScreenId, je_clr256, je_show_vga};

const SDL_BUTTON_LEFT: u8 = 1;
const SDL_BUTTON_RIGHT: u8 = 3;

const SDL_SCANCODE_W: u16 = 26;
const SDL_SCANCODE_RETURN: u16 = 40;
const SDL_SCANCODE_ESCAPE: u16 = 41;
const SDL_SCANCODE_BACKSPACE: u16 = 42;
const SDL_SCANCODE_SPACE: u16 = 44;
const SDL_SCANCODE_F11: usize = 68;
const SDL_SCANCODE_DELETE: u16 = 76;
const SDL_SCANCODE_RIGHT: u16 = 79;
const SDL_SCANCODE_LEFT: u16 = 80;
const SDL_SCANCODE_DOWN: u16 = 81;
const SDL_SCANCODE_UP: u16 = 82;
const SDL_SCANCODE_LCTRL: u16 = 224;
const SDL_SCANCODE_LALT: u16 = 226;
const SDL_SCANCODE_RCTRL: u16 = 228;
const SDL_SCANCODE_RALT: u16 = 230;
const SDL_SCANCODE_ESCAPE_INDEX: usize = 41;
const SDL_SCANCODE_P_INDEX: usize = 19;

/// A C string: `s` up to its first NUL.
fn cstr(s: &[u8]) -> &[u8] {
    &s[..s.iter().position(|&b| b == 0).unwrap_or(s.len())]
}

/// `strcpy(dst, src)`: the bytes after the copied NUL keep their values.
fn strcpy(dst: &mut [u8], src: &[u8]) {
    let s = cstr(src);
    dst[..s.len()].copy_from_slice(s);
    dst[s.len()] = 0;
}

/// `snprintf(buffer, size, ...)` of `text`: what fits before the NUL.
fn snprintf(size: usize, text: Vec<u8>) -> Vec<u8> {
    let mut text = text;
    text.truncate(size - 1);
    text
}

/// `fmt` with `%s` arguments as raw C-string bytes.
fn cat(parts: &[&[u8]]) -> Vec<u8> {
    let mut v = Vec::new();
    for p in parts {
        v.extend_from_slice(cstr(p));
    }
    v
}

fn misc(g: &Game, i: usize) -> Vec<u8> {
    cstr(&g.helptext.misc_text[i]).to_vec()
}

fn vga(g: &Game) -> ScreenId {
    g.video.vga_screen_id()
}

fn seg(g: &Game) -> ScreenId {
    g.video
        .vga_screen_seg
        .expect("VGAScreenSeg used before init_video")
}

fn screen2(g: &Game) -> ScreenId {
    g.video
        .vga_screen2
        .expect("VGAScreen2 used before init_video")
}

/// `memcpy(dst->pixels, src->pixels, pitch * h)`.
fn copy_screen(g: &mut Game, dst: ScreenId, src: ScreenId) {
    let (s, d) = g.video.screen_pair_mut(src, dst);
    d.pixels.copy_from_slice(&s.pixels);
}

/// `SDLNet_Read16(&packet_in[0]->data[0])` when `packet_in[0]` is set.
fn packet_in_type(g: &Game) -> Option<u16> {
    g.network.packet_in[0]
        .as_ref()
        .map(|p| u16::from_be_bytes([p.data[0], p.data[1]]))
}

/// Loads the shop sprite sheet if it is not loaded (mouse pointer and arrow
/// sprites).
fn need_shop_sprite_sheet(g: &mut Game) -> Flow {
    if g.sprite.shop_sprite_sheet.data.is_none() {
        je_load_comp_shapes(g, Sprite2Ref::ShopSpriteSheet, b'1')?;
    }
    Ok(())
}

/// Draws a message at the bottom text window on the playing screen.
#[doc(alias = "JE_drawTextWindow")]
pub fn je_draw_text_window(g: &mut Game, text: &[u8]) {
    let seg = seg(g);
    if g.mainint.text_erase > 0 {
        // erase current text
        blit_sprite(g, seg, 16, 189, OPTION_SHAPES as u32, 36); // in-game text area
    }

    g.mainint.text_erase = 100;
    je_out_text(g, seg, 20, 190, cstr(text), 0, 4);
}

#[doc(alias = "JE_outCharGlow")]
pub fn je_out_char_glow(g: &mut Game, x: u16, y: u16, s: &[u8]) -> Flow {
    let s = cstr(s);
    let mut glowcol = [0i8; 60];
    let mut glowcolc = [0i8; 60];
    let mut textloc = [0u16; 60];

    let bank: u8 = if g.fonthand.warning_red {
        7
    } else if g.mainint.use_last_bank {
        15
    } else {
        14
    };

    if s.is_empty() {
        return Ok(());
    }

    let screen = vga(g);
    if g.nortsong.frame_count_max == 0 {
        je_text_shade(
            g,
            screen,
            i32::from(x),
            i32::from(y),
            s,
            u32::from(bank),
            0,
            PART_SHADE,
        );
        je_show_vga(g);
        return Ok(());
    }

    let maxloc = s.len() as i16;
    for z in 0..60 {
        glowcol[z] = -8;
        glowcolc[z] = 1;
    }

    let mut loc = x as i16;
    for z in 0..maxloc as usize {
        textloc[z] = loc as u16;

        let sprite_id = i32::from(FONT_MAP[usize::from(s[z])]);

        if s[z] == b' ' {
            loc = loc.wrapping_add(6);
        } else if sprite_id != -1 {
            let w = i32::from(sprite(g, TINY_FONT as u32, sprite_id as u32).width);
            loc = (i32::from(loc) + w + 1) as i16;
        }
    }

    let mut loc: i16 = 0;
    while (loc as i32 as u32 as usize) < s.len() + 28 {
        if g.keyboard.esc_pressed {
            break;
        }

        set_frame_count(g, 1);

        let mut sprite_id = -1;

        let mut z = loc.wrapping_sub(28);
        while z <= loc {
            if z >= 0 && z < maxloc {
                let zu = z as usize;
                sprite_id = i32::from(FONT_MAP[usize::from(s[zu])]);

                if sprite_id != -1 {
                    blit_sprite_hv(
                        g,
                        screen,
                        i32::from(textloc[zu]),
                        i32::from(y),
                        TINY_FONT as u32,
                        sprite_id as u32,
                        bank,
                        glowcol[zu],
                    );

                    glowcol[zu] = glowcol[zu].wrapping_add(glowcolc[zu]);
                    if glowcol[zu] > 9 {
                        glowcolc[zu] = -1;
                    }
                }
            }
            z = z.wrapping_add(1);
        }
        if sprite_id != -1 {
            z = z.wrapping_sub(1);
            if z < maxloc {
                blit_sprite_dark(
                    g,
                    screen,
                    i32::from(textloc[z as usize]) + 1,
                    i32::from(y) + 1,
                    TINY_FONT as u32,
                    sprite_id as u32,
                    true,
                );
            }
        }

        je_show_vga(g);

        let mut frame_count = g.nortsong.frame_count_max;
        while frame_count > 0 {
            if g.fonthand.level_warning_display {
                je_update_warning(g, screen);
            }

            wait_until_elapsed(g)?;

            if get_input(g) {
                g.nortsong.frame_count_max = 0;
            }

            if g.keyboard.esc_pressed {
                break;
            }

            set_frame_count(g, 1);
            frame_count -= 1;
        }
        loc = loc.wrapping_add(1);
    }
    Ok(())
}

/// Rear weapon pattern indicator.
#[doc(alias = "JE_drawPortConfigButtons")]
pub fn je_draw_port_config_buttons(g: &mut Game) {
    if g.config.two_player_mode {
        return;
    }

    let seg = seg(g);
    let (a, b) = if g.player.player[0].weapon_mode == 1 {
        (18, 19) // lit, unlit
    } else {
        (19, 18) // unlit, lit
    };
    blit_sprite(g, seg, 285, 44, OPTION_SHAPES as u32, a);
    blit_sprite(g, seg, 302, 44, OPTION_SHAPES as u32, b);
}

/// Selected-index step for the up/down keys of a menu of `count` items.
fn menu_prev(i: usize, count: usize) -> usize {
    if i == 0 { count - 1 } else { i - 1 }
}

fn menu_next(i: usize, count: usize) -> usize {
    if i == count - 1 { 0 } else { i + 1 }
}

#[doc(alias = "JE_helpSystem")]
pub fn je_help_system(g: &mut Game, start_topic: u8) -> Flow {
    need_shop_sprite_sheet(g)?; // need mouse pointer sprites

    let mut topic = start_topic;

    let mut restart = true;

    let menu_items_count = TOPICS - 1;
    let mut selected_index = 0usize;

    let x_center = 320 / 2;
    let y_menu_header = 30;
    let y_menu_items = 60;
    let dy_menu_items = 20;
    let h_menu_item = 13;
    let mut w_menu_item = [0i32; TOPICS - 1];

    loop {
        set_frame_count(g, 1);

        if restart {
            play_song(g, u32::from(SONG_MAPVIEW))?;

            let s2 = screen2(g);
            je_load_pic(g, s2, 2, false)?;
        }

        if topic > 1 {
            if !help_system_page(g, &mut topic, &mut restart)? {
                return Ok(());
            }

            selected_index = usize::from(topic) - 1;
            topic = 1;
            continue;
        }

        // Restore background.
        let screen = vga(g);
        copy_screen(g, screen, screen2(g));

        // Draw header.
        let header = g.helptext.topic_name[0];
        draw_font_hv_shadow_aligned(
            g,
            screen,
            x_center,
            y_menu_header,
            cstr(&header),
            FONT_LARGE,
            ALIGN_CENTER,
            15,
            -3,
            false,
            2,
        );

        // Draw menu items.
        for (i, w) in w_menu_item.iter_mut().enumerate() {
            let text = g.helptext.topic_name[i + 1];

            *w = je_text_width(g, cstr(&text), FONT_NORMAL as u32);
            let y = y_menu_items + dy_menu_items * i as i32;

            let selected = i == selected_index;

            draw_font_hv_shadow_aligned(
                g,
                screen,
                x_center,
                y,
                cstr(&text),
                FONT_NORMAL,
                ALIGN_CENTER,
                15,
                -3 + if selected { 2 } else { 0 },
                false,
                2,
            );
        }

        g.mouse.mouse_cursor = MOUSE_POINTER_NORMAL;

        if restart {
            let colors = g.palette.colors;
            fade_palette(g, &colors, 10, 0, 255)?;

            restart = false;
        }

        je_mouse_start(g);
        je_show_vga(g);
        je_mouse_replace(g);

        wait_until_elapsed(g)?;
        wait_until_has_input(g, INPUT_ANY)?;

        // Handle interaction.

        let mut action = false;
        let mut done = false;

        let mut mouse_input = MouseInput::default();
        let mut keyboard_input = KeyboardInput::default();

        if mouse_get_input(g, INPUT_ANY, Some(&mut mouse_input)) {
            // Find menu item that was hovered or clicked.
            for (i, &w) in w_menu_item.iter().enumerate() {
                let x_menu_item = x_center - w / 2;
                if mouse_input.x >= x_menu_item && mouse_input.x < x_menu_item + w {
                    let y_menu_item = y_menu_items + dy_menu_items * i as i32;
                    if mouse_input.y >= y_menu_item && mouse_input.y < y_menu_item + h_menu_item {
                        if selected_index != i {
                            je_play_sample_num(g, S_CURSOR);

                            selected_index = i;
                        }

                        if mouse_input.button == SDL_BUTTON_LEFT {
                            action = true;
                        }

                        break;
                    }
                }
            }

            if mouse_input.button == SDL_BUTTON_RIGHT {
                je_play_sample_num(g, S_SPRING);

                done = true;
            }
        } else if keyboard_get_input(g, Some(&mut keyboard_input)) {
            match keyboard_input.scancode {
                SDL_SCANCODE_UP => {
                    je_play_sample_num(g, S_CURSOR);

                    selected_index = menu_prev(selected_index, menu_items_count);
                }
                SDL_SCANCODE_DOWN => {
                    je_play_sample_num(g, S_CURSOR);

                    selected_index = menu_next(selected_index, menu_items_count);
                }
                SDL_SCANCODE_SPACE | SDL_SCANCODE_RETURN => {
                    action = true;
                }
                SDL_SCANCODE_ESCAPE => {
                    je_play_sample_num(g, S_SPRING);

                    done = true;
                }
                _ => {}
            }
        }

        if action {
            je_play_sample_num(g, S_SELECT);

            topic = (selected_index + 2) as u8;

            if selected_index == menu_items_count - 1 {
                done = true;
            }
        }

        if done {
            fade_black(g, 15)?;

            return Ok(());
        }
    }
}

/// `JE_HBox` calls per help page: (y, message number).
const HELP_PAGES: [&[(i32, u8)]; MAX_PAGE as usize] = [
    &[(20, 2), (50, 5), (80, 21), (110, 1), (140, 28)], // One-Player Menu
    &[(20, 1), (60, 2), (100, 21), (140, 28)],          // Two-Player Menu
    &[(20, 5), (70, 6), (110, 7)],                      // Upgrade Ship
    &[(20, 8), (55, 9), (87, 10), (120, 11), (170, 13)],
    &[(20, 14), (80, 15), (120, 16)],
    &[(20, 17), (40, 18), (130, 20)],
    &[(20, 21), (70, 22), (110, 23), (140, 24)], // Options
    &[(20, 25), (60, 26), (100, 27), (140, 28), (170, 29)],
];

/// `helpSystemPage(&topic, &restart)`.
#[doc(alias = "helpSystemPage")]
pub fn help_system_page(g: &mut Game, topic: &mut u8, restart: &mut bool) -> Flow<bool> {
    let mut page = TOPIC_START[usize::from(*topic) - 1];

    let x_center = 320 / 2;

    loop {
        if page == 0 {
            *topic = 1;
            return Ok(true);
        } else if page > MAX_PAGE {
            *topic = (TOPICS - 1) as u8;
            return Ok(true);
        }

        for (temp, &start) in TOPIC_START.iter().enumerate() {
            if start <= page {
                *topic = temp as u8 + 1;
            } else {
                break;
            }
        }

        set_frame_count(g, 1);

        // Restore background.
        let screen = vga(g);
        copy_screen(g, screen, screen2(g));

        fill_rectangle_wh(g, screen, 0, 192, 320, 8, 0);

        let text = g.helptext.topic_name[usize::from(*topic) - 1];

        // Draw header.
        draw_font_hv_shadow_aligned(
            g,
            screen,
            x_center,
            1,
            cstr(&text),
            FONT_NORMAL,
            ALIGN_CENTER,
            15,
            -3,
            false,
            2,
        );

        // Draw footer.
        let page_in_topic = i32::from(page) - i32::from(TOPIC_START[usize::from(*topic) - 1]) + 1;
        let mut buffer = misc(g, 24);
        buffer.extend_from_slice(format!(" {page_in_topic}").as_bytes());
        let buffer = snprintf(128, buffer);
        draw_font_hv_aligned(g, screen, 10, 192, &buffer, FONT_SMALL, ALIGN_LEFT, 13, 5);

        let mut buffer = misc(g, 25);
        buffer.extend_from_slice(format!(" {page} of {MAX_PAGE}").as_bytes());
        let buffer = snprintf(128, buffer);
        draw_font_hv_aligned(
            g,
            screen,
            320 - 10,
            192,
            &buffer,
            FONT_SMALL,
            ALIGN_RIGHT,
            13,
            5,
        );

        // Draw text.
        if (1..=MAX_PAGE).contains(&page) {
            for &(y, message) in HELP_PAGES[usize::from(page) - 1] {
                je_h_box(g, screen, 10, y, message, 60, 8, 12, 3);
            }
        }

        if *restart {
            let colors = g.palette.colors;
            fade_palette(g, &colors, 10, 0, 255)?;

            *restart = false;
        }

        loop {
            g.mouse.mouse_cursor = if g.keyboard.mouse_x < x_center {
                MOUSE_POINTER_LEFT
            } else {
                MOUSE_POINTER_RIGHT
            };

            je_mouse_start(g);
            je_show_vga(g);
            je_mouse_replace(g);

            wait_until_elapsed(g)?;
            wait_until_has_input(g, INPUT_ANY)?;

            if has_input(g, INPUT_NO_MOTION) {
                break;
            }

            set_frame_count(g, 1);
        }

        // Handle interaction.

        let mut done = false;

        let mut mouse_input = MouseInput::default();
        let mut keyboard_input = KeyboardInput::default();

        if mouse_get_input(g, INPUT_NO_MOTION, Some(&mut mouse_input)) {
            match mouse_input.button {
                SDL_BUTTON_LEFT => {
                    je_play_sample_num(g, S_CURSOR);

                    if mouse_input.x < x_center {
                        page = page.wrapping_sub(1);
                    } else {
                        page = page.wrapping_add(1);
                    }
                }
                SDL_BUTTON_RIGHT => {
                    je_play_sample_num(g, S_SPRING);

                    done = true;
                }
                _ => {}
            }
        } else if keyboard_get_input(g, Some(&mut keyboard_input)) {
            match keyboard_input.scancode {
                SDL_SCANCODE_LEFT => {
                    je_play_sample_num(g, S_CURSOR);

                    page = page.wrapping_sub(1);
                }
                SDL_SCANCODE_RIGHT | SDL_SCANCODE_SPACE | SDL_SCANCODE_RETURN => {
                    je_play_sample_num(g, S_CURSOR);

                    page = page.wrapping_add(1);
                }
                SDL_SCANCODE_ESCAPE => {
                    je_play_sample_num(g, S_SPRING);

                    done = true;
                }
                _ => {}
            }
        }

        if done {
            fade_black(g, 15)?;

            return Ok(false);
        }
    }
}

/// Cost to upgrade a weapon power from `power - 1` (where `power == 0`
/// indicates an unupgraded weapon). C sums `power + (power-1) + … + 1` in
/// an `unsigned int`; the closed form gives the same value modulo 2³².
pub fn weapon_upgrade_cost(base_cost: i64, power: u32) -> i64 {
    debug_assert!(power <= 11);

    // 0 1 3 6 10 15 21 29 ...
    let n = u64::from(power);
    let temp = (u128::from(n) * u128::from(n + 1) / 2) as u32;

    base_cost.wrapping_mul(i64::from(temp))
}

#[doc(alias = "JE_getCost")]
pub fn je_get_cost(g: &mut Game, item_type: u8, item_num: u16) -> u64 {
    let n = usize::from(item_num);
    let mut cost: i64 = 0;

    match item_type {
        2 => {
            cost = if item_num > 90 {
                100
            } else {
                i64::from(g.episodes.ships[n].cost)
            };
        }
        3 | 4 => {
            cost = i64::from(g.episodes.weapon_port[n].cost);

            let port = usize::from(item_type - 3);
            let item_power = (i32::from(g.player.player[0].items.weapon[port].power) - 1) as u32;

            g.mainint.downgrade_cost = weapon_upgrade_cost(cost, item_power) as u16;
            g.mainint.upgrade_cost = weapon_upgrade_cost(cost, item_power.wrapping_add(1)) as u16;
        }
        5 => cost = i64::from(g.episodes.shields[n].cost),
        6 => cost = i64::from(g.episodes.power_sys[n].cost),
        7 | 8 => cost = i64::from(g.episodes.options[n].cost),
        _ => {}
    }

    cost as u64
}

#[doc(alias = "JE_loadScreen")]
pub fn je_load_screen(g: &mut Game) -> Flow<bool> {
    need_shop_sprite_sheet(g)?; // need mouse pointer and arrow sprites

    let mut restart = true;

    let mut players_index = 0usize;
    let menu_items_count = 12usize;
    let mut selected_index = 0usize;

    let x_center = 320 / 2;
    let y_menu_header = 5;
    let x_menu_item = 10;
    let x_menu_item_name = x_menu_item;
    let x_menu_item_last_level = 120;
    let x_menu_item_episode = 250;
    let w_menu_item = 300;
    let y_menu_items = 30;
    let dy_menu_items = 13;
    let h_menu_item = 8;
    let x_left_control = 83;
    let x_right_control = 213;
    let w_control = 24;
    let y_controls = 179;

    loop {
        set_frame_count(g, 1);

        let screen = vga(g);
        let s2 = screen2(g);
        if restart {
            je_load_pic(g, s2, 2, false)?;
            fill_rectangle_wh(g, s2, 0, 192, 320, 8, 0);
        }

        // Restore background.
        copy_screen(g, screen, s2);

        // Draw header.
        let header = misc(g, 38 + players_index);
        draw_font_hv_shadow_aligned(
            g,
            screen,
            x_center,
            y_menu_header,
            &header,
            FONT_LARGE,
            ALIGN_CENTER,
            15,
            -3,
            false,
            2,
        );

        // Draw menu items.

        for i in 0..menu_items_count {
            let y = y_menu_items + dy_menu_items * i as i32;

            let selected = i == selected_index;

            if i == menu_items_count - 1 {
                let t = misc(g, 33);
                je_text_shade(
                    g,
                    screen,
                    x_menu_item_name,
                    y,
                    &t,
                    13,
                    if selected { 6 } else { 2 },
                    FULL_SHADE,
                );
                continue;
            }

            let save_file = g.config.save_files[players_index * 11 + i];

            let disabled = save_file.level == 0;

            let b2 = g.helptext.misc_text_b[2];
            if disabled {
                let t = misc(g, 2);
                je_text_shade(
                    g,
                    screen,
                    x_menu_item_name,
                    y,
                    &t,
                    13,
                    if selected { 6 } else { 0 },
                    FULL_SHADE,
                );

                let buffer = snprintf(22, cat(&[&b2, b" -----"]));
                je_text_shade(
                    g,
                    screen,
                    x_menu_item_last_level,
                    y,
                    &buffer,
                    5,
                    if selected { 6 } else { 0 },
                    FULL_SHADE,
                );
            } else {
                je_text_shade(
                    g,
                    screen,
                    x_menu_item_name,
                    y,
                    cstr(&save_file.name),
                    13,
                    if selected { 6 } else { 2 },
                    FULL_SHADE,
                );

                let buffer = snprintf(22, cat(&[&b2, b" ", &save_file.level_name]));
                je_text_shade(
                    g,
                    screen,
                    x_menu_item_last_level,
                    y,
                    &buffer,
                    5,
                    if selected { 6 } else { 2 },
                    FULL_SHADE,
                );

                let b1 = g.helptext.misc_text_b[1];
                let mut buffer = cat(&[&b1]);
                buffer.extend_from_slice(format!(" {}", save_file.episode).as_bytes());
                let buffer = snprintf(22, buffer);
                je_text_shade(
                    g,
                    screen,
                    x_menu_item_episode,
                    y,
                    &buffer,
                    5,
                    if selected { 6 } else { 2 },
                    FULL_SHADE,
                );
            }
        }

        // Draw paging controls.

        let left_control_visible = players_index > 0;
        let right_control_visible = players_index < 1;

        if left_control_visible {
            blit_sprite2x2(
                g,
                screen,
                x_left_control,
                y_controls,
                Sprite2Ref::ShopSpriteSheet,
                279,
            );
        }

        if right_control_visible {
            blit_sprite2x2(
                g,
                screen,
                x_right_control,
                y_controls,
                Sprite2Ref::ShopSpriteSheet,
                281,
            );
        }

        let help = misc(g, 55);
        je_help_box(g, screen, 103, 182, &help, 25, 7, 15, 1, FULL_SHADE as u8);

        if restart {
            g.mouse.mouse_cursor = MOUSE_POINTER_NORMAL;

            let colors = g.palette.colors;
            fade_palette(g, &colors, 10, 0, 255)?;

            restart = false;
        }

        je_mouse_start(g);
        je_show_vga(g);
        je_mouse_replace(g);

        wait_until_elapsed(g)?;
        wait_until_has_input(g, INPUT_ANY)?;

        // Handle interaction.

        let mut left_action = false;
        let mut right_action = false;
        let mut action = false;
        let mut done = false;

        let mut mouse_input = MouseInput::default();
        let mut keyboard_input = KeyboardInput::default();

        if mouse_get_input(g, INPUT_ANY, Some(&mut mouse_input)) {
            if left_control_visible
                && mouse_input.y >= y_controls
                && mouse_input.x >= x_left_control
                && mouse_input.x < x_left_control + w_control
            {
                if mouse_input.button == SDL_BUTTON_LEFT {
                    je_play_sample_num(g, S_CURSOR);

                    left_action = true;
                }
            } else if right_control_visible
                && mouse_input.y >= y_controls
                && mouse_input.x >= x_right_control
                && mouse_input.x < x_right_control + w_control
            {
                if mouse_input.button == SDL_BUTTON_LEFT {
                    je_play_sample_num(g, S_CURSOR);

                    right_action = true;
                }
            } else if mouse_input.x >= x_menu_item && mouse_input.x < x_menu_item + w_menu_item {
                // Find menu item that was hovered or clicked.
                for i in 0..menu_items_count {
                    let y_menu_item = y_menu_items + dy_menu_items * i as i32;
                    if mouse_input.y >= y_menu_item && mouse_input.y < y_menu_item + h_menu_item {
                        if selected_index != i {
                            je_play_sample_num(g, S_CURSOR);

                            selected_index = i;
                        }

                        if mouse_input.button == SDL_BUTTON_LEFT {
                            action = true;
                        }

                        break;
                    }
                }
            }

            if mouse_input.button == SDL_BUTTON_RIGHT {
                je_play_sample_num(g, S_SPRING);

                done = true;
            }
        } else if keyboard_get_input(g, Some(&mut keyboard_input)) {
            match keyboard_input.scancode {
                SDL_SCANCODE_LEFT => {
                    je_play_sample_num(g, S_CURSOR);

                    left_action = true;
                }
                SDL_SCANCODE_RIGHT => {
                    je_play_sample_num(g, S_CURSOR);

                    right_action = true;
                }
                SDL_SCANCODE_UP => {
                    je_play_sample_num(g, S_CURSOR);

                    selected_index = menu_prev(selected_index, menu_items_count);
                }
                SDL_SCANCODE_DOWN => {
                    je_play_sample_num(g, S_CURSOR);

                    selected_index = menu_next(selected_index, menu_items_count);
                }
                SDL_SCANCODE_SPACE | SDL_SCANCODE_RETURN => {
                    action = true;
                }
                SDL_SCANCODE_ESCAPE => {
                    je_play_sample_num(g, S_SPRING);

                    done = true;
                }
                _ => {}
            }
        }

        if left_action {
            players_index = if players_index == 0 { 1 } else { 0 };
        } else if right_action {
            players_index = if players_index == 1 { 0 } else { 1 };
        } else if action {
            if selected_index == menu_items_count - 1 {
                // "Exit to Main Menu"
                je_play_sample_num(g, S_SELECT);

                done = true;
            } else {
                let save_file_index = players_index * 11 + selected_index;

                if g.config.save_files[save_file_index].level == 0 {
                    // "EMPTY SLOT"
                    je_play_sample_num(g, S_CLINK);
                } else {
                    je_play_sample_num(g, S_SELECT);

                    g.mainint.perform_save = false;
                    je_operation(g, (save_file_index + 1) as u8)?;

                    fade_black(g, 15)?;

                    return Ok(g.varz.game_loaded);
                }
            }
        }

        if done {
            fade_black(g, 15)?;

            return Ok(false);
        }
    }
}

/// `JE_totalScore(&player[p])` (rule R8: the player by index).
#[doc(alias = "JE_totalScore")]
pub fn je_total_score(g: &mut Game, this_player: usize) -> u64 {
    let items = g.player.player[this_player].items;
    let mut temp = g.player.player[this_player].cash;

    let value = |g: &mut Game, t: u8, n: u8| je_get_value(g, t, u16::from(n)) as i64 as u64;
    temp = temp.wrapping_add(value(g, 2, items.ship));
    temp = temp.wrapping_add(value(g, 3, items.weapon[FRONT_WEAPON].id));
    temp = temp.wrapping_add(value(g, 4, items.weapon[REAR_WEAPON].id));
    temp = temp.wrapping_add(value(g, 5, items.shield));
    temp = temp.wrapping_add(value(g, 6, items.generator));
    temp = temp.wrapping_add(value(g, 7, items.sidekick[LEFT_SIDEKICK]));
    temp = temp.wrapping_add(value(g, 8, items.sidekick[RIGHT_SIDEKICK]));

    temp
}

#[doc(alias = "JE_getValue")]
pub fn je_get_value(g: &mut Game, item_type: u8, item_num: u16) -> i32 {
    let n = usize::from(item_num);
    let mut value: i64 = 0;

    match item_type {
        2 => value = i64::from(g.episodes.ships[n].cost),
        3 | 4 => {
            let base_value = i64::from(g.episodes.weapon_port[n].cost);

            // if two-player, use first player's front and second player's rear weapon
            let port = usize::from(item_type - 3);
            let p = if g.config.two_player_mode { port } else { 0 };
            let item_power = (i32::from(g.player.player[p].items.weapon[port].power) - 1) as u32;

            value = base_value;
            let mut i = 1u32;
            while i <= item_power {
                value = value.wrapping_add(weapon_upgrade_cost(base_value, i));
                i += 1;
            }
        }
        5 => value = i64::from(g.episodes.shields[n].cost),
        6 => value = i64::from(g.episodes.power_sys[n].cost),
        7 | 8 => value = i64::from(g.episodes.options[n].cost),
        _ => {}
    }

    value as i32
}

#[doc(alias = "JE_nextEpisode")]
pub fn je_next_episode(g: &mut Game) -> Flow {
    strcpy(&mut g.config.last_level_name, b"Completed\0");

    if g.episodes.episode_num == g.episodes.initial_episode_num
        && !g.config.game_has_repeated
        && usize::from(g.episodes.episode_num) != EPISODE_AVAILABLE
        && !g.network.is_network_game
        && !g.params.constant_play
    {
        je_high_score_check(g)?;
    }

    let new_episode = je_find_next_episode(g);

    if g.episodes.jump_back_to_episode1 {
        if g.episodes.episode_num > 2 && !g.params.constant_play {
            je_play_credits(g)?;
        }

        // randomly give player the SuperCarrot
        if mt_rand(g).is_multiple_of(6) {
            let p0 = &mut g.player.player[0];
            p0.items.ship = 2; // SuperCarrot
            p0.items.weapon[FRONT_WEAPON].id = 23; // Banana Blast
            p0.items.weapon[REAR_WEAPON].id = 24; // Banana Blast Rear

            for w in p0.items.weapon.iter_mut() {
                w.power = 1;
            }

            g.player.player[1].items.weapon[REAR_WEAPON].id = 24; // Banana Blast Rear

            g.player.player[0].last_items = g.player.player[0].items;
        }
    }

    if new_episode != u32::from(g.episodes.episode_num) {
        je_init_episode(g, new_episode as u8)?;
    }

    g.varz.game_loaded = true;
    g.config.main_level = FIRST_LEVEL;
    g.config.save_level = FIRST_LEVEL;

    play_song(g, 26)?;

    let screen = vga(g);
    je_clr256(g, screen);
    g.palette.colors = g.palette.palettes[6 - 1];

    let name = g.menus.episode_name[usize::from(g.episodes.episode_num)];
    let x = je_font_center(g, cstr(&name), SMALL_FONT_SHAPES as u32);
    je_d_string(g, screen, x, 130, cstr(&name), SMALL_FONT_SHAPES as u32);
    let text = misc(g, 5 - 1);
    let x = je_font_center(g, &text, SMALL_FONT_SHAPES as u32);
    je_d_string(g, screen, x, 185, &text, SMALL_FONT_SHAPES as u32);

    je_show_vga(g);
    let colors = g.palette.colors;
    fade_palette(g, &colors, 15, 0, 255)?;

    if !g.params.constant_play {
        wait_until_get_input(g)?;
    }

    fade_black(g, 15)
}

#[doc(alias = "JE_initPlayerData")]
pub fn je_init_player_data(g: &mut Game) {
    // JE: New Game Items/Data

    let p0 = &mut g.player.player[0];
    p0.items.ship = 1; // USP Talon
    p0.items.weapon[FRONT_WEAPON].id = 1; // Pulse Cannon
    p0.items.weapon[REAR_WEAPON].id = 0; // None
    p0.items.shield = 4; // Gencore High Energy Shield
    p0.items.generator = 2; // Advanced MR-12
    p0.items.sidekick = [0; 2]; // None
    p0.items.special = 0; // None

    p0.last_items = p0.items;

    let items = p0.items;
    let p1 = &mut g.player.player[1];
    p1.items = items;
    p1.items.weapon[REAR_WEAPON].id = 15; // Vulcan Cannon
    p1.items.sidekick_level = 101; // 101, 102, 103
    p1.items.sidekick_series = 0; // None

    g.config.game_has_repeated = false;
    g.config.one_player_action = false;
    g.config.super_arcade_mode = SA_NONE;
    g.config.super_tyrian = false;
    g.config.two_player_mode = false;

    g.config.secret_hint = ((mt_rand(g) % 3) + 1) as u8;

    for p in 0..2 {
        let ship = usize::from(g.player.player[p].items.ship);
        let dmg = g.episodes.ships[ship].dmg;
        let pl = &mut g.player.player[p];
        for w in pl.items.weapon.iter_mut() {
            w.power = 1;
        }

        pl.weapon_mode = 1;
        pl.armor = u32::from(dmg);

        pl.is_dragonwing = p == 1;
        pl.lives = Some(p);
    }

    g.config.main_level = FIRST_LEVEL;
    g.config.save_level = FIRST_LEVEL;

    let text = g.helptext.misc_text[19];
    strcpy(&mut g.config.last_level_name, &text);
}

#[doc(alias = "JE_sortHighScores")]
pub fn je_sort_high_scores(g: &mut Game) {
    g.varz.temp = 0;
    for _x in 0..6 {
        je_sort(g);
        g.varz.temp = g.varz.temp.wrapping_add(3);
    }
}

#[doc(alias = "JE_highScoreScreen")]
pub fn je_high_score_screen(g: &mut Game) -> Flow {
    need_shop_sprite_sheet(g)?; // need mouse pointer and arrow sprites

    let mut restart = true;

    let mut episode_index = 0usize;
    // Save file only has space for scores for 3 episodes.
    let episode_count = 3usize;

    let x_center = 320 / 2;
    let y_menu_header = 3;
    let y_episode_header = 30;
    let x_left_control = 83;
    let x_right_control = 213;
    let w_control = 24;
    let y_controls = 179;

    let rank_count = g.helptext.difficulty_name_b.len();

    loop {
        set_frame_count(g, 1);

        let screen = vga(g);
        let s2 = screen2(g);
        if restart {
            je_load_pic(g, s2, 2, false)?;
            fill_rectangle_wh(g, s2, 0, 192, 320, 8, 0);

            // Draw header.
            let header = misc(g, 50);
            draw_font_hv_shadow_aligned(
                g,
                s2,
                x_center,
                y_menu_header,
                &header,
                FONT_LARGE,
                ALIGN_CENTER,
                15,
                -3,
                false,
                2,
            );
        }

        // Restore background and header.
        copy_screen(g, screen, s2);

        let disabled = !g.episodes.episode_avail[episode_index];

        // Draw episode header.
        let name = g.menus.episode_name[episode_index + 1];
        draw_font_hv_shadow_aligned(
            g,
            screen,
            x_center,
            y_episode_header,
            cstr(&name),
            FONT_NORMAL,
            ALIGN_CENTER,
            15,
            -3 + if disabled { -4 } else { 0 },
            false,
            2,
        );

        // Draw 1-player and 2-player scores.
        for (heading, y_heading, y0, first) in [(46, 55, 75, 0), (47, 120, 135, 3)] {
            let text = misc(g, heading);
            draw_font_hv_shadow_aligned(
                g,
                screen,
                x_center,
                y_heading,
                &text,
                FONT_NORMAL,
                ALIGN_CENTER,
                15,
                -3,
                false,
                2,
            );

            for i in 0..3u8 {
                let y = y0 + 10 * i32::from(i);

                let save_file = g.config.save_files[episode_index * 6 + first + usize::from(i)];
                let rank = usize::from(save_file.high_score_diff).min(rank_count - 1);

                let buffer = snprintf(
                    29,
                    format!("~#{}:~  {}", i + 1, save_file.high_score1).into_bytes(),
                );
                je_text_shade(g, screen, 20, y, &buffer, 15, 0, FULL_SHADE);
                je_text_shade(
                    g,
                    screen,
                    110,
                    y,
                    cstr(&save_file.high_score_name),
                    15,
                    2,
                    FULL_SHADE,
                );
                let rank_name = g.helptext.difficulty_name_b[rank];
                let rank = rank as i32;
                je_text_shade(
                    g,
                    screen,
                    250,
                    y,
                    cstr(&rank_name),
                    15,
                    rank + if rank == 0 { 0 } else { -1 },
                    FULL_SHADE,
                );
            }
        }

        // Draw paging controls.

        let left_control_visible = episode_index > 0;
        let right_control_visible = episode_index < episode_count - 1;

        if left_control_visible {
            blit_sprite2x2(
                g,
                screen,
                x_left_control,
                y_controls,
                Sprite2Ref::ShopSpriteSheet,
                279,
            );
        }

        if right_control_visible {
            blit_sprite2x2(
                g,
                screen,
                x_right_control,
                y_controls,
                Sprite2Ref::ShopSpriteSheet,
                281,
            );
        }

        let help = misc(g, 56);
        je_help_box(g, screen, 103, 182, &help, 25, 7, 15, 1, FULL_SHADE as u8);

        if restart {
            g.mouse.mouse_cursor = MOUSE_POINTER_NORMAL;

            let colors = g.palette.colors;
            fade_palette(g, &colors, 10, 0, 255)?;

            restart = false;
        }

        loop {
            je_mouse_start(g);
            je_show_vga(g);
            je_mouse_replace(g);

            wait_until_elapsed(g)?;
            wait_until_has_input(g, INPUT_ANY)?;

            if has_input(g, INPUT_NO_MOTION) {
                break;
            }

            set_frame_count(g, 1);
        }

        // Handle interaction.

        let mut left_action = false;
        let mut right_action = false;
        let mut done = false;

        let mut mouse_input = MouseInput::default();
        let mut keyboard_input = KeyboardInput::default();

        if mouse_get_input(g, INPUT_NO_MOTION, Some(&mut mouse_input)) {
            match mouse_input.button {
                SDL_BUTTON_LEFT => {
                    if left_control_visible
                        && mouse_input.y >= y_controls
                        && mouse_input.x >= x_left_control
                        && mouse_input.x < x_left_control + w_control
                    {
                        je_play_sample_num(g, S_CURSOR);

                        left_action = true;
                    } else if right_control_visible
                        && mouse_input.y >= y_controls
                        && mouse_input.x >= x_right_control
                        && mouse_input.x < x_right_control + w_control
                    {
                        je_play_sample_num(g, S_CURSOR);

                        right_action = true;
                    }
                }
                SDL_BUTTON_RIGHT => {
                    je_play_sample_num(g, S_SPRING);

                    done = true;
                }
                _ => {}
            }
        } else if keyboard_get_input(g, Some(&mut keyboard_input)) {
            match keyboard_input.scancode {
                SDL_SCANCODE_LEFT => {
                    je_play_sample_num(g, S_CURSOR);

                    left_action = true;
                }
                SDL_SCANCODE_RIGHT => {
                    je_play_sample_num(g, S_CURSOR);

                    right_action = true;
                }
                SDL_SCANCODE_SPACE | SDL_SCANCODE_RETURN | SDL_SCANCODE_ESCAPE => {
                    je_play_sample_num(g, S_SPRING);

                    done = true;
                }
                _ => {}
            }
        }

        if left_action {
            episode_index = menu_prev(episode_index, episode_count);
        } else if right_action {
            episode_index = menu_next(episode_index, episode_count);
        }

        if done {
            fade_black(g, 15)?;

            return Ok(());
        }
    }
}

#[doc(alias = "JE_gammaCorrect_func")]
pub fn je_gamma_correct_func(col: &mut u8, r: f32) {
    let mut temp = (f32::from(*col) * r).round() as i32;
    if temp > 255 {
        temp = 255;
    }
    *col = temp as u8;
}

#[doc(alias = "JE_gammaCorrect")]
pub fn je_gamma_correct(color_buffer: &mut Palette, gamma: u8) {
    let r = 1.0 + f32::from(gamma) / 10.0;

    for c in color_buffer.iter_mut() {
        je_gamma_correct_func(&mut c.r, r);
        je_gamma_correct_func(&mut c.g, r);
        je_gamma_correct_func(&mut c.b, r);
    }
}

#[doc(alias = "JE_gammaCheck")]
pub fn je_gamma_check(g: &mut Game) -> bool {
    let temp = g.keyboard.keysactive[SDL_SCANCODE_F11];
    if temp {
        g.keyboard.keysactive[SDL_SCANCODE_F11] = false;
        g.config.gamma_correction = g.config.gamma_correction.wrapping_add(1) % 4;
        g.palette.colors = g.palette.palettes[usize::from(PCXPAL[3 - 1])];
        let gamma = g.config.gamma_correction;
        je_gamma_correct(&mut g.palette.colors, gamma);
        let colors = g.palette.colors;
        set_palette(g, &colors, 0, 255);
    }
    temp
}

#[doc(alias = "JE_doInGameSetup")]
pub fn je_do_in_game_setup(g: &mut Game) -> Flow {
    mouse_set_relative(g, false);

    g.network.halt_game = false;

    if g.network.is_network_game {
        network_prepare(g, PACKET_GAME_MENU);
        network_send(g, 4); // PACKET_GAME_MENU

        loop {
            set_frame_count(g, 1);

            if packet_in_type(g) == Some(PACKET_GAME_MENU) {
                network_update(g);
                break;
            }

            network_update(g);

            wait_until_elapsed(g)?;
        }
    }

    if g.network.your_in_game_menu_request {
        if je_in_game_setup(g)? {
            g.varz.really_end_level = true;
            g.varz.player_end_level = true;
        }
        g.varz.quit_requested = false;

        g.keyboard.keysactive[SDL_SCANCODE_ESCAPE_INDEX] = false;

        if g.network.is_network_game {
            if !g.varz.player_end_level {
                network_prepare(g, PACKET_WAITING);
                network_send(g, 4); // PACKET_WAITING
            } else {
                network_prepare(g, PACKET_GAME_QUIT);
                network_send(g, 4); // PACKET_GAMEQUIT
            }
        }
    }

    if g.network.is_network_game {
        let temp_surface = g.video.vga_screen;
        g.video.vga_screen = g.video.vga_screen_seg; // side-effect of game_screen

        if !g.network.your_in_game_menu_request {
            let screen = vga(g);
            je_bar_shade(g, screen, 3, 60, 257, 80); // Help Box
            je_bar_shade(g, screen, 5, 62, 255, 78);
            je_d_string(
                g,
                screen,
                10,
                65,
                b"Other player in options menu.",
                SMALL_FONT_SHAPES as u32,
            );
            je_show_vga(g);

            loop {
                set_frame_count(g, 1);

                match packet_in_type(g) {
                    Some(PACKET_WAITING) => {
                        network_check(g)?;
                        break;
                    }
                    Some(PACKET_GAME_QUIT) => {
                        g.varz.really_end_level = true;
                        g.varz.player_end_level = true;

                        network_check(g)?;
                        break;
                    }
                    _ => {}
                }

                network_update(g);

                wait_until_elapsed(g)?;
            }
        }

        while !network_is_sync(g) {
            set_frame_count(g, 1);

            wait_until_elapsed(g)?;
        }

        g.video.vga_screen = temp_surface; // side-effect of game_screen
    }

    g.network.your_in_game_menu_request = false;

    mouse_set_relative(g, true);
    Ok(())
}

/// `JE_inGameSetup` menu items.
const MENU_ITEM_MUSIC_VOLUME: usize = 0;
const MENU_ITEM_EFFECTS_VOLUME: usize = 1;
const MENU_ITEM_DETAIL_LEVEL: usize = 2;
const MENU_ITEM_GAME_SPEED: usize = 3;
const MENU_ITEM_RETURN_TO_GAME: usize = 4;
const MENU_ITEM_QUIT: usize = 5;

/// `JE_changeVolume(&tyrMusicVolume, music_delta, &fxVolume, sample_delta)`.
fn change_volume(g: &mut Game, music_delta: i32, sample_delta: i32) {
    let (mut music, mut sample) = (g.nortsong.tyr_music_volume, g.nortsong.fx_volume);
    je_change_volume(g, &mut music, music_delta, &mut sample, sample_delta);
    g.nortsong.tyr_music_volume = music;
    g.nortsong.fx_volume = sample;
}

#[doc(alias = "JE_inGameSetup")]
pub fn je_in_game_setup(g: &mut Game) -> Flow<bool> {
    let mut result = false;

    let temp_surface = g.video.vga_screen;
    g.video.vga_screen = g.video.vga_screen_seg; // side-effect of game_screen

    let help_indexes: [usize; 6] = [14, 14, 27, 28, 25, 26];

    need_shop_sprite_sheet(g)?; // need mouse pointer sprites

    let mut restart = true;

    let menu_items_count = g.helptext.in_game_text.len();
    let mut selected_index = MENU_ITEM_MUSIC_VOLUME;

    let y_menu_items = 20;
    let dy_menu_items = 20;
    let x_menu_item = 10;
    let x_menu_item_name = x_menu_item;
    let w_menu_item_name = 110;
    let x_menu_item_value = x_menu_item_name + w_menu_item_name;
    let w_menu_item_value = 90;
    let w_menu_item = w_menu_item_name + w_menu_item_value;
    let h_menu_item = 13;

    let mut done = false;
    while !done {
        set_frame_count(g, 1);

        let screen = vga(g);
        if restart {
            // Main box
            je_bar_shade(g, screen, 3, 13, 217, 137);
            je_bar_shade(g, screen, 5, 15, 215, 135);

            // Help box
            je_bar_shade(g, screen, 3, 143, 257, 157);
            je_bar_shade(g, screen, 5, 145, 255, 155);

            copy_screen(g, screen2(g), screen);

            g.mouse.mouse_cursor = MOUSE_POINTER_NORMAL;

            restart = false;
        }

        // Restore background.
        copy_screen(g, screen, screen2(g));

        // Draw menu items.
        for i in 0..menu_items_count {
            let y = y_menu_items + dy_menu_items * i as i32;

            let name = g.helptext.in_game_text[i];

            let selected = i == selected_index;
            let value = -4 + if selected { 2 } else { 0 };

            draw_font_hv_shadow(
                g,
                screen,
                x_menu_item_name,
                y,
                cstr(&name),
                FONT_NORMAL,
                15,
                value,
                false,
                2,
            );

            match i {
                MENU_ITEM_MUSIC_VOLUME => {
                    let col = if audio(g).music_disabled { 12 } else { 16 };
                    let amt = (g.nortsong.tyr_music_volume.wrapping_add(6)) / 12;
                    je_bar_draw_shadow(
                        g,
                        screen,
                        x_menu_item_value as u16,
                        y as u16,
                        1,
                        col,
                        amt,
                        3,
                        13,
                    );
                }
                MENU_ITEM_EFFECTS_VOLUME => {
                    let col = if audio(g).samples_disabled { 12 } else { 16 };
                    let amt = (g.nortsong.fx_volume.wrapping_add(6)) / 12;
                    je_bar_draw_shadow(
                        g,
                        screen,
                        x_menu_item_value as u16,
                        y as u16,
                        1,
                        col,
                        amt,
                        3,
                        13,
                    );
                }
                MENU_ITEM_DETAIL_LEVEL => {
                    let text = g.helptext.detail_level[usize::from(g.config.processor_type) - 1];
                    draw_font_hv_shadow(
                        g,
                        screen,
                        x_menu_item_value,
                        y,
                        cstr(&text),
                        FONT_NORMAL,
                        15,
                        value,
                        false,
                        2,
                    );
                }
                MENU_ITEM_GAME_SPEED => {
                    let text = g.helptext.game_speed_text[usize::from(g.config.game_speed) - 1];
                    draw_font_hv_shadow(
                        g,
                        screen,
                        x_menu_item_value,
                        y,
                        cstr(&text),
                        FONT_NORMAL,
                        15,
                        value,
                        false,
                        2,
                    );
                }
                _ => {}
            }
        }

        // Draw help text.
        let help = g.helptext.main_menu_help[help_indexes[selected_index]];
        je_out_text_adjust(
            g,
            screen,
            10,
            147,
            cstr(&help),
            14,
            6,
            TINY_FONT as u32,
            true,
        );

        je_mouse_start(g);
        je_show_vga(g);
        je_mouse_replace(g);

        wait_until_elapsed(g)?;
        wait_until_has_input(g, INPUT_ANY)?;

        // Handle interaction.

        let mut action = false;
        let mut left_action = false;
        let mut right_action = false;

        let mut mouse_input = MouseInput::default();
        let mut keyboard_input = KeyboardInput::default();

        if mouse_get_input(g, INPUT_ANY, Some(&mut mouse_input)) {
            // Find menu item that was hovered or clicked.
            if mouse_input.x >= x_menu_item && mouse_input.x < x_menu_item + w_menu_item {
                for i in 0..menu_items_count {
                    let y_menu_item = y_menu_items + dy_menu_items * i as i32;
                    if mouse_input.y >= y_menu_item && mouse_input.y < y_menu_item + h_menu_item {
                        if selected_index != i {
                            je_play_sample_num(g, S_CURSOR);

                            selected_index = i;
                        }

                        if mouse_input.button == SDL_BUTTON_LEFT {
                            // Act on menu item via name.
                            if mouse_input.x >= x_menu_item_name
                                && mouse_input.x < x_menu_item_name + w_menu_item_name
                            {
                                action = true;
                            }
                            // Act on menu item via value.
                            else if mouse_input.x >= x_menu_item_value
                                && mouse_input.x < x_menu_item_value + w_menu_item_value
                            {
                                let w = ((255 + 6) / 12) * (3 + 1) - 1;
                                let value =
                                    (mouse_input.x - x_menu_item_value).wrapping_mul(255) / (w - 1);
                                match i {
                                    MENU_ITEM_MUSIC_VOLUME => {
                                        je_play_sample_num(g, S_CURSOR);

                                        g.nortsong.tyr_music_volume = value.clamp(0, 255) as u16;

                                        let (m, s) =
                                            (g.nortsong.tyr_music_volume, g.nortsong.fx_volume);
                                        set_volume(g, m as u8, s as u8);
                                    }
                                    MENU_ITEM_EFFECTS_VOLUME => {
                                        g.nortsong.fx_volume = value.clamp(0, 255) as u16;

                                        let (m, s) =
                                            (g.nortsong.tyr_music_volume, g.nortsong.fx_volume);
                                        set_volume(g, m as u8, s as u8);

                                        je_play_sample_num(g, S_CURSOR);
                                    }
                                    MENU_ITEM_DETAIL_LEVEL | MENU_ITEM_GAME_SPEED => {
                                        right_action = true;
                                    }
                                    _ => {}
                                }
                            }
                        }

                        break;
                    }
                }
            }

            if mouse_input.button == SDL_BUTTON_RIGHT {
                je_play_sample_num(g, S_SPRING);

                done = true;
            }
        } else if keyboard_get_input(g, Some(&mut keyboard_input)) {
            match keyboard_input.scancode {
                SDL_SCANCODE_UP => {
                    je_play_sample_num(g, S_CURSOR);

                    selected_index = menu_prev(selected_index, menu_items_count);
                }
                SDL_SCANCODE_DOWN => {
                    je_play_sample_num(g, S_CURSOR);

                    selected_index = menu_next(selected_index, menu_items_count);
                }
                SDL_SCANCODE_LEFT => left_action = true,
                SDL_SCANCODE_RIGHT => right_action = true,
                SDL_SCANCODE_SPACE | SDL_SCANCODE_RETURN => action = true,
                SDL_SCANCODE_ESCAPE => {
                    je_play_sample_num(g, S_SPRING);

                    done = true;
                }
                SDL_SCANCODE_W if selected_index == MENU_ITEM_DETAIL_LEVEL => {
                    g.config.processor_type = 6;
                    je_init_processor_type(g);
                }
                _ => {}
            }
        }

        if action {
            match selected_index {
                MENU_ITEM_MUSIC_VOLUME => {
                    je_play_sample_num(g, S_SELECT);

                    let mut a = audio(g);
                    a.music_disabled = !a.music_disabled;
                }
                MENU_ITEM_EFFECTS_VOLUME => {
                    {
                        let mut a = audio(g);
                        a.samples_disabled = !a.samples_disabled;
                    }

                    je_play_sample_num(g, S_SELECT);
                }
                MENU_ITEM_RETURN_TO_GAME => {
                    je_play_sample_num(g, S_SELECT);

                    done = true;
                }
                MENU_ITEM_QUIT => {
                    je_play_sample_num(g, S_SELECT);

                    if g.params.constant_play {
                        je_tyrian_halt(g, 0)?;
                    }

                    if g.network.is_network_game {
                        // Tell other computer to exit
                        g.network.halt_game = true;
                        g.varz.player_end_level = true;
                    }

                    result = true;
                    done = true;
                }
                _ => {}
            }
        } else if left_action || right_action {
            let step: i32 = if left_action { -1 } else { 1 };
            match selected_index {
                MENU_ITEM_MUSIC_VOLUME => {
                    je_play_sample_num(g, S_CURSOR);

                    change_volume(g, 12 * step, 0);
                }
                MENU_ITEM_EFFECTS_VOLUME => {
                    change_volume(g, 0, 12 * step);

                    je_play_sample_num(g, S_CURSOR);
                }
                MENU_ITEM_DETAIL_LEVEL => {
                    je_play_sample_num(g, S_CURSOR);

                    let p = g.config.processor_type;
                    g.config.processor_type = if left_action {
                        if p > 1 { p - 1 } else { 4 }
                    } else if p < 4 {
                        p + 1
                    } else {
                        1
                    };
                    je_init_processor_type(g);
                    je_set_new_game_speed(g);
                }
                MENU_ITEM_GAME_SPEED => {
                    je_play_sample_num(g, S_CURSOR);

                    let s = g.config.game_speed;
                    g.config.game_speed = if left_action {
                        if s > 1 { s - 1 } else { 5 }
                    } else if s < 5 {
                        s + 1
                    } else {
                        1
                    };
                    je_init_processor_type(g);
                    je_set_new_game_speed(g);
                }
                _ => {}
            }
        }
    }

    g.video.vga_screen = temp_surface; // side-effect of game_screen

    Ok(result)
}

#[doc(alias = "JE_inGameHelp")]
pub fn je_in_game_help(g: &mut Game) -> Flow {
    mouse_set_relative(g, false);

    set_frame_count(g, 1);

    let temp_surface = g.video.vga_screen;
    g.video.vga_screen = g.video.vga_screen_seg; // side-effect of game_screen

    let screen = vga(g);
    let seg = seg(g);
    let opt = OPTION_SHAPES as u32;

    je_bar_shade(g, screen, 1, 1, 262, 182); // Main Box
    je_bar_shade(g, screen, 3, 3, 260, 180);
    je_bar_shade(g, screen, 5, 5, 258, 178);
    je_bar_shade(g, screen, 7, 7, 256, 176);
    fill_rectangle_xy(g, screen, 9, 9, 254, 174, 0);

    if g.config.two_player_mode {
        // Two-Player Help
        je_h_box(g, screen, 20, 4, 36, 50, 7, 3, 3);

        // weapon help
        blit_sprite(g, seg, 2, 21, opt, 43);
        je_h_box(g, screen, 55, 20, 37, 40, 7, 5, 3);

        // sidekick help
        blit_sprite(g, seg, 5, 36, opt, 41);
        je_h_box(g, screen, 40, 43, 34, 44, 7, 5, 3);

        // shield/armor help
        blit_sprite(g, seg, 2, 79, opt, 42);
        je_h_box(g, screen, 54, 84, 35, 40, 7, 5, 3);

        je_h_box(g, screen, 5, 126, 38, 55, 7, 5, 3);
        je_h_box(g, screen, 5, 160, 39, 55, 7, 5, 3);
    } else {
        // power bar help
        blit_sprite(g, seg, 15, 5, opt, 40);
        je_h_box(g, screen, 40, 10, 31, 45, 7, 5, 3);

        // weapon help
        blit_sprite(g, seg, 5, 37, opt, 39);
        je_h_box(g, screen, 40, 40, 32, 44, 7, 5, 3);
        je_h_box(g, screen, 40, 60, 33, 44, 7, 5, 3);

        // sidekick help
        blit_sprite(g, seg, 5, 98, opt, 41);
        je_h_box(g, screen, 40, 103, 34, 44, 7, 5, 3);

        // shield/armor help
        blit_sprite(g, seg, 2, 138, opt, 42);
        je_h_box(g, screen, 54, 143, 35, 40, 7, 5, 3);
    }

    // "press a key"
    blit_sprite(g, seg, 16, 189, opt, 36); // in-game text area
    let text = misc(g, 5 - 1);
    let x = 120 - je_text_width(g, &text, TINY_FONT as u32) / 2 + 20;
    je_out_text(g, seg, x, 190, &text, 0, 4);

    loop {
        je_mouse_start(g);
        je_show_vga(g);
        je_mouse_replace(g);

        wait_until_elapsed(g)?;
        wait_until_has_input(g, INPUT_ANY)?;

        if get_input(g) {
            break;
        }

        set_frame_count(g, 1);
    }

    g.mainint.text_erase = 1;

    g.video.vga_screen = temp_surface;

    mouse_set_relative(g, true);
    Ok(())
}

/// The blinking text cursor of the name-entry boxes (`JE_highScoreCheck`,
/// `JE_operation`): draws `text` and the cursor, then waits up to 14 frames
/// for input. Returns when there is non-motion input.
fn name_entry_cursor(
    g: &mut Game,
    flash: &mut u8,
    text: &[u8],
    mut fadein: Option<&mut bool>,
) -> Flow {
    loop {
        *flash = if *flash == 8 * 16 + 10 {
            8 * 16 + 2
        } else {
            8 * 16 + 10
        };
        g.varz.temp3 = if g.varz.temp3 == 6 { 2 } else { 6 };

        let screen = vga(g);
        je_out_text(g, screen, 65, 89, text, 8, 3);
        g.varz.temp_w = (65 + je_text_width(g, text, TINY_FONT as u32)) as u16;
        let tw = i32::from(g.varz.temp_w);
        je_bar_shade(g, screen, tw + 2, 90, tw + 6, 95);
        fill_rectangle_xy(g, screen, tw + 1, 89, tw + 5, 94, *flash);

        for _ in 0..14 {
            je_mouse_start(g);
            je_show_vga(g);
            if let Some(f) = fadein.as_deref_mut()
                && *f
            {
                let colors = g.palette.colors;
                fade_palette(g, &colors, 15, 0, 255)?;
                *f = false;
            }
            je_mouse_replace(g);

            wait_until_elapsed(g)?;

            if has_input(g, INPUT_NO_MOTION) {
                break;
            }

            set_frame_count(g, 1);
        }

        if has_input(g, INPUT_NO_MOTION) {
            return Ok(());
        }
    }
}

#[doc(alias = "JE_highScoreCheck")]
pub fn je_high_score_check(g: &mut Game) -> Flow {
    need_shop_sprite_sheet(g)?; // need mouse pointer sprite

    let mut temp_score: i32;

    let players = if g.config.two_player_mode { 2 } else { 1 };
    for temp_p in 0..players {
        je_sort_high_scores(g);

        let mut p = temp_p;

        if g.config.two_player_mode {
            // ask for the highest scorer first
            if g.player.player[0].cash < g.player.player[1].cash {
                p = if temp_p == 0 { 1 } else { 0 };
            }

            temp_score = g.player.player[p].cash as i32;
        } else {
            // single player highscore includes cost of upgrades
            temp_score = je_total_score(g, 0) as i32;
        }

        let first_slot = (i32::from(g.episodes.initial_episode_num) - 1) * 6
            + if g.config.two_player_mode { 3 } else { 0 };
        let slot_limit = first_slot + 3;

        let mut slot = first_slot;
        while slot < slot_limit {
            if temp_score > g.config.save_files[slot as usize].high_score1 {
                break;
            }
            slot += 1;
        }

        // did you get a high score?
        if slot < slot_limit {
            // shift down old scores
            let mut i = slot_limit - 1;
            while i > slot {
                let (iu, prev) = (i as usize, i as usize - 1);
                g.config.save_files[iu].high_score1 = g.config.save_files[prev].high_score1;
                let name = g.config.save_files[prev].high_score_name;
                strcpy(&mut g.config.save_files[iu].high_score_name, &name);
                i -= 1;
            }

            let screen = vga(g);
            je_clr256(g, screen);
            je_show_vga(g);
            g.palette.colors = g.palette.palettes[0];

            play_song(g, 33)?;

            // Enter Thy name

            let mut flash: u8 = 8 * 16 + 10;
            let mut fadein = true;
            let mut quit = false;
            let mut cancel = false;
            let mut stemp = [0u8; 30];

            strcpy(&mut stemp, b"                             \0");
            g.varz.temp = 0;

            je_bar_shade(g, screen, 65, 55, 255, 155);

            g.platform.start_text_input();

            loop {
                set_frame_count(g, 1);

                let screen = vga(g);
                let title = misc(g, 51);
                let x = je_font_center(g, &title, FONT_SHAPES as u32);
                je_d_string(g, screen, x, 3, &title, FONT_SHAPES as u32);

                g.varz.temp3 = if g.config.two_player_mode {
                    58 + p as u8
                } else {
                    53
                };

                let text = misc(g, usize::from(g.varz.temp3) - 1);
                let x = je_font_center(g, &text, SMALL_FONT_SHAPES as u32);
                je_d_string(g, screen, x, 30, &text, SMALL_FONT_SHAPES as u32);

                let seg = seg(g);
                blit_sprite(g, seg, 50, 50, OPTION_SHAPES as u32, 35); // message box

                if g.config.two_player_mode {
                    let buffer = cat(&[&misc(g, 48 + p), b" ", &misc(g, 53)]);
                    je_text_shade(g, screen, 60, 55, &buffer, 11, 4, FULL_SHADE);
                } else {
                    let text = misc(g, 53);
                    je_text_shade(g, screen, 60, 55, &text, 11, 4, FULL_SHADE);
                }

                let mut buffer = misc(g, 37);
                buffer.extend_from_slice(format!(" {temp_score}").as_bytes());
                je_text_shade(g, screen, 70, 70, &buffer, 11, 4, FULL_SHADE);

                // TODO: Rework this so that cursor blink timing is independent of input.
                let shown = stemp[..usize::from(g.varz.temp)].to_vec();
                name_entry_cursor(g, &mut flash, &shown, Some(&mut fadein))?;

                if !audio(g).lds.playing {
                    play_song(g, 31)?;
                }

                let mut mouse_input = MouseInput::default();
                let mut keyboard_input = KeyboardInput::default();

                if mouse_get_input(g, INPUT_NO_MOTION, Some(&mut mouse_input)) {
                    if mouse_input.x > 56
                        && mouse_input.x < 142
                        && mouse_input.y > 123
                        && mouse_input.y < 149
                    {
                        quit = true;
                    } else if mouse_input.x > 151
                        && mouse_input.x < 237
                        && mouse_input.y > 123
                        && mouse_input.y < 149
                    {
                        quit = true;
                        cancel = true;
                    }
                } else if keyboard_get_input(g, Some(&mut keyboard_input)) {
                    match keyboard_input.scancode {
                        SDL_SCANCODE_BACKSPACE | SDL_SCANCODE_DELETE => {
                            if g.varz.temp > 0 {
                                g.varz.temp -= 1;
                                stemp[usize::from(g.varz.temp)] = b' ';
                            }
                        }
                        SDL_SCANCODE_ESCAPE => {
                            quit = true;
                            cancel = true;
                        }
                        SDL_SCANCODE_RETURN => quit = true,
                        _ => {}
                    }

                    let ch = keyboard_input.ch;
                    if (ch == b' ' || FONT_MAP[usize::from(ch)] != -1) && g.varz.temp < 28 {
                        stemp[usize::from(g.varz.temp)] = ch;
                        g.varz.temp += 1;
                    }
                }
                if quit {
                    break;
                }
            }

            g.platform.stop_text_input();

            let slot = slot as usize;
            if !cancel {
                g.config.save_files[slot].high_score1 = temp_score;
                strcpy(&mut g.config.save_files[slot].high_score_name, &stemp);
                g.config.save_files[slot].high_score_diff = g.config.difficulty_level as u8;
            }

            fade_black(g, 15)?;
            let screen = vga(g);
            je_load_pic(g, screen, 2, false)?;

            let title = misc(g, 50);
            let x = je_font_center(g, &title, FONT_SHAPES as u32);
            je_d_string(g, screen, x, 10, &title, FONT_SHAPES as u32);
            let name = g.menus.episode_name[usize::from(g.episodes.episode_num)];
            let x = je_font_center(g, cstr(&name), SMALL_FONT_SHAPES as u32);
            je_d_string(g, screen, x, 35, cstr(&name), SMALL_FONT_SHAPES as u32);

            let (first, limit) = (first_slot as usize, slot_limit as usize);
            for i in first..limit {
                if i != slot {
                    let row = (i - first + 1) as i32;
                    let buffer =
                        format!("~#{}:~  {}", row, g.config.save_files[i].high_score1).into_bytes();
                    je_text_shade(g, screen, 20, row * 12 + 65, &buffer, 15, 0, FULL_SHADE);
                    let name = g.config.save_files[i].high_score_name;
                    je_text_shade(
                        g,
                        screen,
                        150,
                        row * 12 + 65,
                        cstr(&name),
                        15,
                        2,
                        FULL_SHADE,
                    );
                }
            }

            je_show_vga(g);

            let colors = g.palette.colors;
            fade_palette(g, &colors, 15, 0, 255)?;

            let row = (slot - first + 1) as i32;
            let buffer =
                format!("~#{}:~  {}", row, g.config.save_files[slot].high_score1).into_bytes();

            g.nortsong.frame_count_max = 6;
            g.fonthand.text_glow_font = TINY_FONT as u8;

            let seg = seg(g);
            g.fonthand.text_glow_brightness = 10;
            je_out_text_glow(g, seg, 20, row * 12 + 65, &buffer)?;
            g.fonthand.text_glow_brightness = 10;
            let name = g.config.save_files[slot].high_score_name;
            je_out_text_glow(g, seg, 150, row * 12 + 65, cstr(&name))?;
            g.fonthand.text_glow_brightness = 10;
            let text = misc(g, 4);
            let x = je_font_center(g, &text, TINY_FONT as u32);
            je_out_text_glow(g, seg, x, 180, &text)?;

            je_show_vga(g);

            if g.nortsong.frame_count_max != 0 {
                wait_until_get_input(g)?;
            }

            fade_black(g, 15)?;
        }
    }
    Ok(())
}

/// Increases game difficulty based on player's total score / total of
/// players' scores.
pub fn adjust_difficulty(g: &mut Game) {
    const SCORE_MULTIPLIER: [f32; 10] = [
        0.0, // Wimp  (doesn't exist)
        0.4, // Easy
        0.8, // Normal
        1.3, // Hard
        1.6, // Impossible
        2.0, // Insanity
        2.0, // Suicide
        3.0, // Maniacal
        3.0, // Zinglon
        3.0, // Nortaneous
    ];

    debug_assert!(g.config.initial_difficulty > 0 && g.config.initial_difficulty < 10);

    let score = if g.config.two_player_mode {
        g.player.player[0]
            .cash
            .wrapping_add(g.player.player[1].cash)
    } else {
        je_total_score(g, 0)
    };
    let adjusted_score =
        (score as f32 * SCORE_MULTIPLIER[g.config.initial_difficulty as usize]).round() as u64;

    let thresholds: [(u64, i8); 8] = if g.config.two_player_mode {
        [
            (10000, DIFFICULTY_EASY),
            (20000, DIFFICULTY_NORMAL),
            (50000, DIFFICULTY_HARD),
            (80000, DIFFICULTY_IMPOSSIBLE),
            (125000, DIFFICULTY_INSANITY),
            (200000, DIFFICULTY_SUICIDE),
            (400000, DIFFICULTY_MANIACAL),
            (600000, DIFFICULTY_ZINGLON),
        ]
    } else {
        [
            (40000, DIFFICULTY_EASY),
            (70000, DIFFICULTY_NORMAL),
            (150000, DIFFICULTY_HARD),
            (300000, DIFFICULTY_IMPOSSIBLE),
            (600000, DIFFICULTY_INSANITY),
            (1000000, DIFFICULTY_SUICIDE),
            (2000000, DIFFICULTY_MANIACAL),
            (3000000, DIFFICULTY_ZINGLON),
        ]
    };
    let new_difficulty = thresholds
        .iter()
        .find(|&&(limit, _)| adjusted_score < limit)
        .map_or(DIFFICULTY_NORTANEOUS, |&(_, d)| d) as u32;

    let current = g.config.difficulty_level as i32 as u32;
    g.config.difficulty_level = current.max(new_difficulty) as i8;
}

/// Street Fighter codes.
#[doc(alias = "JE_SFCodes")]
pub fn je_sf_codes(g: &mut Game, player_num: u8, px: i16, py: i16, mouse_x: i16, mouse_y: i16) {
    let pi = usize::from(player_num) - 1;
    let mut ship = u32::from(g.player.player[pi].items.ship);

    // Get direction
    if player_num == 2 && ship < 15 {
        ship = 0;
    }

    if ship < 15 {
        let mut temp2: u8 = u8::from(mouse_y > py) // UP
            + u8::from(mouse_y < py) // DOWN
            + u8::from(px < mouse_x) // LEFT
            + u8::from(px > mouse_x); // RIGHT
        let mut temp: u8 = u8::from(mouse_y > py) // UP
            + u8::from(mouse_y < py) * 2 // DOWN
            + u8::from(px < mouse_x) * 3 // LEFT
            + u8::from(px > mouse_x) * 4; // RIGHT

        if temp == 0 {
            // no direction being pressed
            if !g.mainint.button[0] {
                // if fire button is released
                temp = 9;
                temp2 = 1;
            } else {
                temp2 = 0;
                temp = 99;
            }
        }

        if temp2 == 1 {
            // if exactly one direction pressed or fire button is released
            temp = temp.wrapping_add(u8::from(g.mainint.button[0]) * 4);

            let temp3: u8 = if g.config.super_tyrian { 21 } else { 3 };
            for temp2 in 0..usize::from(temp3) {
                // Use SuperTyrian ShipCombos or not?
                // C reads one row past `shipCombos` for ship 14, which no
                // ship table entry has (D-UB-SFCODES-SHIP): no combo.
                let temp5 = if g.config.super_tyrian {
                    SHIP_COMBOS_B[temp2]
                } else {
                    SHIP_COMBOS.get(ship as usize).map_or(0, |row| row[temp2])
                };

                let code = &mut g.varz.sf_current_code[pi][temp2];
                // temp5 == selected combo in ship
                if temp5 == 0 {
                    // combo doesn't exists
                    // mark twiddles as cancelled/finished
                    *code = 0;
                } else {
                    let combo = &KEYBOARD_COMBOS[usize::from(temp5) - 1];
                    // get next combo key
                    let mut temp4 = combo[usize::from(*code)];

                    // correct key
                    if temp4 == temp {
                        *code += 1;

                        temp4 = combo[usize::from(*code)];
                        if temp4 > 100 && usize::from(temp4) <= 100 + crate::lvlmast::SPECIAL_NUM {
                            *code = 0;
                            g.varz.sf_executed[pi] = temp4 - 100;
                        }
                    } else if temp != 9
                        && (i32::from(temp4) - 1) % 4 != (i32::from(temp) - 1) % 4
                        && (*code == 0 || combo[usize::from(*code) - 1] != temp)
                    {
                        *code = 0;
                    }
                }
            }
        }
    }
}

#[doc(alias = "JE_sort")]
pub fn je_sort(g: &mut Game) {
    let base = usize::from(g.varz.temp);
    for a in 0..2 {
        for b in a + 1..3 {
            let (ia, ib) = (base + a, base + b);
            let f = &mut g.config.save_files;
            if f[ia].high_score1 < f[ib].high_score1 {
                let mut temp_str = [0u8; 30];

                let temp_li = f[ia].high_score1;
                f[ia].high_score1 = f[ib].high_score1;
                f[ib].high_score1 = temp_li;

                strcpy(&mut temp_str, &f[ia].high_score_name);
                let nb = f[ib].high_score_name;
                strcpy(&mut f[ia].high_score_name, &nb);
                strcpy(&mut f[ib].high_score_name, &temp_str);

                let temp_byte = f[ia].high_score_diff;
                f[ia].high_score_diff = f[ib].high_score_diff;
                f[ib].high_score_diff = temp_byte;
            }
        }
    }
}

/// Lines of `tyrian.cdt`.
const CREDITS_LINES: usize = 131;

#[doc(alias = "JE_playCredits")]
pub fn je_play_credits(g: &mut Game) -> Flow {
    let mut credstr = vec![[0u8; 65 + 1]; CREDITS_LINES];

    let mut currentpic: u8 = 0;
    let mut fade: u8 = 0;
    let mut fadechg: i8 = 1;
    let mut currentship: u8 = 0;
    let mut shipx: i16 = 0;
    let mut shipxwait: i16 = 0;
    let mut shipxc: i8 = 0;
    let mut shipxca: i8 = 0;

    load_sprites_file(g, EXTRA_SHAPES as u32, "estsc.shp")?;

    set_frame_count2(g, 1000);

    play_song(g, 8)?;

    let filename = "tyrian.cdt";

    let mut file = data_file_open(g, filename, "rb");
    if file.error {
        let message = format!(
            "Failed to open file '{filename}': {}",
            file_get_error(&file)
        );
        log_fatal(g, &message);
        return Err(Halt { code: EXIT_FAILURE });
    }

    // load credits text
    for line in credstr.iter_mut() {
        read_encrypted_string(&mut file, Some(line));
    }

    if file.error {
        let message = format!(
            "Failed to read from file '{filename}': {}",
            file_get_error(&file)
        );
        log_fatal(g, &message);
        return Err(Halt { code: EXIT_FAILURE });
    }

    file_close(&mut file);

    g.palette.colors = g.palette.palettes[6 - 1];
    let screen = vga(g);
    je_clr256(g, screen);
    je_show_vga(g);
    let colors = g.palette.colors;
    fade_palette(g, &colors, 2, 0, 255)?;

    let seg = seg(g);
    let extra = EXTRA_SHAPES as u32;
    let ticks_max = (CREDITS_LINES * 20 * 3) as u32;
    let mut ticks: u32 = 0;
    while ticks < ticks_max {
        set_frame_count(g, 1);

        let screen = vga(g);
        je_clr256(g, screen);

        let (w, h) = {
            let s = sprite(g, extra, u32::from(currentpic));
            (i32::from(s.width), i32::from(s.height))
        };
        blit_sprite_hv(
            g,
            seg,
            319 - w,
            100 - h / 2,
            extra,
            u32::from(currentpic),
            0x0,
            (i32::from(fade) - 15) as i8,
        );

        fade = (i32::from(fade) + i32::from(fadechg)) as u8;
        if fade == 0 && fadechg == -1 {
            fadechg = 1;
            currentpic = currentpic.wrapping_add(1);
            if usize::from(currentpic) >= g.sprite.sprite_table[EXTRA_SHAPES].count {
                currentpic = 0;
            }
        }
        if fade == 15 {
            fadechg = 0;
        }

        if get_frame_count2_ticks(g) == 0 {
            set_frame_count2(g, 900);

            fadechg = -1;
        }

        if ticks.is_multiple_of(200) {
            currentship = ((mt_rand(g) % 11) + 1) as u8;
            shipxwait = ((mt_rand(g) % 80) + 10) as i16;
            if (mt_rand(g) % 2) == 1 {
                shipx = 1;
                shipxc = 0;
                shipxca = 1;
            } else {
                shipx = 900;
                shipxc = 0;
                shipxca = -1;
            }
        }

        shipxwait = shipxwait.wrapping_sub(1);
        if shipxwait == 0 {
            if shipx == 1 || shipx == 900 {
                shipxc = 0;
            }
            shipxca = shipxca.wrapping_neg();
            shipxwait = ((mt_rand(g) % 40) + 15) as i16;
        }
        shipxc = shipxc.wrapping_add(shipxca);
        shipx = shipx.wrapping_add(i16::from(shipxc));
        if shipx < 1 {
            shipx = 1;
            shipxwait = 1;
        }
        if shipx > 900 {
            shipx = 900;
            shipxwait = 1;
        }
        let tmp_unknown = i32::from(shipxc) * i32::from(shipxc);
        if 450 + tmp_unknown < 0 || 450 + tmp_unknown > 900 {
            if shipxca < 0 && shipxc < 0 {
                shipxwait = 1;
            }
            if shipxca > 0 && shipxc > 0 {
                shipxwait = 1;
            }
        }

        let mut ship_sprite = u32::from(g.episodes.ships[usize::from(currentship)].shipgraphic);
        if shipxc < -10 {
            ship_sprite = ship_sprite.wrapping_sub(if shipxc < -20 { 4 } else { 2 });
        } else if shipxc > 10 {
            ship_sprite = ship_sprite.wrapping_add(if shipxc > 20 { 4 } else { 2 });
        }

        blit_sprite2x2(
            g,
            screen,
            i32::from(shipx) / 40,
            184 - (ticks % 200) as i32,
            Sprite2Ref::SpriteSheet9,
            ship_sprite,
        );

        let bottom_line = ((ticks / 3) / 20) as i32;
        let mut y = 20u32.wrapping_sub((ticks / 3) % 20) as i32;

        for line in bottom_line - 10..bottom_line {
            if line >= 0 && (line as usize) < CREDITS_LINES {
                let l = credstr[line as usize];
                if l[0] != b'.' && l[0] != 0 {
                    let color = l[0].wrapping_sub(65);
                    let text = cstr(&l[1..]);

                    let x = 110 - je_text_width(g, text, SMALL_FONT_SHAPES as u32) / 2;

                    je_out_text_adjust(
                        g,
                        screen,
                        x + ((y / 18) % 4 - 2).abs() - 1,
                        y - 1,
                        text,
                        u32::from(color),
                        -8,
                        SMALL_FONT_SHAPES as u32,
                        false,
                    );
                    je_out_text_adjust(
                        g,
                        screen,
                        x,
                        y,
                        text,
                        u32::from(color),
                        -2,
                        SMALL_FONT_SHAPES as u32,
                        false,
                    );
                }
            }

            y += 20;
        }

        fill_rectangle_xy(g, screen, 0, 0, 319, 10, 0);
        fill_rectangle_xy(g, screen, 0, 190, 319, 199, 0);

        if usize::from(currentpic) == g.sprite.sprite_table[EXTRA_SHAPES].count.wrapping_sub(1) {
            let text = misc(g, 54);
            je_out_text_adjust(
                g,
                screen,
                5,
                180,
                &text,
                2,
                -2,
                SMALL_FONT_SHAPES as u32,
                false,
            ); // levels-in-episode
        }

        if bottom_line == (CREDITS_LINES - 8) as i32 {
            fade_song(g);
        }

        if ticks == ticks_max - 1 {
            ticks -= 1;
            play_song(g, 9)?;
        }

        je_show_vga(g);

        if wait_until_get_input_or_elapsed(g)? {
            break;
        }
        ticks += 1;
    }

    fade_black(g, 10)?;

    free_sprites(g, extra);
    Ok(())
}

#[doc(alias = "JE_endLevelAni")]
pub fn je_end_level_ani(g: &mut Game) -> Flow {
    if !g.params.constant_play {
        // Grant ShipEdit privileges

        let avail_len = g.config.editor_item_avail.len();
        for p in 0..2 {
            let items = g.player.player[p].items;
            for w in items.weapon {
                let e = usize::MAX.wrapping_add(usize::from(w.id));
                if e < avail_len {
                    g.config.editor_item_avail[e] = 1;
                }
            }

            for s in items.sidekick {
                let e = 50 + usize::from(s);
                if e < avail_len {
                    g.config.editor_item_avail[e] = 1;
                }
            }
        }

        let e = 80 + usize::from(g.player.player[0].items.special);
        if e < avail_len {
            g.config.editor_item_avail[e] = 1;
        }
    }

    adjust_difficulty(g);

    g.player.player[0].last_items = g.player.player[0].items;
    let level_name = g.config.level_name;
    strcpy(&mut g.config.last_level_name, &level_name);

    g.nortsong.frame_count_max = 4;
    g.fonthand.text_glow_font = SMALL_FONT_SHAPES as u8;

    let white = Color {
        r: 255,
        g: 255,
        b: 255,
        a: 0,
    };
    set_colors(g, white, 254, 254);

    if !g.varz.level_timer || g.varz.level_timer_countdown > 0 || g.episodes.episode_num != 4 {
        je_play_sample_num(g, V_LEVEL_END);
    } else {
        play_song(g, 21)?;
    }

    let seg = seg(g);
    if g.episodes.bonus_level {
        let text = misc(g, 17 - 1);
        je_out_text_glow(g, seg, 20, 20, &text)?;
    } else if all_players_alive(g) {
        let temp_str = cat(&[&misc(g, 27 - 1), b" ", &level_name]); // "Completed"
        je_out_text_glow(g, seg, 20, 20, &temp_str)?;
    } else {
        let temp_str = cat(&[&misc(g, 62 - 1), b" ", &level_name]); // "Exiting"
        je_out_text_glow(g, seg, 20, 20, &temp_str)?;
    }

    if g.config.two_player_mode {
        for i in 0..2 {
            let mut temp_str = misc(g, 40 + i);
            temp_str.extend_from_slice(format!(" {}", g.player.player[i].cash).as_bytes());
            let temp_str = snprintf(256, temp_str);
            je_out_text_glow(g, seg, 30, 50 + 20 * i as i32, &temp_str)?;
        }
    } else {
        let mut temp_str = misc(g, 28 - 1);
        temp_str.extend_from_slice(format!(" {}", g.player.player[0].cash).as_bytes());
        je_out_text_glow(g, seg, 30, 50, &temp_str)?;
    }

    // C converts the rounded float to `JE_byte` through `int`.
    let temp: u8 = if g.varz.total_enemy == 0 {
        0
    } else {
        let pct = i32::from(g.varz.enemy_killed) * 100 / i32::from(g.varz.total_enemy);
        (pct as f32).round() as i32 as u8
    };
    let mut temp_str = misc(g, 63 - 1);
    temp_str.extend_from_slice(format!(" {temp}%").as_bytes());
    je_out_text_glow(g, seg, 40, 90, &temp_str)?;

    if !g.params.constant_play {
        g.config.editor_level = g.config.editor_level.wrapping_add(u16::from(temp / 5));
    }

    if !g.config.one_player_action && !g.config.two_player_mode {
        let text = misc(g, 4 - 1);
        je_out_text_glow(g, seg, 30, 120, &text)?; // Cubes

        if g.config.cube_max > 0 {
            let cube_list_len = g.config.cube_list.len() as u16;
            if g.config.cube_max > cube_list_len {
                g.config.cube_max = cube_list_len;
            }

            if g.nortsong.frame_count_max != 0 {
                g.nortsong.frame_count_max = 1;
            }

            let mut temp: u8 = 1;
            while u16::from(temp) <= g.config.cube_max {
                je_play_sample_num(g, S_ITEM);
                let x = 20 + 30 * u16::from(temp);
                let y = 135u16;
                je_draw_cube(g, seg, x, y, 9, 0);
                je_show_vga(g);

                for i in (-15i8..=10).chain((0..=10).rev()) {
                    let max = g.nortsong.frame_count_max;
                    set_frame_count(g, max);

                    blit_sprite_hv(
                        g,
                        seg,
                        i32::from(x),
                        i32::from(y),
                        OPTION_SHAPES as u32,
                        25,
                        0x9,
                        i,
                    );

                    je_show_vga(g);

                    if wait_until_get_input_or_elapsed(g)? {
                        g.nortsong.frame_count_max = 0;
                    }
                }
                temp += 1;
            }
        } else {
            let text = misc(g, 15 - 1);
            je_out_text_glow(g, seg, 50, 135, &text)?; // "None"
        }
    }

    let slow = if g.nortsong.frame_count_max != 0 {
        g.nortsong.frame_count_max = 6;
        true
    } else {
        false
    };
    g.varz.temp2 = if g.config.two_player_mode { 150 } else { 160 };
    let text = misc(g, 5 - 1);
    let y = i32::from(g.varz.temp2);
    je_out_text_glow(g, seg, 90, y, &text)?;

    if !g.params.constant_play && !(g.nortsong.frame_count_max == 0 && slow) {
        wait_until_get_input(g)?;
    }

    fade_black(g, 15)?;
    let screen = vga(g);
    je_clr256(g, screen);
    Ok(())
}

#[doc(alias = "JE_drawCube")]
pub fn je_draw_cube(g: &mut Game, screen: ScreenId, x: u16, y: u16, filter: u8, brightness: u8) {
    let (x, y) = (i32::from(x), i32::from(y));
    blit_sprite_dark(g, screen, x + 4, y + 4, OPTION_SHAPES as u32, 25, false);
    blit_sprite_dark(g, screen, x + 3, y + 3, OPTION_SHAPES as u32, 25, false);
    blit_sprite_hv(
        g,
        screen,
        x,
        y,
        OPTION_SHAPES as u32,
        25,
        filter,
        brightness as i8,
    );
}

/// `JE_handleChat`: empty upstream.
#[doc(alias = "JE_handleChat")]
pub fn je_handle_chat(_g: &mut Game) {}

/// `str_pop_int(str, &val)`: parses a leading decimal integer (`strtol`,
/// base 10) off the C string `str`, moving the rest to its start.
pub fn str_pop_int(s: &mut [u8], val: &mut i32) -> bool {
    let mut success = false;

    let len = cstr(s).len();
    debug_assert!(len < 256);

    // grab the value from str
    let at = |i: usize| if i < len { s[i] } else { 0 };
    let mut i = 0;
    while matches!(at(i), b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r') {
        i += 1;
    }
    let negative = at(i) == b'-';
    if matches!(at(i), b'-' | b'+') {
        i += 1;
    }
    let digits_start = i;
    let mut value: i64 = 0;
    let mut overflow = false;
    while at(i).is_ascii_digit() {
        let d = i64::from(at(i) - b'0');
        match value.checked_mul(10).and_then(|v| {
            if negative {
                v.checked_sub(d)
            } else {
                v.checked_add(d)
            }
        }) {
            Some(v) => value = v,
            None => overflow = true,
        }
        i += 1;
    }
    let end = if i == digits_start { 0 } else { i };
    if overflow {
        value = if negative { i64::MIN } else { i64::MAX };
    }
    *val = value as i32;

    if end != 0 {
        success = true;

        // shift the rest to the beginning
        let rest = s[end..=len].to_vec();
        s[..rest.len()].copy_from_slice(&rest);
    }

    success
}

#[doc(alias = "JE_operation")]
pub fn je_operation(g: &mut Game, slot: u8) -> Flow {
    if !g.mainint.perform_save {
        if g.config.save_files[usize::from(slot) - 1].level > 0 {
            g.config.game_just_loaded = true;
            je_load_game(g, slot)?;
            g.varz.game_loaded = true;
        }
    } else if !slot.is_multiple_of(11) {
        let mut stemp = [0u8; 21];
        strcpy(&mut stemp, b"              \0");
        let name = g.config.save_files[usize::from(slot) - 1].name;
        let name = cstr(&name);
        stemp[..name.len()].copy_from_slice(name);
        g.varz.temp = cstr(&stemp).len() as u8;
        while stemp[usize::from(g.varz.temp) - 1] == b' ' {
            g.varz.temp -= 1;
            if g.varz.temp == 0 {
                break;
            }
        }

        let mut flash: u8 = 8 * 16 + 10;

        let screen = vga(g);
        je_bar_shade(g, screen, 65, 55, 255, 155);

        g.platform.start_text_input();

        let mut quit = false;
        while !quit {
            set_frame_count(g, 1);

            let screen = vga(g);
            blit_sprite(g, screen, 50, 50, OPTION_SHAPES as u32, 35); // message box

            let text = misc(g, 0); // miscText[1-1]
            je_text_shade(g, screen, 60, 55, &text, 11, 4, DARKEN);
            let level_name = g.config.level_name;
            je_text_shade(g, screen, 70, 70, cstr(&level_name), 11, 4, DARKEN);

            for (i, x0) in [(9usize, 54), (10, 149)] {
                let text = misc(g, i);
                let text_x = x0 + 45 - (je_text_width(g, &text, FONT_SHAPES as u32) / 2);
                je_out_text_adjust(
                    g,
                    screen,
                    text_x,
                    128,
                    &text,
                    15,
                    -5,
                    FONT_SHAPES as u32,
                    true,
                );
            }

            // TODO: Rework this so that cursor blink timing is independent of input.
            let temp_str = cat(&[&misc(g, 2 - 1), &stemp[..usize::from(g.varz.temp)]]);
            name_entry_cursor(g, &mut flash, &temp_str, None)?;

            let mut mouse_input = MouseInput::default();
            let mut keyboard_input = KeyboardInput::default();

            if mouse_get_input(g, INPUT_NO_MOTION, Some(&mut mouse_input)) {
                if mouse_input.x > 56
                    && mouse_input.x < 142
                    && mouse_input.y > 123
                    && mouse_input.y < 149
                {
                    quit = true;
                    je_save_game(g, slot, cstr(&stemp));
                    je_play_sample_num(g, S_SELECT);
                } else if mouse_input.x > 151
                    && mouse_input.x < 237
                    && mouse_input.y > 123
                    && mouse_input.y < 149
                {
                    quit = true;
                    je_play_sample_num(g, S_SPRING);
                }
            } else if keyboard_get_input(g, Some(&mut keyboard_input)) {
                match keyboard_input.scancode {
                    SDL_SCANCODE_BACKSPACE | SDL_SCANCODE_DELETE => {
                        if g.varz.temp > 0 {
                            g.varz.temp -= 1;
                            stemp[usize::from(g.varz.temp)] = b' ';
                            je_play_sample_num(g, S_CLICK);
                        }
                    }
                    SDL_SCANCODE_ESCAPE => {
                        quit = true;
                        je_play_sample_num(g, S_SPRING);
                    }
                    SDL_SCANCODE_RETURN => {
                        quit = true;
                        je_save_game(g, slot, cstr(&stemp));
                        je_play_sample_num(g, S_SELECT);
                    }
                    _ => {}
                }

                let ch = keyboard_input.ch;
                if (ch == b' ' || FONT_MAP[usize::from(ch)] != -1) && g.varz.temp < 14 {
                    stemp[usize::from(g.varz.temp)] = ch;
                    g.varz.temp += 1;
                    je_play_sample_num(g, S_CURSOR);
                }
            }
        }

        g.platform.stop_text_input();
    }
    Ok(())
}

#[doc(alias = "JE_inGameDisplays")]
pub fn je_in_game_displays(g: &mut Game) {
    let screen = vga(g);
    let players = if g.config.two_player_mode && !g.config.galaga_mode {
        2
    } else {
        1
    };
    for i in 0..players {
        let tempstr = g.player.player[i].cash.to_string().into_bytes();
        je_text_shade(
            g,
            screen,
            30 + 200 * i as i32,
            175,
            &tempstr,
            2,
            4,
            FULL_SHADE,
        );
    }

    // Special Weapon?
    let special = g.player.player[0].items.special;
    if special > 0 {
        let graphic = u32::from(g.episodes.special[usize::from(special)].itemgraphic);
        blit_sprite2x2(g, screen, 25, 1, Sprite2Ref::SpriteSheet10, graphic);
    }

    // Lives Left
    if g.config.one_player_action || g.config.two_player_mode {
        let count = if g.config.one_player_action { 1 } else { 2 };
        for temp in 0..count {
            let extra_lives = (i32::from(g.player.player[temp].lives()) - 1) as u32;

            let y = if temp == 0 && g.player.player[0].items.special > 0 {
                35
            } else {
                15
            };
            g.varz.temp_w = if temp == 0 { 30 } else { 270 };

            if extra_lives >= 5 {
                let tw = i32::from(g.varz.temp_w);
                blit_sprite2(g, screen, tw, y, Sprite2Ref::SpriteSheet9, 285);
                g.varz.temp_w = if temp == 0 { 45 } else { 250 };
                let tempstr = (extra_lives as i32).to_string().into_bytes();
                let tw = i32::from(g.varz.temp_w);
                je_text_shade(g, screen, tw, y + 3, &tempstr, 15, 1, FULL_SHADE);
            } else if extra_lives >= 1 {
                for _ in 0..extra_lives {
                    let tw = i32::from(g.varz.temp_w);
                    blit_sprite2(g, screen, tw, y, Sprite2Ref::SpriteSheet9, 285);

                    g.varz.temp_w = if temp == 0 {
                        g.varz.temp_w.wrapping_add(12)
                    } else {
                        g.varz.temp_w.wrapping_sub(12)
                    };
                }
            }

            let mut stemp = if temp == 0 {
                misc(g, 49 - 1)
            } else {
                misc(g, 50 - 1)
            };
            if g.network.is_network_game {
                stemp = je_get_name(g, temp as u8 + 1);
            }

            g.varz.temp_w = if temp == 0 {
                28
            } else {
                (285 - je_text_width(g, &stemp, TINY_FONT as u32)) as u16
            };
            let tw = i32::from(g.varz.temp_w);
            je_text_shade(g, screen, tw, y - 7, &stemp, 2, 6, FULL_SHADE);
        }
    }

    // Super Bombs!!
    for i in 0..2 {
        let mut x = if i == 0 { 30 } else { 270 };

        for _ in 0..g.player.player[i].superbombs {
            blit_sprite2(g, screen, x, 160, Sprite2Ref::SpriteSheet9, 304);
            x += if i == 0 { 12 } else { -12 };
        }
    }

    if g.config.you_are_cheating {
        je_out_text(g, screen, 90, 170, b"Cheaters always prosper.", 3, 4);
    }
}

#[doc(alias = "JE_pauseGame")]
pub fn je_pause_game(g: &mut Game) -> Flow {
    mouse_set_relative(g, false);

    let mut done = false;

    let temp_surface = g.video.vga_screen;
    g.video.vga_screen = g.video.vga_screen_seg; // side-effect of game_screen

    if !g.config.super_pause {
        let seg = seg(g);
        let text = misc(g, 22);
        je_d_string(g, seg, 120, 90, &text, FONT_SHAPES as u32);

        g.video.vga_screen = g.video.vga_screen_seg;
        je_show_vga(g);
    }

    let (m, s) = (g.nortsong.tyr_music_volume, g.nortsong.fx_volume);
    set_volume(g, (m / 2) as u8, s as u8);

    if g.network.is_network_game {
        network_prepare(g, PACKET_GAME_PAUSE);
        network_send(g, 4); // PACKET_GAME_PAUSE

        loop {
            set_frame_count(g, 1);

            if packet_in_type(g) == Some(PACKET_GAME_PAUSE) {
                network_update(g);
                break;
            }

            network_update(g);

            wait_until_elapsed(g)?;
        }
    }

    while !done {
        set_frame_count(g, 1);

        wait_until_has_input_or_elapsed(g)?;

        let mut keyboard_input = KeyboardInput::default();

        if (keyboard_get_input(g, Some(&mut keyboard_input))
            && !matches!(
                keyboard_input.scancode,
                SDL_SCANCODE_LCTRL | SDL_SCANCODE_RCTRL | SDL_SCANCODE_LALT | SDL_SCANCODE_RALT
            ))
            || mouse_get_input(g, INPUT_NO_MOTION, None)
        {
            if g.network.is_network_game {
                network_prepare(g, PACKET_WAITING);
                network_send(g, 4); // PACKET_WAITING
            }

            g.keyboard.keysactive[SDL_SCANCODE_P_INDEX] = false;

            done = true;
        }

        if g.network.is_network_game {
            network_check(g)?;

            if packet_in_type(g) == Some(PACKET_WAITING) {
                network_check(g)?;

                done = true;
            }
        }
    }

    if g.network.is_network_game {
        while !network_is_sync(g) {
            set_frame_count(g, 1);

            wait_until_elapsed(g)?;
        }
    }

    let (m, s) = (g.nortsong.tyr_music_volume, g.nortsong.fx_volume);
    set_volume(g, m as u8, s as u8);

    g.video.vga_screen = temp_surface; // side-effect of game_screen

    mouse_set_relative(g, true);
    Ok(())
}

/// `JE_getName(pnum)`: the C string it returns.
#[doc(alias = "JE_getName")]
pub fn je_get_name(g: &Game, pnum: u8) -> Vec<u8> {
    let first = |name: &Option<Vec<u8>>| -> Option<Vec<u8>> {
        let name = name.as_deref().expect("network names are never NULL");
        (name.first().copied().unwrap_or(0) != 0).then(|| cstr(name).to_vec())
    };
    if u32::from(pnum) == g.network.this_player_num
        && let Some(name) = first(&g.network.network_player_name)
    {
        return name;
    } else if let Some(name) = first(&g.network.network_opponent_name) {
        return name;
    }

    misc(g, 47 + usize::from(pnum))
}
