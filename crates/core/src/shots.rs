// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `shots.c` / `shots.h`.

use crate::Game;
use crate::episodes::JeWeaponType;
use crate::lvlmast::{PORT_NUM, WEAP_NUM};
use crate::sprite::blit_sprite2;
use crate::varz::Sprite2Ref;
use libm::{cosf, sinf};
use serde::{Deserialize, Serialize};
use std::f64::consts::{FRAC_PI_2, PI};

/// `PlayerShotDataType`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerShotDataType {
    pub shot_x: i16,
    pub shot_y: i16,
    pub shot_xm: i16,
    pub shot_ym: i16,
    pub shot_xc: i16,
    pub shot_yc: i16,
    pub shot_complicated: bool,
    pub shot_dev_x: i16,
    pub shot_dir_x: i16,
    pub shot_dev_y: i16,
    pub shot_dir_y: i16,
    pub shot_cir_size_x: i16,
    pub shot_cir_size_y: i16,
    pub shot_trail: u8,
    pub shot_gr: u16,
    pub shot_ani: u16,
    pub shot_ani_max: u16,
    pub shot_dmg: u8,
    pub shot_blast_filter: u8,
    pub chain_reaction: u8,
    pub player_number: u8,
    pub aim_at_enemy: u8,
    pub aim_delay: u8,
    pub aim_delay_max: u8,
}

pub const MAX_PWEAPON: usize = 81;

/// `linkMultiGr` `[0..16]`.
pub const LINK_MULTI_GR: [u16; 17] = [
    77, 221, 183, 301, 1, 282, 164, 202, 58, 201, 163, 281, 39, 300, 182, 220, 77,
];
/// `linkSonicGr` `[0..16]`.
pub const LINK_SONIC_GR: [u16; 17] = [
    85, 242, 131, 303, 47, 284, 150, 223, 66, 224, 149, 283, 9, 302, 130, 243, 85,
];
/// `linkMult2Gr` `[0..16]`.
pub const LINK_MULT2_GR: [u16; 17] = [
    78, 299, 295, 297, 2, 278, 276, 280, 59, 279, 275, 277, 40, 296, 294, 298, 78,
];

/// Mutable globals of this module (rule R3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    #[serde(with = "crate::serde_big::boxed")]
    pub player_shot_data: Box<[PlayerShotDataType; MAX_PWEAPON + 1]>,
    #[serde(with = "crate::serde_big")]
    pub shot_avail: [u8; MAX_PWEAPON],
}

impl Default for State {
    fn default() -> Self {
        State {
            player_shot_data: Box::new([PlayerShotDataType::default(); MAX_PWEAPON + 1]),
            shot_avail: [0; MAX_PWEAPON],
        }
    }
}

/// `soundChannel` (function-local constant of `player_shot_create`).
const SOUND_CHANNEL: [u8; 11] = [0, 2, 4, 4, 2, 2, 5, 5, 1, 4, 1];

pub fn simulate_player_shots(g: &mut Game) {
    // Player Shot Images
    for z in 0..MAX_PWEAPON {
        if g.shots.shot_avail[z] == 0 {
            continue;
        }
        g.shots.shot_avail[z] -= 1;
        if z == MAX_PWEAPON - 1 {
            continue;
        }

        let delta_y_shot_move = g.player.player[0].delta_y_shot_move;
        let shot = &mut g.shots.player_shot_data[z];

        shot.shot_xm = shot.shot_xm.wrapping_add(shot.shot_xc);

        if shot.shot_xm <= 100 {
            shot.shot_x = shot.shot_x.wrapping_add(shot.shot_xm);
        }

        shot.shot_ym = shot.shot_ym.wrapping_add(shot.shot_yc);
        shot.shot_y = shot.shot_y.wrapping_add(shot.shot_ym);

        if shot.shot_ym > 100 {
            shot.shot_y = shot.shot_y.wrapping_sub(120);
            shot.shot_y = (i32::from(shot.shot_y).wrapping_add(delta_y_shot_move)) as i16;
        }

        if shot.shot_complicated {
            shot.shot_dev_x = shot.shot_dev_x.wrapping_add(shot.shot_dir_x);
            shot.shot_x = shot.shot_x.wrapping_add(shot.shot_dev_x);

            if i32::from(shot.shot_dev_x).abs() == i32::from(shot.shot_cir_size_x) {
                shot.shot_dir_x = shot.shot_dir_x.wrapping_neg();
            }

            shot.shot_dev_y = shot.shot_dev_y.wrapping_add(shot.shot_dir_y);
            shot.shot_y = shot.shot_y.wrapping_add(shot.shot_dev_y);

            if i32::from(shot.shot_dev_y).abs() == i32::from(shot.shot_cir_size_y) {
                shot.shot_dir_y = shot.shot_dir_y.wrapping_neg();
            }
            // Double Speed Circle Shots - add a second copy of above loop
        }

        let temp_shot_x = i32::from(shot.shot_x);
        let temp_shot_y = i32::from(shot.shot_y);

        if shot.shot_x < 0 || shot.shot_x > 140 || shot.shot_y < 0 || shot.shot_y > 170 {
            g.shots.shot_avail[z] = 0;
            continue;
        }

        let mut anim_frame = shot.shot_gr.wrapping_add(shot.shot_ani);
        shot.shot_ani = shot.shot_ani.wrapping_add(1);
        if shot.shot_ani == shot.shot_ani_max {
            shot.shot_ani = 0;
        }

        if anim_frame < 60000 {
            if anim_frame > 1000 {
                anim_frame %= 1000;
            }
            let screen = g.video.vga_screen_id();
            if anim_frame > 500 {
                blit_sprite2(
                    g,
                    screen,
                    temp_shot_x + 1,
                    temp_shot_y,
                    Sprite2Ref::SpriteSheet12,
                    u32::from(anim_frame - 500),
                );
            } else {
                blit_sprite2(
                    g,
                    screen,
                    temp_shot_x + 1,
                    temp_shot_y,
                    Sprite2Ref::SpriteSheet8,
                    u32::from(anim_frame),
                );
            }
        }
    }
}

pub fn player_shot_set_direction(g: &mut Game, shot_id: i16, weapon_id: u32, direction: f32) {
    let shot = &mut g.shots.player_shot_data[shot_id as usize];

    shot.shot_xm = (-(sinf(direction) * f32::from(shot.shot_ym)).round()) as i16;
    shot.shot_ym = (-(cosf(direction) * f32::from(shot.shot_ym)).round()) as i16;

    // 16 directions; C computes the product in double and rounds it as float.
    let rounded_dir = || ((f64::from(direction) * (16.0 / (2.0 * PI))) as f32).round() as usize;

    // Some weapons have sprites for each direction, use those.
    match weapon_id {
        27 | 32 | 10 => shot.shot_gr = LINK_MULTI_GR[rounded_dir()],
        28 | 33 | 11 => shot.shot_gr = LINK_SONIC_GR[rounded_dir()],
        30 | 35 | 14 => {
            let d = f64::from(direction);
            if d > FRAC_PI_2 && d < PI + FRAC_PI_2 {
                shot.shot_yc = 1;
            }
        }
        38 | 22 => shot.shot_gr = LINK_MULT2_GR[rounded_dir()],
        _ => {}
    }
}

/// The per-position fields of a weapon at index `i` of its 8-entry arrays,
/// where `i` may be 8: upstream then reads the next field of the struct
/// (`ub-guard`, see `D-UB-SHOTS-1`), which is reproduced here.
struct WeaponPos {
    attack: u8,
    del: u8,
    sx: i8,
    sy: i8,
    bx: i8,
    by: i8,
    sg: u16,
}

fn weapon_pos(w: &JeWeaponType, i: usize) -> WeaponPos {
    if i < 8 {
        return WeaponPos {
            attack: w.attack[i],
            del: w.del[i],
            sx: w.sx[i],
            sy: w.sy[i],
            bx: w.bx[i],
            by: w.by[i],
            sg: w.sg[i],
        };
    }
    debug_assert_eq!(i, 8);
    WeaponPos {
        attack: w.del[0],
        del: w.sx[0] as u8,
        sx: w.sy[0],
        sy: w.bx[0],
        bx: w.by[0],
        by: w.sg[0] as u8 as i8,
        sg: u16::from_le_bytes([w.acceleration as u8, w.accelerationx as u8]),
    }
}

/// `player_shot_create` (FKA `Varz.initPlayerShot`).
#[allow(clippy::too_many_arguments)]
pub fn player_shot_create(
    g: &mut Game,
    port_num: u16,
    bay_i: u32,
    px: u16,
    py: u16,
    mouse_x: u16,
    mouse_y: u16,
    wp_num: u16,
    player_num: u8,
) -> i16 {
    const NONE: i16 = MAX_PWEAPON as i16;
    let bay_i = bay_i as usize;

    // Bounds check
    if usize::from(port_num) > PORT_NUM || wp_num == 0 || usize::from(wp_num) > WEAP_NUM {
        return NONE;
    }

    let weapon = g.episodes.weapons[usize::from(wp_num)];
    let poweruse = g.episodes.weapon_port[usize::from(port_num)].poweruse;

    if g.config.power < u32::from(poweruse) {
        return NONE;
    }
    g.config.power -= u32::from(poweruse);

    if weapon.sound > 0 {
        g.varz.sound_queue[usize::from(SOUND_CHANNEL[bay_i])] = weapon.sound;
    }

    let mut shot_id = MAX_PWEAPON;
    // Rot
    for _multi_i in 1..=weapon.multi {
        shot_id = g
            .shots
            .shot_avail
            .iter()
            .position(|&a| a == 0)
            .unwrap_or(MAX_PWEAPON);
        if shot_id == MAX_PWEAPON {
            return NONE;
        }

        let multi_pos = &mut g.config.shot_multi_pos[bay_i];
        if *multi_pos == weapon.max || *multi_pos > 8 {
            *multi_pos = 1;
        } else {
            *multi_pos += 1;
        }
        let w = weapon_pos(&weapon, usize::from(*multi_pos) - 1);

        let shot = &mut g.shots.player_shot_data[shot_id];
        shot.chain_reaction = 0;

        shot.player_number = player_num;

        shot.shot_ani = 0;

        shot.shot_complicated = weapon.circlesize != 0;

        if weapon.circlesize == 0 {
            shot.shot_dev_x = 0;
            shot.shot_dir_x = 0;
            shot.shot_dev_y = 0;
            shot.shot_dir_y = 0;
            shot.shot_cir_size_x = 0;
            shot.shot_cir_size_y = 0;
        } else {
            let mut circsize = weapon.circlesize;

            if circsize > 19 {
                let circsize_mod20 = circsize % 20;
                shot.shot_cir_size_x = i16::from(circsize_mod20);
                shot.shot_dev_x = i16::from(circsize_mod20 >> 1);

                circsize /= 20;
                shot.shot_cir_size_y = i16::from(circsize);
                shot.shot_dev_y = i16::from(circsize >> 1);
            } else {
                shot.shot_cir_size_x = i16::from(circsize);
                shot.shot_cir_size_y = i16::from(circsize);
                shot.shot_dev_x = i16::from(circsize >> 1);
                shot.shot_dev_y = i16::from(circsize >> 1);
            }
            shot.shot_dir_x = 1;
            shot.shot_dir_y = -1;
        }

        shot.shot_trail = weapon.trail;

        if w.attack > 99 && w.attack < 250 {
            shot.chain_reaction = w.attack - 100;
            shot.shot_dmg = 1;
        } else {
            shot.shot_dmg = w.attack;
        }

        shot.shot_blast_filter = weapon.shipblastfilter;

        let mut tmp_by = i16::from(w.by);

        // Note: Only front selection used for player shots...

        shot.shot_x = (i32::from(px) + i32::from(w.bx)) as i16;

        shot.shot_y = (i32::from(py) + i32::from(tmp_by)) as i16;
        shot.shot_yc = -i16::from(weapon.acceleration);
        shot.shot_xc = i16::from(weapon.accelerationx);

        shot.shot_xm = i16::from(w.sx);

        // Not sure what this field does exactly.
        let mut del = w.del;

        if del == 121 {
            shot.shot_trail = 0;
            del = 255;
        }

        shot.shot_gr = w.sg;
        g.shots.shot_avail[shot_id] = if shot.shot_gr == 0 { 0 } else { del };

        if del > 100 && del < 120 {
            shot.shot_ani_max = u16::from(del) - 100 + 1;
        } else {
            shot.shot_ani_max = weapon.weapani.wrapping_add(1);
        }

        if del == 99 || del == 98 {
            tmp_by = (i32::from(px) - i32::from(mouse_x)) as i16;
            tmp_by = tmp_by.clamp(-5, 5);
            shot.shot_xm = shot.shot_xm.wrapping_add(tmp_by);
        }

        let player = |p: u8| &g.player.player[usize::from(p) - 1];

        if del == 99 || del == 100 {
            tmp_by = (i32::from(py) - i32::from(mouse_y) - i32::from(w.sy)) as i16;
            tmp_by = tmp_by.clamp(-4, 4);
            shot.shot_ym = tmp_by;
        } else if w.sy == 98 {
            shot.shot_ym = 0;
            shot.shot_yc = -1;
        } else if w.sy > 100 {
            shot.shot_ym = i16::from(w.sy);
            shot.shot_y =
                (i32::from(shot.shot_y) - player(shot.player_number).delta_y_shot_move) as i16;
        } else {
            shot.shot_ym = -i16::from(w.sy);
        }

        if w.sx > 100 {
            shot.shot_xm = i16::from(w.sx);
            shot.shot_x =
                (i32::from(shot.shot_x) - player(shot.player_number).delta_x_shot_move) as i16;
            if shot.shot_xm == 101 {
                shot.shot_y =
                    (i32::from(shot.shot_y) - player(shot.player_number).delta_y_shot_move) as i16;
            }
        }

        if weapon.aim > 5 {
            // Guided Shot
            let mut best_dist: u32 = 65000;
            let mut closest_enemy: u8 = 0;
            // Find Closest Enemy
            g.varz.x = 0;
            while g.varz.x < 100 {
                let x = usize::from(g.varz.x);
                let enemy = &g.varz.enemy[x];
                if g.varz.enemy_avail[x] != 1 && !enemy.scoreitem {
                    g.varz.y = ((i32::from(enemy.ex) - i32::from(shot.shot_x)).abs()
                        + (i32::from(enemy.ey) - i32::from(shot.shot_y)).abs())
                        as u16;
                    if u32::from(g.varz.y) < best_dist {
                        best_dist = u32::from(g.varz.y);
                        closest_enemy = (x + 1) as u8;
                    }
                }
                g.varz.x += 1;
            }
            shot.aim_at_enemy = closest_enemy;
            shot.aim_delay = 5;
            shot.aim_delay_max = weapon.aim - 5;
        } else {
            shot.aim_at_enemy = 0;
        }

        g.config.shot_repeat[bay_i] = weapon.shotrepeat;
    }

    shot_id as i16
}
