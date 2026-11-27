// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `game_menu.c` / `game_menu.h`.

use crate::config::SAVE_FILES_NUM;
use crate::helptext::MENU_MAX;
use crate::player::PlayerItems;
use serde::{Deserialize, Serialize};

pub const MENU_FULL_GAME: u8 = 0;
pub const MENU_UPGRADES: u8 = 1;
pub const MENU_OPTIONS: u8 = 2;
pub const MENU_PLAY_NEXT_LEVEL: u8 = 3;
pub const MENU_UPGRADE_SUB: u8 = 4;
pub const MENU_KEYBOARD_CONFIG: u8 = 5;
pub const MENU_LOAD_SAVE: u8 = 6;
pub const MENU_DATA_CUBES: u8 = 7;
pub const MENU_DATA_CUBE_SUB: u8 = 8;
pub const MENU_2_PLAYER_ARCADE: u8 = 9;
/// Also networked games.
pub const MENU_1_PLAYER_ARCADE: u8 = 10;
/// Hides save/load menus.
pub const MENU_LIMITED_OPTIONS: u8 = 11;
pub const MENU_JOYSTICK_CONFIG: u8 = 12;
pub const MENU_SUPER_TYRIAN: u8 = 13;

/// `JE_MenuChoiceType`.
pub type JeMenuChoiceType = [u8; MENU_MAX];

/// `struct cube_struct`: one data cube; NUL-terminated `char` buffers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CubeStruct {
    #[serde(with = "crate::serde_big")]
    pub title: [u8; 81],
    pub header: [u8; 13],
    pub face_sprite: i32,
    #[serde(with = "crate::serde_big::d2")]
    pub text: [[u8; 36]; 90],
    pub last_line: i32,
}

impl Default for CubeStruct {
    fn default() -> Self {
        CubeStruct {
            title: [0; 81],
            header: [0; 13],
            face_sprite: 0,
            text: [[0; 36]; 90],
            last_line: 0,
        }
    }
}

/// `menuChoicesDefault`.
pub const MENU_CHOICES_DEFAULT: JeMenuChoiceType = [
    7,
    9,
    8,
    0,
    0,
    11,
    (SAVE_FILES_NUM / 2 + 2) as u8,
    0,
    0,
    6,
    4,
    6,
    7,
    5,
];
/// `menuEsc`.
pub const MENU_ESC: [u8; MENU_MAX] = [0, 1, 1, 1, 2, 3, 3, 1, 8, 0, 0, 11, 3, 0];
/// `itemAvailMap`.
pub const ITEM_AVAIL_MAP: [u8; 7] = [1, 2, 3, 9, 4, 6, 7];
/// `planetX`.
pub const PLANET_X: [u16; 21] = [
    200, 150, 240, 300, 270, 280, 320, 260, 220, 150, 160, 210, 80, 240, 220, 180, 310, 330, 150,
    240, 200,
];
/// `planetY`.
pub const PLANET_Y: [u16; 21] = [
    40, 90, 90, 80, 170, 30, 50, 130, 120, 150, 220, 200, 80, 50, 160, 10, 55, 55, 90, 90, 40,
];
/// `cube_line_chars`.
pub const CUBE_LINE_CHARS: u32 = 36 - 1;
/// `cube_line_width`.
pub const CUBE_LINE_WIDTH: u32 = 150;

/// Mutable globals of `game_menu.c` (rule R3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// Which joystick is being configured in the menu.
    pub joystick_config: i32,
    pub y_loc: u16,
    pub y_chg: i8,
    pub new_pal: i32,
    pub cur_pal: i32,
    pub old_pal: i32,
    pub quik_save: bool,
    pub old_menu: u8,
    pub back_from_help: bool,
    pub last_direction: i16,
    pub first_menu9: bool,
    pub palette_changed: bool,
    pub menu_choices: JeMenuChoiceType,
    pub col: i16,
    pub col_c: i16,
    pub last_cur_sel: u8,
    pub cur_menu: i16,
    pub cur_sel: [u8; MENU_MAX],
    pub cur_item_type: u8,
    pub cur_item: u8,
    pub cursor: u8,
    pub left_power: bool,
    pub right_power: bool,
    pub right_power_afford: bool,
    pub current_cube: u8,
    pub planet_ani: u8,
    pub planet_ani_wait: u8,
    pub current_dot_num: u8,
    pub current_dot_wait: u8,
    pub nav_x: f32,
    pub nav_y: f32,
    pub new_nav_x: f32,
    pub new_nav_y: f32,
    pub temp_nav_x: i16,
    pub temp_nav_y: i16,
    pub planet_dots: [u8; 5],
    pub planet_dot_x: [[i16; 10]; 5],
    pub planet_dot_y: [[i16; 10]; 5],
    pub old_items: [PlayerItems; 2],
    #[serde(with = "crate::serde_big::boxed")]
    pub cube: Box<[CubeStruct; 4]>,
}

impl Default for State {
    fn default() -> Self {
        State {
            joystick_config: 0,
            y_loc: 0,
            y_chg: 0,
            new_pal: 0,
            cur_pal: 0,
            old_pal: 0,
            quik_save: false,
            old_menu: 0,
            back_from_help: false,
            last_direction: 0,
            first_menu9: false,
            palette_changed: false,
            menu_choices: [0; MENU_MAX],
            col: 0,
            col_c: 0,
            last_cur_sel: 0,
            cur_menu: 0,
            cur_sel: [0; MENU_MAX],
            cur_item_type: 0,
            cur_item: 0,
            cursor: 0,
            left_power: false,
            right_power: false,
            right_power_afford: false,
            current_cube: 0,
            planet_ani: 0,
            planet_ani_wait: 0,
            current_dot_num: 0,
            current_dot_wait: 0,
            nav_x: 0.0,
            nav_y: 0.0,
            new_nav_x: 0.0,
            new_nav_y: 0.0,
            temp_nav_x: 0,
            temp_nav_y: 0,
            planet_dots: [0; 5],
            planet_dot_x: [[0; 10]; 5],
            planet_dot_y: [[0; 10]; 5],
            old_items: [PlayerItems::default(); 2],
            cube: vec![CubeStruct::default(); 4]
                .into_boxed_slice()
                .try_into()
                .unwrap_or_else(|_| unreachable!("length is 4")),
        }
    }
}
