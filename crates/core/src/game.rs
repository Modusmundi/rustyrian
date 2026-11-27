// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) rustyrian contributors

//! `Game`: every module's `State` plus the platform (rule R3), and the `Halt`
//! control flow that replaces `exit()` (rule R9).

use crate::platform::Platform;
use crate::*;
use std::sync::{Arc, Mutex};

/// `exit(code)` in upstream; propagated with `?` up to `main`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Halt {
    pub code: i32,
}

/// The result type of every function that can (transitively) call `exit`.
pub type Flow<T = ()> = Result<T, Halt>;

/// `exit(EXIT_FAILURE)`.
pub const EXIT_FAILURE: i32 = 1;
/// `exit(EXIT_SUCCESS)`.
pub const EXIT_SUCCESS: i32 = 0;

macro_rules! game_states {
    ($($m:ident),* $(,)?) => {
        /// The whole game: the platform and the state of every ported module.
        pub struct Game {
            pub platform: Box<dyn Platform>,
            /// State shared with the audio callback (`loudness.c`), which runs on
            /// the platform's audio thread.
            pub audio: Arc<Mutex<loudness::AudioState>>,
            /// The libc `rand()` stream (rule R7).
            pub crand: crand::State,
            $(pub $m: $m::State,)*
        }

        impl Game {
            /// A game in the state of a freshly started process: every global
            /// at its C initial value.
            pub fn new(platform: Box<dyn Platform>) -> Box<Game> {
                Box::new(Game {
                    platform,
                    audio: Arc::new(Mutex::new(loudness::AudioState::default())),
                    crand: crand::State::default(),
                    $($m: $m::State::default(),)*
                })
            }
        }

        /// Every module state except the platform and audio, for snapshots
        /// and `--sim-digest`.
        #[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
        pub struct Snapshot {
            pub crand: crand::State,
            $(pub $m: $m::State,)*
        }

        impl Game {
            pub fn snapshot(&self) -> Snapshot {
                Snapshot {
                    crand: self.crand.clone(),
                    $($m: self.$m.clone(),)*
                }
            }
        }
    };
}

game_states!(
    animlib,
    arg_parse,
    backgrnd,
    config,
    config_file,
    demo,
    destruct,
    editship,
    episodes,
    file,
    font,
    fonthand,
    game_menu,
    helptext,
    joystick,
    jukebox,
    keyboard,
    lds_play,
    logging,
    loudness,
    lvllib,
    lvlmast,
    mainint,
    memreader,
    memwriter,
    menus,
    mouse,
    mtrand,
    musmast,
    network,
    nortsong,
    nortvars,
    opentyr,
    opl,
    palette,
    params,
    pcxload,
    pcxmast,
    picload,
    player,
    shots,
    sndmast,
    sprite,
    starlib,
    swar,
    tyrian2,
    varz,
    vga256d,
    vga_palette,
    video,
    video_scale,
    video_scale_hqnx,
    xmas,
);
