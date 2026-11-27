// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `tyrian2.c` / `tyrian2.h`.

use crate::Game;
use crate::config::{
    DIFFICULTY_MANIACAL, DIFFICULTY_NORMAL, DIFFICULTY_SUICIDE, DIFFICULTY_ZINGLON,
};
use crate::file::{self, File};
use crate::game::{EXIT_FAILURE, Flow, Halt};
use crate::keyboard::{INPUT_NO_MOTION, KeyboardInput};
use crate::lvlmast::EVENT_MAXIMUM;
use crate::mtrand::mt_rand;
use crate::sprite::FONT_SHAPES;
use crate::varz::{ENEMY_SHOT_MAX, JeEventRecType, SA_NONE, Sprite2Ref};
use crate::video::ScreenId;
use serde::{Deserialize, Serialize};

/// `boss_bar_t`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BossBarT {
    pub link_num: u8,
    pub armor: u8,
    pub color: u8,
}

/// Mutable globals of `tyrian2.c` (rule R3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub boss_bar: [BossBarT; 2],
    pub quit: bool,
    pub load_level_ok: bool,
    #[serde(with = "crate::serde_big::boxed")]
    pub event_rec: Box<[JeEventRecType; EVENT_MAXIMUM]>,
    pub level_enemy_max: u16,
    pub level_enemy_frequency: u16,
    #[serde(with = "crate::serde_big")]
    pub level_enemy: [u16; 40],
    pub temp_str: [u8; 31],
    pub item_avail: [[u8; 10]; 9],
    pub item_avail_max: [u8; 9],
}

impl Default for State {
    fn default() -> Self {
        State {
            boss_bar: [BossBarT::default(); 2],
            quit: false,
            load_level_ok: false,
            event_rec: vec![JeEventRecType::default(); EVENT_MAXIMUM]
                .into_boxed_slice()
                .try_into()
                .unwrap_or_else(|_| unreachable!("length is EVENT_MAXIMUM")),
            level_enemy_max: 0,
            level_enemy_frequency: 0,
            level_enemy: [0; 40],
            temp_str: [0; 31],
            item_avail: [[0; 10]; 9],
            item_avail_max: [0; 9],
        }
    }
}

/// `SDL_SCANCODE_SCROLLLOCK`.
const SDL_SCANCODE_SCROLLLOCK: u16 = 71;

/// `SDL_LOG_CATEGORY_APPLICATION` (upstream's `logWarn`).
const LOG_CATEGORY: i32 = 0;

/// The C string held in `b` (up to its first NUL).
fn c_str(b: &[u8]) -> &[u8] {
    &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]
}

/// `blit_enemy(surface, i, x_offset, y_offset, sprite_offset)`: draws enemy
/// `i`'s current animation frame, offset by `sprite_offset`.
pub fn blit_enemy(
    g: &mut Game,
    surface: ScreenId,
    i: u32,
    x_offset: i32,
    y_offset: i32,
    sprite_offset: i32,
) {
    let e = g.varz.enemy[i as usize];
    let Some(sprite2s) = e.sprite2s else {
        return;
    };

    let x = i32::from(e.ex) + x_offset + i32::from(g.backgrnd.temp_map_x_ofs);
    let y = i32::from(e.ey) + y_offset;
    let index = u32::from(e.egr[usize::from(e.enemycycle) - 1]).wrapping_add(sprite_offset as u32);

    if e.filter != 0 {
        crate::sprite::blit_sprite2_filter(g, surface, x, y, sprite2s, index, e.filter);
    } else {
        crate::sprite::blit_sprite2(g, surface, x, y, sprite2s, index);
    }
}

/// `readEpisodeString(file, dst, size)` with `size = dst.len()`.
#[doc(alias = "readEpisodeString")]
pub fn read_episode_string(g: &mut Game, file: &mut File, dst: &mut [u8]) -> Flow {
    crate::helptext::read_encrypted_string(file, Some(dst));

    if file.error {
        let message = format!(
            "Failed to read from file '{}': {}",
            String::from_utf8_lossy(c_str(&g.episodes.episode_filename)),
            file::file_get_error(file)
        );
        crate::logging::log_fatal(g, &message);
        return Err(Halt { code: EXIT_FAILURE });
    }
    Ok(())
}

/// Waits for a key other than SCROLL LOCK, or a mouse button.
fn wait_for_key_not_scroll_lock(g: &mut Game) -> Flow {
    loop {
        crate::keyboard::wait_until_has_input(g, INPUT_NO_MOTION)?;

        let mut keyboard_input = KeyboardInput::default();
        if (crate::keyboard::keyboard_get_input(g, Some(&mut keyboard_input))
            && keyboard_input.scancode != SDL_SCANCODE_SCROLLLOCK)
            || crate::keyboard::mouse_get_input(g, INPUT_NO_MOTION, None)
        {
            return Ok(());
        }
    }
}

#[doc(alias = "newSuperTyrianGame")]
pub fn new_super_tyrian_game(g: &mut Game) -> Flow {
    // SuperTyrian

    g.config.initial_difficulty = if g.keyboard.keysactive[usize::from(SDL_SCANCODE_SCROLLLOCK)] {
        DIFFICULTY_SUICIDE
    } else {
        DIFFICULTY_ZINGLON
    };

    let vga = g.video.vga_screen_id();
    crate::video::je_clr256(g, vga);
    let out = |g: &mut Game, y: i32, s: &str| {
        crate::fonthand::je_out_text(g, vga, 10, y, s.as_bytes(), 15, 4);
    };
    out(g, 10, "Cheat codes have been disabled.");
    if g.config.initial_difficulty == DIFFICULTY_ZINGLON {
        out(g, 20, "Difficulty level has been set to Lord of Game.");
    } else {
        out(g, 20, "Difficulty level has been set to Suicide.");
    }
    out(
        g,
        30,
        "It is imperative that you discover the special codes.",
    );
    if g.config.initial_difficulty == DIFFICULTY_ZINGLON {
        out(
            g,
            40,
            "(Next time, for an easier challenge hold down SCROLL LOCK.)",
        );
    }
    out(g, 60, "Prepare to play...");

    // snprintf(buf, 10 + 1 + 15 + 1, "%s %s", miscTextB[4], pName[0])
    let mut buf = [0u8; 10 + 1 + 15 + 1];
    let text = [
        c_str(&g.helptext.misc_text_b[4]),
        b" ",
        c_str(&g.helptext.p_name[0]),
    ]
    .concat();
    let n = text.len().min(buf.len() - 1);
    buf[..n].copy_from_slice(&text[..n]);
    let x = crate::fonthand::je_font_center(g, &buf, FONT_SHAPES as u32);
    crate::fonthand::je_d_string(g, vga, x, 110, &buf, FONT_SHAPES as u32);

    crate::loudness::play_song(g, 16)?;
    crate::nortsong::je_play_sample_num(g, crate::sndmast::V_DANGER);

    crate::video::je_show_vga(g);
    let colors = g.palette.colors;
    crate::palette::fade_palette(g, &colors, 10, 0, 255)?;

    wait_for_key_not_scroll_lock(g)?;

    crate::episodes::je_init_episode(g, 1)?;
    g.params.constant_die = false;
    g.config.super_tyrian = true;
    g.config.one_player_action = true;
    g.varz.game_loaded = true;
    g.config.difficulty_level = g.config.initial_difficulty;

    let p = &mut g.player.player[0];
    p.cash = 0;

    p.items.ship = 13; // The Stalker 21.126
    p.items.weapon[crate::player::FRONT_WEAPON].id = 39; // Atomic RailGun

    crate::palette::fade_black(g, 10)
}

pub fn intro_logos(g: &mut Game) -> Flow {
    g.varz.move_tyrian_logo_up = true;

    g.video.vga_mut().fill_rect(None, 0);

    crate::palette::fade_white(g, 25)?;

    for (pic, steps) in [(10, 25), (12, 10)] {
        let vga = g.video.vga_screen_id();
        crate::picload::je_load_pic(g, vga, pic, false)?;
        crate::video::je_show_vga(g);

        let colors = g.palette.colors;
        crate::palette::fade_palette(g, &colors, steps, 0, 255)?;

        crate::nortsong::set_frame_count(g, 200);
        crate::keyboard::wait_until_get_input_or_elapsed(g)?;

        crate::palette::fade_black(g, 10)?;
    }
    Ok(())
}

#[doc(alias = "JE_readTextSync")]
pub fn je_read_text_sync(_g: &mut Game) {
    // this function seems to be unnecessary
}

/// The x/y acceleration step of `JE_drawEnemy`: steers velocity `v` towards
/// the player by one, at most to `accel - 89`. C compares `accel - 89u`
/// (unsigned) with `mt_rand() % 11`, drawing only when `accel` is non-zero.
fn steer(g: &mut Game, accel: u8, v: i8, player_ahead: bool) -> i8 {
    if accel == 0 || u64::from(u32::from(accel).wrapping_sub(89)) <= mt_rand(g) % 11 {
        return v;
    }
    let limit = i32::from(accel) - 89;
    if player_ahead {
        if i32::from(v) < limit {
            return v.wrapping_add(1);
        }
    } else if v >= 0 || -i32::from(v) < limit {
        return v.wrapping_sub(1);
    }
    v
}

/// `(target + 25) - from - tempMapXOfs - 4` and `target - from`, each made
/// non-zero, and the larger magnitude, as `JE_integer`s.
fn aim(target_x: i32, target_y: i32, from_x: i16, from_y: i16, map_x_ofs: u16) -> (i16, i16, i16) {
    let mut aim_x = (target_x + 25 - i32::from(from_x) - i32::from(map_x_ofs) - 4) as i16;
    if aim_x == 0 {
        aim_x = 1;
    }
    let mut aim_y = (target_y - i32::from(from_y)) as i16;
    if aim_y == 0 {
        aim_y = 1;
    }
    let max_mag_aim = i32::from(aim_x).abs().max(i32::from(aim_y).abs()) as i16;
    (aim_x, aim_y, max_mag_aim)
}

/// `roundf((float)a / max * scale)`.
fn aimed(a: i16, max: i16, scale: u8) -> f32 {
    (f32::from(a) / f32::from(max) * f32::from(scale)).round()
}

/// The x/y circular-motion step of `JE_drawEnemy` on (velocity, change,
/// counter, counter max, reversal point, add).
fn circle(v: &mut i8, cc: &mut i8, ccw: &mut i8, ccwmax: u8, rev: &mut i8, add: &mut i16) {
    if *cc == 0 {
        return;
    }
    *ccw = ccw.wrapping_sub(1);
    if *ccw > 0 {
        return;
    }
    let flip = |cc: &mut i8, rev: &mut i8, add: &mut i16| {
        *cc = cc.wrapping_neg();
        *rev = rev.wrapping_neg();
        *add = add.wrapping_neg();
    };
    if *v == *rev {
        flip(cc, rev, add);
    } else {
        *v = (i32::from(*v) + i32::from(*add)) as i8;
        *ccw = ccwmax as i8;
        if *v == *rev {
            flip(cc, rev, add);
        }
    }
}

/// `JE_drawEnemy(enemyOffset)`: moves, draws, fires and launches the 25
/// enemies before `enemy_offset` (it actually does a whole lot more than
/// just drawing).
#[doc(alias = "JE_drawEnemy")]
pub fn je_draw_enemy(g: &mut Game, enemy_offset: i32) {
    g.player.player[0].x = g.player.player[0].x.wrapping_sub(25);

    for i in enemy_offset - 25..enemy_offset {
        if g.varz.enemy_avail[i as usize] != 1 {
            draw_one_enemy(g, enemy_offset, i as usize);
        }
    }

    g.player.player[0].x = g.player.player[0].x.wrapping_add(25);
}

/// One live enemy of `JE_drawEnemy`; returning is `goto draw_enemy_end`.
fn draw_one_enemy(g: &mut Game, enemy_offset: i32, i: usize) {
    let map_x_ofs = g.backgrnd.temp_map_x_ofs;
    g.varz.enemy[i].mapoffset = map_x_ofs;

    let e = g.varz.enemy[i];
    let px = g.player.player[0].x;
    let exc = steer(g, e.xaccel, e.exc, px > i32::from(e.ex));
    g.varz.enemy[i].exc = exc;
    let e = g.varz.enemy[i];
    let py = g.player.player[0].y;
    let eyc = steer(g, e.yaccel, e.eyc, py > i32::from(e.ey));
    g.varz.enemy[i].eyc = eyc;

    let e = g.varz.enemy[i];
    let on_screen_x = i32::from(e.ex) + i32::from(map_x_ofs);
    if on_screen_x > -29 && on_screen_x < 300 {
        let e = &mut g.varz.enemy[i];
        if e.aniactive == 1 {
            e.enemycycle = e.enemycycle.wrapping_add(1);

            if e.enemycycle == e.animax {
                e.aniactive = e.aniwhenfire;
            } else if e.enemycycle > e.ani {
                e.enemycycle = e.animin;
            }
        }

        if e.egr[usize::from(e.enemycycle) - 1] == 999 {
            g.varz.enemy_avail[i] = 1; // enemy_gone
            return;
        }

        let (size, ey) = (e.size, e.ey);
        let vga = g.video.vga_screen_id();
        let i32_i = i as u32;
        if size == 1 {
            // 2x2 enemy
            if ey > -13 {
                blit_enemy(g, vga, i32_i, -6, -7, 0);
                blit_enemy(g, vga, i32_i, 6, -7, 1);
            }
            if ey > -26 && ey < 182 {
                blit_enemy(g, vga, i32_i, -6, 7, 19);
                blit_enemy(g, vga, i32_i, 6, 7, 20);
            }
        } else if ey > -13 {
            blit_enemy(g, vga, i32_i, 0, 0, 0);
        }

        g.varz.enemy[i].filter = 0;
    }

    let e = &mut g.varz.enemy[i];
    circle(
        &mut e.exc,
        &mut e.excc,
        &mut e.exccw,
        e.exccwmax,
        &mut e.exrev,
        &mut e.exccadd,
    );
    circle(
        &mut e.eyc,
        &mut e.eycc,
        &mut e.eyccw,
        e.eyccwmax,
        &mut e.eyrev,
        &mut e.eyccadd,
    );

    e.ey = e.ey.wrapping_add(e.fixedmovey);

    e.ex = (i32::from(e.ex) + i32::from(e.exc)) as i16;
    if e.ex < -80 || e.ex > 340 {
        g.varz.enemy_avail[i] = 1; // enemy_gone
        return;
    }

    e.ey = (i32::from(e.ey) + i32::from(e.eyc)) as i16;
    if e.ey < -112 || e.ey > 190 {
        g.varz.enemy_avail[i] = 1; // enemy_gone
        return;
    }

    // enemy_still_exists:

    // X bounce
    if e.ex <= e.xminbounce || e.ex >= e.xmaxbounce {
        e.exc = e.exc.wrapping_neg();
    }

    // Y bounce
    if e.ey <= e.yminbounce || e.ey >= e.ymaxbounce {
        e.eyc = e.eyc.wrapping_neg();
    }

    // Evalue != 0 - score item at boundary
    if e.scoreitem {
        if e.ex < -5 {
            e.ex += 1;
        }
        if e.ex > 245 {
            e.ex -= 1;
        }
    }

    e.ey = (i32::from(e.ey) + i32::from(g.varz.temp_back_move)) as i16;

    if e.ex <= -24 || e.ex >= 296 {
        return;
    }

    let temp_x = e.ex;
    let temp_y = e.ey;

    g.varz.temp = e.enemytype as u8;

    // Enemy Shots
    if e.edamaged {
        return;
    }

    g.varz.enemy_on_screen = g.varz.enemy_on_screen.wrapping_add(1);

    if e.iced != 0 {
        e.iced -= 1;
        if e.enemyground {
            e.filter = 0x09;
        }
        return;
    }

    for j in (1..=3usize).rev() {
        if g.varz.enemy[i].freq[j - 1] == 0 {
            continue;
        }
        let temp3 = g.varz.enemy[i].tur[j - 1];
        g.varz.temp3 = temp3;

        let e = &mut g.varz.enemy[i];
        e.eshotwait[j - 1] = e.eshotwait[j - 1].wrapping_sub(1);
        if e.eshotwait[j - 1] != 0 || temp3 == 0 {
            continue;
        }

        let difficulty = g.config.difficulty_level;
        e.eshotwait[j - 1] = e.freq[j - 1];
        if difficulty > DIFFICULTY_NORMAL {
            e.eshotwait[j - 1] = (e.eshotwait[j - 1] / 2) + 1;
            if difficulty > DIFFICULTY_MANIACAL {
                e.eshotwait[j - 1] = (e.eshotwait[j - 1] / 2) + 1;
            }
        }

        if g.config.galaga_mode
            && (g.varz.enemy[i].eyc == 0 || mt_rand(g) % 400 >= u64::from(g.varz.galaga_shot_freq))
        {
            return;
        }

        let p0 = g.player.player[0];
        let dist = |p: &crate::player::Player| {
            (p.x - i32::from(temp_x)).abs() + (p.y - i32::from(temp_y)).abs()
        };
        let near_magnet = |p: &crate::player::Player, dx: i32| {
            (p.x + dx - 14 - i32::from(temp_x)).abs() < 24 && (p.y - i32::from(temp_y)).abs() < 28
        };
        match temp3 {
            252 => {
                // Savara Boss DualMissile
                if g.varz.enemy[i].ey > 20 {
                    let back = i32::from(g.backgrnd.back_move) * 8;
                    let y = (i32::from(temp_y) - 20 - back) as i16;
                    let x1 = (i32::from(temp_x) - 8 + i32::from(map_x_ofs)) as i16;
                    let x2 = (i32::from(temp_x) + 4 + i32::from(map_x_ofs)) as i16;
                    crate::varz::je_setup_explosion(g, x1, y, -2, 6, false, false);
                    crate::varz::je_setup_explosion(g, x2, y, -2, 6, false, false);
                }
            }
            251 => {
                // Suck-O-Magnet
                let attraction = (4 - dist(&p0) / 100) as i16;
                if attraction > 0 {
                    let a = i32::from(attraction);
                    let p = &mut g.player.player[0];
                    p.x_velocity += if p.x > i32::from(temp_x) { -a } else { a };
                }
            }
            253 | 254 => {
                // Left ShortRange Magnet
                let d = if temp3 == 253 { 2 } else { -2 };
                if near_magnet(&p0, 25) {
                    g.player.player[0].x_velocity += d;
                }
                let p1 = g.player.player[1];
                if g.config.two_player_mode && near_magnet(&p1, 0) {
                    g.player.player[1].x_velocity += d;
                }
            }
            255 => {
                // Magneto RePulse!!
                if difficulty != crate::config::DIFFICULTY_EASY {
                    if j == 3 {
                        g.varz.enemy[i].filter = 0x70;
                    } else {
                        let repulsion = (4 - dist(&p0) / 20) as i16;
                        if repulsion > 0 {
                            let r = i32::from(repulsion);
                            let p = &mut g.player.player[0];
                            p.x_velocity += if p.x > i32::from(temp_x) { r } else { -r };
                        }
                    }
                }
            }
            _ => {
                // Rot
                let w = g.episodes.weapons[usize::from(temp3)];
                for _ in 0..w.multi {
                    g.varz.b = 0;
                    while (g.varz.b as usize) < ENEMY_SHOT_MAX {
                        if g.varz.enemy_shot_avail[g.varz.b as usize] {
                            break;
                        }
                        g.varz.b += 1;
                    }
                    if g.varz.b as usize == ENEMY_SHOT_MAX {
                        return;
                    }
                    let b = g.varz.b as usize;

                    g.varz.enemy_shot_avail[b] = !g.varz.enemy_shot_avail[b];

                    if w.sound > 0 {
                        loop {
                            g.varz.temp = (mt_rand(g) % 8) as u8;
                            if g.varz.temp != 3 {
                                break;
                            }
                        }
                        g.varz.sound_queue[usize::from(g.varz.temp)] = w.sound;
                    }

                    let e = &mut g.varz.enemy[i];
                    if e.aniactive == 2 {
                        e.aniactive = 1;
                    }

                    e.eshotmultipos[j - 1] = e.eshotmultipos[j - 1].wrapping_add(1);
                    if e.eshotmultipos[j - 1] > w.max {
                        e.eshotmultipos[j - 1] = 1;
                    }

                    let pos = usize::from(e.eshotmultipos[j - 1]) - 1;

                    if j == 1 {
                        g.varz.temp2 = 4;
                    }

                    let s = &mut g.varz.enemy_shot[b];
                    s.sx = (i32::from(temp_x) + i32::from(w.bx[pos]) + i32::from(map_x_ofs)) as i16;
                    s.sy = (i32::from(temp_y) + i32::from(w.by[pos])) as i16;
                    s.sdmg = w.attack[pos];
                    s.tx = w.tx;
                    s.ty = w.ty;
                    s.duration = w.del[pos];
                    s.animate = 0;
                    s.animax = w.weapani;

                    s.sgr = w.sg[pos];
                    match j {
                        1 => {
                            s.syc = w.acceleration;
                            s.sxc = w.accelerationx;

                            s.sxm = i16::from(w.sx[pos]);
                            s.sym = i16::from(w.sy[pos]);
                        }
                        3 => {
                            s.sxc = w.acceleration.wrapping_neg();
                            s.syc = w.accelerationx;

                            s.sxm = -i16::from(w.sy[pos]);
                            s.sym = -i16::from(w.sx[pos]);
                        }
                        _ => {
                            s.sxc = w.acceleration;
                            s.syc = w.acceleration.wrapping_neg();

                            s.sxm = i16::from(w.sy[pos]);
                            s.sym = -i16::from(w.sx[pos]);
                        }
                    }

                    if w.aim > 0 {
                        let mut aim_strength = w.aim;

                        // DIF
                        if difficulty > DIFFICULTY_NORMAL {
                            aim_strength = aim_strength.wrapping_add((difficulty - 2) as u8);
                        }

                        let p = g.player.player;
                        let mut target_x = p[0].x as u16;
                        let mut target_y = p[0].y as u16;

                        if g.config.two_player_mode {
                            // fire at live player(s)
                            g.varz.temp = if p[0].is_alive && !p[1].is_alive {
                                0
                            } else if p[1].is_alive && !p[0].is_alive {
                                1
                            } else {
                                (mt_rand(g) % 2) as u8
                            };

                            if g.varz.temp == 1 {
                                target_x = (p[1].x - 25) as u16;
                                target_y = p[1].y as u16;
                            }
                        }

                        let (ax, ay, max) = aim(
                            i32::from(target_x),
                            i32::from(target_y),
                            temp_x,
                            temp_y,
                            map_x_ofs,
                        );
                        let s = &mut g.varz.enemy_shot[b];
                        s.sxm = aimed(ax, max, aim_strength) as i16;
                        s.sym = aimed(ay, max, aim_strength) as i16;
                    }
                }
            }
        }
    }

    // Enemy Launch Routine
    let e = &mut g.varz.enemy[i];
    if e.launchfreq == 0 {
        return;
    }
    e.launchwait = e.launchwait.wrapping_sub(1);
    if e.launchwait != 0 {
        return;
    }
    e.launchwait = e.launchfreq;

    if e.launchspecial != 0 {
        // Type 1: Must be inline with player
        if (i32::from(e.ey) - g.player.player[0].y).abs() > 5 {
            return;
        }
    }

    if e.aniactive == 2 {
        e.aniactive = 1;
    }

    if e.launchtype == 0 {
        return;
    }

    g.varz.temp_w = e.launchtype;
    let (launchspecial, linknum) = (e.launchspecial, e.linknum);
    let offset = if enemy_offset == 50 {
        75
    } else {
        enemy_offset - 25
    };
    g.varz.b = je_new_enemy(g, offset, g.varz.temp_w, 0);

    // Launch Enemy Placement
    if g.varz.b > 0 {
        let bi = (g.varz.b - 1) as usize;
        let startyc = g.episodes.enemy_dat[usize::from(g.varz.enemy[bi].enemytype)].startyc;
        let e = &mut g.varz.enemy[bi];

        e.ex = temp_x;
        e.ey = (i32::from(temp_y) + i32::from(startyc)) as i16;
        if e.size == 0 {
            e.ey = e.ey.wrapping_sub(7);
        }

        if e.launchtype > 0 && e.launchfreq == 0 {
            if e.launchtype > 90 {
                let k = i32::from(e.launchtype) - 90;
                let r = mt_rand(g) % (k * 4) as u64;
                let e = &mut g.varz.enemy[bi];
                e.ex = (i64::from(e.ex) as u64)
                    .wrapping_add(r)
                    .wrapping_sub((k * 2) as u64) as i16;
            } else {
                let p0 = g.player.player[0];
                let (ax, ay, max) = aim(p0.x, p0.y, temp_x, temp_y, map_x_ofs);
                let lt = e.launchtype as u8;
                e.exc = aimed(ax, max, lt) as i8;
                e.eyc = aimed(ay, max, lt) as i8;
            }
        }

        loop {
            g.varz.temp = (mt_rand(g) % 8) as u8;
            if g.varz.temp != 3 {
                break;
            }
        }
        let sound = crate::varz::RANDOM_ENEMY_LAUNCH_SOUNDS[(mt_rand(g) % 3) as usize];
        g.varz.sound_queue[usize::from(g.varz.temp)] = sound;

        if launchspecial == 1 && linknum < 100 {
            g.varz.enemy[bi].linknum = linknum;
        }
    }
}

#[doc(alias = "JE_newEnemy")]
pub fn je_new_enemy(
    g: &mut Game,
    enemy_offset: i32,
    e_dat_i: u16,
    unique_shape_table_i: i16,
) -> i16 {
    for i in enemy_offset..enemy_offset + 25 {
        let i = i as usize;
        if g.varz.enemy_avail[i] == 1 {
            g.varz.enemy_avail[i] = je_make_enemy(g, i, e_dat_i, unique_shape_table_i) as u8;
            return (i + 1) as i16;
        }
    }

    0
}

/// `JE_makeEnemy(&enemy[slot], eDatI, uniqueShapeTableI)`. `eDatI` indexes
/// `enemyDat`; the level loader keeps event data within it.
#[doc(alias = "JE_makeEnemy")]
pub fn je_make_enemy(
    g: &mut Game,
    slot: usize,
    mut e_dat_i: u16,
    unique_shape_table_i: i16,
) -> u32 {
    let avail: u32;

    if g.config.super_arcade_mode != SA_NONE && e_dat_i == 534 {
        e_dat_i = 533;
    }
    let dat = g.episodes.enemy_dat[usize::from(e_dat_i)];

    let shape_table_i: u8 = if unique_shape_table_i > 0 {
        unique_shape_table_i as u8
    } else {
        dat.shapebank
    };

    let mut sprite2s = None;
    if shape_table_i == 21 {
        sprite2s = Some(Sprite2Ref::SpriteSheet11); // Coins&Gems
    } else if shape_table_i == 26 {
        sprite2s = Some(Sprite2Ref::SpriteSheet10); // Two-Player Stuff
    } else {
        for (i, &id) in g.sprite.enemy_sprite_sheet_ids.iter().enumerate() {
            if shape_table_i == id {
                sprite2s = Some(Sprite2Ref::EnemySpriteSheets(i as u8));
            }
        }
    }

    if sprite2s.is_some() {
        g.varz.enemy[slot].sprite2s = sprite2s;
    } else {
        // Use shape table value from previous enemy that occupied the enemy
        // slot. (Ex. APPROACH.)
        let message = format!("Ignoring sprite from unloaded shape table {shape_table_i}.");
        g.platform.log_warn(LOG_CATEGORY, &message);
    }

    if g.varz.enemy[slot].sprite2s.is_none() {
        g.platform
            .log_warn(LOG_CATEGORY, "Enemy sprite is missing.");
    }

    // `startx + (mt_rand() % (startxc * 2)) - startxc + 1`, computed in
    // `unsigned long` as C does, then stored in a 16-bit field.
    let start = |g: &mut Game, s: i16, c: i8| -> i16 {
        if c != 0 {
            let m = (i64::from(c) * 2) as u64;
            let r = mt_rand(g) % m;
            (i64::from(s) as u64)
                .wrapping_add(r)
                .wrapping_sub(i64::from(c) as u64)
                .wrapping_add(1) as i16
        } else {
            s.wrapping_add(1)
        }
    };
    let ex = start(g, dat.startx, dat.startxc);
    let ey = start(g, dat.starty, dat.startyc);

    let difficulty_level = g.config.difficulty_level;
    let enemy = &mut g.varz.enemy[slot];

    enemy.enemydatofs = Some(usize::from(e_dat_i));

    enemy.mapoffset = 0;

    enemy.eshotmultipos = [0; 3];

    enemy.enemyground = (dat.explosiontype & 1) == 0;
    enemy.explonum = dat.explosiontype >> 1;

    enemy.launchfreq = dat.elaunchfreq;
    enemy.launchwait = dat.elaunchfreq;
    enemy.launchtype = dat.elaunchtype % 1000;
    enemy.launchspecial = (dat.elaunchtype / 1000) as u8;

    enemy.xaccel = dat.xaccel as u8;
    enemy.yaccel = dat.yaccel as u8;

    // Far enough away to be impossible to reach
    enemy.xminbounce = -10000;
    enemy.xmaxbounce = 10000;
    enemy.yminbounce = -10000;
    enemy.ymaxbounce = 10000;

    enemy.tur = dat.tur;

    enemy.ani = dat.ani;
    enemy.animin = 1;

    match dat.animate {
        0 => {
            enemy.enemycycle = 1;
            enemy.aniactive = 0;
            enemy.animax = 0;
            enemy.aniwhenfire = 0;
        }
        1 => {
            enemy.enemycycle = 0;
            enemy.aniactive = 1;
            enemy.animax = 0;
            enemy.aniwhenfire = 0;
        }
        2 => {
            enemy.enemycycle = 1;
            enemy.aniactive = 2;
            enemy.animax = enemy.ani;
            enemy.aniwhenfire = 2;
        }
        _ => {}
    }

    enemy.ex = ex;
    enemy.ey = ey;

    enemy.exc = dat.xmove;
    enemy.eyc = dat.ymove;
    enemy.excc = dat.xcaccel;
    enemy.eycc = dat.ycaccel;
    enemy.exccw = i32::from(enemy.excc).abs() as i8;
    enemy.exccwmax = enemy.exccw as u8;
    enemy.eyccw = i32::from(enemy.eycc).abs() as i8;
    enemy.eyccwmax = enemy.eyccw as u8;
    enemy.exccadd = if enemy.excc > 0 { 1 } else { -1 };
    enemy.eyccadd = if enemy.eycc > 0 { 1 } else { -1 };
    enemy.special = false;
    enemy.iced = 0;

    enemy.exrev = match dat.xrev {
        0 => 100,
        -99 => 0,
        v => v,
    };
    enemy.eyrev = match dat.yrev {
        0 => 100,
        -99 => 0,
        v => v,
    };

    enemy.exca = if enemy.xaccel > 0 { 1 } else { -1 };
    enemy.eyca = if enemy.yaccel > 0 { 1 } else { -1 };

    enemy.enemytype = e_dat_i;

    for i in 0..3 {
        enemy.eshotwait[i] = if enemy.tur[i] == 252 {
            1
        } else if enemy.tur[i] > 0 {
            20
        } else {
            255
        };
    }
    enemy.egr = dat.egraphic;
    enemy.size = dat.esize;
    enemy.linknum = 0;
    enemy.edamaged = dat.dani < 0;
    enemy.enemydie = dat.eenemydie;

    enemy.freq = dat.freq;

    enemy.edani = dat.dani;
    enemy.edgr = dat.dgr;
    enemy.edlevel = dat.dlevel;

    enemy.fixedmovey = 0;

    enemy.filter = 0x00;

    let value = i32::from(dat.value);
    let mut temp_value: i32 = 0;
    if value > 1 && value < 10000 {
        let scaled = |f: f32| (value as f32 * f) as i32;
        match difficulty_level {
            -1 | 0 => temp_value = scaled(0.75),
            1 | 2 => temp_value = value,
            3 => temp_value = scaled(1.125),
            4 => temp_value = scaled(1.5),
            5 => temp_value = value * 2,
            6 => temp_value = scaled(2.5),
            7 | 8 => temp_value = value * 4,
            9 | 10 => temp_value = value * 8,
            _ => {}
        }
        if temp_value > 10000 {
            temp_value = 10000;
        }
        enemy.evalue = temp_value as i16;
    } else {
        enemy.evalue = dat.value;
    }

    if dat.armor > 0 {
        let armor = i32::from(dat.armor);
        let mut temp_armor: i32 = 1;
        if dat.armor != 255 {
            let scaled = |f: f32| (armor as f32 * f) as i32;
            let scaled_plus_one = |f: f32| (armor as f32 * f + 1.0) as i32;
            match difficulty_level {
                -1 | 0 => temp_armor = scaled_plus_one(0.5),
                1 => temp_armor = scaled_plus_one(0.75),
                2 => temp_armor = armor,
                3 => temp_armor = scaled(1.2),
                4 => temp_armor = scaled(1.5),
                5 => temp_armor = scaled(1.8),
                6 => temp_armor = armor * 2,
                7 => temp_armor = armor * 3,
                8 => temp_armor = armor * 4,
                9 | 10 => temp_armor = armor * 8,
                _ => {}
            }

            if temp_armor > 254 {
                temp_armor = 254;
            }
        } else {
            temp_armor = 255;
        }

        enemy.armorleft = temp_armor as u8;

        avail = 0;
        enemy.scoreitem = false;
    } else {
        avail = 2;
        enemy.armorleft = 255;
        if enemy.evalue != 0 {
            enemy.scoreitem = true;
        }
    }

    if !enemy.scoreitem {
        g.varz.total_enemy = g.varz.total_enemy.wrapping_add(1); // Destruction ratio
    }

    // indicates what to set ENEMYAVAIL to
    avail
}

#[doc(alias = "JE_createNewEventEnemy")]
pub fn je_create_new_event_enemy(
    g: &mut Game,
    enemy_type_ofs: u8,
    enemy_offset: u16,
    unique_shape_table_i: i16,
) {
    g.varz.b = 0;

    for i in enemy_offset..enemy_offset + 25 {
        if g.varz.enemy_avail[usize::from(i)] == 1 {
            g.varz.b = (i + 1) as i16;
            break;
        }
    }

    if g.varz.b == 0 {
        return;
    }

    let ev = g.tyrian2.event_rec[usize::from(g.varz.event_loc) - 1];
    g.varz.temp_w = (i32::from(ev.eventdat) + i32::from(enemy_type_ofs)) as u16;

    let slot = (g.varz.b - 1) as usize;
    g.varz.enemy_avail[slot] = je_make_enemy(g, slot, g.varz.temp_w, unique_shape_table_i) as u8;

    let bk = &g.backgrnd;
    let (map_x, map_x3) = (i32::from(bk.map_x), i32::from(bk.map_x3));
    let back_moves = (bk.back_move, bk.back_move2, bk.back_move3);
    let (background3x1, background3x1b) = (g.varz.background3x1, g.varz.background3x1b);
    let small_enemy_adjust = g.varz.small_enemy_adjust;
    let e = &mut g.varz.enemy[slot];
    let sub = |v: i16, d: u16| (i32::from(v) - i32::from(d)) as i16;

    if ev.eventdat2 != -99 {
        let dat2 = i32::from(ev.eventdat2);
        match enemy_offset {
            0 => {
                e.ex = (dat2 - (map_x - 1) * 24) as i16;
                e.ey = sub(e.ey, back_moves.1);
            }
            25 | 75 => {
                e.ex = (dat2 - (map_x - 1) * 24 - 12) as i16;
                e.ey = sub(e.ey, back_moves.0);
            }
            50 => {
                e.ex = if background3x1 {
                    (dat2 - (map_x - 1) * 24 - 12) as i16
                } else {
                    (dat2 - map_x3 * 24 - 24 * 2 + 6) as i16
                };
                e.ey = sub(e.ey, back_moves.2);

                if background3x1b {
                    e.ex = e.ex.wrapping_sub(6);
                }
            }
            _ => {}
        }
        e.ey = -28;
        if background3x1b && enemy_offset == 50 {
            e.ey += 4;
        }
    }

    if small_enemy_adjust && e.size == 0 {
        e.ex = e.ex.wrapping_sub(10);
        e.ey = e.ey.wrapping_sub(7);
    }

    e.ey = e.ey.wrapping_add(i16::from(ev.eventdat5));
    e.eyc = e.eyc.wrapping_add(ev.eventdat3);
    e.linknum = ev.eventdat4;
    e.fixedmovey = i16::from(ev.eventdat6);
}

/// `JE_eventJump(jump)`: 65535 returns to the saved location. The event list
/// holds an event at or after every location the level script jumps to.
#[doc(alias = "JE_eventJump")]
pub fn je_event_jump(g: &mut Game, jump: u16) {
    let v = &mut g.varz;
    if jump == 65535 {
        v.cur_loc = v.return_loc;
    } else {
        v.return_loc = v.cur_loc.wrapping_add(1);
        v.cur_loc = jump;
    }
    let mut temp_w: u16 = 0;
    loop {
        temp_w = temp_w.wrapping_add(1);
        if g.tyrian2.event_rec[usize::from(temp_w) - 1].eventtime >= v.cur_loc {
            break;
        }
    }
    v.event_loc = temp_w - 1;
}

/// `JE_searchFor(PLType, out_index)`: whether a live enemy has link number
/// `pl_type`; `out_index` receives the last such enemy.
#[doc(alias = "JE_searchFor")]
pub fn je_search_for(g: &mut Game, pl_type: u8, out_index: Option<&mut u8>) -> bool {
    let mut found_id: Option<usize> = None;

    for i in 0..100 {
        if g.varz.enemy_avail[i] == 0 && g.varz.enemy[i].linknum == pl_type {
            found_id = Some(i);
            if g.config.galaga_mode {
                let e = &mut g.varz.enemy[i];
                e.evalue = e.evalue.wrapping_add(e.evalue);
            }
        }
    }

    match found_id {
        Some(i) => {
            if let Some(out) = out_index {
                *out = i as u8;
            }
            true
        }
        None => false,
    }
}

/// The pixel buffers `JE_whoa` works on, taken out of their surfaces while
/// they are processed.
fn take_pixels(g: &mut Game, id: ScreenId) -> Vec<u8> {
    std::mem::take(&mut g.video.screen_mut(id).pixels)
}

#[doc(alias = "JE_whoa")]
pub fn je_whoa(g: &mut Game) -> Flow {
    // 'whoa' gets us that nifty screen fade used when you type in 'engage'.
    // game_screen and VGAScreen2 serve as the two temporary buffers; their
    // roles swap every loop.
    let seg = g.video.vga_screen_seg.expect("VGAScreenSeg");
    let mut temp1 = g.video.game_screen.expect("game_screen");
    let mut temp2 = g.video.vga_screen2.expect("VGAScreen2");

    let s = g.video.screen(seg);
    let pitch = s.pitch as usize;
    let screen_size = s.h as usize * pitch;
    let top_border = pitch * 4; // Seems an arbitrary number of lines
    let bottom_border = pitch * 7;

    debug_assert!(
        g.video.screen(temp2).pixels.len() >= screen_size
            && g.video.screen(temp1).pixels.len() >= screen_size
    );

    // Clear the top and bottom borders. We don't want to process them and
    // we don't want to draw them.
    let s = g.video.screen_mut(seg);
    s.pixels[..top_border].fill(0);
    s.pixels[screen_size - bottom_border..screen_size].fill(0);

    // Copy our test subject to one of the temporary buffers. Blank the other.
    g.video.screen_mut(temp1).pixels[..screen_size].fill(0);
    let (src, dst) = g.video.screen_pair_mut(seg, temp2);
    dst.pixels[..screen_size].copy_from_slice(&src.pixels[..screen_size]);

    for _ in 0..300 {
        crate::nortsong::set_frame_count(g, 1);

        let mut t1 = take_pixels(g, temp1);
        let t2 = take_pixels(g, temp2);
        let mut i = screen_size - bottom_border;
        let mut j = top_border / 2;
        while i > 0 {
            let offset = j + i / 8192 - 4;
            let color = (u32::from(t2[offset]) * 12
                + u32::from(t1[offset - pitch])
                + u32::from(t1[offset - 1])
                + u32::from(t1[offset + 1])
                + u32::from(t1[offset + pitch]))
                / 16;

            t1[j] = color as u8;
            i -= 1;
            j += 1;
        }

        // Now copy that mess to the buffer.
        let n = screen_size - bottom_border;
        g.video.screen_mut(seg).pixels[top_border..top_border + n]
            .copy_from_slice(&t1[top_border..top_border + n]);
        g.video.screen_mut(temp1).pixels = t1;
        g.video.screen_mut(temp2).pixels = t2;

        crate::video::je_show_vga(g);

        crate::keyboard::wait_until_elapsed(g)?;

        let mut keyboard_input = KeyboardInput::default();
        if (crate::keyboard::keyboard_get_input(g, Some(&mut keyboard_input))
            && keyboard_input.scancode != SDL_SCANCODE_SCROLLLOCK)
            || crate::keyboard::mouse_get_input(g, INPUT_NO_MOTION, None)
        {
            break;
        }

        // Flip the buffer.
        std::mem::swap(&mut temp1, &mut temp2);
    }

    g.fonthand.level_warning_lines = 4;
    Ok(())
}

#[doc(alias = "JE_barX")]
pub fn je_bar_x(g: &mut Game, x1: u16, y1: u16, x2: u16, y2: u16, col: u8) {
    let vga = g.video.vga_screen_id();
    let (x1, y1, x2, y2) = (i32::from(x1), i32::from(y1), i32::from(x2), i32::from(y2));
    crate::vga256d::fill_rectangle_xy(g, vga, x1, y1, x2, y1, col.wrapping_add(1));
    crate::vga256d::fill_rectangle_xy(g, vga, x1, y1 + 1, x2, y2 - 1, col);
    crate::vga256d::fill_rectangle_xy(g, vga, x1, y2, x2, y2, col.wrapping_sub(1));
}

pub fn draw_boss_bar(g: &mut Game) {
    for b in 0..2 {
        if g.tyrian2.boss_bar[b].link_num == 0 {
            continue;
        }

        let mut armor: u32 = 256; // higher than armor max

        for e in 0..100 {
            // find most damaged
            let en = &g.varz.enemy[e];
            if g.varz.enemy_avail[e] != 1
                && en.linknum == g.tyrian2.boss_bar[b].link_num
                && u32::from(en.armorleft) < armor
            {
                armor = u32::from(en.armorleft);
            }
        }

        let bar = &mut g.tyrian2.boss_bar[b];
        if armor > 255 || armor == 0 {
            // boss dead?
            bar.link_num = 0;
        } else {
            // 255 would make the bar too long
            bar.armor = if armor == 255 { 254 } else { armor as u8 };
        }
    }

    let bars = u32::from(g.tyrian2.boss_bar[0].link_num != 0)
        + u32::from(g.tyrian2.boss_bar[1].link_num != 0);

    // if only one bar left, make it the first one
    if bars == 1 && g.tyrian2.boss_bar[0].link_num == 0 {
        g.tyrian2.boss_bar[0] = g.tyrian2.boss_bar[1];
        g.tyrian2.boss_bar[1].link_num = 0;
    }

    for b in 0..bars as usize {
        let x: u32 = if bars == 2 {
            if b == 0 { 125 } else { 185 }
        } else if g.varz.level_timer {
            250 // level timer and boss bar would overlap
        } else {
            155
        };

        je_bar_x(g, (x - 25) as u16, 7, (x + 25) as u16, 12, 115);
        let bar = g.tyrian2.boss_bar[b];
        je_bar_x(
            g,
            (x - u32::from(bar.armor) / 10) as u16,
            7,
            (x + (u32::from(bar.armor) + 5) / 10) as u16,
            12,
            118u8.wrapping_add(bar.color),
        );

        if bar.color > 0 {
            g.tyrian2.boss_bar[b].color -= 1;
        }
    }
}

#[doc(alias = "JE_displayText")]
pub fn je_display_text(g: &mut Game) -> Flow {
    // Display Warning Text
    let mut temp_y: u16 = 55;
    if g.fonthand.warning_red {
        temp_y = 2;
    }
    g.varz.temp = 0;
    while g.varz.temp < g.fonthand.level_warning_lines {
        if !g.keyboard.esc_pressed {
            let text = g.fonthand.level_warning_text[usize::from(g.varz.temp)];
            crate::mainint::je_out_char_glow(g, 10, temp_y, &text)?;

            if g.network.halt_game {
                crate::varz::je_tyrian_halt(g, 5)?;
            }

            temp_y = temp_y.wrapping_add(10);
        }
        g.varz.temp = g.varz.temp.wrapping_add(1);
    }

    let slow = if g.nortsong.frame_count_max != 0 {
        g.nortsong.frame_count_max = 6;
        true
    } else {
        false
    };
    g.varz.temp_w = 184;
    if g.fonthand.warning_red {
        g.varz.temp_w = 7 * 16 + 6;
    }

    let text = g.helptext.misc_text[4];
    let x = crate::fonthand::je_font_center(g, &text, crate::sprite::TINY_FONT as u32);
    crate::mainint::je_out_char_glow(g, x as u16, g.varz.temp_w, &text)?;

    loop {
        crate::nortsong::set_frame_count(g, 1);

        if g.fonthand.level_warning_display {
            let vga = g.video.vga_screen_id();
            crate::fonthand::je_update_warning(g, vga);
        }

        if crate::keyboard::wait_until_get_input_or_elapsed(g)? {
            break;
        }

        if (g.nortsong.frame_count_max == 0 && slow) || g.keyboard.esc_pressed {
            break;
        }
    }

    g.fonthand.level_warning_display = false;
    Ok(())
}

/// The enemy slot range `JE_createNewEventEnemy` searches for the custom 4x4
/// ground enemy (event 12), selected by `eventdat6`.
fn event_enemy_offset(selector: i32) -> u16 {
    match selector {
        0 | 1 => 25,
        3 => 50,
        4 => 75,
        _ => 0,
    }
}

/// `JE_eventSystem()`: runs the event at `eventLoc` and advances `eventLoc`.
/// Event data comes from the level file (`JE_loadMap` validates it, R10).
#[doc(alias = "JE_eventSystem")]
pub fn je_event_system(g: &mut Game) -> Flow {
    let loc = usize::from(g.varz.event_loc) - 1;
    let ev = g.tyrian2.event_rec[loc];
    let in_new_pl = |d3: i8| (80..90).contains(&d3);

    match ev.eventtype {
        1 => g.backgrnd.starfield_speed = i32::from(ev.eventdat),

        2 => {
            let b = &mut g.backgrnd;
            (
                b.map1_y_delay,
                b.map1_y_delay_max,
                b.map2_y_delay,
                b.map2_y_delay_max,
            ) = (1, 1, 1, 1);

            b.back_move = ev.eventdat as u16;
            b.back_move2 = ev.eventdat2 as u16;

            g.varz.explode_move = if b.back_move2 > 0 {
                b.back_move2
            } else {
                b.back_move
            };

            b.back_move3 = ev.eventdat3 as u16;

            if b.back_move > 0 {
                g.varz.stop_background_num = 0;
            }
        }

        3 => {
            let b = &mut g.backgrnd;
            b.back_move = 1;
            b.map1_y_delay = 3;
            b.map1_y_delay_max = 3;
            b.back_move2 = 1;
            b.map2_y_delay = 2;
            b.map2_y_delay_max = 2;
            b.back_move3 = 1;
        }

        4 => {
            g.varz.stop_backgrounds = true;
            match ev.eventdat {
                0 | 1 => g.varz.stop_background_num = 1,
                2 => g.varz.stop_background_num = 2,
                3 => g.varz.stop_background_num = 3,
                _ => {}
            }
        }

        5 => {
            // load enemy shape banks
            let positive = |v: i16| if v > 0 { v as u8 } else { 0 };
            let new_enemy_shape_tables = [
                positive(ev.eventdat),
                positive(ev.eventdat2),
                positive(i16::from(ev.eventdat3)),
                ev.eventdat4,
            ];

            for (i, &table) in new_enemy_shape_tables.iter().enumerate() {
                if g.sprite.enemy_sprite_sheet_ids[i] != table {
                    if table > 0 {
                        let file = crate::lvlmast::SHAPE_FILE[usize::from(table) - 1];
                        crate::sprite::je_load_comp_shapes(
                            g,
                            Sprite2Ref::EnemySpriteSheets(i as u8),
                            file,
                        )?;
                    } else {
                        crate::sprite::free_sprite2s(&mut g.sprite.enemy_sprite_sheets[i]);
                    }

                    g.sprite.enemy_sprite_sheet_ids[i] = table;
                }
            }
        }

        6 => je_create_new_event_enemy(g, 0, 25, 0), // Ground Enemy
        7 => je_create_new_event_enemy(g, 0, 50, 0), // Top Enemy
        8 => g.config.star_active = false,
        9 => g.config.star_active = true,
        10 => je_create_new_event_enemy(g, 0, 75, 0), // Ground Enemy 2

        11 => {
            let v = &mut g.varz;
            if v.all_players_gone || ev.eventdat == 1 {
                v.really_end_level = true;
            } else if !v.end_level {
                v.ready_to_end_level = false;
                v.end_level = true;
                v.level_end = 40;
            }
        }

        12 => {
            // Custom 4x4 Ground Enemy
            let offset = event_enemy_offset(i32::from(ev.eventdat6));
            g.tyrian2.event_rec[loc].eventdat6 = 0; // We use EVENTDAT6 for the background
            let last = |g: &Game| (g.varz.b > 0).then(|| (g.varz.b - 1) as usize);
            je_create_new_event_enemy(g, 0, offset, 0);
            je_create_new_event_enemy(g, 1, offset, 0);
            if let Some(e) = last(g) {
                g.varz.enemy[e].ex = g.varz.enemy[e].ex.wrapping_add(24);
            }
            je_create_new_event_enemy(g, 2, offset, 0);
            if let Some(e) = last(g) {
                g.varz.enemy[e].ey = g.varz.enemy[e].ey.wrapping_sub(28);
            }
            je_create_new_event_enemy(g, 3, offset, 0);
            if let Some(e) = last(g) {
                let en = &mut g.varz.enemy[e];
                en.ex = en.ex.wrapping_add(24);
                en.ey = en.ey.wrapping_sub(28);
            }
        }

        13 => g.varz.enemies_active = false,
        14 => g.varz.enemies_active = true,
        15 => je_create_new_event_enemy(g, 0, 0, 0), // Sky Enemy

        16 => {
            // Window Text
            let id = ev.eventdat;
            if !(1..=9).contains(&id) {
                let message = format!(
                    "Event {:05} {} has invalid window text ID {}.",
                    ev.eventtime, ev.eventtype, id
                );
                g.platform.log_warn(LOG_CATEGORY, &message);
            } else {
                let text = g.helptext.outputs[(id - 1) as usize];
                crate::mainint::je_draw_text_window(g, &text);
                g.varz.sound_queue[3] = crate::sndmast::WINDOW_TEXT_SAMPLES[(id - 1) as usize];
            }
        }

        17 | 18 => {
            // Ground Bottom / Sky Enemy on Bottom
            je_create_new_event_enemy(g, 0, if ev.eventtype == 17 { 25 } else { 0 }, 0);
            set_last_enemy_y(g, 190 + i32::from(ev.eventdat5));
        }

        19 => {
            // Enemy Global Move
            let (mut initial_i, mut max_i, mut all_enemies) = (0, 0, false);

            if in_new_pl(ev.eventdat3) {
                (initial_i, max_i, all_enemies) = (0, 100, false);
                g.tyrian2.event_rec[loc].eventdat4 = g.varz.new_pl[(ev.eventdat3 - 80) as usize];
            } else {
                match ev.eventdat3 {
                    0 => (initial_i, max_i, all_enemies) = (0, 100, false),
                    2 => (initial_i, max_i, all_enemies) = (0, 25, true),
                    1 => (initial_i, max_i, all_enemies) = (25, 50, true),
                    3 => (initial_i, max_i, all_enemies) = (50, 75, true),
                    99 => (initial_i, max_i, all_enemies) = (0, 100, true),
                    _ => {}
                }
            }

            let ev = g.tyrian2.event_rec[loc];
            for e in g.varz.enemy[initial_i..max_i].iter_mut() {
                if all_enemies || e.linknum == ev.eventdat4 {
                    if ev.eventdat != -99 {
                        e.exc = ev.eventdat as i8;
                    }

                    if ev.eventdat2 != -99 {
                        e.eyc = ev.eventdat2 as i8;
                    }

                    if ev.eventdat6 != 0 {
                        e.fixedmovey = i16::from(ev.eventdat6);
                    }

                    if ev.eventdat6 == -99 {
                        e.fixedmovey = 0;
                    }

                    if ev.eventdat5 > 0 {
                        e.enemycycle = ev.eventdat5 as u8;
                    }
                }
            }
        }

        20 => {
            // Enemy Global Accel
            if in_new_pl(ev.eventdat3) {
                g.tyrian2.event_rec[loc].eventdat4 = g.varz.new_pl[(ev.eventdat3 - 80) as usize];
            }
            let ev = g.tyrian2.event_rec[loc];

            for i in 0..100 {
                let e = &mut g.varz.enemy[i];
                if g.varz.enemy_avail[i] != 1 && (e.linknum == ev.eventdat4 || ev.eventdat4 == 0) {
                    if ev.eventdat != -99 {
                        e.excc = ev.eventdat as i8;
                        e.exccw = i32::from(ev.eventdat).abs() as i8;
                        e.exccwmax = i32::from(ev.eventdat).unsigned_abs() as u8;
                        e.exccadd = if ev.eventdat > 0 { 1 } else { -1 };
                    }

                    if ev.eventdat2 != -99 {
                        e.eycc = ev.eventdat2 as i8;
                        e.eyccw = i32::from(ev.eventdat2).abs() as i8;
                        e.eyccwmax = i32::from(ev.eventdat2).unsigned_abs() as u8;
                        e.eyccadd = if ev.eventdat2 > 0 { 1 } else { -1 };
                    }

                    if ev.eventdat5 > 0 {
                        e.enemycycle = ev.eventdat5 as u8;
                    }
                    if ev.eventdat6 > 0 {
                        e.ani = ev.eventdat6 as u8;
                        e.animin = ev.eventdat5 as u8;
                        e.animax = 0;
                        e.aniactive = 1;
                    }
                }
            }
            g.varz.temp = 100;
        }

        21 => g.config.background3over = 1,
        22 => g.config.background3over = 0,

        23 => {
            // Sky Enemy on Bottom
            je_create_new_event_enemy(g, 0, 50, 0);
            set_last_enemy_y(g, 180 + i32::from(ev.eventdat5));
        }

        24 => {
            // Enemy Global Animate
            for e in g.varz.enemy.iter_mut() {
                if e.linknum == ev.eventdat4 {
                    e.aniactive = 1;
                    e.aniwhenfire = 0;
                    if ev.eventdat2 > 0 {
                        e.enemycycle = ev.eventdat2 as u8;
                        e.animin = e.enemycycle;
                    } else {
                        e.enemycycle = 0;
                    }

                    if ev.eventdat > 0 {
                        e.ani = ev.eventdat as u8;
                    }

                    if ev.eventdat3 == 1 {
                        e.animax = e.ani;
                    } else if ev.eventdat3 == 2 {
                        e.aniactive = 2;
                        e.animax = e.ani;
                        e.aniwhenfire = 2;
                    }
                }
            }
            g.varz.temp = 100;
        }

        25 => {
            // Enemy Global Damage change
            let galaga = g.config.galaga_mode;
            let half_difficulty = i32::from(g.config.difficulty_level) / 2;
            for e in g.varz.enemy.iter_mut() {
                if ev.eventdat4 == 0 || e.linknum == ev.eventdat4 {
                    e.armorleft = if galaga {
                        // float -> JE_byte goes through a 32-bit int on both
                        // oracle targets.
                        ((i32::from(ev.eventdat) * half_difficulty) as f32).round() as i32 as u8
                    } else {
                        ev.eventdat as u8
                    };
                }
            }
            g.varz.temp = 100;
        }

        26 => g.varz.small_enemy_adjust = ev.eventdat != 0,

        27 => {
            // Enemy Global AccelRev
            if in_new_pl(ev.eventdat3) {
                g.tyrian2.event_rec[loc].eventdat4 = g.varz.new_pl[(ev.eventdat3 - 80) as usize];
            }
            let ev = g.tyrian2.event_rec[loc];

            for e in g.varz.enemy.iter_mut() {
                if ev.eventdat4 == 0 || e.linknum == ev.eventdat4 {
                    if ev.eventdat != -99 {
                        e.exrev = ev.eventdat as i8;
                    }
                    if ev.eventdat2 != -99 {
                        e.eyrev = ev.eventdat2 as i8;
                    }
                    if ev.eventdat3 != 0 && ev.eventdat3 < 17 {
                        e.filter = ev.eventdat3 as u8;
                    }
                }
            }
            g.varz.temp = 100;
        }

        28 => g.config.top_enemy_over = false,
        29 => g.config.top_enemy_over = true,

        30 => {
            let b = &mut g.backgrnd;
            (
                b.map1_y_delay,
                b.map1_y_delay_max,
                b.map2_y_delay,
                b.map2_y_delay_max,
            ) = (1, 1, 1, 1);

            b.back_move = ev.eventdat as u16;
            b.back_move2 = ev.eventdat2 as u16;
            g.varz.explode_move = b.back_move2;
            b.back_move3 = ev.eventdat3 as u16;
        }

        31 => {
            // Enemy Fire Override
            let mut matched = false;
            for e in g.varz.enemy.iter_mut() {
                if ev.eventdat4 == 99 || e.linknum == ev.eventdat4 {
                    e.freq = [ev.eventdat as u8, ev.eventdat2 as u8, ev.eventdat3 as u8];
                    e.eshotwait = [1; 3];
                    matched = true;
                    if e.launchtype > 0 {
                        e.launchfreq = ev.eventdat5 as u8;
                        e.launchwait = 1;
                    }
                }
            }
            g.varz.temp = 100;
            if matched {
                // The inner `for (temp2 = 0; temp2 < 3; temp2++)` loop.
                g.varz.temp2 = 3;
            }
        }

        32 => {
            // create enemy
            je_create_new_event_enemy(g, 0, 50, 0);
            set_last_enemy_y(g, 190);
        }

        33 => {
            // Enemy From other Enemies
            let c = &g.config;
            if !((ev.eventdat == 512 || ev.eventdat == 513)
                && (c.two_player_mode || c.one_player_action || c.super_tyrian))
            {
                if g.config.super_arcade_mode != SA_NONE {
                    if ev.eventdat == 534 {
                        g.tyrian2.event_rec[loc].eventdat = 827;
                    }
                } else if !g.config.super_tyrian {
                    random_special_drop(g, loc);
                }
                if g.tyrian2.event_rec[loc].eventdat == 534 && g.config.super_tyrian {
                    let k = (mt_rand(g) % 4) as usize;
                    g.tyrian2.event_rec[loc].eventdat =
                        828 + i16::from(crate::varz::SUPER_TYRIAN_SPECIALS[k]);
                }

                set_enemy_die(g, loc);
            }
        }

        34 => {
            // Start Music Fade
            if g.varz.first_game_over {
                g.musmast.music_fade = true;
                g.nortsong.temp_volume = g.nortsong.tyr_music_volume;
            }
        }

        35 => {
            // Play new song
            if g.varz.first_game_over {
                crate::loudness::play_song(g, (i32::from(ev.eventdat) - 1) as u32)?;
                let (music, fx) = (g.nortsong.tyr_music_volume, g.nortsong.fx_volume);
                crate::loudness::set_volume(g, music as u8, fx as u8);
            }
            g.musmast.music_fade = false;
        }

        36 => g.varz.ready_to_end_level = true,
        37 => g.tyrian2.level_enemy_frequency = ev.eventdat as u16,

        38 => {
            g.varz.cur_loc = ev.eventdat as u16;
            let mut new_event_loc: i32 = 1;
            g.varz.temp_w = 0;
            while g.varz.temp_w < g.varz.max_event {
                if g.tyrian2.event_rec[usize::from(g.varz.temp_w)].eventtime <= g.varz.cur_loc {
                    new_event_loc = i32::from(g.varz.temp_w) + 1 - 1;
                }
                g.varz.temp_w += 1;
            }
            g.varz.event_loc = new_event_loc as u16;
        }

        39 => {
            // Enemy Global Linknum Change
            for e in g.varz.enemy.iter_mut() {
                if i16::from(e.linknum) == ev.eventdat {
                    e.linknum = ev.eventdat2 as u8;
                }
            }
            g.varz.temp = 100;
        }

        40 => g.varz.enemy_continual_damage = true, // Enemy Continual Damage

        41 => {
            if ev.eventdat == 0 {
                g.varz.enemy_avail = [1; 100];
            } else {
                g.varz.enemy_avail[..=24].fill(1);
                g.varz.x = 25;
            }
        }

        42 => g.config.background3over = 2,
        43 => g.config.background2over = ev.eventdat as u8,

        44 => {
            let c = &mut g.config;
            c.filter_active = ev.eventdat > 0;
            c.filter_fade = ev.eventdat == 2;
            c.level_filter = ev.eventdat2 as i8;
            c.level_brightness = ev.eventdat3;
            c.level_filter_new = ev.eventdat4 as i8;
            c.level_brightness_chg = ev.eventdat5;
            c.filter_fade_start = ev.eventdat6 == 0;
        }

        45 => {
            // arcade-only enemy from other enemies
            if !g.config.super_tyrian {
                random_special_drop(g, loc);
                if g.config.two_player_mode || g.config.one_player_action {
                    set_enemy_die(g, loc);
                }
            }
        }

        46 => {
            // change difficulty
            if ev.eventdat3 != 0 {
                g.varz.damage_rate = ev.eventdat3 as u8;
            }

            let c = &mut g.config;
            if ev.eventdat2 == 0 || c.two_player_mode || c.one_player_action {
                c.difficulty_level = ((i32::from(c.difficulty_level) + i32::from(ev.eventdat))
                    as i8)
                    .clamp(crate::config::DIFFICULTY_EASY, crate::config::DIFFICULTY_10);
            }
        }

        47 => {
            // Enemy Global AccelRev
            for e in g.varz.enemy.iter_mut() {
                if ev.eventdat4 == 0 || e.linknum == ev.eventdat4 {
                    e.armorleft = ev.eventdat as u8;
                }
            }
            g.varz.temp = 100;
        }

        48 => g.config.background2not_transparent = true, // Background 2 Cannot be Transparent

        49..=52 => {
            let v = &mut g.varz;
            let e = &mut g.tyrian2.event_rec[loc];
            v.temp_dat2 = e.eventdat;
            e.eventdat = 0;
            v.temp_dat = i16::from(e.eventdat3);
            e.eventdat3 = 0;
            v.temp_dat3 = i16::from(e.eventdat6);
            e.eventdat6 = 0;
            g.episodes.enemy_dat[0].armor = v.temp_dat3 as u8;
            g.episodes.enemy_dat[0].egraphic[0] = v.temp_dat2 as u16;
            v.temp = match ev.eventtype - 48 {
                1 => 25,
                2 => 0,
                3 => 50,
                _ => 75,
            };
            let (offset, unique) = (u16::from(v.temp), v.temp_dat);
            je_create_new_event_enemy(g, 0, offset, unique);
            let (v, e) = (&g.varz, &mut g.tyrian2.event_rec[loc]);
            e.eventdat = v.temp_dat2;
            e.eventdat3 = v.temp_dat as i8;
            e.eventdat6 = v.temp_dat3 as i8;
        }

        53 => g.varz.force_events = ev.eventdat != 99,
        54 => je_event_jump(g, ev.eventdat as u16),

        55 => {
            // Enemy Global AccelRev
            if in_new_pl(ev.eventdat3) {
                g.tyrian2.event_rec[loc].eventdat4 = g.varz.new_pl[(ev.eventdat3 - 80) as usize];
            }
            let ev = g.tyrian2.event_rec[loc];

            for e in g.varz.enemy.iter_mut() {
                if ev.eventdat4 == 0 || e.linknum == ev.eventdat4 {
                    if ev.eventdat != -99 {
                        e.xaccel = ev.eventdat as u8;
                    }
                    if ev.eventdat2 != -99 {
                        e.yaccel = ev.eventdat2 as u8;
                    }
                }
            }
            g.varz.temp = 100;
        }

        56 => {
            // Ground2 Bottom
            je_create_new_event_enemy(g, 0, 75, 0);
            set_last_enemy_y(g, 190);
        }

        57 => g.varz.super_enemy254_jump = ev.eventdat as u16,

        60 => {
            // Assign Special Enemy
            for e in g.varz.enemy.iter_mut() {
                if e.linknum == ev.eventdat4 {
                    e.special = true;
                    e.flagnum = ev.eventdat as u8;
                    e.setto = ev.eventdat2 == 1;
                }
            }
            g.varz.temp = 100;
        }

        61 => {
            // if specific flag set to specific value, skip events
            let flag = g.varz.global_flags[(i32::from(ev.eventdat) - 1) as usize];
            if i16::from(flag) == ev.eventdat2 {
                g.varz.event_loc = (i32::from(g.varz.event_loc) + i32::from(ev.eventdat3)) as u16;
            }
        }

        62 => g.varz.sound_queue[3] = ev.eventdat as u8, // Play sound effect

        63 => {
            // skip events if not in 2-player mode
            if !g.config.two_player_mode && !g.config.one_player_action {
                g.varz.event_loc = (i32::from(g.varz.event_loc) + i32::from(ev.eventdat)) as u16;
            }
        }

        64 => {
            if !(ev.eventdat == 6
                && g.config.two_player_mode
                && g.config.difficulty_level > DIFFICULTY_NORMAL)
            {
                g.config.smoothies[(i32::from(ev.eventdat) - 1) as usize] = ev.eventdat2 != 0;
                g.varz.temp = ev.eventdat as u8;
                if g.varz.temp == 5 {
                    g.varz.temp = 3;
                }
                g.backgrnd.smoothie_data[usize::from(g.varz.temp) - 1] = ev.eventdat3 as u8;
            }
        }

        65 => g.varz.background3x1 = ev.eventdat == 0,

        66 => {
            // If not on this difficulty level or higher then...
            if i16::from(g.config.initial_difficulty) <= ev.eventdat {
                g.varz.event_loc = (i32::from(g.varz.event_loc) + i32::from(ev.eventdat2)) as u16;
            }
        }

        67 => {
            g.varz.level_timer = ev.eventdat == 1;
            g.varz.level_timer_countdown = (i32::from(ev.eventdat3) * 100) as u16;
            g.varz.level_timer_jump_to = ev.eventdat2 as u16;
        }

        68 => g.varz.random_explosions = ev.eventdat == 1,

        69 => {
            for p in g.player.player.iter_mut() {
                p.invulnerable_ticks = i32::from(ev.eventdat) as u32;
            }
        }

        70 => {
            if ev.eventdat2 == 0 {
                // 1-10
                let mut found = false;

                g.varz.temp = 1;
                while g.varz.temp <= 19 {
                    let t = g.varz.temp;
                    found = found || je_search_for(g, t, None);
                    g.varz.temp += 1;
                }

                if !found {
                    je_event_jump(g, ev.eventdat as u16);
                }
            } else if !je_search_for(g, ev.eventdat2 as u8, None)
                && (ev.eventdat3 == 0 || !je_search_for(g, ev.eventdat3 as u8, None))
                && (ev.eventdat4 == 0 || !je_search_for(g, ev.eventdat4, None))
            {
                je_event_jump(g, ev.eventdat as u16);
            }
        }

        71 => {
            // C divides the byte distance (`intptr_t`) by `sizeof(JE_byte *)`
            // (`size_t`), so the row offset compared is unsigned.
            const PTR: u64 = 8;
            let pos = g.backgrnd.map_y_pos.expect("mapYPos is set") as i64;
            let rows = (pos.wrapping_mul(PTR as i64) as u64) / PTR;
            if rows.wrapping_mul(2) <= u64::from(ev.eventdat2 as u32) {
                je_event_jump(g, ev.eventdat as u16);
            }
        }

        72 => g.varz.background3x1b = ev.eventdat == 1,
        73 => g.config.sky_enemy_over_all = ev.eventdat == 1,

        74 => {
            // Enemy Global BounceParams
            for e in g.varz.enemy.iter_mut() {
                if ev.eventdat4 == 0 || e.linknum == ev.eventdat4 {
                    if ev.eventdat5 != -99 {
                        e.xminbounce = i16::from(ev.eventdat5);
                    }

                    if ev.eventdat6 != -99 {
                        e.yminbounce = i16::from(ev.eventdat6);
                    }

                    if ev.eventdat != -99 {
                        e.xmaxbounce = ev.eventdat;
                    }

                    if ev.eventdat2 != -99 {
                        e.ymaxbounce = ev.eventdat2;
                    }
                }
            }
            g.varz.temp = 100;
        }

        75 => {
            let mut temp_no_clue = false;

            for i in 0..100 {
                let e = &g.varz.enemy[i];
                if g.varz.enemy_avail[i] == 0
                    && e.eyc == 0
                    && i16::from(e.linknum) >= ev.eventdat
                    && i16::from(e.linknum) <= ev.eventdat2
                {
                    temp_no_clue = true;
                }
            }
            g.varz.temp = 100;

            let pl = (i32::from(ev.eventdat3) - 80) as usize;
            if temp_no_clue {
                let span = (i32::from(ev.eventdat2) + 1 - i32::from(ev.eventdat)) as u64;
                loop {
                    let r = mt_rand(g) % span;
                    g.varz.temp = (r as i64 + i64::from(ev.eventdat)) as u8;
                    let mut enemy_i = 0u8;
                    let t = g.varz.temp;
                    if je_search_for(g, t, Some(&mut enemy_i))
                        && g.varz.enemy[usize::from(enemy_i)].eyc == 0
                    {
                        break;
                    }
                }

                g.varz.new_pl[pl] = g.varz.temp;
            } else {
                g.varz.new_pl[pl] = 255;
                if ev.eventdat4 > 0 {
                    // Skip
                    let target = loc + usize::from(ev.eventdat4);
                    g.varz.cur_loc = g.tyrian2.event_rec[target].eventtime.wrapping_sub(1);
                    g.varz.event_loc =
                        (i32::from(g.varz.event_loc) + i32::from(ev.eventdat4) - 1) as u16;
                }
            }
        }

        76 => g.varz.return_active = true,

        77 => {
            g.backgrnd.map_y_pos = Some(isize::from(ev.eventdat / 2));
            g.backgrnd.map_y2_pos = Some(isize::from(if ev.eventdat2 > 0 {
                ev.eventdat2 / 2
            } else {
                ev.eventdat / 2
            }));
        }

        78 => {
            if g.varz.galaga_shot_freq < 10 {
                g.varz.galaga_shot_freq += 1;
            }
        }

        79 => {
            g.tyrian2.boss_bar[0].link_num = ev.eventdat as u8;
            g.tyrian2.boss_bar[1].link_num = ev.eventdat2 as u8;
        }

        80 => {
            // skip events if in 2-player mode
            if g.config.two_player_mode {
                g.varz.event_loc = (i32::from(g.varz.event_loc) + i32::from(ev.eventdat)) as u16;
            }
        }

        81 => {
            // WRAP2
            g.varz.bkwrap2 = Some(isize::from(ev.eventdat / 2));
            g.varz.bkwrap2to = Some(isize::from(ev.eventdat2 / 2));
        }

        82 => {
            // Give SPECIAL WEAPON
            g.player.player[0].items.special = ev.eventdat as u8;
            let c = &mut g.config;
            c.shot_multi_pos[crate::config::SHOT_SPECIAL] = 0;
            c.shot_repeat[crate::config::SHOT_SPECIAL] = 0;
            c.shot_multi_pos[crate::config::SHOT_SPECIAL2] = 0;
            c.shot_repeat[crate::config::SHOT_SPECIAL2] = 0;
        }

        _ => {
            let message = format!(
                "Event {:05} {} has invalid type.",
                ev.eventtime, ev.eventtype
            );
            g.platform.log_warn(LOG_CATEGORY, &message);
        }
    }

    g.varz.event_loc = g.varz.event_loc.wrapping_add(1);
    Ok(())
}

/// `if (b > 0) enemy[b-1].ey = y;` after creating an event enemy.
fn set_last_enemy_y(g: &mut Game, y: i32) {
    if g.varz.b > 0 {
        g.varz.enemy[(g.varz.b - 1) as usize].ey = y as i16;
    }
}

/// Events 33 and 45: an enemy that drops item 533 may drop a random special
/// weapon instead, more likely with more lives.
fn random_special_drop(g: &mut Game, loc: usize) {
    let lives = g.player.player[0].lives();

    if g.tyrian2.event_rec[loc].eventdat == 533
        && (lives == 11 || (mt_rand(g) % 15) < u64::from(lives))
    {
        // enemy will drop random special weapon
        g.tyrian2.event_rec[loc].eventdat = 829 + (mt_rand(g) % 6) as i16;
    }
}

/// Events 33 and 45: enemies with the event's link number drop its item.
fn set_enemy_die(g: &mut Game, loc: usize) {
    let ev = g.tyrian2.event_rec[loc];
    for e in g.varz.enemy.iter_mut() {
        if e.linknum == ev.eventdat4 {
            e.enemydie = ev.eventdat as u16;
        }
    }
    g.varz.temp = 100;
}

#[doc(alias = "networkStartScreen")]
pub fn network_start_screen(g: &mut Game) -> Flow {
    let vga = g.video.vga_screen_id();
    let vga2 = g.video.vga_screen2.expect("VGAScreen2");
    crate::picload::je_load_pic(g, vga, 2, false)?;
    copy_screen(g, vga, vga2);
    let small = crate::sprite::SMALL_FONT_SHAPES as u32;
    let text = b"Waiting for other player.";
    let x = crate::fonthand::je_font_center(g, text, small);
    crate::fonthand::je_d_string(g, vga, x, 140, text, small);
    crate::video::je_show_vga(g);
    let colors = g.palette.colors;
    crate::palette::fade_palette(g, &colors, 10, 0, 255)?;

    crate::network::network_connect(g)?;

    g.config.two_player_mode = true;
    if g.network.this_player_num == 1 {
        crate::palette::fade_black(g, 10)?;

        if crate::menus::episode_select(g)? && crate::menus::difficulty_select(g)? {
            g.config.initial_difficulty = g.config.difficulty_level;

            // Make it one step harder for 2-player mode!
            g.config.difficulty_level = g.config.difficulty_level.wrapping_add(1);

            crate::network::network_prepare(g, crate::network::PACKET_DETAILS);
            let (episode, difficulty) = (
                u16::from(g.episodes.episode_num),
                i32::from(g.config.difficulty_level) as u16,
            );
            let p = g.network.packet_out_temp.as_mut().expect("packet_out_temp");
            p.data[4..6].copy_from_slice(&episode.to_be_bytes());
            p.data[6..8].copy_from_slice(&difficulty.to_be_bytes());
            crate::network::network_send(g, 8); // PACKET_DETAILS
        } else {
            crate::network::network_prepare(g, crate::network::PACKET_QUIT);
            crate::network::network_send(g, 4); // PACKET QUIT

            crate::network::network_tyrian_halt(g, 0, true)?;
        }
    } else {
        copy_screen(g, vga2, vga);
        let text = g.helptext.network_text[4 - 1];
        let x = crate::fonthand::je_font_center(g, &text, small);
        crate::fonthand::je_d_string(g, vga, x, 140, &text, small);
        crate::video::je_show_vga(g);

        // until opponent sends details packet
        let details = |g: &Game| {
            g.network.packet_in[0]
                .as_ref()
                .map(|p| u16::from_be_bytes([p.data[0], p.data[1]]))
        };
        loop {
            crate::nortsong::set_frame_count(g, 1);

            if details(g) == Some(crate::network::PACKET_DETAILS) {
                break;
            }

            crate::network::network_update(g);

            crate::keyboard::wait_until_elapsed(g)?;
        }

        let p = g.network.packet_in[0].as_ref().expect("details packet");
        let (episode, difficulty) = (
            u16::from_be_bytes([p.data[4], p.data[5]]),
            u16::from_be_bytes([p.data[6], p.data[7]]),
        );
        crate::episodes::je_init_episode(g, episode as u8)?;
        g.config.difficulty_level = difficulty as i8;
        g.config.initial_difficulty = (i32::from(g.config.difficulty_level) - 1) as i8;
        crate::palette::fade_black(g, 10)?;

        crate::network::network_update(g);
    }

    for p in g.player.player.iter_mut() {
        p.cash = 0;
    }

    g.player.player[0].items.ship = 11; // Silver Ship

    while !crate::network::network_is_sync(g) {
        crate::nortsong::set_frame_count(g, 1);

        crate::keyboard::wait_until_elapsed(g)?;
    }
    Ok(())
}

/// `memcpy(dst->pixels, src->pixels, dst->pitch * dst->h)`.
fn copy_screen(g: &mut Game, src: ScreenId, dst: ScreenId) {
    let (s, d) = g.video.screen_pair_mut(src, dst);
    let n = (d.pitch * d.h) as usize;
    d.pixels[..n].copy_from_slice(&s.pixels[..n]);
}

#[doc(alias = "newGame")]
pub fn new_game(g: &mut Game) -> Flow<bool> {
    if crate::menus::gameplay_select(g)? {
        if crate::menus::episode_select(g)? && crate::menus::difficulty_select(g)? {
            g.varz.game_loaded = true;
        }

        g.config.initial_difficulty = g.config.difficulty_level;

        let p = &mut g.player.player;
        if g.config.one_player_action {
            p[0].cash = 0;

            p[0].items.ship = 8; // Stalker
        } else if g.config.two_player_mode {
            for pl in p.iter_mut() {
                pl.cash = 0;
            }

            p[0].items.ship = 11; // Silver Ship

            // Make it one step harder for 2-player mode!
            g.config.difficulty_level = g.config.difficulty_level.wrapping_add(1);

            g.config.input_device = [1, 2];
        } else if g.params.rich_mode {
            p[0].cash = 1_000_000;
        } else if g.varz.game_loaded {
            // allows player to smuggle arcade/super-arcade ships into full game
            const INITIAL_CASH: [u64; 4] = [10000, 15000, 20000, 30000];

            let episode = usize::from(g.episodes.episode_num);
            debug_assert!((1..=INITIAL_CASH.len()).contains(&episode));
            p[0].cash = INITIAL_CASH[episode - 1];
        }
    }

    Ok(g.varz.game_loaded)
}

#[doc(alias = "newSuperArcadeGame")]
pub fn new_super_arcade_game(g: &mut Game, i: u32) -> Flow<bool> {
    let i = i as usize;
    g.player.player[0].items.ship = crate::varz::SA_SHIP[i];

    if crate::menus::episode_select(g)? && crate::menus::difficulty_select(g)? {
        // Start special mode!
        let vga = g.video.vga_screen_id();
        crate::picload::je_load_pic(g, vga, 1, false)?;
        crate::video::je_clr256(g, vga);
        let title = g.helptext.super_ships[0];
        let x = crate::fonthand::je_font_center(g, &title, FONT_SHAPES as u32);
        crate::fonthand::je_d_string(g, vga, x, 30, &title, FONT_SHAPES as u32);
        let name = g.helptext.super_ships[i + 1];
        let small = crate::sprite::SMALL_FONT_SHAPES as u32;
        let x = crate::fonthand::je_font_center(g, &name, small);
        crate::fonthand::je_d_string(g, vga, x, 100, &name, small);
        g.varz.temp_w = g.episodes.ships[usize::from(g.player.player[0].items.ship)].shipgraphic;
        if g.varz.temp_w != 1 {
            let w = u32::from(g.varz.temp_w);
            crate::sprite::blit_sprite2x2(g, vga, 148, 70, Sprite2Ref::SpriteSheet9, w);
        }

        crate::video::je_show_vga(g);
        let colors = g.palette.colors;
        crate::palette::fade_palette(g, &colors, 50, 0, 255)?;

        crate::keyboard::wait_until_get_input(g)?;

        let c = &mut g.config;
        c.two_player_mode = false;
        c.one_player_action = true;
        c.super_arcade_mode = (i + 1) as u8;
        g.varz.game_loaded = true;
        c.difficulty_level = c.difficulty_level.wrapping_add(1);
        c.initial_difficulty = c.difficulty_level;

        let p = &mut g.player.player[0];
        p.cash = 0;

        p.items.weapon[crate::player::FRONT_WEAPON].id = crate::varz::SA_WEAPON[i][0] as u8;
        p.items.special = crate::varz::SA_SPECIAL_WEAPON[i] as u8;
        if c.super_arcade_mode == crate::varz::SA_NORTSHIPZ {
            p.items.sidekick = [24; 2]; // Companion Ship Quicksilver
        }

        crate::palette::fade_black(g, 10)?;
    }

    Ok(g.varz.game_loaded)
}

/// `SDL_SCANCODE_*` keys of the title menu.
const SDL_SCANCODE_RETURN: u16 = 40;
const SDL_SCANCODE_ESCAPE: u16 = 41;
const SDL_SCANCODE_SPACE: u16 = 44;
const SDL_SCANCODE_DOWN: u16 = 81;
const SDL_SCANCODE_UP: u16 = 82;
/// `SDL_BUTTON_LEFT`, `SDL_BUTTON_RIGHT`.
const SDL_BUTTON_LEFT: u8 = 1;
const SDL_BUTTON_RIGHT: u8 = 3;

/// `titleScreen()`: the title menu. Returns true to start a game, demo or
/// Destruct, false to quit.
#[doc(alias = "titleScreen")]
pub fn title_screen(g: &mut Game) -> Flow<bool> {
    const MENU_ITEM_NEW_GAME: usize = 0;
    const MENU_ITEM_LOAD_GAME: usize = 1;
    const MENU_ITEM_HIGH_SCORES: usize = 2;
    const MENU_ITEM_INSTRUCTIONS: usize = 3;
    const MENU_ITEM_SETUP: usize = 4;
    const MENU_ITEM_DEMO: usize = 5;
    const MENU_ITEM_QUIT: usize = 6;

    // override "Ordering Info"
    let item = &mut g.helptext.menu_text[4];
    let n = b"Setup".len().min(item.len() - 1);
    item[..n].copy_from_slice(&b"Setup"[..n]);
    item[n] = 0;

    if g.sprite.shop_sprite_sheet.data.is_none() {
        // need mouse pointer sprites
        crate::sprite::je_load_comp_shapes(g, Sprite2Ref::ShopSpriteSheet, b'1')?;
    }

    let mut restart = true;

    let mut selected_index = MENU_ITEM_NEW_GAME;
    let mut special_name_progress = [0usize; crate::varz::SA_ENGAGE as usize];

    let menu_count = g.helptext.menu_text.len();
    let x_center = g.video.vga().w / 2;
    let y_menu_items = 104;
    let h_menu_item = 13;
    let mut w_menu_item = [0i32; 7];
    let normal = crate::font::FONT_NORMAL;

    loop {
        crate::nortsong::set_frame_count(g, 1);

        let vga = g.video.vga_screen_id();
        let vga2 = g.video.vga_screen2.expect("VGAScreen2");
        let seg = g.video.vga_screen_seg.expect("VGAScreenSeg");
        if restart {
            crate::loudness::play_song(g, u32::from(crate::musmast::SONG_TITLE))?;

            crate::picload::je_load_pic(g, vga, 4, false)?;

            let version = g.opentyr.opentyrian_version.clone();
            let small = crate::font::FONT_SMALL;
            crate::font::draw_font_hv_shadow(g, vga, 2, 192, &version, small, 15, 0, false, 1);

            let planet = crate::sprite::PLANET_SHAPES as u32;
            if g.varz.move_tyrian_logo_up {
                copy_screen(g, vga, vga2);

                crate::sprite::blit_sprite(g, seg, 11, 62, planet, 146); // tyrian logo

                let colors = g.palette.colors;
                crate::palette::fade_palette(g, &colors, 10, 0, 255 - 16)?;

                let mut y = 60;
                while y >= 4 {
                    crate::nortsong::set_frame_count(g, 2);

                    copy_screen(g, vga2, vga);

                    crate::sprite::blit_sprite(g, seg, 11, y, planet, 146); // tyrian logo

                    crate::video::je_show_vga(g);

                    crate::keyboard::wait_until_elapsed(g)?;
                    y -= 2;
                }

                g.varz.move_tyrian_logo_up = false;
            } else {
                crate::sprite::blit_sprite(g, seg, 11, 4, planet, 146); // tyrian logo

                let colors = g.palette.colors;
                crate::palette::fade_palette(g, &colors, 10, 0, 255 - 16)?;
            }

            // Draw menu items.
            for (i, w) in w_menu_item.iter_mut().enumerate() {
                let text = g.helptext.menu_text[i];

                *w = crate::fonthand::je_text_width(g, &text, normal as u32);
                let x = x_center - *w / 2;
                let y = y_menu_items + h_menu_item * i as i32;

                let passes = [
                    (-1, -1, -10),
                    (1, 1, -10),
                    (1, -1, -10),
                    (-1, 1, -10),
                    (0, 0, -3),
                ];
                for (dx, dy, value) in passes {
                    crate::font::draw_font_hv(g, vga, x + dx, y + dy, &text, normal, 15, value);
                }
            }

            copy_screen(g, vga, vga2);

            g.mouse.mouse_cursor = crate::mouse::MOUSE_POINTER_NORMAL;

            // Fade in menu items.
            let colors = g.palette.colors;
            crate::palette::fade_palette(g, &colors, 20, 255 - 16 + 1, 255)?;

            restart = false;
        }

        copy_screen(g, vga2, vga);

        // Highlight selected menu item.
        let text = g.helptext.menu_text[selected_index];
        let w = g.video.vga().w;
        crate::font::draw_font_hv_aligned(
            g,
            vga,
            w / 2,
            y_menu_items + h_menu_item * selected_index as i32,
            &text,
            normal,
            crate::font::ALIGN_CENTER,
            15,
            -1,
        );

        crate::mouse::je_mouse_start_filter(g, 0xF0);
        crate::video::je_show_vga(g);
        crate::mouse::je_mouse_replace(g);

        let idle_start_tick = g.platform.get_ticks();

        loop {
            // Play demo after idle for 30 seconds.
            if g.platform.get_ticks().wrapping_sub(idle_start_tick) > 30000 {
                crate::palette::fade_black(g, 15)?;

                g.demo.play_demo = true;
                return Ok(true);
            }

            crate::keyboard::wait_until_elapsed(g)?;

            if crate::keyboard::has_input(g, crate::keyboard::INPUT_ANY) {
                break;
            }

            crate::nortsong::set_frame_count(g, 1);
        }

        // Handle interaction.

        let mut action = false;
        let mut done = false;

        let mut mouse_input = crate::keyboard::MouseInput::default();
        let mut keyboard_input = KeyboardInput::default();

        if crate::keyboard::mouse_get_input(g, crate::keyboard::INPUT_ANY, Some(&mut mouse_input)) {
            // Find menu item that was hovered or clicked.
            for (i, &w) in w_menu_item.iter().enumerate() {
                let x_menu_item = x_center - w / 2;
                if mouse_input.x >= x_menu_item && mouse_input.x < x_menu_item + w {
                    let y_menu_item = y_menu_items + h_menu_item * i as i32;
                    if mouse_input.y >= y_menu_item && mouse_input.y < y_menu_item + h_menu_item {
                        if selected_index != i {
                            crate::nortsong::je_play_sample_num(g, crate::sndmast::S_CURSOR);

                            selected_index = i;
                        }

                        if mouse_input.button == SDL_BUTTON_LEFT {
                            action = true;
                        }

                        break;
                    }
                }
            }

            if mouse_input.button == SDL_BUTTON_RIGHT {
                crate::nortsong::je_play_sample_num(g, crate::sndmast::S_SPRING);

                done = true;
            }
        } else if crate::keyboard::keyboard_get_input(g, Some(&mut keyboard_input)) {
            match keyboard_input.scancode {
                SDL_SCANCODE_UP => {
                    crate::nortsong::je_play_sample_num(g, crate::sndmast::S_CURSOR);

                    selected_index = if selected_index == 0 {
                        menu_count - 1
                    } else {
                        selected_index - 1
                    };
                }
                SDL_SCANCODE_DOWN => {
                    crate::nortsong::je_play_sample_num(g, crate::sndmast::S_CURSOR);

                    selected_index = if selected_index == menu_count - 1 {
                        0
                    } else {
                        selected_index + 1
                    };
                }
                SDL_SCANCODE_SPACE | SDL_SCANCODE_RETURN => action = true,
                SDL_SCANCODE_ESCAPE => {
                    crate::nortsong::je_play_sample_num(g, crate::sndmast::S_SPRING);

                    done = true;
                }
                _ => {}
            }

            let sym = keyboard_input.sym;
            // Convert key code (which is Unicode where possible) to uppercase
            // ASCII.
            let ch = if (0..0x80).contains(&sym) {
                (sym as u8).to_ascii_uppercase()
            } else {
                0
            };

            for i in 0..usize::from(crate::varz::SA_ENGAGE) {
                let name = g.helptext.special_name[i];
                if special_name_progress[i] >= name.len() - 1
                    || ch != name[special_name_progress[i]]
                {
                    special_name_progress[i] = 0;
                    continue;
                }

                special_name_progress[i] += 1;

                if name[special_name_progress[i]] == 0 {
                    if i + 1 == usize::from(crate::varz::SA_DESTRUCT) {
                        crate::palette::fade_black(g, 10)?;

                        g.varz.load_destruct = true;
                        return Ok(true);
                    } else if i + 1 == usize::from(crate::varz::SA_ENGAGE) {
                        crate::nortsong::je_play_sample_num(g, crate::sndmast::V_DATA_CUBE);

                        je_whoa(g)?;
                        let black = crate::palette::Color {
                            r: 0,
                            g: 0,
                            b: 0,
                            a: 0,
                        };
                        crate::palette::set_colors(g, black, 0, 255);

                        new_super_tyrian_game(g)?;
                        return Ok(true);
                    } else {
                        crate::palette::fade_black(g, 10)?;

                        if new_super_arcade_game(g, i as u32)? {
                            return Ok(true);
                        }

                        restart = true;
                    }
                }
            }
        }

        if action {
            crate::nortsong::je_play_sample_num(g, crate::sndmast::S_SELECT);

            match selected_index {
                MENU_ITEM_NEW_GAME => {
                    crate::palette::fade_black(g, 15)?;

                    if new_game(g)? {
                        return Ok(true);
                    }

                    restart = true;
                }
                MENU_ITEM_LOAD_GAME => {
                    crate::palette::fade_black(g, 15)?;

                    if crate::mainint::je_load_screen(g)? {
                        return Ok(true);
                    }

                    restart = true;
                }
                MENU_ITEM_HIGH_SCORES => {
                    crate::palette::fade_black(g, 15)?;

                    crate::mainint::je_high_score_screen(g)?;

                    restart = true;
                }
                MENU_ITEM_INSTRUCTIONS => {
                    crate::palette::fade_black(g, 15)?;

                    crate::mainint::je_help_system(g, 1)?;

                    restart = true;
                }
                MENU_ITEM_SETUP => {
                    crate::palette::fade_black(g, 15)?;

                    crate::opentyr::setup_menu(g)?;

                    restart = true;
                }
                MENU_ITEM_DEMO => {
                    crate::palette::fade_black(g, 15)?;

                    g.demo.play_demo = true;
                    return Ok(true);
                }
                MENU_ITEM_QUIT => {
                    crate::palette::fade_black(g, 15)?;

                    return Ok(false);
                }
                _ => {}
            }
        }

        if done {
            crate::palette::fade_black(g, 15)?;

            return Ok(false);
        }
    }
}
