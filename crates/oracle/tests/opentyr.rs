//! Differential tests for `opentyr.c` / `opentyr.h`.

use proptest::prelude::*;
use rustyrian_core::opentyr::{self, State};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;
use std::ffi::{CStr, c_char};

mod net_support;
use net_support::Rig;
mod menus_support;
use menus_support::{Act, any_act, output_rect, run_capped, script};

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let s = State {
        opentyrian_str: b"Other".to_vec(),
        opentyrian_version: b"v9.9".to_vec(),
        // C has registered nothing in this process.
        atexit: Vec::new(),
    };
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
    State::default().store_to_c();
    assert_eq!(diff::load::<State>(), State::default());
}

/// The display picker's item count, with the display query succeeding and
/// failing (SDL returns a negative count).
#[test]
fn display_picker_items_count_matches() {
    for fail in [false, true] {
        let mut d = Diff::new();
        if fail {
            d.fail_after("SDL_GetNumVideoDisplays", 0);
        }
        // SAFETY: plain C call.
        let c = unsafe { b::ot_s_opentyr_getDisplayPickerItemsCount() };
        assert_eq!(
            c as usize,
            opentyr::get_display_picker_items_count(&mut d.game)
        );
        assert_eq!(d.c_calls(), d.rust_calls());
    }
}

/// C's picker item `i` into a `len`-byte buffer: the returned text and the
/// buffer.
fn c_item(
    f: unsafe extern "C" fn(u64, *mut c_char, u64) -> *const c_char,
    i: usize,
    len: usize,
) -> (Vec<u8>, Vec<u8>) {
    let mut buffer = vec![0x55 as c_char; len];
    // SAFETY: `buffer` holds `len` bytes; the result is a NUL-terminated
    // string (a literal or `buffer`).
    let text = unsafe {
        let p = f(i as u64, buffer.as_mut_ptr(), len as u64);
        CStr::from_ptr(p).to_bytes().to_vec()
    };
    (text, buffer.into_iter().map(|c| c as u8).collect())
}

proptest! {
    /// Display names ("Window", "Display N") formatted into buffers of any
    /// size, truncated as `snprintf` does.
    #[test]
    fn display_picker_item_matches(i in prop_oneof![0usize..10, any::<u32>().prop_map(|v| v as usize)], len in 1usize..24) {
        let _d = Diff::new();
        let (c_text, c_buffer) = c_item(b::ot_s_opentyr_getDisplayPickerItem, i, len);
        let mut buffer = vec![0x55u8; len];
        let r_text = opentyr::get_display_picker_item(i, &mut buffer);
        prop_assert_eq!(c_text, r_text);
        prop_assert_eq!(c_buffer, buffer);
    }
}

/// Every scaler and scaling-mode name, and both counts.
#[test]
fn scaler_and_scaling_mode_pickers_match() {
    let _d = Diff::new();
    // SAFETY: plain C calls.
    let (scalers, modes) = unsafe {
        (
            b::ot_s_opentyr_getScalerPickerItemsCount(),
            b::ot_s_opentyr_getScalingModePickerItemsCount(),
        )
    };
    assert_eq!(scalers as usize, opentyr::get_scaler_picker_items_count());
    assert_eq!(
        modes as usize,
        opentyr::get_scaling_mode_picker_items_count()
    );
    for i in 0..scalers as usize {
        let (c, _) = c_item(b::ot_s_opentyr_getScalerPickerItem, i, 4);
        assert_eq!(c, opentyr::get_scaler_picker_item(i, &mut [0; 4]));
    }
    for i in 0..modes as usize {
        let (c, _) = c_item(b::ot_s_opentyr_getScalingModePickerItem, i, 4);
        assert_eq!(c, opentyr::get_scaling_mode_picker_item(i, &mut [0; 4]));
    }
}

/// Runs `setupMenu` on both engines with `acts` (then ESC taps); compares
/// every call and the whole state, then quits through `JE_tyrianHalt` and
/// compares the saved `opentyrian.cfg`, `tyrian.cfg` and `tyrian.sav` bytes.
fn run_setup(acts: &[(Act, u32)]) {
    let mut rig = Rig::new(&script(output_rect(), 300, acts));
    rig.video();
    rig.clear();
    // SAFETY: plain C call inside the guard.
    let r = rig.run(|| unsafe { b::setupMenu() }, opentyr::setup_menu);
    assert_eq!(r, Ok(()));
    rig.check();
    let r = rig.run(
        // SAFETY: plain C call inside the guard.
        || unsafe { b::JE_tyrianHalt(0) },
        |g| rustyrian_core::varz::je_tyrian_halt(g, 0),
    );
    assert_eq!(r, Err(0));
    rig.check_halted();
}

fn keys(k: &[&'static str]) -> Vec<(Act, u32)> {
    k.iter().map(|&k| (Act::Key(k), 20)).collect()
}

/// Screen point of item `i`'s name column.
fn name_at(i: i32) -> (i32, i32) {
    (100, 43 + 21 * i)
}

/// Screen point of item `i`'s value column.
fn value_at(i: i32) -> (i32, i32) {
    (220, 43 + 21 * i)
}

/// Every scaler, scaling mode and display chosen through the keyboard
/// pickers, and a picker cancelled.
#[test]
fn setup_menu_graphics_pickers_match() {
    for k in 0..9 {
        let mut a = vec!["RETURN", "DOWN", "RETURN"];
        a.extend(std::iter::repeat_n("DOWN", k));
        a.push("RETURN");
        run_setup(&keys(&a));
    }
    for k in 0..4 {
        let mut a = vec!["RETURN", "DOWN", "DOWN", "SPACE"];
        a.extend(std::iter::repeat_n("UP", k));
        a.push("RETURN");
        run_setup(&keys(&a));
    }
    // Fullscreen on display 1, back to a window; then a cancelled picker.
    run_setup(&keys(&[
        "RETURN", "RETURN", "DOWN", "RETURN", "RETURN", "UP", "RETURN",
    ]));
    run_setup(&keys(&["RETURN", "RETURN", "ESCAPE", "UP", "RETURN"]));
}

/// Volume steps both ways (clamped at both ends), mute toggles and Done.
#[test]
fn setup_menu_sound_matches() {
    let mut a = vec![
        "DOWN", "RETURN", "LEFT", "RIGHT", "RIGHT", "RETURN", "RETURN", "DOWN",
    ];
    a.extend(std::iter::repeat_n("RIGHT", 34));
    a.extend(["RETURN", "RETURN", "LEFT", "LEFT", "DOWN", "RETURN"]);
    a.extend(std::iter::repeat_n("LEFT", 34));
    run_setup(&keys(&a));
}

/// The jukebox entry, returning to the setup menu afterwards.
#[test]
fn setup_menu_jukebox_matches() {
    run_setup(&keys(&["UP", "UP", "RETURN"]));
}

/// The mouse: names and values clicked, volume bars clicked at several
/// positions, picker items hovered and clicked, right clicks backing out.
#[test]
fn setup_menu_mouse_matches() {
    let at = |(x, y): (i32, i32)| (Act::Move(x, y), 20);
    let click = |b: u8| (Act::Click(b), 20);
    // Graphics by name, the display picker by value, its second item.
    run_setup(&[
        at(name_at(0)),
        click(1),
        at(value_at(0)),
        click(1),
        at((200, 43 + 15)),
        at((200, 43)),
        at((200, 43 + 15)),
        click(1),
        at(value_at(1)),
        click(1),
        at((200, 43 + 15)),
        click(3),
        click(3),
    ]);
    // Sound, then the volume bars at the left end, middle and right end.
    let mut a = vec![at(name_at(1)), click(1)];
    for (i, x) in [(0, 180), (0, 230), (0, 274), (1, 181), (1, 274), (1, 200)] {
        a.push(at((x, 43 + 21 * i)));
        a.push(click(1));
    }
    a.extend([at(value_at(2)), click(1), at(name_at(2)), click(1)]);
    run_setup(&a);
}

/// Random keyboard and mouse sessions through every menu. Each case runs the
/// whole setup menu (and possibly the jukebox) on both engines, so the case
/// count is capped at 32 whatever `PROPTEST_CASES` says.
#[test]
fn setup_menu_matches() {
    let targets = (0..4)
        .flat_map(|i| [name_at(i), value_at(i), (200, 43 + 15 * i)])
        .collect();
    let keys = [
        "UP", "DOWN", "LEFT", "RIGHT", "RETURN", "SPACE", "ESCAPE", "RETURN", "DOWN",
    ]
    .map(Act::Key)
    .to_vec();
    let s = prop::collection::vec((any_act(targets, keys), 20u32..200), 0..14);
    run_capped(32, s, |acts| {
        run_setup(&acts);
        Ok(())
    });
}

/// How a run of `main` ended on both engines.
#[derive(Debug, PartialEq)]
enum MainEnd {
    Returned(i32),
    Exited(i32),
}

/// Runs upstream `main` (C) and `opentyr::main` (Rust) on `argv` with the
/// input `script`; compares the outcome, then runs the `atexit` handlers on
/// both sides and compares every platform call and the state (the files
/// written too, when the run quit through `JE_tyrianHalt`).
fn run_main(argv: &[&str], script: &str) -> MainEnd {
    run_main_with(argv, script, |_| {})
}

/// [`run_main`] after `setup` prepared both engines (injected failures).
fn run_main_with(argv: &[&str], script: &str, setup: impl FnOnce(&mut Rig)) -> MainEnd {
    use std::ffi::CString;
    let mut rig = Rig::new(script);
    setup(&mut rig);
    rig.clear();
    let c_args: Vec<CString> = argv
        .iter()
        .map(|a| CString::new(*a).expect("no NUL"))
        .collect();
    let mut ptrs: Vec<*mut c_char> = c_args.iter().map(|c| c.as_ptr().cast_mut()).collect();
    ptrs.push(std::ptr::null_mut());
    let argc = c_args.len() as i32;
    let r_args: Vec<String> = argv.iter().map(|a| (*a).to_owned()).collect();
    let r = rig.run(
        // SAFETY: argv is a NULL-terminated array of NUL-terminated strings
        // that outlive the call (inside the guard).
        || unsafe { rustyrian_oracle::ot_c_main(argc, ptrs.as_mut_ptr()) },
        |g| opentyr::main(g, &r_args),
    );
    let end = match r {
        Ok(code) => MainEnd::Returned(code),
        Err(code) => MainEnd::Exited(code),
    };
    let code = match end {
        MainEnd::Returned(c) | MainEnd::Exited(c) => c,
    };
    // The atexit handlers (SDL_Quit) run on both sides, as at process exit.
    rustyrian_oracle::session::run_exit(code);
    opentyr::run_atexit(&mut rig.g);
    let halted =
        rig.g.video.screens.iter().all(Option::is_none) && matches!(end, MainEnd::Exited(0));
    if halted {
        rig.check_halted();
    } else {
        rig.check();
    }
    end
}

/// `--help` prints upstream's help and exits 0.
#[test]
fn main_help_matches() {
    assert_eq!(
        run_main(&["opentyrian", "--help"], "ots 1\n"),
        MainEnd::Exited(0)
    );
}

/// An unknown option exits with EXIT_FAILURE.
#[test]
fn main_bad_argument_matches() {
    assert_eq!(
        run_main(&["opentyrian", "--no-such-option"], "ots 1\n"),
        MainEnd::Exited(1)
    );
}

/// No data files: `main` returns EXIT_FAILURE.
#[test]
fn main_missing_data_matches() {
    let dir = std::env::temp_dir().join(format!("rustyrian-nodata-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let arg = format!("--data={}", dir.display());
    assert_eq!(
        run_main(&["opentyrian", &arg], "ots 1\n"),
        MainEnd::Returned(1)
    );
}

/// The logos, the title screen and quitting from it: `main` ends through
/// `JE_tyrianHalt(0)`.
#[test]
fn main_logos_title_quit_matches() {
    let data = net_support::data_dir();
    let arg = format!("--data={}", data.display());
    let mut s = String::from("ots 1\n");
    let mut t = 500;
    while t < 120_000 {
        s.push_str(&format!(
            "@{t} key-down ESCAPE\n@{} key-up ESCAPE\n",
            t + 40
        ));
        t += 700;
    }
    assert_eq!(run_main(&["opentyrian", &arg], &s), MainEnd::Exited(0));
}

/// ESC taps every 0.7 s for two minutes: enough to leave the logos and quit
/// from the title screen.
fn quit_script() -> String {
    let mut s = String::from("ots 1\n");
    let mut t = 500;
    while t < 120_000 {
        s.push_str(&format!(
            "@{t} key-down ESCAPE\n@{} key-up ESCAPE\n",
            t + 40
        ));
        t += 700;
    }
    s
}

/// `SDL_Init` failing: `main` logs and returns EXIT_FAILURE.
#[test]
fn main_sdl_init_failure_matches() {
    let end = run_main_with(&["opentyrian"], "ots 1\n", |rig| {
        rig.c_fake.fail_next("SDL_Init");
        rig.r_fake.fail_next("SDL_Init");
    });
    assert_eq!(end, MainEnd::Returned(1));
}

/// Data directories holding the v1.0/v1.1 or Tyrian 2000 `tyrian.shp`
/// (identified by its first word: 11 or 13) are refused. The rest of the
/// freeware files are present, so `findDataFiles` succeeds.
#[test]
fn main_old_data_versions_match() {
    for (version, name) in [(11u16, "v10"), (13, "t2000")] {
        let dir = data_without("tyrian.shp");
        std::fs::write(dir.join("tyrian.shp"), version.to_le_bytes()).expect("write");
        let arg = format!("--data={}", dir.display());
        assert_eq!(
            run_main(&["opentyrian", &arg], "ots 1\n"),
            MainEnd::Returned(1),
            "{name}"
        );
    }
}

/// `--no-sound`, `--record` and `--xmas` (Christmas data missing from the
/// freeware set), then logos, title and quit.
#[test]
fn main_options_logos_title_quit_match() {
    let data = net_support::data_dir();
    let arg = format!("--data={}", data.display());
    let end = run_main(
        &["opentyrian", &arg, "--no-sound", "--record", "--xmas"],
        &quit_script(),
    );
    assert_eq!(end, MainEnd::Exited(0));
}

/// The freeware data without `skip`, as symlinks in a scratch directory.
fn data_without(skip: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("rustyrian-no-{skip}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    for e in std::fs::read_dir(net_support::data_dir()).expect("data dir") {
        let e = e.expect("entry");
        if e.file_name() != skip {
            std::os::unix::fs::symlink(e.path(), dir.join(e.file_name())).expect("symlink");
        }
    }
    dir
}

/// `--xmas` without the Christmas data: the mode is dropped with a warning.
#[test]
fn main_xmas_missing_matches() {
    let arg = format!("--data={}", data_without("tyrianc.shp").display());
    let end = run_main(&["opentyrian", &arg, "--xmas"], &quit_script());
    assert_eq!(end, MainEnd::Exited(0));
}

/// A network game whose delay overflows the packet queue: `network_init`
/// fails and the game ends through `network_tyrian_halt(3)`.
#[test]
fn main_network_init_failure_matches() {
    let data = net_support::data_dir();
    let arg = format!("--data={}", data.display());
    let end = run_main(
        &["opentyrian", &arg, "--net=127.0.0.1", "--net-delay=9"],
        &quit_script(),
    );
    assert_eq!(end, MainEnd::Exited(0));
}
