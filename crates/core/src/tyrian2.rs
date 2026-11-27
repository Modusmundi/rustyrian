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

/// Logs `message` as fatal and exits with `EXIT_FAILURE` (upstream's
/// `logFatal(...); exit(EXIT_FAILURE);`).
fn fatal<T>(g: &mut Game, message: &str) -> Flow<T> {
    crate::logging::log_fatal(g, message);
    Err(Halt { code: EXIT_FAILURE })
}

/// `SDL_strlcpy(dst, src, size)` for C strings in byte buffers: copies up to
/// `size - 1` bytes of `src` and terminates them.
fn strlcpy(dst: &mut [u8], src: &[u8], size: usize) {
    let n = c_str(src).len().min(size - 1);
    dst[..n].copy_from_slice(&src[..n]);
    dst[n] = 0;
}

/// `atoi(s + at)`.
fn atoi_at(s: &[u8], at: usize) -> i32 {
    crate::params::atoi(&s[at..])
}

/// The ship shape tables (`shapeFile`) event 5 can load.
const SHAPE_BANKS: usize = crate::lvlmast::SHAPE_FILE.len();

/// R10 (D-UB-TYRIAN2-1): the level data `JE_eventSystem`, `JE_main` and
/// `JE_eventJump` later use as indices or search targets, checked once at
/// load. Upstream indexes with these values unchecked; every shipped level
/// passes.
fn level_events_valid(g: &Game) -> bool {
    let enemy = |v: u16| usize::from(v) <= crate::lvlmast::ENEMY_NUM;
    let jump = |v: i16| v as u16 <= 65500 || v as u16 == 65535;
    // Events 61/63/66/80 add to `eventLoc` (then `JE_eventSystem` adds 1);
    // `JE_main` reads `eventRec[eventLoc-1]`, so it must stay in 1..=2500.
    let skip = |i: usize, delta: i32| (1..=EVENT_MAXIMUM as i32).contains(&(i as i32 + 2 + delta));
    let t2 = &g.tyrian2;
    let max_event = usize::from(g.varz.max_event);
    let levels_ok = t2.level_enemy[..usize::from(t2.level_enemy_max)]
        .iter()
        .all(|&e| enemy(e));
    levels_ok
        && t2.event_rec[..max_event].iter().enumerate().all(|(i, ev)| {
            let bank = |v: i16| v <= SHAPE_BANKS as i16;
            match ev.eventtype {
                5 => {
                    bank(ev.eventdat)
                        && bank(ev.eventdat2)
                        && bank(i16::from(ev.eventdat3))
                        && usize::from(ev.eventdat4) <= SHAPE_BANKS
                }
                6 | 7 | 10 | 15 | 17 | 18 | 23 | 32 | 56 => enemy(ev.eventdat as u16),
                12 => (ev.eventdat as u16).checked_add(3).is_some_and(enemy),
                33 | 45 => enemy(ev.eventdat as u16),
                38 | 54 | 57 | 70 | 71 => jump(ev.eventdat),
                67 => jump(ev.eventdat2),
                61 => (1..=10).contains(&ev.eventdat) && skip(i, i32::from(ev.eventdat3)),
                62 => usize::from(ev.eventdat as u8) <= crate::sndmast::SOUND_COUNT,
                64 => (1..=9).contains(&ev.eventdat),
                75 => {
                    (80..90).contains(&ev.eventdat3)
                        && i + usize::from(ev.eventdat4) < EVENT_MAXIMUM
                }
                63 | 80 => skip(i, i32::from(ev.eventdat)),
                66 => skip(i, i32::from(ev.eventdat2)),
                _ => true,
            }
        })
}

/// Runs the episode script from section `mainLevel` until it starts a level
/// (then loads that level's events, shapes and maps) or returns to the
/// title screen (`mainLevel` 0).
#[doc(alias = "JE_loadMap")]
pub fn je_load_map(g: &mut Game) -> Flow {
    let mut s = [0u8; 256];

    g.config.last_cube_max = g.config.cube_max;

    // Defaults
    g.musmast.song_buy = crate::musmast::DEFAULT_SONG_BUY; // Item Screen default song

    // Load LEVELS.DAT - Section = MAINLEVEL
    g.config.save_level = g.config.main_level;

    'new_game: loop {
        g.config.galaga_mode = false;
        g.mainint.use_last_bank = false;
        g.config.extra_game = false;
        g.network.halt_game = false;

        g.varz.game_loaded = false;

        if g.demo.play_demo {
            break;
        }

        loop {
            let name = String::from_utf8_lossy(c_str(&g.episodes.episode_filename)).into_owned();
            let mut episode_file = file::data_file_open(g, &name, "rb");
            if episode_file.error {
                let message = format!(
                    "Failed to open file '{}': {}",
                    name,
                    file::file_get_error(&episode_file)
                );
                return fatal(g, &message);
            }

            g.mainint.jump_section = false;
            g.tyrian2.load_level_ok = false;

            // Seek Section # Mainlevel
            let mut i = 0;
            while i < g.config.main_level {
                read_episode_string(g, &mut episode_file, &mut s)?;

                if s[0] == b'*' {
                    s[0] = b' ';
                    i += 1;
                }
            }

            g.keyboard.esc_pressed = false;

            loop {
                if g.varz.game_loaded {
                    file::file_close(&mut episode_file);

                    if g.config.main_level == 0 {
                        // if quit itemscreen: back to title screen
                        return Ok(());
                    }
                    continue 'new_game;
                }

                s.fill(0);

                read_episode_string(g, &mut episode_file, &mut s)?;

                if s[0] == b']' && script_command(g, &mut episode_file, &mut s)? == Script::Return {
                    return Ok(());
                }

                if g.tyrian2.load_level_ok || g.mainint.jump_section {
                    break;
                }
            }

            file::file_close(&mut episode_file);

            if g.tyrian2.load_level_ok {
                break;
            }
        }
        break;
    }

    if g.demo.play_demo {
        crate::demo::begin_play_demo(g)?;
    } else {
        crate::palette::fade_black(g, 50)?;

        if g.demo.record_demo {
            crate::demo::begin_record_demo(g)?;
        }
    }

    load_level_file(g)
}

/// What the episode script asks of `JE_loadMap` after a command.
#[derive(PartialEq)]
enum Script {
    Continue,
    /// Back to the title screen (`mainLevel` is 0).
    Return,
}

/// One `]X` command of the episode script (`s`), reading further lines of
/// `episode_file` where the command has them.
fn script_command(g: &mut Game, episode_file: &mut File, s: &mut [u8; 256]) -> Flow<Script> {
    match s[1] {
        b'A' => crate::animlib::play_anim(g, "tyrend.anm", 0, 7)?, // Show animation.

        b'G' => {
            // Set next level choices.
            g.varz.map_origin = atoi_at(s, 4) as u16;
            g.varz.map_p_num = atoi_at(s, 7) as u16;
            // R10 (D-UB-TYRIAN2-2): at most `mapPlanet`'s five choices.
            let n = g.varz.map_planet.len();
            if usize::from(g.varz.map_p_num) > n {
                g.varz.map_p_num = n as u16;
            }
            for i in 0..usize::from(g.varz.map_p_num) {
                g.varz.map_planet[i] = atoi_at(s, 1 + (i + 1) * 8) as u8;
                g.varz.map_section[i] = atoi_at(s, 4 + (i + 1) * 8) as u8;
            }
        }

        b'?' => {
            // Set data cubes.
            let c = &mut g.config;
            g.varz.temp = atoi_at(s, 4) as u8;
            if usize::from(g.varz.temp) > c.cube_list.len() {
                g.varz.temp = c.cube_list.len() as u8;
            }
            for i in 0..usize::from(g.varz.temp) {
                c.cube_list[i] = atoi_at(s, 3 + (i + 1) * 4) as u16;
            }
            if c.cube_max > u16::from(g.varz.temp) {
                c.cube_max = u16::from(g.varz.temp);
            }
        }

        b'!' => {
            // Set number of data cubes acquired.
            let c = &mut g.config;
            c.cube_max = atoi_at(s, 4) as u16; // Auto set CubeMax
            c.cube_max = c.cube_max.min(c.cube_list.len() as u16);
        }

        b'+' => {
            // Increase number of data cubes acquired.
            g.varz.temp = atoi_at(s, 4) as u8;
            let c = &mut g.config;
            c.cube_max = c.cube_max.wrapping_add(u16::from(g.varz.temp));
            c.cube_max = c.cube_max.min(c.cube_list.len() as u16);
        }

        b'g' => {
            // Enable GALAGA mode.  (Used for bonus games.)
            g.config.galaga_mode = true;

            let p = &mut g.player.player;
            p[1].items = p[0].items;
            p[1].items.weapon[crate::player::REAR_WEAPON].id = 15; // Vulcan Cannon
            p[1].items.sidekick = [0; 2]; // None
        }

        b'x' => g.config.extra_game = true, // Enable bonus game.

        b'e' => {
            // Enable ENGAGE mode.  (Used for bonus games.)
            g.varz.do_not_save_backup = true;
            g.params.constant_die = false;
            g.config.one_player_action = true;
            g.config.super_tyrian = true;
            g.config.two_player_mode = false;

            let p = &mut g.player.player[0];
            p.cash = 0;

            let it = &mut p.items;
            it.ship = 13; // The Stalker 21.126
            it.weapon[crate::player::FRONT_WEAPON].id = 39; // Atomic RailGun
            it.weapon[crate::player::REAR_WEAPON].id = 0; // None
            it.sidekick = [0; 2]; // None
            it.generator = 2; // Advanced MR-12
            it.shield = 4; // Advanced Integrity Field
            it.special = 0; // None

            it.weapon[crate::player::FRONT_WEAPON].power = 3;
            it.weapon[crate::player::REAR_WEAPON].power = 1;
        }

        b'J' | b'2' | b'w' | b't' | b'l' => {
            // Jump to section: always, in two-player or one-player arcade,
            // with the Stalker 21.126, when the level timer expired, or when
            // a player died.
            g.varz.temp = atoi_at(s, 3) as u8;
            let jump = match s[1] {
                b'J' => true,
                b'2' => g.config.two_player_mode || g.config.one_player_action,
                b'w' => g.player.player[0].items.ship == 13,
                b't' => g.varz.level_timer && g.varz.level_timer_countdown == 0,
                _ => !crate::player::all_players_alive(g),
            };
            if jump {
                g.config.main_level = g.varz.temp;
                g.mainint.jump_section = true;
            }
        }

        b's' => g.config.save_level = g.config.main_level, // store savepoint

        b'b' => {
            // Explicit auto-save.  (Used before bonus games.)
            g.varz.temp = if g.config.two_player_mode { 22 } else { 11 };
            crate::config::je_save_game(g, 11, b"LAST LEVEL    ");
        }

        b'i' => {
            // Set menu music track.
            g.varz.temp = atoi_at(s, 3) as u8;
            g.musmast.song_buy = g.varz.temp.wrapping_sub(1);
        }

        b'I' => {
            // Menu: load items available information.
            g.tyrian2.item_avail = Default::default();

            for i in 0..9 {
                read_episode_string(g, episode_file, s)?;

                let mut buf = [0u8; 256];
                if c_str(s).len() > 8 {
                    let tail = c_str(&s[8..]);
                    buf[..tail.len()].copy_from_slice(tail);
                }

                let mut j = 0;
                let mut temp = 0;
                while crate::mainint::str_pop_int(&mut buf, &mut temp) {
                    // R10 (D-UB-TYRIAN2-2): at most a row's ten items.
                    if j < g.tyrian2.item_avail[i].len() {
                        g.tyrian2.item_avail[i][j] = temp as u8;
                        j += 1;
                    }
                }
                g.tyrian2.item_avail_max[i] = j as u8;
            }

            crate::game_menu::je_item_screen(g)?;
        }

        b'L' => {
            // Play level.
            let c = &mut g.config;
            c.next_level = atoi_at(s, 9) as u8;
            strlcpy(&mut c.level_name, &s[13..], 10);
            g.varz.level_song = atoi_at(s, 22) as u8;
            if c.next_level == 0 {
                c.next_level = c.main_level.wrapping_add(1);
            }
            g.varz.lvl_file_num = atoi_at(s, 25) as u8;
            g.tyrian2.load_level_ok = true;
            let len = c_str(s).len();
            g.varz.bonus_level_current = len > 28 && s[28] == b'$';
            g.varz.normal_bonus_level_current = len > 27 && s[27] == b'$';
            c.game_just_loaded = false;
        }

        b'@' => g.mainint.use_last_bank = !g.mainint.use_last_bank, // Toggle text color bank.

        b'Q' => return end_of_episode(g, episode_file, s),

        b'P' => {
            // Show picture or clear and set palette.
            if !g.params.constant_play {
                let temp_x = atoi_at(s, 3) as u16;
                let vga = g.video.vga_screen_id();
                if temp_x > 900 {
                    // R10 (D-UB-TYRIAN2-2): `pcxpal` has 13 entries.
                    let Some(&pal) = crate::pcxmast::PCXPAL.get(usize::from(temp_x) - 1 - 900)
                    else {
                        return Ok(Script::Continue);
                    };
                    g.palette.colors = g.palette.palettes[usize::from(pal)];
                    crate::video::je_clr256(g, vga);
                    crate::video::je_show_vga(g);
                    let colors = g.palette.colors;
                    crate::palette::fade_palette(g, &colors, 1, 0, 255)?;
                } else {
                    if temp_x == 0 {
                        crate::pcxload::je_load_pcx(g, "tshp2.pcx")?;
                    } else {
                        crate::picload::je_load_pic(g, vga, temp_x as u8, false)?;
                    }

                    crate::video::je_show_vga(g);
                    let colors = g.palette.colors;
                    crate::palette::fade_palette(g, &colors, 10, 0, 255)?;
                }
            }
        }

        b'U' | b'V' | b'R' => {
            // Pan up to, slide up, or pan right to a picture.
            if !g.params.constant_play {
                pan_to_picture(g, s[1], atoi_at(s, 3) as u16)?;
            }
        }

        b'C' => {
            // Fade to black, clear, and reset palette.
            if !g.network.is_network_game {
                crate::palette::fade_black(g, 10)?;
            }
            let vga = g.video.vga_screen_id();
            crate::video::je_clr256(g, vga);
            crate::video::je_show_vga(g);
            g.palette.colors = g.palette.palettes[7];
            let colors = g.palette.colors;
            crate::palette::set_palette(g, &colors, 0, 255);
        }

        b'B' => {
            // Fade to black.
            if !g.network.is_network_game {
                crate::palette::fade_black(g, 10)?;
            }
        }

        b'F' => {
            // Flash and clear.
            if !g.network.is_network_game {
                crate::palette::fade_white(g, 100)?;
                crate::palette::fade_black(g, 30)?;
            }
            let vga = g.video.vga_screen_id();
            crate::video::je_clr256(g, vga);
            crate::video::je_show_vga(g);
        }

        b'W' => {
            // Show text, optionally with warning flashers and sirens.
            if !g.params.constant_play && !g.keyboard.esc_pressed {
                let f = &mut g.fonthand;
                f.warning_col = 14 * 16 + 5;
                f.warning_col_change = 1;
                f.warning_sound_delay = 0;
                f.level_warning_display = s[2] == b'y';
                f.level_warning_lines = 0;
                g.nortsong.frame_count_max = atoi_at(s, 4) as u16;
                crate::nortsong::set_frame_count2(g, 6);
                g.fonthand.warning_red = g.nortsong.frame_count_max / 10 != 0;
                g.nortsong.frame_count_max %= 10;

                read_warning_lines(g, episode_file, s, "Text has too many lines.")?;

                je_display_text(g)?;
            }
        }

        b'H' => {
            // Jump to section if difficulty is less than hard.
            if g.config.initial_difficulty < crate::config::DIFFICULTY_HARD {
                g.config.main_level = atoi_at(s, 4) as u8;
                g.mainint.jump_section = true;
            }
        }

        b'h' => {
            // Skip next line of script if difficulty is hard or higher.
            if g.config.initial_difficulty > DIFFICULTY_NORMAL {
                read_episode_string(g, episode_file, s)?;
            }
        }

        b'S' => {
            // (Not used.)
            if g.network.is_network_game {
                je_read_text_sync(g);
            }
        }

        b'n' => g.keyboard.esc_pressed = false, // End of scene.

        b'M' => {
            // Play music track.
            g.varz.temp = atoi_at(s, 3) as u8;
            crate::loudness::play_song(g, (i32::from(g.varz.temp) - 1) as u32)?;
        }

        _ => {}
    }
    Ok(Script::Continue)
}

/// Reads text lines into `levelWarningText` up to a line starting with `#`,
/// warning (`too_many`) about lines past the tenth.
fn read_warning_lines(
    g: &mut Game,
    episode_file: &mut File,
    s: &mut [u8; 256],
    too_many: &str,
) -> Flow {
    loop {
        read_episode_string(g, episode_file, s)?;

        if s[0] == b'#' {
            return Ok(());
        }

        let f = &mut g.fonthand;
        if usize::from(f.level_warning_lines) >= f.level_warning_text.len() {
            g.platform.log_warn(LOG_CATEGORY, too_many);
            continue;
        }

        let line = &mut f.level_warning_text[usize::from(f.level_warning_lines)];
        let size = line.len();
        strlcpy(line, s, size);
        f.level_warning_lines += 1;
    }
}

/// `]Q`: end of episode. Shows the score and a random hint, moves to the
/// next episode and, after the last one, shows the super-arcade code.
fn end_of_episode(g: &mut Game, episode_file: &mut File, s: &mut [u8; 256]) -> Flow<Script> {
    g.keyboard.esc_pressed = false;
    let r = (mt_rand(g) % 3) * 3;
    g.varz.temp = (u64::from(g.config.secret_hint) + r) as u8;

    let misc =
        |g: &Game, i: usize| String::from_utf8_lossy(c_str(&g.helptext.misc_text[i])).into_owned();
    if g.config.two_player_mode {
        for i in 0..2 {
            let text = format!("{} {}", misc(g, 40 + i), g.player.player[i].cash);
            crate::episodes::snprintf_into(&mut g.fonthand.level_warning_text[i], &text);
        }
        g.fonthand.level_warning_text[2][0] = 0;
        g.fonthand.level_warning_lines = 3;
    } else {
        let score = crate::mainint::je_total_score(g, 0);
        let text = format!("{} {}", misc(g, 37), score);
        // Bounded: upstream's `sprintf` has no limit (D-UB-TYRIAN2-2).
        crate::episodes::snprintf_into(&mut g.fonthand.level_warning_text[0], &text);
        g.fonthand.level_warning_text[1][0] = 0;
        g.fonthand.level_warning_lines = 2;
    }

    // Skip to hint.
    for _ in 0..i32::from(g.varz.temp) - 1 {
        loop {
            read_episode_string(g, episode_file, s)?;

            if s[0] == b'#' {
                break;
            }
        }
    }

    // Read hint.
    read_warning_lines(g, episode_file, s, "Hint has too many lines.")?;

    g.nortsong.frame_count_max = 4;
    if !g.params.constant_play {
        je_display_text(g)?;
    }

    crate::palette::fade_black(g, 15)?;

    crate::mainint::je_next_episode(g)?;

    if g.episodes.jump_back_to_episode1 && !g.config.two_player_mode {
        let vga = g.video.vga_screen_id();
        crate::picload::je_load_pic(g, vga, 1, false)?; // huh?
        crate::video::je_clr256(g, vga);

        if g.config.super_tyrian {
            // if completed Zinglon's Revenge, show SuperTyrian and Destruct
            // codes; if completed SuperTyrian, show Nort-Ship Z code
            g.config.super_arcade_mode = if g.config.initial_difficulty == DIFFICULTY_ZINGLON {
                8
            } else {
                1
            };
        }

        if g.config.super_arcade_mode < crate::varz::SA_ENGAGE {
            let next = crate::varz::SA_NEXT_SHIP[usize::from(g.config.super_arcade_mode)];
            let font = FONT_SHAPES as u32;
            let small = crate::sprite::SMALL_FONT_SHAPES as u32;
            let text = |b: &[u8]| String::from_utf8_lossy(c_str(b)).into_owned();
            if next == crate::varz::SA_ENGAGE {
                let buffer = format!(
                    "{} {}",
                    text(&g.helptext.misc_text_b[4]),
                    text(&g.helptext.p_name[0])
                );
                let x = crate::fonthand::je_font_center(g, buffer.as_bytes(), font);
                crate::fonthand::je_d_string(g, vga, x, 100, buffer.as_bytes(), font);

                let buffer = format!("Or play... {}", text(&g.helptext.special_name[7]));
                crate::fonthand::je_d_string(g, vga, 80, 180, buffer.as_bytes(), small);
            } else {
                let title = g.helptext.super_ships[0];
                let x = crate::fonthand::je_font_center(g, &title, font);
                crate::fonthand::je_d_string(g, vga, x, 30, &title, font);
                let name = g.helptext.super_ships[usize::from(next)];
                let x = crate::fonthand::je_font_center(g, &name, small);
                crate::fonthand::je_d_string(g, vga, x, 100, &name, small);
            }

            if next < crate::varz::SA_NORTSHIPZ {
                let ship = crate::varz::SA_SHIP[usize::from(next) - 1];
                let graphic = g.episodes.ships[usize::from(ship)].shipgraphic;
                crate::sprite::blit_sprite2x2(
                    g,
                    vga,
                    148,
                    70,
                    Sprite2Ref::SpriteSheet9,
                    u32::from(graphic),
                );
            } else if next == crate::varz::SA_NORTSHIPZ {
                g.config.trent_win = true;
            }

            let buffer = format!(
                "Type {} at Title",
                text(&g.helptext.special_name[usize::from(next) - 1])
            );
            let x = crate::fonthand::je_font_center(g, buffer.as_bytes(), small);
            crate::fonthand::je_d_string(g, vga, x, 160, buffer.as_bytes(), small);
            crate::video::je_show_vga(g);

            let colors = g.palette.colors;
            crate::palette::fade_palette(g, &colors, 50, 0, 255)?;

            if !g.params.constant_play {
                crate::keyboard::wait_until_get_input(g)?;
            }
        }

        g.mainint.jump_section = true;

        if g.network.is_network_game {
            je_read_text_sync(g);
        }

        if g.config.super_tyrian {
            crate::palette::fade_black(g, 10)?;

            // back to titlescreen
            g.config.main_level = 0;
            return Ok(Script::Return);
        }
    }
    Ok(Script::Continue)
}

/// `]U`, `]V`, `]R`: replaces the screen with picture `temp_x`, panning up,
/// sliding up or panning right, one row or column per frame until input.
fn pan_to_picture(g: &mut Game, how: u8, temp_x: u16) -> Flow {
    let vga = g.video.vga_screen_id();
    let vga2 = g.video.vga_screen2.expect("VGAScreen2");
    copy_screen(g, vga, vga2);

    crate::picload::je_load_pic(g, vga, temp_x as u8, false)?;
    let mut pic_buffer = vec![0u8; 320 * 200];
    pic_buffer.copy_from_slice(&g.video.screen(vga).pixels[..320 * 200]);

    let last_z = if how == b'R' { 318 } else { 199 };
    for z in 0..=last_z {
        if g.keyboard.esc_pressed {
            break;
        }

        crate::nortsong::set_frame_count(g, 1);

        let (v, v2) = g.video.screen_pair_mut(vga, vga2);
        let (pitch, pitch2) = (v.pitch as usize, v2.pitch as usize);
        let (mut vga_at, mut vga2_at) = (0usize, 0usize);
        match how {
            b'U' => {
                let mut pic_at = (199 - z) * 320;
                for y in 0..=199 {
                    if y <= z {
                        v.pixels[vga_at..vga_at + 320]
                            .copy_from_slice(&pic_buffer[pic_at..pic_at + 320]);
                        pic_at += 320;
                    } else {
                        v.pixels[vga_at..vga_at + pitch]
                            .copy_from_slice(&v2.pixels[vga2_at..vga2_at + pitch]);
                        vga2_at += pitch;
                    }
                    vga_at += pitch;
                }
            }
            b'V' => {
                let mut pic_at = 0;
                for y in 0..199 {
                    if y <= 199 - z {
                        v.pixels[vga_at..vga_at + pitch]
                            .copy_from_slice(&v2.pixels[vga2_at..vga2_at + pitch]);
                        vga2_at += pitch;
                    } else {
                        v.pixels[vga_at..vga_at + 320]
                            .copy_from_slice(&pic_buffer[pic_at..pic_at + 320]);
                        pic_at += 320;
                    }
                    vga_at += pitch;
                }
            }
            _ => {
                let mut pic_at = 0;
                for _ in 0..200 {
                    let n = 319 - z;
                    v.pixels[vga_at..vga_at + n]
                        .copy_from_slice(&v2.pixels[vga2_at + z..vga2_at + z + n]);
                    vga_at += 320 - z;
                    vga2_at += pitch2;
                    // D-UB-TYRIAN2-3: upstream's last row writes one byte
                    // past the screen; that byte is dropped.
                    let end = (vga_at + z + 1).min(v.pixels.len());
                    v.pixels[vga_at..end]
                        .copy_from_slice(&pic_buffer[pic_at..pic_at + (end - vga_at)]);
                    vga_at += z;
                    pic_at += 320;
                }
            }
        }

        crate::video::je_show_vga(g);

        if crate::keyboard::wait_until_get_input_or_elapsed(g)? {
            break;
        }
    }

    g.video.screen_mut(vga).pixels[..320 * 200].copy_from_slice(&pic_buffer);
    Ok(())
}

/// The level part of `JE_loadMap`: events, shape references and the three
/// background maps of level `lvlFileNum` of the level file.
fn load_level_file(g: &mut Game) -> Flow {
    let level_filename = String::from_utf8_lossy(c_str(&g.lvllib.level_filename)).into_owned();
    let mut level_file = file::data_file_open(g, &level_filename, "rb");
    if level_file.error {
        let message = format!(
            "Failed to open file '{}': {}",
            level_filename,
            file::file_get_error(&level_file)
        );
        return fatal(g, &message);
    }

    // R10 (D-UB-TYRIAN2-2): the script's level number must name a level.
    let pos_i = (usize::from(g.varz.lvl_file_num))
        .wrapping_sub(1)
        .wrapping_mul(2);
    let Some(&pos) = g.lvllib.lvl_pos.get(pos_i) else {
        return fatal(
            g,
            "Episode script names a level the level file does not have.",
        );
    };
    file::file_set_position(&mut level_file, i64::from(pos));

    let _char_map_file = file::file_read_char(&mut level_file);
    let char_shape_file = file::file_read_char(&mut level_file);
    g.backgrnd.map_x = file::file_read_u16(&mut level_file);
    g.backgrnd.map_x2 = file::file_read_u16(&mut level_file);
    g.backgrnd.map_x3 = file::file_read_u16(&mut level_file);

    g.tyrian2.level_enemy_max = file::file_read_u16(&mut level_file);
    // R10 (UF-2, D-UB-TYRIAN2-1): `levelEnemy` holds 40, and `JE_main` picks
    // one at random (a count of 0 divides by zero).
    let max = usize::from(g.tyrian2.level_enemy_max);
    if max == 0 || max > g.tyrian2.level_enemy.len() {
        return fatal(g, "Level has an invalid number of enemies.");
    }
    file::file_read_u16_array(&mut level_file, &mut g.tyrian2.level_enemy[..max]);

    g.varz.max_event = file::file_read_u16(&mut level_file);
    if usize::from(g.varz.max_event) >= g.tyrian2.event_rec.len() {
        return fatal(g, "Level has too many events.");
    }
    let mut x = 0;
    while x < usize::from(g.varz.max_event) {
        let f = &mut level_file;
        g.tyrian2.event_rec[x] = JeEventRecType {
            eventtime: file::file_read_u16(f),
            eventtype: file::file_read_u8(f),
            eventdat: file::file_read_s16(f),
            eventdat2: file::file_read_s16(f),
            eventdat3: file::file_read_s8(f),
            eventdat5: file::file_read_s8(f),
            eventdat6: file::file_read_s8(f),
            eventdat4: file::file_read_u8(f),
        };
        x += 1;
    }
    g.tyrian2.event_rec[x].eventtime = 65500; // Not needed but just in case

    if !level_events_valid(g) {
        return fatal(g, "Level has an invalid event.");
    }

    // MAP SHAPE LOOKUP TABLE - Each map is directly after level
    let mut map_sh = [[0u16; 128]; 3];
    for m in map_sh.iter_mut() {
        file::file_read_u16be_array(&mut level_file, m);
    }

    // Read Shapes.DAT
    let shape_char = (char_shape_file as u8).to_ascii_lowercase();
    let mut shapes_filename = b"shapes".to_vec();
    shapes_filename.push(shape_char);
    shapes_filename.extend_from_slice(b".dat");
    let shapes_filename = String::from_utf8_lossy(c_str(&shapes_filename)).into_owned();

    let mut shapes_file = file::data_file_open(g, &shapes_filename, "rb");
    if shapes_file.error {
        let message = format!(
            "Failed to open file '{}': {}",
            shapes_filename,
            file::file_get_error(&shapes_file)
        );
        return fatal(g, &message);
    }

    // `ref`: the shape each map byte stands for (by `megaData` shape index),
    // or none. Entries no shape matches stay empty, the zero the oracle's
    // locals start with (D-UB-TYRIAN2-1).
    let mut refs = [[None::<u8>; 128]; 3];
    let mut shape: crate::varz::JeDanCShape = [0; 24 * 28];
    for z in 0..600u16 {
        let shape_blank = file::file_read_bool(&mut shapes_file);
        if shape_blank {
            shape.fill(0);
        } else {
            file::file_read_exactly(&mut shapes_file, &mut shape);
        }
        let fill = u8::from(shape[..(24 * 28) >> 1].iter().all(|&p| p != 0));
        let md = &mut g.varz;

        // Match 1
        for x in 0..=71 {
            if map_sh[0][x] == z + 1 {
                md.mega_data1.shapes[x].sh = shape;

                refs[0][x] = Some(x as u8);
            }
        }

        // Match 2
        for x in 0..=71 {
            if map_sh[1][x] == z + 1 {
                if x != 71 && !shape_blank {
                    md.mega_data2.shapes[x].sh = shape;
                    md.mega_data2.shapes[x].fill = fill;
                    refs[1][x] = Some(x as u8);
                } else {
                    refs[1][x] = None;
                }
            }
        }

        // Match 3
        for x in 0..=71 {
            if map_sh[2][x] == z + 1 {
                if x < 70 && !shape_blank {
                    md.mega_data3.shapes[x].sh = shape;
                    md.mega_data3.shapes[x].fill = fill;
                    refs[2][x] = Some(x as u8);
                } else {
                    refs[2][x] = None;
                }
            }
        }
    }

    if shapes_file.error {
        let message = format!(
            "Failed to read from file '{}': {}",
            shapes_filename,
            file::file_get_error(&shapes_file)
        );
        return fatal(g, &message);
    }

    file::file_close(&mut shapes_file);

    let mut map_buf = vec![0u8; 15 * 600];
    file::file_read_u8_array(&mut level_file, &mut map_buf[..14 * 300]); // MAP NUMBER 1
    if !map_refs_valid(&map_buf[..14 * 300]) {
        return fatal(g, "Level map has an invalid shape.");
    }
    for (row, chunk) in g
        .varz
        .mega_data1
        .mainmap
        .iter_mut()
        .zip(map_buf.as_chunks::<14>().0)
    {
        for (cell, &b) in row.iter_mut().zip(chunk) {
            *cell = refs[0][usize::from(b)];
        }
    }

    file::file_read_u8_array(&mut level_file, &mut map_buf[..14 * 600]); // MAP NUMBER 2
    if !map_refs_valid(&map_buf[..14 * 600]) {
        return fatal(g, "Level map has an invalid shape.");
    }
    for (row, chunk) in g
        .varz
        .mega_data2
        .mainmap
        .iter_mut()
        .zip(map_buf.as_chunks::<14>().0)
    {
        for (cell, &b) in row.iter_mut().zip(chunk) {
            *cell = refs[1][usize::from(b)];
        }
    }

    file::file_read_u8_array(&mut level_file, &mut map_buf[..15 * 600]); // MAP NUMBER 3
    if !map_refs_valid(&map_buf[..15 * 600]) {
        return fatal(g, "Level map has an invalid shape.");
    }
    for (row, chunk) in g
        .varz
        .mega_data3
        .mainmap
        .iter_mut()
        .zip(map_buf.as_chunks::<15>().0)
    {
        for (cell, &b) in row.iter_mut().zip(chunk) {
            *cell = refs[2][usize::from(b)];
        }
    }

    if level_file.error {
        let message = format!(
            "Failed to read from file '{}': {}",
            level_filename,
            file::file_get_error(&level_file)
        );
        return fatal(g, &message);
    }

    file::file_close(&mut level_file);
    Ok(())
}

/// R10 (D-UB-TYRIAN2-1): map bytes index the 128-entry `ref` tables.
fn map_refs_valid(buf: &[u8]) -> bool {
    buf.iter().all(|&b| b < 128)
}

/// The labels of `JE_main` (rule R9).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum MainLabel {
    StartLevel,
    StartLevelFirst,
    LevelLoop,
}

/// `JE_main()`: plays levels until the player quits to the title screen (or
/// a demo ends). Exits only through `Halt`.
#[doc(alias = "JE_main")]
pub fn je_main(g: &mut Game) -> Flow {
    // `uint old_weapon_bar[2]`: only redrawn when they change; set where C
    // declares it, at the end of `start_level_first`.
    let mut old_weapon_bar = [0u32; 2];

    let mut label = MainLabel::StartLevelFirst;
    loop {
        label = match label {
            MainLabel::StartLevel => {
                if main_start_level(g)? {
                    return Ok(());
                }
                MainLabel::StartLevelFirst
            }
            MainLabel::StartLevelFirst => {
                if !main_start_level_first(g)? {
                    return Ok(()); // back to titlescreen
                }
                old_weapon_bar = [0, 0];
                MainLabel::LevelLoop
            }
            MainLabel::LevelLoop => main_level_loop(g, &mut old_weapon_bar)?,
        };
    }
}

/// `start_level`: finishes the level that ended. Returns true when `JE_main`
/// returns (a demo ended).
fn main_start_level(g: &mut Game) -> Flow<bool> {
    crate::keyboard::keyboard_clear_input(g);
    crate::keyboard::mouse_clear_input(g);

    crate::keyboard::mouse_set_relative(g, false);

    if g.config.galaga_mode {
        g.config.two_player_mode = false;
    }

    for sheet in g.sprite.enemy_sprite_sheets.iter_mut() {
        crate::sprite::free_sprite2s(sheet);
    }

    // Normal speed
    if g.config.fast_play != 0 {
        g.config.smooth_scroll = true;
        crate::nortsong::set_frame_speed(g, 0x4300);
    }

    if g.demo.play_demo {
        crate::demo::end_play_demo(g);

        crate::loudness::stop_song(g);
        crate::palette::fade_black(g, 10)?;
    } else if g.demo.record_demo {
        crate::demo::end_record_demo(g)?;
    }

    g.config.difficulty_level = g.config.old_difficulty_level; // Return difficulty to normal

    if !g.demo.play_demo {
        let v = &g.varz;
        if (!crate::player::all_players_dead(g)
            || v.normal_bonus_level_current
            || v.bonus_level_current)
            && !v.player_end_level
        {
            g.config.main_level = g.config.next_level;
            crate::mainint::je_end_level_ani(g)?;

            crate::loudness::fade_song(g);
        } else {
            crate::loudness::fade_song(g);
            crate::palette::fade_black(g, 10)?;

            crate::config::je_load_game(g, if g.config.two_player_mode { 22 } else { 11 })?;
            if g.varz.do_not_save_backup {
                g.config.super_tyrian = false;
                g.config.one_player_action = false;
                g.player.player[0].items.super_arcade_mode = SA_NONE;
            }
            if g.varz.bonus_level_current && !g.varz.player_end_level {
                g.config.main_level = g.config.next_level;
            }
        }
    }
    g.varz.do_not_save_backup = false;

    Ok(g.demo.play_demo)
}

/// `start_level_first`: loads and sets up the next level. Returns false when
/// `JE_main` returns (the item screen quit to the title screen).
fn main_start_level_first(g: &mut Game) -> Flow<bool> {
    let (music, fx) = (g.nortsong.tyr_music_volume, g.nortsong.fx_volume);
    crate::loudness::set_volume(g, music as u8, fx as u8);

    g.varz.end_level = false;
    g.varz.really_end_level = false;
    g.varz.player_end_level = false;
    g.config.extra_game = false;

    g.varz.do_not_save_backup = false;
    je_load_map(g)?;

    if g.config.main_level == 0 {
        // if quit itemscreen: back to titlescreen
        return Ok(false);
    }

    if !g.demo.play_demo {
        crate::keyboard::mouse_set_relative(g, true);
    }

    crate::loudness::fade_song(g);

    for p in g.player.player.iter_mut() {
        p.is_alive = true;
    }

    let c = &mut g.config;
    c.old_difficulty_level = c.difficulty_level;
    if usize::from(g.episodes.episode_num) == crate::episodes::EPISODE_AVAILABLE {
        c.difficulty_level = c.difficulty_level.wrapping_sub(1);
    }
    if c.difficulty_level < crate::config::DIFFICULTY_EASY {
        c.difficulty_level = crate::config::DIFFICULTY_EASY;
    }

    let p = &mut g.player.player;
    (p[0].x, p[0].y) = (100, 180);
    (p[1].x, p[1].y) = (190, 180);

    for pl in p.iter_mut() {
        for j in 0..pl.old_x.len() {
            pl.old_x[j] = pl.x - (19 - j as i32);
            pl.old_y[j] = pl.y - 18;
        }

        pl.last_x_shot_move = pl.x;
        pl.last_y_shot_move = pl.y;
    }

    let vga = g.video.vga_screen_id();
    crate::picload::je_load_pic(g, vga, if g.config.two_player_mode { 6 } else { 3 }, false)?;

    crate::varz::je_draw_options(g);

    let name = g.config.level_name;
    let y = if g.config.two_player_mode { 76 } else { 118 };
    crate::fonthand::je_out_text(g, vga, 268, y, &name, 12, 4);

    crate::video::je_show_vga(g);
    let gamma = g.config.gamma_correction;
    crate::mainint::je_gamma_correct(&mut g.palette.colors, gamma);
    let colors = g.palette.colors;
    crate::palette::fade_palette(g, &colors, 50, 0, 255)?;

    if g.sprite.explosion_sprite_sheet.data.is_none() {
        crate::sprite::je_load_comp_shapes(g, Sprite2Ref::ExplosionSpriteSheet, b'6')?;
    }

    // MAPX will already be set correctly
    let bk = &mut g.backgrnd;
    bk.map_y = 300 - 8;
    bk.map_y2 = 600 - 8;
    bk.map_y3 = 600 - 8;
    bk.map_y_pos = Some(isize::from(bk.map_y as i16) * 14 - 1);
    bk.map_y2_pos = Some(isize::from(bk.map_y2 as i16) * 14 - 1);
    bk.map_y3_pos = Some(isize::from(bk.map_y3 as i16) * 15 - 1);
    bk.map_x_pos = 0;
    bk.map_x_ofs = 0;
    bk.map_x2_pos = 0;
    bk.map_x3_pos = 0;
    bk.map_x3_ofs = 0;
    bk.map_xbp_pos = 0;
    bk.map_x2bp_pos = 0;
    bk.map_x3bp_pos = 0;

    bk.map1_y_delay = 1;
    bk.map1_y_delay_max = 1;
    bk.map2_y_delay = 1;
    bk.map2_y_delay_max = 1;

    g.musmast.music_fade = false;

    let bk = &mut g.backgrnd;
    bk.back_pos = 0;
    bk.back_pos2 = 0;
    bk.back_pos3 = 0;
    g.config.power = 0;
    bk.starfield_speed = 1;

    // Setup player ship graphics
    crate::varz::je_get_ship_info(g);

    for p in g.player.player.iter_mut() {
        p.x_velocity = 0;
        p.y_velocity = 0;

        p.invulnerable_ticks = 100;
    }

    // Initialize Level Data and Debug Mode
    let v = &mut g.varz;
    v.level_end = 255;
    v.level_end_warp = -4;
    v.level_end_fx_wait = 0;
    let f = &mut g.fonthand;
    f.warning_col = 120;
    f.warning_col_change = 1;
    f.warning_sound_delay = 0;
    f.armor_ship_delay = 50;

    g.episodes.bonus_level = false;
    v.ready_to_end_level = false;
    v.first_game_over = true;
    v.event_loc = 1;
    v.cur_loc = 0;
    let bk = &mut g.backgrnd;
    bk.back_move = 1;
    bk.back_move2 = 2;
    bk.back_move3 = 3;
    v.explode_move = 2;
    v.enemies_active = true;
    v.temp = 0;
    while v.temp < 3 {
        g.mainint.button[usize::from(v.temp)] = false;
        v.temp += 1;
    }
    v.stop_backgrounds = false;
    v.stop_background_num = 0;
    v.background3x1 = false;
    v.background3x1b = false;
    let c = &mut g.config;
    c.background3over = 0;
    c.background2over = 1;
    c.top_enemy_over = false;
    c.sky_enemy_over_all = false;
    v.small_enemy_adjust = false;
    c.star_active = true;
    v.enemy_continual_damage = false;
    g.tyrian2.level_enemy_frequency = 96;
    v.quit_requested = false;

    for bar in g.tyrian2.boss_bar.iter_mut() {
        bar.link_num = 0;
    }

    v.force_events = false; // Force events to continue if background movement = 0

    v.super_enemy254_jump = 0; // When Enemy with PL 254 dies

    // Filter Status
    c.filter_active = true;
    c.filter_fade = true;
    c.filter_fade_start = false;
    c.level_filter = -99;
    c.level_brightness = -14;
    c.level_brightness_chg = 1;

    c.background2not_transparent = false;

    // Initially erase power bars
    c.last_power = c.power / 10;

    // Initial Text
    let text = g.helptext.misc_text[20];
    crate::mainint::je_draw_text_window(g, &text);

    // Setup Armor/Shield Data
    g.config.shield_wait = 1;
    let shield = usize::from(g.player.player[0].items.shield);
    g.config.shield_t = (u32::from(g.episodes.shields[shield].tpwr) * 20) as u8;

    for i in 0..g.player.player.len() {
        let s = usize::from(g.player.player[i].items.shield);
        let p = &mut g.player.player[i];
        p.shield = u32::from(g.episodes.shields[s].mpwr);
        p.shield_max = p.shield * 2;
    }

    crate::varz::je_draw_shield(g);
    crate::varz::je_draw_armor(g);

    for p in g.player.player.iter_mut() {
        p.superbombs = 0;
    }

    // Set cubes to 0
    g.config.cube_max = 0;

    // Secret Level Display
    g.varz.flash = 0;
    g.varz.flash_change = 1;
    g.varz.display_time = 0;

    crate::loudness::play_song(g, (i32::from(g.varz.level_song) - 1) as u32)?;

    crate::mainint::je_draw_port_config_buttons(g);

    // --- MAIN LOOP ---

    if g.network.is_network_game {
        crate::network::je_clear_special_requests(g);
        crate::mtrand::mt_srand(g, 32402394);
    }

    crate::backgrnd::initialize_starfield(g);

    crate::config::je_set_new_game_speed(g);

    let (music, fx) = (g.nortsong.tyr_music_volume, g.nortsong.fx_volume);
    crate::loudness::set_volume(g, music as u8, fx as u8);

    // Save backup game
    if !g.demo.play_demo && !g.varz.do_not_save_backup {
        g.varz.temp = if g.config.two_player_mode { 22 } else { 11 };
        let slot = g.varz.temp;
        crate::config::je_save_game(g, slot, b"LAST LEVEL    ");
    }

    g.config.two_player_linked = false;
    g.config.link_gun_direc = std::f64::consts::PI as f32;

    for p in 0..g.player.player.len() {
        crate::player::calc_purple_balls_needed(g, p);
    }

    let v = &mut g.varz;
    v.damage_rate = 2; // Normal Rate for Collision Damage

    v.charge_wait = 5;
    v.charge_level = 0;
    v.charge_max = 5;
    v.charge_gr = 0;
    v.charge_gr_wait = 3;

    g.config.port_config_change = false;

    // Destruction Ratio
    v.total_enemy = 0;
    v.enemy_killed = 0;

    v.astral_duration = 0;

    g.config.super_arcade_power_up = 1;

    g.network.your_in_game_menu_request = false;

    g.mainint.constant_last_x = -1;

    for p in g.player.player.iter_mut() {
        p.exploding_ticks = 0;
    }

    if g.network.is_network_game {
        crate::episodes::je_load_item_dat(g)?;
    }

    let v = &mut g.varz;
    v.enemy_avail = [1; 100];
    v.enemy_shot_avail = [true; ENEMY_SHOT_MAX];

    // Initialize Shots
    g.shots.player_shot_data.fill(Default::default());
    g.shots.shot_avail = [0; crate::shots::MAX_PWEAPON];
    g.config.shot_multi_pos = [0; 11];
    g.config.shot_repeat = [1; 11];

    g.mainint.button = [false; 4];

    v.global_flags = [false; 10];

    v.explosions.fill(Default::default());
    v.rep_explosions.fill(Default::default());

    // --- Clear Sound Queue ---
    v.sound_queue = [0; 8];
    v.sound_queue[3] = crate::sndmast::V_GOOD_LUCK;

    g.sprite.enemy_sprite_sheet_ids = [0; 4];
    v.enemy.fill(Default::default());

    v.sf_current_code = [[0; 21]; 2];
    v.sf_executed = [0; 2];

    v.zinglon_duration = 0;
    v.special_wait = 0;
    v.next_special_wait = 0;
    v.option_attachment_move = 0; // Launch the Attachments!
    v.option_attachment_linked = true;

    v.edit_ship1 = false;
    v.edit_ship2 = false;

    g.config.smoothies = [false; 9];

    v.level_timer = false;
    v.random_explosions = false;

    v.last_superpixel = 0;
    v.superpixels.fill(Default::default());

    v.return_active = false;

    v.galaga_shot_freq = 0;

    if g.config.galaga_mode {
        g.config.difficulty_level = DIFFICULTY_NORMAL;
    }
    v.galaga_life = 10000;

    crate::varz::je_draw_option_level(g);

    // keeps map from scrolling past the top
    let v = &mut g.varz;
    (v.bkwrap1, v.bkwrap1to) = (Some(14), Some(14));
    (v.bkwrap2, v.bkwrap2to) = (Some(14), Some(14));
    (v.bkwrap3, v.bkwrap3to) = (Some(15), Some(15));

    Ok(true)
}

/// One frame of `level_loop`; returns where `JE_main` goes next.
fn main_level_loop(g: &mut Game, old_weapon_bar: &mut [u32; 2]) -> Flow<MainLabel> {
    if g.network.is_network_game {
        g.config.smoothies[9 - 1] = false;
        g.config.smoothies[6 - 1] = false;
    } else {
        let s = &g.config.smoothies;
        g.config.star_show_vga_special_code = u8::from(s[9 - 1]) + (u8::from(s[6 - 1]) << 1);
    }

    // Background Wrapping
    let (v, bk) = (&g.varz, &mut g.backgrnd);
    if bk.map_y_pos <= v.bkwrap1 {
        bk.map_y_pos = v.bkwrap1to;
    }
    if bk.map_y2_pos <= v.bkwrap2 {
        bk.map_y2_pos = v.bkwrap2to;
    }
    if bk.map_y3_pos <= v.bkwrap3 {
        bk.map_y3_pos = v.bkwrap3to;
    }

    let p = &g.player.player;
    let (one_action, two) = (g.config.one_player_action, g.config.two_player_mode);
    g.varz.all_players_gone = crate::player::all_players_dead(g)
        && ((p[0].lives() == 1 && p[0].exploding_ticks == 0) || (!one_action && !two))
        && ((p[1].lives() == 1 && p[1].exploding_ticks == 0) || !two);

    // -----MUSIC FADE------
    if g.musmast.music_fade {
        if g.nortsong.temp_volume > 10 {
            g.nortsong.temp_volume -= 1;
            let (music, fx) = (g.nortsong.temp_volume, g.nortsong.fx_volume);
            crate::loudness::set_volume(g, music as u8, fx as u8);
        } else {
            g.musmast.music_fade = false;
        }
    }

    let playing = crate::loudness::audio(g).lds.playing;
    if !g.varz.all_players_gone && g.varz.level_end > 0 && g.varz.end_level {
        crate::loudness::play_song(g, 9)?;
        g.musmast.music_fade = false;
    } else if !playing && g.varz.first_game_over {
        crate::loudness::play_song(g, (i32::from(g.varz.level_song) - 1) as u32)?;
    }

    if !g.varz.end_level {
        draw_hud(g, old_weapon_bar);
    }

    // use game_screen for all the generic drawing functions
    g.video.vga_screen = g.video.game_screen;

    // ---------------------------EVENTS-------------------------
    // D-UB-TYRIAN2-4: after `JE_eventJump` to the first event (from a
    // return, the level timer or enemy 254), `eventLoc` is 0 and upstream
    // reads `eventRec[-1]`; no event is due then.
    while g.varz.event_loc >= 1
        && g.tyrian2.event_rec[usize::from(g.varz.event_loc) - 1].eventtime <= g.varz.cur_loc
        && g.varz.event_loc <= g.varz.max_event
    {
        je_event_system(g)?;
    }

    if g.network.is_network_game && g.varz.really_end_level {
        return Ok(MainLabel::StartLevel);
    }

    // SMOOTHIES!
    crate::backgrnd::je_check_smoothies(g);
    if g.backgrnd.any_smoothies {
        // this makes things complicated, but we do it anyway :(
        g.video.vga_screen = g.video.vga_screen2;
    }

    draw_backgrounds_and_enemies(g)?;

    // Player Shot Images
    for z in 0..crate::shots::MAX_PWEAPON {
        if g.shots.shot_avail[z] != 0 {
            player_shot_hits(g, z)?;
        }
    }

    // Player movement indicators for shots that track your ship
    for p in g.player.player.iter_mut() {
        p.last_x_shot_move = p.x;
        p.last_y_shot_move = p.y;
    }

    // =======Collisions Detection======

    let players = if g.config.two_player_mode { 2 } else { 1 };
    for i in 0..players {
        if g.player.player[i].is_alive && !g.varz.end_level {
            crate::mainint::je_player_collide(g, i, i as u8 + 1)?;
        }
    }

    if g.varz.first_game_over {
        crate::mainint::je_main_game_player_functions(g)?; // PLAYER DRAW+MOVEMENT
    }

    if !g.varz.end_level {
        // MAIN DRAWING IS STOPPED STARTING HERE
        draw_enemy_shots(g);
    }

    let vga = g.video.vga_screen_id();
    if g.config.background3over == 1 {
        crate::backgrnd::draw_background_3(g, vga);
    }

    // Draw Top Enemy
    if g.config.top_enemy_over {
        let bk = &mut g.backgrnd;
        bk.temp_map_x_ofs = if !g.varz.background3x1 {
            bk.old_map_x3_ofs
        } else {
            bk.old_map_x_ofs
        };
        g.varz.temp_back_move = g.backgrnd.back_move3;
        je_draw_enemy(g, 75);
    }

    // Draw Sky Enemy
    if g.config.sky_enemy_over_all {
        let last_enemy_on_screen = g.varz.enemy_on_screen;

        g.backgrnd.temp_map_x_ofs = g.backgrnd.map_x2_ofs;
        g.varz.temp_back_move = 0;
        je_draw_enemy(g, 25);

        if g.varz.enemy_on_screen == last_enemy_on_screen && g.varz.stop_background_num == 2 {
            g.varz.stop_background_num = 9;
        }
    }

    draw_explosions(g);

    if !g.config.port_config_change {
        g.config.port_config_done = true;
    }

    // -----------------------BACKGROUND 2------------------------
    let c = &g.config;
    if !(c.smoothies[2 - 1] && c.processor_type < 4)
        && !(c.smoothies[0] && c.processor_type == 3)
        && c.background2over == 2
    {
        draw_background_2_maybe_blended(g);
    }

    low_armor_warning(g);

    // ------- Random Explosions --------
    if g.varz.random_explosions && mt_rand(g) % 10 == 1 {
        let x = (mt_rand(g) % 280) as i16;
        let y = (mt_rand(g) % 180) as i16;
        crate::varz::je_setup_explosion_large(g, false, 20, x, y);
    }

    // =======The Sound Routine=========
    if g.varz.first_game_over {
        play_sound_queue(g);
    }

    if g.varz.return_active && g.varz.enemy_on_screen == 0 {
        je_event_jump(g, 65535);
        g.varz.return_active = false;
    }

    // -------      DEbug      ---------
    g.varz.debug_time = g.platform.get_ticks();

    if g.varz.debug {
        draw_debug(g);
    }

    if g.varz.display_time > 0 {
        g.varz.display_time -= 1;
        let vga = g.video.vga_screen_id();
        let text = g.helptext.misc_text[59];
        let brightness = (i32::from(g.varz.flash) - 8) as u32;
        crate::fonthand::je_out_text_and_darken(
            g,
            vga,
            90,
            10,
            &text,
            15,
            brightness,
            FONT_SHAPES as u32,
        );
        let v = &mut g.varz;
        v.flash = v.flash.wrapping_add(v.flash_change as u8);
        if v.flash > 4 || v.flash == 0 {
            v.flash_change = v.flash_change.wrapping_neg();
        }
    }

    level_timer_tick(g);

    game_over(g)?;

    if g.demo.play_demo {
        // input stops demo
        crate::joystick::push_joysticks_as_keyboard(g);

        if crate::keyboard::has_input(g, INPUT_NO_MOTION) {
            g.varz.really_end_level = true;

            g.demo.stopped_demo = true;
        }
    } else {
        // input handling for pausing, menu, cheats

        // Ensure gameplay input does not affect pause or menu.
        crate::keyboard::mouse_clear_input(g);

        if crate::keyboard::keyboard_has_input(g) {
            // Pause, menu, and cheats are triggered on keysactive, so this is fine.
            crate::keyboard::keyboard_clear_input(g);

            g.varz.skip_star_show_vga = false;
            crate::mainint::je_main_keyboard_input(g)?;
            if g.varz.skip_star_show_vga {
                return Ok(MainLabel::LevelLoop);
            }
        }

        if g.mainint.pause_pressed || !g.keyboard.window_has_focus {
            g.mainint.pause_pressed = false;

            if g.network.is_network_game {
                g.network.pause_request = true;
            } else {
                crate::mainint::je_pause_game(g)?;
            }
        }

        if g.mainint.ingamemenu_pressed {
            g.mainint.ingamemenu_pressed = false;

            if g.network.is_network_game {
                g.network.in_game_menu_request = true;
            } else {
                g.network.your_in_game_menu_request = true;
                crate::mainint::je_do_in_game_setup(g)?;
                g.varz.skip_star_show_vga = true;
            }
        }
    }

    // Network Update
    if g.network.is_network_game {
        network_frame(g)?;
    }

    // Test
    crate::varz::je_draw_sp(g);

    // Filtration
    if g.config.filter_active {
        let (filter, brightness) = (g.config.level_filter, g.config.level_brightness);
        crate::backgrnd::je_filter_screen(g, filter, brightness);
    }

    draw_boss_bar(g);

    crate::mainint::je_in_game_displays(g);

    g.video.vga_screen = g.video.vga_screen_seg; // side-effect of game_screen

    if !g.varz.player_end_level && !g.varz.skip_star_show_vga {
        let seg = g.video.vga_screen_seg.expect("VGAScreenSeg");
        let game = g.video.game_screen.expect("game_screen");
        let code = g.config.star_show_vga_special_code;
        if code == 1 {
            crate::backgrnd::show_flipped(g, seg, game);
        } else if code == 2 && g.config.processor_type >= 2 {
            let (x, y) = (g.player.player[0].x as i16, g.player.player[0].y as i16);
            crate::backgrnd::show_headlight(g, seg, game, x, y);
        } else {
            crate::backgrnd::show_normal(g, seg, game);
        }

        // Scale the screen before the delay so that it can be presented
        // immediately after it.
        let vga = g.video.vga_screen_id();
        crate::video::scale_and_render_screen(g, vga);

        if g.config.smooth_scroll {
            crate::nortsong::delay_until_elapsed(g);

            let max = g.nortsong.frame_count_max;
            crate::nortsong::set_frame_count(g, max);
        }

        crate::video::present_screen(g);
    }

    g.varz.quit_requested = false;
    g.varz.skip_star_show_vga = false;

    end_of_frame_checks(g);

    // Other Network Functions
    crate::mainint::je_handle_chat(g);

    crate::keyboard::handle_sdl_events(g)?;

    Ok(if g.varz.really_end_level {
        MainLabel::StartLevel
    } else {
        MainLabel::LevelLoop
    })
}

/// The HUD part of `level_loop` (when the level has not ended): message bar,
/// shield generator, weapon power bars and the power bar.
fn draw_hud(g: &mut Game, old_weapon_bar: &mut [u32; 2]) {
    g.video.vga_screen = g.video.vga_screen_seg; // side-effect of game_screen
    let seg = g.video.vga_screen_seg.expect("VGAScreenSeg");

    // -----------------------Message Bar------------------------
    if g.mainint.text_erase > 0 {
        g.mainint.text_erase -= 1;
        if g.mainint.text_erase == 0 {
            // in-game message area
            crate::sprite::blit_sprite(g, seg, 16, 189, crate::sprite::OPTION_SHAPES as u32, 36);
        }
    }

    // ------------------------Shield Gen-------------------------
    if g.config.galaga_mode {
        for p in g.player.player.iter_mut() {
            p.shield = 0;
        }

        // spawned dragonwing died :(
        if g.player.player[1].lives() == 0 || g.player.player[1].armor == 0 {
            g.config.two_player_mode = false;
        }

        if g.player.player[0].cash >= u64::from(g.varz.galaga_life as u32) {
            g.varz.sound_queue[6] = crate::sndmast::S_EXPLOSION_11;
            g.varz.sound_queue[7] = crate::sndmast::S_SOUL_OF_ZINGLON;

            let lives = g.player.player[0].lives();
            if lives < 11 {
                g.player.player[0].set_lives(lives + 1);
            } else {
                g.player.player[0].cash = g.player.player[0].cash.wrapping_add(1000);
            }

            if g.varz.galaga_life == 10000 {
                g.varz.galaga_life = 20000;
            } else {
                g.varz.galaga_life = g.varz.galaga_life.wrapping_add(25000);
            }
        }
    } else if g.config.two_player_mode {
        g.config.shield_wait = g.config.shield_wait.wrapping_sub(1);
        if g.config.shield_wait == 0 {
            g.config.shield_wait = 15;

            for p in g.player.player.iter_mut() {
                if p.shield < p.shield_max && p.is_alive {
                    p.shield += 1;
                }
            }

            crate::varz::je_draw_shield(g);
        }
    } else if g.player.player[0].is_alive
        && g.player.player[0].shield < g.player.player[0].shield_max
        && g.config.power > u32::from(g.config.shield_t)
    {
        g.config.shield_wait = g.config.shield_wait.wrapping_sub(1);
        if g.config.shield_wait == 0 {
            g.config.shield_wait = 15;

            g.config.power -= u32::from(g.config.shield_t);

            let p = &mut g.player.player;
            p[0].shield += 1;
            if p[1].shield < p[0].shield_max {
                p[1].shield += 1;
            }

            crate::varz::je_draw_shield(g);
        }
    }

    // ---------------------Weapon Display-------------------------
    let two = g.config.two_player_mode;
    for (i, old) in old_weapon_bar.iter_mut().enumerate() {
        let item_power = u32::from(g.player.player[if two { i } else { 0 }].items.weapon[i].power);

        if *old != item_power {
            *old = item_power;

            let mut x = if two { 286 } else { 289 };
            let y = if i == 0 {
                if two { 6 } else { 17 }
            } else if two {
                100
            } else {
                38
            };

            crate::vga256d::fill_rectangle_xy(g, seg, x, y, x + 1 + 10 * 2, y + 2, 0);

            let vga = g.video.vga_screen_id();
            for j in 1..=item_power {
                crate::vga256d::je_rectangle(g, vga, x, y, x + 1, y + 2, 115 + j as i32); // SEGa000
                x += 2;
            }
        }
    }

    // ------------------------Power Bar-------------------------
    if g.config.two_player_mode || g.config.one_player_action {
        g.config.power = 900;
    } else {
        let c = &mut g.config;
        c.power = c.power.wrapping_add(c.power_add);
        if c.power > 900 {
            c.power = 900;
        }

        g.varz.temp = (c.power / 10) as u8;

        let (temp, last) = (u32::from(g.varz.temp), c.last_power);
        if temp != last {
            if temp > last {
                let color = (113 + temp / 7) as u8;
                let (y1, y2) = (113 - 11 - temp as i32, 114 - 11 - last as i32);
                crate::vga256d::fill_rectangle_xy(g, seg, 269, y1, 276, y2, color);
            } else {
                let (y1, y2) = (113 - 11 - last as i32, 114 - 11 - temp as i32);
                crate::vga256d::fill_rectangle_xy(g, seg, 269, y1, 276, y2, 0);
            }

            g.config.last_power = temp;
        }
    }

    g.backgrnd.old_map_x3_ofs = g.backgrnd.map_x3_ofs;

    g.varz.enemy_on_screen = 0;
}

/// `draw_background_2_blend` when wild and not opaque, else
/// `draw_background_2`, on `VGAScreen`.
fn draw_background_2_maybe_blended(g: &mut Game) {
    let vga = g.video.vga_screen_id();
    if g.config.wild && !g.config.background2not_transparent {
        crate::backgrnd::draw_background_2_blend(g, vga);
    } else {
        crate::backgrnd::draw_background_2(g, vga);
    }
}

/// `filter(game_screen, VGAScreen); VGAScreen = game_screen;`
fn apply_filter(g: &mut Game, filter: fn(&mut Game, ScreenId, ScreenId)) {
    let game = g.video.game_screen.expect("game_screen");
    let vga = g.video.vga_screen_id();
    filter(g, game, vga);
    g.video.vga_screen = Some(game);
}

/// The backgrounds, filters, ground/sky/top enemies and a random new enemy
/// of `level_loop`, in upstream's drawing order.
fn draw_backgrounds_and_enemies(g: &mut Game) -> Flow {
    // --- BACKGROUND 1 ---

    if g.varz.force_events && g.backgrnd.back_move == 0 {
        g.varz.cur_loc = g.varz.cur_loc.wrapping_add(1);
    }

    let bk = &mut g.backgrnd;
    if bk.map1_y_delay_max > 1 && bk.back_move < 2 {
        bk.back_move = u16::from(bk.map1_y_delay == 1);
    }

    // Draw background
    let vga = g.video.vga_screen_id();
    if g.varz.astral_duration == 0 {
        crate::backgrnd::draw_background_1(g, vga);
    } else {
        crate::video::je_clr256(g, vga);
    }

    // Set Movement of background 1
    let bk = &mut g.backgrnd;
    bk.map1_y_delay = bk.map1_y_delay.wrapping_sub(1);
    if bk.map1_y_delay == 0 {
        bk.map1_y_delay = bk.map1_y_delay_max;

        g.varz.cur_loc = g.varz.cur_loc.wrapping_add(bk.back_move);

        bk.back_pos = bk.back_pos.wrapping_add(bk.back_move);

        if bk.back_pos > 27 {
            bk.back_pos -= 28;
            bk.map_y = bk.map_y.wrapping_sub(1);
            bk.map_y_pos = bk.map_y_pos.map(|p| p - 14); // Map Width
        }
    }

    if g.config.star_active || g.varz.astral_duration > 0 {
        let speed = g.backgrnd.starfield_speed;
        crate::backgrnd::update_and_draw_starfield(g, vga, speed);
    }

    let c = &g.config;
    if c.processor_type > 1 && c.smoothies[5 - 1] {
        apply_filter(g, crate::backgrnd::iced_blur_filter);
    }

    // -----------------------BACKGROUND 2------------------------
    if g.config.background2over == 3 {
        let vga = g.video.vga_screen_id();
        crate::backgrnd::draw_background_2(g, vga);
        g.config.background2 = true;
    }

    let c = &g.config;
    if c.background2over == 0
        && !(c.smoothies[2 - 1] && c.processor_type < 4)
        && !(c.smoothies[0] && c.processor_type == 3)
    {
        draw_background_2_maybe_blended(g);
    }

    let c = &g.config;
    if c.smoothies[0] && c.processor_type > 2 && g.backgrnd.smoothie_data[0] == 0 {
        apply_filter(g, crate::backgrnd::lava_filter);
    }
    let c = &g.config;
    if c.smoothies[2 - 1] && c.processor_type > 2 {
        apply_filter(g, crate::backgrnd::water_filter);
    }

    // -----------------------Ground Enemy------------------------
    let last_enemy_on_screen = g.varz.enemy_on_screen;

    g.backgrnd.temp_map_x_ofs = g.backgrnd.map_x_ofs;
    g.varz.temp_back_move = g.backgrnd.back_move;
    je_draw_enemy(g, 50);
    je_draw_enemy(g, 100);

    let v = &mut g.varz;
    if (v.enemy_on_screen == 0 || v.enemy_on_screen == last_enemy_on_screen)
        && v.stop_background_num == 1
    {
        v.stop_background_num = 9;
    }

    let c = &g.config;
    if c.smoothies[0] && c.processor_type > 2 && g.backgrnd.smoothie_data[0] > 0 {
        apply_filter(g, crate::backgrnd::lava_filter);
    }

    if g.config.super_wild {
        g.varz.neat = g.varz.neat.wrapping_add(3);
        let neat = g.varz.neat;
        crate::backgrnd::je_darken_background(g, neat);
    }

    // -----------------------BACKGROUND 2------------------------
    let c = &g.config;
    if !(c.smoothies[2 - 1] && c.processor_type < 4)
        && !(c.smoothies[0] && c.processor_type == 3)
        && c.background2over == 1
    {
        draw_background_2_maybe_blended(g);
    }

    if g.config.super_wild {
        g.varz.neat = g.varz.neat.wrapping_add(1);
        let neat = g.varz.neat;
        crate::backgrnd::je_darken_background(g, neat);
    }

    if g.config.background3over == 2 {
        let vga = g.video.vga_screen_id();
        crate::backgrnd::draw_background_3(g, vga);
    }

    // New Enemy
    if g.varz.enemies_active && mt_rand(g) % 100 > u64::from(g.tyrian2.level_enemy_frequency) {
        let i = (mt_rand(g) % u64::from(g.tyrian2.level_enemy_max)) as usize;
        g.varz.temp_w = g.tyrian2.level_enemy[i];
        if g.varz.temp_w == 2 {
            g.varz.sound_queue[3] = crate::sndmast::S_WEAPON_7;
        }
        let e = g.varz.temp_w;
        g.varz.b = je_new_enemy(g, 0, e, 0);
    }

    let c = &g.config;
    if c.processor_type > 1 && c.smoothies[3 - 1] {
        apply_filter(g, crate::backgrnd::iced_blur_filter);
    }
    let c = &g.config;
    if c.processor_type > 1 && c.smoothies[4 - 1] {
        apply_filter(g, crate::backgrnd::blur_filter);
    }

    // Draw Sky Enemy
    if !g.config.sky_enemy_over_all {
        let last_enemy_on_screen = g.varz.enemy_on_screen;

        g.backgrnd.temp_map_x_ofs = g.backgrnd.map_x2_ofs;
        g.varz.temp_back_move = 0;
        je_draw_enemy(g, 25);

        if g.varz.enemy_on_screen == last_enemy_on_screen && g.varz.stop_background_num == 2 {
            g.varz.stop_background_num = 9;
        }
    }

    if g.config.background3over == 0 {
        let vga = g.video.vga_screen_id();
        crate::backgrnd::draw_background_3(g, vga);
    }

    // Draw Top Enemy
    if !g.config.top_enemy_over {
        let bk = &mut g.backgrnd;
        bk.temp_map_x_ofs = if !g.varz.background3x1 {
            bk.old_map_x3_ofs
        } else {
            bk.map_x_ofs
        };
        g.varz.temp_back_move = g.backgrnd.back_move3;
        je_draw_enemy(g, 75);
    }
    Ok(())
}

/// The player shot `z` of `level_loop`: moves and draws it, then checks it
/// against every live enemy (`goto draw_player_shot_loop_end` is a return).
fn player_shot_hits(g: &mut Game, z: usize) -> Flow {
    const ZINGLON: usize = crate::shots::MAX_PWEAPON - 1;
    let mut out = crate::shots::PlayerShotOut {
        blast_filter: g.varz.temp2,
        ..Default::default()
    };
    let moved = crate::shots::player_shot_move_and_draw(g, z as i32, &mut out);
    g.varz.temp2 = out.blast_filter;
    if !moved {
        return Ok(());
    }
    let (shot_x, shot_y) = (out.shotx, out.shoty);
    let (radius_w, radius_h) = (
        i32::from(out.special_radiusw),
        i32::from(out.special_radiush),
    );
    let mut chain = out.chain;
    let mut damage = out.shot_damage;
    let player_num = out.player_num;

    g.varz.b = 0;
    while g.varz.b < 100 {
        let bi = g.varz.b as usize;
        if g.varz.enemy_avail[bi] == 0 {
            let e = &g.varz.enemy[bi];
            let ex = i32::from(e.ex) + i32::from(e.mapoffset);
            let ey = i32::from(e.ey);
            let collided = if z == ZINGLON {
                g.varz.temp = (25 - (i32::from(g.varz.zinglon_duration) - 25).abs()) as u8;
                let hit = (ex - (g.player.player[0].x + 7)).abs() < i32::from(g.varz.temp);
                g.varz.temp2 = 9;
                chain = 0;
                damage = 10;
                hit
            } else if out.is_special {
                (e.enemycycle == 0
                    && (ex - shot_x - radius_w).abs() < 25 + radius_w
                    && (ey - shot_y - 12 - radius_h).abs() < 29 + radius_h)
                    || (e.enemycycle > 0
                        && (ex - shot_x - radius_w).abs() < 13 + radius_w
                        && (ey - shot_y - 6 - radius_h).abs() < 15 + radius_h)
            } else {
                (e.enemycycle == 0 && (ex - shot_x).abs() < 25 && (ey - shot_y - 12).abs() < 29)
                    || (e.enemycycle > 0
                        && (ex - shot_x).abs() < 13
                        && (ey - shot_y - 6).abs() < 15)
            };

            if collided {
                if chain > 0 {
                    g.config.shot_multi_pos[crate::config::SHOT_MISC] = 0;
                    let (mx, my) = (g.player.player[0].mouse_x, g.player.player[0].mouse_y);
                    g.varz.b = crate::shots::player_shot_create(
                        g,
                        0,
                        crate::config::SHOT_MISC as u32,
                        shot_x as u16,
                        shot_y as u16,
                        mx,
                        my,
                        u16::from(chain),
                        player_num,
                    );
                    g.shots.shot_avail[z] = 0;
                    return Ok(());
                }

                g.varz.infinite_shot = false;

                if damage == 99 {
                    damage = 0;
                    g.varz.do_iced = 40;
                    g.varz.enemy[bi].iced = 40;
                } else {
                    g.varz.do_iced = 0;
                    if damage >= 250 {
                        damage -= 250;
                        g.varz.infinite_shot = true;
                    }
                }

                let armorleft = i32::from(g.varz.enemy[bi].armorleft);

                g.varz.temp = g.varz.enemy[bi].linknum;
                if g.varz.temp == 0 {
                    g.varz.temp = 255;
                }
                let temp = g.varz.temp;

                if g.varz.enemy[bi].armorleft < 255 {
                    for bar in g.tyrian2.boss_bar.iter_mut() {
                        if temp == bar.link_num {
                            bar.color = 6;
                        }
                    }

                    let v = &mut g.varz;
                    if v.enemy[bi].enemyground {
                        v.enemy[bi].filter = v.temp2;
                    }

                    for e in 0..v.enemy.len() {
                        if v.enemy[e].linknum == temp
                            && v.enemy_avail[e] != 1
                            && v.enemy[e].enemyground
                        {
                            if v.do_iced != 0 {
                                v.enemy[e].iced = v.do_iced;
                            }
                            v.enemy[e].filter = v.temp2;
                        }
                    }
                }

                if armorleft > i32::from(damage) {
                    if z != ZINGLON {
                        if g.varz.enemy[bi].armorleft != 255 {
                            let a = &mut g.varz.enemy[bi].armorleft;
                            *a = a.wrapping_sub(damage as u8);
                            crate::varz::je_setup_explosion(
                                g,
                                shot_x as i16,
                                shot_y as i16,
                                0,
                                0,
                                false,
                                false,
                            );
                        } else {
                            let d = i32::from(damage);
                            crate::varz::je_do_sp(
                                g,
                                (shot_x + 6) as u16,
                                (shot_y + 6) as u16,
                                (d / 2 + 3) as u16,
                                (d / 4 + 2) as u8,
                                g.varz.temp2,
                            );
                        }
                    }

                    g.varz.sound_queue[5] = crate::sndmast::S_ENEMY_HIT;

                    let e = &g.varz.enemy[bi];
                    if armorleft - i32::from(damage) <= i32::from(e.edlevel)
                        && ((!e.edamaged) ^ (e.edani < 0))
                    {
                        damage_linked_enemies(g, bi, temp);
                    }
                } else {
                    if temp == 254 && g.varz.super_enemy254_jump > 0 {
                        let jump = g.varz.super_enemy254_jump;
                        je_event_jump(g, jump);
                    }

                    kill_linked_enemies(g, bi, temp, player_num);
                }

                if g.varz.infinite_shot {
                    damage += 250;
                } else if z != ZINGLON {
                    if i32::from(damage) <= armorleft {
                        g.shots.shot_avail[z] = 0;
                        return Ok(());
                    }
                    let dmg = &mut g.shots.player_shot_data[z].shot_dmg;
                    *dmg = dmg.wrapping_sub(armorleft as u8);
                }
            }
        }
        g.varz.b += 1;
    }
    Ok(())
}

/// A shot damaged enemy `b` (link `temp`) below its damage level: it and the
/// enemies linked to it switch to their damaged look, or die.
fn damage_linked_enemies(g: &mut Game, b: usize, temp: u8) {
    g.varz.temp3 = 0;
    while g.varz.temp3 < 100 {
        let t3 = usize::from(g.varz.temp3);
        if g.varz.enemy_avail[t3] != 1 {
            let linknum = i32::from(g.varz.enemy[t3].linknum);
            let temp = i32::from(temp);
            let v = &g.varz;
            if t3 == b
                || (temp != 255
                    && ((v.enemy[t3].edlevel > 0 && linknum == temp)
                        || (v.enemy_continual_damage && temp - 100 == linknum)
                        || (linknum > 40 && linknum / 20 == temp / 20 && linknum <= temp)))
            {
                let v = &mut g.varz;
                let e = &mut v.enemy[t3];
                e.enemycycle = 1;

                e.edamaged = !e.edamaged;

                if e.edani != 0 {
                    e.ani = i32::from(e.edani).unsigned_abs() as u8;
                    e.aniactive = 1;
                    e.animax = 0;
                    e.animin = e.edgr as u8;
                    e.enemycycle = e.animin.wrapping_sub(1);
                } else if e.edgr > 0 {
                    e.egr[0] = e.edgr;
                    e.ani = 1;
                    e.aniactive = 0;
                    e.animax = 0;
                    e.animin = 1;
                } else {
                    v.enemy_avail[t3] = 1;
                    v.enemy_killed = v.enemy_killed.wrapping_add(1);
                }

                let e = &mut v.enemy[t3];
                e.aniwhenfire = 0;

                if e.armorleft > e.edlevel as u8 {
                    e.armorleft = e.edlevel as u8;
                }

                let x = (i32::from(e.ex) + i32::from(e.mapoffset)) as i16;
                let y = e.ey;
                let (ground, explonum) = (e.enemyground, e.explonum);

                if g.episodes.enemy_dat[usize::from(e.enemytype)].esize != 1 {
                    crate::varz::je_setup_explosion(g, x, y.wrapping_sub(6), 0, 1, false, false);
                } else {
                    crate::varz::je_setup_explosion_large(g, ground, explonum / 2, x, y);
                }
            }
        }
        g.varz.temp3 += 1;
    }
}

/// A shot destroyed enemy `b` (link `temp`): it and the enemies linked to it
/// die (or turn into their remains), dropping items, scoring and exploding.
fn kill_linked_enemies(g: &mut Game, b: usize, temp: u8, player_num: u8) {
    let temp = i32::from(temp);
    g.varz.temp2 = 0;
    while g.varz.temp2 < 100 {
        let t2 = usize::from(g.varz.temp2);
        if g.varz.enemy_avail[t2] != 1 {
            g.varz.temp3 = g.varz.enemy[t2].linknum;
            let temp3 = i32::from(g.varz.temp3);
            if t2 == b
                || temp == 254
                || (temp != 255
                    && (temp == temp3
                        || temp - 100 == temp3
                        || (temp3 > 40 && temp3 / 20 == temp / 20 && temp3 <= temp)))
            {
                let e = &g.varz.enemy[t2];
                let enemy_screen_x = i32::from(e.ex) + i32::from(e.mapoffset);

                if e.special {
                    let flag = usize::from(e.flagnum).wrapping_sub(1);
                    debug_assert!(flag < g.varz.global_flags.len());
                    let setto = e.setto;
                    // D-UB-TYRIAN2-4: a flag number outside 1..=10 sets nothing.
                    if let Some(f) = g.varz.global_flags.get_mut(flag) {
                        *f = setto;
                    }
                }

                let enemydie = g.varz.enemy[t2].enemydie;
                if enemydie > 0
                    && !(g.config.super_arcade_mode != SA_NONE
                        && g.episodes.enemy_dat[usize::from(enemydie)].value == 30000)
                {
                    let temp_b = g.varz.b;
                    g.varz.temp_w = enemydie;
                    let mut enemy_offset = (t2 - (t2 % 25)) as i32;
                    if g.episodes.enemy_dat[usize::from(g.varz.temp_w)].value > 30000 {
                        enemy_offset = 0;
                    }
                    let w = g.varz.temp_w;
                    g.varz.b = je_new_enemy(g, enemy_offset, w, 0);
                    if g.varz.b != 0 {
                        let nb = (g.varz.b - 1) as usize;
                        if g.config.super_arcade_mode != SA_NONE && g.varz.enemy[nb].evalue > 30000
                        {
                            let c = &mut g.config;
                            c.super_arcade_power_up += 1;
                            if c.super_arcade_power_up > 5 {
                                c.super_arcade_power_up = 1;
                            }
                            let up = c.super_arcade_power_up;
                            g.varz.enemy[nb].egr[0] = 5 + u16::from(up) * 2;
                            g.varz.enemy[nb].evalue = 30000 + i16::from(up);
                        }

                        let v = &mut g.varz;
                        v.enemy[nb].scoreitem = v.enemy[nb].evalue != 0;

                        v.enemy[nb].ex = v.enemy[t2].ex;
                        v.enemy[nb].ey = v.enemy[t2].ey;
                    }
                    g.varz.b = temp_b;
                }

                let evalue = g.varz.enemy[t2].evalue;
                if evalue > 0 && evalue < 10000 {
                    if evalue == 1 {
                        g.config.cube_max = g.config.cube_max.wrapping_add(1);
                    } else {
                        // in galaga mode player 2 is sidekick, so give cash
                        // to player 1
                        let who = if g.config.galaga_mode {
                            Some(0)
                        } else {
                            usize::from(player_num).checked_sub(1)
                        };
                        // D-UB-TYRIAN2-4: a kill with no shooting player
                        // (the Zinglon beam) scores for nobody.
                        if let Some(p) = who.and_then(|p| g.player.player.get_mut(p)) {
                            p.cash = p.cash.wrapping_add(evalue as u64);
                        }
                    }
                }

                let v = &mut g.varz;
                if v.enemy[t2].edlevel == -1 && temp == temp3 {
                    let e = &mut v.enemy[t2];
                    e.edlevel = 0;
                    v.enemy_avail[t2] = 2;
                    let e = &mut v.enemy[t2];
                    e.egr[0] = e.edgr;
                    e.ani = 1;
                    e.aniactive = 0;
                    e.animax = 0;
                    e.animin = 1;
                    e.edamaged = true;
                    e.enemycycle = 1;
                } else {
                    v.enemy_avail[t2] = 1;
                    v.enemy_killed = v.enemy_killed.wrapping_add(1);
                }

                let e = &g.varz.enemy[t2];
                let (ground, explonum, ey) = (e.enemyground, e.explonum, e.ey);
                if g.episodes.enemy_dat[usize::from(e.enemytype)].esize == 1 {
                    crate::varz::je_setup_explosion_large(
                        g,
                        ground,
                        explonum,
                        enemy_screen_x as i16,
                        ey,
                    );
                    g.varz.sound_queue[6] = crate::sndmast::S_EXPLOSION_9;
                } else {
                    crate::varz::je_setup_explosion(
                        g,
                        enemy_screen_x as i16,
                        ey,
                        0,
                        1,
                        false,
                        false,
                    );
                    g.varz.sound_queue[6] = crate::sndmast::S_EXPLOSION_8;
                }
            }
        }
        g.varz.temp2 += 1;
    }
}

/// Moves, collides and draws the enemy shots.
fn draw_enemy_shots(g: &mut Game) {
    for z in 0..ENEMY_SHOT_MAX {
        if g.varz.enemy_shot_avail[z] {
            continue;
        }
        let (p0x, p0y) = (g.player.player[0].x, g.player.player[0].y);
        let s = &mut g.varz.enemy_shot[z];
        s.sxm = s.sxm.wrapping_add(i16::from(s.sxc));
        s.sx = s.sx.wrapping_add(s.sxm);

        if s.tx != 0 {
            if i32::from(s.sx) > p0x {
                if i32::from(s.sxm) > -i32::from(s.tx) {
                    s.sxm -= 1;
                }
            } else if i32::from(s.sxm) < i32::from(s.tx) {
                s.sxm += 1;
            }
        }

        s.sym = s.sym.wrapping_add(i16::from(s.syc));
        s.sy = s.sy.wrapping_add(s.sym);

        if s.ty != 0 {
            if i32::from(s.sy) > p0y {
                if i32::from(s.sym) > -i32::from(s.ty) {
                    s.sym -= 1;
                }
            } else if i32::from(s.sym) < i32::from(s.ty) {
                s.sym += 1;
            }
        }

        let expired = s.duration == 0;
        s.duration = s.duration.wrapping_sub(1);
        if expired || s.sy > 190 || s.sy <= -14 || s.sx > 275 || s.sx <= 0 {
            g.varz.enemy_shot_avail[z] = true;
            continue;
        }

        // check if shot collided with player
        let players = if g.config.two_player_mode { 2 } else { 1 };
        for i in 0..players {
            let p = &g.player.player[i];
            let s = &g.varz.enemy_shot[z];
            let (sx, sy) = (i32::from(s.sx), i32::from(s.sy));
            let (ax, ay) = (p.shot_hit_area_x as i32, p.shot_hit_area_y as i32);
            if p.is_alive && sx > p.x - ax && sx < p.x + ax && sy > p.y - ay && sy < p.y + ay {
                let (x, y) = (s.sx, s.sy);
                g.varz.temp = s.sdmg;

                g.varz.enemy_shot_avail[z] = true;

                crate::varz::je_setup_explosion(g, x, y, 0, 0, false, false);

                if g.player.player[i].invulnerable_ticks == 0 {
                    let temp = g.varz.temp;
                    g.varz.temp = crate::varz::je_player_damage(g, temp, i);
                    if g.varz.temp > 0 {
                        let s = &g.varz.enemy_shot[z];
                        let t = i32::from(g.varz.temp);
                        let (dx, dy) = ((i32::from(s.sxm) * t) / 2, (i32::from(s.sym) * t) / 2);
                        let p = &mut g.player.player[i];
                        p.x_velocity = p.x_velocity.wrapping_add(dx);
                        p.y_velocity = p.y_velocity.wrapping_add(dy);
                    }
                }

                break;
            }
        }

        if !g.varz.enemy_shot_avail[z] {
            let s = &mut g.varz.enemy_shot[z];
            if s.animax != 0 {
                s.animate = s.animate.wrapping_add(1);
                if s.animate >= s.animax {
                    s.animate = 0;
                }
            }

            let (x, y) = (i32::from(s.sx), i32::from(s.sy));
            let frame = u32::from(s.sgr) + u32::from(s.animate);
            let vga = g.video.vga_screen_id();
            if s.sgr >= 500 {
                crate::sprite::blit_sprite2(g, vga, x, y, Sprite2Ref::SpriteSheet12, frame - 500);
            } else {
                crate::sprite::blit_sprite2(g, vga, x, y, Sprite2Ref::SpriteSheet8, frame);
            }
        }
    }
}

/// Sequenced (repeating) explosions, then every explosion moved and drawn.
fn draw_explosions(g: &mut Game) {
    g.varz.enemy_still_exploding = false;
    for i in 0..crate::varz::MAX_REPEATING_EXPLOSIONS {
        if g.varz.rep_explosions[i].ttl == 0 {
            continue;
        }
        g.varz.enemy_still_exploding = true;

        let back_move2 = u32::from(g.backgrnd.back_move2);
        let r = &mut g.varz.rep_explosions[i];
        if r.delay > 0 {
            r.delay -= 1;
            continue;
        }

        r.y = r.y.wrapping_add(back_move2 + 1);
        let (rx, ry) = (r.x, r.y);
        let x = (u64::from(rx).wrapping_add(mt_rand(g) % 24).wrapping_sub(12)) as i16;
        let y = (u64::from(ry).wrapping_add(mt_rand(g) % 27).wrapping_sub(24)) as i16;

        if g.varz.rep_explosions[i].big {
            crate::varz::je_setup_explosion_large(g, false, 2, x, y);

            if g.varz.rep_explosions[i].ttl == 1 || mt_rand(g) % 5 == 1 {
                g.varz.sound_queue[7] = crate::sndmast::S_EXPLOSION_11;
            } else {
                g.varz.sound_queue[6] = crate::sndmast::S_EXPLOSION_9;
            }

            g.varz.rep_explosions[i].delay = 4 + (mt_rand(g) % 3) as u32;
        } else {
            crate::varz::je_setup_explosion(g, x, y, 0, 1, false, false);

            g.varz.sound_queue[5] = crate::sndmast::S_EXPLOSION_4;

            g.varz.rep_explosions[i].delay = 3;
        }

        g.varz.rep_explosions[i].ttl -= 1;
    }

    // ---------------------------- Draw Explosions ----------------------------
    for j in 0..crate::varz::MAX_EXPLOSIONS {
        if g.varz.explosions[j].ttl == 0 {
            continue;
        }
        let v = &mut g.varz;
        let (explode_move, fx, fy) = (
            v.explode_move as i16,
            v.explosion_follow_amount_x,
            v.explosion_follow_amount_y,
        );
        let e = &mut v.explosions[j];
        if !e.fixed_position {
            e.sprite = e.sprite.wrapping_add(1);
            e.y = e.y.wrapping_add(explode_move);
        } else if e.follow_player {
            e.x = e.x.wrapping_add(fx);
            e.y = e.y.wrapping_add(fy);
        }
        e.y = e.y.wrapping_add(e.delta_y);

        if e.y > 200 - 14 {
            e.ttl = 0;
        } else {
            let (x, y, sprite) = (i32::from(e.x), i32::from(e.y), u32::from(e.sprite) + 1);
            let vga = g.video.vga_screen_id();
            if g.config.explosion_transparent {
                crate::sprite::blit_sprite2_blend(
                    g,
                    vga,
                    x,
                    y,
                    Sprite2Ref::ExplosionSpriteSheet,
                    sprite,
                );
            } else {
                crate::sprite::blit_sprite2(g, vga, x, y, Sprite2Ref::ExplosionSpriteSheet, sprite);
            }

            g.varz.explosions[j].ttl -= 1;
        }
    }
}

/// The low-armor warning: repair ship, siren and flashing bars.
fn low_armor_warning(g: &mut Game) {
    let p = &g.player.player;
    let two = g.config.two_player_mode;
    if !((p[0].is_alive && p[0].armor < 6)
        || (two && !g.config.galaga_mode && p[1].is_alive && p[1].armor < 6))
    {
        return;
    }
    let armor_amount = if p[0].is_alive && p[0].armor < 6 {
        p[0].armor
    } else {
        p[1].armor
    } as i32;

    if g.fonthand.armor_ship_delay > 0 {
        g.fonthand.armor_ship_delay -= 1;
    } else {
        g.varz.temp_w = 560;
        g.varz.b = je_new_enemy(g, 50, 560, 0);
        if g.varz.b > 0 {
            let r = (mt_rand(g) % 3) as u16;
            let bm3 = g.backgrnd.back_move3;
            let e = &mut g.varz.enemy[(g.varz.b - 1) as usize];
            e.enemydie = 560 + r + 1;
            e.eyc = (i32::from(e.eyc) - i32::from(bm3)) as i8;
            e.armorleft = 4;
        }
        g.fonthand.armor_ship_delay = 500;
    }

    let p = &g.player.player;
    let (net, me) = (g.network.is_network_game, g.network.this_player_num);
    if (p[0].is_alive && p[0].armor < 6 && (!net || me == 1))
        || (two && p[1].is_alive && p[1].armor < 6 && (!net || me == 2))
    {
        g.varz.temp_w = (armor_amount * 4 + 8) as u16;
        let f = &mut g.fonthand;
        if u16::from(f.warning_sound_delay) > g.varz.temp_w {
            f.warning_sound_delay = g.varz.temp_w as u8;
        }

        if f.warning_sound_delay > 1 {
            f.warning_sound_delay -= 1;
        } else {
            g.varz.sound_queue[7] = crate::sndmast::S_WARNING;
            f.warning_sound_delay = g.varz.temp_w as u8;
        }

        f.warning_col = f.warning_col.wrapping_add(f.warning_col_change as u8);
        let top = 113 + (14 - armor_amount * 2);
        if i32::from(f.warning_col) > top {
            f.warning_col_change = f.warning_col_change.wrapping_neg();
            f.warning_col = top as u8;
        } else if f.warning_col < 113 {
            f.warning_col_change = f.warning_col_change.wrapping_neg();
        }
        let col = f.warning_col;
        let vga = g.video.vga_screen_id();
        crate::vga256d::fill_rectangle_xy(g, vga, 24, 181, 138, 183, col);
        crate::vga256d::fill_rectangle_xy(g, vga, 175, 181, 287, 183, col);
        crate::vga256d::fill_rectangle_xy(g, vga, 24, 0, 287, 3, col);

        crate::fonthand::je_out_text(g, vga, 140, 178, b"WARNING\0", 7, i32::from((col % 16) / 2));
    }
}

/// Plays the queued sounds, one per channel.
fn play_sound_queue(g: &mut Game) {
    g.varz.temp = 0;
    g.varz.temp2 = 0;
    while usize::from(g.varz.temp2) < g.varz.sound_queue.len() {
        let ch = g.varz.temp2;
        let sound = g.varz.sound_queue[usize::from(ch)];
        if sound != crate::sndmast::S_NONE {
            g.varz.temp = sound;
            let vol = crate::nortsong::FX_PLAY_VOL as u8;
            g.varz.temp3 = if ch == 3 {
                vol
            } else if sound == 15 {
                vol / 4
            } else {
                // Lightning
                vol / 2
            };

            let i = usize::from(sound) - 1;
            let samples =
                g.nortsong.sound_samples[i]
                    .clone()
                    .map(|buf| crate::loudness::ChannelSamples {
                        sound: i,
                        buf,
                        pos: 0,
                    });
            let count = g.nortsong.sound_sample_count[i];
            let vol = g.varz.temp3;
            crate::loudness::multi_sample_play(g, samples, count, ch, vol);

            g.varz.sound_queue[usize::from(ch)] = crate::sndmast::S_NONE;
        }
        g.varz.temp2 += 1;
    }
}

/// The debug overlay: smoothies, enemies on screen, frame rate, position.
fn draw_debug(g: &mut Game) {
    let vga = g.video.vga_screen_id();
    for i in 0..9 {
        g.tyrian2.temp_str[i] = b'0' + u8::from(g.config.smoothies[i]);
    }
    g.tyrian2.temp_str[9] = 0;
    let sm = String::from_utf8_lossy(c_str(&g.tyrian2.temp_str)).into_owned();
    crate::fonthand::je_out_text(g, vga, 30, 70, format!("SM = {sm}\0").as_bytes(), 4, 0);

    crate::fonthand::je_out_text(g, vga, 30, 80, b"Memory left = -1\0", 4, 0);
    let text = format!("Enemies onscreen = {}\0", g.varz.enemy_on_screen);
    crate::fonthand::je_out_text(g, vga, 30, 90, text.as_bytes(), 6, 0);

    let v = &mut g.varz;
    v.debug_hist = v
        .debug_hist
        .wrapping_add(v.debug_time.wrapping_sub(v.last_debug_time));
    v.debug_hist_count = v.debug_hist_count.wrapping_add(1);
    let fps = v.debug_hist_count as f32 / (v.debug_hist as f32 * 1e-3f32);
    crate::episodes::snprintf_into(&mut g.tyrian2.temp_str, &format!("{fps:2.3}"));
    let fps = String::from_utf8_lossy(c_str(&g.tyrian2.temp_str)).into_owned();
    let p = &g.player.player[0];
    let text = format!(
        "X:{} Y:{:<5}  {} FPS  {} {} {} {}\0",
        (i32::from(g.backgrnd.map_x) - 1) * 12 + p.x,
        g.varz.cur_loc,
        fps,
        p.x_velocity,
        p.y_velocity,
        p.x,
        p.y
    );
    crate::fonthand::je_out_text(g, vga, 45, 175, text.as_bytes(), 15, 3);
    g.varz.last_debug_time = g.varz.debug_time;
}

/// The level timer: counts down, jumps when it runs out, ticks and shows.
fn level_timer_tick(g: &mut Game) {
    if !(g.varz.level_timer && g.varz.level_timer_countdown > 0) {
        return;
    }
    g.varz.level_timer_countdown -= 1;
    if g.varz.level_timer_countdown == 0 {
        let to = g.varz.level_timer_jump_to;
        je_event_jump(g, to);
    }

    let count = g.varz.level_timer_countdown;
    if count > 200 {
        if count.is_multiple_of(100) {
            g.varz.sound_queue[7] = crate::sndmast::S_WARNING;
        }

        if count.is_multiple_of(10) {
            g.varz.sound_queue[6] = crate::sndmast::S_CLICK;
        }
    } else if count.is_multiple_of(20) {
        g.varz.sound_queue[7] = crate::sndmast::S_WARNING;
    }

    let vga = g.video.vga_screen_id();
    let text = g.helptext.misc_text[66];
    let shade = crate::fonthand::FULL_SHADE;
    crate::fonthand::je_text_shade(g, vga, 140, 6, &text, 7, i32::from((count % 20) / 3), shade);
    let buffer = format!("{:.1}", f32::from(count) / 100.0f32);
    crate::fonthand::je_d_string(
        g,
        vga,
        100,
        2,
        buffer.as_bytes(),
        crate::sprite::SMALL_FONT_SHAPES as u32,
    );
}

/// GAME OVER: when every player is gone, the game-over text and music, and
/// input ends the level.
fn game_over(g: &mut Game) -> Flow {
    if g.params.constant_play || g.params.constant_die || !g.varz.all_players_gone {
        return Ok(());
    }
    if g.player.player[0].exploding_ticks > 0 || g.player.player[1].exploding_ticks > 0 {
        if g.config.galaga_mode {
            g.player.player[1].exploding_ticks = 0;
        }

        g.musmast.music_fade = true;
        return Ok(());
    }

    if g.demo.play_demo || g.varz.normal_bonus_level_current || g.varz.bonus_level_current {
        g.varz.really_end_level = true;
    } else {
        let vga = g.video.vga_screen_id();
        let text = g.helptext.misc_text[21];
        crate::fonthand::je_d_string(g, vga, 120, 60, &text, FONT_SHAPES as u32); // game over
    }

    if g.varz.first_game_over {
        if !g.demo.play_demo {
            crate::loudness::play_song(g, u32::from(crate::musmast::SONG_GAMEOVER))?;
            let (music, fx) = (g.nortsong.tyr_music_volume, g.nortsong.fx_volume);
            crate::loudness::set_volume(g, music as u8, fx as u8);
        }
        g.varz.first_game_over = false;
    }

    if !g.demo.play_demo {
        crate::joystick::push_joysticks_as_keyboard(g);

        if crate::keyboard::has_input(g, INPUT_NO_MOTION) {
            g.varz.really_end_level = true;
        }
    }

    if g.network.is_network_game {
        g.varz.really_end_level = true;
    }
    Ok(())
}

/// The network update of `level_loop`: sends this frame's state, applies
/// the requests in the opponent's, and flags a desynchronized position.
fn network_frame(g: &mut Game) -> Flow {
    use crate::network::{je_clear_special_requests, network_state_send, network_state_update};
    let read16 = |p: &Option<crate::platform::UdpPacket>, at: usize| {
        let d = &p.as_ref().expect("state packet").data;
        u16::from_be_bytes([d[at], d[at + 1]])
    };

    if !g.varz.really_end_level {
        let n = &g.network;
        let requests = u16::from(n.pause_request)
            | (u16::from(n.in_game_menu_request) << 1)
            | (u16::from(n.skip_level_request) << 2)
            | (u16::from(n.nort_ship_request) << 3);
        let p = &g.player.player;
        let fields = [
            (14, requests),
            (16, i32::from(g.config.difficulty_level) as u16),
            (18, p[0].x as u16),
            (20, p[1].x as u16),
            (22, p[0].y as u16),
            (24, p[1].y as u16),
            (26, g.varz.cur_loc),
        ];
        let out = g.network.packet_state_out[0]
            .as_mut()
            .expect("state packet");
        for (at, v) in fields {
            out.data[at..at + 2].copy_from_slice(&v.to_be_bytes());
        }

        network_state_send(g);

        if network_state_update(g)? {
            let n = &g.network;
            let delay = n.network_delay as usize;
            debug_assert_eq!(
                read16(&n.packet_state_in[0], 26),
                read16(&n.packet_state_out[delay], 26)
            );

            let requests =
                read16(&n.packet_state_in[0], 14) ^ read16(&n.packet_state_out[delay], 14);
            if requests & 1 != 0 {
                crate::mainint::je_pause_game(g)?;
            }
            if requests & 2 != 0 {
                let n = &g.network;
                g.network.your_in_game_menu_request =
                    read16(&n.packet_state_out[delay], 14) & 2 != 0;
                crate::mainint::je_do_in_game_setup(g)?;
                g.network.your_in_game_menu_request = false;
                if g.network.halt_game {
                    g.varz.really_end_level = true;
                }
            }
            if requests & 4 != 0 {
                g.varz.level_timer = true;
                g.varz.level_timer_countdown = 0;
                g.varz.end_level = true;
                g.varz.level_end = 40;
            }
            if requests & 8 != 0 {
                // nortship
                let it = &mut g.player.player[0].items;
                it.ship = 12; // Nort Ship
                it.special = 13; // Astral Zone
                it.weapon[crate::player::FRONT_WEAPON].id = 36; // NortShip Super Pulse
                it.weapon[crate::player::REAR_WEAPON].id = 37; // NortShip Spreader
                g.varz.ship_gr = 1;
            }

            for i in 0..2 {
                let n = &g.network;
                let (in0, out) = (&n.packet_state_in[0], &n.packet_state_out[delay]);
                if read16(in0, 18 + i * 2) != read16(out, 18 + i * 2)
                    || read16(in0, 20 + i * 2) != read16(out, 20 + i * 2)
                {
                    let text = format!("Player {} is unsynchronized!\0", i + 1);
                    let game = g.video.game_screen.expect("game_screen");
                    let y = 110 + i as i32 * 10;
                    let shade = crate::fonthand::FULL_SHADE;
                    crate::fonthand::je_text_shade(g, game, 40, y, text.as_bytes(), 9, 2, shade);
                }
            }
        }
    }

    je_clear_special_requests(g);
    Ok(())
}

/// After the frame: restart stopped backgrounds when no enemies remain, and
/// end the level when it is ready to.
fn end_of_frame_checks(g: &mut Game) {
    let v = &mut g.varz;
    let bk = &mut g.backgrnd;
    if v.stop_background_num == 9 && bk.back_move == 0 && !v.enemy_still_exploding {
        bk.back_move = 1;
        bk.back_move2 = 2;
        bk.back_move3 = 3;
        v.explode_move = 2;
        v.stop_background_num = 0;
        v.stop_backgrounds = false;
        if v.wait_to_end_level {
            v.end_level = true;
            v.level_end = 40;
        }
        if v.all_players_gone {
            v.really_end_level = true;
        }
    }

    if !v.end_level && v.enemy_on_screen == 0 {
        if v.ready_to_end_level && !v.enemy_still_exploding {
            if v.level_timer_countdown > 0 {
                v.level_timer = false;
            }
            v.ready_to_end_level = false;
            v.end_level = true;
            v.level_end = 40;
            if v.all_players_gone {
                v.really_end_level = true;
            }
        }
        if v.stop_backgrounds {
            v.stop_backgrounds = false;
            bk.back_move = 1;
            bk.back_move2 = 2;
            bk.back_move3 = 3;
            v.explode_move = 2;
        }
    }
}
