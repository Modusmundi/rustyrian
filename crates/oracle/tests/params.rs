//! Differential tests for `params.c` / `params.h`.

use proptest::prelude::*;
use rustyrian_core::Game;
use rustyrian_core::game::Halt;
use rustyrian_core::params::{self as p, State};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;
use rustyrian_oracle::mirror::file::cstr_from_c;
use std::ffi::{CStr, CString, c_char};

/// Every global `JE_paramCheck` writes, as seen by each engine.
#[derive(Debug, PartialEq)]
struct Effects {
    params: State,
    audio_disabled: bool,
    ignore_joystick: bool,
    xmas: bool,
    record_demo: bool,
    custom_data_dir_path: Option<Vec<u8>>,
    is_network_game: bool,
    network_opponent_host: Option<Vec<u8>>,
    network_opponent_port: u16,
    network_player_name: Option<Vec<u8>>,
    network_player_port: u16,
    network_delay: i32,
    this_player_num: u32,
}

fn c_effects() -> Effects {
    // SAFETY: C runs only on this thread, inside calls we make; string
    // globals are NULL or NUL-terminated.
    unsafe {
        Effects {
            params: diff::load::<State>(),
            audio_disabled: b::audio_disabled,
            ignore_joystick: b::ignore_joystick,
            xmas: b::xmas,
            record_demo: b::recordDemo,
            custom_data_dir_path: cstr_from_c(b::customDataDirPath),
            is_network_game: b::isNetworkGame,
            network_opponent_host: cstr_from_c(b::network_opponent_host),
            network_opponent_port: b::network_opponent_port,
            network_player_name: cstr_from_c(b::network_player_name),
            network_player_port: b::network_player_port,
            network_delay: b::network_delay,
            this_player_num: b::thisPlayerNum,
        }
    }
}

fn rust_effects(g: &Game) -> Effects {
    Effects {
        params: g.params.clone(),
        audio_disabled: g.loudness.audio_disabled,
        ignore_joystick: g.joystick.ignore_joystick,
        xmas: g.xmas.xmas,
        record_demo: g.demo.record_demo,
        custom_data_dir_path: g.file.custom_data_dir_path.clone(),
        is_network_game: g.network.is_network_game,
        network_opponent_host: g.network.network_opponent_host.clone(),
        network_opponent_port: g.network.network_opponent_port,
        network_player_name: g.network.network_player_name.clone(),
        network_player_port: g.network.network_player_port,
        network_delay: g.network.network_delay,
        this_player_num: g.network.this_player_num,
    }
}

fn any_arg() -> impl Strategy<Value = String> {
    prop_oneof![
        prop::sample::select(vec![
            "-h",
            "--help",
            "-s",
            "-j",
            "-x",
            "-X",
            "-c",
            "-k",
            "-r",
            "-l",
            "-sjx",
            "--no-sound",
            "--no-joystick",
            "--no-xmas",
            "--xmas",
            "--constant",
            "--death",
            "--record",
            "--loot",
            "-t",
            "--data=/d",
            "-tdir",
            "-n",
            "--net=host",
            "--net=host:2000",
            "--net=h:0",
            "--net=h:49152",
            "--net=h:x",
            "-nhost:1",
            "--net-player-name=bob",
            "--net-player-name",
            "--net-player-number=1",
            "--net-player-number=2",
            "--net-player-number=3",
            "--net-player-number= 2x",
            "-p1333",
            "-p",
            "--net-port=0",
            "--net-port=49151",
            "--net-port=-1",
            "-d3",
            "-d",
            "--net-delay=x",
            "--net-delay= -4",
            "--net-delay=2147483647",
            "--net-",
            "--bogus",
            "-q",
            "--",
            "-",
            "loot",
            "Record",
            "nojoy",
            "constant",
            "death",
            "nosound",
            "noxmas",
            "yesxmas",
            "LOOTX",
            "file",
        ])
        .prop_map(str::to_owned),
        "[ -~]{0,6}",
    ]
}

/// Runs `JE_paramCheck` on `args` in both engines and compares everything.
fn check(args: &[String]) -> Result<Option<i32>, TestCaseError> {
    let mut d = Diff::new();
    // Earlier cases in this process changed the C globals.
    d.store_to_c();
    let mut all = vec![b"tyrian".to_vec()];
    all.extend(args.iter().map(|s| s.as_bytes().to_vec()));
    let owned: Vec<CString> = all
        .iter()
        .map(|s| CString::new(s.clone()).expect("no NUL"))
        .collect();
    let mut c_argv: Vec<*mut c_char> = owned.iter().map(|s| s.as_ptr().cast_mut()).collect();
    let argc = c_argv.len() as i32;
    let c_argv_ptr = c_argv.as_mut_ptr();
    // SAFETY: argv holds `argc` writable C strings; exit is caught.
    let c = unsafe { rustyrian_oracle::guard_call(|| b::JE_paramCheck(argc, c_argv_ptr)) };
    let mut r_argv = all;
    let r = p::je_param_check(&mut d.game, &mut r_argv);
    prop_assert_eq!(c.is_some(), r.is_err());
    if let Some(code) = c {
        prop_assert_eq!(r, Err(Halt { code }));
    }
    let c_strings: Vec<Vec<u8>> = c_argv
        .iter()
        // SAFETY: the pointers still point at `owned`.
        .map(|&s| unsafe { CStr::from_ptr(s) }.to_bytes().to_vec())
        .collect();
    prop_assert_eq!(c_strings, r_argv);
    prop_assert_eq!(c_effects(), rust_effects(&d.game));
    prop_assert_eq!(
        diff::load::<rustyrian_core::arg_parse::State>(),
        d.game.arg_parse.clone()
    );
    prop_assert_eq!(d.c_calls(), d.rust_calls());
    Ok(c)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 1024, ..ProptestConfig::default() })]

    #[test]
    fn je_param_check_matches(args in prop::collection::vec(any_arg(), 0..6)) {
        check(&args)?;
    }
}

/// Every option and exit path at least once.
#[test]
fn je_param_check_paths_match() {
    let cases: &[&[&str]] = &[
        &[],
        &["--help"],
        &["-h", "-s"],
        &["--bogus"],
        &["-t"],
        &["--net"],
        &["-s", "-j", "-x", "-c", "-k", "-r", "-l", "-X"],
        &[
            "-tdata",
            "--net=host:2000",
            "--net-player-name=me",
            "--net-player-number=2",
            "-p",
            "4000",
            "-d",
            "5",
        ],
        &["--net=hostonly"],
        &["--net=h:0"],
        &["--net-player-number=7"],
        &["--net-port=60000"],
        &["--net-delay=oops"],
        &[
            "loot", "record", "nojoy", "constant", "death", "nosound", "noxmas", "yesxmas",
        ],
        &["file", "-s", "--", "-j", "loot"],
    ];
    let mut exits = 0;
    for args in cases {
        let args: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
        if check(&args).expect("engines match").is_some() {
            exits += 1;
        }
    }
    assert!(exits >= 8);
}

#[test]
fn atoi_matches() {
    for s in [
        "",
        "12",
        " -7",
        "+3x",
        "x1",
        "99999999999",
        "-99999999999999999999",
        "0x10",
        "\t42",
    ] {
        let c = CString::new(s).expect("no NUL");
        // SAFETY: a valid C string.
        let cv = unsafe { libc::atoi(c.as_ptr()) };
        assert_eq!(cv, p::atoi(s.as_bytes()), "{s:?}");
    }
}

#[test]
fn initial_state_matches_c() {
    let d = Diff::new();
    assert_eq!(diff::load::<State>(), d.game.params);
}

#[test]
fn mirror_round_trip() {
    let mut d = Diff::new();
    d.game.params = State {
        rich_mode: true,
        constant_play: false,
        constant_die: true,
    };
    d.game.params.store_to_c();
    assert_eq!(diff::load::<State>(), d.game.params);
}
