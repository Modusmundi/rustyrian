//! The command line shared by `ot-ref-run` (C engine) and `rs-run` (Rust
//! engine):
//!
//! ```text
//! --data DIR --userdir DIR --input FILE --trace-out FILE
//! [--frames N] [--until-ms T] [--clock virtual|real] [--net loopback|udp]
//! [--sim-digest] [--dump-frames DIR --dump-every K] [--log-out FILE]
//! [--echo-logs] -- <game args>
//! ```
//!
//! At least one of `--frames`/`--until-ms` is required. The runner sets
//! `XDG_CONFIG_HOME` and `HOME` to the user directory (so C uses
//! `<userdir>/opentyrian` and Rust `<userdir>/rustyrian`), `TZ=UTC`, and passes
//! `--data=DIR` to the game.

use anyhow::{Context, Result, bail};
use rustyrian_harness::{
    Clock, FakeConfig, FakePlatform, InstrumentConfig, Instrumented, NetBackend, Session, script,
};
use std::path::PathBuf;

#[derive(Debug, Default)]
pub struct RunArgs {
    pub data: PathBuf,
    pub userdir: PathBuf,
    pub input: PathBuf,
    pub trace_out: PathBuf,
    pub frames: Option<u64>,
    pub until_ms: Option<u32>,
    pub clock_real: bool,
    pub net_udp: bool,
    pub sim_digest: bool,
    pub dump_frames: Option<PathBuf>,
    pub dump_every: u64,
    pub log_out: Option<PathBuf>,
    pub echo_logs: bool,
    pub game_args: Vec<String>,
}

pub fn parse_args(args: impl IntoIterator<Item = String>) -> Result<RunArgs> {
    let mut it = args.into_iter();
    let mut a = RunArgs {
        dump_every: 1,
        ..RunArgs::default()
    };
    let (mut data, mut userdir, mut input, mut trace) = (None, None, None, None);
    while let Some(arg) = it.next() {
        let mut value = |name: &str| it.next().with_context(|| format!("{name} needs a value"));
        match arg.as_str() {
            "--data" => data = Some(PathBuf::from(value("--data")?)),
            "--userdir" => userdir = Some(PathBuf::from(value("--userdir")?)),
            "--input" => input = Some(PathBuf::from(value("--input")?)),
            "--trace-out" => trace = Some(PathBuf::from(value("--trace-out")?)),
            "--frames" => a.frames = Some(value("--frames")?.parse()?),
            "--until-ms" => a.until_ms = Some(value("--until-ms")?.parse()?),
            "--clock" => {
                a.clock_real = match value("--clock")?.as_str() {
                    "virtual" => false,
                    "real" => true,
                    other => bail!("--clock must be virtual or real, not {other}"),
                }
            }
            "--net" => {
                a.net_udp = match value("--net")?.as_str() {
                    "loopback" => false,
                    "udp" => true,
                    other => bail!("--net must be loopback or udp, not {other}"),
                }
            }
            "--sim-digest" => a.sim_digest = true,
            "--dump-frames" => a.dump_frames = Some(PathBuf::from(value("--dump-frames")?)),
            "--dump-every" => a.dump_every = value("--dump-every")?.parse()?,
            "--log-out" => a.log_out = Some(PathBuf::from(value("--log-out")?)),
            "--echo-logs" => a.echo_logs = true,
            "--" => {
                a.game_args = it.by_ref().collect();
                break;
            }
            other => bail!("unknown argument {other}"),
        }
    }
    a.data = data.context("--data is required")?;
    a.userdir = userdir.context("--userdir is required")?;
    a.input = input.context("--input is required")?;
    a.trace_out = trace.context("--trace-out is required")?;
    if a.frames.is_none() && a.until_ms.is_none() {
        bail!("one of --frames or --until-ms is required");
    }
    // Canonical, so the game sees the same path however it was spelled and
    // redaction replaces only the real directory prefix.
    a.data = a
        .data
        .canonicalize()
        .with_context(|| format!("data directory {}", a.data.display()))?;
    Ok(a)
}

/// Prepares the environment and builds the session for an engine.
pub fn prepare(args: &RunArgs, engine: &str) -> Result<Session> {
    std::fs::create_dir_all(&args.userdir)?;
    let userdir = args.userdir.canonicalize()?;
    let base = userdir.join("_base");
    std::fs::create_dir_all(&base)?;
    // SAFETY: called at startup, before any other thread exists.
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", &userdir);
        std::env::set_var("HOME", &userdir);
        std::env::set_var("TZ", "UTC");
    }
    let text = std::fs::read_to_string(&args.input)
        .with_context(|| format!("reading {}", args.input.display()))?;
    let script =
        script::parse(&text).map_err(|e| anyhow::anyhow!("{}:{e}", args.input.display()))?;
    let scenario = args
        .input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let fake = FakePlatform::new(FakeConfig {
        clock: if args.clock_real {
            Clock::Real
        } else {
            Clock::Virtual
        },
        net: if args.net_udp {
            NetBackend::Udp
        } else {
            NetBackend::Loopback
        },
        base_path: Some(format!("{}/", base.display())),
        echo_logs: args.echo_logs,
    });
    let session = Instrumented::new(
        fake,
        InstrumentConfig {
            engine: engine.to_owned(),
            scenario,
            script: Some(script),
            trace_out: Some(args.trace_out.clone()),
            log_out: args.log_out.clone(),
            perf_log: None,
            frames: args.frames,
            until_ms: args.until_ms,
            dump_frames: args.dump_frames.clone().map(|d| (d, args.dump_every)),
            sim_digest: args.sim_digest,
            redact: vec![
                (userdir.display().to_string(), "<userdir>".to_owned()),
                (args.data.display().to_string(), "<data>".to_owned()),
            ],
        },
    )?;
    Ok(session)
}

/// `argv` for the game: program name, `--data=DIR`, then the game arguments.
pub fn game_argv(args: &RunArgs, program: &str) -> Vec<String> {
    let mut v = vec![
        program.to_owned(),
        format!("--data={}", args.data.display()),
    ];
    v.extend(args.game_args.iter().cloned());
    v
}
