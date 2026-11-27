// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `player.c` / `player.h`.

use crate::Game;
use serde::{Deserialize, Serialize};

pub const FRONT_WEAPON: usize = 0;
pub const REAR_WEAPON: usize = 1;

pub const LEFT_SIDEKICK: usize = 0;
pub const RIGHT_SIDEKICK: usize = 1;

/// Element of `PlayerItems::weapon` (anonymous struct in C).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerItemsWeapon {
    pub id: u8,
    pub power: u8,
}

/// `PlayerItems`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerItems {
    pub ship: u8,
    pub generator: u8,
    pub shield: u8,
    pub weapon: [PlayerItemsWeapon; 2],
    pub sidekick: [u8; 2],
    pub special: u8,
    /// Dragonwing only: repeatedly collecting the same powerup gives a series
    /// of sidekick upgrades.
    pub sidekick_series: u8,
    pub sidekick_level: u8,
    /// Single-player only; stored as an item for compatibility.
    pub super_arcade_mode: u8,
}

/// Element of `Player::sidekick` (anonymous struct in C).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerSidekick {
    // calculatable
    pub ammo_max: i32,
    pub ammo_refill_ticks_max: u32,
    /// Affects movement and size.
    pub style: u32,
    // state
    pub x: i32,
    pub y: i32,
    pub ammo: i32,
    pub ammo_refill_ticks: u32,
    pub animation_enabled: bool,
    pub animation_frame: u32,
    pub charge: u32,
    pub charge_ticks: u32,
}

/// `Player`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Player {
    /// `ulong` (rule R4).
    pub cash: u64,
    pub items: PlayerItems,
    pub last_items: PlayerItems,
    /// I.e., is player 2.
    pub is_dragonwing: bool,
    /// `Uint8 *lives` (rule R8): `Some(w)` means `&self.items.weapon[w].power`,
    /// the only target upstream assigns (`mainint.c:1053`); `None` is NULL.
    /// Read it through [`Player::lives`].
    pub lives: Option<usize>,
    // calculatable
    pub shield_max: u32,
    pub initial_armor: u32,
    pub shot_hit_area_x: u32,
    pub shot_hit_area_y: u32,
    // state
    pub is_alive: bool,
    /// Ticks until ship can be damaged.
    pub invulnerable_ticks: u32,
    /// Ticks until ship done exploding.
    pub exploding_ticks: u32,
    pub shield: u32,
    pub armor: u32,
    pub weapon_mode: u32,
    pub superbombs: u32,
    pub purple_balls_needed: u32,
    /// FKA `NortVars.mouseX`, `Varz.mouseXB`.
    pub mouse_x: u16,
    /// FKA `NortVars.mouseY`, `Varz.mouseYB`.
    pub mouse_y: u16,
    pub x: i32,
    pub y: i32,
    pub old_x: [i32; 20],
    pub old_y: [i32; 20],
    pub x_velocity: i32,
    pub y_velocity: i32,
    /// Ticks until friction is applied.
    pub x_friction_ticks: u32,
    pub y_friction_ticks: u32,
    pub delta_x_shot_move: i32,
    pub delta_y_shot_move: i32,
    pub last_x_shot_move: i32,
    pub last_y_shot_move: i32,
    pub last_x_explosion_follow: i32,
    pub last_y_explosion_follow: i32,
    pub sidekick: [PlayerSidekick; 2],
}

impl Player {
    /// `*player->lives`. Upstream dereferences only after assigning it.
    pub fn lives(&self) -> u8 {
        let w = self.lives.expect("player lives pointer is set");
        self.items.weapon[w].power
    }

    /// `*player->lives = v`.
    pub fn set_lives(&mut self, v: u8) {
        let w = self.lives.expect("player lives pointer is set");
        self.items.weapon[w].power = v;
    }
}

/// Mutable globals of this module (rule R3).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub player: [Player; 2],
}

/// `purple_balls_required` (function-local constant of
/// `calc_purple_balls_needed`).
const PURPLE_BALLS_REQUIRED: [u32; 12] = [1, 1, 2, 4, 8, 12, 16, 20, 25, 30, 40, 50];

/// `calc_purple_balls_needed(&player[p])`.
pub fn calc_purple_balls_needed(g: &mut Game, p: usize) {
    let this_player = &mut g.player.player[p];
    this_player.purple_balls_needed = PURPLE_BALLS_REQUIRED[usize::from(this_player.lives())];
}

/// `power_up_weapon(&player[p], port)`.
pub fn power_up_weapon(g: &mut Game, p: usize, port: u32) -> bool {
    let port = port as usize;
    let this_player = &mut g.player.player[p];
    let can_power_up =
        this_player.items.weapon[port].id != 0 && this_player.items.weapon[port].power < 11;
    if can_power_up {
        this_player.items.weapon[port].power += 1;
        g.config.shot_multi_pos[port] = 0;

        calc_purple_balls_needed(g, p);
    } else {
        // cash consolation prize
        this_player.cash = this_player.cash.wrapping_add(1000);
    }

    can_power_up
}

/// `handle_got_purple_ball(&player[p])`.
pub fn handle_got_purple_ball(g: &mut Game, p: usize) {
    let this_player = &mut g.player.player[p];
    if this_player.purple_balls_needed > 1 {
        this_player.purple_balls_needed -= 1;
    } else {
        let port = if this_player.is_dragonwing {
            REAR_WEAPON
        } else {
            FRONT_WEAPON
        };
        power_up_weapon(g, p, port as u32);
    }
}
