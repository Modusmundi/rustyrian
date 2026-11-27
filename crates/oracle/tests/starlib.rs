//! Differential tests for `starlib.c` / `starlib.h`.

use proptest::prelude::*;
use rustyrian_core::starlib::{self as s, JeStarType, State};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;

/// The generator states the star functions read.
fn any_state() -> impl Strategy<Value = State> {
    (
        0u8..=16,
        prop_oneof![Just(0.0f32), -5000.0f32..5000.0, Just(-0.1f32)],
        0.0f32..3.0,
        prop_oneof![any::<u16>(), Just(65534u16), Just(1u16), Just(0u16)],
        prop_oneof![Just(1i8), Just(-1i8), any::<i8>()],
        any::<bool>(),
    )
        .prop_map(|(setup, nsp, inc, nsp2, inc2, init)| State {
            setup,
            nsp,
            nsp_var_inc: inc,
            nsp2,
            nsp_var2_inc: inc2,
            temp_x: 12345,
            temp_y: -54321,
            je_starlib_init_initialized: init,
            ..State::default()
        })
}

fn run_both(
    state: State,
    seed: u64,
    c: impl FnOnce(),
    r: impl FnOnce(&mut rustyrian_core::Game),
) -> Result<(), TestCaseError> {
    let mut d = Diff::new();
    d.game.starlib = state;
    rustyrian_core::mtrand::mt_srand(&mut d.game, seed);
    d.store_to_c();
    c();
    r(&mut d.game);
    prop_assert_eq!(diff::load::<State>(), d.game.starlib.clone());
    prop_assert_eq!(
        diff::load::<rustyrian_core::mtrand::State>(),
        d.game.mtrand.clone()
    );
    prop_assert!(d.c_calls().is_empty() && d.rust_calls().is_empty());
    Ok(())
}

proptest! {
    /// Several consecutive stars, so the generators advance from each state.
    #[test]
    fn je_new_star_matches(state in any_state(), seed: u64) {
        run_both(
            state,
            seed,
            // SAFETY: plain C calls.
            || for _ in 0..20 { unsafe { b::JE_newStar() } },
            |g| for _ in 0..20 { s::je_new_star(g) },
        )?;
    }

    #[test]
    fn je_change_setup_matches(state in any_state(), setup_type in prop_oneof![Just(0u8), any::<u8>()], seed: u64) {
        run_both(
            state,
            seed,
            // SAFETY: plain C call.
            || unsafe { b::JE_changeSetup(setup_type) },
            |g| s::je_change_setup(g, setup_type),
        )?;
    }

    #[test]
    fn je_reset_values_and_wacky_col_match(state in any_state()) {
        run_both(
            state,
            0,
            // SAFETY: plain C calls.
            || unsafe { b::JE_resetValues(); b::JE_wackyCol() },
            |g| { s::je_reset_values(g); s::je_wacky_col(g) },
        )?;
    }

    #[test]
    fn je_starlib_init_matches(state in any_state(), seed: u64) {
        run_both(
            state,
            seed,
            // SAFETY: plain C calls.
            || unsafe { b::JE_starlib_init(); b::JE_starlib_init() },
            |g| { s::je_starlib_init(g); s::je_starlib_init(g) },
        )?;
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
