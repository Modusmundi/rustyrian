// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `starlib.c` / `starlib.h`.

use crate::Game;
use crate::mtrand::mt_rand;
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
