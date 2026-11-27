//! Differential tests for `mainint.c` / `mainint.h`.

use rustyrian_core::mainint::State;
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
    let s = State {
        button: [true, false, true, true],
        constant_last_x: -5,
        text_erase: 100,
        upgrade_cost: 2000,
        downgrade_cost: 1500,
        perform_save: true,
        jump_section: true,
        use_last_bank: true,
        pause_pressed: true,
        ingamemenu_pressed: true,
    };
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}
