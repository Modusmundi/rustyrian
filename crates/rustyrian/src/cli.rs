// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) rustyrian contributors

//! The Rust-only command-line flags of the shipped binary (plan Step 11).
//! Everything else is passed through unchanged to upstream's own argument
//! handling (`--data`, `--net`, `--no-sound`, …).

use std::fmt;
use std::path::PathBuf;

/// Help for the Rust-only flags, printed before upstream's `--help` text.
pub const HELP: &str = "\
rustyrian options (in addition to OpenTyrian's, listed below):
  --headless              run without a window or audio device (fake platform)
  --clock virtual|real    headless clock (default: virtual)
  --userdir DIR           keep configuration and saves in DIR
  --input-script FILE     inject input from an .ots script
  --trace-out FILE        write a per-frame .jsonl trace
  --log-out FILE          write every platform call as .jsonl
  --frames N              stop after N presented frames
  --until-ms T            stop when the session clock reaches T ms
  --dump-frames DIR       write presented frames as PNG to DIR
  --dump-every K          with --dump-frames: every K-th frame (default 1)
  --perf-log FILE         per-frame simulation/render CPU time (µs) as CSV
";

/// Parsed command line.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub headless: bool,
    pub clock_real: bool,
    pub userdir: Option<PathBuf>,
    pub input_script: Option<PathBuf>,
    pub trace_out: Option<PathBuf>,
    pub log_out: Option<PathBuf>,
    pub frames: Option<u64>,
    pub until_ms: Option<u32>,
    pub dump_frames: Option<PathBuf>,
    pub dump_every: u64,
    pub perf_log: Option<PathBuf>,
    /// `-h`/`--help` was given (also passed on to upstream).
    pub help: bool,
    /// `argv` for upstream: the program name, then every argument this
    /// parser did not consume, in order.
    pub game_args: Vec<String>,
}

/// A command line the binary rejects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CliError {
    MissingValue(&'static str),
    BadNumber { flag: &'static str, value: String },
    BadClock(String),
    ClockNeedsHeadless,
    DumpEveryNeedsDumpFrames,
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::MissingValue(flag) => write!(f, "{flag} needs a value"),
            CliError::BadNumber { flag, value } => {
                write!(f, "{flag} needs a non-negative integer, not `{value}`")
            }
            CliError::BadClock(v) => write!(f, "--clock is `virtual` or `real`, not `{v}`"),
            CliError::ClockNeedsHeadless => f.write_str("--clock only applies with --headless"),
            CliError::DumpEveryNeedsDumpFrames => {
                f.write_str("--dump-every only applies with --dump-frames")
            }
        }
    }
}

impl std::error::Error for CliError {}

/// The flags that take a value, as written on the command line.
const VALUE_FLAGS: &[&str] = &[
    "--clock",
    "--userdir",
    "--input-script",
    "--trace-out",
    "--log-out",
    "--frames",
    "--until-ms",
    "--dump-frames",
    "--dump-every",
    "--perf-log",
];

fn number<T: std::str::FromStr>(flag: &'static str, value: &str) -> Result<T, CliError> {
    value.parse().map_err(|_| CliError::BadNumber {
        flag,
        value: value.to_owned(),
    })
}

/// Splits `args` (including the program name) into the Rust-only options
/// and upstream's `argv`. Both `--flag VALUE` and `--flag=VALUE` are accepted.
pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Options, CliError> {
    let mut args = args.into_iter();
    let mut o = Options {
        dump_every: 1,
        ..Options::default()
    };
    o.game_args.extend(args.next());
    let mut clock_given = false;
    let mut dump_every_given = false;
    while let Some(arg) = args.next() {
        let (name, inline) = match arg.split_once('=') {
            Some((n, v)) if VALUE_FLAGS.contains(&n) => (n.to_owned(), Some(v.to_owned())),
            _ => (arg.clone(), None),
        };
        let Some(&flag) = VALUE_FLAGS.iter().find(|f| **f == name) else {
            match arg.as_str() {
                "--headless" => o.headless = true,
                "-h" | "--help" => {
                    o.help = true;
                    o.game_args.push(arg);
                }
                _ => o.game_args.push(arg),
            }
            continue;
        };
        let value = match inline {
            Some(v) => v,
            None => args.next().ok_or(CliError::MissingValue(flag))?,
        };
        match flag {
            "--clock" => {
                clock_given = true;
                o.clock_real = match value.as_str() {
                    "virtual" => false,
                    "real" => true,
                    _ => return Err(CliError::BadClock(value)),
                };
            }
            "--userdir" => o.userdir = Some(value.into()),
            "--input-script" => o.input_script = Some(value.into()),
            "--trace-out" => o.trace_out = Some(value.into()),
            "--log-out" => o.log_out = Some(value.into()),
            "--frames" => o.frames = Some(number(flag, &value)?),
            "--until-ms" => o.until_ms = Some(number(flag, &value)?),
            "--dump-frames" => o.dump_frames = Some(value.into()),
            "--dump-every" => {
                dump_every_given = true;
                o.dump_every = number(flag, &value)?;
            }
            "--perf-log" => o.perf_log = Some(value.into()),
            _ => unreachable!("every VALUE_FLAGS entry is handled above"),
        }
    }
    if clock_given && !o.headless {
        return Err(CliError::ClockNeedsHeadless);
    }
    if dump_every_given && o.dump_frames.is_none() {
        return Err(CliError::DumpEveryNeedsDumpFrames);
    }
    Ok(o)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn rust_flags_are_consumed_and_the_rest_passes_through_in_order() {
        let o = parse(args(
            "rustyrian --data=/d --headless --clock real --frames=600 -n host --userdir u \
             --input-script s.ots --trace-out t.jsonl --dump-frames f --dump-every 10 --no-sound",
        ))
        .expect("a valid command line");
        assert!(o.headless && o.clock_real);
        assert_eq!(o.frames, Some(600));
        assert_eq!(o.userdir, Some("u".into()));
        assert_eq!(o.input_script, Some("s.ots".into()));
        assert_eq!(o.trace_out, Some("t.jsonl".into()));
        assert_eq!((o.dump_frames, o.dump_every), (Some("f".into()), 10));
        assert_eq!(o.game_args, args("rustyrian --data=/d -n host --no-sound"));
    }

    #[test]
    fn help_is_noted_and_still_reaches_upstream() {
        let o = parse(args("rustyrian --help")).expect("help parses");
        assert!(o.help);
        assert_eq!(o.game_args, args("rustyrian --help"));
    }

    #[test]
    fn invalid_combinations_are_rejected() {
        assert_eq!(
            parse(args("r --clock real")),
            Err(CliError::ClockNeedsHeadless)
        );
        assert_eq!(
            parse(args("r --headless --clock fast")),
            Err(CliError::BadClock("fast".into()))
        );
        assert_eq!(
            parse(args("r --dump-every 5")),
            Err(CliError::DumpEveryNeedsDumpFrames)
        );
        assert_eq!(
            parse(args("r --frames")),
            Err(CliError::MissingValue("--frames"))
        );
        assert_eq!(
            parse(args("r --until-ms soon")),
            Err(CliError::BadNumber {
                flag: "--until-ms",
                value: "soon".into()
            })
        );
    }
}
