//! Differential tests for `varz.c` / `varz.h`.

use proptest::prelude::*;
use rustyrian_core::Game;
use rustyrian_core::lvlmast::{OPTION_NUM, POWER_NUM, SHIP_NUM, SPECIAL_NUM, WEAP_NUM};
use rustyrian_core::varz::{
    self as vz, EnemyShotType, Explosion, JeSingleEnemyType, MAX_SUPERPIXELS, RepExplosionType,
    Sprite2Ref, State, SuperpixelType,
};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;

/// Mid-level states (episode data loaded, the game screen current).
const FRAMES: [u64; 2] = [500, 2000];

/// Loads module `$m` from C and compares it with the Rust engine's.
macro_rules! same {
    ($d:expr, $m:ident) => {
        prop_assert_eq!(
            &diff::load::<rustyrian_core::$m::State>(),
            &$d.game.$m,
            "{} differs",
            stringify!($m)
        )
    };
}

/// The states these functions read or write, and the platform calls, agree.
fn check(d: &Diff) -> Result<(), TestCaseError> {
    same!(d, varz);
    same!(d, player);
    same!(d, config);
    same!(d, shots);
    same!(d, mtrand);
    same!(d, nortsong);
    same!(d, editship);
    same!(d, mainint);
    let c = diff::load::<rustyrian_core::video::State>();
    for (k, (cs, rs)) in c.screens.iter().zip(&d.game.video.screens).enumerate() {
        if let (Some(cs), Some(rs)) = (cs, rs) {
            let at = cs.pixels.iter().zip(&rs.pixels).position(|(a, b)| a != b);
            prop_assert!(
                at.is_none(),
                "screen {} differs at pixel {:?} (x {}, y {}): C {} Rust {}",
                k,
                at,
                at.unwrap_or(0) % cs.pitch as usize,
                at.unwrap_or(0) / cs.pitch as usize,
                at.map_or(0, |i| cs.pixels[i]),
                at.map_or(0, |i| rs.pixels[i])
            );
        }
    }
    prop_assert!(c == d.game.video, "video state differs");
    prop_assert_eq!(d.c_calls(), d.rust_calls());
    Ok(())
}

/// Decoded fixtures, and the frame whose whole state C last received.
type FixtureCache = (Vec<(u64, rustyrian_core::game::Snapshot)>, Option<u64>);

thread_local! {
    static FIXTURES: std::cell::RefCell<FixtureCache> =
        const { std::cell::RefCell::new((Vec::new(), None)) };
}

/// Both engines in the fixture state at `frame`, after `tweak`. The whole
/// state goes to C once per process and frame; later cases store only the
/// modules these functions read or write (reset from the fixture).
fn fixture(frame: u64, tweak: impl FnOnce(&mut Game)) -> Diff {
    let mut d = Diff::new();
    let full = FIXTURES.with(|f| {
        let mut f = f.borrow_mut();
        if !f.0.iter().any(|(k, _)| *k == frame) {
            f.0.push((frame, diff::fixture("demo-cycle", frame)));
        }
        let s =
            f.0.iter()
                .find(|(k, _)| *k == frame)
                .map(|(_, s)| s.clone());
        d.game.restore(s.expect("decoded above"));
        let full = f.1 != Some(frame);
        f.1 = Some(frame);
        full
    });
    tweak(&mut d.game);
    if full {
        d.store_to_c();
    } else {
        let g = &d.game;
        g.video.store_to_c();
        g.varz.store_to_c();
        g.player.store_to_c();
        g.config.store_to_c();
        g.shots.store_to_c();
        g.mtrand.store_to_c();
        g.nortsong.store_to_c();
        g.editship.store_to_c();
        g.mainint.store_to_c();
        g.episodes.store_to_c();
    }
    d.c_calls();
    d.rust_calls();
    d
}

/// Player state in the ranges play keeps it in: `x` is clamped to 40..=256
/// (`mainint.c:3857-3864`); shields stay at most `shield_max = 2 * mpwr` with
/// `mpwr <= 14` in every episode's shield table (`tyrian2.c:797-798`). Larger
/// shields would draw the two-player HUD bars above row 0, which
/// `JE_dBar3`/`JE_rectangle` do not check. Armor is clamped before drawing.
fn tweak_player(g: &mut Game, p: usize, v: (i32, u32, u32, u32, bool)) {
    let pl = &mut g.player.player[p];
    pl.x = 40 + v.0.rem_euclid(217);
    pl.shield_max = v.2 % 29;
    pl.shield = v.1 % (pl.shield_max + 1);
    pl.armor = v.3 % 60;
    pl.is_alive = v.4;
}

fn player_values() -> impl Strategy<Value = (i32, u32, u32, u32, bool)> {
    any::<(i32, u32, u32, u32, bool)>()
}

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

/// A ship number of the shop table or of the custom ships (`> 90`).
fn any_ship() -> impl Strategy<Value = u8> {
    prop_oneof![0u8..=SHIP_NUM as u8, 91u8..=100]
}

/// A special of type `stype` with a weapon index valid for that type (an
/// option for the sidekick specials 17 and 18, else a weapon).
fn any_special() -> impl Strategy<Value = (u8, u8, u16)> {
    (
        0u8..=19,
        prop_oneof![
            Just(0u8),
            1u8..98,
            Just(98),
            Just(99),
            100u8..=130,
            any::<u8>()
        ],
    )
        .prop_flat_map(|(stype, pwr)| {
            let wpn = if stype == 17 || stype == 18 {
                (0u16..=OPTION_NUM as u16).boxed()
            } else {
                (0u16..=WEAP_NUM as u16).boxed()
            };
            (Just(stype), Just(pwr), wpn)
        })
}

proptest! {
    /// Shop and custom ships for both players, any generator; the custom
    /// ships' graphic numbers are kept in `1..=15` (C indexes `GR[tempW-1]`,
    /// `varz.c:380`; divergence D-UB-VARZ-2 covers the rest).
    #[test]
    fn je_get_ship_info_matches(
        frame in prop::sample::select(&FRAMES[..]),
        ships in (any_ship(), any_ship()),
        generator in 0u8..=POWER_NUM as u8,
        extra in prop::collection::vec(any::<u8>(), 154),
        graphics in prop::collection::vec(1u8..=15, 10),
        still: (bool, bool),
    ) {
        let d = fixture(frame, |g| {
            g.player.player[0].items.ship = ships.0;
            g.player.player[1].items.ship = ships.1;
            g.player.player[0].items.generator = generator;
            g.editship.extra_ships.copy_from_slice(&extra);
            for (k, &gr) in graphics.iter().enumerate() {
                g.editship.extra_ships[k * 15] = gr;
            }
            // Ships without animation frames (none in the freeware data).
            for (s, still) in [(ships.0, still.0), (ships.1, still.1)] {
                if still && usize::from(s) <= SHIP_NUM {
                    g.episodes.ships[usize::from(s)].ani = 0;
                }
            }
        });
        // SAFETY: plain C call on fixture state.
        unsafe { b::JE_getShipInfo() };
        let mut d = d;
        vz::je_get_ship_info(&mut d.game);
        check(&d)?;
    }

    #[test]
    fn je_draw_options_matches(
        frame in prop::sample::select(&FRAMES[..]),
        two_player_mode: bool,
        sidekicks in (0u8..=OPTION_NUM as u8, 0u8..=OPTION_NUM as u8),
        sidekick_level in 98u8..=105,
    ) {
        let mut d = fixture(frame, |g| {
            g.config.two_player_mode = two_player_mode;
            let p = usize::from(two_player_mode);
            g.player.player[p].items.sidekick = [sidekicks.0, sidekicks.1];
            g.player.player[1].items.sidekick_level = sidekick_level;
        });
        // SAFETY: plain C call on fixture state.
        unsafe { b::JE_drawOptions() };
        vz::je_draw_options(&mut d.game);
        check(&d)?;
    }

    #[test]
    fn je_special_complete_matches(
        frame in prop::sample::select(&FRAMES[..]),
        special_type in 0u8..=SPECIAL_NUM as u8,
        sp in any_special(),
        player_num in 1u8..=2,
        temp2: u8,
        super_arcade_mode in 0u8..=9,
        flare_duration in prop_oneof![Just(0u16), 1u16..300],
        left_is_wpn: bool,
        front_power in 0u8..=11,
        players in (player_values(), player_values()),
        two_player_mode: bool,
    ) {
        let mut d = fixture(frame, |g| {
            let s = &mut g.episodes.special[usize::from(special_type)];
            (s.stype, s.pwr, s.wpn) = sp;
            g.varz.temp2 = temp2;
            g.config.super_arcade_mode = super_arcade_mode;
            g.config.two_player_mode = two_player_mode;
            g.varz.flare_duration = flare_duration;
            g.player.player[0].items.weapon[0].power = front_power;
            if left_is_wpn {
                g.player.player[0].items.sidekick[0] = sp.2 as u8;
            }
            tweak_player(g, 0, players.0);
            tweak_player(g, 1, players.1);
        });
        // SAFETY: plain C call on fixture state.
        unsafe { b::JE_specialComplete(player_num, special_type) };
        vz::je_special_complete(&mut d.game, player_num, special_type);
        check(&d)?;
    }

    /// Several frames of the special weapon: costs, flares (filters,
    /// brightness, sprays, player-linked shots), Zinglon bars, the fire
    /// button and the HUD.
    #[test]
    fn je_do_special_shot_matches(
        frame in prop::sample::select(&FRAMES[..]),
        player_num in 1u8..=2,
        p in 0usize..=1,
        specials in (0u8..=SPECIAL_NUM as u8, 0u8..=SPECIAL_NUM as u8),
        sps in (any_special(), any_special()),
        timers in (0u8..3, 0u8..3, prop_oneof![0u16..6, 0u16..300], 0u8..=50, 0u8..3),
        flags in any::<(bool, bool, bool, bool, bool, bool)>(),
        filter in (prop::sample::select(vec![-99i8, 1, 3, 7]), -1i8..9, prop::sample::select(vec![-99i8, 1, 3, 7]), prop_oneof![Just(-99i8), -8i8..=8], 0u8..4),
        buttons: [bool; 4],
        super_arcade_mode in 0u8..=9,
        players in (player_values(), player_values()),
        modes: (bool, bool),
        frames in 1usize..12,
    ) {
        let mut d = fixture(frame, |g| {
            g.player.player[0].items.special = specials.0;
            g.varz.sf_executed[usize::from(player_num) - 1] = specials.1;
            for (i, sp) in [(specials.0, sps.0), (specials.1, sps.1)] {
                let s = &mut g.episodes.special[usize::from(i)];
                (s.stype, s.pwr, s.wpn) = sp;
            }
            g.config.shot_repeat[8] = timers.0;
            g.varz.special_wait = timers.1;
            g.varz.flare_duration = timers.2;
            g.varz.zinglon_duration = timers.3;
            g.varz.astral_duration = timers.4;
            let v = &mut g.varz;
            (v.flare_start, v.link_to_player, v.spray_special, v.fire_button_held) =
                (flags.0, flags.1, flags.2, flags.3);
            g.config.filter_active = flags.4;
            g.config.you_are_cheating = flags.5;
            (v.special_weapon_filter, v.special_weapon_freq) = (filter.0, filter.1);
            (g.config.level_filter, g.config.level_brightness) = (filter.2, filter.3);
            v.next_special_wait = filter.4;
            g.mainint.button = buttons;
            g.config.super_arcade_mode = super_arcade_mode;
            (g.config.two_player_mode, g.config.galaga_mode) = modes;
            tweak_player(g, 0, players.0);
            tweak_player(g, 1, players.1);
        });
        for _ in 0..frames {
            // SAFETY: valid pointers into C's `player[p]`.
            unsafe {
                b::JE_doSpecialShot(
                    player_num,
                    &raw mut b::player[p].armor,
                    &raw mut b::player[p].shield,
                )
            };
            vz::je_do_special_shot(&mut d.game, player_num, p);
            check(&d)?;
        }
    }

    /// Explosion types of the table (`explosion_data[type]`, `varz.c:908`;
    /// divergence D-UB-VARZ-1 covers the rest) into partly used slots.
    #[test]
    fn je_setup_explosion_matches(
        frame in prop::sample::select(&FRAMES[..]),
        used in prop::collection::vec(any::<bool>(), 200),
        rep_used in prop::collection::vec(any::<bool>(), 20),
        calls in prop::collection::vec(
            (any::<i16>(), -40i16..230, any::<i16>(), prop_oneof![0i16..53, Just(98)], any::<bool>(), any::<bool>(), any::<bool>(), any::<u8>()),
            1..20,
        ),
    ) {
        let mut d = fixture(frame, |g| {
            for (e, &u) in g.varz.explosions.iter_mut().zip(&used) {
                e.ttl = if u { e.ttl.max(1) } else { 0 };
            }
            for (e, &u) in g.varz.rep_explosions.iter_mut().zip(&rep_used) {
                e.ttl = if u { e.ttl.max(1) } else { 0 };
            }
        });
        for &(x, y, delta_y, t, fixed, follow, large, explo_num) in &calls {
            if large {
                // SAFETY: plain C call on fixture state.
                unsafe { b::JE_setupExplosionLarge(fixed, explo_num, x, y) };
                vz::je_setup_explosion_large(&mut d.game, fixed, explo_num, x, y);
            } else {
                // SAFETY: as above; `t` has a table entry.
                unsafe { b::JE_setupExplosion(x, y, delta_y, t, fixed, follow) };
                vz::je_setup_explosion(&mut d.game, x, y, delta_y, t, fixed, follow);
            }
        }
        check(&d)?;
    }

    #[test]
    fn je_player_damage_matches(
        frame in prop::sample::select(&FRAMES[..]),
        temp: u8,
        p in 0usize..=1,
        players in (player_values(), player_values()),
        modes: (bool, bool, bool),
    ) {
        let mut d = fixture(frame, |g| {
            tweak_player(g, 0, players.0);
            tweak_player(g, 1, players.1);
            (g.config.two_player_mode, g.config.galaga_mode, g.config.you_are_cheating) = modes;
        });
        // SAFETY: a valid pointer to C's `player[p]`.
        let c = unsafe { b::JE_playerDamage(temp, &raw mut b::player[p]) };
        let r = vz::je_player_damage(&mut d.game, temp, p);
        prop_assert_eq!(c, r);
        check(&d)?;
    }

    #[test]
    fn hud_bars_and_port_configs_match(
        frame in prop::sample::select(&FRAMES[..]),
        players in (player_values(), player_values()),
        modes: (bool, bool),
        rear in (0u8..=42, 0u8..=42),
        which in 0usize..4,
    ) {
        let mut d = fixture(frame, |g| {
            tweak_player(g, 0, players.0);
            tweak_player(g, 1, players.1);
            (g.config.two_player_mode, g.config.galaga_mode) = modes;
            g.player.player[0].items.weapon[1].id = rear.0;
            g.player.player[1].items.weapon[1].id = rear.1;
        });
        match which {
            0 => {
                // SAFETY: plain C call on fixture state.
                unsafe { b::JE_wipeShieldArmorBars() };
                vz::je_wipe_shield_armor_bars(&mut d.game);
            }
            1 => {
                // SAFETY: as above.
                unsafe { b::JE_drawShield() };
                vz::je_draw_shield(&mut d.game);
            }
            2 => {
                // SAFETY: as above.
                unsafe { b::JE_drawArmor() };
                vz::je_draw_armor(&mut d.game);
            }
            _ => {
                // SAFETY: as above.
                let c = unsafe { b::JE_portConfigs() };
                prop_assert_eq!(c, vz::je_port_configs(&mut d.game));
            }
        }
        check(&d)?;
    }

    /// Super pixels spawned anywhere (drawing clips to the screen), then
    /// drawn over several frames.
    #[test]
    fn super_pixels_match(
        frame in prop::sample::select(&FRAMES[..]),
        spawns in prop::collection::vec((any::<u16>(), any::<u16>(), 0u16..40, any::<u8>(), any::<u8>()), 0..6),
        near in prop::collection::vec((0u16..340, 0u16..220, 0u16..40, 0u8..30, any::<u8>()), 0..6),
        last in 0u32..MAX_SUPERPIXELS as u32,
        draws in 1usize..18,
    ) {
        let mut d = fixture(frame, |g| g.varz.last_superpixel = last);
        for &(x, y, num, width, color) in spawns.iter().chain(&near) {
            // SAFETY: plain C call; `num` is small (C counts in a byte).
            unsafe { b::JE_doSP(x, y, num, width, color) };
            vz::je_do_sp(&mut d.game, x, y, num, width, color);
        }
        for _ in 0..draws {
            // SAFETY: plain C call on fixture state.
            unsafe { b::JE_drawSP() };
            vz::je_draw_sp(&mut d.game);
        }
        check(&d)?;
    }
}

/// D-UB-VARZ-1: an explosion type without a table entry sets nothing up.
#[test]
fn explosion_types_outside_the_table_set_nothing() {
    let mut d = Diff::new();
    for t in [53i16, 97, 99, 255, -1, i16::MIN] {
        vz::je_setup_explosion(&mut d.game, 10, 10, 0, t, false, false);
    }
    assert_eq!(d.game.varz, State::default());
}

/// D-UB-VARZ-2: custom-ship graphic numbers outside `1..=15`, ships past the
/// custom table, and item numbers past their tables read as zero.
#[test]
fn out_of_table_item_reads_are_zero() {
    let mut d = Diff::new();
    let mut ptr = None;
    assert_eq!(vz::je_s_gr(&mut d.game, 1, &mut ptr), 0); // graphic 0
    d.game.editship.extra_ships[0] = 16;
    assert_eq!(vz::je_s_gr(&mut d.game, 1, &mut ptr), 0);
    assert_eq!(ptr, Some(Sprite2Ref::ExtraShapes));
    assert_eq!(vz::je_s_gr(&mut d.game, 0, &mut ptr), 0);
    assert_eq!(vz::je_s_gr(&mut d.game, 200, &mut ptr), 0);
    d.game.player.player[0].items.weapon[1].id = 200;
    assert_eq!(vz::je_port_configs(&mut d.game), 0);
    d.game.player.player[0].items.ship = 255;
    d.game.player.player[1].items.ship = 60;
    d.game.player.player[0].items.generator = 99;
    vz::je_get_ship_info(&mut d.game);
    assert_eq!(d.game.player.player[0].armor, 0);
    assert_eq!(d.game.config.power_add, 0);
}

/// Each function once from the frame-500 fixture, comparing the whole game
/// state (every module), so a write outside the modules `check` loads is
/// caught too.
#[test]
fn every_function_leaves_the_whole_state_equal() {
    /// A plain C call on fixture state.
    macro_rules! c {
        ($call:expr) => {
            // SAFETY: plain C call on the test thread; pointer arguments
            // point into C's `player` array.
            unsafe { $call }
        };
    }
    type Call = (fn(), fn(&mut Game));
    let calls: [Call; 14] = [
        (|| c!(b::JE_getShipInfo()), vz::je_get_ship_info),
        (|| c!(b::JE_drawOptions()), vz::je_draw_options),
        (|| c!(b::JE_drawOptionLevel()), vz::je_draw_option_level),
        (
            || c!(b::JE_specialComplete(1, 17)),
            |g| vz::je_special_complete(g, 1, 17),
        ),
        (
            || {
                c!(b::JE_doSpecialShot(
                    1,
                    &raw mut b::player[0].armor,
                    &raw mut b::player[0].shield
                ))
            },
            |g| vz::je_do_special_shot(g, 1, 0),
        ),
        (
            || c!(b::JE_setupExplosion(100, 50, 1, 6, false, true)),
            |g| vz::je_setup_explosion(g, 100, 50, 1, 6, false, true),
        ),
        (
            || c!(b::JE_setupExplosionLarge(true, 13, 100, 50)),
            |g| vz::je_setup_explosion_large(g, true, 13, 100, 50),
        ),
        (
            || c!(b::JE_wipeShieldArmorBars()),
            vz::je_wipe_shield_armor_bars,
        ),
        (
            || {
                c!(b::JE_playerDamage(5, &raw mut b::player[0]));
            },
            |g| {
                vz::je_player_damage(g, 5, 0);
            },
        ),
        (
            || {
                c!(b::JE_portConfigs());
            },
            |g| {
                vz::je_port_configs(g);
            },
        ),
        (|| c!(b::JE_drawShield()), vz::je_draw_shield),
        (|| c!(b::JE_drawArmor()), vz::je_draw_armor),
        (
            || c!(b::JE_doSP(120, 80, 20, 9, 32)),
            |g| vz::je_do_sp(g, 120, 80, 20, 9, 32),
        ),
        (|| c!(b::JE_drawSP()), vz::je_draw_sp),
    ];
    for (k, (c, r)) in calls.into_iter().enumerate() {
        let mut d = Diff::from_fixture("demo-cycle", 500);
        d.c_calls();
        d.rust_calls();
        c();
        r(&mut d.game);
        assert!(
            d.c_game_state() == d.game.snapshot(),
            "call {k}: state differs"
        );
        assert_eq!(d.c_calls(), d.rust_calls(), "call {k}");
    }
}
