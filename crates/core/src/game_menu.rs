// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `game_menu.c` / `game_menu.h`.

use crate::backgrnd::{initialize_starfield, update_and_draw_starfield};
use crate::config::{SAVE_FILES_NUM, SHOT_LEFT_SIDEKICK, SHOT_RIGHT_SIDEKICK};
use crate::fonthand::{self, c_str};
use crate::game::{Flow, Game};
use crate::helptext::{self, MENU_HELP, MENU_MAX};
use crate::keyboard::{self, INPUT_NO_MOTION, KeyboardInput, MouseInput, SDL_SCANCODE_RETURN};
use crate::player::{FRONT_WEAPON, LEFT_SIDEKICK, PlayerItems, REAR_WEAPON, RIGHT_SIDEKICK};
use crate::sdl::upper_blit;
use crate::shots::{player_shot_create, simulate_player_shots};
use crate::sndmast::{S_CLICK, S_CURSOR, S_SPRING};
use crate::sprite::{
    self, FONT_SHAPES, OPTION_SHAPES, PLANET_SHAPES, SMALL_FONT_SHAPES, TINY_FONT, WEAPON_SHAPES,
    blit_sprite, blit_sprite_dark, blit_sprite_hv, blit_sprite2x2,
};
use crate::varz::{P_ANI, PGR, Sprite2Ref, je_s_gr};
use crate::vga256d::{fill_rectangle_xy, je_bar_shade, je_pix, je_pix3, je_rectangle};
use crate::video::{self, ScreenId};
use crate::{joystick, mouse, network, nortsong, picload};
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

/// Width and height of planet sprite `index`.
fn planet_size(g: &Game, index: i32) -> (u32, u32) {
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
        let origin = usize::from(g.varz.map_origin) - 1;
        let dest = usize::from(g.varz.map_planet[usize::from(cur) - 2]) - 1;
        let (ow, oh) = planet_size(g, i32::from(PGR[origin]) - 1);
        let (dw, dh) = planet_size(g, i32::from(PGR[dest]) - 1);
        let (ox, oy, dx, dy) = (ow / 2, oh / 2, dw / 2, dh / 2);

        // unsigned arithmetic, as in C (int minus unsigned)
        let nx = u32::from(PLANET_X[origin])
            .wrapping_sub(ox)
            .wrapping_add(u32::from(PLANET_X[dest]))
            .wrapping_sub(dx);
        let ny = u32::from(PLANET_Y[origin])
            .wrapping_sub(oy)
            .wrapping_add(u32::from(PLANET_Y[dest]))
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
    let origin = usize::from(g.varz.map_origin) - 1;
    let mut x: u8 = 0;
    while u16::from(x) < g.varz.map_p_num {
        let xi = usize::from(x);
        let dest = usize::from(g.varz.map_planet[xi]) - 1;
        let dist_x = i32::from(PLANET_X[dest]) - i32::from(PLANET_X[origin]);
        let dist_y = i32::from(PLANET_Y[dest]) - i32::from(PLANET_Y[origin]);
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
            let tx = je_part_way(PLANET_X[origin] as i16, PLANET_X[dest] as i16, dots, y);
            let ty = je_part_way(PLANET_Y[origin] as i16, PLANET_Y[dest] as i16, dots, y);
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
        g.varz.temp = g.tyrian2.item_avail[slot][tw];
        let temp = usize::from(g.varz.temp);
        let e = &g.episodes;
        let name = match item_num {
            2 => Some(e.ships[temp].name),
            3 | 4 => Some(e.weapon_port[temp].name),
            5 => Some(e.shields[temp].name),
            6 => Some(e.power_sys[temp].name),
            7 | 8 => Some(e.options[temp].name),
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
