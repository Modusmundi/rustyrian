// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `arg_parse.c` / `arg_parse.h`: essentially a
//! reimplementation of `getopt_long()`.
//!
//! `argv` is `&mut [Vec<u8>]` (C permutes the pointer array in place; each
//! element is one argument's bytes without the NUL, and `argc` is its length).
//! C's `Option` type is [`Option_`] (Rust's prelude owns `Option`).

use crate::Game;
use serde::{Deserialize, Serialize};

/// Indicates that `argv[argn..argc)` are not options.
pub const NOT_OPTION: i32 = 0;
// Behaviour of `parse_args()` is undefined after it has returned any of the
// following values.
pub const INVALID_OPTION: i32 = -1;
pub const AMBIGUOUS_OPTION: i32 = -2;
pub const OPTION_MISSING_ARG: i32 = -3;

/// `Options`: one entry of an option table. A table ends with the sentinel
/// `{ 0, 0, NULL, false }` (`short_opt == 0 && long_opt == None`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub value: i32,
    /// C `char`; 0 for none.
    pub short_opt: u8,
    pub long_opt: Option<&'static str>,
    pub has_arg: bool,
}

/// C's `Option` (renamed: Rust's prelude owns `Option`). `arg` is a copy of
/// the bytes C's `const char *arg` points at (inside an `argv` string).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Option_ {
    pub value: i32,
    pub arg: Option<Vec<u8>>,
    pub argn: i32,
}

/// Mutable globals of `arg_parse.c` (rule R3): its function-local statics.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub parse_args_argn: i32,
    pub parse_args_no_more_options: bool,
    pub parse_args_first_nonopt: i32,
    pub parse_short_opt_offset: usize,
}

impl Default for State {
    fn default() -> Self {
        State {
            parse_args_argn: 1,
            parse_args_no_more_options: false,
            parse_args_first_nonopt: 1,
            parse_short_opt_offset: 1, // ignore the "-"
        }
    }
}

/// `s[i]`, reading the terminating NUL (and beyond, as 0).
fn at(s: &[u8], i: usize) -> u8 {
    s.get(i).copied().unwrap_or(0)
}

pub fn parse_args(g: &mut Game, argv: &mut [Vec<u8>], options: &[Options]) -> Option_ {
    let argc = argv.len() as i32;

    let mut option = Option_ {
        value: NOT_OPTION,
        arg: None,
        argn: 0,
    };
    option.argn = g.arg_parse.parse_args_first_nonopt;

    while g.arg_parse.parse_args_argn < argc {
        let argn = g.arg_parse.parse_args_argn;
        let arg = &argv[argn as usize];
        let arg_len = arg.len();

        if !g.arg_parse.parse_args_no_more_options && at(arg, 0) == b'-' && arg_len > 1 {
            option.argn = argn;

            if at(arg, 1) == b'-' {
                // string begins with "--"
                if arg_len == 2 {
                    // "--" alone indicates end of options
                    g.arg_parse.parse_args_argn += 1;
                    g.arg_parse.parse_args_no_more_options = true;
                } else {
                    g.arg_parse.parse_args_argn = parse_long_opt(g, argv, options, &mut option);
                }
            } else {
                g.arg_parse.parse_args_argn = parse_short_opt(g, argv, options, &mut option);
            }

            // shift option in front of non-options
            let mut first_nonopt = g.arg_parse.parse_args_first_nonopt;
            permute(
                argv,
                &mut first_nonopt,
                &mut option.argn,
                g.arg_parse.parse_args_argn,
            );
            g.arg_parse.parse_args_first_nonopt = first_nonopt;

            // don't include "--" in non-options
            if g.arg_parse.parse_args_no_more_options {
                option.argn += 1;
            }
            break;
        } else {
            // skip non-options, permute later when option encountered
            g.arg_parse.parse_args_argn += 1;
        }
    }

    option
}

pub fn permute(argv: &mut [Vec<u8>], first_nonopt: &mut i32, first_opt: &mut i32, after_opt: i32) {
    let nonopts = *first_opt - *first_nonopt;

    // slide each of the options in front of the non-options
    for i in *first_opt..after_opt {
        let mut j = i;
        while j > *first_nonopt {
            // swap argv[j] and argv[j - 1]
            argv.swap(j as usize, (j - 1) as usize);
            j -= 1;
        }

        // position of first non-option shifts right once for each option
        *first_nonopt += 1;
    }

    // position of first option is initial position of first non-option
    *first_opt -= nonopts;
}

/// Iterates a C option table up to (not including) its sentinel.
fn table(options: &[Options]) -> impl Iterator<Item = &Options> {
    options
        .iter()
        .take_while(|o| !(o.short_opt == 0 && o.long_opt.is_none()))
}

/// Formats a log message from C bytes (`%s`/`%c` copy bytes verbatim).
fn message(parts: &[&[u8]]) -> String {
    String::from_utf8_lossy(&parts.concat()).into_owned()
}

pub fn parse_short_opt(
    g: &mut Game,
    argv: &[Vec<u8>],
    options: &[Options],
    option: &mut Option_,
) -> i32 {
    let mut offset = g.arg_parse.parse_short_opt_offset;

    let mut argn = option.argn;
    let argc = argv.len() as i32;

    let arg = &argv[argn as usize];

    let arg_len = arg.len();

    let arg_attached = offset + 1 < arg_len; // possible argument attached?
    let last_in_argv = argn == argc - 1;

    option.value = INVALID_OPTION;

    for o in table(options) {
        if o.short_opt != 0 && o.short_opt == at(arg, offset) {
            option.value = o.value;

            if o.has_arg {
                if arg_attached {
                    // arg directly follows option
                    option.arg = Some(arg[offset + 1..].to_vec());

                    offset = arg_len;
                } else if !last_in_argv {
                    // arg is next in argv
                    argn += 1;
                    option.arg = Some(argv[argn as usize].clone());

                    offset = arg_len;
                } else {
                    option.value = OPTION_MISSING_ARG;
                    break;
                }
            }

            break;
        }
    }

    let c = at(&argv[option.argn as usize], offset);
    match option.value {
        INVALID_OPTION => g.platform.log_error(
            0,
            &message(&[&argv[0], b": invalid option -- '", &[c], b"'"]),
        ),
        OPTION_MISSING_ARG => g.platform.log_error(
            0,
            &message(&[&argv[0], b": option requires an argument -- '", &[c], b"'"]),
        ),
        _ => {}
    }

    offset += 1;
    if offset >= arg_len {
        argn += 1;
        offset = 1;
    }
    g.arg_parse.parse_short_opt_offset = offset;

    argn // which arg in argv that parse_args() should examine when called again
}

pub fn parse_long_opt(
    g: &mut Game,
    argv: &[Vec<u8>],
    options: &[Options],
    option: &mut Option_,
) -> i32 {
    let mut argn = option.argn;
    let argc = argv.len() as i32;

    let arg = &argv[argn as usize][2..]; // ignore the "--"

    let arg_len = arg.len();
    let arg_opt_len = ot_strchrnul(arg, b'='); // length before "="

    let arg_attached = arg_opt_len < arg_len; // argument attached using "="?
    let last_in_argv = argn == argc - 1;

    option.value = INVALID_OPTION;

    for o in table(options) {
        let Some(long_opt) = o.long_opt else {
            continue;
        };
        // strncmp(long_opt, arg, arg_opt_len) == 0: `arg` has no NUL before
        // `arg_opt_len`, so this is a prefix test.
        if long_opt.as_bytes().starts_with(&arg[..arg_opt_len]) {
            // matches (partially, at least)
            if option.value != INVALID_OPTION {
                // other match already found
                option.value = AMBIGUOUS_OPTION;
                break;
            }

            option.value = o.value;

            if o.has_arg {
                if arg_attached {
                    // arg is after "="
                    option.arg = Some(arg[arg_opt_len + 1..].to_vec());
                } else if !last_in_argv {
                    // arg is next in argv
                    argn += 1;
                    option.arg = Some(argv[argn as usize].clone());
                } else {
                    // arg is missing
                    option.value = OPTION_MISSING_ARG;
                    // can't break, gotta check for ambiguity
                }
            }

            if arg_opt_len == long_opt.len() {
                // exact match
                break;
            }
            // can't break for partial match, gotta check for ambiguity
        }
    }

    let whole = &argv[option.argn as usize];
    match option.value {
        INVALID_OPTION => g.platform.log_error(
            0,
            &message(&[&argv[0], b": unrecognized option '", whole, b"'"]),
        ),
        AMBIGUOUS_OPTION => g.platform.log_error(
            0,
            &message(&[&argv[0], b": option '", whole, b"' is ambiguous"]),
        ),
        OPTION_MISSING_ARG => g.platform.log_error(
            0,
            &message(&[&argv[0], b": option '", whole, b"' requires an argument"]),
        ),
        _ => {}
    }

    argn += 1;

    argn // which arg in argv that parse_args() should examine when called again
}

/// Returns the index of the first `c` in `s`, or of its terminating NUL.
pub fn ot_strchrnul(s: &[u8], c: u8) -> usize {
    s.iter().position(|&b| b == c || b == 0).unwrap_or(s.len())
}
