//! `cargo xtask coverage`: C line coverage of every upstream function from the
//! differential tests and the scenario runs, written to `porting/coverage.json`
//! for `check-trace`'s 95% rule.
//!
//! The oracle is built with `OT_REF_COVERAGE=1` (clang `--coverage`, gcov
//! format) into `target/cov`. The counters are read with `llvm-cov gcov
//! --stdout` (`OT_REF_GCOV` overrides the command) and parsed here: gcovr's
//! parser rejects counts wider than the gcov column (ten or more digits, which
//! the scaler tests reach).

use crate::inventory;
use crate::scenario;
use crate::tools::{run_capture, run_status};
use anyhow::{Context, Result, bail};
use serde_json::json;
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
        if Command::new(v)
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success())
        {
            return format!("{v} gcov");
        }
    }
    "llvm-cov gcov".to_owned()
}

fn cov_env(cmd: &mut Command) {
    cmd.env("OT_REF_COVERAGE", "1")
        .env("CARGO_TARGET_DIR", TARGET_DIR);
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
    best.map(|(_, p)| p)
        .context("no oracle object directory under target/cov")
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

    // Long tests are `#[ignore]`d for the per-push CI run; coverage runs them.
    // Local-only advisory reproductions (git-ignored) need the sanitizer
    // build and are left out.
    eprintln!("coverage: differential tests (including ignored ones)");
    let mut tests = Command::new("cargo");
    tests.current_dir(root).args([
        "nextest",
        "run",
        "-p",
        "rustyrian-oracle",
        "--no-fail-fast",
        "--run-ignored",
        "all",
        "-E",
        "not binary(/^advisory_/)",
    ]);
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

    eprintln!("coverage: collecting with llvm-cov gcov");
    let lines = collect_gcov(root, &objdir)?;

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

/// Upstream source file name -> line -> hit count, summed over every
/// translation unit (headers are compiled into several).
fn collect_gcov(root: &Path, objdir: &Path) -> Result<BTreeMap<String, BTreeMap<u64, u64>>> {
    let gcov = gcov_command();
    let mut words = gcov.split_whitespace();
    let program = words.next().context("empty OT_REF_GCOV")?;
    let pre_args: Vec<&str> = words.collect();
    let mut gcdas: Vec<PathBuf> = std::fs::read_dir(objdir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "gcda"))
        .collect();
    gcdas.sort();
    let mut lines: BTreeMap<String, BTreeMap<u64, u64>> = BTreeMap::new();
    for gcda in &gcdas {
        // Sources are recorded relative to the repository root (the oracle's
        // build.rs compiles from there), so run from it.
        let out = run_capture(
            Command::new(program)
                .current_dir(root)
                .args(&pre_args)
                .arg("--stdout")
                .arg("-o")
                .arg(objdir)
                .arg(gcda),
        )?;
        // Upstream sources are ASCII; anything else only occurs in comments.
        parse_gcov(&String::from_utf8_lossy(&out), &mut lines);
    }
    Ok(lines)
}

/// Adds the executable lines of `llvm-cov gcov --stdout` output for upstream
/// sources to `lines`. Each line is `COUNT:LINE:SOURCE`; `COUNT` is `-` for a
/// non-executable line, `#####`/`=====` for zero, otherwise the hit count
/// (possibly wider than its column). `Source:` headers (line 0) start a file.
fn parse_gcov(out: &str, lines: &mut BTreeMap<String, BTreeMap<u64, u64>>) {
    let mut current: Option<String> = None;
    for l in out.lines() {
        let mut parts = l.splitn(3, ':');
        let (Some(count), Some(line), Some(rest)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let (count, line) = (count.trim(), line.trim());
        if line == "0" {
            if let Some(src) = rest.strip_prefix("Source:") {
                current = src
                    .strip_prefix("vendor/opentyrian/src/")
                    .map(str::to_owned);
            }
            continue;
        }
        let (Some(file), Ok(line)) = (current.as_ref(), line.parse::<u64>()) else {
            continue;
        };
        let hits = match count {
            "-" => continue,
            "#####" | "=====" => 0,
            c => match c.trim_end_matches('*').parse::<u64>() {
                Ok(n) => n,
                Err(_) => continue,
            },
        };
        *lines
            .entry(file.clone())
            .or_default()
            .entry(line)
            .or_default() += hits;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_wide_counts_headers_and_zero_markers() {
        let out = "        -:    0:Source:vendor/opentyrian/src/video_scale.c\n\
                   6676414800:  195:\t\tfor (int x = 0; x < width; x++)\n\
                   \x20   #####:  196:\t\t\tx++;\n\
                   \x20       -:  197:}\n\
                   \x20       3:  198:return;\n\
                   \x20       -:    0:Source:crates/oracle/generated/tu/video_scale.c\n\
                   \x20       9:    1:#include \"x\"\n";
        let mut lines = BTreeMap::new();
        parse_gcov(out, &mut lines);
        parse_gcov(out, &mut lines);
        let vs = &lines["video_scale.c"];
        assert_eq!(vs.get(&195), Some(&(2 * 6_676_414_800)));
        assert_eq!(vs.get(&196), Some(&0));
        assert_eq!(vs.get(&197), None);
        assert_eq!(vs.get(&198), Some(&6));
        assert_eq!(lines.len(), 1, "only upstream sources are kept");
    }
}
