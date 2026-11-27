// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `opentyr.c` / `opentyr.h`.

use crate::Game;
use crate::destruct::je_destruct_game;
use crate::episodes::snprintf_into;
use crate::font::{
    ALIGN_CENTER, FONT_LARGE, FONT_NORMAL, draw_font_hv_shadow, draw_font_hv_shadow_aligned,
};
use crate::fonthand::{PART_SHADE, je_text_shade};
use crate::game::{EXIT_FAILURE, Flow, Halt};
use crate::jukebox::jukebox;
use crate::keyboard::{
    INPUT_ANY, KeyboardInput, MouseInput, has_input, keyboard_get_input, mouse_get_input,
    wait_until_elapsed,
};
use crate::loudness::{audio, restart_song, set_volume};
use crate::mouse::{MOUSE_POINTER_NORMAL, je_mouse_replace, je_mouse_start};
use crate::nortsong::{je_change_volume, je_play_sample_num, set_frame_count};
use crate::nortvars::je_bar_draw_shadow;
use crate::palette::{fade_black, fade_palette};
use crate::picload::je_load_pic;
use crate::sndmast::{S_CLICK, S_CURSOR, S_SELECT, S_SPRING};
use crate::sprite::je_load_comp_shapes;
use crate::varz::Sprite2Ref;
use crate::vga256d::{fill_rectangle_wh, je_rectangle};
use crate::video::{
    SCALING_MODE_MAX, SCALING_MODE_NAMES, ScalingMode, init_scaler, je_show_vga, reinit_fullscreen,
};
use crate::video_scale::{SCALERS, SCALERS_COUNT};
use serde::{Deserialize, Serialize};

/// `OPENTYRIAN_VERSION`: `rustyrian-<workspace version>`, as the oracle is
/// built too. Upstream logs it and shows it on the title screen, so both
/// engines must agree on it for parity, and `tests/golden/` is re-blessed
/// with each version bump.
pub const OPENTYRIAN_VERSION: &str = concat!("rustyrian-", env!("CARGO_PKG_VERSION"));

/// `M_PI`.
pub const M_PI: f64 = std::f64::consts::PI;
/// `M_PI_2`.
pub const M_PI_2: f64 = std::f64::consts::FRAC_PI_2;
/// `M_PI_4`.
pub const M_PI_4: f64 = std::f64::consts::FRAC_PI_4;

/// A handler `main` registers with `atexit`; the entry point runs them in
/// reverse order before the process ends, as C's `exit` does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AtExit {
    /// `SDL_Quit`.
    SdlQuit,
}

/// Mutable globals of `opentyr.c` (rule R3), plus the `atexit` registrations
/// `main` makes.
///
/// `opentyrian_str`/`opentyrian_version` are non-`const` pointers to string
/// literals; they hold the string bytes without the NUL.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub opentyrian_str: Vec<u8>,
    pub opentyrian_version: Vec<u8>,
    /// `atexit` registrations, oldest first.
    pub atexit: Vec<AtExit>,
}

impl Default for State {
    fn default() -> Self {
        State {
            opentyrian_str: b"OpenTyrian".to_vec(),
            opentyrian_version: OPENTYRIAN_VERSION.as_bytes().to_vec(),
            atexit: Vec::new(),
        }
    }
}

/// Runs the `atexit` registrations in reverse order and clears them, as C's
/// `exit` does before the process ends.
pub fn run_atexit(g: &mut Game) {
    while let Some(handler) = g.opentyr.atexit.pop() {
        match handler {
            AtExit::SdlQuit => g.platform.quit(),
        }
    }
}

/// The bytes before the first NUL of `buffer`.
fn c_str(buffer: &[u8]) -> &[u8] {
    let n = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    &buffer[..n]
}

/// Setup-menu picker: the window plus one entry per display.
#[doc(alias = "getDisplayPickerItemsCount")]
pub fn get_display_picker_items_count(g: &mut Game) -> usize {
    // `(size_t)` of the `int` count.
    1usize.wrapping_add(g.platform.get_num_video_displays() as usize)
}

/// Setup-menu picker item `i`; display names are formatted into `buffer`
/// (truncated as `snprintf` does). Returns the text without its NUL.
#[doc(alias = "getDisplayPickerItem")]
pub fn get_display_picker_item(i: usize, buffer: &mut [u8]) -> Vec<u8> {
    if i == 0 {
        return b"Window".to_vec();
    }

    snprintf_into(buffer, &format!("Display {}", i as i32));
    c_str(buffer).to_vec()
}

#[doc(alias = "getScalerPickerItemsCount")]
pub fn get_scaler_picker_items_count() -> usize {
    SCALERS_COUNT as usize
}

#[doc(alias = "getScalerPickerItem")]
pub fn get_scaler_picker_item(i: usize, _buffer: &mut [u8]) -> Vec<u8> {
    SCALERS[i].name.as_bytes().to_vec()
}

#[doc(alias = "getScalingModePickerItemsCount")]
pub fn get_scaling_mode_picker_items_count() -> usize {
    SCALING_MODE_MAX
}

#[doc(alias = "getScalingModePickerItem")]
pub fn get_scaling_mode_picker_item(i: usize, _buffer: &mut [u8]) -> Vec<u8> {
    SCALING_MODE_NAMES[i].as_bytes().to_vec()
}

/// `MenuItemId` of `setupMenu`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MenuItemId {
    None,
    Done,
    Graphics,
    Sound,
    Jukebox,
    #[expect(
        dead_code,
        reason = "upstream comments the Destruct entry out of the setup menu"
    )]
    Destruct,
    Display,
    Scaler,
    ScalingMode,
    MusicVolume,
    SoundVolume,
}

/// The picker callbacks of a menu item (rule R8: function pointers become an
/// enum).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Picker {
    Display,
    Scaler,
    ScalingMode,
}

impl Picker {
    /// `getPickerItemsCount()`.
    fn count(self, g: &mut Game) -> usize {
        match self {
            Picker::Display => get_display_picker_items_count(g),
            Picker::Scaler => get_scaler_picker_items_count(),
            Picker::ScalingMode => get_scaling_mode_picker_items_count(),
        }
    }

    /// `getPickerItem(i, buffer, sizeof buffer)`.
    fn item(self, i: usize, buffer: &mut [u8]) -> Vec<u8> {
        match self {
            Picker::Display => get_display_picker_item(i, buffer),
            Picker::Scaler => get_scaler_picker_item(i, buffer),
            Picker::ScalingMode => get_scaling_mode_picker_item(i, buffer),
        }
    }
}

/// `MenuItem` of `setupMenu`.
struct MenuItem {
    id: MenuItemId,
    name: &'static [u8],
    description: &'static [u8],
    picker: Option<Picker>,
}

const fn item(id: MenuItemId, name: &'static [u8], description: &'static [u8]) -> MenuItem {
    MenuItem {
        id,
        name,
        description,
        picker: None,
    }
}

const fn picker_item(
    id: MenuItemId,
    name: &'static [u8],
    description: &'static [u8],
    picker: Picker,
) -> MenuItem {
    MenuItem {
        id,
        name,
        description,
        picker: Some(picker),
    }
}

/// `Menu` of `setupMenu`: the items before the `-1` terminator.
struct Menu {
    header: &'static [u8],
    items: &'static [MenuItem],
}

const MENU_NONE: usize = 0;
const MENU_SETUP: usize = 1;
const MENU_GRAPHICS: usize = 2;
const MENU_SOUND: usize = 3;

/// The setup menus, indexed by `MenuId`. Upstream keeps the Destruct entry
/// commented out of the setup menu.
const MENUS: [Menu; 4] = [
    Menu {
        header: b"",
        items: &[],
    },
    Menu {
        header: b"Setup",
        items: &[
            item(
                MenuItemId::Graphics,
                b"Graphics...",
                b"Change the graphics settings.",
            ),
            item(
                MenuItemId::Sound,
                b"Sound...",
                b"Change the sound settings.",
            ),
            item(
                MenuItemId::Jukebox,
                b"Jukebox",
                b"Listen to the music of Tyrian.",
            ),
            item(MenuItemId::Done, b"Done", b"Return to the main menu."),
        ],
    },
    Menu {
        header: b"Graphics",
        items: &[
            picker_item(
                MenuItemId::Display,
                b"Display:",
                b"Change the display mode.",
                Picker::Display,
            ),
            picker_item(
                MenuItemId::Scaler,
                b"Scaler:",
                b"Change the pixel art scaling algorithm.",
                Picker::Scaler,
            ),
            picker_item(
                MenuItemId::ScalingMode,
                b"Scaling Mode:",
                b"Change the scaling mode.",
                Picker::ScalingMode,
            ),
            item(MenuItemId::Done, b"Done", b"Return to the previous menu."),
        ],
    },
    Menu {
        header: b"Sound",
        items: &[
            item(
                MenuItemId::MusicVolume,
                b"Music Volume",
                b"Change volume with the left/right arrow keys.",
            ),
            item(
                MenuItemId::SoundVolume,
                b"Sound Volume",
                b"Change volume with the left/right arrow keys.",
            ),
            item(MenuItemId::Done, b"Done", b"Return to the previous menu."),
        ],
    },
];

const SDL_BUTTON_LEFT: u8 = 1;
const SDL_BUTTON_RIGHT: u8 = 3;
const SDL_SCANCODE_RETURN: u16 = 40;
const SDL_SCANCODE_ESCAPE: u16 = 41;
const SDL_SCANCODE_SPACE: u16 = 44;
const SDL_SCANCODE_RIGHT: u16 = 79;
const SDL_SCANCODE_LEFT: u16 = 80;
const SDL_SCANCODE_DOWN: u16 = 81;
const SDL_SCANCODE_UP: u16 = 82;

/// The setup menu: graphics (display, scaler, scaling mode pickers), sound
/// (volumes, mute toggles) and the jukebox. Returns when backed out of.
#[doc(alias = "setupMenu")]
pub fn setup_menu(g: &mut Game) -> Flow {
    let mut buffer = [0u8; 100];

    if g.sprite.shop_sprite_sheet.data.is_none() {
        // The mouse pointer sprites.
        je_load_comp_shapes(g, Sprite2Ref::ShopSpriteSheet, b'1')?;
    }

    let mut restart = true;

    let mut menu_parents = [MENU_NONE; MENUS.len()];
    let mut selected_menu_item_indexes = [0usize; MENUS.len()];
    let mut current_menu = MENU_SETUP;
    let mut current_picker = MenuItemId::None;
    let mut picker_selected_index: usize = 0;

    const X_CENTER: i32 = 320 / 2;
    const Y_MENU_HEADER: i32 = 4;
    const X_MENU_ITEM: i32 = 45;
    const X_MENU_ITEM_NAME: i32 = X_MENU_ITEM;
    const W_MENU_ITEM_NAME: i32 = 135;
    const X_MENU_ITEM_VALUE: i32 = X_MENU_ITEM_NAME + W_MENU_ITEM_NAME;
    const W_MENU_ITEM_VALUE: i32 = 95;
    const W_MENU_ITEM: i32 = W_MENU_ITEM_NAME + W_MENU_ITEM_VALUE;
    const Y_MENU_ITEMS: i32 = 37;
    const DY_MENU_ITEMS: i32 = 21;
    const H_MENU_ITEM: i32 = 13;

    loop {
        set_frame_count(g, 1);

        let screen2 = g
            .video
            .vga_screen2
            .expect("VGAScreen2 used before init_video");
        if restart {
            je_load_pic(g, screen2, 2, false)?;
            fill_rectangle_wh(g, screen2, 0, 192, 320, 8, 0);
        }

        // Restore the background.
        let screen = g.video.vga_screen_id();
        {
            let (d, s) = g.video.screen_pair_mut(screen, screen2);
            let n = (d.pitch * d.h) as usize;
            d.pixels[..n].copy_from_slice(&s.pixels[..n]);
        }

        let menu = &MENUS[current_menu];

        // Header.
        draw_font_hv_shadow_aligned(
            g,
            screen,
            X_CENTER,
            Y_MENU_HEADER,
            menu.header,
            FONT_LARGE,
            ALIGN_CENTER,
            15,
            -3,
            false,
            2,
        );

        let mut y_picker: i32 = 0;
        const DY_PICKER_ITEM: i32 = 15;
        const DY_PICKER_ITEM_PADDING: i32 = 2;
        const H_PICKER_ITEM: i32 = DY_PICKER_ITEM - DY_PICKER_ITEM_PADDING;

        let menu_items = menu.items;

        // Menu items.

        let menu_items_count = menu_items.len();
        for (i, menu_item) in menu_items.iter().enumerate() {
            let y = Y_MENU_ITEMS + DY_MENU_ITEMS * i as i32;

            let selected = i == selected_menu_item_indexes[current_menu];
            let disabled = current_picker != MenuItemId::None && !selected;

            if selected {
                y_picker = y;
            }

            let value = -3 + if selected { 2 } else { 0 } + if disabled { -4 } else { 0 };
            draw_font_hv_shadow(
                g,
                screen,
                X_MENU_ITEM_NAME,
                y,
                menu_item.name,
                FONT_NORMAL,
                15,
                value,
                false,
                2,
            );

            match menu_item.id {
                MenuItemId::Display => {
                    let mut text: &[u8] = b"Window";
                    if g.video.fullscreen_display >= 0 {
                        let d = g.video.fullscreen_display.wrapping_add(1);
                        snprintf_into(&mut buffer, &format!("Display {d}"));
                        text = c_str(&buffer);
                    }

                    draw_font_hv_shadow(
                        g,
                        screen,
                        X_MENU_ITEM_VALUE,
                        y,
                        text,
                        FONT_NORMAL,
                        15,
                        value,
                        false,
                        2,
                    );
                }
                MenuItemId::Scaler => {
                    let name = SCALERS[g.video_scale.scaler as usize].name.as_bytes();
                    draw_font_hv_shadow(
                        g,
                        screen,
                        X_MENU_ITEM_VALUE,
                        y,
                        name,
                        FONT_NORMAL,
                        15,
                        value,
                        false,
                        2,
                    );
                }
                MenuItemId::ScalingMode => {
                    let name = SCALING_MODE_NAMES[g.video.scaling_mode.to_c() as usize].as_bytes();
                    draw_font_hv_shadow(
                        g,
                        screen,
                        X_MENU_ITEM_VALUE,
                        y,
                        name,
                        FONT_NORMAL,
                        15,
                        value,
                        false,
                        2,
                    );
                }
                MenuItemId::MusicVolume | MenuItemId::SoundVolume => {
                    let (disabled_flag, volume) = if menu_item.id == MenuItemId::MusicVolume {
                        (audio(g).music_disabled, g.nortsong.tyr_music_volume)
                    } else {
                        (audio(g).samples_disabled, g.nortsong.fx_volume)
                    };
                    let col = if disabled_flag { 170 } else { 174 };
                    je_bar_draw_shadow(
                        g,
                        screen,
                        X_MENU_ITEM_VALUE as u16,
                        y as u16,
                        1,
                        col,
                        ((i32::from(volume) + 4) / 8) as u16,
                        2,
                        10,
                    );
                    je_rectangle(
                        g,
                        screen,
                        X_MENU_ITEM_VALUE - 2,
                        y - 2,
                        X_MENU_ITEM_VALUE + 96,
                        y + 11,
                        242,
                    );
                }
                _ => {}
            }
        }

        // Status text.
        let description = menu_items[selected_menu_item_indexes[current_menu]].description;
        je_text_shade(
            g,
            screen,
            X_MENU_ITEM_NAME,
            190,
            description,
            15,
            4,
            PART_SHADE,
        );

        // Picker box and items.

        if current_picker != MenuItemId::None {
            let selected_menu_item = &menu_items[selected_menu_item_indexes[current_menu]];
            let picker = selected_menu_item.picker.expect("a picker item");
            let picker_items_count = picker.count(g);

            // `size_t` arithmetic stored into an `int`.
            let h_picker = (DY_PICKER_ITEM as usize)
                .wrapping_mul(picker_items_count)
                .wrapping_sub(DY_PICKER_ITEM_PADDING as usize) as i32;
            y_picker = y_picker.min(200 - 10 - (h_picker + 5 + 2));

            let (xv, wv) = (X_MENU_ITEM_VALUE, W_MENU_ITEM_VALUE);
            je_rectangle(
                g,
                screen,
                xv - 5,
                y_picker - 3,
                xv + wv + 5 - 1,
                y_picker + h_picker + 3 - 1,
                248,
            );
            je_rectangle(
                g,
                screen,
                xv - 4,
                y_picker - 4,
                xv + wv + 4 - 1,
                y_picker + h_picker + 4 - 1,
                250,
            );
            je_rectangle(
                g,
                screen,
                xv - 3,
                y_picker - 5,
                xv + wv + 3 - 1,
                y_picker + h_picker + 5 - 1,
                248,
            );
            fill_rectangle_wh(
                g,
                screen,
                xv - 2,
                y_picker - 2,
                (wv + 2 + 2) as u32,
                (h_picker + 2 + 2) as u32,
                224,
            );

            for i in 0..picker_items_count {
                let y = y_picker + DY_PICKER_ITEM * i as i32;

                let selected = i == picker_selected_index;

                let text = picker.item(i, &mut buffer);

                let value = -3 + if selected { 2 } else { 0 };
                draw_font_hv_shadow(g, screen, xv, y, &text, FONT_NORMAL, 15, value, false, 2);
            }
        }

        if restart {
            g.mouse.mouse_cursor = MOUSE_POINTER_NORMAL;

            let colors = g.palette.colors;
            fade_palette(g, &colors, 10, 0, 255)?;

            restart = false;
        }

        je_mouse_start(g);
        je_show_vga(g);
        je_mouse_replace(g);

        let old_fullscreen_display = g.video.fullscreen_display;
        loop {
            wait_until_elapsed(g)?;

            // Fullscreen toggled with the keyboard shortcut: redraw the
            // display setting.
            if g.video.fullscreen_display != old_fullscreen_display {
                break;
            }

            if has_input(g, INPUT_ANY) {
                break;
            }

            set_frame_count(g, 1);
        }

        let selected_menu_item_index = &mut selected_menu_item_indexes[current_menu];

        if current_picker == MenuItemId::None {
            // Menu item interaction.

            let mut action = false;

            let mut mouse_input = MouseInput::default();
            let mut keyboard_input = KeyboardInput::default();

            if mouse_get_input(g, INPUT_ANY, Some(&mut mouse_input)) {
                let (mx, my) = (mouse_input.x, mouse_input.y);
                // The item whose name or value is hovered or clicked.
                if (X_MENU_ITEM..X_MENU_ITEM + W_MENU_ITEM).contains(&mx) {
                    for i in 0..menu_items_count {
                        let y_menu_item = Y_MENU_ITEMS + DY_MENU_ITEMS * i as i32;
                        let rows = y_menu_item..y_menu_item + H_MENU_ITEM;
                        if rows.contains(&my) {
                            if *selected_menu_item_index != i {
                                je_play_sample_num(g, S_CURSOR);

                                *selected_menu_item_index = i;
                            }

                            if mouse_input.button == SDL_BUTTON_LEFT && rows.contains(&my) {
                                if (X_MENU_ITEM_NAME..X_MENU_ITEM_NAME + W_MENU_ITEM_NAME)
                                    .contains(&mx)
                                {
                                    // Act on the item through its name.
                                    action = true;
                                } else if (X_MENU_ITEM_VALUE..X_MENU_ITEM_VALUE + W_MENU_ITEM_VALUE)
                                    .contains(&mx)
                                {
                                    // Act on the item through its value.
                                    let value =
                                        (mx - X_MENU_ITEM_VALUE) * 255 / (W_MENU_ITEM_VALUE - 1);
                                    match menu_items[*selected_menu_item_index].id {
                                        MenuItemId::Display
                                        | MenuItemId::Scaler
                                        | MenuItemId::ScalingMode => action = true,
                                        MenuItemId::MusicVolume => {
                                            je_play_sample_num(g, S_CURSOR);

                                            g.nortsong.tyr_music_volume =
                                                value.clamp(0, 255) as u16;

                                            let n = &g.nortsong;
                                            let (m, f) =
                                                (n.tyr_music_volume as u8, n.fx_volume as u8);
                                            set_volume(g, m, f);
                                        }
                                        MenuItemId::SoundVolume => {
                                            g.nortsong.fx_volume = value.clamp(0, 255) as u16;

                                            let n = &g.nortsong;
                                            let (m, f) =
                                                (n.tyr_music_volume as u8, n.fx_volume as u8);
                                            set_volume(g, m, f);

                                            je_play_sample_num(g, S_CURSOR);
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

                    current_menu = menu_parents[current_menu];
                }
            } else if keyboard_get_input(g, Some(&mut keyboard_input)) {
                let id = menu_items[*selected_menu_item_index].id;
                match keyboard_input.scancode {
                    SDL_SCANCODE_UP => {
                        je_play_sample_num(g, S_CURSOR);

                        *selected_menu_item_index = if *selected_menu_item_index == 0 {
                            menu_items_count - 1
                        } else {
                            *selected_menu_item_index - 1
                        };
                    }
                    SDL_SCANCODE_DOWN => {
                        je_play_sample_num(g, S_CURSOR);

                        *selected_menu_item_index =
                            if *selected_menu_item_index == menu_items_count - 1 {
                                0
                            } else {
                                *selected_menu_item_index + 1
                            };
                    }
                    SDL_SCANCODE_LEFT | SDL_SCANCODE_RIGHT => {
                        let delta = if keyboard_input.scancode == SDL_SCANCODE_LEFT {
                            -8
                        } else {
                            8
                        };
                        match id {
                            MenuItemId::MusicVolume => {
                                je_play_sample_num(g, S_CURSOR);

                                change_volume(g, delta, 0);
                            }
                            MenuItemId::SoundVolume => {
                                change_volume(g, 0, delta);

                                je_play_sample_num(g, S_CURSOR);
                            }
                            _ => {}
                        }
                    }
                    SDL_SCANCODE_SPACE | SDL_SCANCODE_RETURN => action = true,
                    SDL_SCANCODE_ESCAPE => {
                        je_play_sample_num(g, S_SPRING);

                        current_menu = menu_parents[current_menu];
                    }
                    _ => {}
                }
            }

            if action {
                let selected_menu_item_id = menu_items[*selected_menu_item_index].id;

                match selected_menu_item_id {
                    MenuItemId::Done => {
                        je_play_sample_num(g, S_SELECT);

                        current_menu = menu_parents[current_menu];
                    }
                    MenuItemId::Graphics => {
                        je_play_sample_num(g, S_SELECT);

                        menu_parents[MENU_GRAPHICS] = current_menu;
                        current_menu = MENU_GRAPHICS;
                        selected_menu_item_indexes[current_menu] = 0;
                    }
                    MenuItemId::Sound => {
                        je_play_sample_num(g, S_SELECT);

                        menu_parents[MENU_SOUND] = current_menu;
                        current_menu = MENU_SOUND;
                        selected_menu_item_indexes[current_menu] = 0;
                    }
                    MenuItemId::Jukebox => {
                        je_play_sample_num(g, S_SELECT);

                        fade_black(g, 10)?;

                        jukebox(g)?;

                        restart = true;
                    }
                    MenuItemId::Destruct => {
                        je_play_sample_num(g, S_SELECT);

                        fade_black(g, 10)?;

                        je_destruct_game(g)?;

                        restart = true;
                    }
                    MenuItemId::Display => {
                        je_play_sample_num(g, S_CLICK);

                        current_picker = selected_menu_item_id;
                        picker_selected_index = g.video.fullscreen_display.wrapping_add(1) as usize;
                    }
                    MenuItemId::Scaler => {
                        je_play_sample_num(g, S_CLICK);

                        current_picker = selected_menu_item_id;
                        picker_selected_index = g.video_scale.scaler as usize;
                    }
                    MenuItemId::ScalingMode => {
                        je_play_sample_num(g, S_CLICK);

                        current_picker = selected_menu_item_id;
                        picker_selected_index = g.video.scaling_mode.to_c() as usize;
                    }
                    MenuItemId::MusicVolume => {
                        je_play_sample_num(g, S_CLICK);

                        let enabled = {
                            let mut a = audio(g);
                            a.music_disabled = !a.music_disabled;
                            !a.music_disabled
                        };
                        if enabled {
                            restart_song(g);
                        }
                    }
                    MenuItemId::SoundVolume => {
                        {
                            let mut a = audio(g);
                            a.samples_disabled = !a.samples_disabled;
                        }

                        je_play_sample_num(g, S_CLICK);
                    }
                    MenuItemId::None => {}
                }
            }

            if current_menu == MENU_NONE {
                fade_black(g, 10)?;

                return Ok(());
            }
        } else {
            let selected_menu_item = &menu_items[*selected_menu_item_index];
            let picker = selected_menu_item.picker.expect("a picker item");

            // Picker interaction.

            let mut action = false;

            let mut mouse_input = MouseInput::default();
            let mut keyboard_input = KeyboardInput::default();

            if mouse_get_input(g, INPUT_ANY, Some(&mut mouse_input)) {
                let picker_items_count = picker.count(g);

                let (mx, my) = (mouse_input.x, mouse_input.y);
                // The picker item hovered or clicked.
                if (X_MENU_ITEM_VALUE..X_MENU_ITEM_VALUE + W_MENU_ITEM_VALUE).contains(&mx) {
                    for i in 0..picker_items_count {
                        let y_picker_item = y_picker + DY_PICKER_ITEM * i as i32;
                        let rows = y_picker_item..y_picker_item + H_PICKER_ITEM;

                        if rows.contains(&my) {
                            if picker_selected_index != i {
                                je_play_sample_num(g, S_CURSOR);

                                picker_selected_index = i;
                            }

                            // Act on the picker item. Upstream compares `y`
                            // with the item's right edge here.
                            if mouse_input.button == SDL_BUTTON_LEFT
                                && mx >= X_MENU_ITEM_VALUE
                                && my < X_MENU_ITEM_VALUE + W_MENU_ITEM_NAME
                                && rows.contains(&my)
                            {
                                action = true;
                            }
                        }
                    }
                }

                if mouse_input.button == SDL_BUTTON_RIGHT {
                    je_play_sample_num(g, S_SPRING);

                    current_picker = MenuItemId::None;
                }
            } else if keyboard_get_input(g, Some(&mut keyboard_input)) {
                match keyboard_input.scancode {
                    SDL_SCANCODE_UP => {
                        je_play_sample_num(g, S_CURSOR);

                        let picker_items_count = picker.count(g);

                        picker_selected_index = if picker_selected_index == 0 {
                            picker_items_count.wrapping_sub(1)
                        } else {
                            picker_selected_index - 1
                        };
                    }
                    SDL_SCANCODE_DOWN => {
                        je_play_sample_num(g, S_CURSOR);

                        let picker_items_count = picker.count(g);

                        picker_selected_index =
                            if picker_selected_index == picker_items_count.wrapping_sub(1) {
                                0
                            } else {
                                picker_selected_index + 1
                            };
                    }
                    SDL_SCANCODE_SPACE | SDL_SCANCODE_RETURN => action = true,
                    SDL_SCANCODE_ESCAPE => {
                        je_play_sample_num(g, S_SPRING);

                        current_picker = MenuItemId::None;
                    }
                    _ => {}
                }
            }

            if action {
                je_play_sample_num(g, S_CLICK);

                match selected_menu_item.id {
                    MenuItemId::Display => {
                        let display = (picker_selected_index as i32).wrapping_sub(1);
                        if display != g.video.fullscreen_display {
                            reinit_fullscreen(g, display);
                        }
                    }
                    MenuItemId::Scaler => {
                        if picker_selected_index != g.video_scale.scaler as usize {
                            let old_scaler = g.video_scale.scaler;
                            // Try the new scaler; revert on failure.
                            if !init_scaler(g, picker_selected_index as u32)?
                                && !init_scaler(g, old_scaler)?
                            {
                                return Err(Halt { code: EXIT_FAILURE });
                            }
                        }
                    }
                    MenuItemId::ScalingMode => {
                        g.video.scaling_mode = ScalingMode::from_c(picker_selected_index as u32)
                            .expect("a scaling mode picker index");
                    }
                    _ => {}
                }

                current_picker = MenuItemId::None;
            }
        }
    }
}

/// `JE_changeVolume(&tyrMusicVolume, music_delta, &fxVolume, sample_delta)`.
fn change_volume(g: &mut Game, music_delta: i32, sample_delta: i32) {
    let (mut music, mut sample) = (g.nortsong.tyr_music_volume, g.nortsong.fx_volume);
    je_change_volume(g, &mut music, music_delta, &mut sample, sample_delta);
    g.nortsong.tyr_music_volume = music;
    g.nortsong.fx_volume = sample;
}

/// `main(argc, argv)`: initialises everything, then loops over the logos,
/// title screen and game until the player quits. `return EXIT_FAILURE` paths
/// return the code; the final `JE_tyrianHalt(0)` (and any other `exit`)
/// arrives as `Err(Halt)`. The entry point then runs [`run_atexit`].
pub fn main(g: &mut Game, argv: &[String]) -> Flow<i32> {
    use crate::config::{load_configuration, load_saves};
    use crate::editship::load_extra_shapes;
    use crate::episodes::je_scan_for_episodes;
    use crate::file;
    use crate::helptext::je_load_help_text;
    use crate::joystick::init_joysticks;
    use crate::keyboard::init_keyboard;
    use crate::logging::{SDL_LOG_CATEGORY_APPLICATION, log_fatal};
    use crate::loudness::init_audio;
    use crate::mainint::{je_init_player_data, je_sort_high_scores};
    use crate::mtrand::mt_srand;
    use crate::network::{network_init, network_tyrian_halt};
    use crate::nortsong::load_snd_file;
    use crate::palette::load_pals;
    use crate::params::je_param_check;
    use crate::sprite::{free_main_shape_tables, je_load_main_shape_tables};
    use crate::tyrian2::{intro_logos, je_main, network_start_screen, title_screen};
    use crate::varz::je_tyrian_halt;
    use crate::video::init_video;
    use crate::xmas::{xmas_prompt, xmas_time};

    // `mt_srand(time(NULL))`: `time_t` to `unsigned long`.
    let now = g.platform.time();
    mt_srand(g, now as u64);

    let version = String::from_utf8_lossy(&g.opentyr.opentyrian_version).into_owned();
    let name = String::from_utf8_lossy(&g.opentyr.opentyrian_str).into_owned();
    g.platform.log("");
    g.platform
        .log(&format!("Welcome to... >> {name} {version} <<"));
    g.platform.log("");
    g.platform
        .log("Copyright (C) The OpenTyrian Development Team");
    g.platform.log("");
    g.platform
        .log("This program comes with ABSOLUTELY NO WARRANTY.");
    g.platform
        .log("This is free software, and you are welcome to redistribute it");
    g.platform
        .log("under certain conditions.  See the file COPYING for details.");
    g.platform.log("");

    if g.platform.init(0) != 0 {
        let e = g.platform.get_error();
        log_fatal(g, &format!("Failed to initialize SDL: {e}"));
        return Ok(EXIT_FAILURE);
    }

    g.opentyr.atexit.push(AtExit::SdlQuit);

    load_configuration(g);
    load_saves(g);

    // The argument handler may override this.
    g.xmas.xmas = xmas_time(g);

    let mut args: Vec<Vec<u8>> = argv.iter().map(|a| a.as_bytes().to_vec()).collect();
    je_param_check(g, &mut args)?;

    if !file::find_data_files(g) {
        log_fatal(
            g,
            "The Tyrian data files were not found.  OpenTyrian requires the Tyrian v2.0/v2.1 data files.",
        );
        return Ok(EXIT_FAILURE);
    }

    let mut f = file::data_file_open(g, "tyrian.shp", "rb");
    let temp = file::file_read_u16(&mut f);
    file::file_close(&mut f);

    if temp == 11 {
        log_fatal(
            g,
            "The Tyrian v1.0/v1.1 data files were found.  OpenTyrian requires the Tyrian v2.0/v2.1 data files.",
        );
        return Ok(EXIT_FAILURE);
    } else if temp == 13 {
        log_fatal(
            g,
            "The Tyrian 2000 data files were found.  OpenTyrian requires the Tyrian v2.0/v2.1 data files.",
        );
        return Ok(EXIT_FAILURE);
    }

    je_scan_for_episodes(g);

    init_video(g)?;
    init_keyboard(g);
    init_joysticks(g);
    if g.mouse.has_mouse {
        // SDL cannot tell whether there is none.
        g.platform.log("Assuming mouse detected.");
    }

    if g.xmas.xmas
        && (!file::data_file_exists(g, "tyrianc.shp") || !file::data_file_exists(g, "voicesc.snd"))
    {
        g.xmas.xmas = false;

        g.platform
            .log_warn(SDL_LOG_CATEGORY_APPLICATION, "Christmas is missing.");
    }

    load_pals(g)?;
    let shapes = if g.xmas.xmas {
        "tyrianc.shp"
    } else {
        "tyrian.shp"
    };
    je_load_main_shape_tables(g, shapes)?;

    if g.xmas.xmas && !xmas_prompt(g)? {
        g.xmas.xmas = false;

        free_main_shape_tables(g);
        je_load_main_shape_tables(g, "tyrian.shp")?;
    }

    // Default options.
    g.config.you_are_cheating = false;
    g.config.smooth_scroll = true;
    g.varz.load_destruct = false;

    if !g.loudness.audio_disabled {
        if init_audio(g) {
            let xmas = g.xmas.xmas;
            load_snd_file(g, xmas)?;
        }
    } else {
        g.platform.log("Audio is disabled.");
    }

    if g.demo.record_demo {
        g.platform.log("Game will be recorded.");
    }

    load_extra_shapes(g);

    je_load_help_text(g)?;

    if g.network.is_network_game && network_init(g) != 0 {
        network_tyrian_halt(g, 3, false)?;
    }

    let mut do_logos = true;

    loop {
        do_logos |= g.demo.play_demo && !g.demo.stopped_demo;

        if !g.network.is_network_game && do_logos {
            // `NDEBUG` build (rule R11).
            intro_logos(g)?;

            do_logos = false;
        }

        je_init_player_data(g);
        je_sort_high_scores(g);

        g.demo.play_demo = false;
        g.demo.stopped_demo = false;

        g.varz.game_loaded = false;
        g.mainint.jump_section = false;

        if g.network.is_network_game {
            network_start_screen(g)?;
        } else if !title_screen(g)? {
            // The player quit from the title screen.
            break;
        }

        if g.varz.load_destruct {
            je_destruct_game(g)?;

            g.varz.load_destruct = false;
        } else {
            je_main(g)?;

            if g.config.trent_win {
                // The player beat SuperTyrian.
                break;
            }
        }
    }

    je_tyrian_halt(g, 0)?;

    Ok(0)
}
