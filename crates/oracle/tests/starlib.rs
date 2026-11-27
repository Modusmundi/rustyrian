//! Differential tests for `starlib.c` / `starlib.h`.

use rustyrian_core::starlib::{JeStarType, State};
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let mut s = State {
        temp_x: -100,
        temp_y: 200,
        run: true,
        setup: 5,
        step_counter: 6,
        nsp2: 7,
        nsp_var2_inc: -8,
        nsp: 1.5,
        nsp_var_inc: -0.25,
        nsp_var_var_inc: 1e-3,
        change_time: 9,
        do_change: true,
        gray_b: true,
        starlib_speed: -10,
        speed_change: 11,
        p_color: 12,
        je_starlib_init_initialized: true,
        ..State::default()
    };
    for (i, st) in s.star.iter_mut().enumerate() {
        let i = i as i16;
        *st = JeStarType {
            sp_x: i,
            sp_y: -i,
            sp_z: i * 3,
            last_x: i ^ 0x55,
            last_y: -i * 2,
        };
    }
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}
