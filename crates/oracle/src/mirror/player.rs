//! Mirror of `rustyrian_core::player::State` <-> the C globals of `player.c`.

use super::{Mirror, global, global_mut};
use crate::bindings as b;
use rustyrian_core::player::{Player, PlayerItems, PlayerItemsWeapon, PlayerSidekick, State};

pub fn player_items_from_c(c: &b::PlayerItems) -> PlayerItems {
    PlayerItems {
        ship: c.ship,
        generator: c.generator,
        shield: c.shield,
        weapon: c.weapon.map(|w| PlayerItemsWeapon {
            id: w.id,
            power: w.power,
        }),
        sidekick: c.sidekick,
        special: c.special,
        sidekick_series: c.sidekick_series,
        sidekick_level: c.sidekick_level,
        super_arcade_mode: c.super_arcade_mode,
    }
}

pub fn player_items_to_c(r: &PlayerItems) -> b::PlayerItems {
    b::PlayerItems {
        ship: r.ship,
        generator: r.generator,
        shield: r.shield,
        weapon: r.weapon.map(|w| b::PlayerItems__bindgen_ty_1 {
            id: w.id,
            power: w.power,
        }),
        sidekick: r.sidekick,
        special: r.special,
        sidekick_series: r.sidekick_series,
        sidekick_level: r.sidekick_level,
        super_arcade_mode: r.super_arcade_mode,
    }
}

fn sidekick_from_c(c: &b::Player__bindgen_ty_1) -> PlayerSidekick {
    PlayerSidekick {
        ammo_max: c.ammo_max,
        ammo_refill_ticks_max: c.ammo_refill_ticks_max,
        style: c.style,
        x: c.x,
        y: c.y,
        ammo: c.ammo,
        ammo_refill_ticks: c.ammo_refill_ticks,
        animation_enabled: c.animation_enabled,
        animation_frame: c.animation_frame,
        charge: c.charge,
        charge_ticks: c.charge_ticks,
    }
}

fn sidekick_to_c(r: &PlayerSidekick) -> b::Player__bindgen_ty_1 {
    b::Player__bindgen_ty_1 {
        ammo_max: r.ammo_max,
        ammo_refill_ticks_max: r.ammo_refill_ticks_max,
        style: r.style,
        x: r.x,
        y: r.y,
        ammo: r.ammo,
        ammo_refill_ticks: r.ammo_refill_ticks,
        animation_enabled: r.animation_enabled,
        animation_frame: r.animation_frame,
        charge: r.charge,
        charge_ticks: r.charge_ticks,
    }
}

/// Converts `player[p]` (at address `c`). `lives` must point at
/// `c->items.weapon[w].power`.
fn player_from_c(c: &b::Player) -> Player {
    let lives = if c.lives.is_null() {
        None
    } else {
        let w = (0..2).find(|&w| std::ptr::eq(c.lives, &c.items.weapon[w].power));
        Some(w.expect("player.lives points at its own items.weapon[w].power"))
    };
    #[allow(clippy::useless_conversion)]
    let cash = u64::from(c.cash);
    Player {
        cash,
        items: player_items_from_c(&c.items),
        last_items: player_items_from_c(&c.last_items),
        is_dragonwing: c.is_dragonwing,
        lives,
        shield_max: c.shield_max,
        initial_armor: c.initial_armor,
        shot_hit_area_x: c.shot_hit_area_x,
        shot_hit_area_y: c.shot_hit_area_y,
        is_alive: c.is_alive,
        invulnerable_ticks: c.invulnerable_ticks,
        exploding_ticks: c.exploding_ticks,
        shield: c.shield,
        armor: c.armor,
        weapon_mode: c.weapon_mode,
        superbombs: c.superbombs,
        purple_balls_needed: c.purple_balls_needed,
        mouse_x: c.mouseX,
        mouse_y: c.mouseY,
        x: c.x,
        y: c.y,
        old_x: c.old_x,
        old_y: c.old_y,
        x_velocity: c.x_velocity,
        y_velocity: c.y_velocity,
        x_friction_ticks: c.x_friction_ticks,
        y_friction_ticks: c.y_friction_ticks,
        delta_x_shot_move: c.delta_x_shot_move,
        delta_y_shot_move: c.delta_y_shot_move,
        last_x_shot_move: c.last_x_shot_move,
        last_y_shot_move: c.last_y_shot_move,
        last_x_explosion_follow: c.last_x_explosion_follow,
        last_y_explosion_follow: c.last_y_explosion_follow,
        sidekick: c.sidekick.map(|s| sidekick_from_c(&s)),
    }
}

/// Writes `r` into the C player at `c` (in place, so `lives` can point into
/// it).
fn player_to_c(r: &Player, c: &mut b::Player) {
    c.cash = r.cash as b::ulong;
    c.items = player_items_to_c(&r.items);
    c.last_items = player_items_to_c(&r.last_items);
    c.is_dragonwing = r.is_dragonwing;
    c.lives = match r.lives {
        None => std::ptr::null_mut(),
        Some(w) => &raw mut c.items.weapon[w].power,
    };
    c.shield_max = r.shield_max;
    c.initial_armor = r.initial_armor;
    c.shot_hit_area_x = r.shot_hit_area_x;
    c.shot_hit_area_y = r.shot_hit_area_y;
    c.is_alive = r.is_alive;
    c.invulnerable_ticks = r.invulnerable_ticks;
    c.exploding_ticks = r.exploding_ticks;
    c.shield = r.shield;
    c.armor = r.armor;
    c.weapon_mode = r.weapon_mode;
    c.superbombs = r.superbombs;
    c.purple_balls_needed = r.purple_balls_needed;
    c.mouseX = r.mouse_x;
    c.mouseY = r.mouse_y;
    c.x = r.x;
    c.y = r.y;
    c.old_x = r.old_x;
    c.old_y = r.old_y;
    c.x_velocity = r.x_velocity;
    c.y_velocity = r.y_velocity;
    c.x_friction_ticks = r.x_friction_ticks;
    c.y_friction_ticks = r.y_friction_ticks;
    c.delta_x_shot_move = r.delta_x_shot_move;
    c.delta_y_shot_move = r.delta_y_shot_move;
    c.last_x_shot_move = r.last_x_shot_move;
    c.last_y_shot_move = r.last_y_shot_move;
    c.last_x_explosion_follow = r.last_x_explosion_follow;
    c.last_y_explosion_follow = r.last_y_explosion_follow;
    c.sidekick = r.sidekick.map(|s| sidekick_to_c(&s));
}

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make.
        let c = unsafe { global(&raw const b::player) };
        self.player = [player_from_c(&c[0]), player_from_c(&c[1])];
    }

    fn store_to_c(&self) {
        // SAFETY: as above.
        let c = unsafe { global_mut(&raw mut b::player) };
        for (r, c) in self.player.iter().zip(c.iter_mut()) {
            player_to_c(r, c);
        }
    }
}
