//! Differential tests for `joystick.c` / `joystick.h`.

use rustyrian_core::joystick::{Joystick, JoystickAssignment, JoystickAssignmentTypes, State};
use rustyrian_core::platform::JoystickId;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

fn pad(k: i32) -> Joystick {
    let mut j = Joystick {
        handle: Some(JoystickId(7 + k as u32)),
        analog: true,
        sensitivity: 5 + k,
        threshold: 3,
        x: -k,
        y: 2 * k,
        analog_direction: [1, -2, 3, -4],
        direction: [true, false, true, false],
        direction_pressed: [false, true, false, true],
        confirm: true,
        cancel: false,
        action: [true, false, false, true, true, false],
        action_pressed: [false, true, true, false, false, true],
        joystick_delay: 1234 + k as u32,
        input_pressed: true,
        ..Joystick::default()
    };
    for (i, a) in j.assignment.iter_mut().enumerate() {
        a[0] = JoystickAssignment {
            r#type: JoystickAssignmentTypes::Hat,
            num: i as i32,
            x_axis: true,
            negative_axis: false,
        };
        a[1] = JoystickAssignment {
            r#type: JoystickAssignmentTypes::Axis,
            num: -(i as i32),
            x_axis: false,
            negative_axis: true,
        };
    }
    j
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let s = State {
        joystick_repeat_delay: 123,
        joydown: true,
        ignore_joystick: true,
        joysticks: 2,
        joystick: Some(vec![pad(0), pad(1)]),
        assignment_to_code_name: *b"BT12\0\0\0",
    };
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);

    let empty = State {
        joystick: Some(vec![]),
        ..State::default()
    };
    empty.store_to_c();
    assert_eq!(diff::load::<State>(), empty);

    State::default().store_to_c();
    assert_eq!(diff::load::<State>(), State::default());
}
