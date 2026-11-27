//! `cargo xtask shipped-trace <scenario> --binary PATH [--out DIR]`: runs the
//! shipped binary (`rustyrian --headless`) on every run of a scenario the way
//! the oracle runners do, so its traces compare with the C engine's
//! (`ot-ref-run`, `cargo xtask ref-trace`) through `cargo xtask trace-cmp`.
//!
//! The mapping from the runner command line (`scenario::command`,
//! `rustyrian_oracle::runner`):
//! - same script, limits, clock and game arguments, and `--data=DIR` first;
//! - the user directory comes from the environment exactly as in the
//!   runners: `XDG_CONFIG_HOME` and `HOME` (Unix), `APPDATA` (Windows) name
//!   the scenario's user directory, so the engine keeps its files in
//!   `<userdir>/rustyrian`. `--userdir` is not used: it sets the directory
//!   directly (D-PRODUCT-2) and so skips upstream's `SDL_GetBasePath` call,
//!   which the C traces record;
//! - the base path is the executable's directory; both the runners and the
//!   shipped binary record it as `<base>`.
//!
//! Writes `rust.jsonl`, then `rust.2.jsonl`, ... for `then` runs, under
//! `target/shipped/<scenario>` unless `--out` says otherwise.

use crate::scenario::{self, RunSpec};
use anyhow::{Context, Result, bail};
use std::path::Path;
use std::process::Command;

pub fn run(root: &Path, name: &str, binary: &Path, out: Option<&Path>) -> Result<()> {
    let sc = scenario::load(root, name)?;
    if !sc.peers.is_empty() {
        bail!("`{name}` is a netplay scenario; run it with `cargo xtask crossplay`");
    }
    let dir = out.map_or_else(|| root.join("target/shipped").join(name), Path::to_path_buf);
    let userdir = std::path::absolute(scenario::fresh_userdir(root, name, &dir, "rust")?)?;
    // Not canonicalised: on Windows that yields a `\\?\` path, which does not
    // accept the `/` upstream joins file names with.
    let data = std::path::absolute(root.join("data"))?;
    for (i, spec) in sc.runs(root, name).iter().enumerate() {
        let trace = dir.join(scenario::out_name("rust", i + 1, ".jsonl"));
        let mut cmd = command(root, binary, spec, &userdir, &data, &trace)?;
        eprintln!("shipped-trace: {name} run {}: {}", i + 1, binary.display());
        let status = cmd
            .status()
            .with_context(|| format!("running {}", binary.display()))?;
        if !trace.is_file() {
            bail!("{} wrote no trace ({status})", binary.display());
        }
        eprintln!("shipped-trace: wrote {}", trace.display());
    }
    Ok(())
}

/// The shipped binary's command line and environment for one run.
fn command(
    root: &Path,
    binary: &Path,
    spec: &RunSpec,
    userdir: &Path,
    data: &Path,
    trace: &Path,
) -> Result<Command> {
    if spec.net.as_deref().is_some_and(|n| n != "loopback") || spec.sim_digest {
        bail!(
            "{}: the shipped binary has no `--net`/`--sim-digest`",
            spec.input.display()
        );
    }
    let mut cmd = Command::new(binary);
    cmd.current_dir(root)
        .env("TZ", "UTC")
        .arg("--headless")
        .arg("--input-script")
        .arg(&spec.input)
        .arg("--trace-out")
        .arg(trace);
    if cfg!(windows) {
        cmd.env("APPDATA", userdir);
    } else {
        cmd.env("XDG_CONFIG_HOME", userdir).env("HOME", userdir);
    }
    if let Some(ms) = spec.until_ms {
        cmd.args(["--until-ms", &ms.to_string()]);
    }
    if let Some(n) = spec.frames {
        cmd.args(["--frames", &n.to_string()]);
    }
    if let Some(c) = &spec.clock {
        cmd.args(["--clock", c]);
    }
    cmd.arg(format!("--data={}", data.display()))
        .args(&spec.args);
    Ok(cmd)
}
