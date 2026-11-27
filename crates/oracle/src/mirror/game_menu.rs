//! Mirror of `rustyrian_core::game_menu::State` <-> the C globals of
//! `game_menu.c`. `struct cube_struct` is file-local, so its C layout is
//! declared here.

use super::Mirror;
use super::player::{player_items_from_c, player_items_to_c};
use super::video::sv;
use crate::bindings as b;
use rustyrian_core::game_menu::{CubeStruct, JeMenuChoiceType, State};
use rustyrian_core::helptext::MENU_MAX;
use std::ffi::{c_char, c_int};

/// C `struct cube_struct`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CCube {
    pub title: [c_char; 81],
    pub header: [c_char; 13],
    pub face_sprite: c_int,
    pub text: [[c_char; 36]; 90],
    pub last_line: c_int,
}

fn u8s<const N: usize>(c: &[c_char; N]) -> [u8; N] {
    c.map(|x| x as u8)
}

fn chars<const N: usize>(r: &[u8; N]) -> [c_char; N] {
    r.map(|x| x as c_char)
}

fn cube_from_c(c: &CCube) -> CubeStruct {
    CubeStruct {
        title: u8s(&c.title),
        header: u8s(&c.header),
        face_sprite: c.face_sprite,
        text: c.text.map(|l| u8s(&l)),
        last_line: c.last_line,
    }
}

fn cube_to_c(r: &CubeStruct) -> CCube {
    CCube {
        title: chars(&r.title),
        header: chars(&r.header),
        face_sprite: r.face_sprite,
        text: r.text.map(|l| chars(&l)),
        last_line: r.last_line,
    }
}

macro_rules! statics {
    ($self:ident, $dir:ident; $($field:ident: $t:ty = $getter:ident),* $(,)?) => {
        $( statics!(@one $self, $dir, $field, $t, $getter); )*
    };
    (@one $self:ident, load, $field:ident, $t:ty, $getter:ident) => {
        $self.$field = *sv::<$t>(b::$getter);
    };
    (@one $self:ident, store, $field:ident, $t:ty, $getter:ident) => {
        *sv::<$t>(b::$getter) = $self.$field;
    };
}

macro_rules! all_statics {
    ($self:ident, $dir:ident) => {
        statics!($self, $dir;
            joystick_config: c_int = ot_sv_game_menu_joystick_config,
            y_loc: u16 = ot_sv_game_menu_yLoc,
            y_chg: i8 = ot_sv_game_menu_yChg,
            new_pal: c_int = ot_sv_game_menu_newPal,
            cur_pal: c_int = ot_sv_game_menu_curPal,
            old_pal: c_int = ot_sv_game_menu_oldPal,
            quik_save: bool = ot_sv_game_menu_quikSave,
            old_menu: u8 = ot_sv_game_menu_oldMenu,
            back_from_help: bool = ot_sv_game_menu_backFromHelp,
            last_direction: i16 = ot_sv_game_menu_lastDirection,
            first_menu9: bool = ot_sv_game_menu_firstMenu9,
            palette_changed: bool = ot_sv_game_menu_paletteChanged,
            menu_choices: JeMenuChoiceType = ot_sv_game_menu_menuChoices,
            col: i16 = ot_sv_game_menu_col,
            col_c: i16 = ot_sv_game_menu_colC,
            last_cur_sel: u8 = ot_sv_game_menu_lastCurSel,
            cur_menu: i16 = ot_sv_game_menu_curMenu,
            cur_sel: [u8; MENU_MAX] = ot_sv_game_menu_curSel,
            cur_item_type: u8 = ot_sv_game_menu_curItemType,
            cur_item: u8 = ot_sv_game_menu_curItem,
            cursor: u8 = ot_sv_game_menu_cursor,
            left_power: bool = ot_sv_game_menu_leftPower,
            right_power: bool = ot_sv_game_menu_rightPower,
            right_power_afford: bool = ot_sv_game_menu_rightPowerAfford,
            current_cube: u8 = ot_sv_game_menu_currentCube,
            planet_ani: u8 = ot_sv_game_menu_planetAni,
            planet_ani_wait: u8 = ot_sv_game_menu_planetAniWait,
            current_dot_num: u8 = ot_sv_game_menu_currentDotNum,
            current_dot_wait: u8 = ot_sv_game_menu_currentDotWait,
            nav_x: f32 = ot_sv_game_menu_navX,
            nav_y: f32 = ot_sv_game_menu_navY,
            new_nav_x: f32 = ot_sv_game_menu_newNavX,
            new_nav_y: f32 = ot_sv_game_menu_newNavY,
            temp_nav_x: i16 = ot_sv_game_menu_tempNavX,
            temp_nav_y: i16 = ot_sv_game_menu_tempNavY,
            planet_dots: [u8; 5] = ot_sv_game_menu_planetDots,
            planet_dot_x: [[i16; 10]; 5] = ot_sv_game_menu_planetDotX,
            planet_dot_y: [[i16; 10]; 5] = ot_sv_game_menu_planetDotY,
        );
    };
}

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make; each
        // static is read through its own C type.
        unsafe {
            all_statics!(self, load);
            let items = sv::<[b::PlayerItems; 2]>(b::ot_sv_game_menu_old_items);
            for (r, c) in self.old_items.iter_mut().zip(items.iter()) {
                *r = player_items_from_c(c);
            }
            let cube = sv::<[CCube; 4]>(b::ot_sv_game_menu_cube);
            for (r, c) in self.cube.iter_mut().zip(cube.iter()) {
                *r = cube_from_c(c);
            }
        }
    }

    fn store_to_c(&self) {
        // SAFETY: as in `load_from_c`.
        unsafe {
            all_statics!(self, store);
            let items = sv::<[b::PlayerItems; 2]>(b::ot_sv_game_menu_old_items);
            for (r, c) in self.old_items.iter().zip(items.iter_mut()) {
                *c = player_items_to_c(r);
            }
            let cube = sv::<[CCube; 4]>(b::ot_sv_game_menu_cube);
            for (r, c) in self.cube.iter().zip(cube.iter_mut()) {
                *c = cube_to_c(r);
            }
        }
    }
}
