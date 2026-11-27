//! Differential tests for `destruct.c`.

use rustyrian_core::destruct::{
    DestructExploS, DestructShotS, DestructUnitS, DestructWallS, MODE_NONE, State, UNIT_HELI,
};
use rustyrian_core::video::ScreenId;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

fn sample() -> State {
    let mut s = State::default();
    s.weapon_systems[3][4] = true;
    s.basetypes[9][10] = 3;
    s.default_key_config[1][7][3] = 100;
    s.destruct_temp_screen = Some(ScreenId::GameScreen);
    s.destruct_first_time = true;
    s.config.max_shots = 3;
    s.config.max_explosions = 2;
    s.config.max_walls = 4;
    s.config.max_installations = 2;
    s.config.allow_custom = true;
    s.config.ai = [false, true];
    for (p, pl) in s.destruct_player.iter_mut().enumerate() {
        pl.is_cpu = p == 1;
        pl.ai_memory.c_angle = -3;
        pl.ai_memory.c_no_down = 9;
        pl.unit = Some(
            (0..2)
                .map(|i| DestructUnitS {
                    unit_x: 10 * i + p as u32,
                    unit_y: 1.5,
                    unit_y_mov: -0.25,
                    is_y_in_air: true,
                    unit_type: UNIT_HELI,
                    shot_type: -1,
                    angle: 0.75,
                    power: 3.0,
                    last_move: -1,
                    ani_frame: 2,
                    health: 77,
                })
                .collect(),
        );
        pl.moves.actions[5] = true;
        pl.keys.config[2][1] = 42;
        pl.team = p as u32;
        pl.units_remaining = 4;
        pl.unit_selected = 1;
        pl.shot_delay = 30;
        pl.score = 12;
    }
    s.world.base_map[319] = 150;
    s.world.vga_screen = Some(ScreenId::VgaScreenSeg);
    s.world.map_walls = Some(vec![
        DestructWallS {
            wall_exist: true,
            wall_x: 5,
            wall_y: 6,
        };
        4
    ]);
    s.world.destruct_mode = MODE_NONE;
    s.world.map_flags = 0x1f;
    s.shot_rec = Some(vec![
        DestructShotS {
            is_available: true,
            x: 1.0,
            y: 2.0,
            xmov: 0.5,
            ymov: -0.5,
            gravity: true,
            shottype: 4,
            trailx: [1, 2, 3, 4],
            traily: [5, 6, 7, 8],
            trailc: [9, 10, 11, 12],
        };
        3
    ]);
    s.explo_rec = Some(vec![
        DestructExploS {
            is_available: false,
            x: 1,
            y: 2,
            explowidth: 3,
            explomax: 4,
            explofill: 5,
            explo_type: 2,
        };
        2
    ]);
    s.je_e_sound_explo_sound_channel = 3;
    s.de_run_tick_end_delay = 80;
    s
}

#[test]
fn mirror_round_trip() {
    let d = Diff::new();
    // `destructTempScreen`/`world.VGAScreen` refer to surfaces, which exist
    // after `init_video`.
    let mut video = rustyrian_core::video::State::default();
    for (i, slot) in video.screens.iter_mut().enumerate() {
        let mut surf = rustyrian_core::sdl::Surface::new_8bit(320, 200);
        surf.pixels.fill(i as u8);
        *slot = Some(surf);
    }
    video.vga_screen_seg = Some(ScreenId::VgaScreenSeg);
    video.vga_screen2 = Some(ScreenId::VgaScreen2);
    video.game_screen = Some(ScreenId::GameScreen);
    video.store_to_c();
    drop(d);

    let s = sample();
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
    State::default().store_to_c();
    assert_eq!(diff::load::<State>(), State::default());
}

/// The mirror's hand-written `#[repr(C)]` layouts against C: C writes late
/// fields of every heap and player struct (and reads `world.mapFlags`), and
/// the mirror must load exactly those writes.
#[test]
fn c_writes_land_in_mirrored_fields() {
    use rustyrian_core::destruct::{DEFAULT_CPU_WEAPON_B, UNIT_HELI};
    use rustyrian_oracle::bindings as b;
    let d = Diff::new();
    let mut video = rustyrian_core::video::State::default();
    for slot in video.screens.iter_mut() {
        *slot = Some(rustyrian_core::sdl::Surface::new_8bit(320, 200));
    }
    video.vga_screen_seg = Some(ScreenId::VgaScreenSeg);
    video.vga_screen2 = Some(ScreenId::VgaScreen2);
    video.game_screen = Some(ScreenId::GameScreen);
    video.store_to_c();
    drop(d);

    let mut s = sample();
    s.destruct_player[0].is_cpu = false;
    s.destruct_player[1].is_cpu = true;
    for sh in s.shot_rec.iter_mut().flatten() {
        sh.is_available = false;
    }
    s.store_to_c();
    // SAFETY: the statics these use are allocated and sized above.
    unsafe {
        b::ot_s_destruct_DE_ResetAI();
        b::ot_s_destruct_DE_ResetPlayers();
        b::ot_s_destruct_DE_ResetUnits();
        b::ot_s_destruct_DE_ResetWeapons();
    }

    let mut want = s.clone();
    // DE_ResetAI: player 1 is the CPU; `world.mapFlags` has MAP_WALLS.
    for u in want.destruct_player[1].unit.iter_mut().flatten() {
        assert_eq!(u.unit_type, UNIT_HELI);
        u.angle = std::f64::consts::FRAC_PI_4 as f32;
        u.power = 4.0;
        u.shot_type = DEFAULT_CPU_WEAPON_B[UNIT_HELI as usize];
    }
    for (i, p) in want.destruct_player.iter_mut().enumerate() {
        p.is_cpu = false;
        p.unit_selected = 0;
        p.shot_delay = 0;
        p.score = 0;
        p.ai_memory = Default::default();
        p.keys.config = want.default_key_config[i];
        for u in p.unit.iter_mut().flatten() {
            u.health = 0;
        }
    }
    for sh in want.shot_rec.iter_mut().flatten() {
        sh.is_available = true;
    }
    for e in want.explo_rec.iter_mut().flatten() {
        e.is_available = true;
    }
    assert_eq!(diff::load::<State>(), want);
}
