// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `jukebox.c` / `jukebox.h`. The file has no mutable
//! globals.

use crate::Game;
use crate::font::{Font, FontAlignment, draw_font_hv_aligned};
use crate::game::Flow;
use crate::keyboard::{INPUT_NO_MOTION, KeyboardInput, mouse_get_input, wait_until_elapsed};
use crate::loudness::{audio, play_song, restart_song, set_volume, stop_song};
use crate::mtrand;
use crate::musmast::{MUSIC_NUM, MUSIC_TITLE};
use crate::nortsong::{je_play_sample_num, set_frame_count};
use crate::palette::{Color, init_step_fade_palette, init_step_fade_solid, step_fade_palette};
use crate::sndmast::{SOUND_COUNT, SOUND_TITLE};
use crate::starlib::{je_starlib_init, star_lib_main};
use crate::video::je_show_vga;
use serde::{Deserialize, Serialize};

/// Mutable globals of this module (rule R3).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {}

const KMOD_SHIFT: u32 = 0x0003;
const KMOD_CTRL: u32 = 0x00c0;
const KMOD_ALT: u32 = 0x0300;
const KMOD_GUI: u32 = 0x0c00;

const SC_F: u32 = 9;
const SC_N: u32 = 17;
const SC_Q: u32 = 20;
const SC_R: u32 = 21;
const SC_S: u32 = 22;
const SC_T: u32 = 23;
const SC_V: u32 = 25;
const SC_RETURN: u32 = 40;
const SC_ESCAPE: u32 = 41;
const SC_SPACE: u32 = 44;
const SC_SEMICOLON: u32 = 51;
const SC_COMMA: u32 = 54;
const SC_PERIOD: u32 = 55;
const SC_SLASH: u32 = 56;
const SC_RIGHT: u32 = 79;
const SC_LEFT: u32 = 80;
const SC_DOWN: u32 = 81;
const SC_UP: u32 = 82;

/// `KEY_COMBO(mod, scancode)`.
fn key_combo(m: u32, scancode: u32) -> u32 {
    let mut mods = 0;
    for group in [KMOD_SHIFT, KMOD_CTRL, KMOD_ALT, KMOD_GUI] {
        if m & group != 0 {
            mods |= group;
        }
    }
    scancode | (mods << 16)
}

/// A NUL-terminated title's bytes.
fn title(t: &[u8]) -> &[u8] {
    &t[..t.iter().position(|&c| c == 0).unwrap_or(t.len())]
}

/// `snprintf(buffer, 60, "%d %s", n, title)`.
fn caption(n: i64, t: &[u8]) -> Vec<u8> {
    let mut b = format!("{n} ").into_bytes();
    b.extend_from_slice(title(t));
    b.truncate(59);
    b
}

/// The jukebox (FKA Setup.jukeboxGo): plays the songs over the star field
/// until ESC, Q or a mouse click.
pub fn jukebox(g: &mut Game) -> Flow {
    let mut trigger_quit = false; // the user wants to quit
    let mut quitting = false;

    let mut hide_text = false;

    let mut fade_looped_songs = true;
    let mut fading_song = false;
    let mut stopped = false;

    let mut fx = false;
    let mut fx_num: i32 = 0;

    let mut palette_fade_steps: i32 = 15;

    let mut diff = [[0i32; 3]; 256];
    let palette = g.vga_palette.vga_palette;
    init_step_fade_palette(g, &mut diff, &palette, 0, 255);

    je_starlib_init(g);

    let mut fade_volume = i32::from(g.nortsong.tyr_music_volume);

    loop {
        if !stopped && !g.loudness.audio_disabled {
            if audio(g).lds.songlooped && fade_looped_songs {
                fading_song = true;
            }

            if fading_song {
                if fade_volume > 5 {
                    fade_volume -= 2;
                } else {
                    fade_volume = i32::from(g.nortsong.tyr_music_volume);

                    fading_song = false;
                }

                let fx_volume = g.nortsong.fx_volume as u8;
                set_volume(g, fade_volume as u8, fx_volume);
            }

            let (playing, songlooped) = {
                let a = audio(g);
                (a.lds.playing, a.lds.songlooped)
            };
            if !playing || (songlooped && fade_looped_songs && !fading_song) {
                let song = (mtrand::mt_rand(g) % MUSIC_NUM as u64) as u32;
                play_song(g, song)?;
            }
        }

        set_frame_count(g, 1);

        let seg = g
            .video
            .vga_screen_seg
            .expect("VGAScreenSeg used before init_video");
        g.video.screen_mut(seg).fill_rect(None, 0);

        let mut keyboard_input = KeyboardInput::default();

        let got_keyboard_input = star_lib_main(g, &mut keyboard_input)?;

        if !hide_text {
            let buffer = if fx {
                caption(i64::from(fx_num) + 1, &SOUND_TITLE[fx_num as usize])
            } else {
                let n = g.loudness.song_playing;
                caption(i64::from(n) + 1, &MUSIC_TITLE[n as usize])
            };

            let vga = g.video.vga_screen_id();
            let x = g.video.vga().w / 2;

            let (small, center) = (Font::Small, FontAlignment::Center);
            draw_font_hv_aligned(
                g,
                vga,
                x,
                170,
                b"Press ESC to quit the jukebox.",
                small,
                center,
                1,
                0,
            );
            draw_font_hv_aligned(
                g,
                vga,
                x,
                180,
                b"Arrow keys change the song being played.",
                small,
                center,
                1,
                0,
            );
            draw_font_hv_aligned(g, vga, x, 190, &buffer, small, center, 1, 4);
        }

        if palette_fade_steps > 0 {
            step_fade_palette(g, &mut diff, palette_fade_steps, 0, 255);
            palette_fade_steps -= 1;
        }

        je_show_vga(g);

        wait_until_elapsed(g)?;

        // Quit on mouse click.
        if mouse_get_input(g, INPUT_NO_MOTION, None) {
            trigger_quit = true;
        }

        if got_keyboard_input {
            let shift = |sc: u32| key_combo(KMOD_SHIFT, sc);
            let combo = key_combo(
                u32::from(keyboard_input.r#mod),
                u32::from(keyboard_input.scancode),
            );
            match combo {
                c if c == SC_ESCAPE || c == SC_Q || c == shift(SC_Q) => trigger_quit = true,

                SC_SPACE => hide_text = !hide_text,

                c if c == SC_F || c == shift(SC_F) => fading_song = !fading_song,
                c if c == SC_N || c == shift(SC_N) => fade_looped_songs = !fade_looped_songs,
                // Not implemented upstream.
                c if c == SC_V || c == shift(SC_V) || c == SC_T || c == shift(SC_T) => {}

                SC_SLASH => fx = !fx,
                SC_COMMA => {
                    if fx {
                        fx_num -= 1;
                        if fx_num < 0 {
                            fx_num = SOUND_COUNT as i32 - 1;
                        }
                    }
                }
                SC_PERIOD => {
                    if fx {
                        fx_num += 1;
                        if fx_num >= SOUND_COUNT as i32 {
                            fx_num = 0;
                        }
                    }
                }
                SC_SEMICOLON => {
                    if fx {
                        je_play_sample_num(g, (fx_num + 1) as u8);
                    }
                }

                SC_LEFT | SC_UP => {
                    let n = g.loudness.song_playing;
                    play_song(g, if n > 0 { n } else { MUSIC_NUM as u32 } - 1)?;
                    stopped = false;
                }
                SC_RETURN | SC_RIGHT | SC_DOWN => {
                    let n = g.loudness.song_playing;
                    play_song(g, (n + 1) % MUSIC_NUM as u32)?;
                    stopped = false;
                }
                c if c == SC_S || c == shift(SC_S) => {
                    stop_song(g);
                    stopped = true;
                }
                c if c == SC_R || c == shift(SC_R) => {
                    restart_song(g);
                    stopped = false;
                }

                _ => {}
            }
        }

        // the user wants to quit: start the fade-out
        if trigger_quit && !quitting {
            palette_fade_steps = 15;

            let black = Color {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            };
            init_step_fade_solid(g, &mut diff, black, 0, 255);

            quitting = true;
        }

        // the fade-out has finished: quit
        if quitting && palette_fade_steps == 0 {
            break;
        }
    }

    let (music, fx_volume) = (g.nortsong.tyr_music_volume, g.nortsong.fx_volume);
    set_volume(g, music as u8, fx_volume as u8);

    Ok(())
}
