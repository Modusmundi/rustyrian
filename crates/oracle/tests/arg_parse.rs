//! Differential tests for `arg_parse.c` / `arg_parse.h`.

use proptest::prelude::*;
use rustyrian_core::arg_parse::{self as a, Option_, Options};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;
use std::ffi::{CStr, CString, c_char};

/// An option table with short-only, long-only, argument-taking and
/// prefix-sharing entries (like `params.c`'s).
const TABLE: [(i32, u8, Option<&str>, bool); 9] = [
    (b'h' as i32, b'h', Some("help"), false),
    (b's' as i32, b's', Some("no-sound"), false),
    (b't' as i32, b't', Some("data"), true),
    (b'n' as i32, b'n', Some("net"), true),
    (256, 0, Some("net-player-name"), true),
    (257, 0, Some("net-player-number"), true),
    (b'p' as i32, b'p', Some("net-port"), true),
    (b'X' as i32, b'X', None, false),
    (0, 0, None, false),
];

fn rust_table() -> Vec<Options> {
    TABLE
        .iter()
        .map(|&(value, short_opt, long_opt, has_arg)| Options {
            value,
            short_opt,
            long_opt,
            has_arg,
        })
        .collect()
}

/// The C table; the returned strings back the `long_opt` pointers.
fn c_table() -> (Vec<b::Options>, Vec<CString>) {
    let names: Vec<CString> = TABLE
        .iter()
        .map(|t| CString::new(t.2.unwrap_or("")).expect("no NUL"))
        .collect();
    let opts = TABLE
        .iter()
        .zip(&names)
        .map(|(&(value, short_opt, long_opt, has_arg), n)| b::Options {
            value,
            short_opt: short_opt as c_char,
            long_opt: if long_opt.is_some() {
                n.as_ptr()
            } else {
                std::ptr::null()
            },
            has_arg,
        })
        .collect();
    (opts, names)
}

fn any_arg() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        prop::sample::select(vec![
            "-h",
            "-s",
            "-t",
            "-tdir",
            "-sh",
            "-hst",
            "-st",
            "-shtx",
            "-X",
            "-q",
            "-",
            "--",
            "--help",
            "--he",
            "--data",
            "--data=d",
            "--net",
            "--net-p",
            "--net-pl",
            "--net-player-name=x",
            "--net-port",
            "--n",
            "--=x",
            "--bogus",
            "--no-sound=1",
            "file",
            "LOOT",
            "-\u{e9}",
        ])
        .prop_map(|s| s.as_bytes().to_vec()),
        "[-a-z=]{0,6}".prop_map(String::into_bytes),
    ]
}

/// Strings of a C `argv`.
fn c_strings(argv: &[*const c_char]) -> Vec<Vec<u8>> {
    argv.iter()
        // SAFETY: every element points at a live CString.
        .map(|&p| unsafe { CStr::from_ptr(p) }.to_bytes().to_vec())
        .collect()
}

proptest! {
    /// Calls `parse_args` until it returns `NOT_OPTION`: every returned
    /// option, the permuted `argv`, the statics and the logged errors match.
    #[test]
    fn parse_args_matches(args in prop::collection::vec(any_arg(), 0..7)) {
        let mut d = Diff::new();
        // Earlier cases in this process changed the C statics.
        d.game.arg_parse.store_to_c();
        let mut all = vec![b"prog".to_vec()];
        all.extend(args);
        let owned: Vec<CString> = all.iter().map(|s| CString::new(s.clone()).expect("no NUL")).collect();
        let mut c_argv: Vec<*const c_char> = owned.iter().map(|s| s.as_ptr()).collect();
        let mut r_argv = all.clone();
        let (c_opts, _names) = c_table();
        let r_opts = rust_table();
        for _ in 0..32 {
            // SAFETY: `c_argv` holds `argc` valid strings; the table ends in
            // the sentinel.
            let c = unsafe { b::parse_args(c_argv.len() as i32, c_argv.as_mut_ptr(), c_opts.as_ptr()) };
            let r = a::parse_args(&mut d.game, &mut r_argv, &r_opts);
            let c = Option_ {
                value: c.value,
                // SAFETY: NULL or a pointer into an argv string.
                arg: (!c.arg.is_null()).then(|| unsafe { CStr::from_ptr(c.arg) }.to_bytes().to_vec()),
                argn: c.argn,
            };
            prop_assert_eq!(&c, &r);
            prop_assert_eq!(c_strings(&c_argv), r_argv.clone());
            prop_assert_eq!(diff::load::<a::State>(), d.game.arg_parse.clone());
            prop_assert_eq!(d.c_calls(), d.rust_calls());
            if r.value == a::NOT_OPTION {
                break;
            }
        }
    }

    #[test]
    fn permute_matches(
        n in 1usize..8,
        first_nonopt in 0i32..8,
        first_opt in 0i32..8,
        after_opt in 0i32..8,
    ) {
        let (first_nonopt, first_opt, after_opt) = {
            let mut v = [first_nonopt, first_opt, after_opt].map(|x| x.min(n as i32));
            v.sort_unstable();
            (v[0], v[1], v[2])
        };
        let owned: Vec<CString> = (0..n).map(|i| CString::new(format!("a{i}")).expect("no NUL")).collect();
        let mut c_argv: Vec<*const c_char> = owned.iter().map(|s| s.as_ptr()).collect();
        let mut r_argv: Vec<Vec<u8>> = (0..n).map(|i| format!("a{i}").into_bytes()).collect();
        let (mut cn, mut co) = (first_nonopt, first_opt);
        // SAFETY: indices stay within `c_argv`.
        unsafe { b::ot_s_arg_parse_permute(c_argv.as_mut_ptr(), &raw mut cn, &raw mut co, after_opt) };
        let (mut rn, mut ro) = (first_nonopt, first_opt);
        a::permute(&mut r_argv, &mut rn, &mut ro, after_opt);
        prop_assert_eq!((cn, co), (rn, ro));
        prop_assert_eq!(c_strings(&c_argv), r_argv);
    }

    #[test]
    fn ot_strchrnul_matches(s in "[a-c=]{0,8}", c in prop::sample::select(vec![b'=', b'a', b'z'])) {
        let cs = CString::new(s.clone()).expect("no NUL");
        // SAFETY: a valid C string.
        let p = unsafe { b::ot_s_arg_parse_ot_strchrnul(cs.as_ptr(), i32::from(c)) };
        prop_assert_eq!(p as usize - cs.as_ptr() as usize, a::ot_strchrnul(s.as_bytes(), c));
    }
}

#[test]
fn initial_state_matches_c() {
    let d = Diff::new();
    assert_eq!(diff::load::<a::State>(), d.game.arg_parse);
}

#[test]
fn mirror_round_trip() {
    let mut d = Diff::new();
    d.game.arg_parse = a::State {
        parse_args_argn: 4,
        parse_args_no_more_options: true,
        parse_args_first_nonopt: 3,
        parse_short_opt_offset: 2,
    };
    d.game.arg_parse.store_to_c();
    assert_eq!(diff::load::<a::State>(), d.game.arg_parse);
}
