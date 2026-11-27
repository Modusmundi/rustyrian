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

mod platform_support;
use platform_support::{Both, fail_next};
use proptest::prelude::*;
use rustyrian_core::config_file::{
    self as cf, config_add_value, config_find_or_add_section, config_set_option,
};
use rustyrian_core::joystick as js;
use rustyrian_core::platform::{JoystickChange, Platform};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::mirror::video::handle_to_c;
use std::ffi::{CStr, CString, c_char, c_int};

unsafe extern "C" {
    fn joystick_axis_threshold(j: c_int, value: c_int) -> c_int;
    fn check_assigned(
        handle: *mut b::SDL_Joystick,
        assignment: *const b::Joystick_assignment,
    ) -> c_int;
    fn code_to_assignment(assignment: *mut b::Joystick_assignment, buffer: *const c_char);
    fn assignment_to_code(assignment: *const b::Joystick_assignment) -> *const c_char;
}

fn a_to_c(r: &JoystickAssignment) -> b::Joystick_assignment {
    b::Joystick_assignment {
        type_: r.r#type.to_c(),
        num: r.num,
        x_axis: r.x_axis,
        negative_axis: r.negative_axis,
    }
}

fn a_from_c(c: &b::Joystick_assignment) -> JoystickAssignment {
    JoystickAssignment {
        r#type: JoystickAssignmentTypes::from_c(c.type_).expect("valid type"),
        num: c.num,
        x_axis: c.x_axis,
        negative_axis: c.negative_axis,
    }
}

fn any_type() -> impl Strategy<Value = JoystickAssignmentTypes> {
    prop::sample::select(vec![
        JoystickAssignmentTypes::None,
        JoystickAssignmentTypes::Axis,
        JoystickAssignmentTypes::Button,
        JoystickAssignmentTypes::Hat,
    ])
}

/// An assignment; `num` mostly small (a real axis/button/hat index).
fn any_assignment() -> impl Strategy<Value = JoystickAssignment> {
    (
        any_type(),
        prop_oneof![4 => -1i32..5, 1 => any::<i32>()],
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(|(t, num, x_axis, negative_axis)| JoystickAssignment {
            r#type: t,
            num,
            x_axis,
            negative_axis,
        })
}

fn any_assignments() -> impl Strategy<Value = [[JoystickAssignment; 2]; 10]> {
    prop::array::uniform10(prop::array::uniform2(any_assignment()))
}

/// Joystick-code-like strings: the three upstream forms with variations.
fn any_code() -> impl Strategy<Value = String> {
    let ws = prop::sample::select(vec!["", " ", "  ", "\t", " \n"]);
    let word = prop::sample::select(vec!["AX", "BTN", "H", "ax", "B", "HX", "X", ""]);
    let num = prop_oneof![
        (-3i64..20).prop_map(|n| n.to_string()),
        Just("99999999999".to_owned()),
        Just("-2147483649".to_owned()),
        Just("+5".to_owned()),
        Just("-".to_owned()),
        Just("".to_owned()),
        Just("x".to_owned()),
    ];
    let tail = prop::sample::select(vec![
        "", "+", "-", "X+", "Y-", "x-", "y+", "X", " +", "--", "Z",
    ]);
    prop_oneof![
        4 => (ws.clone(), word, ws, num, tail).prop_map(|(a, w, b, n, t)| format!("{a}{w}{b}{n}{t}")),
        1 => "[ -~]{0,10}",
    ]
}

proptest! {
    #[test]
    fn code_to_assignment_matches_c(code in any_code(), prior in any_assignment()) {
        let mut c = a_to_c(&prior);
        let s = CString::new(code.clone()).unwrap();
        // SAFETY: plain C call with valid pointers.
        unsafe { code_to_assignment(&raw mut c, s.as_ptr()) };
        let mut r = prior;
        js::code_to_assignment(&mut r, code.as_bytes());
        prop_assert_eq!(a_from_c(&c), r, "{:?}", code);
    }

    #[test]
    fn assignment_to_code_matches_c(seq in prop::collection::vec(any_assignment(), 1..6)) {
        let mut d = Both::new(&[]);
        for a in seq {
            let c = a_to_c(&a);
            // SAFETY: plain C call; returns C's static buffer.
            let cs = unsafe { CStr::from_ptr(assignment_to_code(&raw const c)) }.to_bytes().to_vec();
            let r = js::assignment_to_code(&mut d.game, &a);
            let n = r.iter().position(|&b| b == 0).unwrap();
            prop_assert_eq!(&cs[..], &r[..n]);
            d.assert_same();
        }
    }

    #[test]
    fn joystick_assignment_cmp_matches_c(a in any_assignment(), bb in any_assignment(), same in 0u8..4) {
        // Copy a prefix of `a`'s fields so every `&&` operand is reached.
        let mut bb = bb;
        if same >= 1 {
            bb.r#type = a.r#type;
        }
        if same >= 2 {
            bb.num = a.num;
        }
        if same >= 3 {
            bb.x_axis = a.x_axis;
        }
        let (ca, cb) = (a_to_c(&a), a_to_c(&bb));
        // SAFETY: plain C call.
        let c = unsafe { b::joystick_assignment_cmp(&raw const ca, &raw const cb) };
        prop_assert_eq!(c, js::joystick_assignment_cmp(&a, &bb));
    }

    #[test]
    fn joystick_assignments_to_string_matches_c(
        pair in prop::array::uniform2(any_assignment()),
        len in prop_oneof![15usize..=30, 0usize..15],
        prefill: u8,
    ) {
        let mut pair = pair;
        if len < 15 {
            // Truncating anything but the last entry is undefined behaviour
            // in C (the remaining length wraps).
            pair[1].r#type = JoystickAssignmentTypes::None;
        }
        let mut d = Both::new(&[]);
        let mut cbuf = [prefill as c_char; 30];
        let mut rbuf = [prefill; 30];
        let c = pair.map(|a| a_to_c(&a));
        // SAFETY: the buffer holds 30 bytes, at least `len`.
        unsafe { b::joystick_assignments_to_string(cbuf.as_mut_ptr(), len, c.as_ptr()) };
        js::joystick_assignments_to_string(&mut d.game, &mut rbuf, len, &pair);
        prop_assert_eq!(cbuf.map(|c| c as u8), rbuf);
        d.assert_same();
    }
}

/// Two engines with `pads` virtual joysticks declared.
fn with_pads(pads: &[(i32, i32, i32)], fail: &[&str]) -> Both {
    let mut d = Both::new(fail);
    for (i, &(axes, buttons, hats)) in pads.iter().enumerate() {
        d.joystick(JoystickChange::Declare {
            name: format!("Pad {i}"),
            axes,
            buttons,
            hats,
        });
    }
    d
}

fn any_pads() -> impl Strategy<Value = Vec<(i32, i32, i32)>> {
    prop::collection::vec((0i32..4, 0i32..10, 0i32..3), 0..4)
}

/// Option key and its values.
type ConfigOptions = Vec<(String, Vec<String>)>;

/// `joystick` sections for some of the pads, with option values from codes
/// and other strings.
fn any_config() -> impl Strategy<Value = Vec<(usize, ConfigOptions)>> {
    let key = prop::sample::select(
        js::ASSIGNMENT_NAMES
            .iter()
            .chain(&["analog", "sensitivity", "threshold", "other"])
            .map(|s| (*s).to_owned())
            .collect::<Vec<_>>(),
    );
    let value = prop_oneof![
        any_code(),
        prop::sample::select(vec![
            "yes", "no", "1", "0", "on", "true", "3", "15x", "0x10", "-7"
        ])
        .prop_map(str::to_owned),
    ];
    prop::collection::vec(
        (
            0usize..4,
            prop::collection::vec((key, prop::collection::vec(value, 0..4)), 0..12),
        ),
        0..4,
    )
}

fn apply_config(d: &mut Both, config: &[(usize, ConfigOptions)]) {
    for (pad, options) in config {
        let name = format!("Pad {pad}");
        let section = config_find_or_add_section(
            &mut d.game.config.opentyrian_config,
            b"joystick",
            Some(name.as_bytes()),
        );
        for (key, values) in options {
            let mut option = config_set_option(section, key.as_bytes(), None);
            if values.len() == 1 {
                option = cf::config_set_value(option, Some(values[0].as_bytes()));
            }
            if values.len() > 1 {
                for v in values {
                    option = config_add_value(option, v.as_bytes());
                }
            }
            let _ = option;
        }
    }
}

fn init_both(d: &mut Both) {
    // SAFETY: plain C call.
    unsafe { b::init_joysticks() };
    js::init_joysticks(&mut d.game);
}

proptest! {
    #[test]
    fn init_and_deinit_joysticks_match_c(
        pads in any_pads(),
        config in any_config(),
        ignore: bool,
        failure in prop::sample::select(vec!["", "SDL_InitSubSystem", "SDL_JoystickOpen"]),
    ) {
        let mut d = with_pads(&pads, &[]);
        d.game.joystick.ignore_joystick = ignore;
        apply_config(&mut d, &config);
        d.store();
        fail_next(&mut d, failure);
        init_both(&mut d);
        d.assert_same();

        // Change some state, then save it all back.
        if let Some(v) = d.game.joystick.joystick.as_mut() {
            for (i, j) in v.iter_mut().enumerate() {
                j.analog = i % 2 == 0;
                j.sensitivity += i as i32;
            }
        }
        d.store();
        // SAFETY: plain C call.
        unsafe { b::deinit_joysticks() };
        js::deinit_joysticks(&mut d.game);
        d.assert_same();
    }

    #[test]
    fn reset_and_load_assignments_match_c(pads in any_pads(), config in any_config(), j in 0usize..4) {
        prop_assume!(j < pads.len());
        let mut d = with_pads(&pads, &[]);
        d.store();
        init_both(&mut d);
        apply_config(&mut d, &config);
        d.store();
        d.clear_calls();
        // SAFETY: plain C calls.
        unsafe { b::reset_joystick_assignments(j as c_int) };
        js::reset_joystick_assignments(&mut d.game, j as i32);
        d.assert_same();
        // SAFETY: plain C call on C's config.
        let c = unsafe {
            b::load_joystick_assignments(&raw mut b::opentyrian_config, j as c_int)
        };
        prop_assert_eq!(c, js::load_joystick_assignments(&mut d.game, j as i32));
        d.assert_same();
        // SAFETY: as above.
        let c = unsafe {
            b::save_joystick_assignments(&raw mut b::opentyrian_config, j as c_int)
        };
        prop_assert_eq!(c, js::save_joystick_assignments(&mut d.game, j as i32));
        d.assert_same();
    }

    #[test]
    fn axis_helpers_match_c(
        threshold in prop_oneof![0i32..10, any::<i32>()],
        sensitivity in prop_oneof![0i32..15, 16i32..30, any::<i32>()],
        x: i32,
        y: i32,
        value in prop_oneof![-40000i32..40000, any::<i32>()],
        angle in -10.0f32..10.0,
    ) {
        // A sensitivity of 15 divides by zero in C (D-UB-JOYSTICK-SENSITIVITY).
        prop_assume!(3000i32.wrapping_sub(200i32.wrapping_mul(sensitivity)) != 0);
        let mut d = with_pads(&[(2, 2, 1)], &[]);
        d.store();
        init_both(&mut d);
        {
            let p = &mut d.game.joystick.joystick.as_mut().unwrap()[0];
            p.threshold = threshold;
            p.sensitivity = sensitivity;
            p.x = x;
            p.y = y;
        }
        d.store();
        d.clear_calls();
        // SAFETY: plain C calls.
        unsafe {
            prop_assert_eq!(joystick_axis_threshold(0, value), js::joystick_axis_threshold(&mut d.game, 0, value));
            prop_assert_eq!(b::joystick_axis_reduce(0, value), js::joystick_axis_reduce(&mut d.game, 0, value));
            let (mut ca, mut ra) = (angle, angle);
            let c = b::joystick_analog_angle(0, &raw mut ca);
            let r = js::joystick_analog_angle(&mut d.game, 0, &mut ra);
            prop_assert_eq!((c, ca.to_bits()), (r, ra.to_bits()));
        }
        d.assert_same();
    }

    #[test]
    fn poll_and_push_joysticks_match_c(
        pads in prop::collection::vec((0i32..4, 0i32..8, 0i32..3), 1..3),
        assignments in prop::collection::vec(any_assignments(), 3),
        changes in prop::collection::vec((0i32..3, 0u8..3, 0i32..8, any::<i16>()), 0..30),
        rounds in 1usize..4,
        delays in prop::collection::vec(0u32..700, 4),
        repeat_delay in prop_oneof![Just(300i32), any::<i32>()],
        no_handle: bool,
    ) {
        let mut d = with_pads(&pads, &[]);
        d.game.joystick.joystick_repeat_delay = repeat_delay;
        d.store();
        init_both(&mut d);
        for (j, p) in d.game.joystick.joystick.as_mut().unwrap().iter_mut().enumerate() {
            p.assignment = assignments[j];
        }
        if no_handle {
            d.game.joystick.joystick.as_mut().unwrap()[0].handle = None;
        }
        d.store();
        d.clear_calls();
        let mut changes = changes.into_iter();
        for (round, &ms) in delays.iter().enumerate().take(rounds) {
            for (device, kind, index, value) in changes.by_ref().take(10) {
                d.joystick(match kind {
                    0 => JoystickChange::Axis { device, axis: index, value },
                    1 => JoystickChange::Button { device, button: index, pressed: value & 1 != 0 },
                    _ => JoystickChange::Hat { device, hat: index, value: value as u8 & 0xF },
                });
            }
            d.both(|p| p.delay(ms));
            // SAFETY: plain C calls.
            unsafe {
                if round % 2 == 0 {
                    b::push_joysticks_as_keyboard();
                    js::push_joysticks_as_keyboard(&mut d.game);
                } else {
                    b::poll_joysticks();
                    js::poll_joysticks(&mut d.game);
                }
            }
            d.assert_same();
        }
        // The pushed key events are identical too.
        let c: Vec<_> = std::iter::from_fn(|| rustyrian_oracle::session::with(|s| s.poll_event())).collect();
        let r: Vec<_> = std::iter::from_fn(|| d.game.platform.poll_event()).collect();
        prop_assert_eq!(c, r);
    }

    #[test]
    fn check_assigned_matches_c(
        pad in (0i32..4, 0i32..8, 0i32..3),
        assignment in prop::array::uniform2(any_assignment()),
        changes in prop::collection::vec((0u8..3, 0i32..8, any::<i16>()), 0..12),
    ) {
        let mut d = with_pads(&[pad], &[]);
        let h = d.both(|p| p.joystick_open(0)).expect("pad");
        for (kind, index, value) in changes {
            d.joystick(match kind {
                0 => JoystickChange::Axis { device: 0, axis: index, value },
                1 => JoystickChange::Button { device: 0, button: index, pressed: value & 1 != 0 },
                _ => JoystickChange::Hat { device: 0, hat: index, value: value as u8 & 0xF },
            });
        }
        d.both(|p| p.joystick_update());
        d.clear_calls();
        let c = assignment.map(|a| a_to_c(&a));
        // SAFETY: plain C call with a fake handle.
        let cr = unsafe { check_assigned(handle_to_c(Some(h.0)), c.as_ptr()) };
        prop_assert_eq!(cr, js::check_assigned(&mut d.game, h, &assignment));
        d.assert_same();
    }
}

/// D-UB-JOYSTICK-SENSITIVITY: a sensitivity of 15 makes C divide by zero;
/// Rust reduces to 0.
#[test]
fn axis_reduce_with_zero_divisor_is_zero() {
    let mut d = with_pads(&[(2, 2, 1)], &[]);
    js::init_joysticks(&mut d.game);
    let p = &mut d.game.joystick.joystick.as_mut().unwrap()[0];
    p.threshold = 0;
    p.sensitivity = 15;
    assert_eq!(js::joystick_axis_reduce(&mut d.game, 0, 20000), 0);
}
