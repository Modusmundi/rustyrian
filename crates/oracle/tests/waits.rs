//! Differential tests for the frame and input wait loops and the functions
//! built on them: `keyboard.c` (`waitUntil*`), `palette.c` (`fade_*`),
//! `fonthand.c` (`JE_outTextGlow`), `starlib.c` (`starLibMain`), `animlib.c`
//! (`playAnim`) and `joystick.c` (`detect_joystick_assignment`).
//!
//! Both engines run the same input script on recorded fake platforms; each
//! test compares the results, every platform call and the whole game state.

use proptest::prelude::*;
use rustyrian_core::joystick::{JoystickAssignment, JoystickAssignmentTypes};
use rustyrian_core::keyboard::{self as kb, KeyboardInput};
use rustyrian_core::palette::{self, Color, Palette};
use rustyrian_core::platform::Event;
use rustyrian_core::starlib::{self, STARLIB_MAX_STARS};
use rustyrian_core::video::ScreenId;
use rustyrian_core::{animlib, fonthand, joystick, nortsong};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::mirror::video::screen_to_c;
use std::ffi::CString;

mod net_support;
use net_support::{Rig, data_dir, taps, tmp};

/// Script events (without the header) from `taps`.
fn tap_events(from_ms: u32, until_ms: u32) -> String {
    taps(from_ms, until_ms)
        .trim_start_matches("ots 1\n")
        .to_owned()
}

/// Sets the frame countdown on both engines.
fn set_frame_count(rig: &mut Rig, n: u16) {
    // SAFETY: plain C call.
    unsafe { b::setFrameCount(n) };
    nortsong::set_frame_count(&mut rig.g, n);
}

/// Initializes the network on both engines and marks the game as a network
/// game, so the wait loops poll it.
fn with_network(rig: &mut Rig) {
    rig.net();
    rig.g.network.is_network_game = true;
    rig.g.network.connected = true;
    let now = rig.both(|p| p.get_ticks());
    rig.g.network.last_in_tick = now;
    rig.g.network.network_check_keep_alive_tick = now;
    rig.store();
    rig.clear();
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 96, ..ProptestConfig::default() })]

    /// The five `waitUntil*` loops: input arriving before or after the frame
    /// period, no input, the network polled, and `SDL_QUIT` (exit).
    #[test]
    fn wait_loops_match(
        which in 0u8..5,
        frames in 0u16..6,
        input_at in prop::option::of(0u32..200),
        mouse: bool,
        quit_at in prop::option::of(0u32..300),
        network: bool,
        flags in 0u32..4,
    ) {
        let mut script = String::from("ots 1\n");
        let mut events = Vec::new();
        if let Some(t) = input_at {
            if mouse {
                events.push((t, "mouse-down 1".to_owned()));
                events.push((t + 45, "mouse-up 1".to_owned()));
            } else {
                events.push((t, "key-down SPACE".to_owned()));
                events.push((t + 45, "key-up SPACE".to_owned()));
            }
        }
        if let Some(t) = quit_at {
            events.push((t, "quit".to_owned()));
        }
        events.sort_by_key(|e| e.0);
        for (t, e) in events {
            script.push_str(&format!("@{t} {e}\n"));
        }
        script.push_str(&tap_events(1000, 60_000));
        let mut rig = Rig::new(&script);
        rig.video();
        if network {
            with_network(&mut rig);
        }
        set_frame_count(&mut rig, frames);
        rig.clear();
        let r = match which {
            // SAFETY: plain C call inside the guard.
            0 => rig.run(|| unsafe { b::waitUntilElapsed() }, kb::wait_until_elapsed).map(|()| false),
            // SAFETY: plain C call inside the guard.
            1 => rig.run(|| unsafe { b::waitUntilGetInput() }, kb::wait_until_get_input).map(|()| false),
            2 => rig
                .run(
                    // SAFETY: plain C call inside the guard.
                    || unsafe { b::waitUntilHasInput(flags as _) },
                    |g| kb::wait_until_has_input(g, flags),
                )
                .map(|()| false),
            // SAFETY: plain C call inside the guard.
            3 => rig.run(|| unsafe { b::waitUntilHasInputOrElapsed() }, kb::wait_until_has_input_or_elapsed),
            // SAFETY: plain C call inside the guard.
            _ => rig.run(|| unsafe { b::waitUntilGetInputOrElapsed() }, kb::wait_until_get_input_or_elapsed),
        };
        if let Err(code) = r {
            prop_assert_eq!(code, 0);
        }
        rig.check();
    }
}

fn any_color() -> impl Strategy<Value = Color> {
    any::<[u8; 3]>().prop_map(|[r, g, b_]| Color { r, g, b: b_, a: 0 })
}

fn any_palette() -> impl Strategy<Value = Palette> {
    prop::collection::vec(any_color(), 256).prop_map(|v| v.try_into().expect("256 colors"))
}

fn color_to_c(c: Color) -> b::SDL_Color {
    b::SDL_Color {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 32, ..ProptestConfig::default() })]

    /// `fade_palette`, `fade_solid`, `fade_black` and `fade_white` over any
    /// palettes, step counts and color ranges.
    #[test]
    fn fades_match(
        which in 0u8..4,
        current in any_palette(),
        target in any_palette(),
        solid in any_color(),
        steps in 1i32..12,
        (first, last) in (0u32..256).prop_flat_map(|f| (Just(f), f..256)),
        input_at in 0u32..400,
    ) {
        let script = format!("ots 1\n@{input_at} key-down SPACE\n@{} key-up SPACE\n", input_at + 45);
        let mut rig = Rig::new(&script);
        rig.video();
        palette::set_palette(&mut rig.g, &current, 0, 255);
        rig.store();
        rig.clear();
        let mut c_target = target.map(color_to_c);
        let r = match which {
            0 => rig.run(
                // SAFETY: plain C call inside the guard; `c_target` holds 256 colors.
                || unsafe { b::fade_palette(c_target.as_mut_ptr(), steps, first, last) },
                |g| palette::fade_palette(g, &target, steps, first, last),
            ),
            1 => rig.run(
                // SAFETY: plain C call inside the guard.
                || unsafe { b::fade_solid(color_to_c(solid), steps, first, last) },
                |g| palette::fade_solid(g, solid, steps, first, last),
            ),
            // SAFETY: plain C call inside the guard.
            2 => rig.run(|| unsafe { b::fade_black(steps) }, |g| palette::fade_black(g, steps)),
            // SAFETY: plain C call inside the guard.
            _ => rig.run(|| unsafe { b::fade_white(steps) }, |g| palette::fade_white(g, steps)),
        };
        prop_assert_eq!(r, Ok(()));
        rig.check();
    }

    /// `JE_outTextGlow`: every glow font and brightness, with and without
    /// animation, interrupted by input or not.
    #[test]
    fn je_out_text_glow_matches(
        text in "[ -~]{0,30}",
        x in -20i32..330,
        y in -10i32..200,
        font in 0u8..3,
        brightness in 0u8..15,
        warning_red: bool,
        frame_count_max in 0u16..3,
        input_at in prop::option::of(0u32..600),
    ) {
        let mut script = String::from("ots 1\n");
        if let Some(t) = input_at {
            script.push_str(&format!("@{t} key-down SPACE\n@{} key-up SPACE\n", t + 45));
        }
        let mut rig = Rig::new(&script);
        rig.video();
        rig.g.fonthand.text_glow_font = font;
        rig.g.fonthand.text_glow_brightness = brightness;
        rig.g.fonthand.warning_red = warning_red;
        rig.g.nortsong.frame_count_max = frame_count_max;
        rig.store();
        rig.clear();
        let s = CString::new(text.clone()).expect("no NUL");
        let screen = ScreenId::VgaScreen2;
        let r = rig.run(
            // SAFETY: plain C call inside the guard; a valid surface and string.
            || unsafe { b::JE_outTextGlow(screen_to_c(Some(screen)), x, y, s.as_ptr()) },
            |g| fonthand::je_out_text_glow(g, screen, x, y, text.as_bytes()),
        );
        prop_assert_eq!(r, Ok(()));
        rig.check();
    }
}

/// Keys `starLibMain` handles, plus one it ignores.
const STAR_KEYS: [u16; 20] = [
    46, 45, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 6, 22, 27, 41, 47, 48, 53, 19,
];

/// Runs `starLibMain` `calls` times on both engines; `key` (scancode, shift)
/// is queued first.
fn run_star_lib(rig: &mut Rig, key: Option<(u16, bool)>, calls: usize) {
    if let Some((scancode, shift)) = key {
        let e = Event::KeyDown {
            scancode,
            keycode: 0,
            modifiers: if shift { 0x0001 } else { 0 },
            repeat: false,
        };
        rig.both(|p| p.push_event(&e));
    }
    rig.clear();
    for _ in 0..calls {
        let mut c_in = b::KeyboardInput::default();
        let mut r_in = KeyboardInput::default();
        let r = rig.run(
            // SAFETY: plain C call inside the guard.
            || unsafe { b::starLibMain(&raw mut c_in) },
            |g| starlib::star_lib_main(g, &mut r_in),
        );
        assert!(r.is_ok());
        if r == Ok(true) {
            assert_eq!(
                (c_in.sym, c_in.scancode, c_in.mod_, c_in.ch),
                (r_in.sym, r_in.scancode, r_in.r#mod, r_in.ch)
            );
        }
    }
    rig.check();
}

/// A star-field rig whose stars all stay in range at speed 0 (so a frame
/// draws no random numbers before its own).
fn steady_stars(seed: u64) -> Rig {
    let mut rig = Rig::new(&format!("ots 1\n{}", tap_events(400, 30_000)));
    rig.video();
    rustyrian_core::mtrand::mt_srand(&mut rig.g, seed);
    let s = &mut rig.g.starlib;
    for star in s.star.iter_mut() {
        *star = starlib::JeStarType {
            sp_x: 0,
            sp_y: 0,
            sp_z: 500,
            last_x: 160,
            last_y: 100,
        };
    }
    s.nsp_var_inc = 0.1;
    s.run = true;
    rig.store();
    rig
}

/// Every key `starLibMain` handles, with and without shift.
#[test]
fn star_lib_main_every_key_matches() {
    for &scancode in &STAR_KEYS {
        for shift in [false, true] {
            let mut rig = steady_stars(u64::from(scancode));
            run_star_lib(&mut rig, Some((scancode, shift)), 1);
        }
    }
}

/// The 1-in-1000 frame that picks a new `nspVarVarInc`.
#[test]
fn star_lib_main_rare_change_matches() {
    let mut g = rustyrian_core::Game::new(Box::new(rustyrian_harness::test_session().0));
    let seed = (0u64..)
        .find(|&seed| {
            rustyrian_core::mtrand::mt_srand(&mut g, seed);
            rustyrian_core::mtrand::mt_rand(&mut g) % 1000 == 1
        })
        .expect("a seed");
    let mut rig = steady_stars(seed);
    let before = rig.g.starlib.nsp_var_var_inc;
    run_star_lib(&mut rig, None, 1);
    assert_ne!(rig.g.starlib.nsp_var_var_inc, before);
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 96, ..ProptestConfig::default() })]

    /// `starLibMain` over random star fields and settings, with every handled
    /// key (with and without shift) and none.
    #[test]
    fn star_lib_main_matches(
        stars in prop::collection::vec((any::<i16>(), any::<i16>(), 1i16..600, -5i16..330, -5i16..210), STARLIB_MAX_STARS),
        speed in -3i16..8,
        speed_change in -2i8..3,
        setup in 0u8..15,
        p_color: u8,
        do_change: bool,
        step_counter in 0u16..20,
        change_time in 0u16..20,
        key in prop::option::of((prop::sample::select(STAR_KEYS.to_vec()), any::<bool>())),
        calls in 1usize..3,
        seed: u64,
    ) {
        let mut rig = Rig::new(&format!("ots 1\n{}", tap_events(400, 30_000)));
        rig.video();
        rustyrian_core::mtrand::mt_srand(&mut rig.g, seed);
        let s = &mut rig.g.starlib;
        for (star, (sp_x, sp_y, sp_z, last_x, last_y)) in s.star.iter_mut().zip(stars) {
            *star = starlib::JeStarType { sp_x, sp_y, sp_z, last_x, last_y };
        }
        s.starlib_speed = speed;
        s.speed_change = speed_change;
        s.setup = setup;
        s.p_color = p_color;
        s.do_change = do_change;
        s.step_counter = step_counter;
        s.change_time = change_time;
        s.run = true;
        s.nsp_var_inc = 0.1;
        rig.store();
        run_star_lib(&mut rig, key, calls);
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 16, ..ProptestConfig::default() })]

    /// `playAnim` on the real animation from any starting frame, at any speed,
    /// interrupted by input or played to the end.
    #[test]
    fn play_anim_matches(start in 0u8..40, speed in 1u8..4, input_at in prop::option::of(200u32..3000)) {
        let mut script = String::from("ots 1\n");
        if let Some(t) = input_at {
            script.push_str(&format!("@{t} key-down SPACE\n@{} key-up SPACE\n", t + 45));
        }
        let mut rig = Rig::new(&script);
        rig.video();
        let name = CString::new("tyrend.anm").expect("no NUL");
        let r = rig.run(
            // SAFETY: plain C call inside the guard.
            || unsafe { b::playAnim(name.as_ptr(), start, speed) },
            |g| animlib::play_anim(g, "tyrend.anm", start, speed),
        );
        prop_assert_eq!(r, Ok(()));
        rig.check();
    }

    /// `playAnim` on truncated copies of the animation (read errors at every
    /// stage) and on a missing file.
    #[test]
    fn play_anim_damaged_files_match(cut in prop_oneof![0usize..3000, 3000usize..400_000], missing: bool) {
        let dir = tmp("anm");
        if !missing {
            let full = std::fs::read(data_dir().join("tyrend.anm")).expect("tyrend.anm");
            std::fs::write(dir.join("tyrend.anm"), &full[..cut.min(full.len())]).expect("write");
        }
        let mut rig = Rig::with_data(&format!("ots 1\n{}", tap_events(1500, 30_000)), &dir);
        // SAFETY: plain C call inside the guard.
        assert_eq!(rig.run(|| unsafe { b::init_video() }, rustyrian_core::video::init_video), Ok(()));
        rig.clear();
        let name = CString::new("tyrend.anm").expect("no NUL");
        let r = rig.run(
            // SAFETY: plain C call inside the guard.
            || unsafe { b::playAnim(name.as_ptr(), 0, 1) },
            |g| animlib::play_anim(g, "tyrend.anm", 0, 1),
        );
        prop_assert_eq!(r, Ok(()));
        rig.check();
    }
}

/// A scripted joystick event (axis, button or hat) for
/// `detect_joystick_assignment`.
#[derive(Clone, Debug)]
enum Joy {
    Axis(i32, i16),
    Button(i32, bool),
    Hat(i32, u8),
    Key,
}

fn any_joy() -> impl Strategy<Value = Joy> {
    prop_oneof![
        (
            0i32..2,
            prop_oneof![Just(-32768i16), Just(32767), -30000i16..30000]
        )
            .prop_map(|(a, v)| Joy::Axis(a, v)),
        (0i32..4, any::<bool>()).prop_map(|(n, p)| Joy::Button(n, p)),
        (
            0i32..1,
            prop::sample::select(vec![0u8, 1, 2, 3, 4, 6, 8, 9, 12])
        )
            .prop_map(|(n, v)| Joy::Hat(n, v)),
        Just(Joy::Key),
    ]
}

/// Runs `detect_joystick_assignment` on both engines: `initial` joystick
/// events at 0 ms (applied before detection), then `events` at increasing
/// times.
fn run_detect(initial: &[Joy], events: &[(Joy, u32)], network: bool) {
    let mut script = String::from("ots 1\njoystick \"Pad\" 2 4 1\n");
    let mut t = 0;
    let line = |j: &Joy, t: u32| match *j {
        Joy::Axis(a, v) => format!("@{t} joy-axis 0 {a} {v}\n"),
        Joy::Button(n, p) => format!("@{t} joy-button 0 {n} {}\n", u8::from(p)),
        Joy::Hat(n, v) => format!("@{t} joy-hat 0 {n} {v}\n"),
        Joy::Key => format!("@{t} key-down SPACE\n@{} key-up SPACE\n", t + 45),
    };
    for j in initial {
        if !matches!(j, Joy::Key) {
            script.push_str(&line(j, 0));
        }
    }
    for (j, dt) in events {
        t += dt;
        script.push_str(&line(j, t));
        t += 50;
    }
    script.push_str(&tap_events(t + 1000, t + 60_000));
    let mut rig = Rig::new(&script);
    if network {
        with_network(&mut rig);
    }
    // SAFETY: plain C call.
    unsafe { b::init_joysticks() };
    joystick::init_joysticks(&mut rig.g);
    // The joystick's starting state (events at 0 ms) before detection.
    assert_eq!(
        // SAFETY: plain C call inside the guard.
        rig.run(|| unsafe { b::handleSdlEvents() }, kb::handle_sdl_events),
        Ok(())
    );
    rig.clear();
    let mut c_a = b::Joystick_assignment::default();
    let mut r_a = JoystickAssignment::default();
    let r = rig.run(
        // SAFETY: plain C call inside the guard.
        || unsafe { b::detect_joystick_assignment(0, &raw mut c_a) },
        |g| joystick::detect_joystick_assignment(g, 0, &mut r_a),
    );
    assert!(r.is_ok());
    assert_eq!(
        (
            JoystickAssignmentTypes::from_c(c_a.type_),
            c_a.num,
            c_a.x_axis,
            c_a.negative_axis
        ),
        (Some(r_a.r#type), r_a.num, r_a.x_axis, r_a.negative_axis)
    );
    rig.check();
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    /// `detect_joystick_assignment` with axis, button and hat movements
    /// (including releases and returns to centre that it ignores) and keys
    /// that cancel it.
    #[test]
    fn detect_joystick_assignment_matches(
        initial in prop::collection::vec(any_joy(), 0..3),
        events in prop::collection::vec((any_joy(), 20u32..200), 1..6),
        network: bool,
    ) {
        run_detect(&initial, &events, network);
    }
}

/// A held button released and a held hat centred are ignored; the key then
/// cancels.
#[test]
fn detect_joystick_assignment_ignores_releases() {
    run_detect(
        &[Joy::Button(1, true), Joy::Hat(0, 1)],
        &[
            (Joy::Button(1, false), 50),
            (Joy::Hat(0, 0), 50),
            (Joy::Key, 50),
        ],
        false,
    );
}
