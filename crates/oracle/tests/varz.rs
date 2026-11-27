//! Differential tests for `varz.c` / `varz.h`.

use rustyrian_core::varz::{
    EnemyShotType, Explosion, JeSingleEnemyType, RepExplosionType, Sprite2Ref, State,
    SuperpixelType,
};
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

fn sample() -> State {
    let mut s = State {
        temp_dat: -1,
        temp_dat2: 2,
        temp_dat3: -3,
        lvl_file_num: 4,
        max_event: 500,
        event_loc: 12,
        level_end_warp: -2,
        end_level: true,
        quit_requested: true,
        galaga_life: -100_000,
        debug: true,
        debug_hist: 7,
        cur_loc: 333,
        total_enemy: 77,
        flash_change: -1,
        damage_rate: 2,
        option_satellite_rotate: 1.25,
        option_attachment_move: -4,
        neat: 9,
        last_superpixel: 50,
        x: 11,
        y: 12,
        b: -13,
        bkwrap1to: Some(14),
        bkwrap2to: Some(-1),
        bkwrap3to: Some(15 * 599 + 14),
        bkwrap1: Some(0),
        bkwrap2: Some(14 * 600 + 3),
        bkwrap3: None,
        special_weapon_filter: -7,
        ship_gr: 5,
        ship_gr_ptr: Some(Sprite2Ref::ExtraShapes),
        ship_gr2ptr: Some(Sprite2Ref::EnemySpriteSheets(3)),
        ..State::default()
    };
    s.sf_current_code[1][20] = 9;
    s.new_pl[9] = 1;
    s.global_flags[3] = true;
    s.map_planet = [1, 2, 3, 4, 5];
    s.sound_queue[7] = 30;
    s.enemy_avail[99] = 2;
    s.enemy_shot_avail[59] = true;
    s.mega_data1.mainmap[0][0] = Some(71);
    s.mega_data1.mainmap[299][13] = Some(0);
    s.mega_data1.shapes[71].sh[671] = 0xee;
    s.mega_data1.tempdat1 = 1;
    s.mega_data2.mainmap[599][13] = Some(70);
    s.mega_data2.shapes[3].nothing = [1, 2, 3];
    s.mega_data2.shapes[3].fill = 1;
    s.mega_data2.shapes[3].sh[5] = 7;
    s.mega_data3.mainmap[599][14] = Some(69);
    s.mega_data3.shapes[69].fill = 1;
    s.mega_data3.tempdat3 = 3;
    for (i, e) in s.enemy.iter_mut().enumerate() {
        *e = JeSingleEnemyType {
            ex: i as i16 - 50,
            eyc: -3,
            egr: [i as u16; 20],
            sprite2s: [
                None,
                Some(Sprite2Ref::SpriteSheet10),
                Some(Sprite2Ref::ShopSpriteSheet),
            ][i % 3],
            enemydatofs: (i % 2 == 0).then_some(i * 8),
            edamaged: true,
            launchtype: 1000 + i as u16,
            ymaxbounce: -9,
            fill: [1, 2, 3],
            ..JeSingleEnemyType::default()
        };
    }
    s.enemy_shot[0] = EnemyShotType {
        sx: -1,
        sym: 5,
        syc: -2,
        sgr: 300,
        fill: [9; 12],
        ..EnemyShotType::default()
    };
    s.explosions[199] = Explosion {
        ttl: 3,
        x: -4,
        y: 5,
        sprite: 6,
        follow_player: true,
        fixed_position: true,
        delta_y: -7,
    };
    s.rep_explosions[19] = RepExplosionType {
        delay: 1,
        ttl: 2,
        x: 3,
        y: 4,
        big: true,
    };
    s.superpixels[100] = SuperpixelType {
        x: 1,
        y: 2,
        z: 3,
        delta_x: -4,
        delta_y: -5,
        color: 6,
    };
    s
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let s = sample();
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}

/// Gives every number and bool leaf of `v` a value derived from its position
/// (numbers 1..=97, so they fit every field type), so that an omitted or
/// cross-wired field in the mirror changes the loaded state. `null`s
/// (pointer `Option`s) are left alone; `sample()` covers those.
fn fill(v: &mut serde_json::Value, n: &mut u64) {
    use serde_json::Value;
    match v {
        Value::Bool(b) => {
            *n += 1;
            *b = !(*n).is_multiple_of(3);
        }
        Value::Number(x) => {
            *n += 1;
            *x = (*n % 97 + 1).into();
        }
        Value::Array(a) => a.iter_mut().for_each(|e| fill(e, n)),
        Value::Object(o) => o.values_mut().for_each(|e| fill(e, n)),
        Value::Null | Value::String(_) => {}
    }
}

#[test]
fn mirror_round_trip_every_field() {
    let _d = Diff::new();
    let mut v = serde_json::to_value(sample()).unwrap();
    let mut n = 0;
    fill(&mut v, &mut n);
    let mut s: State = serde_json::from_value(v).unwrap();
    // Pointer-derived indices must name real targets; keep `sample()`'s.
    let p = sample();
    for (e, pe) in s.enemy.iter_mut().zip(p.enemy.iter()) {
        e.sprite2s = pe.sprite2s;
        e.enemydatofs = pe.enemydatofs;
    }
    s.mega_data1.mainmap = p.mega_data1.mainmap;
    s.mega_data2.mainmap = p.mega_data2.mainmap;
    s.mega_data3.mainmap = p.mega_data3.mainmap;
    (s.bkwrap1, s.bkwrap2, s.bkwrap3) = (p.bkwrap1, p.bkwrap2, p.bkwrap3);
    (s.bkwrap1to, s.bkwrap2to, s.bkwrap3to) = (p.bkwrap1to, p.bkwrap2to, p.bkwrap3to);
    (s.ship_gr_ptr, s.ship_gr2ptr) = (p.ship_gr_ptr, p.ship_gr2ptr);
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}

#[test]
fn const_tables_match_c() {
    use rustyrian_core::varz::*;
    use rustyrian_oracle::bindings as b;
    use rustyrian_oracle::mirror::global;
    // SAFETY: C constants (bindgen marks some `static mut`; never written).
    unsafe {
        assert_eq!(SA_NEXT_SHIP, b::SANextShip);
        assert_eq!(SA_SPECIAL_WEAPON, b::SASpecialWeapon);
        assert_eq!(SA_SPECIAL_WEAPON_B, b::SASpecialWeaponB);
        assert_eq!(SA_SHIP, b::SAShip);
        assert_eq!(SA_WEAPON, *global(&raw const b::SAWeapon));
        assert_eq!(SPECIAL_ARCADE_WEAPON, b::specialArcadeWeapon);
        assert_eq!(OPTION_SELECT, *global(&raw const b::optionSelect));
        assert_eq!(PGR, b::PGR);
        assert_eq!(P_ANI, b::PAni);
        assert_eq!(LINK_GUN_WEAPONS, b::linkGunWeapons);
        assert_eq!(CHARGE_GUN_WEAPONS, b::chargeGunWeapons);
        assert_eq!(RANDOM_ENEMY_LAUNCH_SOUNDS, b::randomEnemyLaunchSounds);
        assert_eq!(KEYBOARD_COMBOS, *global(&raw const b::keyboardCombos));
        assert_eq!(SHIP_COMBOS_B, b::shipCombosB);
        assert_eq!(SUPER_TYRIAN_SPECIALS, b::superTyrianSpecials);
        assert_eq!(SHIP_COMBOS, *global(&raw const b::shipCombos));
        assert_eq!(SHADOW_Y_DIST, b::shadowYDist);
    }
}
