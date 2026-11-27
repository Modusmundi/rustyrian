//! `cargo xtask crossplay [--rust-runner PATH]`: scenario `netplay-crossplay`.
//! The C oracle (`ot-ref-run`) is player 1 and the Rust runner (`rs-run`)
//! player 2; they play over UDP on 127.0.0.1 with the real clock and
//! `--sim-digest`.
//!
//! Pass: both exit 0, neither log contains `unsynchronized`, and the `sim`
//! values are equal for every frame both traces contain. The engines present
//! menu frames at their own pace (player 1 waits in "Waiting for other
//! player" without presenting), so the traces are aligned first: at the frame
//! offset whose run of equal `sim` values reaches the end of the overlap
//! furthest back. From there to the end of the shorter trace every frame must
//! match, and that stretch must cover the level (at least `MIN_LOCKSTEP`
//! frames with `MIN_DISTINCT` different digests), so an idle pair of menus
//! cannot pass.

use crate::parity::read_trace;
use crate::scenario;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

const SCENARIO: &str = "netplay-crossplay";
const MIN_LOCKSTEP: usize = 500;
const MIN_DISTINCT: usize = 300;

pub fn run(root: &Path, rust_runner: Option<&Path>, release: bool) -> Result<()> {
    let sc = scenario::load(root, SCENARIO)?;
    let peers = sc.peer_runs(root);
    if peers.len() != 2 {
        bail!("{SCENARIO} needs two peers");
    }
    let c_exe = scenario::build_runner(root, "ot-ref-run", release)?;
    let rust_exe: PathBuf = match rust_runner {
        Some(p) => p.to_path_buf(),
        None => scenario::build_runner(root, "rs-run", release)?,
    };
    let dir = root.join("target/crossplay");
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    let exes = [&c_exe, &rust_exe];
    let mut children = Vec::new();
    let mut outs = Vec::new();
    for (i, (spec, exe)) in peers.iter().zip(exes).enumerate() {
        let tag = format!("p{}", i + 1);
        let userdir = scenario::fresh_userdir(root, SCENARIO, &dir, &tag)?;
        let trace = dir.join(format!("{tag}.jsonl"));
        let log = dir.join(format!("{tag}.log.jsonl"));
        let mut cmd = scenario::command(root, exe, spec, &userdir, &trace, Some(&log), &[]);
        eprintln!("crossplay: player {} on {}", i + 1, exe.display());
        children.push(
            cmd.spawn()
                .with_context(|| format!("spawning {}", exe.display()))?,
        );
        outs.push((trace, log));
    }
    let mut failures = Vec::new();
    for (i, mut child) in children.into_iter().enumerate() {
        let status = child.wait()?;
        if !status.success() {
            failures.push(format!("player {} exited with {status}", i + 1));
        }
    }
    let mut sims = Vec::new();
    for (i, (trace, log)) in outs.iter().enumerate() {
        let text =
            std::fs::read_to_string(log).with_context(|| format!("reading {}", log.display()))?;
        if text.contains("unsynchronized") {
            failures.push(format!("player {}'s log reports `unsynchronized`", i + 1));
        }
        let lines = read_trace(trace)?;
        let sim: Option<Vec<String>> = lines
            .iter()
            .filter(|l| l.get("frame").is_some())
            .map(|l| l.get("sim").and_then(|s| s.as_str()).map(str::to_owned))
            .collect();
        sims.push(sim.with_context(|| format!("{}: frames without `sim`", trace.display()))?);
    }
    match lockstep(&sims[0], &sims[1]) {
        Some(l) => {
            eprintln!(
                "crossplay: player 1 frames {}.. match player 2 frames {}.. for {} frames ({} distinct digests)",
                l.start_a, l.start_b, l.len, l.distinct
            );
            if l.len < MIN_LOCKSTEP || l.distinct < MIN_DISTINCT {
                failures.push(format!(
                    "the matching stretch is too short to cover the level ({} frames, {} distinct; need {MIN_LOCKSTEP} and {MIN_DISTINCT})",
                    l.len, l.distinct
                ));
            }
        }
        None => failures.push("the sim digests never match at the end of the traces".to_owned()),
    }
    if !failures.is_empty() {
        bail!("crossplay failed:\n  {}", failures.join("\n  "));
    }
    println!("crossplay: OK");
    Ok(())
}

/// The longest alignment of two digest sequences whose equal stretch reaches
/// the end of their overlap.
#[derive(Debug, PartialEq, Eq)]
pub struct Lockstep {
    pub start_a: usize,
    pub start_b: usize,
    pub len: usize,
    pub distinct: usize,
}

pub fn lockstep(a: &[String], b: &[String]) -> Option<Lockstep> {
    let mut best: Option<Lockstep> = None;
    // Frame i of `a` against frame i + d of `b`, for every offset d with an
    // overlap.
    let (la, lb) = (a.len() as isize, b.len() as isize);
    for d in (1 - la)..lb {
        let start_a = (-d).max(0) as usize;
        let start_b = d.max(0) as usize;
        let overlap = (a.len() - start_a).min(b.len() - start_b);
        let len = (0..overlap)
            .rev()
            .take_while(|&k| a[start_a + k] == b[start_b + k])
            .count();
        if len > 0 && best.as_ref().is_none_or(|x| len > x.len) {
            let first = overlap - len;
            let mut seen: Vec<&String> = a[start_a + first..start_a + overlap].iter().collect();
            seen.sort();
            seen.dedup();
            best = Some(Lockstep {
                start_a: start_a + first,
                start_b: start_b + first,
                len,
                distinct: seen.len(),
            });
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| (*x).to_owned()).collect()
    }

    #[test]
    fn aligns_after_menus_of_different_length() {
        // Player 1 presented fewer menu frames; the level is frames x, y, z.
        let a = s(&["m", "m", "x", "y", "z"]);
        let b = s(&["m", "m", "m", "m", "x", "y", "z", "w"]);
        let l = lockstep(&a, &b).expect("aligned");
        assert_eq!((l.start_a, l.start_b, l.len, l.distinct), (0, 2, 5, 4));
    }

    #[test]
    fn a_late_divergence_breaks_the_stretch() {
        let a = s(&["x", "y", "z", "q"]);
        let b = s(&["x", "y", "z", "r"]);
        let l = lockstep(&a, &b);
        assert!(l.is_none_or(|l| l.len < 3));
    }
}
