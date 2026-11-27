//! Differential tests for `shots.c` / `shots.h`.

use proptest::prelude::*;
use rustyrian_core::lvlmast::{PORT_NUM, WEAP_NUM};
use rustyrian_core::shots::{self as sh, MAX_PWEAPON, PlayerShotDataType, State};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;

/// Shot data, the bookkeeping in `config`, the players and the `varz`
/// globals the functions touch agree.
fn check(d: &Diff) -> Result<(), TestCaseError> {
    let c = d.c_game_state();
    let r = d.game.snapshot();
    prop_assert_eq!(c.shots, r.shots);
    prop_assert_eq!(c.config, r.config);
    prop_assert_eq!(c.player, r.player);
    prop_assert_eq!((c.varz.x, c.varz.y), (r.varz.x, r.varz.y));
    prop_assert_eq!(c.varz.sound_queue, r.varz.sound_queue);
    prop_assert!(d.c_calls().is_empty() && d.rust_calls().is_empty());
    Ok(())
}

proptest! {
    #[test]
    fn player_shot_set_direction_matches(
        shot_id in 0i16..=MAX_PWEAPON as i16,
        weapon_id in prop_oneof![
            prop::sample::select(vec![10u32, 11, 14, 22, 27, 28, 30, 32, 33, 35, 38]),
            any::<u32>(),
        ],
        direction in 0.0f32..std::f32::consts::TAU,
        ym: i16,
        yc: i16,
    ) {
        let mut d = Diff::new();
        let shot = &mut d.game.shots.player_shot_data[shot_id as usize];
        shot.shot_ym = ym % 200;
        shot.shot_yc = yc;
        shot.shot_gr = 7;
        d.store_to_c();
        // SAFETY: plain C call.
        unsafe { b::player_shot_set_direction(shot_id, weapon_id, direction) };
        sh::player_shot_set_direction(&mut d.game, shot_id, weapon_id, direction);
        check(&d)?;
    }

    /// From mid-level fixtures (episode item data loaded, enemies on screen)
    /// with perturbed arguments and shot bookkeeping, including multi
    /// positions past a weapon's `max` (`D-UB-SHOTS-1`).
    #[test]
    fn player_shot_create_matches(
        frame in prop::sample::select(vec![500u64, 2000]),
        port_num in 0u16..=PORT_NUM as u16 + 1,
        bay_i in 0u32..11,
        pos: (u16, u16, u16, u16),
        wp_num in prop_oneof![0u16..=WEAP_NUM as u16 + 1, 0u16..30],
        player_num in 1u8..=2,
        power in prop_oneof![0u32..50, any::<u32>()],
        multi_pos in 0u8..=10,
        avail in prop::collection::vec(prop_oneof![Just(0u8), any::<u8>()], MAX_PWEAPON),
        tweak: (i8, i8, u8, u8, u8, u8),
        deltas: (i32, i32, i32, i32),
    ) {
        let mut d = Diff::new();
        d.game.restore(diff::fixture("demo-cycle", frame));
        let g = &mut d.game;
        g.config.power = power;
        g.config.shot_multi_pos[bay_i as usize] = multi_pos;
        g.shots.shot_avail.copy_from_slice(&avail);
        let (px, py, mx, my) = pos;
        let (px, py, mx, my) = (px % 400, py % 300, mx % 400, my % 300);
        if let Some(w) = g.episodes.weapons.get_mut(usize::from(wp_num)) {
            // Exercise the special `sx`/`sy`/`del` values and guided shots.
            let i = usize::from(tweak.2 % 8);
            w.sx[i] = [w.sx[i], 101, 102, tweak.0][usize::from(tweak.3 % 4)];
            w.sy[i] = [w.sy[i], 98, 101, tweak.1][usize::from(tweak.4 % 4)];
            w.del[i] = [w.del[i], 98, 99, 100, 105, 121, tweak.2][usize::from(tweak.5 % 7)];
            if tweak.5 % 3 == 0 {
                w.aim = 6 + tweak.2 % 4;
            }
            if tweak.3 % 5 == 0 {
                w.circlesize = tweak.4;
            }
        }
        for p in g.player.player.iter_mut() {
            p.delta_x_shot_move = deltas.0 % 1000;
            p.delta_y_shot_move = deltas.1 % 1000;
        }
        g.varz.x = deltas.2 as u16;
        g.varz.y = deltas.3 as u16;
        d.store_to_c();
        // SAFETY: plain C call on fixture state.
        let c = unsafe { b::player_shot_create(port_num, bay_i, px, py, mx, my, wp_num, player_num) };
        let r = sh::player_shot_create(&mut d.game, port_num, bay_i, px, py, mx, my, wp_num, player_num);
        prop_assert_eq!(c, r);
        check(&d)?;
    }
}

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let mut s = State::default();
    for (i, x) in s.player_shot_data.iter_mut().enumerate() {
        let n = i as i16;
        *x = PlayerShotDataType {
            shot_x: n,
            shot_y: -n,
            shot_xm: 2 * n,
            shot_ym: -2 * n,
            shot_xc: 3,
            shot_yc: -3,
            shot_complicated: i % 2 == 0,
            shot_dev_x: 4,
            shot_dir_x: -1,
            shot_dev_y: 5,
            shot_dir_y: 1,
            shot_cir_size_x: 6,
            shot_cir_size_y: 7,
            shot_trail: 8,
            shot_gr: 900 + i as u16,
            shot_ani: 10,
            shot_ani_max: 11,
            shot_dmg: 12,
            shot_blast_filter: 13,
            chain_reaction: 14,
            player_number: 2,
            aim_at_enemy: 15,
            aim_delay: 16,
            aim_delay_max: 17,
        };
    }
    for (i, a) in s.shot_avail.iter_mut().enumerate() {
        *a = i as u8;
    }
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}
