//! Differential tests for `keyboard.c` / `keyboard.h`.

use rustyrian_core::keyboard::{KeyboardInput, MouseInput, State};
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
        esc_pressed: true,
        window_has_focus: true,
        lord_key_syms_down: [true, false, true, false],
        keyboard_inputs_front: 3,
        keyboard_inputs_back: 7,
        keyboard_inputs_count: 4,
        mouse_x: -12,
        mouse_y: 345,
        mouse_buttons_down: 5,
        mouse_inputs_front: 1,
        mouse_inputs_back: 2,
        mouse_inputs_count: 3,
        mouse_has_motion_input: true,
        mouse_relative_enabled: true,
        mouse_window_x_relative: -99,
        mouse_window_y_relative: 77,
        ..State::default()
    };
    for (i, k) in s.keysactive.iter_mut().enumerate() {
        *k = i % 3 == 0;
    }
    for (i, k) in s.keyboard_inputs.iter_mut().enumerate() {
        *k = KeyboardInput {
            sym: 1000 + i as i32,
            scancode: 4 + i as u16,
            r#mod: 0x40 | i as u16,
            ch: b'a' + i as u8,
        };
    }
    for (i, m) in s.mouse_inputs.iter_mut().enumerate() {
        *m = MouseInput {
            x: -(i as i32),
            y: 10 * i as i32,
            button: i as u8 + 1,
        };
    }
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}

#[test]
fn const_tables_match_c() {
    // SAFETY: C constant.
    let c = unsafe { rustyrian_oracle::bindings::lordKeySyms };
    assert_eq!(rustyrian_core::keyboard::LORD_KEY_SYMS, c);
}
