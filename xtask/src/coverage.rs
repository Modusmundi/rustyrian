//! `cargo xtask coverage`: C line coverage of every upstream function from the
//! differential tests and the scenario runs, written to `porting/coverage.json`
//! for `check-trace`'s 95% rule.
//!
//! The oracle is built with `OT_REF_COVERAGE=1` (clang `--coverage`, gcov
//! format) into `target/cov`; `gcovr` reads the counters through
//! `llvm-cov gcov` (`OT_REF_GCOV` overrides the command).

use crate::inventory;
use crate::scenario;
use crate::tools::{run_capture, run_status};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

const TARGET_DIR: &str = "target/cov";

fn gcov_command() -> String {
    if let Ok(c) = std::env::var("OT_REF_GCOV") {
        return c;
    }
    if cfg!(target_os = "macos") {
        return "xcrun llvm-cov gcov".to_owned();
    }
    for v in ["llvm-cov-18", "llvm-cov"] {
        if Command::new(v).arg("--version").output().is_ok_and(|o| o.status.success()) {
            return format!("{v} gcov");
        }
    }
    "llvm-cov gcov".to_owned()
}

fn cov_env(cmd: &mut Command) {
    cmd.env("OT_REF_COVERAGE", "1").env("CARGO_TARGET_DIR", TARGET_DIR);
}

/// The oracle build's object directory (holds `.gcno`/`.gcda`).
fn object_dir(root: &Path) -> Result<PathBuf> {
    let build = root.join(TARGET_DIR).join("debug/build");
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for e in std::fs::read_dir(&build).with_context(|| format!("reading {}", build.display()))? {
        let p = e?.path().join("out/otref");
        if p.is_dir() {
            let t = std::fs::metadata(&p)?.modified()?;
            if best.as_ref().is_none_or(|(bt, _)| t > *bt) {
                best = Some((t, p));
            }
        }
    }
    best.map(|(_, p)| p).context("no oracle object directory under target/cov")
}

pub fn run(root: &Path) -> Result<()> {
    if !root.join("data/tyrian1.lvl").is_file() {
        bail!("data/ is missing; run `cargo xtask fetch-data`");
    }
    eprintln!("coverage: building the instrumented oracle into {TARGET_DIR}");
    let mut build = Command::new("cargo");
    build
        .current_dir(root)
        .args(["build", "-p", "rustyrian-oracle", "--bins", "--tests"]);
    cov_env(&mut build);
    run_status(&mut build)?;
    let objdir = object_dir(root)?;
    for e in std::fs::read_dir(&objdir)? {
        let p = e?.path();
        if p.extension().is_some_and(|x| x == "gcda") {
            std::fs::remove_file(p)?;
        }
    }

    eprintln!("coverage: differential tests");
    let mut tests = Command::new("cargo");
    tests
        .current_dir(root)
        .args(["nextest", "run", "-p", "rustyrian-oracle", "--no-fail-fast"]);
    cov_env(&mut tests);
    let tests_ok = tests.status()?.success();

    eprintln!("coverage: scenario runs");
    let exe = root
        .join(TARGET_DIR)
        .join(format!("debug/ot-ref-run{}", std::env::consts::EXE_SUFFIX));
    let out = root.join(TARGET_DIR).join("scenarios");
    let mut names: Vec<String> = std::fs::read_dir(root.join("tests/scenarios"))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "ots"))
        .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .collect();
    names.sort();
    for name in &names {
        let sc = scenario::load(root, name)?;
        if sc.net.as_deref() == Some("udp") {
            continue;
        }
        scenario::run(root, &exe, name, &sc, &out.join(name), "c", &[])?;
    }

    eprintln!("coverage: collecting with gcovr");
    let report = root.join(TARGET_DIR).join("gcovr.json");
    let src = root.join("vendor/opentyrian/src");
    run_capture(
        Command::new("gcovr")
            .current_dir(root)
            .arg("--root")
            .arg(&src)
            .arg("--object-directory")
            .arg(&objdir)
            .arg("--gcov-executable")
            .arg(gcov_command())
            .arg("--json")
            .arg("--output")
            .arg(&report)
            .arg(&objdir),
    )?;
    let gcovr: Value = serde_json::from_str(&std::fs::read_to_string(&report)?)?;

    // file name -> line -> hit count
    let mut lines: BTreeMap<String, BTreeMap<u64, u64>> = BTreeMap::new();
    for f in gcovr["files"].as_array().into_iter().flatten() {
        let name = Path::new(f["file"].as_str().unwrap_or_default())
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let entry = lines.entry(name).or_default();
        for l in f["lines"].as_array().into_iter().flatten() {
            let (Some(n), Some(c)) = (l["line_number"].as_u64(), l["count"].as_u64()) else {
                continue;
            };
            *entry.entry(n).or_default() += c;
        }
    }

    let inv = inventory::load(root)?;
    let mut functions = serde_json::Map::new();
    let (mut total, mut covered) = (0u64, 0u64);
    for f in &inv.functions {
        let file_lines = lines.get(&f.file);
        let mut n = 0u64;
        let mut uncovered = Vec::new();
        for line in u64::from(f.begin_line)..=u64::from(f.end_line) {
            if let Some(&count) = file_lines.and_then(|m| m.get(&line)) {
                n += 1;
                if count == 0 {
                    uncovered.push(line);
                }
            }
        }
        total += n;
        covered += n - uncovered.len() as u64;
        functions.insert(
            f.key(),
            json!({ "lines": n, "covered": n - uncovered.len() as u64, "uncovered": uncovered }),
        );
    }
    let doc = json!({
        "upstream": inv.upstream,
        "totals": { "lines": total, "covered": covered },
        "functions": functions,
    });
    std::fs::write(
        root.join("porting/coverage.json"),
        serde_json::to_string_pretty(&doc)? + "\n",
    )?;
    eprintln!(
        "coverage: {covered}/{total} upstream function lines covered ({:.1}%); wrote porting/coverage.json",
        100.0 * covered as f64 / total.max(1) as f64
    );
    if !tests_ok {
        bail!("some differential tests failed under coverage");
    }
    Ok(())
}
