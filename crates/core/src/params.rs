// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `params.c` / `params.h`: command-line parameters.

use crate::Game;
use crate::arg_parse::{
    self, AMBIGUOUS_OPTION, INVALID_OPTION, NOT_OPTION, OPTION_MISSING_ARG, Options,
};
use crate::game::{EXIT_FAILURE, EXIT_SUCCESS, Flow, Halt};
use crate::logging::SDL_LOG_CATEGORY_APPLICATION;
use serde::{Deserialize, Serialize};

/// `pars`: legacy parameters. (YKS: Note: LOOT cheat had non letters
/// removed.)
pub const PARS: [&[u8]; 8] = [
    b"LOOT",
    b"RECORD",
    b"NOJOY",
    b"CONSTANT",
    b"DEATH",
    b"NOSOUND",
    b"NOXMAS",
    b"YESXMAS",
];

/// Mutable globals of `params.c` (rule R3).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub rich_mode: bool,
    pub constant_play: bool,
    pub constant_die: bool,
}

const fn opt(value: i32, short_opt: u8, long_opt: &'static str, has_arg: bool) -> Options {
    Options {
        value,
        short_opt,
        long_opt: Some(long_opt),
        has_arg,
    }
}

/// `JE_paramCheck`'s option table.
const OPTIONS: [Options; 16] = [
    opt(b'h' as i32, b'h', "help", false),
    opt(b's' as i32, b's', "no-sound", false),
    opt(b'j' as i32, b'j', "no-joystick", false),
    opt(b'x' as i32, b'x', "no-xmas", false),
    opt(b't' as i32, b't', "data", true),
    opt(b'n' as i32, b'n', "net", true),
    // no short codes because there should be a menu for entering these in
    // the future
    opt(256, 0, "net-player-name", true),
    opt(257, 0, "net-player-number", true),
    opt(b'p' as i32, b'p', "net-port", true),
    opt(b'd' as i32, b'd', "net-delay", true),
    opt(b'X' as i32, b'X', "xmas", false),
    opt(b'c' as i32, b'c', "constant", false),
    opt(b'k' as i32, b'k', "death", false),
    opt(b'r' as i32, b'r', "record", false),
    opt(b'l' as i32, b'l', "loot", false),
    Options {
        value: 0,
        short_opt: 0,
        long_opt: None,
        has_arg: false,
    },
];

/// C `atoi` (`(int)strtol(s, NULL, 10)`): leading whitespace, optional sign,
/// decimal digits; `strtol` saturates at the `long` range, then the result is
/// truncated to `int`.
pub fn atoi(s: &[u8]) -> i32 {
    match crate::config_file::scan_integer(s, 10, false) {
        Some((v, _)) => v as i32,
        None => 0,
    }
}

/// A message from C string bytes (`%s` copies bytes verbatim).
fn message(parts: &[&[u8]]) -> String {
    String::from_utf8_lossy(&parts.concat()).into_owned()
}

/// `JE_paramCheck(argc, argv)`; `argv` is modified in place as in C (options
/// are permuted in front of non-options, which are then upper-cased).
#[doc(alias = "JE_paramCheck")]
pub fn je_param_check(g: &mut Game, argv: &mut [Vec<u8>]) -> Flow {
    let argc = argv.len();
    let mut option;

    loop {
        option = arg_parse::parse_args(g, argv, &OPTIONS);

        if option.value == NOT_OPTION {
            break;
        }

        let arg = option.arg.clone().unwrap_or_default();
        match option.value {
            INVALID_OPTION | AMBIGUOUS_OPTION | OPTION_MISSING_ARG => {
                let m = message(&[b"Try '", &argv[0], b" --help' for more information."]);
                g.platform.log_error(SDL_LOG_CATEGORY_APPLICATION, &m);
                return Err(Halt { code: EXIT_FAILURE });
            }

            v if v == i32::from(b'h') => {
                let usage = message(&[b"Usage: ", &argv[0], b" [OPTION...]"]);
                g.platform.log(&usage);
                for line in [
                    "",
                    "Options:",
                    "  -h, --help                   Show help about options",
                    "  -s, --no-sound               Disable audio",
                    "  -j, --no-joystick            Disable joystick/gamepad input",
                    "  -x, --no-xmas                Disable Christmas mode",
                    "  -t, --data=DIR               Set Tyrian data directory",
                    "  -n, --net=HOST[:PORT]        Start a networked game",
                    "  --net-player-name=NAME       Set local player name in a networked game",
                    "  --net-player-number=NUMBER   Set local player number in a networked game",
                    "                               (1 or 2)",
                    "  -p, --net-port=PORT          Set local port to bind (default is 1333)",
                    "  -d, --net-delay=FRAMES       Set lag-compensation delay (default is 1)",
                ] {
                    g.platform.log(line);
                }
                return Err(Halt { code: EXIT_SUCCESS });
            }

            // Disables sound/music usage
            v if v == i32::from(b's') => g.loudness.audio_disabled = true,

            // Disables joystick detection
            v if v == i32::from(b'j') => g.joystick.ignore_joystick = true,

            v if v == i32::from(b'x') => g.xmas.xmas = false,

            // set custom Tyrian data directory
            v if v == i32::from(b't') => g.file.custom_data_dir_path = option.arg.clone(),

            v if v == i32::from(b'n') => {
                g.network.is_network_game = true;

                if let Some(temp) = arg.iter().position(|&c| c == b':') {
                    let temp_port = atoi(&arg[temp + 1..]);
                    if temp_port > 0 && temp_port < 49152 {
                        g.network.network_opponent_port = temp_port as u16;
                    } else {
                        let m = message(&[&argv[0], b": invalid network port number"]);
                        g.platform.log_error(SDL_LOG_CATEGORY_APPLICATION, &m);
                        return Err(Halt { code: EXIT_FAILURE });
                    }

                    g.network.network_opponent_host = Some(arg[..temp].to_vec());
                } else {
                    g.network.network_opponent_host = Some(arg.clone());
                }
            }

            // --net-player-name
            256 => g.network.network_player_name = Some(arg.clone()),

            // --net-player-number
            257 => {
                let temp = atoi(&arg);
                if (1..=2).contains(&temp) {
                    g.network.this_player_num = temp as u32;
                } else {
                    let m = message(&[&argv[0], b": invalid network player number"]);
                    g.platform.log_error(SDL_LOG_CATEGORY_APPLICATION, &m);
                    return Err(Halt { code: EXIT_FAILURE });
                }
            }

            v if v == i32::from(b'p') => {
                let temp = atoi(&arg);
                if temp > 0 && temp < 49152 {
                    g.network.network_player_port = temp as u16;
                } else {
                    let m = message(&[&argv[0], b": invalid network port number"]);
                    g.platform.log_error(SDL_LOG_CATEGORY_APPLICATION, &m);
                    return Err(Halt { code: EXIT_FAILURE });
                }
            }

            v if v == i32::from(b'd') => {
                // sscanf(arg, "%d", &temp) == 1
                match crate::config_file::scan_integer(&arg, 10, false) {
                    Some((temp, _)) => g.network.network_delay = 1i32.wrapping_add(temp as i32),
                    None => {
                        let m = message(&[&argv[0], b": invalid network delay value"]);
                        g.platform.log_error(SDL_LOG_CATEGORY_APPLICATION, &m);
                        return Err(Halt { code: EXIT_FAILURE });
                    }
                }
            }

            v if v == i32::from(b'X') => g.xmas.xmas = true,

            // Constant play for testing purposes (C key activates
            // invincibility). This might be useful for publishers to see
            // everything - especially those who can't play it
            v if v == i32::from(b'c') => g.params.constant_play = true,

            v if v == i32::from(b'k') => g.params.constant_die = true,

            v if v == i32::from(b'r') => g.demo.record_demo = true,

            // Gives you mucho bucks
            v if v == i32::from(b'l') => g.params.rich_mode = true,

            _ => debug_assert!(false, "option value outside the table"),
        }
    }

    // legacy parameter support
    for a in argv.iter_mut().take(argc).skip(option.argn.max(0) as usize) {
        // toupper in the C locale
        a.make_ascii_uppercase();

        for (j, par) in PARS.iter().enumerate() {
            if a.as_slice() == *par {
                match j {
                    0 => g.params.rich_mode = true,
                    1 => g.demo.record_demo = true,
                    2 => g.joystick.ignore_joystick = true,
                    3 => g.params.constant_play = true,
                    4 => g.params.constant_die = true,
                    5 => g.loudness.audio_disabled = true,
                    6 => g.xmas.xmas = false,
                    _ => g.xmas.xmas = true,
                }
            }
        }
    }

    Ok(())
}
