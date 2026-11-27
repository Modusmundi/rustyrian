// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) rustyrian contributors

//! Faithful Rust port of the OpenTyrian game logic. See `porting/RULES.md`.
//!
//! One module per upstream C file (rule R1); every C global lives in that
//! module's `State`, aggregated in [`game::Game`] (rule R3).
#![forbid(unsafe_code)]

pub mod cconv;
pub mod crand;
pub mod game;
pub mod platform;
pub mod sdl;
pub mod serde_big;

pub mod animlib;
pub mod arg_parse;
pub mod backgrnd;
pub mod config;
pub mod config_file;
pub mod demo;
pub mod destruct;
pub mod editship;
pub mod episodes;
pub mod file;
pub mod font;
pub mod fonthand;
pub mod game_menu;
pub mod helptext;
pub mod joystick;
pub mod jukebox;
pub mod keyboard;
pub mod lds_play;
pub mod logging;
pub mod loudness;
pub mod lvllib;
pub mod lvlmast;
pub mod mainint;
pub mod memreader;
pub mod memwriter;
pub mod menus;
pub mod mouse;
pub mod mtrand;
pub mod musmast;
pub mod network;
pub mod nortsong;
pub mod nortvars;
pub mod opentyr;
pub mod opl;
pub mod palette;
pub mod params;
pub mod pcxload;
pub mod pcxmast;
pub mod picload;
pub mod player;
pub mod shots;
pub mod sndmast;
pub mod sprite;
pub mod starlib;
pub mod swar;
pub mod tyrian2;
pub mod varz;
pub mod vga256d;
pub mod vga_palette;
pub mod video;
pub mod video_scale;
pub mod video_scale_hqnx;
pub mod xmas;

pub use game::{EXIT_FAILURE, EXIT_SUCCESS, Flow, Game, Halt};
