//! Differential tests for `player.c` / `player.h`.

use proptest::prelude::*;
use rustyrian_core::player::{
    self as pl, Player, PlayerItems, PlayerItemsWeapon, PlayerSidekick, State,
};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;

/// A player whose `lives` points at one of its weapons' power (0..=11, the
/// range upstream keeps it in).
fn any_player() -> impl Strategy<Value = Player> {
    (
        any::<[u8; 2]>(),
        prop::array::uniform2(0u8..=12),
        0usize..2,
        0u32..4,
        any::<bool>(),
        any::<u64>(),
    )
        .prop_map(|(ids, powers, w, balls, dragon, cash)| {
            let mut p = Player::default();
            for i in 0..2 {
                p.items.weapon[i].id = ids[i] % 3;
                p.items.weapon[i].power = powers[i];
            }
            p.items.weapon[w].power = p.items.weapon[w].power.min(11);
            p.lives = Some(w);
            p.purple_balls_needed = balls;
            p.is_dragonwing = dragon;
            p.cash = cash;
            p
        })
}

fn setup(players: [Player; 2], multi: [u8; 11]) -> Diff {
    let mut d = Diff::new();
    d.game.player.player = players;
    d.game.config.shot_multi_pos = multi;
    d.store_to_c();
    d
}

fn check(d: &Diff) -> Result<(), TestCaseError> {
    prop_assert_eq!(diff::load::<State>(), d.game.player.clone());
    prop_assert_eq!(
        diff::load::<rustyrian_core::config::State>(),
        d.game.config.clone()
    );
    prop_assert!(d.c_calls().is_empty() && d.rust_calls().is_empty());
    Ok(())
}

proptest! {
    #[test]
    fn calc_purple_balls_needed_matches(p0 in any_player(), p1 in any_player(), which in 0usize..2, multi: [u8; 11]) {
        let mut d = setup([p0, p1], multi);
        // SAFETY: `player[which]` with `lives` pointing into it.
        unsafe { b::calc_purple_balls_needed(&raw mut b::player[which]) };
        pl::calc_purple_balls_needed(&mut d.game, which);
        check(&d)?;
    }

    #[test]
    fn power_up_weapon_matches(p0 in any_player(), p1 in any_player(), which in 0usize..2, port in 0u32..2, multi: [u8; 11]) {
        let mut d = setup([p0, p1], multi);
        // SAFETY: as above.
        let c = unsafe { b::power_up_weapon(&raw mut b::player[which], port) };
        prop_assert_eq!(c, pl::power_up_weapon(&mut d.game, which, port));
        check(&d)?;
    }

    #[test]
    fn handle_got_purple_ball_matches(p0 in any_player(), p1 in any_player(), which in 0usize..2, multi: [u8; 11]) {
        let mut d = setup([p0, p1], multi);
        // SAFETY: as above.
        unsafe { b::handle_got_purple_ball(&raw mut b::player[which]) };
        pl::handle_got_purple_ball(&mut d.game, which);
        check(&d)?;
    }
}

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

fn items(k: u8) -> PlayerItems {
    PlayerItems {
        ship: k,
        generator: k + 1,
        shield: k + 2,
        weapon: [
            PlayerItemsWeapon {
                id: k + 3,
                power: 4,
            },
            PlayerItemsWeapon {
                id: k + 5,
                power: 6,
            },
        ],
        sidekick: [k + 7, k + 8],
        special: k + 9,
        sidekick_series: k + 10,
        sidekick_level: k + 11,
        super_arcade_mode: k + 12,
    }
}

fn player(p: usize) -> Player {
    let k = p as i32;
    Player {
        cash: 0x1_0000_0000 + p as u64,
        items: items(p as u8 * 20),
        last_items: items(p as u8 * 20 + 50),
        is_dragonwing: p == 1,
        lives: Some(p),
        shield_max: 1,
        initial_armor: 2,
        shot_hit_area_x: 3,
        shot_hit_area_y: 4,
        is_alive: true,
        invulnerable_ticks: 5,
        exploding_ticks: 6,
        shield: 7,
        armor: 8,
        weapon_mode: 9,
        superbombs: 10,
        purple_balls_needed: 11,
        mouse_x: 12,
        mouse_y: 13,
        x: -k,
        y: k * 100,
        old_x: std::array::from_fn(|i| i as i32 - k),
        old_y: std::array::from_fn(|i| k - i as i32),
        x_velocity: -14,
        y_velocity: 15,
        x_friction_ticks: 16,
        y_friction_ticks: 17,
        delta_x_shot_move: -18,
        delta_y_shot_move: 19,
        last_x_shot_move: -20,
        last_y_shot_move: 21,
        last_x_explosion_follow: -22,
        last_y_explosion_follow: 23,
        sidekick: std::array::from_fn(|s| PlayerSidekick {
            ammo_max: -(s as i32),
            ammo_refill_ticks_max: 24,
            style: 25,
            x: 26,
            y: -27,
            ammo: 28,
            ammo_refill_ticks: 29,
            animation_enabled: true,
            animation_frame: 30,
            charge: 31,
            charge_ticks: 32,
        }),
    }
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let s = State {
        player: [player(0), player(1)],
    };
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
    assert_eq!(s.player[1].lives(), s.player[1].items.weapon[1].power);
}
