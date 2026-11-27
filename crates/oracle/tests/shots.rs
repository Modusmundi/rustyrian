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

/// A runner capped at `cases` whatever `PROPTEST_CASES` says (as
/// `diff::capped_runner`), which also replays the cases saved in
/// `shots.proptest-regressions` first.
fn runner(cases: u32) -> proptest::test_runner::TestRunner {
    proptest::test_runner::TestRunner::new(ProptestConfig {
        cases,
        source_file: Some(file!()),
        ..ProptestConfig::default()
    })
}

#[test]
fn player_shot_set_direction_matches() {
    // Capped: each case stores the full Game and compares the full C state
    // (about 0.02 s per case).
    let strategy = (
        0i16..=MAX_PWEAPON as i16,
        prop_oneof![
            prop::sample::select(vec![10u32, 11, 14, 22, 27, 28, 30, 32, 33, 35, 38]),
            any::<u32>(),
        ],
        0.0f32..std::f32::consts::TAU,
        any::<i16>(),
        any::<i16>(),
    );
    runner(64)
        .run(&strategy, |(shot_id, weapon_id, direction, ym, yc)| {
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
            Ok(())
        })
        .expect("player_shot_set_direction matches");
}

/// From mid-level fixtures (episode item data loaded, enemies on screen)
/// with perturbed arguments and shot bookkeeping, including multi positions
/// past a weapon's `max` (`D-UB-SHOTS-1`).
#[test]
fn player_shot_create_matches() {
    // Capped: each case restores a fixture and compares the full C state
    // (about 0.35 s per case).
    let strategy = (
        prop::sample::select(vec![500u64, 2000]),
        0u16..=PORT_NUM as u16 + 1,
        0u32..11,
        any::<(u16, u16, u16, u16)>(),
        prop_oneof![0u16..=WEAP_NUM as u16 + 1, 0u16..30],
        1u8..=2,
        prop_oneof![0u32..50, any::<u32>()],
        0u8..=10,
        prop::collection::vec(prop_oneof![Just(0u8), any::<u8>()], MAX_PWEAPON),
        any::<(i8, i8, u8, u8, u8, u8)>(),
        any::<(i32, i32, i32, i32)>(),
    );
    runner(64)
        .run(
            &strategy,
            |(
                frame,
                port_num,
                bay_i,
                pos,
                wp_num,
                player_num,
                power,
                multi_pos,
                avail,
                tweak,
                deltas,
            )| {
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
                    let del = [w.del[i], 98, 99, 100, 105, 121, tweak.2][usize::from(tweak.5 % 7)];
                    if del == 121 {
                        // Trail-less shots: every position, so whichever multi
                        // position fires takes that path.
                        w.del = [121; 8];
                    } else {
                        w.del[i] = del;
                    }
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
                let c = unsafe {
                    b::player_shot_create(port_num, bay_i, px, py, mx, my, wp_num, player_num)
                };
                let r = sh::player_shot_create(
                    &mut d.game,
                    port_num,
                    bay_i,
                    px,
                    py,
                    mx,
                    my,
                    wp_num,
                    player_num,
                );
                prop_assert_eq!(c, r);
                check(&d)?;
                Ok(())
            },
        )
        .expect("player_shot_create matches");
}

/// A shot in or near the playfield, with graphics in every range the
/// function distinguishes (sheet 8, sheet 12, `% 1000`, not drawn).
fn any_shot() -> impl Strategy<Value = PlayerShotDataType> {
    (
        (
            -20i16..170,
            -20i16..200,
            -6i16..120,
            -6i16..120,
            -3i16..4,
            -3i16..4,
        ),
        (
            any::<bool>(),
            -5i16..6,
            prop_oneof![Just(1i16), Just(-1i16)],
            0i16..12,
            0i16..12,
        ),
        (
            // Not drawn: 60000 and up, without wrapping past 65535 (a wrap
            // gives frame 0; see the mapping in the test).
            prop_oneof![1u16..500, 501u16..900, 1001u16..1600, 60000u16..=65530],
            0u16..6,
            1u16..7,
        ),
    )
        .prop_map(
            |((x, y, xm, ym, xc, yc), (comp, dev, dir, cx, cy), (gr, ani, ani_max))| {
                PlayerShotDataType {
                    shot_x: x,
                    shot_y: y,
                    shot_xm: xm,
                    shot_ym: ym,
                    shot_xc: xc,
                    shot_yc: yc,
                    shot_complicated: comp,
                    shot_dev_x: dev,
                    shot_dir_x: dir,
                    shot_dev_y: -dev,
                    shot_dir_y: -dir,
                    shot_cir_size_x: cx,
                    shot_cir_size_y: cy,
                    shot_gr: gr,
                    shot_ani: ani,
                    shot_ani_max: ani_max,
                    ..PlayerShotDataType::default()
                }
            },
        )
}

/// From mid-level fixtures (sprite sheets loaded) with every shot slot
/// perturbed; the screen, shots and players must agree.
#[test]
fn simulate_player_shots_matches() {
    // Capped: each case restores a fixture and compares the screens (about
    // 0.25 s per case).
    let strategy = (
        prop::sample::select(vec![500u64, 2000]),
        prop::collection::vec(any_shot(), MAX_PWEAPON + 1),
        prop::collection::vec(prop_oneof![Just(0u8), 1u8..4, any::<u8>()], MAX_PWEAPON),
        any::<i32>(),
    );
    runner(32)
        .run(&strategy, |(frame, shots, avail, delta)| {
            let mut d = Diff::new();
            d.game.restore(diff::fixture("demo-cycle", frame));
            // Every drawn frame stays inside its sheet's offset table (304
            // entries in the freeware data): frames outside it are D-UB-SPRITE-1
            // territory (`shots.c:101-108` → `sprite.c:550`), where C's behaviour
            // is undefined, so only in-table frames are compared.
            let count = |s: &rustyrian_core::sprite::Sprite2Array| {
                let data = s.data.as_deref().expect("sheet loaded by the fixture");
                u16::from_le_bytes([data[0], data[1]]) / 2
            };
            let (n8, n12) = (
                count(&d.game.sprite.sprite_sheet8).min(500),
                count(&d.game.sprite.sprite_sheet12).min(499),
            );
            prop_assert!(n8 > 6 && n12 > 6);
            let mut shots = shots;
            for s in shots.iter_mut() {
                s.shot_gr = match s.shot_gr {
                    g @ 1..=500 => 1 + g % (n8 - 6),
                    g @ 501..=1000 => 501 + g % (n12 - 6),
                    g @ 1001..=1600 => 1001 + g % (n8 - 6),
                    g => g,
                };
            }
            d.game.shots.player_shot_data.copy_from_slice(&shots);
            d.game.shots.shot_avail.copy_from_slice(&avail);
            d.game.player.player[0].delta_y_shot_move = delta % 200;
            d.store_to_c();
            // SAFETY: plain C call on fixture state.
            unsafe { b::simulate_player_shots() };
            sh::simulate_player_shots(&mut d.game);
            check(&d)?;
            prop_assert!(
                diff::load::<rustyrian_core::video::State>() == d.game.video,
                "surfaces differ"
            );
            Ok(())
        })
        .expect("simulate_player_shots matches");
}

/// The offset-table entries of a loaded `Sprite2_array`.
fn sheet_count(s: &rustyrian_core::sprite::Sprite2Array) -> u16 {
    let d = s.data.as_deref().expect("sheet loaded by the fixture");
    u16::from_le_bytes([d[0], d[1]]) / 2
}

/// A shot graphic drawn from inside its sheet (sheet 8, sheet 12, a
/// `JE_doSP` frame above 1000, or a special `OPTION_SHAPES` frame below the
/// 151-entry table). Frames outside are D-UB-SPRITE-1 / D-UB-SHOTS-2
/// territory (`shots.c:282-304` → `sprite.c:550`).
fn graphic(kind: u8, k: u16, n8: u16, n12: u16) -> u16 {
    match kind {
        0 => 1 + k % (n8 - 6),
        1 => 501 + k % (n12 - 6),
        2 => 1001 + (k % 5) * 1000 + k % (n8 - 6),
        _ => 60001 + k % 140,
    }
}

/// One shot moved and drawn on a mid-level fixture: every movement rule
/// (`shotXM`/`shotYM` 101 and over-100 follow modes, circles), trails,
/// homing, leaving the screen, normal, shadowed and special graphics.
#[test]
fn player_shot_move_and_draw_matches() {
    // Capped: each case restores a fixture and compares the full Game (about
    // 0.8 s per case).
    let strategy = (
        prop::sample::select(vec![500u64, 2000]),
        // The last slot (MAX_PWEAPON - 1) only counts down.
        prop_oneof![0i32..MAX_PWEAPON as i32 - 1, Just(MAX_PWEAPON as i32 - 1)],
        any_shot(),
        prop::sample::select(vec![0i16, 101, 105]),
        prop_oneof![Just(255u8), Just(98u8), 0u8..53],
        (prop_oneof![Just(0u8), 1u8..=100], 1u8..4, 1u8..6),
        (0u8..3, -40i16..320, -40i16..220),
        (0u8..4, any::<u16>()),
        1u8..=2,
        (-20i32..20, -20i32..20),
        any::<bool>(),
        any::<u8>(),
    );
    runner(48)
        .run(
            &strategy,
            |(
                frame,
                id,
                shot,
                follow,
                trail,
                homing,
                target,
                gfx,
                player_num,
                deltas,
                background2,
                avail,
            )| {
                let mut d = Diff::new();
                d.game.restore(diff::fixture("demo-cycle", frame));
                let g = &mut d.game;
                let (n8, n12) = (
                    sheet_count(&g.sprite.sprite_sheet8),
                    sheet_count(&g.sprite.sprite_sheet12),
                );
                let mut s = shot;
                if follow != 0 {
                    s.shot_xm = follow;
                    s.shot_xc = 0;
                }
                s.shot_trail = trail;
                (s.aim_at_enemy, s.aim_delay, s.aim_delay_max) = homing;
                s.shot_gr = graphic(gfx.0, gfx.1, n8, n12);
                s.shot_ani = 0;
                s.player_number = player_num;
                g.shots.player_shot_data[id as usize] = s;
                g.shots.shot_avail[id as usize] = avail;
                if s.aim_at_enemy != 0 {
                    let t = usize::from(s.aim_at_enemy) - 1;
                    g.varz.enemy_avail[t] = target.0;
                    (g.varz.enemy[t].ex, g.varz.enemy[t].ey) = (target.1, target.2);
                }
                for p in g.player.player.iter_mut() {
                    (p.delta_x_shot_move, p.delta_y_shot_move) = deltas;
                }
                g.config.background2 = background2;
                d.store_to_c();

                let sentinel = sh::PlayerShotOut {
                    is_special: true,
                    shotx: -7777,
                    shoty: -7777,
                    shot_damage: -7777,
                    blast_filter: 0xA5,
                    chain: 0xA5,
                    player_num: 0xA5,
                    special_radiusw: 0xA5A5,
                    special_radiush: 0xA5A5,
                };
                let mut c = sentinel;
                // SAFETY: plain C call on fixture state; every out pointer is valid.
                let c_ret = unsafe {
                    b::player_shot_move_and_draw(
                        id,
                        &raw mut c.is_special,
                        &raw mut c.shotx,
                        &raw mut c.shoty,
                        &raw mut c.shot_damage,
                        &raw mut c.blast_filter,
                        &raw mut c.chain,
                        &raw mut c.player_num,
                        &raw mut c.special_radiusw,
                        &raw mut c.special_radiush,
                    )
                };
                let mut r = sentinel;
                let r_ret = sh::player_shot_move_and_draw(&mut d.game, id, &mut r);
                prop_assert_eq!(c_ret, r_ret);
                prop_assert_eq!(c, r);
                if let Some(diff) = diff::first_difference(&d.c_game_state(), &d.game.snapshot()) {
                    panic!("states differ at {diff}");
                }
                prop_assert_eq!(d.c_calls(), d.rust_calls());
                Ok(())
            },
        )
        .expect("player_shot_move_and_draw matches");
}

/// D-UB-SHOTS-2: a special graphic past the 151-entry `OPTION_SHAPES` table
/// (the shipped weapon data has such frames) draws nothing and has radius 0.
/// Rust only: upstream reads outside the sprite table.
#[test]
fn player_shot_move_and_draw_special_past_table_has_no_radius() {
    let mut d = Diff::new();
    d.game.restore(diff::fixture("demo-cycle", 500));
    for frame in [60001 + 151, 62720, 65535] {
        let id = 3usize;
        d.game.shots.player_shot_data[id] = PlayerShotDataType {
            shot_x: 100,
            shot_y: 100,
            shot_gr: frame,
            shot_ani_max: 2,
            shot_trail: 255,
            player_number: 1,
            ..PlayerShotDataType::default()
        };
        let before = d.game.video.clone();
        let mut out = sh::PlayerShotOut::default();
        assert!(sh::player_shot_move_and_draw(
            &mut d.game,
            id as i32,
            &mut out
        ));
        assert!(out.is_special);
        assert_eq!((out.special_radiusw, out.special_radiush), (0, 0));
        assert!(d.game.video == before, "nothing drawn");
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
