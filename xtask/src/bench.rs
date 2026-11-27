//! `cargo xtask bench-ratio [--rust-runner PATH]`: headless throughput of the
//! Rust runner relative to the C oracle on scenario `demo-cycle`, 600 s of
//! virtual clock, release builds, 5 alternating runs each, medians. Fails below
//! 0.90.

use crate::scenario;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const SCENARIO: &str = "demo-cycle";
const UNTIL_MS: u32 = 600_000;
const RUNS: usize = 5;
const MIN_RATIO: f64 = 0.90;

pub fn run(root: &Path, rust_runner: Option<&Path>) -> Result<()> {
    let mut sc = scenario::load(root, SCENARIO)?;
    sc.until_ms = Some(UNTIL_MS);
    sc.frames = None;
    sc.clock = Some("virtual".to_owned());
    let spec = &sc.runs(root, SCENARIO)[0];
    let c_exe = scenario::build_runner(root, "ot-ref-run", true)?;
    let rust_exe: PathBuf = match rust_runner {
        Some(p) => p.to_path_buf(),
        None => scenario::build_runner(root, "rs-run", true)?,
    };
    let dir = root.join("target/bench-ratio");
    let time = |exe: &Path, tag: &str| -> Result<Duration> {
        let userdir = scenario::fresh_userdir(root, SCENARIO, &dir, tag)?;
        let trace = dir.join(format!("{tag}.jsonl"));
        let mut cmd = scenario::command(root, exe, spec, &userdir, &trace, None, &[]);
        let start = Instant::now();
        let status = cmd
            .status()
            .with_context(|| format!("running {}", exe.display()))?;
        let took = start.elapsed();
        if !status.success() {
            bail!("{} exited with {status}", exe.display());
        }
        Ok(took)
    };
    let (mut c, mut rust) = (Vec::new(), Vec::new());
    for i in 0..RUNS {
        // Interleave, swapping which engine goes first, so that drift in
        // machine load and cache warmth hit both engines alike.
        if i % 2 == 0 {
            c.push(time(&c_exe, "c")?);
            rust.push(time(&rust_exe, "rust")?);
        } else {
            rust.push(time(&rust_exe, "rust")?);
            c.push(time(&c_exe, "c")?);
        }
        eprintln!(
            "bench-ratio: run {}: c {:.3} s, rust {:.3} s",
            i + 1,
            c[i].as_secs_f64(),
            rust[i].as_secs_f64()
        );
    }
    let (mc, mr) = (median(&mut c), median(&mut rust));
    let ratio = mc.as_secs_f64() / mr.as_secs_f64();
    println!(
        "bench-ratio: {SCENARIO} {} s virtual: median c {:.3} s, rust {:.3} s; Rust/C throughput {ratio:.3} (minimum {MIN_RATIO})",
        UNTIL_MS / 1000,
        mc.as_secs_f64(),
        mr.as_secs_f64()
    );
    if ratio < MIN_RATIO {
        bail!("Rust/C throughput {ratio:.3} is below {MIN_RATIO}");
    }
    Ok(())
}

fn median(v: &mut [Duration]) -> Duration {
    v.sort();
    v[v.len() / 2]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_of_odd_count() {
        let mut v = [3, 1, 5, 2, 4].map(Duration::from_millis);
        assert_eq!(median(&mut v), Duration::from_millis(3));
    }
}
