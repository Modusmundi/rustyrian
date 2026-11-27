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
    DIFFICULTY_ZINGLON, KEY_SETTING_CHANGE_FIRE, KEY_SETTING_DOWN, KEY_SETTING_FIRE,
    KEY_SETTING_LEFT, KEY_SETTING_LEFT_SIDEKICK, KEY_SETTING_RIGHT, KEY_SETTING_RIGHT_SIDEKICK,
    KEY_SETTING_UP, SHOT_FRONT, SHOT_LEFT_SIDEKICK, SHOT_MISC, SHOT_NORTSPARKS, SHOT_P1_SUPERBOMB,
    SHOT_P2_CHARGE, SHOT_REAR, SHOT_RIGHT_SIDEKICK, SHOT_SPECIAL, SHOT_SPECIAL2,
    je_init_processor_type, je_load_game, je_save_game, je_set_new_game_speed,
};
use crate::demo::{play_demo_keys, record_demo_keys};
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
use crate::joystick::{joystick_analog_angle, joystick_axis_reduce, poll_joystick};
use crate::keyboard::{
    INPUT_ANY, INPUT_NO_MOTION, KeyboardInput, MouseInput, get_input, has_input,
    keyboard_get_input, mouse_get_input, mouse_get_relative_position, mouse_set_relative,
    wait_until_elapsed, wait_until_get_input, wait_until_get_input_or_elapsed,
    wait_until_has_input, wait_until_has_input_or_elapsed,
};
use crate::logging::log_fatal;
use crate::loudness::{audio, fade_song, play_song, set_volume};
use crate::mouse::{
    MOUSE_POINTER_LEFT, MOUSE_POINTER_NORMAL, MOUSE_POINTER_RIGHT, je_mouse_replace, je_mouse_start,
};
use crate::mtrand::mt_rand;
use crate::musmast::{MUSIC_NUM, SONG_MAPVIEW};
use crate::network::{
    PACKET_GAME_MENU, PACKET_GAME_PAUSE, PACKET_GAME_QUIT, PACKET_WAITING, PacketQueue,
    network_check, network_is_sync, network_prepare, network_send, network_state_is_reset,
    network_state_prepare, network_update,
};
use crate::nortsong::{
    get_frame_count2_ticks, je_change_volume, je_play_sample_num, set_frame_count, set_frame_count2,
};
use crate::nortvars::je_bar_draw_shadow;
use crate::palette::{Color, Palette, fade_black, fade_palette, set_colors, set_palette};
use crate::pcxmast::PCXPAL;
use crate::picload::je_load_pic;
use crate::player::{
    FRONT_WEAPON, LEFT_SIDEKICK, REAR_WEAPON, RIGHT_SIDEKICK, all_players_alive, all_players_dead,
    calc_purple_balls_needed, handle_got_purple_ball, power_up_weapon,
};
use crate::shots::{player_shot_create, player_shot_set_direction};
use crate::sndmast::{
    S_CLICK, S_CLINK, S_CURSOR, S_ENEMY_HIT, S_EXPLOSION_4, S_EXPLOSION_9, S_EXPLOSION_11, S_ITEM,
    S_POWERUP, S_SELECT, S_SPRING, S_WEAPON_26, V_DATA_CUBE, V_LEVEL_END,
};
use crate::sprite::{
    EXTRA_SHAPES, FONT_SHAPES, OPTION_SHAPES, SMALL_FONT_SHAPES, TINY_FONT, blit_sprite,
    blit_sprite_dark, blit_sprite_hv, blit_sprite2, blit_sprite2x2, blit_sprite2x2_blend,
    blit_sprite2x2_darken, free_sprites, je_load_comp_shapes, load_sprites_file, sprite,
};
use crate::varz::{
    CHARGE_GUN_WEAPONS, CURRENT_KEY_SPEED, HUD_SIDEKICK_Y, KEYBOARD_COMBOS, LINK_GUN_WEAPONS,
    OPTION_SELECT, SA_NONE, SA_NORTSHIPZ, SA_SPECIAL_WEAPON, SA_SPECIAL_WEAPON_B, SA_WEAPON,
    SHADOW_Y_DIST, SHIP_COMBOS, SHIP_COMBOS_B, SPECIAL_ARCADE_WEAPON, Sprite2Ref,
    je_do_special_shot, je_draw_armor, je_draw_options, je_draw_shield, je_get_ship_info,
    je_player_damage, je_port_configs, je_setup_explosion, je_setup_explosion_large,
    je_tyrian_halt, je_wipe_shield_armor_bars,
};
use crate::vga256d::{draw_segmented_gauge, fill_rectangle_wh, fill_rectangle_xy, je_bar_shade};
use crate::video::{ScreenId, je_clr256, je_show_vga};
use libm::{cosf, sinf};
use std::f64::consts::{FRAC_PI_2, PI};

const SDL_BUTTON_LMASK: u8 = 1 << 0;
const SDL_BUTTON_MMASK: u8 = 1 << 1;
const SDL_BUTTON_RMASK: u8 = 1 << 2;

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
const SDL_SCANCODE_C: usize = 6;
const SDL_SCANCODE_1: usize = 30;
const SDL_SCANCODE_2: usize = 31;
const SDL_SCANCODE_9: usize = 38;
const SDL_SCANCODE_0: usize = 39;
const SDL_SCANCODE_TAB: usize = 43;
const SDL_SCANCODE_MINUS: usize = 45;
const SDL_SCANCODE_BACKSLASH: usize = 49;
const SDL_SCANCODE_SLASH: usize = 56;
const SDL_SCANCODE_CAPSLOCK: usize = 57;
const SDL_SCANCODE_F1: usize = 58;
const SDL_SCANCODE_F2: usize = 59;
const SDL_SCANCODE_F3: usize = 60;
const SDL_SCANCODE_F4: usize = 61;
const SDL_SCANCODE_F5: usize = 62;
const SDL_SCANCODE_F6: usize = 63;
const SDL_SCANCODE_F7: usize = 64;
const SDL_SCANCODE_F8: usize = 65;
const SDL_SCANCODE_F9: usize = 66;
const SDL_SCANCODE_F10: usize = 67;
const SDL_SCANCODE_F12: usize = 69;
const SDL_SCANCODE_SCROLLLOCK: usize = 71;
const SDL_SCANCODE_NUMLOCKCLEAR: usize = 83;

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

/// `cost` of item `n` of `table`. Item numbers from saves and demos outside
/// the table read a zeroed entry (D-UB-MAININT-ITEMS; C reads past it).
fn cost_of<T>(table: &[T], n: usize, cost: impl Fn(&T) -> u16) -> i64 {
    table.get(n).map_or(0, |e| i64::from(cost(e)))
}

#[doc(alias = "JE_getCost")]
pub fn je_get_cost(g: &mut Game, item_type: u8, item_num: u16) -> u64 {
    let n = usize::from(item_num);
    let e = &g.episodes;
    let mut cost: i64 = 0;

    match item_type {
        2 => {
            cost = if item_num > 90 {
                100
            } else {
                cost_of(&e.ships[..], n, |s| s.cost)
            };
        }
        3 | 4 => {
            cost = cost_of(&e.weapon_port[..], n, |w| w.cost);

            let port = usize::from(item_type - 3);
            let item_power = (i32::from(g.player.player[0].items.weapon[port].power) - 1) as u32;

            g.mainint.downgrade_cost = weapon_upgrade_cost(cost, item_power) as u16;
            g.mainint.upgrade_cost = weapon_upgrade_cost(cost, item_power.wrapping_add(1)) as u16;
        }
        5 => cost = cost_of(&e.shields[..], n, |s| s.cost),
        6 => cost = cost_of(&e.power_sys[..], n, |p| p.cost),
        7 | 8 => cost = cost_of(&e.options[..], n, |o| o.cost),
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
    let e = &g.episodes;
    let mut value: i64 = 0;

    match item_type {
        2 => value = cost_of(&e.ships[..], n, |s| s.cost),
        3 | 4 => {
            let base_value = cost_of(&e.weapon_port[..], n, |w| w.cost);

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
        5 => value = cost_of(&e.shields[..], n, |s| s.cost),
        6 => value = cost_of(&e.power_sys[..], n, |p| p.cost),
        7 | 8 => value = cost_of(&e.options[..], n, |o| o.cost),
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

/// Copies custom ship `x` (scancode `1`..`0`) of `extraShips` into player
/// `p`'s items, for the `{ Edited Ships }` keys of `JE_mainKeyboardInput`.
fn edited_ship(g: &mut Game, p: usize, x: usize) {
    let ships = g.editship.extra_ships;
    let z = x - SDL_SCANCODE_1 + 1;
    let items = &mut g.player.player[p].items;
    items.ship = (90 + z) as u8; // Ships
    let z = (z - 1) * 15;
    items.weapon[FRONT_WEAPON].id = ships[z + 1];
    items.weapon[REAR_WEAPON].id = ships[z + 2];
    items.special = ships[z + 3];
    items.sidekick[LEFT_SIDEKICK] = ships[z + 4];
    items.sidekick[RIGHT_SIDEKICK] = ships[z + 5];
    items.generator = ships[z + 6];
    // Armor
    items.shield = ships[z + 8];
    g.config.shot_multi_pos = [0; 11];

    if g.player.player[p].weapon_mode > u32::from(je_port_configs(g)) {
        g.player.player[p].weapon_mode = 1;
    }

    g.varz.temp_w = g.player.player[p].armor as u16;
    je_get_ship_info(g);
    let edit_ship = if p == 0 {
        g.varz.edit_ship1
    } else {
        g.varz.edit_ship2
    };
    if g.player.player[p].armor > u32::from(g.varz.temp_w) && edit_ship {
        g.player.player[p].armor = u32::from(g.varz.temp_w);
    } else if p == 0 {
        g.varz.edit_ship1 = true;
    } else {
        g.varz.edit_ship2 = true;
    }

    let temp_surface = g.video.vga_screen;
    g.video.vga_screen = g.video.vga_screen_seg;
    je_wipe_shield_armor_bars(g);
    je_draw_armor(g);
    je_draw_shield(g);
    g.video.vga_screen = temp_surface;
    je_draw_options(g);

    g.keyboard.keysactive[x] = false;
}

#[doc(alias = "JE_mainKeyboardInput")]
pub fn je_main_keyboard_input(g: &mut Game) -> Flow {
    je_gamma_check(g);

    // { Network Request Commands }

    if !g.network.is_network_game {
        // { Edited Ships } for Player 1 (Tab), then for Player 2 (Caps Lock)
        for (p, key) in [(0, SDL_SCANCODE_TAB), (1, SDL_SCANCODE_CAPSLOCK)] {
            if g.editship.extra_avail
                && g.keyboard.keysactive[key]
                && !g.network.is_network_game
                && !g.config.super_tyrian
            {
                g.varz.x = SDL_SCANCODE_1 as u16;
                while usize::from(g.varz.x) <= SDL_SCANCODE_0 {
                    let x = usize::from(g.varz.x);
                    if g.keyboard.keysactive[x] {
                        edited_ship(g, p, x);
                    }
                    g.varz.x += 1;
                }
            }
        }
    }

    // { In-Game Help }
    if g.keyboard.keysactive[SDL_SCANCODE_F1] {
        if g.network.is_network_game {
            g.network.help_request = true;
        } else {
            je_in_game_help(g)?;
            g.varz.skip_star_show_vga = true;
        }
    }

    // {!Activate Nort Ship!}
    let k = &g.keyboard.keysactive;
    if k[SDL_SCANCODE_F2]
        && k[SDL_SCANCODE_F4]
        && k[SDL_SCANCODE_F6]
        && k[SDL_SCANCODE_F7]
        && k[SDL_SCANCODE_F9]
        && k[SDL_SCANCODE_BACKSLASH]
        && k[SDL_SCANCODE_SLASH]
    {
        if g.network.is_network_game {
            g.network.nort_ship_request = true;
        } else {
            let items = &mut g.player.player[0].items;
            items.ship = 12; // Nort Ship
            items.special = 13; // Astral Zone
            items.weapon[FRONT_WEAPON].id = 36; // NortShip Super Pulse
            items.weapon[REAR_WEAPON].id = 37; // NortShip Spreader
            g.varz.ship_gr = 1;
        }
    }

    // {Cheating}
    if !g.network.is_network_game
        && !g.config.two_player_mode
        && !g.config.super_tyrian
        && g.config.super_arcade_mode == SA_NONE
    {
        let k = &mut g.keyboard.keysactive;
        if k[SDL_SCANCODE_F2] && k[SDL_SCANCODE_F3] && k[SDL_SCANCODE_F6] {
            g.config.you_are_cheating = !g.config.you_are_cheating;
            k[SDL_SCANCODE_F2] = false;
        }

        if k[SDL_SCANCODE_F2] && k[SDL_SCANCODE_F3] && (k[SDL_SCANCODE_F4] || k[SDL_SCANCODE_F5]) {
            for p in g.player.player.iter_mut() {
                p.armor = 0;
            }

            g.config.you_are_cheating = !g.config.you_are_cheating;
            let text = misc(g, 63 - 1);
            je_draw_text_window(g, &text);
        }

        if g.params.constant_play && g.keyboard.keysactive[SDL_SCANCODE_C] {
            g.config.you_are_cheating = !g.config.you_are_cheating;
            g.keyboard.keysactive[SDL_SCANCODE_C] = false;
        }
    }

    if g.config.super_tyrian {
        g.config.you_are_cheating = false;
    }

    // {Personal Commands}

    // {DEBUG}
    let backspace = usize::from(SDL_SCANCODE_BACKSPACE);
    if g.keyboard.keysactive[SDL_SCANCODE_F10] && g.keyboard.keysactive[backspace] {
        g.keyboard.keysactive[SDL_SCANCODE_F10] = false;
        g.varz.debug = !g.varz.debug;

        g.varz.debug_hist = 0;
        g.varz.debug_hist_count = 0;

        // YKS: clock ticks since midnight replaced by SDL_GetTicks
        g.varz.last_debug_time = g.platform.get_ticks();
    }

    // {CHEAT-SKIP LEVEL}
    let k = &g.keyboard.keysactive;
    if k[SDL_SCANCODE_F2]
        && k[SDL_SCANCODE_F6]
        && (k[SDL_SCANCODE_F7] || k[SDL_SCANCODE_F8])
        && !k[SDL_SCANCODE_F9]
        && !g.config.super_tyrian
        && g.config.super_arcade_mode == SA_NONE
    {
        if g.network.is_network_game {
            g.network.skip_level_request = true;
        } else {
            g.varz.level_timer = true;
            g.varz.level_timer_countdown = 0;
            g.varz.end_level = true;
            g.varz.level_end = 40;
        }
    }

    // pause game
    g.mainint.pause_pressed |= g.keyboard.keysactive[SDL_SCANCODE_P_INDEX];

    // in-game setup
    g.mainint.ingamemenu_pressed |= g.keyboard.keysactive[SDL_SCANCODE_ESCAPE_INDEX];

    if g.keyboard.keysactive[backspace] {
        // toggle screenshot pause
        if g.keyboard.keysactive[SDL_SCANCODE_NUMLOCKCLEAR] {
            g.config.super_pause = !g.config.super_pause;
        }

        if g.keyboard.keysactive[SDL_SCANCODE_F12] && g.keyboard.keysactive[SDL_SCANCODE_SCROLLLOCK]
        {
            // {SMOOTHIES}
            g.varz.temp = SDL_SCANCODE_2 as u8;
            while usize::from(g.varz.temp) <= SDL_SCANCODE_9 {
                let temp = usize::from(g.varz.temp);
                if g.keyboard.keysactive[temp] {
                    let s = &mut g.config.smoothies[temp - SDL_SCANCODE_2];
                    *s = !*s;
                }
                g.varz.temp += 1;
            }
            if g.keyboard.keysactive[SDL_SCANCODE_0] {
                g.config.smoothies[8] = !g.config.smoothies[8];
            }
        } else if g.keyboard.keysactive[SDL_SCANCODE_MINUS] {
            // {CYCLE THROUGH FILTER COLORS}
            if g.config.level_filter == -99 {
                g.config.level_filter = 0;
            } else {
                g.config.level_filter = g.config.level_filter.wrapping_add(1);
                if g.config.level_filter == 16 {
                    g.config.level_filter = -99;
                }
            }
        } else if g.keyboard.keysactive[SDL_SCANCODE_1] {
            // {HYPER-SPEED}
            g.config.fast_play = g.config.fast_play.wrapping_add(1);
            if g.config.fast_play > 2 {
                g.config.fast_play = 0;
            }
            g.keyboard.keysactive[SDL_SCANCODE_1] = false;
            je_set_new_game_speed(g);
        }

        // {IN-GAME RANDOM MUSIC SELECTION}
        if g.keyboard.keysactive[SDL_SCANCODE_SCROLLLOCK] {
            let song = (mt_rand(g) % MUSIC_NUM as u64) as u32;
            play_song(g, song)?;
        }
    }
    Ok(())
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

/// `table[i]` for the item and weapon tables `JE_playerMovement` and
/// `JE_playerCollide` index with item numbers, pick-up values and animation
/// frames: an index outside the table reads a zeroed entry (divergence
/// D-UB-MAININT-TABLES).
fn at<T: Copy + Default>(table: &[T], i: usize) -> T {
    table.get(i).copied().unwrap_or_default()
}

/// C's conversion of a `float` to a `JE_word` argument. Undefined for values
/// outside `0..=65535` (D-UB-MAININT-FLOAT): clang converts through a signed
/// 32-bit integer and the callee keeps the low 16 bits.
fn float_to_word(v: f32) -> u16 {
    v as i32 as u16
}

/// `SDLNet_Read16(&packet->data[at])` of a state packet upstream reads
/// without a NULL check (the state queues are full while not reset).
fn state_read16(queue: &PacketQueue, i: usize, at: usize) -> u16 {
    let p = queue[i].as_ref().expect("state packet present");
    u16::from_be_bytes([p.data[at], p.data[at + 1]])
}

/// The data of `packet_state_out[0]`, allocated by `network_state_prepare`.
fn state_out_data(g: &mut Game) -> &mut [u8] {
    &mut g.network.packet_state_out[0]
        .as_mut()
        .expect("network_state_prepare allocates packet_state_out[0]")
        .data
}

/// `JE_playerMovement(&player[p], inputDevice, playerNum_, shipGr_,
/// shipGrPtr_, &player[p].mouseX, &player[p].mouseY)`: the only caller
/// passes the same player's mouse position, so `mouseX_`/`mouseY_` are
/// `player[p].mouse_x`/`mouse_y` (rule R8).
#[doc(alias = "JE_playerMovement")]
pub fn je_player_movement(
    g: &mut Game,
    p: usize,
    mut input_device: u8,
    player_num: u8,
    ship_gr: u16,
    ship_gr_ptr: Option<Sprite2Ref>,
) -> Flow {
    let ship_sheet = || ship_gr_ptr.expect("shipGrPtr is set by JE_getShipInfo");

    if player_num == 2 || !g.config.two_player_mode {
        let id = g.player.player[p].items.weapon[REAR_WEAPON].id;
        g.varz.temp_w = u16::from(at(&g.episodes.weapon_port, usize::from(id)).opnum);

        if g.player.player[p].weapon_mode > u32::from(g.varz.temp_w) {
            g.player.player[p].weapon_mode = 1;
        }
    }

    if g.network.is_network_game && g.network.this_player_num == u32::from(player_num) {
        network_state_prepare(g);
        state_out_data(g)[4..14].fill(0);
    }

    let mut mouse_xc: i16;
    let mut mouse_yc: i16;
    let mut accel_xc: i16;
    let mut accel_yc: i16;
    let mut link_gun_analog: bool;
    let mut link_gun_angle: f32;

    // redo:
    loop {
        if g.network.is_network_game {
            input_device = 0;
        }

        mouse_xc = 0;
        mouse_yc = 0;
        accel_xc = 0;
        accel_yc = 0;

        link_gun_analog = false;
        link_gun_angle = 0.0;

        // Draw Player
        if !g.player.player[p].is_alive {
            if g.player.player[p].exploding_ticks > 0 {
                g.player.player[p].exploding_ticks -= 1;

                if g.varz.level_end_fx_wait > 0 {
                    g.varz.level_end_fx_wait -= 1;
                } else {
                    g.varz.level_end_fx_wait = ((mt_rand(g) % 6) + 3) as u16;
                    if mt_rand(g) % 3 == 1 {
                        g.varz.sound_queue[6] = S_EXPLOSION_9;
                    } else {
                        g.varz.sound_queue[5] = S_EXPLOSION_11;
                    }
                }

                let (x, y) = (g.player.player[p].x, g.player.player[p].y);
                // `int + unsigned long` (mt_rand), stored back into an int.
                let explosion_x = (x as u64).wrapping_add(mt_rand(g) % 32).wrapping_sub(16) as i32;
                let explosion_y = (y as u64).wrapping_add(mt_rand(g) % 32).wrapping_sub(16) as i32;
                je_setup_explosion_large(
                    g,
                    false,
                    0,
                    explosion_x as i16,
                    explosion_y.wrapping_add(7) as i16,
                );
                je_setup_explosion_large(g, false, 0, x as i16, y.wrapping_add(7) as i16);

                if g.varz.level_end > 0 {
                    g.varz.level_end -= 1;
                }
            } else if g.config.two_player_mode || g.config.one_player_action {
                // if arcade mode
                let lives = g.player.player[p].lives();
                if lives > 1 {
                    // respawn if any extra lives
                    g.player.player[p].set_lives(lives - 1);

                    g.varz.really_end_level = false;
                    g.config.shot_multi_pos[usize::from(player_num) - 1] = 0;
                    calc_purple_balls_needed(g, p);
                    g.config.two_player_linked = false;
                    if g.config.galaga_mode {
                        g.config.two_player_mode = false;
                    }
                    let galaga = g.config.galaga_mode;
                    let episode4 = g.episodes.episode_num == 4;
                    let pl = &mut g.player.player[p];
                    pl.y = 160;
                    pl.invulnerable_ticks = 100;
                    pl.is_alive = true;
                    g.varz.end_level = false;

                    pl.armor = if galaga || episode4 {
                        pl.initial_armor
                    } else {
                        pl.initial_armor / 2
                    };

                    pl.shield = if galaga { 0 } else { pl.shield_max / 2 };

                    g.video.vga_screen = g.video.vga_screen_seg; // side-effect of game_screen
                    je_draw_armor(g);
                    je_draw_shield(g);
                    g.video.vga_screen = g.video.game_screen; // side-effect of game_screen
                    continue; // goto redo
                }

                if g.config.galaga_mode {
                    g.config.two_player_mode = false;
                }
                if g.varz.all_players_gone && g.network.is_network_game {
                    g.varz.really_end_level = true;
                }
            }
        } else if g.params.constant_die {
            // finished exploding?  start dying again
            if g.player.player[p].exploding_ticks == 0 {
                let pl = &mut g.player.player[p];
                pl.shield = 0;

                if pl.armor > 0 {
                    pl.armor -= 1;
                } else {
                    pl.is_alive = false;
                    pl.exploding_ticks = 60;
                    g.varz.level_end = 40;
                }

                je_wipe_shield_armor_bars(g);
                g.video.vga_screen = g.video.vga_screen_seg; // side-effect of game_screen
                je_draw_armor(g);
                g.video.vga_screen = g.video.game_screen; // side-effect of game_screen

                // as if instant death weren't enough, player also gets infinite lives in order to enjoy an infinite number of deaths -_-
                let lives = g.player.player[0].lives();
                if lives < 11 {
                    g.player.player[0].set_lives(lives + 1);
                }
            }
        }
        break;
    }

    if !g.player.player[p].is_alive {
        g.varz.explosion_follow_amount_x = 0;
        g.varz.explosion_follow_amount_y = 0;
        return Ok(());
    }

    if !g.varz.end_level {
        let pl = &mut g.player.player[p];
        pl.mouse_x = pl.x as u16;
        pl.mouse_y = pl.y as u16;
        g.mainint.button = [false; 4];

        // --- Movement Routine Beginning ---

        if !g.network.is_network_game || u32::from(player_num) == g.network.this_player_num {
            if g.varz.end_level {
                let pl = &mut g.player.player[p];
                pl.y = pl.y.wrapping_sub(2);
            } else {
                if g.demo.play_demo || g.demo.record_demo {
                    input_device = 1; // keyboard is required device for demo recording
                }

                // demo playback input
                if g.demo.play_demo && !play_demo_keys(g)? {
                    g.varz.end_level = true;
                    g.varz.level_end = 40;
                }

                // joystick input
                if (input_device == 0 || input_device >= 3) && g.joystick.joysticks > 0 {
                    let device = i32::from(input_device);
                    let j_first = if input_device == 0 { 0 } else { device - 3 };
                    let j_max = if input_device == 0 {
                        g.joystick.joysticks
                    } else {
                        device - 3 + 1
                    };
                    // A device number past the connected joysticks (from a
                    // save) polls nothing (D-UB-MAININT-JOYSTICK).
                    for j in j_first..j_max.min(g.joystick.joysticks) {
                        poll_joystick(g, j);

                        let js =
                            g.joystick.joystick.as_ref().expect("joysticks are open")[j as usize];
                        if js.analog {
                            let v = joystick_axis_reduce(g, j, js.x);
                            mouse_xc = i32::from(mouse_xc).wrapping_add(v) as i16;
                            let v = joystick_axis_reduce(g, j, js.y);
                            mouse_yc = i32::from(mouse_yc).wrapping_add(v) as i16;

                            link_gun_analog = joystick_analog_angle(g, j, &mut link_gun_angle);
                        } else {
                            let step = |on: bool, v: i32| if on { v } else { 0 };
                            let pl = &mut g.player.player[p];
                            pl.x = pl.x.wrapping_add(
                                step(js.direction[3], -CURRENT_KEY_SPEED)
                                    + step(js.direction[1], CURRENT_KEY_SPEED),
                            );
                            pl.y = pl.y.wrapping_add(
                                step(js.direction[0], -CURRENT_KEY_SPEED)
                                    + step(js.direction[2], CURRENT_KEY_SPEED),
                            );
                        }

                        let button = &mut g.mainint.button;
                        button[0] |= js.action[0];
                        button[1] |= js.action[2];
                        button[2] |= js.action[3];
                        button[3] |= js.action_pressed[1];

                        g.mainint.ingamemenu_pressed |= js.action_pressed[4];
                        g.mainint.pause_pressed |= js.action_pressed[5];
                    }
                }

                // mouse input
                if (input_device == 0 || input_device == 2) && g.mouse.has_mouse {
                    let down = g.keyboard.mouse_buttons_down;
                    let third = if g.mouse.mouse_has_three_buttons {
                        SDL_BUTTON_MMASK
                    } else {
                        SDL_BUTTON_RMASK
                    };
                    let button = &mut g.mainint.button;
                    button[0] |= (down & SDL_BUTTON_LMASK) != 0;
                    button[1] |= (down & SDL_BUTTON_RMASK) != 0;
                    button[2] |= (down & third) != 0;

                    let mut mouse_xr = 0i32;
                    let mut mouse_yr = 0i32;
                    mouse_get_relative_position(g, &mut mouse_xr, &mut mouse_yr);
                    mouse_xc = i32::from(mouse_xc).wrapping_add(mouse_xr) as i16;
                    mouse_yc = i32::from(mouse_yc).wrapping_add(mouse_yr) as i16;
                }

                // keyboard input
                if (input_device == 0 || input_device == 1) && !g.demo.play_demo {
                    let settings = g.config.key_settings;
                    let k = &g.keyboard.keysactive;
                    let key = |s: usize| k[settings[s] as usize];
                    let pl = &mut g.player.player[p];
                    if key(KEY_SETTING_UP) {
                        pl.y = pl.y.wrapping_sub(CURRENT_KEY_SPEED);
                    }
                    if key(KEY_SETTING_DOWN) {
                        pl.y = pl.y.wrapping_add(CURRENT_KEY_SPEED);
                    }

                    if key(KEY_SETTING_LEFT) {
                        pl.x = pl.x.wrapping_sub(CURRENT_KEY_SPEED);
                    }
                    if key(KEY_SETTING_RIGHT) {
                        pl.x = pl.x.wrapping_add(CURRENT_KEY_SPEED);
                    }

                    let button = &mut g.mainint.button;
                    button[0] |= key(KEY_SETTING_FIRE);
                    button[3] |= key(KEY_SETTING_CHANGE_FIRE);
                    button[1] |= key(KEY_SETTING_LEFT_SIDEKICK);
                    button[2] |= key(KEY_SETTING_RIGHT_SIDEKICK);

                    if g.params.constant_play {
                        *button = [true; 4];

                        pl.y = pl.y.wrapping_add(1);
                        pl.x = pl.x.wrapping_add(i32::from(g.mainint.constant_last_x));
                    }

                    if g.demo.record_demo {
                        record_demo_keys(g);
                    }
                }

                let pl = &mut g.player.player[p];
                if g.config.smoothies[9 - 1] {
                    pl.mouse_y = pl.y.wrapping_sub(i32::from(pl.mouse_y).wrapping_sub(pl.y)) as u16;
                    mouse_yc = mouse_yc.wrapping_neg();
                }

                accel_xc = i32::from(accel_xc)
                    .wrapping_add(pl.x.wrapping_sub(i32::from(pl.mouse_x)))
                    as i16;
                accel_yc = i32::from(accel_yc)
                    .wrapping_add(pl.y.wrapping_sub(i32::from(pl.mouse_y)))
                    as i16;

                mouse_xc = mouse_xc.clamp(-30, 30);
                mouse_yc = mouse_yc.clamp(-30, 30);

                let (mxc, myc) = (i32::from(mouse_xc), i32::from(mouse_yc));
                if mxc > 0 {
                    pl.x += (mxc + 3) / 4;
                } else if mxc < 0 {
                    pl.x += (mxc - 3) / 4;
                }
                if myc > 0 {
                    pl.y += (myc + 3) / 4;
                } else if myc < 0 {
                    pl.y += (myc - 3) / 4;
                }

                if mxc > 3 {
                    accel_xc = accel_xc.wrapping_add(1);
                } else if mxc < -2 {
                    accel_xc = accel_xc.wrapping_sub(1);
                }
                if myc > 2 {
                    accel_yc = accel_yc.wrapping_add(1);
                } else if myc < -2 {
                    accel_yc = accel_yc.wrapping_sub(1);
                }
            } // endLevel

            if g.network.is_network_game && u32::from(player_num) == g.network.this_player_num {
                let mut buttons: u16 = 0;
                for i in (0..4).rev() {
                    buttons <<= 1;
                    buttons |= u16::from(g.mainint.button[i]);
                }

                let pl = g.player.player[p];
                let data = state_out_data(g);
                let dx = pl.x.wrapping_sub(i32::from(pl.mouse_x)) as u16;
                let dy = pl.y.wrapping_sub(i32::from(pl.mouse_y)) as u16;
                data[4..6].copy_from_slice(&dx.to_be_bytes());
                data[6..8].copy_from_slice(&dy.to_be_bytes());
                data[8..10].copy_from_slice(&(accel_xc as u16).to_be_bytes());
                data[10..12].copy_from_slice(&(accel_yc as u16).to_be_bytes());
                data[12..14].copy_from_slice(&buttons.to_be_bytes());

                let pl = &mut g.player.player[p];
                pl.x = i32::from(pl.mouse_x);
                pl.y = i32::from(pl.mouse_y);

                g.mainint.button = [false; 4];

                accel_xc = 0;
                accel_yc = 0;
            }
        } // isNetworkGame

        // --- Movement Routine Ending ---

        g.network.move_ok = true;

        if g.network.is_network_game && !network_state_is_reset(g) {
            let (queue, i) = if u32::from(player_num) != g.network.this_player_num {
                if g.network.this_player_num == 2 {
                    g.config.difficulty_level =
                        state_read16(&g.network.packet_state_in, 0, 16) as i8;
                }
                (&g.network.packet_state_in, 0)
            } else {
                let delay = g.network.network_delay as usize;
                (&g.network.packet_state_out, delay)
            };
            let mut buttons = state_read16(queue, i, 12);
            let dx = state_read16(queue, i, 4) as i16;
            let dy = state_read16(queue, i, 6) as i16;
            let ax = state_read16(queue, i, 8) as i16;
            let ay = state_read16(queue, i, 10) as i16;
            for b in g.mainint.button.iter_mut() {
                *b = buttons & 1 != 0;
                buttons >>= 1;
            }

            let pl = &mut g.player.player[p];
            pl.x = pl.x.wrapping_add(i32::from(dx));
            pl.y = pl.y.wrapping_add(i32::from(dy));
            accel_xc = ax;
            accel_yc = ay;
        }

        // Street-Fighter codes
        let pl = g.player.player[p];
        je_sf_codes(
            g,
            player_num,
            pl.x as i16,
            pl.y as i16,
            pl.mouse_x as i16,
            pl.mouse_y as i16,
        );

        if g.network.move_ok {
            // END OF MOVEMENT ROUTINES

            // Linking Routines

            let [p0, p1] = g.player.player;
            let pl = g.player.player[p];
            let (mx, my) = (i32::from(pl.mouse_x), i32::from(pl.mouse_y));
            if g.config.two_player_mode
                && !g.config.two_player_linked
                && pl.x == mx
                && pl.y == my
                && p0.x.wrapping_sub(p1.x).wrapping_abs() < 8
                && p0.y.wrapping_sub(p1.y).wrapping_abs() < 8
                && p0.is_alive
                && p1.is_alive
                && !g.config.galaga_mode
            {
                g.config.two_player_linked = true;
            }

            let button = g.mainint.button;
            if player_num == 1 && (button[3 - 1] || button[2 - 1]) && !g.config.galaga_mode {
                g.config.two_player_linked = false;
            }

            if g.config.two_player_mode
                && g.config.two_player_linked
                && player_num == 2
                && (pl.x != mx || pl.y != my)
            {
                if button[0] {
                    let direc = &mut g.config.link_gun_direc;
                    if link_gun_analog {
                        *direc = link_gun_angle;
                    } else {
                        let (dx, dy) = (pl.x - mx, pl.y - my);
                        let temp_r: f32 = if dx.wrapping_abs() > dy.wrapping_abs() {
                            if dx > 0 {
                                FRAC_PI_2 as f32
                            } else {
                                (PI + FRAC_PI_2) as f32
                            }
                        } else if dy > 0 {
                            0.0
                        } else {
                            PI as f32
                        };

                        if (*direc - temp_r).abs() < 0.3 {
                            *direc = temp_r;
                        } else if *direc < temp_r && *direc - temp_r > -3.24 {
                            *direc += 0.2;
                        } else if f64::from(*direc - temp_r) < PI {
                            *direc -= 0.2;
                        } else {
                            *direc += 0.2;
                        }
                    }

                    if f64::from(*direc) >= 2.0 * PI {
                        *direc = (f64::from(*direc) - 2.0 * PI) as f32;
                    } else if *direc < 0.0 {
                        *direc = (f64::from(*direc) + 2.0 * PI) as f32;
                    }
                } else if !g.config.galaga_mode {
                    g.config.two_player_linked = false;
                }
            }
        }
    }

    if g.varz.level_end > 0 && all_players_dead(g) {
        g.varz.really_end_level = true;
    }

    // End Level Fade-Out
    if g.player.player[p].is_alive && g.varz.end_level {
        if g.varz.level_end == 0 {
            g.varz.really_end_level = true;
        } else {
            let pl = &mut g.player.player[p];
            pl.y = pl.y.wrapping_sub(i32::from(g.varz.level_end_warp));
            if pl.y < -200 {
                g.varz.really_end_level = true;
            }

            let (x, y) = (pl.x, pl.y);
            let mut trail_spacing = 1i32;
            let mut trail_y = y;
            let num_trails = (41 - i32::from(g.varz.level_end)).abs().min(20);

            for _ in 0..num_trails {
                trail_y += trail_spacing;
                trail_spacing += 1;
            }

            for _ in 1..num_trails {
                trail_y -= trail_spacing;
                trail_spacing -= 1;

                if trail_y > 0 && trail_y < 170 {
                    let screen = vga(g);
                    let sheet = ship_sheet();
                    if ship_gr == 0 {
                        blit_sprite2x2(g, screen, x - 17, trail_y - 7, sheet, 13);
                        blit_sprite2x2(g, screen, x + 7, trail_y - 7, sheet, 51);
                    } else if ship_gr == 1 {
                        blit_sprite2x2(g, screen, x - 17, trail_y - 7, sheet, 220);
                        blit_sprite2x2(g, screen, x + 7, trail_y - 7, sheet, 222);
                    } else {
                        blit_sprite2x2(g, screen, x - 5, trail_y - 7, sheet, u32::from(ship_gr));
                    }
                }
            }
        }
    }

    if g.demo.play_demo {
        let screen = vga(g);
        let text = misc(g, 7);
        je_d_string(g, screen, 115, 10, &text, SMALL_FONT_SHAPES as u32); // insert coin
    }

    if g.player.player[p].is_alive && !g.varz.end_level {
        if !g.config.two_player_linked || player_num < 2 {
            let pl = &mut g.player.player[p];
            let (mx, my) = (i32::from(pl.mouse_x), i32::from(pl.mouse_y));
            if !g.config.two_player_mode || g.varz.ship_gr2 != 0 {
                // if not dragonwing
                if pl.sidekick[LEFT_SIDEKICK].style == 0 {
                    pl.sidekick[LEFT_SIDEKICK].x = mx - 14;
                    pl.sidekick[LEFT_SIDEKICK].y = my;
                }

                if pl.sidekick[RIGHT_SIDEKICK].style == 0 {
                    pl.sidekick[RIGHT_SIDEKICK].x = mx + 16;
                    pl.sidekick[RIGHT_SIDEKICK].y = my;
                }
            }

            if pl.x_friction_ticks > 0 {
                pl.x_friction_ticks -= 1;
            } else {
                pl.x_friction_ticks = 1;

                if pl.x_velocity < 0 {
                    pl.x_velocity += 1;
                } else if pl.x_velocity > 0 {
                    pl.x_velocity -= 1;
                }
            }

            if pl.y_friction_ticks > 0 {
                pl.y_friction_ticks -= 1;
            } else {
                pl.y_friction_ticks = 2;

                if pl.y_velocity < 0 {
                    pl.y_velocity += 1;
                } else if pl.y_velocity > 0 {
                    pl.y_velocity -= 1;
                }
            }

            pl.x_velocity = pl.x_velocity.wrapping_add(i32::from(accel_xc));
            pl.y_velocity = pl.y_velocity.wrapping_add(i32::from(accel_yc));

            pl.x_velocity = pl.x_velocity.clamp(-4, 4);
            pl.y_velocity = pl.y_velocity.clamp(-4, 4);

            pl.x += pl.x_velocity;
            pl.y += pl.y_velocity;

            // if player moved, add new ship x, y history entry
            if pl.x - mx != 0 || pl.y - my != 0 {
                let n = pl.old_x.len();
                pl.old_x.copy_within(1.., 0);
                pl.old_y.copy_within(1.., 0);
                pl.old_x[n - 1] = pl.x;
                pl.old_y[n - 1] = pl.y;
            }
        } else {
            // twoPlayerLinked
            let p0 = g.player.player[0];
            let pl = &mut g.player.player[p];
            pl.x = if ship_gr == 0 { p0.x - 1 } else { p0.x };
            pl.y = p0.y + 8;

            pl.x_velocity = p0.x_velocity;
            pl.y_velocity = 4;

            let pl = *pl;
            let direc = g.config.link_gun_direc;
            // turret direction marker/shield
            for (d, wp) in [(direc + 0.2, 148), (direc - 0.2, 148), (direc, 147)] {
                g.config.shot_multi_pos[SHOT_MISC] = 0;
                g.varz.b = player_shot_create(
                    g,
                    0,
                    SHOT_MISC as u32,
                    float_to_word((pl.x + 1) as f32 + (sinf(d) * 26.0).round()),
                    float_to_word(pl.y as f32 + (cosf(d) * 26.0).round()),
                    pl.mouse_x,
                    pl.mouse_y,
                    wp,
                    player_num,
                );
            }

            if g.config.shot_repeat[SHOT_REAR] > 0 {
                g.config.shot_repeat[SHOT_REAR] -= 1;
            } else if g.mainint.button[0] {
                g.config.shot_multi_pos[SHOT_REAR] = 0;
                let id = pl.items.weapon[REAR_WEAPON].id;
                g.varz.b = player_shot_create(
                    g,
                    0,
                    SHOT_REAR as u32,
                    float_to_word((pl.x + 1) as f32 + (sinf(direc) * 20.0).round()),
                    float_to_word(pl.y as f32 + (cosf(direc) * 20.0).round()),
                    pl.mouse_x,
                    pl.mouse_y,
                    at(&LINK_GUN_WEAPONS, usize::from(id).wrapping_sub(1)),
                    player_num,
                );
                let b = g.varz.b;
                player_shot_set_direction(g, b, u32::from(id), direc);
            }
        }
    }

    if !g.varz.end_level {
        let network_player1 = g.network.is_network_game && player_num == 1;
        let pl = &mut g.player.player[p];
        if pl.x > 256 {
            pl.x = 256;
            g.mainint.constant_last_x = g.mainint.constant_last_x.wrapping_neg();
        }
        if pl.x < 40 {
            pl.x = 40;
            g.mainint.constant_last_x = g.mainint.constant_last_x.wrapping_neg();
        }

        if network_player1 {
            if pl.y > 154 {
                pl.y = 154;
            }
        } else if pl.y > 160 {
            pl.y = 160;
        }

        if pl.y < 10 {
            pl.y = 10;
        }

        // Determines the ship banking sprite to display, depending on horizontal velocity and acceleration
        let (mx, my) = (pl.mouse_x, pl.mouse_y);
        let mut ship_banking = pl.x_velocity / 2 + (pl.x - i32::from(mx)) / 6;
        ship_banking = ship_banking.clamp(-2, 2);

        let ship_sprite = ship_banking * 2 + i32::from(ship_gr);

        g.varz.explosion_follow_amount_x = (pl.x - pl.last_x_explosion_follow) as i16;
        g.varz.explosion_follow_amount_y = (pl.y - pl.last_y_explosion_follow) as i16;

        if g.varz.explosion_follow_amount_y < 0 {
            g.varz.explosion_follow_amount_y = 0;
        }

        pl.last_x_explosion_follow = pl.x;
        pl.last_y_explosion_follow = pl.y;

        let (x, y) = (pl.x, pl.y);
        let screen = vga(g);
        if g.config.background2 {
            let sx = x - i32::from(g.backgrnd.map_x2_ofs) + 30;
            let sy = (y.wrapping_sub(7) as u32).wrapping_add(SHADOW_Y_DIST) as i32;
            let sheet = ship_sheet();
            if ship_gr == 0 {
                blit_sprite2x2_darken(g, screen, sx - 17, sy, sheet, (ship_sprite + 13) as u32);
                blit_sprite2x2_darken(g, screen, sx + 7, sy, sheet, (ship_sprite + 51) as u32);
                if g.config.super_wild {
                    blit_sprite2x2_darken(g, screen, sx - 16, sy, sheet, (ship_sprite + 13) as u32);
                    blit_sprite2x2_darken(g, screen, sx + 6, sy, sheet, (ship_sprite + 51) as u32);
                }
            } else if ship_gr == 1 {
                blit_sprite2x2_darken(g, screen, sx - 17, sy, sheet, 220);
                blit_sprite2x2_darken(g, screen, sx + 7, sy, sheet, 222);
            } else {
                blit_sprite2x2_darken(g, screen, sx - 5, sy, sheet, ship_sprite as u32);
                if g.config.super_wild {
                    blit_sprite2x2_darken(g, screen, sx - 4, sy, sheet, ship_sprite as u32);
                }
            }
        }

        if g.player.player[p].invulnerable_ticks > 0 {
            g.player.player[p].invulnerable_ticks -= 1;

            let sheet = ship_sheet();
            if ship_gr == 0 {
                blit_sprite2x2_blend(g, screen, x - 17, y - 7, sheet, (ship_sprite + 13) as u32);
                blit_sprite2x2_blend(g, screen, x + 7, y - 7, sheet, (ship_sprite + 51) as u32);
            } else if ship_gr == 1 {
                blit_sprite2x2_blend(g, screen, x - 17, y - 7, sheet, 220);
                blit_sprite2x2_blend(g, screen, x + 7, y - 7, sheet, 222);
            } else {
                blit_sprite2x2_blend(g, screen, x - 5, y - 7, sheet, ship_sprite as u32);
            }
        } else {
            let sheet = ship_sheet();
            if ship_gr == 0 {
                blit_sprite2x2(g, screen, x - 17, y - 7, sheet, (ship_sprite + 13) as u32);
                blit_sprite2x2(g, screen, x + 7, y - 7, sheet, (ship_sprite + 51) as u32);
            } else if ship_gr == 1 {
                blit_sprite2x2(g, screen, x - 17, y - 7, sheet, 220);
                blit_sprite2x2(g, screen, x + 7, y - 7, sheet, 222);

                let mut ship_banking = 0i32;
                match ship_sprite {
                    5 => {
                        blit_sprite2(g, screen, x - 17, y + 7, sheet, 40);
                        g.varz.temp_w = (x - 7) as u16;
                        ship_banking = -2;
                    }
                    3 => {
                        blit_sprite2(g, screen, x - 17, y + 7, sheet, 39);
                        g.varz.temp_w = (x - 7) as u16;
                        ship_banking = -1;
                    }
                    -1 => {
                        blit_sprite2(g, screen, x + 19, y + 7, sheet, 58);
                        g.varz.temp_w = (x + 9) as u16;
                        ship_banking = 1;
                    }
                    -3 => {
                        blit_sprite2(g, screen, x + 19, y + 7, sheet, 59);
                        g.varz.temp_w = (x + 9) as u16;
                        ship_banking = 2;
                    }
                    // case 1: no banking
                    _ => {}
                }
                if ship_banking != 0 {
                    // NortSparks
                    if g.config.shot_repeat[SHOT_NORTSPARKS] > 0 {
                        g.config.shot_repeat[SHOT_NORTSPARKS] -= 1;
                    } else {
                        // clang evaluates the arguments left to right.
                        let px = u64::from(g.varz.temp_w)
                            .wrapping_add(mt_rand(g) % 8)
                            .wrapping_sub(4) as u16;
                        let py = (y as u64).wrapping_add(mt_rand(g) % 8).wrapping_sub(4) as u16;
                        g.varz.b = player_shot_create(
                            g,
                            0,
                            SHOT_NORTSPARKS as u32,
                            px,
                            py,
                            mx,
                            my,
                            671,
                            1,
                        );
                        g.config.shot_repeat[SHOT_NORTSPARKS] = (ship_banking.abs() - 1) as u8;
                    }
                }
            } else {
                blit_sprite2x2(g, screen, x - 5, y - 7, sheet, ship_sprite as u32);
            }
        }

        // Options Location
        if player_num == 2 && ship_gr == 0 {
            // if dragonwing
            let pl = &mut g.player.player[p];
            if pl.sidekick[LEFT_SIDEKICK].style == 0 {
                pl.sidekick[LEFT_SIDEKICK].x = pl.x - 14 + ship_banking * 2;
                pl.sidekick[LEFT_SIDEKICK].y = pl.y;
            }

            if pl.sidekick[RIGHT_SIDEKICK].style == 0 {
                pl.sidekick[RIGHT_SIDEKICK].x = pl.x + 17 + ship_banking * 2;
                pl.sidekick[RIGHT_SIDEKICK].y = pl.y;
            }
        }
    } // !endLevel

    if g.network.move_ok && g.player.player[p].is_alive && !g.varz.end_level {
        player_shots(g, p, player_num, ship_gr);
    }

    // draw sidekicks
    if (player_num == 2 || !g.config.two_player_mode) && !g.varz.end_level {
        for i in 0..2 {
            let option = at(
                &g.episodes.options,
                usize::from(g.player.player[p].items.sidekick[i]),
            );

            if option.option > 0 {
                let sk = &mut g.player.player[p].sidekick[i];
                if sk.animation_enabled {
                    sk.animation_frame = sk.animation_frame.wrapping_add(1);
                    if sk.animation_frame >= u32::from(option.ani) {
                        sk.animation_frame = 0;
                        sk.animation_enabled = option.option == 1;
                    }
                }

                let (x, y, style) = (sk.x, sk.y, sk.style);
                let sprite =
                    u32::from(at(&option.gr, sk.animation_frame as usize)).wrapping_add(sk.charge);

                let screen = vga(g);
                if style == 1 || style == 2 {
                    blit_sprite2x2(g, screen, x - 6, y, Sprite2Ref::SpriteSheet10, sprite);
                } else {
                    blit_sprite2(g, screen, x, y, Sprite2Ref::SpriteSheet9, sprite);
                }
            }

            let sk = &mut g.player.player[p].sidekick[i];
            sk.charge_ticks = sk.charge_ticks.wrapping_sub(1);
            if sk.charge_ticks == 0 {
                if sk.charge < u32::from(option.pwr) {
                    sk.charge += 1;
                }
                sk.charge_ticks = 20;
            }
        }
    }
    Ok(())
}

/// The `moveOk` part of `JE_playerMovement` for a living player before the
/// level ends: weapon mode change, special, main, charge and super bomb
/// shots, sidekick positions and sidekick shots.
fn player_shots(g: &mut Game, p: usize, player_num: u8, ship_gr: u16) {
    {
        let pl = &mut g.player.player[p];
        pl.delta_x_shot_move = pl.x - pl.last_x_shot_move;
        pl.delta_y_shot_move = pl.y - pl.last_y_shot_move;
    }

    // PLAYER SHOT Change
    if g.mainint.button[4 - 1] {
        g.config.port_config_change = true;
        if g.config.port_config_done {
            g.config.shot_multi_pos[SHOT_REAR] = 0;

            let sam = g.config.super_arcade_mode;
            if sam != SA_NONE && sam <= SA_NORTSHIPZ {
                g.config.shot_multi_pos[SHOT_SPECIAL] = 0;
                g.config.shot_multi_pos[SHOT_SPECIAL2] = 0;
                let i = usize::from(sam - 1);
                if u16::from(g.player.player[0].items.special) == SA_SPECIAL_WEAPON[i] {
                    g.player.player[0].items.special = SA_SPECIAL_WEAPON_B[i] as u8;
                    g.player.player[p].weapon_mode = 2;
                } else {
                    g.player.player[0].items.special = SA_SPECIAL_WEAPON[i] as u8;
                    g.player.player[p].weapon_mode = 1;
                }
            } else {
                let pl = &mut g.player.player[p];
                pl.weapon_mode = pl.weapon_mode.wrapping_add(1);
                if g.player.player[p].weapon_mode > u32::from(je_port_configs(g)) {
                    g.player.player[p].weapon_mode = 1;
                }
            }

            je_draw_port_config_buttons(g);
            g.config.port_config_done = false;
        }
    }

    // PLAYER SHOT Creation

    // SpecialShot
    if !g.config.galaga_mode {
        je_do_special_shot(g, player_num, p);
    }

    // Normal Main Weapons
    if !(g.config.two_player_linked && player_num == 2) {
        let (min, max) = if !g.config.two_player_mode {
            (1, 2)
        } else {
            (i32::from(player_num), i32::from(player_num))
        };

        g.varz.temp = (min - 1) as u8;
        while i32::from(g.varz.temp) < max {
            let temp = usize::from(g.varz.temp);
            let pl = g.player.player[p];
            let item = u32::from(pl.items.weapon[temp].id);

            if item > 0 {
                if g.config.shot_repeat[temp] > 0 {
                    g.config.shot_repeat[temp] -= 1;
                } else if g.mainint.button[0] {
                    let item_power = if g.config.galaga_mode {
                        0
                    } else {
                        u32::from(pl.items.weapon[temp].power).wrapping_sub(1)
                    };
                    let item_mode = if temp == REAR_WEAPON {
                        pl.weapon_mode.wrapping_sub(1)
                    } else {
                        0
                    };

                    let port = at(&g.episodes.weapon_port, item as usize);
                    let wp = at(&at(&port.op, item_mode as usize), item_power as usize);
                    g.varz.b = player_shot_create(
                        g,
                        item as u16,
                        temp as u32,
                        pl.x as u16,
                        pl.y as u16,
                        pl.mouse_x,
                        pl.mouse_y,
                        wp,
                        player_num,
                    );
                }
            }
            g.varz.temp += 1;
        }
    }

    // Super Charge Weapons
    if player_num == 2 {
        let pl = g.player.player[p];
        if !g.config.two_player_linked {
            let screen = vga(g);
            let index = 77 + u32::from(g.varz.charge_level) + u32::from(g.varz.charge_gr) * 19;
            blit_sprite2(
                g,
                screen,
                pl.x + i32::from(ship_gr == 0) + 1,
                pl.y - 13,
                Sprite2Ref::SpriteSheet10,
                index,
            );
        }

        if g.varz.charge_gr_wait > 0 {
            g.varz.charge_gr_wait -= 1;
        } else {
            g.varz.charge_gr = g.varz.charge_gr.wrapping_add(1);
            if g.varz.charge_gr == 4 {
                g.varz.charge_gr = 0;
            }
            g.varz.charge_gr_wait = 3;
        }

        let seg = seg(g);
        let charge_bar = |g: &mut Game, color: u8| {
            let y = 107 + (i32::from(g.varz.charge_level) - 1) * 3;
            fill_rectangle_xy(g, seg, 269, y, 275, y + 1, color);
        };

        if g.varz.charge_level > 0 {
            charge_bar(g, 193);
        }

        let rear_power = i32::from(pl.items.weapon[REAR_WEAPON].power);
        if g.varz.charge_wait > 0 {
            g.varz.charge_wait -= 1;
        } else {
            if g.varz.charge_level < g.varz.charge_max {
                g.varz.charge_level += 1;
            }

            g.varz.charge_wait = (28 - rear_power * 2) as u8;
            if g.config.difficulty_level > DIFFICULTY_HARD {
                g.varz.charge_wait = g.varz.charge_wait.wrapping_sub(5);
            }
        }

        if g.varz.charge_level > 0 {
            charge_bar(g, 204);
        }

        if g.config.shot_repeat[SHOT_P2_CHARGE] > 0 {
            g.config.shot_repeat[SHOT_P2_CHARGE] -= 1;
        } else if g.mainint.button[0] && (!g.config.two_player_linked || g.varz.charge_level > 0) {
            g.config.shot_multi_pos[SHOT_P2_CHARGE] = 0;
            let id = g.player.player[1].items.weapon[REAR_WEAPON].id;
            let wp = at(&CHARGE_GUN_WEAPONS, usize::from(id).wrapping_sub(1))
                .wrapping_add(u16::from(g.varz.charge_level));
            g.varz.b = player_shot_create(
                g,
                16,
                SHOT_P2_CHARGE as u32,
                pl.x as u16,
                pl.y as u16,
                pl.mouse_x,
                pl.mouse_y,
                wp,
                player_num,
            );

            if g.varz.charge_level > 0 {
                charge_bar(g, 193);
            }

            g.varz.charge_level = 0;
            g.varz.charge_wait = (30 - rear_power * 2) as u8;
        }
    }

    // SUPER BOMB
    g.varz.temp = player_num;
    if g.varz.temp == 0 {
        g.varz.temp = 1; // Get whether player 1 or 2
    }

    let t = usize::from(g.varz.temp) - 1;
    if g.player.player[t].superbombs > 0 {
        if g.config.shot_repeat[SHOT_P1_SUPERBOMB + t] > 0 {
            g.config.shot_repeat[SHOT_P1_SUPERBOMB + t] -= 1;
        } else if g.mainint.button[3 - 1] || g.mainint.button[2 - 1] {
            g.player.player[t].superbombs -= 1;
            g.config.shot_multi_pos[SHOT_P1_SUPERBOMB + t] = 0;
            let pl = g.player.player[p];
            g.varz.b = player_shot_create(
                g,
                16,
                (SHOT_P1_SUPERBOMB + t) as u32,
                pl.x as u16,
                pl.y as u16,
                pl.mouse_x,
                pl.mouse_y,
                535,
                player_num,
            );
        }
    }

    // sidekicks

    let styles = g.player.player[p].sidekick.map(|s| s.style);
    if styles[LEFT_SIDEKICK] == 4 && styles[RIGHT_SIDEKICK] == 4 {
        g.varz.option_satellite_rotate += 0.2;
    } else if styles[LEFT_SIDEKICK] == 4 || styles[RIGHT_SIDEKICK] == 4 {
        g.varz.option_satellite_rotate += 0.15;
    }
    let rotate = g.varz.option_satellite_rotate;

    let pl = &mut g.player.player[p];
    let half = pl.old_x.len() / 2 - 1;
    match styles[LEFT_SIDEKICK] {
        1 | 3 => {
            // trailing
            pl.sidekick[LEFT_SIDEKICK].x = pl.old_x[half];
            pl.sidekick[LEFT_SIDEKICK].y = pl.old_y[half];
        }
        2 => {
            // front-mounted
            pl.sidekick[LEFT_SIDEKICK].x = pl.x;
            pl.sidekick[LEFT_SIDEKICK].y = 10.max(pl.y - 20);
        }
        4 => {
            // orbiting
            pl.sidekick[LEFT_SIDEKICK].x = (pl.x as f32 + (sinf(rotate) * 20.0).round()) as i32;
            pl.sidekick[LEFT_SIDEKICK].y = (pl.y as f32 + (cosf(rotate) * 20.0).round()) as i32;
        }
        _ => {}
    }

    match styles[RIGHT_SIDEKICK] {
        4 => {
            // orbiting
            pl.sidekick[RIGHT_SIDEKICK].x = (pl.x as f32 - (sinf(rotate) * 20.0).round()) as i32;
            pl.sidekick[RIGHT_SIDEKICK].y = (pl.y as f32 - (cosf(rotate) * 20.0).round()) as i32;
        }
        1 | 3 => {
            // trailing
            pl.sidekick[RIGHT_SIDEKICK].x = pl.old_x[0];
            pl.sidekick[RIGHT_SIDEKICK].y = pl.old_y[0];
        }
        2 => front_mounted_sidekick(g, p),
        _ => {}
    }

    if player_num == 2 || !g.config.two_player_mode {
        // if player has sidekicks
        let hud_y = HUD_SIDEKICK_Y[usize::from(g.config.two_player_mode)];
        for (i, hud_y) in hud_y.into_iter().enumerate() {
            let shot_i = if i == 0 {
                SHOT_LEFT_SIDEKICK
            } else {
                SHOT_RIGHT_SIDEKICK
            };

            let option = at(
                &g.episodes.options,
                usize::from(g.player.player[p].items.sidekick[i]),
            );

            // fire/refill sidekick
            if option.wport == 0 {
                continue;
            }
            if g.config.shot_repeat[shot_i] > 0 {
                g.config.shot_repeat[shot_i] -= 1;
                continue;
            }

            let gauge_y = hud_y + 13;
            let ammo_max = g.player.player[p].sidekick[i].ammo_max;
            let segment_value = 1.max(ammo_max / 10) as u32;

            let fire = |g: &mut Game| {
                let pl = g.player.player[p];
                let sk = pl.sidekick[i];
                g.varz.b = player_shot_create(
                    g,
                    u16::from(option.wport),
                    shot_i as u32,
                    sk.x as u16,
                    sk.y as u16,
                    pl.mouse_x,
                    pl.mouse_y,
                    u32::from(option.wpnum).wrapping_add(sk.charge) as u16,
                    player_num,
                );
            };
            let discharge = |g: &mut Game| {
                let sk = &mut g.player.player[p].sidekick[i];
                if sk.charge > 0 {
                    g.config.shot_multi_pos[shot_i] = 0;
                    sk.charge = 0;
                }
                sk.charge_ticks = 20;
                sk.animation_enabled = true;
            };

            if ammo_max > 0 {
                // sidekick has limited ammo
                let sk = &mut g.player.player[p].sidekick[i];
                if sk.ammo_refill_ticks > 0 {
                    sk.ammo_refill_ticks -= 1;
                } else {
                    // refill one ammo
                    sk.ammo_refill_ticks = sk.ammo_refill_ticks_max;

                    if sk.ammo < ammo_max {
                        sk.ammo += 1;
                    }

                    // draw sidekick refill ammo gauge
                    let ammo = sk.ammo as u32;
                    let seg = seg(g);
                    draw_segmented_gauge(g, seg, 284, gauge_y, 112, 2, 2, segment_value, ammo);
                }

                if g.mainint.button[1 + i] && g.player.player[p].sidekick[i].ammo > 0 {
                    fire(g);

                    g.player.player[p].sidekick[i].ammo -= 1;
                    discharge(g);

                    // draw sidekick discharge ammo gauge
                    let ammo = g.player.player[p].sidekick[i].ammo as u32;
                    let seg = seg(g);
                    fill_rectangle_xy(g, seg, 284, gauge_y, 312, gauge_y + 2, 0);
                    draw_segmented_gauge(g, seg, 284, gauge_y, 112, 2, 2, segment_value, ammo);
                }
            } else if g.mainint.button[0] || g.mainint.button[1 + i] {
                // has infinite ammo
                fire(g);
                discharge(g);
            }
        }
    } // end of if player has sidekicks
}

/// The front-mounted (style 2) right sidekick of `JE_playerMovement`: it
/// flies off and returns on the right-sidekick button.
fn front_mounted_sidekick(g: &mut Game, p: usize) {
    let (px, py) = (g.player.player[p].x, g.player.player[p].y);
    if !g.varz.option_attachment_linked {
        let ret = i16::from(g.varz.option_attachment_return);
        let v = &mut g.varz;
        let sk = &mut g.player.player[p].sidekick[RIGHT_SIDEKICK];
        sk.y += i32::from(v.option_attachment_move / 2);
        if v.option_attachment_move >= -2 {
            v.temp = if v.option_attachment_return { 2 } else { 0 };

            let oam = &mut v.option_attachment_move;
            if sk.y > (py - 20) + 5 {
                v.temp = 2;
                *oam = oam.wrapping_sub(1 + ret);
            } else if sk.y > (py - 20) {
                v.temp = 3;
                if *oam > 0 {
                    *oam -= 1;
                } else {
                    *oam += 1;
                }
            } else if sk.y > (py - 20) - 5 {
                v.temp = 2;
                *oam += 1;
            } else if i32::from(*oam) < 2 + i32::from(ret) * 4 {
                *oam = oam.wrapping_add(1 + ret);
            }

            if v.option_attachment_return {
                v.temp = v.temp.wrapping_mul(2);
            }
            if (sk.x - px).abs() < i32::from(v.temp) {
                v.temp = 1;
            }

            if sk.x > px {
                sk.x -= i32::from(v.temp);
            } else if sk.x < px {
                sk.x += i32::from(v.temp);
            }

            if (sk.y - (py - 20)).abs() + (sk.x - px).abs() < 8 {
                v.option_attachment_linked = true;
                v.sound_queue[2] = S_CLINK;
            }

            if g.mainint.button[3 - 1] {
                v.option_attachment_return = true;
            }
        } else {
            // sidekick needs to catch up to player
            v.option_attachment_move = v.option_attachment_move.wrapping_add(1 + ret);
            let (x, y) = (sk.x + 1, sk.y + 10);
            je_setup_explosion(g, x as i16, y as i16, 0, 0, false, false);
        }
    } else {
        let sk = &mut g.player.player[p].sidekick[RIGHT_SIDEKICK];
        sk.x = px;
        sk.y = py - 20;
        if g.mainint.button[3 - 1] {
            g.varz.option_attachment_linked = false;
            g.varz.option_attachment_return = false;
            g.varz.option_attachment_move = -20;
            g.varz.sound_queue[3] = S_WEAPON_26;
        }
    }

    let sk = &mut g.player.player[p].sidekick[RIGHT_SIDEKICK];
    if sk.y < 10 {
        sk.y = 10;
    }
}

#[doc(alias = "JE_mainGamePlayerFunctions")]
pub fn je_main_game_player_functions(g: &mut Game) -> Flow {
    // PLAYER MOVEMENT/MOUSE ROUTINES

    if g.varz.end_level && g.varz.level_end > 0 {
        g.varz.level_end -= 1;
        g.varz.level_end_warp = g.varz.level_end_warp.wrapping_add(1);
    }

    // Reset Street-Fighter commands
    g.varz.sf_executed = [0; 2];

    g.config.port_config_change = false;

    if g.config.two_player_mode {
        let device = |g: &Game, i: usize| {
            if !g.config.galaga_mode {
                g.config.input_device[i]
            } else {
                0
            }
        };
        let d = device(g, 0);
        let (gr, ptr) = (g.varz.ship_gr, g.varz.ship_gr_ptr);
        je_player_movement(g, 0, d, 1, gr, ptr)?;
        let d = device(g, 1);
        let (gr, ptr) = (g.varz.ship_gr2, g.varz.ship_gr2ptr);
        je_player_movement(g, 1, d, 2, gr, ptr)?;
    } else {
        let (gr, ptr) = (g.varz.ship_gr, g.varz.ship_gr_ptr);
        je_player_movement(g, 0, 0, 1, gr, ptr)?;
    }

    // == Parallax Map Scrolling ==
    let temp_x: u16 = if g.config.two_player_mode {
        ((g.player.player[0].x + g.player.player[1].x) / 2) as u16
    } else {
        g.player.player[0].x as u16
    };

    let f = ((260 - (i32::from(temp_x) - 36)) as f32 / (260 - 36) as f32 * (24 * 3) as f32 - 1.0)
        .floor();
    g.varz.temp_w = float_to_word(f);
    let temp_w = g.varz.temp_w;
    let bg = &mut g.backgrnd;
    bg.map_x3_ofs = temp_w;
    bg.map_x3_pos = bg.map_x3_ofs % 24;
    bg.map_x3bp_pos = 1 - (bg.map_x3_ofs / 24) as isize;

    bg.map_x2_ofs = ((i32::from(temp_w) * 2) / 3) as u16;
    bg.map_x2_pos = bg.map_x2_ofs % 24;
    bg.map_x2bp_pos = 1 - (bg.map_x2_ofs / 24) as isize;

    bg.old_map_x_ofs = bg.map_x_ofs;
    bg.map_x_ofs = bg.map_x2_ofs / 2;
    bg.map_x_pos = bg.map_x_ofs % 24;
    bg.map_xbp_pos = 1 - (bg.map_x_ofs / 24) as isize;

    if g.varz.background3x1 {
        bg.map_x3_ofs = bg.map_x_ofs;
        bg.map_x3_pos = bg.map_x_pos;
        bg.map_x3bp_pos = bg.map_xbp_pos - 1;
    }
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

/// `miscTextB[i]` as a C string.
fn misc_b(g: &Game, i: usize) -> Vec<u8> {
    cstr(&g.helptext.misc_text_b[i]).to_vec()
}

/// The pick-up message `"%s %s %s"` of a network game (`JE_getName(pnum)`,
/// `miscTextB[4-1]`, `item`), bounded to `tempStr` as `snprintf` does.
fn net_pickup_text(g: &Game, pnum: u8, item: &[u8]) -> Vec<u8> {
    let name = je_get_name(g, pnum);
    let got = misc_b(g, 4 - 1);
    snprintf(256, cat(&[&name, b" ", &got, b" ", item]))
}

/// `JE_playerCollide(&player[p], playerNum_)`: pick-ups and collisions of
/// player `p` with the enemies.
#[doc(alias = "JE_playerCollide")]
pub fn je_player_collide(g: &mut Game, p: usize, player_num: u8) -> Flow {
    for z in 0..100 {
        if g.varz.enemy_avail[z] == 1 {
            continue;
        }
        let e = g.varz.enemy[z];
        let enemy_screen_x = i32::from(e.ex) + i32::from(e.mapoffset);
        let pl = g.player.player[p];

        if !((pl.x - enemy_screen_x).abs() < 12 && (pl.y - i32::from(e.ey)).abs() < 14) {
            continue;
        }
        // Collide
        let evalue = i32::from(e.evalue);
        if evalue > 29999 {
            if evalue == 30000 {
                // spawn dragonwing in galaga mode, otherwise just a purple ball
                let pl = &mut g.player.player[p];
                pl.cash = pl.cash.wrapping_add(100);

                if !g.config.galaga_mode {
                    handle_got_purple_ball(g, p);
                } else {
                    // spawn the dragonwing?
                    if g.config.two_player_mode {
                        let pl = &mut g.player.player[p];
                        pl.cash = pl.cash.wrapping_add(2400);
                    }
                    g.config.two_player_mode = true;
                    g.config.two_player_linked = true;
                    let p1 = &mut g.player.player[1];
                    p1.items.weapon[REAR_WEAPON].power = 1;
                    p1.armor = 10;
                    p1.is_alive = true;
                }
                g.varz.enemy_avail[z] = 1;
                g.varz.sound_queue[7] = S_POWERUP;
            } else if g.config.super_arcade_mode != SA_NONE && evalue > 30000 {
                g.config.shot_multi_pos[SHOT_FRONT] = 0;
                g.config.shot_repeat[SHOT_FRONT] = 10;

                let row = at(
                    &SA_WEAPON,
                    usize::from(g.config.super_arcade_mode).wrapping_sub(1),
                );
                g.varz.temp_w = at(&row, (evalue - 30000 - 1) as usize);

                // if picked up already-owned weapon, power weapon up
                if g.varz.temp_w == u16::from(g.player.player[0].items.weapon[FRONT_WEAPON].id) {
                    let pl = &mut g.player.player[p];
                    pl.cash = pl.cash.wrapping_add(1000);
                    power_up_weapon(g, p, FRONT_WEAPON as u32);
                }
                // else weapon also gives purple ball
                else {
                    handle_got_purple_ball(g, p);
                }

                g.player.player[0].items.weapon[FRONT_WEAPON].id = g.varz.temp_w as u8;
                let pl = &mut g.player.player[p];
                pl.cash = pl.cash.wrapping_add(200);
                g.varz.sound_queue[7] = S_POWERUP;
                g.varz.enemy_avail[z] = 1;
            } else if evalue > 32100 {
                if player_num == 1 {
                    let pl = &mut g.player.player[p];
                    pl.cash = pl.cash.wrapping_add(250);
                    g.player.player[0].items.special = (evalue - 32100) as u8;
                    let c = &mut g.config;
                    c.shot_multi_pos[SHOT_SPECIAL] = 0;
                    c.shot_repeat[SHOT_SPECIAL] = 10;
                    c.shot_multi_pos[SHOT_SPECIAL2] = 0;
                    c.shot_repeat[SHOT_SPECIAL2] = 0;

                    let name = at(&g.episodes.special, (evalue - 32100) as usize).name;
                    let text = if g.network.is_network_game {
                        net_pickup_text(g, 1, &name)
                    } else if g.config.two_player_mode {
                        cat(&[&misc(g, 43 - 1), b" ", &name])
                    } else {
                        cat(&[&misc(g, 64 - 1), b" ", &name])
                    };
                    je_draw_text_window(g, &text);
                    g.varz.sound_queue[7] = S_POWERUP;
                    g.varz.enemy_avail[z] = 1;
                }
            } else if evalue > 32000 {
                let name = at(&g.episodes.options, (evalue - 32000) as usize).name;
                if player_num == 2 {
                    g.varz.enemy_avail[z] = 1;
                    let text = if g.network.is_network_game {
                        net_pickup_text(g, 2, &name)
                    } else {
                        cat(&[&misc(g, 44 - 1), b" ", &name])
                    };
                    je_draw_text_window(g, &text);

                    // if picked up a different sidekick than player already has, then reset sidekicks to least powerful, else power them up
                    let items = &mut g.player.player[1].items;
                    if (evalue as u32).wrapping_sub(32000) != u32::from(items.sidekick_series) {
                        items.sidekick_series = (evalue - 32000) as u8;
                        items.sidekick_level = 101;
                    } else if items.sidekick_level < 103 {
                        items.sidekick_level += 1;
                    }

                    let temp = u32::from(items.sidekick_level).wrapping_sub(100 + 1);
                    let select = at(
                        &at(&OPTION_SELECT, usize::from(items.sidekick_series)),
                        temp as usize,
                    );
                    items.sidekick = select;

                    g.config.shot_multi_pos[SHOT_LEFT_SIDEKICK] = 0;
                    g.config.shot_multi_pos[SHOT_RIGHT_SIDEKICK] = 0;
                    je_draw_options(g);
                    g.varz.sound_queue[7] = S_POWERUP;
                } else if g.config.one_player_action {
                    g.varz.enemy_avail[z] = 1;
                    let text = cat(&[&misc(g, 64 - 1), b" ", &name]);
                    je_draw_text_window(g, &text);

                    g.player.player[0].items.sidekick = [(evalue - 32000) as u8; 2];
                    g.config.shot_multi_pos[SHOT_LEFT_SIDEKICK] = 0;
                    g.config.shot_multi_pos[SHOT_RIGHT_SIDEKICK] = 0;

                    je_draw_options(g);
                    g.varz.sound_queue[7] = S_POWERUP;
                }
                if g.varz.enemy_avail[z] == 1 {
                    let pl = &mut g.player.player[p];
                    pl.cash = pl.cash.wrapping_add(250);
                }
            } else if evalue > 31000 {
                let pl = &mut g.player.player[p];
                pl.cash = pl.cash.wrapping_add(250);
                let id = (evalue - 31000) as u8;
                let name = at(&g.episodes.weapon_port, usize::from(id)).name;
                if player_num == 2 {
                    let text = if g.network.is_network_game {
                        net_pickup_text(g, 2, &name)
                    } else {
                        cat(&[&misc(g, 44 - 1), b" ", &name])
                    };
                    je_draw_text_window(g, &text);
                    g.player.player[1].items.weapon[REAR_WEAPON].id = id;
                    g.config.shot_multi_pos[SHOT_REAR] = 0;
                    g.varz.enemy_avail[z] = 1;
                    g.varz.sound_queue[7] = S_POWERUP;
                } else if g.config.one_player_action {
                    let text = cat(&[&misc(g, 64 - 1), b" ", &name]);
                    je_draw_text_window(g, &text);
                    g.player.player[0].items.weapon[REAR_WEAPON].id = id;
                    g.config.shot_multi_pos[SHOT_REAR] = 0;
                    g.varz.enemy_avail[z] = 1;
                    g.varz.sound_queue[7] = S_POWERUP;

                    let rear = &mut g.player.player[0].items.weapon[REAR_WEAPON];
                    if rear.power == 0 {
                        // does this ever happen?
                        rear.power = 1;
                    }
                }
            } else if evalue > 30000 {
                let id = (evalue - 30000) as u8;
                let name = at(&g.episodes.weapon_port, usize::from(id)).name;
                if player_num == 1 && g.config.two_player_mode {
                    let text = if g.network.is_network_game {
                        net_pickup_text(g, 1, &name)
                    } else {
                        cat(&[&misc(g, 43 - 1), b" ", &name])
                    };
                    je_draw_text_window(g, &text);
                    g.player.player[0].items.weapon[FRONT_WEAPON].id = id;
                    g.config.shot_multi_pos[SHOT_FRONT] = 0;
                    g.varz.enemy_avail[z] = 1;
                    g.varz.sound_queue[7] = S_POWERUP;
                } else if g.config.one_player_action {
                    let text = cat(&[&misc(g, 64 - 1), b" ", &name]);
                    je_draw_text_window(g, &text);
                    g.player.player[0].items.weapon[FRONT_WEAPON].id = id;
                    g.config.shot_multi_pos[SHOT_FRONT] = 0;
                    g.varz.enemy_avail[z] = 1;
                    g.varz.sound_queue[7] = S_POWERUP;
                }

                if g.varz.enemy_avail[z] == 1 {
                    let special = at(&SPECIAL_ARCADE_WEAPON, (evalue - 30000 - 1) as usize);
                    g.player.player[0].items.special = special;
                    if special > 0 {
                        let c = &mut g.config;
                        c.shot_multi_pos[SHOT_SPECIAL] = 0;
                        c.shot_repeat[SHOT_SPECIAL] = 0;
                        c.shot_multi_pos[SHOT_SPECIAL2] = 0;
                        c.shot_repeat[SHOT_SPECIAL2] = 0;
                    }
                    let pl = &mut g.player.player[p];
                    pl.cash = pl.cash.wrapping_add(250);
                }
            }
        } else if evalue > 20000 {
            if g.config.two_player_linked {
                // share the armor evenly between linked players
                let share = ((evalue - 20000) as u64 / 2) as u32;
                for pl in g.player.player.iter_mut() {
                    pl.armor = pl.armor.wrapping_add(share);
                    if pl.armor > 28 {
                        pl.armor = 28;
                    }
                }
            } else {
                let pl = &mut g.player.player[p];
                pl.armor = pl.armor.wrapping_add((evalue - 20000) as u32);
                if pl.armor > 28 {
                    pl.armor = 28;
                }
            }
            g.varz.enemy_avail[z] = 1;
            g.video.vga_screen = g.video.vga_screen_seg; // side-effect of game_screen
            je_draw_armor(g);
            g.video.vga_screen = g.video.game_screen; // side-effect of game_screen
            g.varz.sound_queue[7] = S_POWERUP;
        } else if evalue > 10000 && g.varz.enemy_avail[z] == 2 {
            if !g.episodes.bonus_level {
                play_song(g, 30)?; // Zanac
                g.episodes.bonus_level = true;
                g.config.next_level = (evalue - 10000) as u8;
                g.varz.enemy_avail[z] = 1;
                g.varz.display_time = 150;
            }
        } else if e.scoreitem {
            g.varz.enemy_avail[z] = 1;
            g.varz.sound_queue[7] = S_ITEM;
            if evalue == 1 {
                g.config.cube_max = g.config.cube_max.wrapping_add(1);
                g.varz.sound_queue[3] = V_DATA_CUBE;
            } else if evalue == -1 {
                // got front weapon powerup
                let item = misc(g, 45 - 1);
                let text = if g.network.is_network_game {
                    net_pickup_text(g, 1, &item)
                } else if g.config.two_player_mode {
                    cat(&[&misc(g, 43 - 1), b" ", &item])
                } else {
                    item
                };
                je_draw_text_window(g, &text);

                power_up_weapon(g, 0, FRONT_WEAPON as u32);
                g.varz.sound_queue[7] = S_POWERUP;
            } else if evalue == -2 {
                // got rear weapon powerup
                let item = misc(g, 46 - 1);
                let text = if g.network.is_network_game {
                    net_pickup_text(g, 2, &item)
                } else if g.config.two_player_mode {
                    cat(&[&misc(g, 44 - 1), b" ", &item])
                } else {
                    item
                };
                je_draw_text_window(g, &text);

                let target = usize::from(g.config.two_player_mode);
                power_up_weapon(g, target, REAR_WEAPON as u32);
                g.varz.sound_queue[7] = S_POWERUP;
            } else if evalue == -3 {
                // picked up orbiting asteroid killer
                g.config.shot_multi_pos[SHOT_MISC] = 0;
                let p0 = g.player.player[0];
                g.varz.b = player_shot_create(
                    g,
                    0,
                    SHOT_MISC as u32,
                    pl.x as u16,
                    pl.y as u16,
                    p0.mouse_x,
                    p0.mouse_y,
                    104,
                    player_num,
                );
                // An enemy slot past `shotAvail` writes nothing
                // (D-UB-MAININT-SHOTAVAIL).
                if let Some(a) = g.shots.shot_avail.get_mut(z) {
                    *a = 0;
                }
            } else if evalue == -4 {
                let bomber = &mut g.player.player[usize::from(player_num) - 1];
                if bomber.superbombs < 10 {
                    bomber.superbombs += 1;
                }
            } else if evalue == -5 {
                let pls = &mut g.player.player;
                pls[0].items.weapon[FRONT_WEAPON].id = 25; // HOT DOG!
                pls[0].items.weapon[REAR_WEAPON].id = 26;
                pls[1].items.weapon[REAR_WEAPON].id = 26;

                pls[0].last_items = pls[0].items;

                for pl in pls.iter_mut() {
                    pl.weapon_mode = 1;
                }

                g.config.shot_multi_pos = [0; 11];
            } else if g.config.two_player_linked {
                // players get equal share of pick-up cash when linked
                let share = (evalue as i64 as u64) / 2;
                for pl in g.player.player.iter_mut() {
                    pl.cash = pl.cash.wrapping_add(share);
                }
            } else {
                let pl = &mut g.player.player[p];
                pl.cash = pl.cash.wrapping_add(evalue as i64 as u64);
            }
            let explosion_type = g.episodes.enemy_dat[usize::from(e.enemytype)].explosiontype;
            je_setup_explosion(
                g,
                enemy_screen_x as i16,
                e.ey,
                0,
                i16::from(explosion_type),
                true,
                false,
            );
        } else if pl.invulnerable_ticks == 0
            && g.varz.enemy_avail[z] == 0
            && (g.episodes.enemy_dat[usize::from(e.enemytype)].explosiontype & 1) == 0
        {
            // explosiontype & 1 == 0: not ground enemy
            let mut armorleft = i32::from(e.armorleft);
            if armorleft > i32::from(g.varz.damage_rate) {
                armorleft = i32::from(g.varz.damage_rate);
            }

            je_player_damage(g, armorleft as u8, p);

            // player ship gets push-back from collision
            let e = g.varz.enemy[z];
            if e.armorleft > 0 {
                let pl = &mut g.player.player[p];
                pl.x_velocity += (i32::from(e.exc) * i32::from(e.armorleft)) / 2;
                pl.y_velocity += (i32::from(e.eyc) * i32::from(e.armorleft)) / 2;
            }

            let mut armorleft2 = i32::from(e.armorleft);
            if armorleft2 == 255 {
                armorleft2 = 30000;
            }

            g.varz.temp = e.linknum;
            if g.varz.temp == 0 {
                g.varz.temp = 255;
            }

            g.varz.b = z as i16;

            if armorleft2 > armorleft {
                // damage enemy
                if e.armorleft != 255 {
                    let a = &mut g.varz.enemy[z].armorleft;
                    *a = a.wrapping_sub(armorleft as u8);
                }
                g.varz.sound_queue[5] = S_ENEMY_HIT;
            } else {
                // kill enemy
                g.varz.temp2 = 0;
                while g.varz.temp2 < 100 {
                    let t2 = usize::from(g.varz.temp2);
                    if g.varz.enemy_avail[t2] != 1 {
                        g.varz.temp3 = g.varz.enemy[t2].linknum;
                        let temp = i32::from(g.varz.temp);
                        let temp3 = i32::from(g.varz.temp3);
                        if i32::from(g.varz.temp2) == i32::from(g.varz.b)
                            || (temp != 255
                                && (temp == temp3
                                    || temp - 100 == temp3
                                    || (temp3 > 40 && temp3 / 20 == temp / 20 && temp3 <= temp)))
                        {
                            let other = g.varz.enemy[t2];
                            let enemy_screen_x = i32::from(other.ex) + i32::from(other.mapoffset);

                            g.varz.enemy[t2].linknum = 0;

                            g.varz.enemy_avail[t2] = 1;

                            if g.episodes.enemy_dat[usize::from(other.enemytype)].esize == 1 {
                                je_setup_explosion_large(
                                    g,
                                    other.enemyground,
                                    other.explonum,
                                    enemy_screen_x as i16,
                                    other.ey,
                                );
                                g.varz.sound_queue[6] = S_EXPLOSION_9;
                            } else {
                                je_setup_explosion(
                                    g,
                                    enemy_screen_x as i16,
                                    other.ey,
                                    0,
                                    1,
                                    false,
                                    false,
                                );
                                g.varz.sound_queue[5] = S_EXPLOSION_4;
                            }
                        }
                    }
                    g.varz.temp2 += 1;
                }
                g.varz.enemy_avail[z] = 1;
            }
        }
    }
    Ok(())
}
