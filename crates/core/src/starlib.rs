// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `starlib.c` / `starlib.h`.

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
