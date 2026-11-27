// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `game_menu.c` / `game_menu.h`.

use crate::backgrnd::{initialize_starfield, update_and_draw_starfield};
use crate::config::{
    DEFAULT_KEY_SETTINGS, SAVE_FILES_NUM, SHOT_LEFT_SIDEKICK, SHOT_RIGHT_SIDEKICK,
};
use crate::file;
use crate::fonthand::{self, c_str};
use crate::game::{EXIT_FAILURE, Halt};
use crate::game::{Flow, Game};
use crate::helptext::{self, MENU_HELP, MENU_MAX};
use crate::joystick::{Joystick, JoystickAssignment, JoystickAssignmentTypes};
use crate::keyboard::{self, INPUT_NO_MOTION, KeyboardInput, MouseInput, SDL_SCANCODE_RETURN};
use crate::logging::log_fatal;
use crate::mainint::{self, je_get_cost};
use crate::mouse::{MOUSE_POINTER_DOWN, MOUSE_POINTER_NORMAL, MOUSE_POINTER_UP};
use crate::pcxmast::FACEPAL;
use crate::player::{FRONT_WEAPON, LEFT_SIDEKICK, PlayerItems, REAR_WEAPON, RIGHT_SIDEKICK};
use crate::sdl::upper_blit;
use crate::shots::{player_shot_create, simulate_player_shots};
use crate::sndmast::{S_CLICK, S_CURSOR, S_SPRING};
use crate::sndmast::{S_CLINK, S_ITEM};
use crate::sprite::{
    self, FACE_SHAPES, FONT_SHAPES, OPTION_SHAPES, PLANET_SHAPES, SMALL_FONT_SHAPES, TINY_FONT,
    WEAPON_SHAPES, blit_sprite, blit_sprite_dark, blit_sprite_hv, blit_sprite2x2,
};
use crate::varz::je_tyrian_halt;
use crate::varz::{P_ANI, PGR, Sprite2Ref, je_s_gr};
use crate::varz::{SA, SA_NONE};
use crate::vga256d::{fill_rectangle_xy, je_bar_shade, je_pix, je_pix3, je_rectangle};
use crate::video::{self, ScreenId};
use crate::{joystick, mouse, network, nortsong, picload};
use crate::{loudness, nortvars, palette, varz};
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

/// The item slot of `items` that upgrade-menu row `i` edits (rule R8: a
/// reference instead of C's pointer table).
pub fn playeritem_map(items: &mut PlayerItems, i: u32) -> &mut u8 {
    match i {
        0 => &mut items.ship,
        1 => &mut items.weapon[FRONT_WEAPON].id,
        2 => &mut items.weapon[REAR_WEAPON].id,
        3 => &mut items.shield,
        4 => &mut items.generator,
        5 => &mut items.sidekick[LEFT_SIDEKICK],
        6 => &mut items.sidekick[RIGHT_SIDEKICK],
        _ => panic!("playeritem_map: row {i} out of range"),
    }
}

/// `strcpy(dst, src)` into a fixed buffer.
fn strcpy(dst: &mut [u8], src: &[u8]) {
    let s = c_str(src);
    dst[..s.len()].copy_from_slice(s);
    dst[s.len()] = 0;
}

#[doc(alias = "JE_drawMenuHeader")]
pub fn je_draw_menu_header(g: &mut Game) {
    let gm = &g.game_menu;
    let src: &[u8] = match gm.cur_menu {
        m if m == i16::from(MENU_DATA_CUBE_SUB) => {
            let cube = usize::from(gm.cur_sel[usize::from(MENU_DATA_CUBES)]) - 2;
            &gm.cube[cube].header
        }
        m if m == i16::from(MENU_DATA_CUBES) => &g.helptext.menu_int[1][1],
        m if m == i16::from(MENU_LOAD_SAVE) => {
            &g.helptext.menu_int[3][usize::from(g.mainint.perform_save) + 1]
        }
        m => &g.helptext.menu_int[(m + 1) as usize][0],
    };
    let mut buf = [0u8; 18];
    let n = src.len().min(buf.len());
    buf[..n].copy_from_slice(&src[..n]);
    strcpy(&mut g.tyrian2.temp_str, &buf);
    let temp_str = g.tyrian2.temp_str;
    let font = FONT_SHAPES as u32;
    let x = 74 + fonthand::je_font_center(g, &temp_str, font);
    let screen = g.video.vga_screen_id();
    fonthand::je_d_string(g, screen, x, 10, &temp_str, font);
}

#[doc(alias = "JE_drawMenuChoices")]
pub fn je_draw_menu_choices(g: &mut Game) {
    let cur_menu = g.game_menu.cur_menu;
    let menu = cur_menu as usize;
    let mut x: u8 = 2;
    while x <= g.game_menu.menu_choices[menu] {
        let mut temp_y = 38 + (i32::from(x) - 1) * 16;

        if cur_menu == i16::from(MENU_FULL_GAME) && x == 7 {
            temp_y += 16;
        }

        if cur_menu == i16::from(MENU_2_PLAYER_ARCADE) {
            if x > 3 {
                temp_y += 16;
            }
            if x > 4 {
                temp_y += 16;
            }
        }

        if !(cur_menu == i16::from(MENU_PLAY_NEXT_LEVEL) && x == g.game_menu.menu_choices[menu]) {
            temp_y -= 16;
        }

        // '~' (highlight) plus the entry, as upstream builds it in a heap block
        let entry = c_str(&g.helptext.menu_int[menu + 1][usize::from(x) - 1]);
        let mut s = [0u8; 18 + 2];
        let start = usize::from(g.game_menu.cur_sel[menu] == x);
        s[0] = b'~';
        s[start..start + entry.len()].copy_from_slice(entry);
        s[start + entry.len()] = 0;
        let screen = g.video.vga_screen_id();
        fonthand::je_d_string(g, screen, 166, temp_y, &s, SMALL_FONT_SHAPES as u32);

        x = x.wrapping_add(1);
    }
}

/// The screen `VGAScreenSeg` points to.
fn vga_seg(g: &Game) -> ScreenId {
    g.video
        .vga_screen_seg
        .expect("VGAScreenSeg used before init_video")
}

/// Entry `i` of a planet table, for map numbers (1-based, from level data)
/// that may lie outside it: 0 there, where upstream reads outside the table
/// (`D-UB-GAMEMENU-3`).
fn planet_at<T: Copy + Default>(table: &[T], number: impl Into<usize>) -> T {
    number
        .into()
        .checked_sub(1)
        .and_then(|i| table.get(i))
        .copied()
        .unwrap_or_default()
}

/// Width and height of planet sprite `index` (0 for no sprite).
fn planet_size(g: &Game, index: i32) -> (u32, u32) {
    if !(0..sprite::SPRITES_PER_TABLE_MAX as i32).contains(&index) {
        return (0, 0);
    }
    let s = sprite::sprite(g, PLANET_SHAPES as u32, index as u32);
    (u32::from(s.width), u32::from(s.height))
}

#[doc(alias = "JE_updateNavScreen")]
pub fn je_update_nav_screen(g: &mut Game) {
    let gm = &mut g.game_menu;
    gm.temp_nav_x = gm.nav_x.round() as i16;
    gm.temp_nav_y = gm.nav_y.round() as i16;
    let screen = g.video.vga_screen_id();
    fill_rectangle_xy(g, screen, 19, 16, 135, 169, 2);
    je_draw_nav_lines(g, true);
    je_draw_nav_lines(g, false);
    je_draw_dots(g);

    for x in 0..11u8 {
        je_draw_planet(g, x);
    }

    let next = usize::from(MENU_PLAY_NEXT_LEVEL);
    let mut x: u8 = 0;
    while i32::from(x) < i32::from(g.game_menu.menu_choices[next]) - 1 {
        let planet = g.varz.map_planet[usize::from(x)];
        if planet > 11 {
            je_draw_planet(g, planet - 1);
        }
        x = x.wrapping_add(1);
    }

    if g.varz.map_origin > 11 {
        je_draw_planet(g, (g.varz.map_origin - 1) as u8);
    }

    let seg = vga_seg(g);
    blit_sprite(g, seg, 0, 0, OPTION_SHAPES as u32, 28); // navigation screen interface

    let cur = g.game_menu.cur_sel[next];
    if cur < g.game_menu.menu_choices[next] {
        let origin = usize::from(g.varz.map_origin);
        let dest = usize::from(g.varz.map_planet[usize::from(cur) - 2]);
        let (ow, oh) = planet_size(g, i32::from(planet_at(&PGR, origin)) - 1);
        let (dw, dh) = planet_size(g, i32::from(planet_at(&PGR, dest)) - 1);
        let (ox, oy, dx, dy) = (ow / 2, oh / 2, dw / 2, dh / 2);

        // unsigned arithmetic, as in C (int minus unsigned)
        let nx = u32::from(planet_at(&PLANET_X, origin))
            .wrapping_sub(ox)
            .wrapping_add(u32::from(planet_at(&PLANET_X, dest)))
            .wrapping_sub(dx);
        let ny = u32::from(planet_at(&PLANET_Y, origin))
            .wrapping_sub(oy)
            .wrapping_add(u32::from(planet_at(&PLANET_Y, dest)))
            .wrapping_sub(dy);
        g.game_menu.new_nav_x = nx as f32 / 2.0;
        g.game_menu.new_nav_y = ny as f32 / 2.0;
    }

    let gm = &mut g.game_menu;
    gm.nav_x = gm.nav_x + (gm.new_nav_x - gm.nav_x) / 2.0;
    gm.nav_y = gm.nav_y + (gm.new_nav_y - gm.nav_y) / 2.0;

    if (gm.new_nav_x - gm.nav_x).abs() < 1.0 {
        gm.nav_x = gm.new_nav_x;
    }
    if (gm.new_nav_y - gm.nav_y).abs() < 1.0 {
        gm.nav_y = gm.new_nav_y;
    }

    fill_rectangle_xy(g, screen, 314, 0, 319, 199, 230);

    let gm = &mut g.game_menu;
    if gm.planet_ani_wait > 0 {
        gm.planet_ani_wait -= 1;
    } else {
        gm.planet_ani = gm.planet_ani.wrapping_add(1);
        if gm.planet_ani > 14 {
            gm.planet_ani = 0;
        }
        gm.planet_ani_wait = 3;
    }

    if gm.current_dot_wait > 0 {
        gm.current_dot_wait -= 1;
    } else {
        if gm.current_dot_num < gm.planet_dots[usize::from(gm.cur_sel[next]) - 2] {
            gm.current_dot_num += 1;
        }
        gm.current_dot_wait = 5;
    }
}

/// The grid of the ship-specs screen (and its dark shadow when `dark`).
#[doc(alias = "JE_drawLines")]
pub fn je_draw_lines(g: &mut Game, surface: ScreenId, dark: bool) {
    let temp_x2: i16 = -10;
    let temp_y2: i16 = 0;

    let mut temp_w: u16 = 0;
    for _ in 0..20 {
        temp_w = temp_w.wrapping_add(15);
        let temp_x = (i32::from(temp_w) - i32::from(temp_x2)) as i16;
        let tx = i32::from(temp_x);

        if tx > 18 && tx < 135 {
            if dark {
                je_rectangle(g, surface, tx + 1, 0, tx + 1, 199, 32 + 3);
            } else {
                je_rectangle(g, surface, tx, 0, tx, 199, 32 + 5);
            }
        }
    }

    temp_w = 0;
    for _ in 0..20 {
        temp_w = temp_w.wrapping_add(15);
        let temp_y = (i32::from(temp_w) - i32::from(temp_y2)) as i16;
        let ty = i32::from(temp_y);

        if ty > 15 && ty < 169 {
            if dark {
                je_rectangle(g, surface, 0, ty + 1, 319, ty + 1, 32 + 3);
            } else {
                je_rectangle(g, surface, 0, ty, 319, ty, 32 + 5);
            }

            let mut temp_w2: u16 = 0;
            for _ in 0..20 {
                temp_w2 = temp_w2.wrapping_add(15);
                let tx = i32::from((i32::from(temp_w2) - i32::from(temp_x2)) as i16);
                if tx > 18 && tx < 135 {
                    je_pix3(g, surface, tx, ty, 32 + 6);
                }
            }
        }
    }
}

/// The navigation screen's grid, scrolled with the view (and its dark
/// shadow when `dark`).
#[doc(alias = "JE_drawNavLines")]
pub fn je_draw_nav_lines(g: &mut Game, dark: bool) {
    let screen = g.video.vga_screen_id();
    let temp_x2 = g.game_menu.temp_nav_x >> 1;
    let temp_y2 = g.game_menu.temp_nav_y >> 1;

    let mut temp_w: u16 = 0;
    for _ in 1..=20 {
        temp_w = temp_w.wrapping_add(15);
        let tx = i32::from((i32::from(temp_w) - i32::from(temp_x2)) as i16);

        if tx > 18 && tx < 135 {
            if dark {
                je_rectangle(g, screen, tx + 1, 16, tx + 1, 169, 1);
            } else {
                je_rectangle(g, screen, tx, 16, tx, 169, 5);
            }
        }
    }

    temp_w = 0;
    for _ in 1..=20 {
        temp_w = temp_w.wrapping_add(15);
        let ty = i32::from((i32::from(temp_w) - i32::from(temp_y2)) as i16);

        if ty > 15 && ty < 169 {
            if dark {
                je_rectangle(g, screen, 19, ty + 1, 135, ty + 1, 1);
            } else {
                je_rectangle(g, screen, 8, ty, 160, ty, 5);
            }

            let mut temp_w2: u16 = 0;
            for _ in 0..20 {
                temp_w2 = temp_w2.wrapping_add(15);
                let tx = i32::from((i32::from(temp_w2) - i32::from(temp_x2)) as i16);
                if tx > 18 && tx < 135 {
                    je_pix3(g, screen, tx, ty, 7);
                }
            }
        }
    }
}

#[doc(alias = "JE_drawDots")]
pub fn je_draw_dots(g: &mut Game) {
    let seg = vga_seg(g);
    let mut x: u8 = 0;
    while u16::from(x) < g.varz.map_p_num {
        let mut y: u8 = 0;
        while y < g.game_menu.planet_dots[usize::from(x)] {
            let gm = &g.game_menu;
            let (xi, yi) = (usize::from(x), usize::from(y));
            let tx =
                (i32::from(gm.planet_dot_x[xi][yi]) - i32::from(gm.temp_nav_x) + 66 - 2) as i16;
            let ty =
                (i32::from(gm.planet_dot_y[xi][yi]) - i32::from(gm.temp_nav_y) + 85 - 2) as i16;
            let (tx, ty) = (i32::from(tx), i32::from(ty));
            if tx > 0 && tx < 140 && ty > 0 && ty < 168 {
                let current = i32::from(x)
                    == i32::from(gm.cur_sel[usize::from(MENU_PLAY_NEXT_LEVEL)]) - 2
                    && y < gm.current_dot_num;
                let dot = if current { 30 } else { 29 };
                blit_sprite(g, seg, tx, ty, OPTION_SHAPES as u32, dot); // navigation dots
            }
            y = y.wrapping_add(1);
        }
        x = x.wrapping_add(1);
    }
}

#[doc(alias = "JE_drawPlanet")]
pub fn je_draw_planet(g: &mut Game, planet_num: u8) {
    let p = usize::from(planet_num);
    if p >= PGR.len() {
        // a map number past the planets (`D-UB-GAMEMENU-3`)
        return;
    }
    let mut temp_z = (i32::from(PGR[p]) - 1) as i16;
    let (w, h) = planet_size(g, i32::from(temp_z));
    let gm = &g.game_menu;
    // `Uint16` sizes promote to `int`: all `int` arithmetic, stored as JE_integer
    let (w, h) = (w as i32, h as i32);
    let temp_x = i32::from((i32::from(PLANET_X[p]) + 66 - i32::from(gm.temp_nav_x) - w / 2) as i16);
    let temp_y = i32::from((i32::from(PLANET_Y[p]) + 85 - i32::from(gm.temp_nav_y) - h / 2) as i16);

    if temp_x > -7 && temp_x + w < 170 && temp_y > 0 && temp_y < 160 {
        if P_ANI[p] != 0 {
            temp_z = (i32::from(temp_z) + i32::from(gm.planet_ani)) as i16;
        }

        let seg = vga_seg(g);
        let z = i32::from(temp_z) as u32;
        let table = PLANET_SHAPES as u32;
        blit_sprite_dark(g, seg, temp_x + 3, temp_y + 3, table, z, false);
        blit_sprite(g, seg, temp_x, temp_y, table, z); // planets
    }
}

/// Scales all of `src_bitmap` into the box (`x1`, `y1`)-(`x2`, `y2`) of
/// `dst_bitmap` (nearest neighbour, accumulated in `f32` as upstream does).
#[doc(alias = "JE_scaleBitmap")]
pub fn je_scale_bitmap(
    g: &mut Game,
    dst_bitmap: ScreenId,
    src_bitmap: ScreenId,
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
) {
    let (dst_s, src_s) = g.video.screen_pair_mut(dst_bitmap, src_bitmap);
    let w = x2 - x1 + 1;
    let h = y2 - y1 + 1;
    let base_skip_w = src_s.pitch as f32 / w as f32;
    let base_skip_h = src_s.h as f32 / h as f32;

    let mut dst = (y1 * dst_s.pitch + x1) as usize;
    let mut cumulative_skip_h: f32 = 0.0;

    for _ in 0..h {
        // the start of the source line (upstream steps by `w`, not `pitch`)
        let src_w = src_s.w as usize * cumulative_skip_h as u32 as usize;
        let mut src = src_w;
        cumulative_skip_h += base_skip_h;
        let mut cumulative_skip_w: f32 = 0.0;

        for _ in 0..w {
            dst_s.pixels[dst] = src_s.pixels[src];
            dst += 1;

            cumulative_skip_w += base_skip_w;
            src = src_w + cumulative_skip_w as u32 as usize;
        }

        dst = (dst as i64 + i64::from(dst_s.pitch - w)) as usize;
    }
}

#[doc(alias = "JE_initWeaponView")]
pub fn je_init_weapon_view(g: &mut Game) {
    let screen = g.video.vga_screen_id();
    fill_rectangle_xy(g, screen, 8, 8, 144, 177, 0);

    let p = &mut g.player.player[0];
    p.sidekick[LEFT_SIDEKICK].x = 72 - 15;
    p.sidekick[LEFT_SIDEKICK].y = 120;
    p.sidekick[RIGHT_SIDEKICK].x = 72 + 15;
    p.sidekick[RIGHT_SIDEKICK].y = 120;

    p.x = 72;
    p.y = 110;
    p.delta_x_shot_move = 0;
    p.delta_y_shot_move = 0;
    p.last_x_explosion_follow = 72;
    p.last_y_explosion_follow = 110;
    g.config.power = 500;
    g.config.last_power = 500;

    g.shots.shot_avail.fill(0);

    g.config.shot_repeat.fill(1);
    g.config.shot_multi_pos.fill(0);

    initialize_starfield(g);
}

/// The dots of the route from the current planet to each destination.
#[doc(alias = "JE_computeDots")]
pub fn je_compute_dots(g: &mut Game) {
    let origin = usize::from(g.varz.map_origin);
    let (ox, oy) = (planet_at(&PLANET_X, origin), planet_at(&PLANET_Y, origin));
    let mut x: u8 = 0;
    while u16::from(x) < g.varz.map_p_num {
        let xi = usize::from(x);
        let dest = usize::from(g.varz.map_planet[xi]);
        let (dx, dy) = (planet_at(&PLANET_X, dest), planet_at(&PLANET_Y, dest));
        let dist_x = i32::from(dx) - i32::from(ox);
        let dist_y = i32::from(dy) - i32::from(oy);
        let temp_x = (dist_x.abs() + dist_y.abs()) as i16;

        let gm = &mut g.game_menu;
        gm.planet_dots[xi] = if temp_x != 0 {
            let d = (dist_x * dist_x + dist_y * dist_y) as f32;
            (d.sqrt().sqrt().round() - 1.0) as u8
        } else {
            0
        };

        if gm.planet_dots[xi] > 10 {
            gm.planet_dots[xi] = 10;
        }

        let dots = gm.planet_dots[xi];
        for y in 0..dots {
            let tx = je_part_way(ox as i16, dx as i16, dots, y);
            let ty = je_part_way(oy as i16, dy as i16, dots, y);
            gm.planet_dot_x[xi][usize::from(y)] = tx;
            gm.planet_dot_y[xi][usize::from(y)] = ty;
        }
        x = x.wrapping_add(1);
    }
}

/// The position of dot `dist` of `dots` between `start` and `finish`.
#[doc(alias = "JE_partWay")]
pub fn je_part_way(start: i16, finish: i16, dots: u8, dist: u8) -> i16 {
    let (start, finish) = (i32::from(start), i32::from(finish));
    ((finish - start) / (i32::from(dots) + 2) * (i32::from(dist) + 1) + start) as i16
}

/// The ship specs screen: drawn on `game_screen`, scaled in onto `VGAScreen`,
/// shown until input.
#[doc(alias = "JE_doShipSpecs")]
pub fn je_do_ship_specs(g: &mut Game) -> Flow {
    let game_screen = g
        .video
        .game_screen
        .expect("game_screen used before init_video");
    let vga2 = g
        .video
        .vga_screen2
        .expect("VGAScreen2 used before init_video");

    je_draw_ship_specs(g, game_screen, vga2);

    // restore VGAScreen2, which the drawing used as scratch
    picload::je_load_pic(g, vga2, 1, false)?;

    nortsong::je_play_sample_num(g, S_SPRING);
    let vga = g.video.vga_screen_id();
    je_scale_in_picture(g, vga, game_screen)?;

    keyboard::wait_until_get_input(g)
}

#[doc(alias = "JE_drawMainMenuHelpText")]
pub fn je_draw_main_menu_help_text(g: &mut Game) {
    const HELP: [usize; 16] = [
        15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 24, 11,
    ];

    let gm = &g.game_menu;
    let cur_menu = gm.cur_menu;
    let menu = cur_menu as usize;
    let temp = gm.cur_sel[menu].wrapping_sub(2);
    let row = if cur_menu == i16::from(MENU_JOYSTICK_CONFIG) {
        HELP[usize::from(gm.cur_sel[menu]) - 2]
    } else if cur_menu < i16::from(MENU_PLAY_NEXT_LEVEL)
        || cur_menu == i16::from(MENU_2_PLAYER_ARCADE)
        || cur_menu > i16::from(MENU_1_PLAYER_ARCADE)
    {
        usize::from(MENU_HELP[menu][usize::from(temp)]) - 1
    } else if cur_menu == i16::from(MENU_KEYBOARD_CONFIG)
        && gm.cur_sel[usize::from(MENU_KEYBOARD_CONFIG)] == 10
    {
        25 - 1
    } else if gm.left_power || gm.right_power {
        24 - 1
    } else if i32::from(temp) == i32::from(gm.menu_choices[menu]) - 2
        || (cur_menu == i16::from(MENU_DATA_CUBES) && g.config.cube_max == 0)
    {
        12 - 1
    } else {
        (17 + i32::from(cur_menu) - 3) as usize
    };
    // Upstream copies 67 bytes from a 66-byte row; the 67th is past the
    // row's NUL and never drawn.
    let temp_str = g.helptext.main_menu_help[row];

    let screen = g.video.vga_screen_id();
    fonthand::je_text_shade(g, screen, 10, 187, &temp_str, 14, 1, fonthand::DARKEN);
}

/// The "quit?" dialog; `true` when quitting was chosen.
#[doc(alias = "JE_quitRequest")]
pub fn je_quit_request(g: &mut Game) -> Flow<bool> {
    const SDL_SCANCODE_TAB: u16 = 43;
    const SDL_SCANCODE_SPACE: u16 = 44;
    const SDL_SCANCODE_ESCAPE: u16 = 41;
    const SDL_SCANCODE_RIGHT: u16 = 79;
    const SDL_SCANCODE_LEFT: u16 = 80;

    let mut quit_selected = true;
    let mut done = false;
    let screen = g.video.vga_screen_id();

    je_bar_shade(g, screen, 65, 55, 255, 155);

    while !done {
        let mut col: u8 = 8;
        let mut col_c: i32 = 1;

        loop {
            nortsong::set_frame_count(g, 4);

            blit_sprite(g, screen, 50, 50, OPTION_SHAPES as u32, 35); // message box
            let text = g.helptext.misc_text;
            fonthand::je_text_shade(g, screen, 70, 66, &text[28], 0, 5, fonthand::FULL_SHADE);
            helptext::je_help_box(
                g,
                screen,
                70,
                90,
                &text[30],
                30,
                7,
                12,
                1,
                fonthand::FULL_SHADE as u8,
            );

            col = (i32::from(col) + col_c) as u8;
            if !(2..=8).contains(&col) {
                col_c = -col_c;
            }

            let font = FONT_SHAPES as u32;
            let temp_x = 54 + 45 - (fonthand::je_text_width(g, &text[9], font) / 2);
            let temp_c = if quit_selected {
                i32::from(col) - 12
            } else {
                -5
            };

            fonthand::je_out_text_adjust(g, screen, temp_x, 128, &text[9], 15, temp_c, font, true);

            let temp_x = 149 + 45 - (fonthand::je_text_width(g, &text[10], font) / 2);
            let temp_c = if !quit_selected {
                i32::from(col) - 12
            } else {
                -5
            };

            fonthand::je_out_text_adjust(g, screen, temp_x, 128, &text[10], 15, temp_c, font, true);

            mouse::je_mouse_start(g);
            video::je_show_vga(g);
            mouse::je_mouse_replace(g);

            keyboard::wait_until_elapsed(g)?;

            if keyboard::has_input(g, INPUT_NO_MOTION) {
                break;
            }
        }

        let mut mouse_input = MouseInput::default();
        let mut keyboard_input = KeyboardInput::default();

        if keyboard::mouse_get_input(g, INPUT_NO_MOTION, Some(&mut mouse_input)) {
            if mouse_input.y > 123 && mouse_input.y < 149 {
                if mouse_input.x > 56 && mouse_input.x < 142 {
                    quit_selected = true;
                    done = true;
                } else if mouse_input.x > 151 && mouse_input.x < 237 {
                    quit_selected = false;
                    done = true;
                }
            }
        } else if keyboard::keyboard_get_input(g, Some(&mut keyboard_input)) {
            match keyboard_input.scancode {
                SDL_SCANCODE_LEFT | SDL_SCANCODE_RIGHT | SDL_SCANCODE_TAB => {
                    quit_selected = !quit_selected;
                    nortsong::je_play_sample_num(g, S_CURSOR);
                }
                SDL_SCANCODE_RETURN | SDL_SCANCODE_SPACE => done = true,
                SDL_SCANCODE_ESCAPE => {
                    quit_selected = false;
                    done = true;
                }
                _ => {}
            }
        }
    }

    nortsong::je_play_sample_num(g, if quit_selected { S_SPRING } else { S_CLICK });

    if g.network.is_network_game && quit_selected {
        network::network_prepare(g, network::PACKET_QUIT);
        network::network_send(g, 4); // PACKET QUIT

        network::network_tyrian_halt(g, 0, true)?;
    }

    Ok(quit_selected)
}

/// `strcpy(menuInt[menu][row], src)`: like upstream, a string longer than a
/// row continues into the following rows of `menuInt` (one flat array).
fn menu_int_strcpy(g: &mut Game, menu: usize, row: usize, src: &[u8]) {
    let flat = g.helptext.menu_int.as_flattened_mut().as_flattened_mut();
    let start = (menu * 11 + row) * 18;
    let s = c_str(src);
    flat[start..start + s.len()].copy_from_slice(s);
    flat[start + s.len()] = 0;
}

/// Fills the upgrade submenu with the items available for upgrade-menu row
/// `item_num` and selects the one the player has.
#[doc(alias = "JE_genItemMenu")]
pub fn je_gen_item_menu(g: &mut Game, item_num: u8) {
    let slot = usize::from(ITEM_AVAIL_MAP[usize::from(item_num) - 2]) - 1;
    let avail_max = g.tyrian2.item_avail_max[slot];
    g.game_menu.menu_choices[usize::from(MENU_UPGRADE_SUB)] = avail_max.wrapping_add(2);

    g.varz.temp3 = 2;
    g.varz.temp2 = *playeritem_map(&mut g.player.player[0].items, u32::from(item_num) - 2);

    let title = g.helptext.menu_int[2][usize::from(item_num) - 1];
    menu_int_strcpy(g, 5, 0, &title);

    g.varz.temp_w = 0;
    while g.varz.temp_w < u16::from(g.tyrian2.item_avail_max[slot]) {
        let tw = usize::from(g.varz.temp_w);
        g.varz.temp = avail(g, slot, tw as i32);
        let temp = usize::from(g.varz.temp);
        let e = &g.episodes;
        // item numbers past their table: no name (`D-UB-GAMEMENU-2`)
        let none = [0u8; 31];
        let name = match item_num {
            2 => Some(e.ships.get(temp).map_or(none, |x| x.name)),
            3 | 4 => Some(e.weapon_port.get(temp).map_or(none, |x| x.name)),
            5 => Some(e.shields.get(temp).map_or(none, |x| x.name)),
            6 => Some(e.power_sys.get(temp).map_or(none, |x| x.name)),
            7 | 8 => Some(e.options.get(temp).map_or(none, |x| x.name)),
            _ => None,
        };
        if let Some(name) = name {
            strcpy(&mut g.tyrian2.temp_str, &name);
        }
        if g.varz.temp == g.varz.temp2 {
            g.varz.temp3 = (g.varz.temp_w + 2) as u8;
        }
        let temp_str = g.tyrian2.temp_str;
        menu_int_strcpy(g, 5, tw, &temp_str);
        g.varz.temp_w += 1;
    }

    let back = g.helptext.misc_text[13];
    menu_int_strcpy(g, 5, usize::from(g.varz.temp_w), &back);

    g.game_menu.cur_sel[usize::from(MENU_UPGRADE_SUB)] = g.varz.temp3;
}

/// Zooms `src` in onto `dst` from the center, then shows all of it; input
/// skips the zoom.
#[doc(alias = "JE_scaleInPicture")]
pub fn je_scale_in_picture(g: &mut Game, dst: ScreenId, src: ScreenId) -> Flow {
    let mut i = 2;
    while i < 160 {
        let y2 = (100.0 + (i as f32 * 0.625).round() - 1.0) as i32;
        je_scale_bitmap(g, dst, src, 160 - i, 0, 160 + i - 1, y2);
        video::je_show_vga(g);

        g.platform.delay(1);

        joystick::push_joysticks_as_keyboard(g);
        keyboard::handle_sdl_events(g)?;

        if keyboard::get_input(g) {
            break;
        }
        i += 2;
    }

    let (d, s) = g.video.screen_pair_mut(dst, src);
    upper_blit(s, None, d, None);
    video::je_show_vga(g);
    Ok(())
}

/// The ship name and description, the grid, and the ship drawn green, on
/// `screen` (`temp_screen` is scratch).
#[doc(alias = "JE_drawShipSpecs")]
pub fn je_draw_ship_specs(g: &mut Game, screen: ScreenId, temp_screen: ScreenId) {
    video::je_clr256(g, screen);
    je_draw_lines(g, screen, true);
    je_draw_lines(g, screen, false);
    je_rectangle(g, screen, 0, 0, 319, 199, 37);
    je_rectangle(g, screen, 1, 1, 318, 198, 35);

    let ship = g.player.player[0].items.ship;
    let name = g
        .episodes
        .ships
        .get(usize::from(ship))
        .map_or([0; 31], |s| s.name);
    fonthand::je_out_text(g, screen, 10, 2, &name, 12, 3);
    // Ship 0 (or any ship without a description) reads outside `shipInfo`
    // upstream; it has no text here (`D-UB-GAMEMENU-1`).
    let info = usize::from(ship)
        .checked_sub(1)
        .and_then(|i| g.helptext.ship_info.get(i))
        .copied()
        .unwrap_or([[0; 256]; 2]);
    let full = fonthand::FULL_SHADE as u8;
    helptext::je_help_box(g, screen, 100, 20, &info[0], 40, 9, 12, 1, full);
    helptext::je_help_box(g, screen, 100, 100, &info[1], 40, 9, 12, 1, full);

    let footer = g.helptext.misc_text[4];
    let x = fonthand::je_font_center(g, &footer, TINY_FONT as u32);
    fonthand::je_out_text(g, screen, x, 190, &footer, 12, 2);

    // position of the ship illustration
    let big = |id: u8| {
        g.episodes
            .ships
            .get(usize::from(id))
            .map_or(0, |s| i32::from(s.bigshipgraphic))
    };
    let temp_index = if ship > 90 {
        32
    } else if ship > 0 {
        big(ship)
    } else {
        big(g.game_menu.old_items[0].ship)
    };

    let (mut temp_x, temp_y) = match temp_index {
        32 => (35, 33),
        28 => (31, 36),
        33 => (31, 35),
        _ => (0, 0),
    };
    temp_x -= 30;

    // the ship, alone on the scratch screen
    video::je_clr256(g, temp_screen);
    blit_sprite(
        g,
        temp_screen,
        temp_x,
        temp_y,
        OPTION_SHAPES as u32,
        (temp_index - 1) as u32,
    ); // ship illustration

    // greenify: pixels brighter than the average of their neighbours
    let (dst, src) = g.video.screen_pair_mut(screen, temp_screen);
    let (pitch, h) = (dst.pitch as usize, dst.h as usize);
    for y in 0..h {
        for x in 0..pitch {
            let i = y * pitch + x;
            let mut avg = 0;
            if y > 0 {
                avg += i32::from(src.pixels[i - pitch] & 0x0f);
            }
            if y < h - 1 {
                avg += i32::from(src.pixels[i + pitch] & 0x0f);
            }
            if x > 0 {
                avg += i32::from(src.pixels[i - 1] & 0x0f);
            }
            if x < pitch - 1 {
                avg += i32::from(src.pixels[i + 1] & 0x0f);
            }
            avg /= 4;

            if i32::from(src.pixels[i] & 0x0f) > avg {
                dst.pixels[i] = (src.pixels[i] & 0x0f) | 0xc0;
            }
        }
    }
}

/// One frame of the upgrade screen's weapon simulator: starfield, shots
/// from the ship and sidekicks, the interface and the power bar.
#[doc(alias = "JE_weaponViewFrame")]
pub fn je_weapon_view_frame(g: &mut Game) {
    let screen = g.video.vga_screen_id();
    fill_rectangle_xy(g, screen, 8, 8, 143, 182, 0);

    update_and_draw_starfield(g, screen, 1);

    let p = &mut g.player.player[0];
    p.mouse_x = p.x as u16;
    p.mouse_y = p.y as u16;

    // shots of the weapon simulator
    for i in 0..2u32 {
        let iu = i as usize;
        if g.config.shot_repeat[iu] > 0 {
            g.config.shot_repeat[iu] -= 1;
        } else {
            let p = &g.player.player[0];
            let item = u32::from(p.items.weapon[iu].id);
            let item_power = u32::from(p.items.weapon[iu].power).wrapping_sub(1);
            let item_mode = if iu == REAR_WEAPON {
                p.weapon_mode.wrapping_sub(1)
            } else {
                0
            };
            let wp =
                g.episodes.weapon_port[item as usize].op[item_mode as usize][item_power as usize];
            let (x, y, mx, my) = (p.x as u16, p.y as u16, p.mouse_x, p.mouse_y);

            g.varz.b = player_shot_create(g, item as u16, i, x, y, mx, my, wp, 1);
        }
    }

    let sidekick_shot = |g: &mut Game, side: usize, bay: usize| {
        if g.config.shot_repeat[bay] > 0 {
            g.config.shot_repeat[bay] -= 1;
        } else {
            let p = &g.player.player[0];
            let item = usize::from(p.items.sidekick[side]);
            let (x, y) = (p.sidekick[side].x as u16, p.sidekick[side].y as u16);
            let (mx, my) = (p.mouse_x, p.mouse_y);
            let o = &g.episodes.options[item];
            let (wport, wpnum) = (u16::from(o.wport), o.wpnum);

            g.varz.b = player_shot_create(g, wport, bay as u32, x, y, mx, my, wpnum, 1);
        }
    };

    let left = usize::from(g.player.player[0].items.sidekick[LEFT_SIDEKICK]);
    if g.episodes.options[left].wport > 0 {
        sidekick_shot(g, LEFT_SIDEKICK, SHOT_LEFT_SIDEKICK);
    }

    let right = usize::from(g.player.player[0].items.sidekick[RIGHT_SIDEKICK]);
    let p = &mut g.player.player[0];
    if g.episodes.options[right].tr == 2 {
        p.sidekick[RIGHT_SIDEKICK].x = p.x;
        p.sidekick[RIGHT_SIDEKICK].y = 10.max(p.y - 20);
    } else {
        p.sidekick[RIGHT_SIDEKICK].x = 72 + 15;
        p.sidekick[RIGHT_SIDEKICK].y = 120;
    }

    if g.episodes.options[right].wport > 0 {
        sidekick_shot(g, RIGHT_SIDEKICK, SHOT_RIGHT_SIDEKICK);
    }

    simulate_player_shots(g);

    let seg = vga_seg(g);
    blit_sprite(g, seg, 0, 0, OPTION_SHAPES as u32, 12); // upgrade interface

    // power bar
    let c = &mut g.config;
    c.power = c.power.wrapping_add(c.power_add);
    if c.power > 900 {
        c.power = 900;
    }

    let power = c.power;
    g.varz.temp = (power / 10) as u8;

    g.varz.temp = (147 - i32::from(g.varz.temp)) as u8;
    while g.varz.temp <= 146 {
        let v = &mut g.varz;
        let temp = v.temp;
        v.temp2 = (113 + (146 - i32::from(temp)) / 9 + 2) as u8;
        v.temp3 = temp.wrapping_add(1) % 6;
        if v.temp3 == 1 {
            v.temp2 = v.temp2.wrapping_add(3);
        } else if v.temp3 != 0 {
            v.temp2 = v.temp2.wrapping_add(2);
        }

        let (t, t2) = (i32::from(temp), i32::from(v.temp2));
        je_pix(g, screen, 141, t, (t2 - 3) as u8);
        je_pix(g, screen, 142, t, (t2 - 3) as u8);
        je_pix(g, screen, 143, t, (t2 - 2) as u8);
        je_pix(g, screen, 144, t, (t2 - 1) as u8);
        fill_rectangle_xy(g, screen, 145, t, 149, t, t2 as u8);

        let v = &mut g.varz;
        if t2 - 3 < 112 {
            v.temp2 = v.temp2.wrapping_add(1);
        }
        v.temp = v.temp.wrapping_add(1);
    }

    let power = g.config.power;
    let v = &mut g.varz;
    v.temp = (147 - (power / 10) as i32) as u8;
    v.temp2 = (113 + (146 - i32::from(v.temp)) / 9 + 4) as u8;

    let (t, t2) = (i32::from(v.temp), i32::from(v.temp2));
    je_pix(g, screen, 141, t - 1, (t2 - 1) as u8);
    je_pix(g, screen, 142, t - 1, (t2 - 1) as u8);
    je_pix(g, screen, 143, t - 1, (t2 - 1) as u8);
    je_pix(g, screen, 144, t - 1, (t2 - 1) as u8);

    fill_rectangle_xy(g, screen, 145, t - 1, 149, t - 1, t2 as u8);

    g.config.last_power = u32::from(g.varz.temp);
}

/// The item graphic of `item_num` in the table of `item_type`, or 0 for a
/// type without one. An item number past its table (only from damaged saves
/// or demos; upstream reads outside the table) has none (`D-UB-GAMEMENU-2`).
fn item_graphic(g: &Game, item_type: u8, item_num: u16) -> u16 {
    let e = &g.episodes;
    let i = usize::from(item_num);
    match item_type {
        2 | 3 => e.weapon_port.get(i).map(|w| w.itemgraphic),
        5 => e.power_sys.get(i).map(|p| p.itemgraphic),
        6 | 7 => e.options.get(i).map(|o| o.itemgraphic),
        4 => e.shields.get(i).map(|s| s.itemgraphic),
        _ => Some(0),
    }
    .unwrap_or(0)
}

/// Draws item `item_num` of upgrade row `item_type` (1 = ship) at (`x`, `y`).
#[doc(alias = "JE_drawItem")]
pub fn je_draw_item(g: &mut Game, item_type: u8, item_num: u16, x: u16, y: u16) {
    if item_num == 0 {
        return;
    }
    let temp_w = item_graphic(g, item_type, item_num);
    let screen = g.video.vga_screen_id();
    let (x, y) = (i32::from(x), i32::from(y));

    if item_type == 1 {
        if item_num > 90 {
            let mut ptr = Some(Sprite2Ref::SpriteSheet9);
            let gr = je_s_gr(g, item_num - 90, &mut ptr);
            g.varz.ship_gr_ptr = ptr;
            g.varz.ship_gr = gr;
            let sheet = ptr.unwrap_or(Sprite2Ref::SpriteSheet9);
            blit_sprite2x2(g, screen, x, y, sheet, u32::from(gr));
        } else if let Some(ship) = g.episodes.ships.get(usize::from(item_num)) {
            // ship numbers past the table: `D-UB-GAMEMENU-2`
            let gr = ship.shipgraphic;
            blit_sprite2x2(g, screen, x, y, Sprite2Ref::SpriteSheet9, u32::from(gr));
        }
    } else if temp_w > 0 {
        blit_sprite2x2(
            g,
            screen,
            x,
            y,
            Sprite2Ref::ShopSpriteSheet,
            u32::from(temp_w),
        );
    }
}

/// `weapon_sprites`: the illustration sprite of each weapon port.
const WEAPON_SPRITES: [i32; 43] = [
    -1, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 21, 5, 13, -1, 14, 15, 0, 14, 9, 8, 2, 15, 0, 13, 0,
    8, 8, 11, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 1,
];
/// `front_weapon_xy_list`: position slot of each weapon in the front port.
const FRONT_WEAPON_XY_LIST: [i32; 43] = [
    -1, 4, 9, 3, 8, 2, 5, 10, 1, -1, -1, -1, -1, 7, 8, -1, -1, 0, -1, 4, 0, -1, -1, 3, -1, 4, -1,
    4, -1, -1, -1, 9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 9,
];
const FRONT_WEAPON_X: [i32; 12] = [59, 66, 66, 54, 61, 51, 58, 51, 61, 52, 53, 58];
const FRONT_WEAPON_Y: [i32; 12] = [38, 53, 41, 36, 48, 35, 41, 35, 53, 41, 39, 31];
/// `rear_weapon_xy_list`: position slot of each weapon in the rear port.
const REAR_WEAPON_XY_LIST: [i32; 43] = [
    -1, -1, -1, -1, -1, -1, -1, -1, -1, 0, 1, 2, 3, -1, 4, 5, -1, -1, 6, -1, -1, 1, 0, -1, 6, -1,
    5, -1, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -1, -1,
];
const REAR_WEAPON_X: [i32; 7] = [41, 27, 49, 43, 51, 39, 41];
const REAR_WEAPON_Y: [i32; 7] = [92, 92, 113, 102, 97, 96, 76];

/// The weapon illustration at slot `list[id]` of the position tables, if
/// upstream reads inside them (`D-UB-GAMEMENU-2` otherwise).
fn weapon_spot(list: &[i32; 43], xs: &[i32], ys: &[i32], id: u8) -> Option<(i32, i32, u32)> {
    let id = usize::from(id);
    let slot = usize::try_from(*list.get(id)?).ok()?;
    Some((*xs.get(slot)?, *ys.get(slot)?, WEAPON_SPRITES[id] as u32))
}

/// The upgrade screen's picture of the player's ship with its generator,
/// weapons, sidekicks and shield. Parts whose item numbers lie outside the
/// illustration tables (only from damaged saves or demos, where upstream
/// reads outside them) are not drawn (`D-UB-GAMEMENU-2`).
pub fn draw_ship_illustration(g: &mut Game) {
    let seg = vga_seg(g);
    let items = g.player.player[0].items;

    // ship
    {
        let sprite_id = match g.episodes.ships.get(usize::from(items.ship)) {
            Some(s) => i32::from(s.bigshipgraphic) - 1,
            None => 31, // custom ships get a default
        };

        const SHIP_X: [i32; 6] = [31, 0, 0, 0, 35, 31];
        const SHIP_Y: [i32; 6] = [36, 0, 0, 0, 33, 35];

        if let Ok(k) = usize::try_from(sprite_id - 27)
            && k < SHIP_X.len()
        {
            blit_sprite(
                g,
                seg,
                SHIP_X[k],
                SHIP_Y[k],
                OPTION_SHAPES as u32,
                sprite_id as u32,
            );
        }
    }

    // generator
    {
        let gen_ = i32::from(items.generator);
        // generators 1 and 2 share a sprite
        let sprite_id = if gen_ == 1 { gen_ + 15 } else { gen_ + 14 };

        const GENERATOR_X: [i32; 5] = [62, 64, 67, 66, 63];
        const GENERATOR_Y: [i32; 5] = [84, 85, 86, 84, 97];
        if let Ok(k) = usize::try_from(sprite_id - 16)
            && k < GENERATOR_X.len()
        {
            let (x, y) = (GENERATOR_X[k], GENERATOR_Y[k]);
            blit_sprite(g, seg, x, y, WEAPON_SHAPES as u32, sprite_id as u32);
        }
    }

    // front weapon
    let front = items.weapon[FRONT_WEAPON].id;
    if front > 0
        && let Some((x, y, s)) = weapon_spot(
            &FRONT_WEAPON_XY_LIST,
            &FRONT_WEAPON_X,
            &FRONT_WEAPON_Y,
            front,
        )
    {
        blit_sprite(g, seg, x, y, WEAPON_SHAPES as u32, s); // ship illustration: front weapon
    }

    // rear weapon
    let rear = items.weapon[REAR_WEAPON].id;
    if rear > 0
        && let Some((x, y, s)) =
            weapon_spot(&REAR_WEAPON_XY_LIST, &REAR_WEAPON_X, &REAR_WEAPON_Y, rear)
    {
        blit_sprite(g, seg, x, y, WEAPON_SHAPES as u32, s);
    }

    // sidekicks
    je_draw_item(g, 6, u16::from(items.sidekick[LEFT_SIDEKICK]), 3, 84);
    je_draw_item(g, 7, u16::from(items.sidekick[RIGHT_SIDEKICK]), 129, 84);

    // shield
    if let Some(s) = g.episodes.shields.get(usize::from(items.shield)) {
        let value = (i32::from(s.mpwr) - 10) as i8;
        blit_sprite_hv(g, seg, 28, 23, OPTION_SHAPES as u32, 26, 15, value);
    }
}

/// One frame of the weapon simulator with the power-level controls and the
/// player's ship.
#[doc(alias = "JE_weaponSimUpdate")]
pub fn je_weapon_sim_update(g: &mut Game) {
    je_weapon_view_frame(g);

    let screen = g.video.vga_screen_id();
    let seg = vga_seg(g);
    let gm = &g.game_menu;
    let upgrades = gm.cur_sel[usize::from(MENU_UPGRADES)];
    let sub = gm.cur_sel[usize::from(MENU_UPGRADE_SUB)];
    // front or rear weapon, not "Done", not "None"
    let weapon_row = (upgrades == 3 || upgrades == 4)
        && sub < gm.menu_choices[usize::from(MENU_UPGRADE_SUB)]
        && {
            let slot = usize::from(ITEM_AVAIL_MAP[usize::from(upgrades) - 2]) - 1;
            // a selection before the list: `D-UB-GAMEMENU-2`
            usize::from(sub)
                .checked_sub(2)
                .and_then(|i| g.tyrian2.item_avail[slot].get(i))
                .is_some_and(|&a| a != 0)
        };

    if weapon_row {
        if gm.left_power {
            let buf = format!("{}\0", g.mainint.downgrade_cost);
            fonthand::je_out_text(g, screen, 26, 137, buf.as_bytes(), 1, 4);
        } else {
            blit_sprite(g, seg, 24, 149, OPTION_SHAPES as u32, 13); // downgrade disabled
        }

        if g.game_menu.right_power {
            let buf = format!("{}\0", g.mainint.upgrade_cost);
            if !g.game_menu.right_power_afford {
                fonthand::je_out_text(g, screen, 108, 137, buf.as_bytes(), 7, 4);
                blit_sprite(g, seg, 119, 149, OPTION_SHAPES as u32, 14); // upgrade disabled
            } else {
                fonthand::je_out_text(g, screen, 108, 137, buf.as_bytes(), 1, 4);
            }
        } else {
            blit_sprite(g, seg, 119, 149, OPTION_SHAPES as u32, 14); // upgrade disabled
        }

        g.varz.temp = g.player.player[0].items.weapon[usize::from(upgrades) - 3].power;

        for x in 1..=i32::from(g.varz.temp) {
            fill_rectangle_xy(g, screen, 39 + x * 6, 151, 39 + x * 6 + 4, 151, 251);
            je_pix(g, screen, 39 + x * 6, 151, 252);
            fill_rectangle_xy(g, screen, 39 + x * 6, 152, 39 + x * 6 + 4, 164, 250);
            fill_rectangle_xy(g, screen, 39 + x * 6, 165, 39 + x * 6 + 4, 165, 249);
        }

        let buf = format!("POWER: {}\0", g.varz.temp);
        fonthand::je_out_text(g, screen, 58, 137, buf.as_bytes(), 15, 4);
    } else {
        g.game_menu.left_power = false;
        g.game_menu.right_power = false;
        blit_sprite(g, seg, 20, 146, OPTION_SHAPES as u32, 17); // hide power level interface
    }

    let p = &g.player.player[0];
    let (ship, x, y) = (p.items.ship, (p.x - 5) as u16, (p.y - 7) as u16);
    je_draw_item(g, 1, u16::from(ship), x, y);
}

/// The player's cash after buying the upgrade row's selected item and its
/// power levels (C `JE_longint`, truncated as upstream does).
#[doc(alias = "JE_cashLeft")]
pub fn je_cash_left(g: &mut Game) -> i32 {
    let upgrades = g.game_menu.cur_sel[usize::from(MENU_UPGRADES)];
    let mut temp_l = g.player.player[0].cash as i32;
    let item_num = u16::from(*playeritem_map(
        &mut g.player.player[0].items,
        u32::from(upgrades).wrapping_sub(2),
    ));

    temp_l = temp_l.wrapping_sub(je_get_cost(g, upgrades, item_num) as i32);

    g.varz.temp_w = 0;

    if upgrades == 3 || upgrades == 4 {
        let power = u32::from(g.player.player[0].items.weapon[usize::from(upgrades) - 3].power);
        let cost = u32::from(g.episodes.weapon_port[usize::from(item_num)].cost);
        for i in 1..power {
            g.varz.temp_w = g.varz.temp_w.wrapping_add(cost.wrapping_mul(i) as u16);
            temp_l = temp_l.wrapping_sub(i32::from(g.varz.temp_w));
        }
    }

    temp_l
}

/// The cash left, in the upgrade submenus.
#[doc(alias = "JE_drawScore")]
pub fn je_draw_score(g: &mut Game) {
    if g.game_menu.cur_menu == i16::from(MENU_UPGRADE_SUB) {
        let cl = format!("{}", je_cash_left(g));
        let screen = g.video.vga_screen_id();
        fonthand::je_text_shade(g, screen, 65, 173, cl.as_bytes(), 1, 6, fonthand::DARKEN);
    }
}

/// Loads the data cubes the player has collected (at most the 4 cube slots;
/// a larger count from a damaged save reads past `cube` upstream,
/// `D-UB-GAMEMENU-3`).
pub fn load_cubes(g: &mut Game) -> Flow {
    for cube_slot in 0..usize::from(g.config.cube_max).min(4) {
        g.game_menu.cube[cube_slot].text = [[0; 36]; 90];

        let index = i32::from(g.config.cube_list[cube_slot]);
        load_cube(g, cube_slot as i32, index)?;
    }
    Ok(())
}

/// `strcat(dst, src)` into a cube text line. `load_cubes` empties the lines
/// first and the wrapping keeps each within its 35 characters, so nothing is
/// cut here.
fn strcat(dst: &mut [u8], src: &[u8]) {
    let start = c_str(dst).len();
    let n = src.len().min(dst.len() - 1 - start);
    dst[start..start + n].copy_from_slice(&src[..n]);
    dst[start + n] = 0;
}

/// Loads cube `cube_index` (0-based) of the episode's cube text file into
/// slot `cube_slot`: face, title, header, and the text word-wrapped to the
/// cube display.
pub fn load_cube(g: &mut Game, cube_slot: i32, mut cube_index: i32) -> Flow<bool> {
    let name = c_str(&g.episodes.cube_filename).to_vec();
    let filename = String::from_utf8_lossy(&name).into_owned();

    let mut file = file::data_file_open(g, &filename, "rb");
    if file.error {
        let message = format!(
            "Failed to open file '{}': {}",
            filename,
            file::file_get_error(&file)
        );
        log_fatal(g, &message);
        return Err(Halt { code: EXIT_FAILURE });
    }

    let mut buf = [0u8; 256];

    // seek to the cube
    while cube_index > 0 {
        helptext::read_encrypted_string(&mut file, Some(&mut buf));

        if buf[0] == b'*' || file.error {
            cube_index -= 1;
        }
    }

    let slot = cube_slot as usize;
    let mut face = g.game_menu.cube[slot].face_sprite;
    mainint::str_pop_int(&mut buf[4..], &mut face);
    g.game_menu.cube[slot].face_sprite = face.wrapping_sub(1);

    helptext::read_encrypted_string(&mut file, Some(&mut g.game_menu.cube[slot].title));
    helptext::read_encrypted_string(&mut file, Some(&mut g.game_menu.cube[slot].header));

    let (mut line, mut line_chars, mut line_width) = (0u32, 0u32, 0u32);

    // word-wrap every line of text onto the cube's lines
    loop {
        helptext::read_encrypted_string(&mut file, Some(&mut buf));

        // end of data
        if buf[0] == b'*' || file.error {
            break;
        }

        // new paragraph
        if c_str(&buf).is_empty() {
            if line_chars == 0 {
                line += 4; // consecutive new paragraphs make a 4-line break
            } else {
                line += 1;
            }
            line_chars = 0;
            line_width = 0;

            continue;
        }

        let mut word_start = 0;
        let mut i = 0;
        loop {
            let end_of_line = buf[i] == 0;
            let end_of_word = end_of_line || buf[i] == b' ';

            if end_of_word {
                buf[i] = 0;

                let word = buf[word_start..i].to_vec();
                word_start = i + 1;

                let word_chars = word.len() as u32;
                let word_width = fonthand::je_text_width(g, &word, TINY_FONT as u32) as u32;

                // a word longer than a line: give up on this line of text
                if word_chars > CUBE_LINE_CHARS || word_width > CUBE_LINE_WIDTH {
                    break;
                }

                let mut prepend_space = true;

                line_chars += word_chars + 1;
                line_width += word_width + 6;

                // the word goes on the next line
                if line_chars > CUBE_LINE_CHARS || line_width > CUBE_LINE_WIDTH {
                    line += 1;
                    line_chars = word_chars;
                    line_width = word_width;

                    prepend_space = false;
                }

                if (line as usize) < 90 {
                    let cube = &mut g.game_menu.cube[slot];
                    if prepend_space {
                        strcat(&mut cube.text[line as usize], b" ");
                    }
                    strcat(&mut cube.text[line as usize], &word);

                    // the last line with text
                    cube.last_line = (line + 1) as i32;
                }
            }

            if end_of_line {
                break;
            }
            i += 1;
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

    Ok(true)
}

/// `quit_request` then, when confirmed, back to the title screen.
fn quit_to_title(g: &mut Game) -> Flow {
    if je_quit_request(g)? {
        g.varz.game_loaded = true;
        g.config.main_level = 0;
    }
    Ok(())
}

/// The first level section of the next map: the arcade menus' "play".
/// A map without sections (upstream reads before `mapSection`) leaves
/// level 0 (`D-UB-GAMEMENU-3`).
fn play_next_section(g: &mut Game) {
    let i = usize::from(g.varz.map_p_num).wrapping_sub(1);
    g.config.main_level = g.varz.map_section.get(i).copied().unwrap_or(0);
    g.mainint.jump_section = true;
}

/// Cycles 2-player input device `i` (`up`: forward), skipping the other
/// player's device.
fn cycle_input_device(g: &mut Game, i: usize, up: bool) {
    let joysticks = g.joystick.joysticks;
    let other = if i == 0 { 1 } else { 0 };
    loop {
        let d = &mut g.config.input_device;
        if joysticks == 0 {
            d[other] = d[i]; // swap controllers
        }
        if up {
            if i32::from(d[i]) >= 2 + joysticks {
                d[i] = 1;
            } else {
                d[i] = d[i].wrapping_add(1);
            }
        } else if d[i] <= 1 {
            d[i] = (2 + joysticks) as u8;
        } else {
            d[i] -= 1;
        }
        if d[i] != d[other] {
            break;
        }
    }
}

/// The menu after the options/joystick/keyboard menus.
fn options_menu(g: &Game) -> i16 {
    i16::from(if g.network.is_network_game {
        MENU_LIMITED_OPTIONS
    } else {
        MENU_OPTIONS
    })
}

/// Runs the action of row `select` of the current menu.
#[doc(alias = "JE_menuFunction")]
pub fn je_menu_function(g: &mut Game, select: u8) -> Flow {
    g.game_menu.col = 0;
    g.game_menu.col_c = -1;
    nortsong::je_play_sample_num(g, S_CLICK);

    let cur_menu = g.game_menu.cur_menu;
    let cur_select = u16::from(g.game_menu.cur_sel[cur_menu as usize]);

    match u8::try_from(cur_menu).unwrap_or(u8::MAX) {
        MENU_FULL_GAME => match select {
            2 => {
                // data cubes
                g.game_menu.cur_menu = i16::from(MENU_DATA_CUBES);
                g.game_menu.cur_sel[usize::from(MENU_DATA_CUBES)] = 2;
            }
            3 => je_do_ship_specs(g)?,
            4 => g.game_menu.cur_menu = i16::from(MENU_UPGRADES),
            5 => g.game_menu.cur_menu = i16::from(MENU_OPTIONS),
            6 => {
                // next level
                g.game_menu.cur_menu = i16::from(MENU_PLAY_NEXT_LEVEL);
                g.game_menu.new_pal = 18;
                je_compute_dots(g);
                let origin = usize::from(g.varz.map_origin);
                let gm = &mut g.game_menu;
                gm.nav_x = f32::from(planet_at(&PLANET_X, origin));
                gm.nav_y = f32::from(planet_at(&PLANET_Y, origin));
                gm.new_nav_x = gm.nav_x;
                gm.new_nav_y = gm.nav_y;
                let next = usize::from(MENU_PLAY_NEXT_LEVEL);
                gm.menu_choices[next] = g.varz.map_p_num.wrapping_add(2) as u8;
                gm.cur_sel[next] = 2;
                menu_int_strcpy(g, 4, 0, b"Next Level");
                let mut x: u8 = 0;
                while u16::from(x) < g.varz.map_p_num {
                    g.varz.temp = g.varz.map_planet[usize::from(x)];
                    let name = planet_at(&g.helptext.p_name, g.varz.temp);
                    menu_int_strcpy(g, 4, usize::from(x) + 1, &name);
                    x = x.wrapping_add(1);
                }
                let back = g.helptext.misc_text[5];
                menu_int_strcpy(g, 4, usize::from(x) + 1, &back);
            }
            7 => quit_to_title(g)?,
            _ => {}
        },

        MENU_UPGRADES => {
            if select == 9 {
                // done
                g.game_menu.cur_menu = i16::from(MENU_FULL_GAME);
            } else {
                // an item to upgrade
                g.game_menu.old_items[0] = g.player.player[0].items;

                g.game_menu.last_direction = 1;
                je_gen_item_menu(g, select);
                je_init_weapon_view(g);
                g.game_menu.cur_menu = i16::from(MENU_UPGRADE_SUB);
                g.game_menu.last_cur_sel = g.game_menu.cur_sel[usize::from(MENU_UPGRADE_SUB)];
                let left = je_cash_left(g);
                let p = &mut g.player.player[0];
                p.cash = p.cash.wrapping_mul(2).wrapping_sub(i64::from(left) as u64);
            }
        }

        MENU_OPTIONS => match select {
            2 | 3 => {
                g.game_menu.cur_menu = i16::from(MENU_LOAD_SAVE);
                g.mainint.perform_save = select == 3;
                g.game_menu.quik_save = false;
            }
            6 => g.game_menu.cur_menu = i16::from(MENU_JOYSTICK_CONFIG),
            7 => g.game_menu.cur_menu = i16::from(MENU_KEYBOARD_CONFIG),
            8 => g.game_menu.cur_menu = i16::from(MENU_FULL_GAME),
            _ => {}
        },

        MENU_PLAY_NEXT_LEVEL => {
            if select == g.game_menu.menu_choices[usize::from(MENU_PLAY_NEXT_LEVEL)] {
                // exit
                g.game_menu.cur_menu = i16::from(MENU_FULL_GAME);
                g.game_menu.new_pal = 1;
            } else {
                g.config.main_level = g.varz.map_section[usize::from(cur_select) - 2];
                g.mainint.jump_section = true;
            }
        }

        MENU_UPGRADE_SUB => {
            let sub = usize::from(MENU_UPGRADE_SUB);
            if g.game_menu.cur_sel[sub] < g.game_menu.menu_choices[sub] {
                // select "Done"
                g.game_menu.cur_sel[sub] = g.game_menu.menu_choices[sub];
            } else {
                // "Done" selected
                nortsong::je_play_sample_num(g, S_ITEM);

                g.player.player[0].cash = i64::from(je_cash_left(g)) as u64;
                g.game_menu.cur_menu = i16::from(MENU_UPGRADES);
            }
        }

        MENU_KEYBOARD_CONFIG => {
            if cur_select == 10 {
                // reset to defaults
                g.config.key_settings = DEFAULT_KEY_SETTINGS;
            } else if cur_select == 11 {
                // done
                g.game_menu.cur_menu = options_menu(g);
            } else {
                // change a key
                change_key(g, cur_select)?;
            }
        }

        MENU_LOAD_SAVE => {
            if cur_select == 13 {
                if g.game_menu.quik_save {
                    g.game_menu.cur_menu = i16::from(g.game_menu.old_menu);
                    g.game_menu.new_pal = g.game_menu.old_pal;
                } else {
                    g.game_menu.cur_menu = i16::from(MENU_OPTIONS);
                }
            } else {
                g.varz.temp = if g.config.two_player_mode { 11 } else { 0 };
                let slot = cur_select
                    .wrapping_sub(1)
                    .wrapping_add(u16::from(g.varz.temp)) as u8;
                mainint::je_operation(g, slot)?;
                if g.game_menu.quik_save {
                    g.game_menu.cur_menu = i16::from(g.game_menu.old_menu);
                    g.game_menu.new_pal = g.game_menu.old_pal;
                }
            }
        }

        MENU_DATA_CUBES => {
            if cur_select == u16::from(g.game_menu.menu_choices[cur_menu as usize]) {
                g.game_menu.cur_menu = i16::from(MENU_FULL_GAME);
                g.game_menu.new_pal = 1;
            } else if g.config.cube_max > 0 {
                let gm = &mut g.game_menu;
                gm.first_menu9 = true;
                gm.cur_menu = i16::from(MENU_DATA_CUBE_SUB);
                gm.y_loc = 0;
                gm.y_chg = 0;
                gm.current_cube = gm.cur_sel[usize::from(MENU_DATA_CUBES)].wrapping_sub(2);
            } else {
                g.game_menu.cur_menu = i16::from(MENU_FULL_GAME);
                g.game_menu.new_pal = 1;
            }
        }

        MENU_DATA_CUBE_SUB => g.game_menu.cur_menu = i16::from(MENU_DATA_CUBES),

        MENU_2_PLAYER_ARCADE => match g.game_menu.cur_sel[cur_menu as usize] {
            2 => play_next_section(g),
            sel @ (3 | 4) => {
                nortsong::je_play_sample_num(g, S_CURSOR);
                cycle_input_device(g, usize::from(sel - 3), true);
            }
            5 => g.game_menu.cur_menu = i16::from(MENU_OPTIONS),
            6 => quit_to_title(g)?,
            _ => {}
        },

        MENU_1_PLAYER_ARCADE => match g.game_menu.cur_sel[cur_menu as usize] {
            2 => play_next_section(g),
            3 => g.game_menu.cur_menu = options_menu(g),
            4 => quit_to_title(g)?,
            _ => {}
        },

        MENU_LIMITED_OPTIONS => match select {
            2 => g.game_menu.cur_menu = i16::from(MENU_JOYSTICK_CONFIG),
            3 => g.game_menu.cur_menu = i16::from(MENU_KEYBOARD_CONFIG),
            6 => g.game_menu.cur_menu = i16::from(MENU_1_PLAYER_ARCADE),
            _ => {}
        },

        MENU_JOYSTICK_CONFIG => {
            if g.joystick.joysticks != 0 || select == 17 {
                joystick_config_select(g, select)?;
            }
        }

        MENU_SUPER_TYRIAN => match g.game_menu.cur_sel[cur_menu as usize] {
            2 => play_next_section(g),
            3 => je_do_ship_specs(g)?,
            4 => g.game_menu.cur_menu = i16::from(MENU_OPTIONS),
            5 => {
                let quit = je_quit_request(g)?;
                if quit && g.network.is_network_game {
                    je_tyrian_halt(g, 0)?;
                }
                if quit {
                    g.varz.game_loaded = true;
                    g.config.main_level = 0;
                }
            }
            _ => {}
        },

        _ => {}
    }

    g.game_menu.old_items[0] = g.player.player[0].items;
    Ok(())
}

/// The keyboard menu's "change key": waits for a key (or a click, which
/// cancels) and assigns it to row `cur_select`, swapping with a row that
/// had it.
fn change_key(g: &mut Game, cur_select: u16) -> Flow {
    const SDL_SCANCODE_ESCAPE: u16 = 41;
    const SDL_SCANCODE_F11: u16 = 68;
    const SDL_SCANCODE_P: u16 = 19;

    let screen = g.video.vga_screen_id();
    let row = usize::from(cur_select) - 2;
    g.varz.temp2 = 254;
    let temp2 = u32::from(g.varz.temp2);
    let temp_y = 38 + (i32::from(cur_select) - 2) * 12;
    let name = g
        .platform
        .get_scancode_name(g.config.key_settings[row] as i32);
    let shade = (temp2 % 16) as i32 - 8;
    fonthand::je_text_shade(
        g,
        screen,
        236,
        temp_y,
        name.as_bytes(),
        temp2 / 16,
        shade,
        fonthand::DARKEN,
    );
    video::je_show_vga(g);

    g.game_menu.col = 248;
    g.game_menu.col_c = 1;

    let mut joy_held = g.joystick.joydown;
    loop {
        joy_held &= g.joystick.joydown;

        nortsong::set_frame_count(g, 1);

        let gm = &mut g.game_menu;
        gm.col = gm.col.wrapping_add(gm.col_c);
        if gm.col < 243 || gm.col > 248 {
            gm.col_c = gm.col_c.wrapping_mul(-1);
        }
        let col = i32::from(g.game_menu.col);
        je_rectangle(g, screen, 230, temp_y - 2, 300, temp_y + 7, col);

        video::je_show_vga(g);

        network::network_keep_alive(g)?;

        nortsong::delay_until_elapsed(g);

        joystick::poll_joysticks(g); // not push_joysticks_as_keyboard
        keyboard::handle_sdl_events(g)?;

        if keyboard::has_input(g, INPUT_NO_MOTION) || (g.joystick.joydown && !joy_held) {
            break;
        }
    }

    let mut keyboard_input = KeyboardInput::default();

    if keyboard::mouse_get_input(g, INPUT_NO_MOTION, None) {
        // cancel
    } else if keyboard::keyboard_get_input(g, Some(&mut keyboard_input)) {
        let code = u32::from(keyboard_input.scancode);
        let keys = &mut g.config.key_settings;
        // already used? then swap
        if let Some(i) = keys.iter().position(|&k| k == code) {
            keys[i] = keys[row];
        }

        if keyboard_input.scancode != SDL_SCANCODE_ESCAPE // reserved for menu
            && keyboard_input.scancode != SDL_SCANCODE_F11 // reserved for gamma
            && keyboard_input.scancode != SDL_SCANCODE_P
        // reserved for pause
        {
            nortsong::je_play_sample_num(g, S_CLICK);
            g.config.key_settings[row] = code;
        }
    }
    Ok(())
}

/// Joystick `jc` of the initialised joysticks.
fn stick(g: &mut Game, jc: i32) -> &mut Joystick {
    &mut g.joystick.joystick.as_mut().expect("joysticks initialised")[jc as usize]
}

/// The joystick menu's row `select` (with at least one joystick, or "done").
fn joystick_config_select(g: &mut Game, select: u8) -> Flow {
    let jc = g.game_menu.joystick_config;
    let joysticks = g.joystick.joysticks;
    match select {
        2 => {
            g.game_menu.joystick_config = (jc + 1) % joysticks;
        }
        3 => {
            let j = stick(g, jc);
            j.analog = !j.analog;
        }
        4 => {
            let j = stick(g, jc);
            if j.analog {
                j.sensitivity = (j.sensitivity + 1) % 11;
            }
        }
        5 => {
            let j = stick(g, jc);
            if j.analog {
                j.threshold = (j.threshold + 1) % 11;
            }
        }
        16 => joystick::reset_joystick_assignments(g, jc),
        17 => g.game_menu.cur_menu = options_menu(g),
        _ => {
            if joysticks == 0 {
                return Ok(());
            }

            let screen = g.video.vga_screen_id();
            let s = i32::from(select);
            je_rectangle(g, screen, 235, 21 + s * 8, 310, 30 + s * 8, 248);
            video::je_show_vga(g);

            let mut temp = JoystickAssignment::default();
            if joystick::detect_joystick_assignment(g, jc, &mut temp)? {
                let a = &mut stick(g, jc).assignment[usize::from(select) - 6];
                if let Some(i) = a
                    .iter()
                    .position(|x| joystick::joystick_assignment_cmp(&temp, x))
                {
                    // already assigned: unset it
                    a[i].r#type = JoystickAssignmentTypes::None;
                } else if let Some(i) = a
                    .iter()
                    .position(|x| x.r#type == JoystickAssignmentTypes::None)
                {
                    // an empty assignment: set it
                    a[i] = temp;
                } else {
                    // all set: shift them forward and set the last one
                    let n = a.len();
                    a.copy_within(1.., 0);
                    a[n - 1] = temp;
                }

                joystick::poll_joysticks(g);
            }
        }
    }
    Ok(())
}

/// `itemAvail[slot][i]` read through the flat `[9][10]` table, as C's
/// out-of-row indices do; outside the table it reads 0 (`D-UB-GAMEMENU-2`).
fn avail(g: &Game, slot: usize, i: i32) -> u8 {
    let flat = g.tyrian2.item_avail.as_flattened();
    usize::try_from(slot as i32 * 10 + i)
        .ok()
        .and_then(|k| flat.get(k))
        .copied()
        .unwrap_or(0)
}

/// Writes `itemAvail[slot][i]` through the flat table (ignored outside it).
fn set_avail(g: &mut Game, slot: usize, i: i32, v: u8) {
    let flat = g.tyrian2.item_avail.as_flattened_mut();
    if let Some(x) = usize::try_from(slot as i32 * 10 + i)
        .ok()
        .and_then(|k| flat.get_mut(k))
    {
        *x = v;
    }
}

/// `strcpy(tempStr, src)` (a 31-byte global), cut to fit.
fn set_temp_str(g: &mut Game, src: &[u8]) {
    let s = c_str(src);
    let n = s.len().min(30);
    g.tyrian2.temp_str[..n].copy_from_slice(&s[..n]);
    g.tyrian2.temp_str[n] = 0;
}

/// The locals of `JE_itemScreen` that live across its loop.
struct ItemScreen {
    quit: bool,
    /// Power levels of the listed weapons (C: 7 entries, written past when a
    /// shop lists more weapons; here room for every index the table allows).
    temp_weapon_power: [i32; 12],
    mouse_buttons_held: u8,
}

impl ItemScreen {
    /// `temp_weapon_power[i]` (0 outside the array, `D-UB-GAMEMENU-2`).
    fn power(&self, i: i32) -> i32 {
        usize::try_from(i)
            .ok()
            .and_then(|i| self.temp_weapon_power.get(i))
            .copied()
            .unwrap_or(0)
    }

    fn power_mut(&mut self, i: i32) -> Option<&mut i32> {
        usize::try_from(i)
            .ok()
            .and_then(|i| self.temp_weapon_power.get_mut(i))
    }
}

fn menu(g: &Game) -> i16 {
    g.game_menu.cur_menu
}

fn sel(g: &Game, m: u8) -> u8 {
    g.game_menu.cur_sel[usize::from(m)]
}

/// The `itemAvail` row of upgrade row `cur_sel[MENU_UPGRADES]`.
fn upgrade_slot(g: &Game) -> usize {
    usize::from(ITEM_AVAIL_MAP[usize::from(sel(g, MENU_UPGRADES)) - 2]) - 1
}

/// `JE_textShade(VGAScreen, x, y, s, shade / 16, shade % 16 - 8 - dim, DARKEN)`.
fn shade_text(g: &mut Game, x: i32, y: i32, s: &[u8], shade: u8, dim: i32) {
    let screen = g.video.vga_screen_id();
    let (bank, bright) = (u32::from(shade / 16), i32::from(shade % 16) - 8 - dim);
    fonthand::je_text_shade(g, screen, x, y, s, bank, bright, fonthand::DARKEN);
}

/// The shop and options screens between levels: draws the current menu,
/// animates it until input, and handles that input, until a level is chosen,
/// a game is loaded or the player quits.
#[doc(alias = "JE_itemScreen")]
pub fn je_item_screen(g: &mut Game) -> Flow {
    let mut s = ItemScreen {
        quit: false,
        temp_weapon_power: [0; 12],
        mouse_buttons_held: 0,
    };

    if g.sprite.shop_sprite_sheet.data.is_none() {
        sprite::je_load_comp_shapes(g, Sprite2Ref::ShopSpriteSheet, b'1')?;
    }

    load_cubes(g)?;

    g.video.vga_screen = g.video.vga_screen_seg;

    g.game_menu.menu_choices = MENU_CHOICES_DEFAULT;

    let song = u32::from(g.musmast.song_buy);
    loudness::play_song(g, song)?;

    let vga = g.video.vga_screen_id();
    picload::je_load_pic(g, vga, 1, false)?;

    g.game_menu.cur_pal = 1;
    g.game_menu.new_pal = 0;

    video::je_show_vga(g);

    let colors = g.palette.colors;
    palette::set_palette(g, &colors, 0, 255);

    let gm = &mut g.game_menu;
    gm.col = 1;
    g.varz.game_loaded = false;
    gm.cur_item_type = 1;
    gm.cursor = 1;
    gm.cur_item = 0;
    gm.cur_sel = [2; MENU_MAX];
    gm.cur_menu = i16::from(MENU_FULL_GAME);

    // add the player's items to the shops that do not list them
    for i in 0..7u32 {
        let item = *playeritem_map(&mut g.player.player[0].last_items, i);
        let row = usize::from(ITEM_AVAIL_MAP[i as usize]) - 1;
        let max = i32::from(g.tyrian2.item_avail_max[row]);

        let mut slot = 0;
        while slot < max {
            if avail(g, row, slot) == item {
                break;
            }
            slot += 1;
        }

        if slot == max {
            set_avail(g, row, slot, item);
            g.tyrian2.item_avail_max[row] = g.tyrian2.item_avail_max[row].wrapping_add(1);
        }
    }

    copy_screen(
        g,
        vga,
        g.video
            .vga_screen2
            .expect("VGAScreen2 used before init_video"),
    );

    g.game_menu.first_menu9 = false;
    g.game_menu.back_from_help = false;

    // sort the shops' items
    for x in 0..9 {
        let max = g.tyrian2.item_avail_max[x];
        if max > 1 {
            g.varz.temp = 0;
            while i32::from(g.varz.temp) < i32::from(max) - 1 {
                g.varz.temp2 = g.varz.temp;
                while g.varz.temp2 < max {
                    let (t, t2) = (i32::from(g.varz.temp), i32::from(g.varz.temp2));
                    let (a, b) = (avail(g, x, t), avail(g, x, t2));
                    if a == 0 || (a > b && b != 0) {
                        g.varz.temp3 = a;
                        set_avail(g, x, t, b);
                        set_avail(g, x, t2, g.varz.temp3);
                    }
                    g.varz.temp2 = g.varz.temp2.wrapping_add(1);
                }
                g.varz.temp = g.varz.temp.wrapping_add(1);
            }
        }
    }

    loop {
        s.quit = false;

        varz::je_get_ship_info(g);

        draw_item_screen(g, &mut s)?;

        if g.params.constant_play {
            play_next_section(g);
        } else {
            animate_item_screen(g, &mut s)?;
        }

        // the input that ended the animation
        let mut mouse_input = MouseInput::default();
        let mut keyboard_input = KeyboardInput::default();

        if keyboard::mouse_get_input(g, INPUT_NO_MOTION, Some(&mut mouse_input)) {
            item_screen_mouse(g, &mut s, mouse_input)?;
        } else if keyboard::keyboard_get_input(g, Some(&mut keyboard_input)) {
            item_screen_key(g, &mut s, keyboard_input)?;
        }

        if s.quit || g.varz.game_loaded || g.mainint.jump_section {
            break;
        }
    }

    if !s.quit && g.network.is_network_game {
        let vga = g.video.vga_screen_id();
        je_bar_shade(g, vga, 3, 3, 316, 196);
        je_bar_shade(g, vga, 1, 1, 318, 198);
        fonthand::je_d_string(
            g,
            vga,
            10,
            160,
            b"Waiting for other player.",
            SMALL_FONT_SHAPES as u32,
        );

        video::je_show_vga(g);

        network::network_prepare(g, network::PACKET_WAITING);
        network::network_send(g, 4); // PACKET_WAITING

        loop {
            nortsong::set_frame_count(g, 1);

            let waiting = g.network.packet_in[0].as_ref().is_some_and(|p| {
                p.data.len() >= 2
                    && u16::from_be_bytes([p.data[0], p.data[1]]) == network::PACKET_WAITING
            });
            if waiting {
                network::network_update(g);
                break;
            }

            network::network_update(g);

            keyboard::wait_until_elapsed(g)?;
        }

        network::network_state_reset(g);
    }

    if g.network.is_network_game {
        while !network::network_is_sync(g) {
            nortsong::set_frame_count(g, 1);

            keyboard::wait_until_elapsed(g)?;
        }
    }

    if g.varz.game_loaded {
        palette::fade_black(g, 10)?;
    }
    Ok(())
}

/// `memcpy(dst->pixels, src->pixels, pitch * h)` between two screens.
fn copy_screen(g: &mut Game, src: ScreenId, dst: ScreenId) {
    if src != dst {
        let (s, d) = g.video.screen_pair_mut(src, dst);
        d.pixels.copy_from_slice(&s.pixels);
    }
}

/// The drawing part of one pass of `JE_itemScreen` (everything before the
/// animation and input loop).
fn draw_item_screen(g: &mut Game, s: &mut ItemScreen) -> Flow {
    if menu(g) == i16::from(MENU_FULL_GAME) {
        if g.config.two_player_mode {
            g.game_menu.cur_menu = i16::from(MENU_2_PLAYER_ARCADE);
        }

        if g.network.is_network_game || g.config.one_player_action {
            g.game_menu.cur_menu = i16::from(MENU_1_PLAYER_ARCADE);
        }

        if g.config.super_tyrian {
            g.game_menu.cur_menu = i16::from(MENU_SUPER_TYRIAN);
        }
    }

    let gm = &mut g.game_menu;
    gm.palette_changed = false;

    gm.left_power = false;
    gm.right_power = false;

    if gm.first_menu9 {
        s.mouse_buttons_held = g.keyboard.mouse_buttons_down;
    }

    let vga = g.video.vga_screen_id();
    let vga2 = g
        .video
        .vga_screen2
        .expect("VGAScreen2 used before init_video");
    let seg = vga_seg(g);

    if menu(g) != i16::from(MENU_DATA_CUBE_SUB) || g.game_menu.first_menu9 {
        copy_screen(g, vga2, vga);
    }

    if menu(g) == i16::from(MENU_UPGRADES) && matches!(sel(g, MENU_UPGRADES), 3 | 4) {
        // the power levels of the weapons listed in the submenu
        let up = sel(g, MENU_UPGRADES);
        let w = g.player.player[0].items.weapon[usize::from(up) - 3];
        let (item, item_power) = (w.id, i32::from(w.power));
        let row = usize::from(ITEM_AVAIL_MAP[usize::from(up) - 2]) - 1;
        let max = i32::from(g.tyrian2.item_avail_max[row]);

        for slot in 0..max {
            let v = if avail(g, row, slot) == item {
                item_power
            } else {
                1
            };
            if let Some(p) = s.power_mut(slot) {
                *p = v;
            }
        }

        // the power for "Done"
        if let Some(p) = s.power_mut(max) {
            *p = item_power;
        }
    }

    if menu(g) == i16::from(MENU_PLAY_NEXT_LEVEL) {
        let gm = &mut g.game_menu;
        gm.planet_ani = 0;
        gm.current_dot_num = 0;
        gm.current_dot_wait = 8;
        gm.planet_ani_wait = 3;
        je_update_nav_screen(g);
    }

    // the menu title, except in the upgrade submenus
    if menu(g) != i16::from(MENU_UPGRADE_SUB) {
        je_draw_menu_header(g);
    }

    // the rows of the simple menus
    let m = menu(g);
    if (0..=i16::from(MENU_PLAY_NEXT_LEVEL)).contains(&m)
        || (i16::from(MENU_2_PLAYER_ARCADE)..=i16::from(MENU_LIMITED_OPTIONS)).contains(&m)
        || m == i16::from(MENU_SUPER_TYRIAN)
    {
        je_draw_menu_choices(g);
    }

    // data cube icons
    if menu(g) == i16::from(MENU_FULL_GAME) {
        for i in 1..=i32::from(g.config.cube_max) {
            blit_sprite_dark(
                g,
                vga,
                190 + i * 18 + 2,
                37 + 1,
                OPTION_SHAPES as u32,
                34,
                false,
            );
            blit_sprite(g, vga, 190 + i * 18, 37, OPTION_SHAPES as u32, 34); // data cube
        }
    }

    if menu(g) == i16::from(MENU_LOAD_SAVE) {
        draw_load_save(g);
    }

    if menu(g) == i16::from(MENU_KEYBOARD_CONFIG) {
        for x in 2..=11u8 {
            let shade = if x == sel(g, MENU_KEYBOARD_CONFIG) {
                15
            } else {
                28
            };
            g.varz.temp2 = shade;
            let y = 38 + (i32::from(x) - 2) * 12;
            let label = g.helptext.menu_int[6][usize::from(x) - 1];
            shade_text(g, 166, y, &label, shade, 0);

            if x < 10 {
                // 10 = reset to defaults, 11 = done
                let shade = if x == sel(g, MENU_KEYBOARD_CONFIG) {
                    252
                } else {
                    250
                };
                g.varz.temp2 = shade;
                let key = g.config.key_settings[usize::from(x) - 2] as i32;
                let name = g.platform.get_scancode_name(key);
                shade_text(g, 236, y, name.as_bytes(), shade, 0);
            }
        }

        g.game_menu.menu_choices[usize::from(MENU_KEYBOARD_CONFIG)] = 11;
    }

    if menu(g) == i16::from(MENU_JOYSTICK_CONFIG) {
        draw_joystick_config(g);
    }

    if menu(g) == i16::from(MENU_UPGRADE_SUB) {
        draw_upgrade_sub(g);
    }

    // cash and shield/armor bars
    let m = menu(g);
    let up = sel(g, MENU_UPGRADES);
    if ((m <= i16::from(MENU_OPTIONS)
        || m == i16::from(MENU_KEYBOARD_CONFIG)
        || m == i16::from(MENU_LOAD_SAVE)
        || m >= i16::from(MENU_1_PLAYER_ARCADE))
        && !g.config.two_player_mode)
        || (m == i16::from(MENU_UPGRADE_SUB) && (1..=6).contains(&up))
    {
        if m != i16::from(MENU_UPGRADE_SUB) {
            let buf = format!("{}", g.player.player[0].cash);
            shade_text_with(g, 65, 173, buf.as_bytes(), 1, 6, fonthand::DARKEN);
        }
        let armor = g.player.player[0].armor as u16;
        nortvars::je_bar_draw_shadow(g, vga, 42, 152, 3, 14, armor, 2, 13);
        let shield = usize::from(g.player.player[0].items.shield);
        let mpwr = g.episodes.shields.get(shield).map_or(0, |s| s.mpwr);
        let amt = (i32::from(mpwr) * 2) as u16;
        nortvars::je_bar_draw_shadow(g, vga, 104, 152, 2, 14, amt, 2, 13);
    }

    // the left side: scores, super ships or the ship illustration
    if (i16::from(MENU_FULL_GAME)..=i16::from(MENU_OPTIONS)).contains(&m)
        || m == i16::from(MENU_KEYBOARD_CONFIG)
        || m == i16::from(MENU_LOAD_SAVE)
        || m >= i16::from(MENU_2_PLAYER_ARCADE)
        || (m == i16::from(MENU_UPGRADE_SUB) && (up == 2 || up == 5))
    {
        draw_item_screen_left(g);
    }

    // volume bars
    if m == i16::from(MENU_OPTIONS) || m == i16::from(MENU_LIMITED_OPTIONS) {
        let (music_off, samples_off) = {
            let a = loudness::audio(g);
            (a.music_disabled, a.samples_disabled)
        };
        let music = g.nortsong.tyr_music_volume / 12;
        let fx = g.nortsong.fx_volume / 12;
        let col = |off: bool| if off { 12 } else { 16 };
        nortvars::je_bar_draw_shadow(g, vga, 225, 70, 1, col(music_off), music, 3, 13);
        nortvars::je_bar_draw_shadow(g, vga, 225, 86, 1, col(samples_off), fx, 3, 13);
    }

    // data cubes (and a cube being read, when first shown)
    if m == i16::from(MENU_DATA_CUBES)
        || (m == i16::from(MENU_DATA_CUBE_SUB)
            && (g.game_menu.first_menu9 || g.game_menu.back_from_help))
    {
        draw_data_cubes(g, seg);
    }

    // 2-player input devices
    if m == i16::from(MENU_2_PLAYER_ARCADE) {
        let joysticks = g.joystick.joysticks;
        for i in 0..2 {
            let other = if i == 0 { 1 } else { 0 };
            let d = &mut g.config.input_device;
            if i32::from(d[i]) > 2 + joysticks {
                d[i] = if d[other] == 1 { 2 } else { 1 };
            }

            let device = g.config.input_device[i];
            let names = &g.helptext.input_devices;
            let temp = if joysticks > 1 && device > 2 {
                let mut t = c_str(&names[2]).to_vec();
                t.extend_from_slice(format!(" {}", i32::from(device) - 2).as_bytes());
                t
            } else {
                c_str(&names[usize::from(device) - 1]).to_vec()
            };
            let y = 38 + 2 * (i as i32 + 1) * 16;
            fonthand::je_d_string(g, vga, 186, y, &temp, SMALL_FONT_SHAPES as u32);
        }
    }

    // help text for the current row
    g.varz.flash = 0;

    // reset the player's weapons
    g.config.shot_multi_pos.fill(0);

    je_draw_score(g);

    je_draw_main_menu_help_text(g);

    if g.game_menu.new_pal > 0 {
        let p = (g.game_menu.new_pal - 1) as usize;
        g.game_menu.cur_pal = g.game_menu.new_pal;
        g.palette.colors = g.palette.palettes[p];
        let pal = g.palette.palettes[p];
        palette::set_palette(g, &pal, 0, 255);
        g.game_menu.new_pal = 0;
    }

    // the cube's title under the face
    let m = menu(g);
    if (m == i16::from(MENU_DATA_CUBES) || m == i16::from(MENU_DATA_CUBE_SUB))
        && sel(g, MENU_DATA_CUBES) < g.game_menu.menu_choices[usize::from(MENU_DATA_CUBES)]
    {
        let header = g.game_menu.cube[usize::from(sel(g, MENU_DATA_CUBES)) - 2].header;
        let x = 75 - fonthand::je_text_width(g, &header, TINY_FONT as u32) / 2;
        shade_text_with(g, x, 173, &header, 14, 3, fonthand::DARKEN);
    }
    Ok(())
}

/// `JE_textShade(VGAScreen, x, y, s, bank, brightness, shadetype)`.
fn shade_text_with(
    g: &mut Game,
    x: i32,
    y: i32,
    s: &[u8],
    bank: u32,
    brightness: i32,
    shadetype: u32,
) {
    let screen = g.video.vga_screen_id();
    fonthand::je_text_shade(g, screen, x, y, s, bank, brightness, shadetype);
}

/// The save slots of the load/save menu.
fn draw_load_save(g: &mut Game) {
    let (min, max) = if g.config.two_player_mode {
        (13, 24)
    } else {
        (2, 13)
    };

    for x in min..=max {
        let selected = x - min + 2 == i32::from(sel(g, MENU_LOAD_SAVE));
        let shade = if selected { 15 } else { 28 };
        g.varz.temp2 = shade;

        // the slot's name
        let slot = (x - 2) as usize;
        let text = if x == max {
            g.helptext.misc_text[6 - 1].to_vec()
        } else if g.config.save_files[slot].level == 0 {
            g.helptext.misc_text[3 - 1].to_vec()
        } else {
            g.config.save_files[slot].name.to_vec()
        };
        set_temp_str(g, &text);

        let y = 38 + (x - min) * 11;

        let temp_str = g.tyrian2.temp_str;
        shade_text(g, 163, y, &temp_str, shade, 0);

        if x < max {
            // not a save slot
            let shade = if selected { 252 } else { 250 };
            g.varz.temp2 = shade;

            if g.config.save_files[slot].level == 0 {
                set_temp_str(g, b"-----"); // empty slot
            } else {
                let save = &g.config.save_files[slot];
                let (level_name, episode) = (save.level_name, save.episode);
                set_temp_str(g, &level_name);

                let mut buf = c_str(&g.helptext.misc_text_b[0]).to_vec();
                buf.extend_from_slice(episode.to_string().as_bytes());
                buf.truncate(19);
                shade_text(g, 297, y, &buf, shade, 0);
            }

            let temp_str = g.tyrian2.temp_str;
            shade_text(g, 245, y, &temp_str, shade, 0);
        }

        je_draw_menu_header(g);
    }
}

/// The joystick menu's rows and values.
fn draw_joystick_config(g: &mut Game) {
    let mi = g.helptext.menu_int[6];
    let items: [&[u8]; 16] = [
        b"JOYSTICK",
        b"ANALOG AXES",
        b" SENSITIVITY",
        b" THRESHOLD",
        &mi[1],
        &mi[4],
        &mi[2],
        &mi[3],
        &mi[5],
        &mi[6],
        &mi[7],
        &mi[8],
        b"MENU",
        b"PAUSE",
        &mi[9],
        &mi[10],
    ];

    let jc = g.game_menu.joystick_config;
    let joysticks = g.joystick.joysticks;
    for (i, item) in items.iter().enumerate() {
        // `i == curSel - 2u`: unsigned, so a selection below 2 matches nothing
        let selected = i as u32 == u32::from(sel(g, MENU_JOYSTICK_CONFIG)).wrapping_sub(2);
        let mut temp: i32 = if selected { 15 } else { 28 };

        let y = 38 + i as i32 * 8;
        shade_text_with(
            g,
            166,
            y,
            item,
            (temp / 16) as u32,
            temp % 16 - 8,
            fonthand::DARKEN,
        );

        temp = if selected { 252 } else { 250 };

        let mut value = [0u8; 30];
        if joysticks == 0 && i < 14 {
            // no joysticks: everything disabled
            value[0] = b'-';
        } else if i == 0 {
            let s = (jc + 1).to_string();
            value[..s.len()].copy_from_slice(s.as_bytes());
        } else if i < 14 {
            let j = *stick(g, jc);
            if i == 1 {
                let s: &[u8] = if j.analog { b"TRUE" } else { b"FALSE" };
                value[..s.len()].copy_from_slice(s);
            } else if i < 4 {
                if !j.analog {
                    temp -= 3;
                }
                let v = if i == 2 { j.sensitivity } else { j.threshold };
                let s = v.to_string();
                value[..s.len()].copy_from_slice(s.as_bytes());
            } else {
                joystick::joystick_assignments_to_string(g, &mut value, 30, &j.assignment[i - 4]);
            }
        }

        shade_text_with(
            g,
            236,
            y,
            &value,
            (temp / 16) as u32,
            temp % 16 - 8,
            fonthand::DARKEN,
        );
    }

    g.game_menu.menu_choices[usize::from(MENU_JOYSTICK_CONFIG)] = items.len() as u8 + 1;
}

/// The upgrade submenu: the affordable selection, the shown item, the power
/// controls' state, and the list of items with their costs.
fn draw_upgrade_sub(g: &mut Game) {
    let sub = usize::from(MENU_UPGRADE_SUB);
    let up = sel(g, MENU_UPGRADES);
    let row = upgrade_slot(g);

    // move the cursor to "Done" or an item the player can afford
    loop {
        let cur = g.game_menu.cur_sel[sub];
        let choices = g.game_menu.menu_choices[sub];
        if cur >= choices {
            break;
        }
        let item = avail(g, row, i32::from(cur) - 2);
        if je_get_cost(g, up, u16::from(item)) <= g.player.player[0].cash {
            break;
        }
        let gm = &mut g.game_menu;
        gm.cur_sel[sub] = (i32::from(gm.cur_sel[sub]) + i32::from(gm.last_direction)) as u8;
        if gm.cur_sel[sub] < 2 {
            gm.cur_sel[sub] = gm.menu_choices[sub];
        } else if gm.cur_sel[sub] > gm.menu_choices[sub] {
            gm.cur_sel[sub] = 2;
        }
    }

    let cur = g.game_menu.cur_sel[sub];
    let choices = g.game_menu.menu_choices[sub];
    let shown = if cur == choices {
        // on "Done": the item owned before
        *playeritem_map(&mut g.game_menu.old_items[0], u32::from(up).wrapping_sub(2))
    } else {
        avail(g, row, i32::from(cur) - 2)
    };
    *playeritem_map(&mut g.player.player[0].items, u32::from(up).wrapping_sub(2)) = shown;

    // power levels of front and rear weapons
    if (up == 3 || up == 4) && cur < choices && avail(g, row, i32::from(cur) - 2) != 0 {
        let item_level = g.player.player[0].items.weapon[usize::from(up) - 3].power;

        // sets upgradeCost
        let item = avail(g, row, i32::from(cur) - 2);
        je_get_cost(g, up, u16::from(item));

        g.game_menu.left_power = item_level > 1; // can downgrade
        g.game_menu.right_power = item_level < 11; // can upgrade

        if g.game_menu.right_power {
            g.game_menu.right_power_afford = je_cash_left(g) >= i32::from(g.mainint.upgrade_cost);
        }
    } else {
        g.game_menu.left_power = false;
        g.game_menu.right_power = false;
    }

    // submenu title
    let vga = g.video.vga_screen_id();
    let title = g.helptext.menu_int[2][usize::from(up) - 1];
    let x = 74 + fonthand::je_font_center(g, &title, FONT_SHAPES as u32);
    fonthand::je_d_string(g, vga, x, 10, &title, FONT_SHAPES as u32);

    // the submenu's rows
    g.varz.temp_w = 1;
    while g.varz.temp_w < u16::from(g.game_menu.menu_choices[sub]) {
        let tw = i32::from(g.varz.temp_w);
        let choices = i32::from(g.game_menu.menu_choices[sub]);
        let y = 40 + (tw - 1) * 26;

        // item or "Done"?
        let cost = if tw < choices - 1 {
            let item = avail(g, row, tw - 1);
            je_get_cost(g, up, u16::from(item)) as u32
        } else {
            0 // "None" is free
        };

        let afford_shade = if u64::from(cost) > g.player.player[0].cash {
            4
        } else {
            0
        };

        g.varz.temp = avail(g, row, tw - 1); // item number
        let temp = g.varz.temp;
        let e = &g.episodes;
        // item numbers past their table: no name (`D-UB-GAMEMENU-2`)
        let name: Option<Vec<u8>> = match up - 1 {
            1 => Some(if temp > 90 {
                format!("Custom Ship {}", i32::from(temp) - 90).into_bytes()
            } else {
                e.ships
                    .get(usize::from(temp))
                    .map_or(Vec::new(), |s| s.name.to_vec())
            }),
            2 | 3 => Some(
                e.weapon_port
                    .get(usize::from(temp))
                    .map_or(Vec::new(), |w| w.name.to_vec()),
            ),
            4 => Some(
                e.shields
                    .get(usize::from(temp))
                    .map_or(Vec::new(), |w| w.name.to_vec()),
            ),
            5 => Some(
                e.power_sys
                    .get(usize::from(temp))
                    .map_or(Vec::new(), |w| w.name.to_vec()),
            ),
            6 | 7 => Some(
                e.options
                    .get(usize::from(temp))
                    .map_or(Vec::new(), |w| w.name.to_vec()),
            ),
            _ => None,
        };
        if let Some(name) = name {
            set_temp_str(g, &name);
        }
        let selected = tw == i32::from(g.game_menu.cur_sel[usize::from(menu(g) as u8)]) - 1;
        g.varz.temp2 = if selected { 15 } else { 28 };

        varz::je_get_ship_info(g);

        // the owned item's marker
        let owned = *playeritem_map(&mut g.game_menu.old_items[0], u32::from(up).wrapping_sub(2));
        if temp == owned && temp != 0 && tw != choices - 1 {
            fill_rectangle_xy(g, vga, 160, y + 7, 300, y + 11, 227);
            sprite::blit_sprite2(g, vga, 298, y + 2, Sprite2Ref::ShopSpriteSheet, 247);
        }

        // "Done"
        if tw == choices - 1 {
            let done = g.helptext.misc_text[13];
            set_temp_str(g, &done);
        }
        let temp_str = g.tyrian2.temp_str;
        let t2 = g.varz.temp2;
        shade_text(g, 185, y, &temp_str, t2, afford_shade);

        // the item's icon ("None" is an item with a blank icon)
        if tw < choices - 1 {
            je_draw_item(g, up - 1, u16::from(temp), 160, (y - 4) as u16);
        }

        // brighter when selected
        g.varz.temp2 = if selected { 15 } else { 28 };

        // the cost, except for "Done"
        if tw != choices - 1 {
            let buf = format!("Cost: {}", cost as i32);
            let t2 = g.varz.temp2;
            shade_text(g, 187, y + 10, buf.as_bytes(), t2, afford_shade);
        }
        g.varz.temp_w = g.varz.temp_w.wrapping_add(1);
    }
}

/// The left side of the shop: both players' cash, the super ships' texts,
/// or the ship illustration.
fn draw_item_screen_left(g: &mut Game) {
    if g.config.two_player_mode {
        for i in 0..2 {
            let mut buf = c_str(&g.helptext.misc_text[40 + i]).to_vec();
            buf.extend_from_slice(format!(" {}", g.player.player[i].cash).as_bytes());
            buf.truncate(49);
            shade_text_with(g, 25, 50 + 10 * i as i32, &buf, 15, 0, fonthand::FULL_SHADE);
        }
    } else if g.config.super_arcade_mode != SA_NONE || g.config.super_tyrian {
        let vga = g.video.vga_screen_id();
        let full = fonthand::FULL_SHADE as u8;
        let ships = g.helptext.super_ships;
        let title = if !g.config.super_tyrian {
            ships[usize::from(g.config.super_arcade_mode)]
        } else {
            ships[SA + 3]
        };
        helptext::je_help_box(g, vga, 35, 25, &title, 18, 7, 15, 4, full);

        shade_text_with(g, 25, 50, &ships[SA + 1], 15, 0, fonthand::FULL_SHADE);
        let front = usize::from(g.player.player[0].items.weapon[FRONT_WEAPON].id);
        let name = g
            .episodes
            .weapon_port
            .get(front)
            .map_or([0; 31], |w| w.name);
        helptext::je_help_box(g, vga, 25, 60, &name, 22, 7, 12, 1, full);
        shade_text_with(g, 25, 120, &ships[SA + 2], 15, 0, fonthand::FULL_SHADE);
        let special = usize::from(g.player.player[0].items.special);
        let name = g.episodes.special.get(special).map_or([0; 31], |w| w.name);
        helptext::je_help_box(g, vga, 25, 130, &name, 22, 7, 12, 1, full);
    } else {
        draw_ship_illustration(g);
    }
}

/// The data cube list (or a cube's face when one is being read).
fn draw_data_cubes(g: &mut Game, seg: ScreenId) {
    let vga = g.video.vga_screen_id();
    g.game_menu.first_menu9 = false;
    let cubes = usize::from(MENU_DATA_CUBES);
    g.game_menu.menu_choices[cubes] = g.config.cube_max.wrapping_add(2) as u8;
    fill_rectangle_xy(g, vga, 1, 1, 145, 170, 0);

    blit_sprite(g, seg, 1, 1, OPTION_SHAPES as u32, 20); // portrait background

    if menu(g) == i16::from(MENU_DATA_CUBES) {
        let full = fonthand::FULL_SHADE as u8;
        if g.config.cube_max == 0 {
            let text = g.helptext.misc_text[16 - 1];
            helptext::je_help_box(g, vga, 166, 80, &text, 30, 7, 12, 1, full);
            g.varz.temp_w = 160;
            g.varz.temp2 = 252;
        } else {
            for x in 1..=i32::from(g.config.cube_max) {
                mainint::je_draw_cube(g, seg, 166, (38 + (x - 1) * 28) as u16, 13, 0);
                g.varz.temp2 = if x + 1 == i32::from(sel(g, MENU_DATA_CUBES)) {
                    252
                } else {
                    250
                };

                let title = g.game_menu.cube[(x - 1) as usize].title;
                let t2 = g.varz.temp2;
                let darken = fonthand::DARKEN as u8;
                let bright = (i32::from(t2 % 16) - 8) as u8;
                helptext::je_help_box(
                    g,
                    vga,
                    192,
                    44 + (x - 1) * 28,
                    &title,
                    24,
                    7,
                    t2 / 16,
                    bright,
                    darken,
                );
            }
            let x = i32::from(g.config.cube_max) + 1;
            g.varz.temp2 = if x + 1 == i32::from(sel(g, MENU_DATA_CUBES)) {
                252
            } else {
                250
            };
            g.varz.temp_w = (44 + (x - 1) * 28) as u16;
        }

        let text = g.helptext.misc_text[6 - 1];
        let t2 = g.varz.temp2;
        let y = i32::from(g.varz.temp_w);
        shade_text(g, 172, y, &text, t2, 0);
    }

    if sel(g, MENU_DATA_CUBES) < g.game_menu.menu_choices[cubes] {
        let face_sprite = g.game_menu.cube[usize::from(sel(g, MENU_DATA_CUBES)) - 2].face_sprite;

        if face_sprite != -1 {
            let s = sprite::sprite(g, FACE_SHAPES as u32, face_sprite as u32);
            let (w, h) = (i32::from(s.width), i32::from(s.height));
            let (face_x, face_y) = (77 - w / 2, 92 - h / 2);

            blit_sprite(
                g,
                seg,
                face_x,
                face_y,
                FACE_SHAPES as u32,
                face_sprite as u32,
            ); // datacube face

            // the face's palette
            g.game_menu.palette_changed = true;
            // a face without a palette (upstream reads past `facepal`):
            // palette 0 (`D-UB-GAMEMENU-3`)
            g.varz.temp2 = FACEPAL.get(face_sprite as usize).copied().unwrap_or(0);
            g.game_menu.new_pal = 0;

            let p = usize::from(g.varz.temp2);
            g.varz.temp = 1;
            while i32::from(g.varz.temp) <= 255 - 3 * 16 {
                let t = usize::from(g.varz.temp);
                g.palette.colors[t] = g.palette.palettes[p][t];
                g.varz.temp += 1;
            }
        }
    }
}

/// `SDL_SCANCODE_*` values the shop handles.
mod scancode {
    pub const RETURN: u16 = 40;
    pub const ESCAPE: u16 = 41;
    pub const SPACE: u16 = 44;
    pub const SLASH: u16 = 56;
    pub const F1: u16 = 58;
    pub const HOME: u16 = 74;
    pub const PAGEUP: u16 = 75;
    pub const END: u16 = 77;
    pub const PAGEDOWN: u16 = 78;
    pub const RIGHT: u16 = 79;
    pub const LEFT: u16 = 80;
    pub const DOWN: u16 = 81;
    pub const UP: u16 = 82;
}

/// `SDLK_l`, `SDLK_s`, `KMOD_ALT`.
const SDLK_L: i32 = b'l' as i32;
const SDLK_S: i32 = b's' as i32;
const KMOD_ALT: u16 = 0x0100 | 0x0200;

/// `newPal` applied (palette `newPal - 1`), then the face palette if it
/// changed, around the mouse cursor: the frame presentation of most menus.
fn apply_new_pal(g: &mut Game) {
    if g.game_menu.new_pal > 0 {
        g.game_menu.cur_pal = g.game_menu.new_pal;
        let pal = g.palette.palettes[(g.game_menu.new_pal - 1) as usize];
        palette::set_palette(g, &pal, 0, 255);
        g.game_menu.new_pal = 0;
    }
}

fn apply_face_pal(g: &mut Game) {
    if g.game_menu.palette_changed {
        let colors = g.palette.colors;
        palette::set_palette(g, &colors, 0, 255);
        g.game_menu.palette_changed = false;
    }
}

fn fade_back_from_help(g: &mut Game) -> Flow {
    if g.game_menu.back_from_help {
        let colors = g.palette.colors;
        palette::fade_palette(g, &colors, 10, 0, 255)?;
        g.game_menu.back_from_help = false;
    }
    Ok(())
}

/// The animation loop of `JE_itemScreen`: redraws what moves and waits a
/// frame, until there is input.
fn animate_item_screen(g: &mut Game, s: &mut ItemScreen) -> Flow {
    let vga = g.video.vga_screen_id();
    let seg = vga_seg(g);
    loop {
        g.mouse.mouse_cursor = MOUSE_POINTER_NORMAL;

        let gm = &mut g.game_menu;
        gm.col = gm.col.wrapping_add(gm.col_c);
        if gm.col < -2 || gm.col > 6 {
            gm.col_c = gm.col_c.wrapping_neg();
        }

        if menu(g) == i16::from(MENU_DATA_CUBE_SUB) {
            // reading a data cube
            let (mx, my) = (g.keyboard.mouse_x, g.keyboard.mouse_y);
            if mx > 164 && mx < 299 && my > 47 && my < 153 {
                g.mouse.mouse_cursor = if my > 100 {
                    MOUSE_POINTER_DOWN
                } else {
                    MOUSE_POINTER_UP
                };
            }

            fill_rectangle_xy(g, vga, 160, 49, 310, 158, 228);
            let gm = &mut g.game_menu;
            if i32::from(gm.y_loc) + i32::from(gm.y_chg) < 0 {
                gm.y_chg = 0;
                gm.y_loc = 0;
            }

            gm.y_loc = (i32::from(gm.y_loc) + i32::from(gm.y_chg)) as u16;
            let y_loc = gm.y_loc;
            g.varz.temp = (y_loc / 12) as u8;
            g.varz.temp2 = (y_loc % 12) as u8;
            g.varz.temp_w = 38 + 12 - u16::from(g.varz.temp2);
            let c = usize::from(sel(g, MENU_DATA_CUBES)) - 2;
            g.varz.temp3 = g.game_menu.cube[c].last_line as u8;

            let first = i32::from(g.varz.temp) + 1;
            for x in first..=first + 9 {
                if x <= i32::from(g.varz.temp3) {
                    let line = g.game_menu.cube[c].text[(x - 1) as usize];
                    let y = i32::from(g.varz.temp_w);
                    fonthand::je_out_text_and_darken(
                        g,
                        vga,
                        161,
                        y,
                        &line,
                        14,
                        3,
                        TINY_FONT as u32,
                    );
                    g.varz.temp_w = g.varz.temp_w.wrapping_add(12);
                }
            }

            fill_rectangle_xy(g, vga, 160, 39, 310, 48, 228);
            fill_rectangle_xy(g, vga, 160, 157, 310, 166, 228);

            let last_line = g.game_menu.cube[usize::from(g.game_menu.current_cube)].last_line;
            let percent_read = if last_line <= 9 {
                100
            } else {
                (i32::from(g.game_menu.y_loc) * 100) / ((last_line - 9) * 12)
            };

            let mut buf = c_str(&g.helptext.misc_text[11]).to_vec();
            buf.extend_from_slice(format!(" {percent_read}%").as_bytes());
            buf.truncate(54);
            fonthand::je_out_text_and_darken(g, vga, 176, 160, &buf, 14, 1, TINY_FONT as u32);

            let back = g.helptext.misc_text[12];
            fonthand::je_d_string(g, vga, 260, 160, &back, SMALL_FONT_SHAPES as u32);

            if g.varz.temp2 == 0 {
                g.game_menu.y_chg = 0;
            }

            mouse::je_mouse_start(g);

            video::je_show_vga(g);

            fade_back_from_help(g)?;
            mouse::je_mouse_replace(g);

            nortsong::set_frame_count(g, 1);
        } else {
            if menu(g) == i16::from(MENU_PLAY_NEXT_LEVEL) {
                je_update_nav_screen(g);
                je_draw_main_menu_help_text(g);
                je_draw_menu_header(g);
                je_draw_menu_choices(g);
                if g.config.extra_game {
                    let text = g.helptext.misc_text[68 - 1];
                    fonthand::je_d_string(g, vga, 170, 140, &text, FONT_SHAPES as u32);
                }
            }

            if menu(g) == i16::from(MENU_DATA_CUBES)
                && sel(g, MENU_DATA_CUBES) < g.game_menu.menu_choices[usize::from(MENU_DATA_CUBES)]
            {
                // the flashing cube
                let y = 38 + (i32::from(sel(g, MENU_DATA_CUBES)) - 2) * 28;
                let col = g.game_menu.col as i8;
                sprite::blit_sprite_hv_blend(g, seg, 166, y, OPTION_SHAPES as u32, 25, 13, col);
            }

            let up = sel(g, MENU_UPGRADES);
            if menu(g) == i16::from(MENU_UPGRADE_SUB)
                && (up == 3 || up == 4 || (6..=8).contains(&up))
            {
                // the weapon simulator
                nortsong::set_frame_count(g, 3);
                je_weapon_sim_update(g);
                je_draw_score(g);

                apply_new_pal(g);

                mouse::je_mouse_start(g);

                apply_face_pal(g);

                video::je_show_vga(g);

                fade_back_from_help(g)?;

                mouse::je_mouse_replace(g);
            } else {
                // every other menu
                nortsong::set_frame_count(g, 2);

                je_draw_score(g);

                apply_new_pal(g);

                mouse::je_mouse_start(g);

                apply_face_pal(g);

                video::je_show_vga(g);

                mouse::je_mouse_replace(g);

                fade_back_from_help(g)?;
            }
        }

        keyboard::wait_until_elapsed(g)?;

        if menu(g) == i16::from(MENU_DATA_CUBE_SUB) {
            s.mouse_buttons_held &= g.keyboard.mouse_buttons_down;

            let cursor = g.mouse.mouse_cursor;
            if (g.keyboard.mouse_buttons_down & !s.mouse_buttons_held) != 0
                && cursor != MOUSE_POINTER_NORMAL
            {
                g.game_menu.y_chg = if cursor == MOUSE_POINTER_UP { -1 } else { 1 };
            }

            let keys = &g.keyboard.keysactive;
            if keys[usize::from(scancode::PAGEUP)] {
                g.game_menu.y_chg = -2;
            }
            if keys[usize::from(scancode::PAGEDOWN)] {
                g.game_menu.y_chg = 2;
            }

            let (mut joystick_up, mut joystick_down) = (false, false);
            if let Some(js) = g.joystick.joystick.as_ref() {
                for j in js.iter().take(g.joystick.joysticks.max(0) as usize) {
                    joystick_up |= j.direction[0];
                    joystick_down |= j.direction[2];
                }
            }

            if keys[usize::from(scancode::UP)] || joystick_up {
                g.game_menu.y_chg = -1;
            }

            if keys[usize::from(scancode::DOWN)] || joystick_down {
                g.game_menu.y_chg = 1;
            }

            let gm = &mut g.game_menu;
            if gm.y_chg < 0 && gm.y_loc == 0 {
                gm.y_chg = 0;
            }
            let last_line = gm.cube[usize::from(gm.current_cube)].last_line;
            if gm.y_chg > 0 && i32::from(gm.y_loc / 12) > last_line - 10 {
                gm.y_chg = 0;
            }
        }

        if keyboard::has_input(g, INPUT_NO_MOTION) {
            return Ok(());
        }
    }
}

/// `curSel[MENU_UPGRADES]` is the front or rear weapon.
fn weapon_row(g: &Game) -> bool {
    matches!(sel(g, MENU_UPGRADES), 3 | 4)
}

/// Sets the shown weapon's power to its remembered level (front/rear weapon
/// rows of the upgrade submenu).
fn restore_weapon_power(g: &mut Game, s: &ItemScreen) {
    let up = usize::from(sel(g, MENU_UPGRADES));
    let p = s.power(i32::from(sel(g, MENU_UPGRADE_SUB)) - 2);
    g.player.player[0].items.weapon[up - 3].power = p as u8;
}

/// One power level down (`up == false`) or up for the selected weapon.
fn change_power(g: &mut Game, s: &mut ItemScreen, up: bool) {
    let allowed = if up {
        g.game_menu.right_power && g.game_menu.right_power_afford
    } else {
        g.game_menu.left_power
    };
    if allowed {
        let i = i32::from(sel(g, MENU_UPGRADE_SUB)) - 2;
        let v = match s.power_mut(i) {
            Some(p) => {
                *p += if up { 1 } else { -1 };
                *p
            }
            None => 0,
        };
        let w = usize::from(sel(g, MENU_UPGRADES)) - 3;
        g.player.player[0].items.weapon[w].power = v as u8;
    } else {
        nortsong::je_play_sample_num(g, S_CLINK);
    }
}

/// A click in the shop.
fn item_screen_mouse(g: &mut Game, s: &mut ItemScreen, input: MouseInput) -> Flow {
    g.game_menu.last_direction = 1;

    if menu(g) == i16::from(MENU_DATA_CUBES) && g.config.cube_max == 0 {
        g.game_menu.cur_menu = i16::from(MENU_FULL_GAME);
        nortsong::je_play_sample_num(g, S_SPRING);
        g.game_menu.new_pal = 1;
    }

    if menu(g) == i16::from(MENU_DATA_CUBE_SUB)
        && input.x > 258
        && input.x < 290
        && input.y > 159
        && input.y < 171
    {
        g.game_menu.cur_menu = i16::from(MENU_DATA_CUBES);
        nortsong::je_play_sample_num(g, S_SPRING);
    }

    if menu(g) == i16::from(MENU_OPTIONS) || menu(g) == i16::from(MENU_LIMITED_OPTIONS) {
        if input.x >= 225 - 4 && input.y >= 70 && input.y <= 82 {
            let was_disabled = std::mem::replace(&mut loudness::audio(g).music_disabled, false);
            if was_disabled {
                loudness::restart_song(g);
            }

            g.game_menu.cur_sel[usize::from(MENU_OPTIONS)] = 4;

            let v = ((input.x - (225 - 4)) / 4 * 12) as u16;
            g.nortsong.tyr_music_volume = v.min(255);
        }

        if input.x >= 225 - 4 && input.y >= 86 && input.y <= 98 {
            loudness::audio(g).samples_disabled = false;

            g.game_menu.cur_sel[usize::from(MENU_OPTIONS)] = 5;

            let v = ((input.x - (225 - 4)) / 4 * 12) as u16;
            g.nortsong.fx_volume = v.min(255);
        }

        let (music, fx) = (
            g.nortsong.tyr_music_volume as u8,
            g.nortsong.fx_volume as u8,
        );
        loudness::set_volume(g, music, fx);

        nortsong::je_play_sample_num(g, S_CURSOR);
    }

    if input.y > 20 && input.x > 170 && input.x < 308 && menu(g) != i16::from(MENU_DATA_CUBE_SUB) {
        const MOUSE_SELECTION_Y: [i32; MENU_MAX] =
            [16, 16, 16, 16, 26, 12, 11, 28, 0, 16, 16, 16, 8, 16];

        let m = menu(g);
        let mut selection = (input.y - 38) / MOUSE_SELECTION_Y[m as usize] + 2;

        if m == i16::from(MENU_2_PLAYER_ARCADE) {
            if selection > 5 {
                selection -= 1;
            }
            if selection > 3 {
                selection -= 1;
            }
        }

        if m == i16::from(MENU_FULL_GAME) && selection > 7 {
            selection = 7;
        }

        // the next level screen
        let choices = i32::from(g.game_menu.menu_choices[m as usize]);
        if m == i16::from(MENU_PLAY_NEXT_LEVEL) && selection == choices + 1 {
            selection = choices;
        }

        if selection <= choices {
            if m == i16::from(MENU_UPGRADE_SUB) && selection == choices {
                g.player.player[0].cash = i64::from(je_cash_left(g)) as u64;
                g.game_menu.cur_menu = i16::from(MENU_UPGRADES);
                nortsong::je_play_sample_num(g, S_ITEM);
            } else {
                nortsong::je_play_sample_num(g, S_CLICK);
                let cur = g.game_menu.cur_sel[m as usize];
                if i32::from(cur) == selection {
                    je_menu_function(g, cur)?;
                } else {
                    let too_dear = m == i16::from(MENU_UPGRADE_SUB) && {
                        let up = sel(g, MENU_UPGRADES);
                        let item = avail(g, upgrade_slot(g), selection - 2);
                        je_get_cost(g, up, u16::from(item)) > g.player.player[0].cash
                    };
                    if too_dear {
                        nortsong::je_play_sample_num(g, S_CLINK);
                    } else {
                        if sel(g, MENU_UPGRADES) == 4 {
                            g.player.player[0].weapon_mode = 1;
                        }

                        g.game_menu.cur_sel[m as usize] = selection as u8;
                    }

                    // front or rear weapon submenu
                    if m == i16::from(MENU_UPGRADE_SUB) && weapon_row(g) {
                        restore_weapon_power(g, s);
                    }
                }
            }
        }
    }

    if menu(g) == i16::from(MENU_UPGRADE_SUB) && weapon_row(g) {
        if input.x >= 23 && input.x <= 36 && input.y >= 149 && input.y <= 168 {
            nortsong::je_play_sample_num(g, S_CURSOR);
            change_power(g, s, false);
        }

        if input.x >= 119 && input.x <= 131 && input.y >= 149 && input.y <= 168 {
            nortsong::je_play_sample_num(g, S_CURSOR);
            change_power(g, s, true);
        }
    }
    Ok(())
}

/// A key in the shop.
fn item_screen_key(g: &mut Game, s: &mut ItemScreen, input: KeyboardInput) -> Flow {
    let m = menu(g);
    match input.scancode {
        scancode::SLASH => {
            // cycle the rear weapon's modes
            if m == i16::from(MENU_UPGRADE_SUB) && sel(g, MENU_UPGRADES) == 4 {
                let rear = usize::from(g.player.player[0].items.weapon[REAR_WEAPON].id);
                let opnum = u32::from(g.episodes.weapon_port.get(rear).map_or(0, |w| w.opnum));
                let p = &mut g.player.player[0];
                p.weapon_mode = p.weapon_mode.wrapping_add(1);
                if p.weapon_mode > opnum {
                    p.weapon_mode = 1;
                }
            }
        }

        scancode::SPACE | scancode::RETURN => {
            // the front/rear weapon's power for "Done"
            if m == i16::from(MENU_UPGRADE_SUB) && weapon_row(g) {
                let max = i32::from(g.tyrian2.item_avail_max[upgrade_slot(g)]);
                let power =
                    g.player.player[0].items.weapon[usize::from(sel(g, MENU_UPGRADES)) - 3].power;
                if let Some(p) = s.power_mut(max) {
                    *p = i32::from(power);
                }
            }

            let cur = g.game_menu.cur_sel[m as usize];
            je_menu_function(g, cur)?;
        }

        scancode::ESCAPE => {
            nortsong::je_play_sample_num(g, S_SPRING);
            if m == i16::from(MENU_LOAD_SAVE) && g.game_menu.quik_save {
                g.game_menu.cur_menu = i16::from(g.game_menu.old_menu);
                g.game_menu.new_pal = g.game_menu.old_pal;
            } else if MENU_ESC[m as usize] == 0 {
                quit_to_title(g)?;
            } else {
                if m == i16::from(MENU_UPGRADE_SUB) {
                    // leaving without buying
                    g.player.player[0].items = g.game_menu.old_items[0];
                    g.game_menu.cur_sel[usize::from(MENU_UPGRADE_SUB)] = g.game_menu.last_cur_sel;
                    g.player.player[0].cash = i64::from(je_cash_left(g)) as u64;
                }

                if m != i16::from(MENU_DATA_CUBE_SUB) {
                    g.game_menu.new_pal = 1;
                }

                g.game_menu.cur_menu = i16::from(MENU_ESC[m as usize]) - 1;
            }
        }

        scancode::F1 => {
            if !g.network.is_network_game {
                palette::fade_black(g, 10)?;
                mainint::je_help_system(g, 2)?;

                let song = u32::from(g.musmast.song_buy);
                loudness::play_song(g, song)?;

                let vga = g.video.vga_screen_id();
                picload::je_load_pic(g, vga, 1, false)?;
                g.game_menu.new_pal = if m == 3 { 18 } else { 1 };

                let vga2 = g
                    .video
                    .vga_screen2
                    .expect("VGAScreen2 used before init_video");
                copy_screen(g, vga, vga2);

                g.game_menu.cur_pal = g.game_menu.new_pal;
                g.palette.colors = g.palette.palettes[(g.game_menu.new_pal - 1) as usize];
                video::je_show_vga(g);
                g.game_menu.new_pal = 0;
                g.game_menu.back_from_help = true;
            }
        }

        scancode::UP | scancode::DOWN => {
            let up = input.scancode == scancode::UP;
            g.game_menu.last_direction = if up { -1 } else { 1 };

            if m != i16::from(MENU_DATA_CUBE_SUB) {
                nortsong::je_play_sample_num(g, S_CURSOR);
            }

            let gm = &mut g.game_menu;
            let mi = m as usize;
            if up {
                gm.cur_sel[mi] = gm.cur_sel[mi].wrapping_sub(1);
                if gm.cur_sel[mi] < 2 {
                    gm.cur_sel[mi] = gm.menu_choices[mi];
                }
            } else {
                gm.cur_sel[mi] = gm.cur_sel[mi].wrapping_add(1);
                if gm.cur_sel[mi] > gm.menu_choices[mi] {
                    gm.cur_sel[mi] = 2;
                }
            }

            // front or rear weapon submenu
            if m == i16::from(MENU_UPGRADE_SUB) && weapon_row(g) {
                restore_weapon_power(g, s);
                if sel(g, MENU_UPGRADES) == 4 {
                    g.player.player[0].weapon_mode = 1;
                }
            }

            // joystick menu: skip the analog rows when digital
            if m == i16::from(MENU_JOYSTICK_CONFIG) && g.joystick.joysticks > 0 {
                let jc = g.game_menu.joystick_config;
                let analog = stick(g, jc).analog;
                let cur = &mut g.game_menu.cur_sel[mi];
                if !analog && up && *cur == 5 {
                    *cur = 3;
                }
                if !analog && !up && *cur == 4 {
                    *cur = 6;
                }
            }
        }

        scancode::HOME => {
            if m == i16::from(MENU_DATA_CUBE_SUB) {
                g.game_menu.y_loc = 0;
            }
        }

        scancode::END => {
            if m == i16::from(MENU_DATA_CUBE_SUB) {
                let gm = &mut g.game_menu;
                gm.y_loc = ((gm.cube[usize::from(gm.current_cube)].last_line - 9) * 12) as u16;
            }
        }

        scancode::LEFT | scancode::RIGHT => {
            let right = input.scancode == scancode::RIGHT;
            if m == i16::from(MENU_JOYSTICK_CONFIG) && g.joystick.joysticks > 0 {
                let joysticks = g.joystick.joysticks;
                let jc = g.game_menu.joystick_config;
                match sel(g, MENU_JOYSTICK_CONFIG) {
                    2 => {
                        g.game_menu.joystick_config = if right {
                            (jc + 1) % joysticks
                        } else {
                            (if jc == 0 { joysticks } else { jc }) - 1
                        };
                    }
                    3 => {
                        let j = stick(g, jc);
                        j.analog = !j.analog;
                    }
                    4 => {
                        let j = stick(g, jc);
                        j.sensitivity = if right {
                            (j.sensitivity + 1) % 11
                        } else if j.sensitivity == 0 {
                            10
                        } else {
                            j.sensitivity - 1
                        };
                    }
                    5 => {
                        let j = stick(g, jc);
                        j.threshold = if right {
                            (j.threshold + 1) % 11
                        } else if j.threshold == 0 {
                            10
                        } else {
                            j.threshold - 1
                        };
                    }
                    _ => {}
                }
            }

            if m == i16::from(MENU_2_PLAYER_ARCADE) {
                let cur = sel(g, MENU_2_PLAYER_ARCADE);
                if cur == 3 || cur == 4 {
                    nortsong::je_play_sample_num(g, S_CURSOR);
                    cycle_input_device(g, usize::from(cur - 3), right);
                }
            }

            if m == i16::from(MENU_OPTIONS)
                || m == i16::from(MENU_UPGRADE_SUB)
                || m == i16::from(MENU_LIMITED_OPTIONS)
            {
                nortsong::je_play_sample_num(g, S_CURSOR);
            }

            if m == i16::from(MENU_OPTIONS) || m == i16::from(MENU_LIMITED_OPTIONS) {
                let delta = if right { 12 } else { -12 };
                match g.game_menu.cur_sel[m as usize] {
                    4 => {
                        change_volume(g, delta, 0);
                        let was_disabled =
                            std::mem::replace(&mut loudness::audio(g).music_disabled, false);
                        if was_disabled {
                            loudness::restart_song(g);
                        }
                    }
                    5 => {
                        change_volume(g, 0, delta);
                        loudness::audio(g).samples_disabled = false;
                    }
                    _ => {}
                }
            } else if m == i16::from(MENU_UPGRADE_SUB) && weapon_row(g) {
                change_power(g, s, right);
            }
        }

        _ => {
            // Alt+S / Alt+L: quick save / load
            if (input.sym == SDLK_S || input.sym == SDLK_L)
                && input.r#mod & KMOD_ALT != 0
                && m != i16::from(MENU_LOAD_SAVE)
            {
                let gm = &mut g.game_menu;
                if m == i16::from(MENU_DATA_CUBE_SUB) || m == i16::from(MENU_DATA_CUBES) {
                    gm.cur_menu = i16::from(MENU_FULL_GAME);
                }
                gm.quik_save = true;
                gm.old_menu = gm.cur_menu as u8;
                gm.cur_menu = i16::from(MENU_LOAD_SAVE);
                g.mainint.perform_save = input.sym == SDLK_S;
                gm.new_pal = 1;
                gm.old_pal = gm.cur_pal;
            }
        }
    }
    Ok(())
}

/// `JE_changeVolume(&tyrMusicVolume, music, &fxVolume, fx)`.
fn change_volume(g: &mut Game, music: i32, fx: i32) {
    let (mut m, mut f) = (g.nortsong.tyr_music_volume, g.nortsong.fx_volume);
    nortsong::je_change_volume(g, &mut m, music, &mut f, fx);
    g.nortsong.tyr_music_volume = m;
    g.nortsong.fx_volume = f;
}
