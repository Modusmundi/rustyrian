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
            /// The libc `rand()` stream (rule R7). Shared because the audio
            /// callback draws from it too (`opl.c:317`, under `audio`), as do
            /// main-thread callers (`destruct.c`, `xmas.c`); C has one stream.
            /// Lock order: `audio` before `crand`; never hold `crand` across
            /// another call.
            pub crand: Arc<Mutex<crand::State>>,
            $(pub $m: $m::State,)*
        }

        impl Game {
            /// A game in the state of a freshly started process: every global
            /// at its C initial value.
            pub fn new(platform: Box<dyn Platform>) -> Box<Game> {
                Box::new(Game {
                    platform,
                    audio: Arc::new(Mutex::new(loudness::AudioState::default())),
                    crand: Arc::new(Mutex::new(crand::State::default())),
                    $($m: $m::State::default(),)*
                })
            }
        }

        /// Every module state, including the audio-shared state, but not the
        /// platform: fixtures (`--snapshot-at`) and state comparisons.
        #[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
        pub struct Snapshot {
            pub crand: crand::State,
            pub audio: loudness::AudioState,
            $(pub $m: $m::State,)*
        }

        /// Every module state, borrowed apart from the platform (see
        /// [`Game::split`]): lets a `FrameView` snapshot closure read the
        /// states while the platform is called.
        pub struct States<'a> {
            pub crand: &'a Arc<Mutex<crand::State>>,
            pub audio: &'a Arc<Mutex<loudness::AudioState>>,
            $(pub $m: &'a $m::State,)*
        }

        impl States<'_> {
            pub fn snapshot(&self) -> Snapshot {
                Snapshot {
                    crand: self.crand.lock().unwrap_or_else(|e| e.into_inner()).clone(),
                    audio: self.audio.lock().unwrap_or_else(|e| e.into_inner()).clone(),
                    $($m: self.$m.clone(),)*
                }
            }
        }

        impl Game {
            /// The platform and every other state, borrowed disjointly.
            pub fn split(&mut self) -> (&mut dyn Platform, States<'_>) {
                (
                    &mut *self.platform,
                    States {
                        crand: &self.crand,
                        audio: &self.audio,
                        $($m: &self.$m,)*
                    },
                )
            }

            pub fn snapshot(&self) -> Snapshot {
                States {
                    crand: &self.crand,
                    audio: &self.audio,
                    $($m: &self.$m,)*
                }
                .snapshot()
            }

            /// Replaces every module state with `s` (the platform is kept).
            pub fn restore(&mut self, s: Snapshot) {
                *self.crand.lock().unwrap_or_else(|e| e.into_inner()) = s.crand;
                *self.audio.lock().unwrap_or_else(|e| e.into_inner()) = s.audio;
                $(self.$m = s.$m;)*
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

/// The `--sim-digest` value of a frame: FNV-1a 64 over the postcard
/// serialisation of the simulation state that two netplay peers must agree on
/// (and that both runners, `ot-ref-run` and `rs-run`, digest identically).
///
/// In order:
/// - `mtrand.mt_global`: the shared Mersenne Twister; every gameplay draw
///   comes from it, so any divergence in simulation call order shows here.
/// - `player.player`: both ships (position, items, cash, armor, shields,
///   sidekicks).
/// - `shots.player_shot_data`, `shots.shot_avail`: player shots in flight.
/// - `varz.enemy`, `varz.enemy_avail`, `varz.enemy_shot`,
///   `varz.enemy_shot_avail`: enemies and their shots.
/// - `varz.cur_loc`, `varz.event_loc`, `varz.level_end`, `varz.end_level`,
///   `varz.total_enemy`, `varz.enemy_killed`: level progress.
///
/// Peer-local and UI state (menus, palette, screens, audio, network
/// bookkeeping, the libc `rand()` stream) is deliberately left out.
pub fn sim_digest(s: &States<'_>) -> u64 {
    let v = s.varz;
    let state = (
        &s.mtrand.mt_global,
        &s.player.player,
        &s.shots.player_shot_data[..],
        &s.shots.shot_avail[..],
        &v.enemy[..],
        &v.enemy_avail[..],
        &v.enemy_shot[..],
        &v.enemy_shot_avail[..],
        (v.cur_loc, v.event_loc, v.level_end, v.end_level),
        (v.total_enemy, v.enemy_killed),
    );
    let bytes = postcard::to_stdvec(&state).expect("simulation state always serialises");
    bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}
