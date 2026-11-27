//! Differential tests for `keyboard.c` / `keyboard.h`.

use proptest::prelude::*;
use rustyrian_core::game::Halt;
use rustyrian_core::keyboard::{
    self as kb, INPUT_ANY, INPUT_NO_MOTION, KeyboardInput, MouseInput, State,
};
use rustyrian_core::platform::{Event, Rect};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::guard_call;
use rustyrian_oracle::mirror::Mirror;

mod platform_support;
use platform_support::Both;

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
    let c = unsafe { b::lordKeySyms };
    assert_eq!(kb::LORD_KEY_SYMS, c);
}

#[test]
fn init_keyboard_matches_c() {
    let mut d = Both::new(&[]);
    // SAFETY: plain C call.
    unsafe { b::init_keyboard() };
    kb::init_keyboard(&mut d.game);
    d.assert_same();
}

/// A reachable queue state: indices in range, counts within capacity.
fn any_queues() -> impl Strategy<Value = State> {
    (
        0usize..32,
        0usize..=32,
        0usize..4,
        0usize..=4,
        any::<bool>(),
        any::<i32>(),
        any::<i32>(),
        prop::collection::vec((any::<i32>(), any::<u16>(), any::<u16>(), any::<u8>()), 32),
        prop::collection::vec((any::<i32>(), any::<i32>(), any::<u8>()), 4),
    )
        .prop_map(|(kf, kc, mf, mc, motion, mx, my, ki, mi)| {
            let mut s = State {
                keyboard_inputs_front: kf,
                keyboard_inputs_back: (kf + kc) % 32,
                keyboard_inputs_count: kc,
                mouse_inputs_front: mf,
                mouse_inputs_back: (mf + mc) % 4,
                mouse_inputs_count: mc,
                mouse_has_motion_input: motion,
                mouse_x: mx,
                mouse_y: my,
                ..State::default()
            };
            for (d, (sym, scancode, m, ch)) in s.keyboard_inputs.iter_mut().zip(ki) {
                *d = KeyboardInput {
                    sym,
                    scancode,
                    r#mod: m,
                    ch,
                };
            }
            for (d, (x, y, button)) in s.mouse_inputs.iter_mut().zip(mi) {
                *d = MouseInput { x, y, button };
            }
            s
        })
}

fn kin(c: &b::KeyboardInput) -> KeyboardInput {
    KeyboardInput {
        sym: c.sym,
        scancode: c.scancode,
        r#mod: c.mod_,
        ch: c.ch,
    }
}

fn min(c: &b::MouseInput) -> MouseInput {
    MouseInput {
        x: c.x,
        y: c.y,
        button: c.button,
    }
}

proptest! {
    #[test]
    fn input_queues_match_c(s in any_queues(), ops in prop::collection::vec(0u8..10, 1..40)) {
        let mut d = Both::new(&[]);
        d.game.keyboard = s;
        d.store();
        for op in ops {
            let flags = if op % 2 == 0 { INPUT_ANY } else { INPUT_NO_MOTION };
            // SAFETY: plain C calls with valid out-pointers.
            unsafe {
                match op {
                    0 | 1 => prop_assert_eq!(b::hasInput(flags), kb::has_input(&mut d.game, flags)),
                    2 | 3 => prop_assert_eq!(b::mouseHasInput(flags), kb::mouse_has_input(&mut d.game, flags)),
                    4 => prop_assert_eq!(b::keyboardHasInput(), kb::keyboard_has_input(&mut d.game)),
                    5 => prop_assert_eq!(b::getInput(), kb::get_input(&mut d.game)),
                    6 | 7 => {
                        let mut c = b::MouseInput::default();
                        let mut r = MouseInput::default();
                        prop_assert_eq!(
                            b::mouseGetInput(flags, &raw mut c),
                            kb::mouse_get_input(&mut d.game, flags, Some(&mut r))
                        );
                        prop_assert_eq!(min(&c), r);
                        prop_assert_eq!(
                            b::mouseGetInput(flags, std::ptr::null_mut()),
                            kb::mouse_get_input(&mut d.game, flags, None)
                        );
                    }
                    8 => {
                        let mut c = b::KeyboardInput::default();
                        let mut r = KeyboardInput::default();
                        prop_assert_eq!(
                            b::keyboardGetInput(&raw mut c),
                            kb::keyboard_get_input(&mut d.game, Some(&mut r))
                        );
                        prop_assert_eq!(kin(&c), r);
                        prop_assert_eq!(
                            b::keyboardGetInput(std::ptr::null_mut()),
                            kb::keyboard_get_input(&mut d.game, None)
                        );
                    }
                    _ => {
                        b::keyboardClearInput();
                        b::mouseClearInput();
                        kb::keyboard_clear_input(&mut d.game);
                        kb::mouse_clear_input(&mut d.game);
                    }
                }
            }
            prop_assert_eq!(&diff::load::<State>(), &d.game.keyboard);
        }
        d.assert_same();
    }

    #[test]
    fn mouse_relative_matches_c(
        enable: bool,
        focus: bool,
        rx: i32,
        ry: i32,
        rect in any_rect(),
    ) {
        let mut d = Both::new(&[]);
        prop_assert_eq!(d.init_video(), None);
        d.game.keyboard.window_has_focus = focus;
        d.game.keyboard.mouse_window_x_relative = rx;
        d.game.keyboard.mouse_window_y_relative = ry;
        d.game.video.last_output_rect = rect;
        d.store();
        d.clear_calls();
        let (mut cx, mut cy, mut sx, mut sy) = (0, 0, 0, 0);
        // SAFETY: plain C calls.
        unsafe { b::mouseGetRelativePosition(&raw mut cx, &raw mut cy) };
        kb::mouse_get_relative_position(&mut d.game, &mut sx, &mut sy);
        prop_assert_eq!((cx, cy), (sx, sy));
        d.assert_same();
        // SAFETY: plain C call.
        unsafe { b::mouseSetRelative(enable) };
        kb::mouse_set_relative(&mut d.game, enable);
        d.assert_same();
    }
}

fn any_rect() -> impl Strategy<Value = Rect> {
    (-500i32..500, -500i32..500, 1i32..3000, 1i32..3000).prop_map(|(x, y, w, h)| Rect {
        x,
        y,
        w,
        h,
    })
}

/// UTF-8 text as SDL delivers it: ASCII, CP437-mappable, other BMP and
/// non-BMP characters.
fn any_text() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            prop::char::range(' ', '~'),
            prop::sample::select(vec!['Ç', 'ü', 'é', 'ÿ', '₧', 'ƒ', '¿', 'ñ', 'Ñ', 'ª']),
            prop::sample::select(vec!['€', 'ß', 'Ω', 'ア', '\u{7FF}', '\u{800}', '\u{FFFF}']),
            prop::sample::select(vec!['😀', '\u{10000}']),
        ],
        0..20,
    )
    .prop_map(|v| v.into_iter().collect())
}

fn any_event() -> impl Strategy<Value = Event> {
    let key = (
        prop_oneof![Just(40u16), 0u16..512],
        prop_oneof![
            prop::sample::select(kb::LORD_KEY_SYMS.to_vec()),
            any::<i32>(),
            Just(13)
        ],
        prop_oneof![Just(0u16), Just(0x0100), Just(0x0200), any::<u16>()],
        any::<bool>(),
    );
    prop_oneof![
        Just(Event::WindowFocusGained),
        Just(Event::WindowFocusLost),
        (0i32..3000, 0i32..3000).prop_map(|(w, h)| Event::WindowResized { w, h }),
        key.clone()
            .prop_map(|(scancode, keycode, modifiers, repeat)| Event::KeyDown {
                scancode,
                keycode,
                modifiers,
                repeat
            }),
        key.prop_map(|(scancode, keycode, modifiers, repeat)| Event::KeyUp {
            scancode,
            keycode,
            modifiers,
            repeat
        }),
        any_text().prop_map(Event::TextInput),
        (-200i32..3000, -200i32..3000, -5i32..5, -5i32..5)
            .prop_map(|(x, y, xrel, yrel)| Event::MouseMotion { x, y, xrel, yrel }),
        (1u8..32, -200i32..3000, -200i32..3000).prop_map(|(button, x, y)| Event::MouseButtonDown {
            button,
            x,
            y
        }),
        (1u8..32, -200i32..3000, -200i32..3000).prop_map(|(button, x, y)| Event::MouseButtonUp {
            button,
            x,
            y
        }),
    ]
}

proptest! {
    #[test]
    fn handle_sdl_events_matches_c(
        events in prop::collection::vec(any_event(), 0..60),
        quit: bool,
        rect in any_rect(),
        focus: bool,
        relative: bool,
        nearly_full: bool,
        display in -1i32..1,
    ) {
        let mut d = Both::new(&[]);
        d.game.video.fullscreen_display = display;
        d.store();
        prop_assert_eq!(d.init_video(), None);
        d.game.video.last_output_rect = rect;
        d.game.keyboard.window_has_focus = focus;
        d.game.keyboard.mouse_relative_enabled = relative;
        if nearly_full {
            d.game.keyboard.keyboard_inputs_count = 30;
            d.game.keyboard.keyboard_inputs_back = 30;
            d.game.keyboard.mouse_inputs_count = 3;
            d.game.keyboard.mouse_inputs_back = 3;
        }
        d.store();
        d.clear_calls();
        for e in events {
            d.inject(e);
        }
        if quit {
            d.inject(Event::Quit);
            d.inject(Event::WindowFocusLost);
        }
        // SAFETY: plain C call; `exit` is caught by the guard.
        let c = unsafe { guard_call(|| b::handleSdlEvents()) };
        let r = kb::handle_sdl_events(&mut d.game).err().map(|Halt { code }| code);
        prop_assert_eq!(c, r);
        prop_assert_eq!(c, quit.then_some(0));
        d.assert_same();
    }
}

/// D-UB-KEYBOARD-SCANCODE: a scancode outside `keysactive` (SDL never
/// produces one) is ignored instead of written out of bounds.
#[test]
fn out_of_range_scancodes_are_ignored() {
    let mut d = Both::new(&[]);
    assert_eq!(d.init_video(), None);
    let before = d.game.keyboard.keysactive;
    for scancode in [512u16, 600, u16::MAX] {
        for e in [
            Event::KeyDown {
                scancode,
                keycode: 0,
                modifiers: 0,
                repeat: false,
            },
            Event::KeyUp {
                scancode,
                keycode: 0,
                modifiers: 0,
                repeat: false,
            },
        ] {
            d.game.platform.push_event(&e);
        }
    }
    assert_eq!(kb::handle_sdl_events(&mut d.game), Ok(()));
    assert_eq!(d.game.keyboard.keysactive, before);
    assert_eq!(d.game.keyboard.keyboard_inputs_count, 3);
}
