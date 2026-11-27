// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `starlib.c` / `starlib.h`.

use crate::Game;
use crate::game::Flow;
use crate::joystick::push_joysticks_as_keyboard;
use crate::keyboard::{KeyboardInput, handle_sdl_events, keyboard_get_input, wait_until_get_input};
use crate::mtrand::{mt_rand, mt_rand_1};
use libm::{cosf, sinf};
use serde::{Deserialize, Serialize};

/// `starlib_MAX_STARS`.
pub const STARLIB_MAX_STARS: usize = 1000;
pub const MAX_TYPES: u8 = 14;

/// `struct JE_StarType` (file-local type of `starlib.c`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JeStarType {
    pub sp_x: i16,
    pub sp_y: i16,
    pub sp_z: i16,
    pub last_x: i16,
    pub last_y: i16,
}

/// Mutable globals of this module (rule R3); all are file statics.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub temp_x: i32,
    pub temp_y: i32,
    pub run: bool,
    #[serde(with = "crate::serde_big::boxed")]
    pub star: Box<[JeStarType; STARLIB_MAX_STARS]>,
    pub setup: u8,
    pub step_counter: u16,
    pub nsp2: u16,
    pub nsp_var2_inc: i8,
    /// JE: new sprite pointer.
    pub nsp: f32,
    pub nsp_var_inc: f32,
    pub nsp_var_var_inc: f32,
    pub change_time: u16,
    pub do_change: bool,
    pub gray_b: bool,
    pub starlib_speed: i16,
    pub speed_change: i8,
    pub p_color: u8,
    /// `JE_starlib_init`'s local `static JE_boolean initialized`.
    pub je_starlib_init_initialized: bool,
}

impl Default for State {
    fn default() -> Self {
        State {
            temp_x: 0,
            temp_y: 0,
            run: false,
            star: Box::new([JeStarType::default(); STARLIB_MAX_STARS]),
            setup: 0,
            step_counter: 0,
            nsp2: 0,
            nsp_var2_inc: 0,
            nsp: 0.0,
            nsp_var_inc: 0.0,
            nsp_var_var_inc: 0.0,
            change_time: 0,
            do_change: false,
            gray_b: false,
            starlib_speed: 0,
            speed_change: 0,
            p_color: 0,
            je_starlib_init_initialized: false,
        }
    }
}

#[doc(alias = "JE_wackyCol")]
pub fn je_wacky_col(_g: &mut Game) {
    // YKS: Does nothing
}

#[doc(alias = "JE_starlib_init")]
pub fn je_starlib_init(g: &mut Game) {
    if !g.starlib.je_starlib_init_initialized {
        g.starlib.je_starlib_init_initialized = true;

        je_reset_values(g);
        je_change_setup(g, 2);
        g.starlib.do_change = true;

        // RANDOMIZE;
        for x in 0..STARLIB_MAX_STARS {
            g.starlib.star[x].sp_x = (mt_rand(g) % 64000).wrapping_sub(32000) as i16;
            g.starlib.star[x].sp_y = (mt_rand(g) % 40000).wrapping_sub(20000) as i16;
            g.starlib.star[x].sp_z = (x + 1) as i16;
        }
    }
}

#[doc(alias = "JE_resetValues")]
pub fn je_reset_values(g: &mut Game) {
    let s = &mut g.starlib;
    s.nsp2 = 1;
    s.nsp_var2_inc = 1;
    s.nsp_var_inc = 0.1;
    s.nsp_var_var_inc = 0.0001;
    s.nsp = 0.0;
    s.p_color = 32;
    s.starlib_speed = 2;
    s.speed_change = 0;
}

#[doc(alias = "JE_changeSetup")]
pub fn je_change_setup(g: &mut Game, setup_type: u8) {
    g.starlib.step_counter = 0;
    g.starlib.change_time = (mt_rand(g) % 1000) as u16;

    if setup_type > 0 {
        g.starlib.setup = setup_type;
    } else {
        g.starlib.setup = (mt_rand(g) % (u64::from(MAX_TYPES) + 1)) as u8;
    }

    let s = &mut g.starlib;
    if s.setup == 1 {
        s.nsp_var_inc = 0.1;
    }
    if s.nsp_var_inc > 2.2 {
        s.nsp_var_inc = 0.1;
    }
}

/// `nsp2 += nspVar2Inc`, reversing the increment at either end of the range.
fn step_nsp2(s: &mut State) {
    s.nsp2 = (i32::from(s.nsp2) + i32::from(s.nsp_var2_inc)) as u16;
    if s.nsp2 == 65535 || s.nsp2 == 0 {
        s.nsp_var2_inc = s.nsp_var2_inc.wrapping_neg();
    }
}

#[doc(alias = "JE_newStar")]
pub fn je_new_star(g: &mut Game) {
    if g.starlib.setup == 0 {
        g.starlib.temp_x = (mt_rand(g) % 64000).wrapping_sub(32000) as i32;
        g.starlib.temp_y = (mt_rand(g) % 40000).wrapping_sub(20000) as i32;
    } else {
        g.starlib.nsp += g.starlib.nsp_var_inc; // YKS: < lol
        let nsp = g.starlib.nsp;
        match g.starlib.setup {
            1 => {
                g.starlib.temp_x = (sinf(nsp / 30.0) * 20000.0) as i32;
                g.starlib.temp_y = (mt_rand(g) % 40000).wrapping_sub(20000) as i32;
            }
            2 => {
                g.starlib.temp_x = (cosf(nsp) * 20000.0) as i32;
                g.starlib.temp_y = (sinf(nsp) * 20000.0) as i32;
            }
            3 => {
                let k = (nsp / 6.0) as i32 % 200;
                g.starlib.temp_x = ((cosf(nsp * 15.0) * 100.0) as i32).wrapping_mul(k);
                g.starlib.temp_y = ((sinf(nsp * 15.0) * 100.0) as i32).wrapping_mul(k);
            }
            4 => {
                g.starlib.temp_x = (sinf(nsp / 60.0) * 20000.0) as i32;
                let k = (sinf(nsp / 200.0) * 300.0) as i32;
                g.starlib.temp_y = (cosf(nsp) * k as f32 * 100.0) as i32;
            }
            5 => {
                g.starlib.temp_x = (sinf(nsp / 2.0) * 20000.0) as i32;
                let k = (sinf(nsp / 200.0) * 300.0) as i32;
                g.starlib.temp_y = (cosf(nsp) * k as f32 * 100.0) as i32;
            }
            6 => {
                g.starlib.temp_x = (sinf(nsp) * 40000.0) as i32;
                g.starlib.temp_y = (cosf(nsp) * 20000.0) as i32;
            }
            8 => {
                g.starlib.temp_x = (sinf(nsp / 2.0) * 40000.0) as i32;
                g.starlib.temp_y = (cosf(nsp) * 20000.0) as i32;
            }
            7 => {
                g.starlib.temp_x = (mt_rand(g) % 65535) as i32;
                if mt_rand(g).is_multiple_of(2) {
                    g.starlib.temp_y = (cosf(nsp / 80.0) * 10000.0) as i32 + 15000;
                } else {
                    g.starlib.temp_y = 50000 - (cosf(nsp / 80.0) * 13000.0) as i32;
                }
            }
            9 => {
                let s = &mut g.starlib;
                step_nsp2(s);
                let nsp2 = f32::from(s.nsp2);
                s.temp_x = (cosf(sinf(nsp2 / 10.0) + (nsp / 500.0)) * 32000.0) as i32;
                s.temp_y = (sinf(cosf(nsp2 / 10.0) + (nsp / 500.0)) * 30000.0) as i32;
            }
            10 => {
                let s = &mut g.starlib;
                step_nsp2(s);
                let nsp2 = f32::from(s.nsp2);
                s.temp_x = (cosf(sinf(nsp2 / 5.0) + (nsp / 100.0)) * 32000.0) as i32;
                s.temp_y = (sinf(cosf(nsp2 / 5.0) + (nsp / 100.0)) * 30000.0) as i32;
            }
            11 => {
                let s = &mut g.starlib;
                step_nsp2(s);
                let nsp2 = f32::from(s.nsp2);
                s.temp_x = (cosf(sinf(nsp2 / 1000.0) + (nsp / 2.0)) * 32000.0) as i32;
                s.temp_y = (sinf(cosf(nsp2 / 1000.0) + (nsp / 2.0)) * 30000.0) as i32;
            }
            12 => {
                if nsp != 0.0 {
                    let s = &mut g.starlib;
                    step_nsp2(s);
                    let nsp2 = f32::from(s.nsp2);
                    let damp = nsp.abs().sqrt() / 10.0 + 1.0;
                    s.temp_x = (cosf(sinf(nsp2 / 2.0) / damp + (nsp2 / 100.0)) * 32000.0) as i32;
                    s.temp_y = (sinf(cosf(nsp2 / 2.0) / damp + (nsp2 / 100.0)) * 30000.0) as i32;
                }
            }
            13 => {
                if nsp != 0.0 {
                    let s = &mut g.starlib;
                    step_nsp2(s);
                    let nsp2 = f32::from(s.nsp2);
                    s.temp_x = (cosf(sinf(nsp2 / 10.0) / 2.0 + (nsp / 20.0)) * 32000.0) as i32;
                    s.temp_y = (sinf(sinf(nsp2 / 11.0) / 2.0 + (nsp / 20.0)) * 30000.0) as i32;
                }
            }
            14 => {
                let s = &mut g.starlib;
                s.nsp2 = (i32::from(s.nsp2) + i32::from(s.nsp_var2_inc)) as u16;
                let nsp2 = s.nsp2;
                s.temp_x = ((sinf(nsp) + cosf(f32::from(nsp2) / 1000.0) * 3.0) * 12000.0) as i32;
                s.temp_y = (cosf(nsp) * 10000.0) as i32 + i32::from(nsp2);
            }
            _ => {}
        }
    }
}

// Key modifiers and scancodes read by `starLibMain` (SDL2 values).
const KMOD_SHIFT: u32 = 0x0003;
const KMOD_CTRL: u32 = 0x00c0;
const KMOD_ALT: u32 = 0x0300;
const KMOD_GUI: u32 = 0x0c00;
const SC_C: u32 = 6;
const SC_P: u32 = 19;
const SC_S: u32 = 22;
const SC_X: u32 = 27;
const SC_1: u32 = 30;
const SC_0: u32 = 39;
const SC_ESCAPE: u32 = 41;
const SC_MINUS: u32 = 45;
const SC_EQUALS: u32 = 46;
const SC_LEFTBRACKET: u32 = 47;
const SC_RIGHTBRACKET: u32 = 48;
const SC_GRAVE: u32 = 53;

/// `KEY_COMBO(mod, scancode)`: the scancode with the pressed modifier groups
/// in the upper 16 bits.
fn key_combo(m: u32, scancode: u32) -> u32 {
    let mut mods = 0;
    for group in [KMOD_SHIFT, KMOD_CTRL, KMOD_ALT, KMOD_GUI] {
        if m & group != 0 {
            mods |= group;
        }
    }
    scancode | (mods << 16)
}

/// One frame of the star field: erases and redraws every star on `VGAScreen`,
/// then handles one key. Returns whether a key was read into
/// `keyboard_input`.
#[doc(alias = "starLibMain")]
pub fn star_lib_main(g: &mut Game, keyboard_input: &mut KeyboardInput) -> Flow<bool> {
    je_wacky_col(g);

    g.starlib.gray_b = false;

    g.starlib.starlib_speed = g
        .starlib
        .starlib_speed
        .wrapping_add(i16::from(g.starlib.speed_change));

    for s in 0..STARLIB_MAX_STARS {
        let star = g.starlib.star[s];

        // Erase the star's previous position.
        let off = i32::from(star.last_x) + i32::from(star.last_y) * 320;
        if (640..320 * 200 - 640).contains(&off) {
            let surf = &mut g.video.vga_mut().pixels;
            let o = off as usize;
            for p in [
                o,
                o - 1,
                o + 1,
                o - 2,
                o + 2,
                o - 320,
                o + 320,
                o - 640,
                o + 640,
            ] {
                surf[p] = 0;
            }
        }

        // Move it.
        let mut temp_z = star.sp_z;
        g.starlib.temp_x = i32::from(star.sp_x) / i32::from(temp_z) + 160;
        g.starlib.temp_y = i32::from(star.sp_y) / i32::from(temp_z) + 100;
        temp_z = temp_z.wrapping_sub(g.starlib.starlib_speed);

        let (temp_x, temp_y) = (g.starlib.temp_x, g.starlib.temp_y);
        if temp_z <= 0 || temp_y == 0 || temp_y > 198 || !(1..=318).contains(&temp_x) {
            // Out of range: replace it.
            g.starlib.star[s].sp_z = 500;

            je_new_star(g);

            g.starlib.star[s].sp_x = g.starlib.temp_x as i16;
            g.starlib.star[s].sp_y = g.starlib.temp_y as i16;
        } else {
            let st = &mut g.starlib.star[s];
            st.last_x = temp_x as i16;
            st.last_y = temp_y as i16;
            st.sp_z = temp_z;

            let off = temp_x + temp_y * 320;

            let mut temp_col: u8 = if g.starlib.gray_b {
                (temp_z >> 1) as u8
            } else {
                g.starlib.p_color.wrapping_add(((temp_z >> 4) & 31) as u8)
            };

            if (640..320 * 200 - 640).contains(&off) {
                let surf = &mut g.video.vga_mut().pixels;
                let o = off as usize;
                surf[o] = temp_col;

                temp_col = temp_col.wrapping_add(72);
                for p in [o - 1, o + 1, o - 320, o + 320] {
                    surf[p] = temp_col;
                }

                temp_col = temp_col.wrapping_add(72);
                for p in [o - 2, o + 2, o - 640, o + 640] {
                    surf[p] = temp_col;
                }
            }
        }
    }

    push_joysticks_as_keyboard(g);
    handle_sdl_events(g)?;

    let got_keyboard_input = keyboard_get_input(g, Some(keyboard_input));

    if got_keyboard_input {
        let combo = key_combo(
            u32::from(keyboard_input.r#mod),
            u32::from(keyboard_input.scancode),
        );
        let shift = |sc: u32| key_combo(KMOD_SHIFT, sc);
        match combo {
            c if c == shift(SC_EQUALS) => {
                g.starlib.starlib_speed = g.starlib.starlib_speed.wrapping_add(1);
                g.starlib.speed_change = 0;
            }
            SC_MINUS => {
                g.starlib.starlib_speed = g.starlib.starlib_speed.wrapping_sub(1);
                g.starlib.speed_change = 0;
            }
            // 1-9 select setups 1-9, 0 selects 10.
            c if (SC_1..=SC_0).contains(&c) => je_change_setup(g, (c - SC_1 + 1) as u8),
            // Shift+1-4 select setups 11-14.
            c if (shift(SC_1)..=shift(SC_1 + 3)).contains(&c) => {
                je_change_setup(g, (c - shift(SC_1) + 11) as u8);
            }
            c if c == SC_C || c == shift(SC_C) => je_reset_values(g),
            c if c == SC_S || c == shift(SC_S) => {
                g.starlib.nsp_var_var_inc = mt_rand_1(g) * 0.01 - 0.005;
            }
            c if c == SC_X || c == shift(SC_X) || c == SC_ESCAPE => g.starlib.run = false,
            SC_LEFTBRACKET => g.starlib.p_color = g.starlib.p_color.wrapping_sub(1),
            SC_RIGHTBRACKET => g.starlib.p_color = g.starlib.p_color.wrapping_add(1),
            c if c == shift(SC_LEFTBRACKET) => {
                g.starlib.p_color = g.starlib.p_color.wrapping_sub(72);
            }
            c if c == shift(SC_RIGHTBRACKET) => {
                g.starlib.p_color = g.starlib.p_color.wrapping_add(72);
            }
            SC_GRAVE => g.starlib.do_change = !g.starlib.do_change,
            c if c == SC_P || c == shift(SC_P) => wait_until_get_input(g)?,
            _ => {}
        }
    }

    if g.starlib.do_change {
        g.starlib.step_counter = g.starlib.step_counter.wrapping_add(1);
        if g.starlib.step_counter > g.starlib.change_time {
            je_change_setup(g, 0);
        }
    }

    if mt_rand(g) % 1000 == 1 {
        g.starlib.nsp_var_var_inc = mt_rand_1(g) * 0.01 - 0.005;
    }

    g.starlib.nsp_var_inc += g.starlib.nsp_var_var_inc;

    Ok(got_keyboard_input)
}
