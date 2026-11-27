//! `cargo xtask sanitize`: the AddressSanitizer + UBSan oracle
//! (`OT_REF_SANITIZE=1`) on every scenario and the oracle tests.
//!
//! Every run has a deadline and is killed when it passes it, and a short
//! preflight run must finish first, so a sanitized binary that cannot start
//! (for example sdl2-compat failing to load SDL3 on macOS, which blocks on a
//! modal dialog) fails the command quickly instead of hanging.
//!
//! ASan errors fail the command. UBSan reports from upstream C are collected,
//! deduplicated by source location, into `target/sanitize/ubsan.txt` for triage
//! into `porting/upstream-findings.md`.

use crate::scenario;
use anyhow::{Result, bail};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const TARGET_DIR: &str = "target/asan";

/// Environment for building and running the sanitized oracle. On macOS the
/// binaries carry an rpath to SDL3 (see the oracle's build.rs), so no `DYLD_*`
/// variable is needed; the preflight run proves it.
fn sanitizer_env(cmd: &mut Command) {
    // allocator_may_return_null: huge requests from damaged data files must
    // return NULL as libc's malloc does (the paths the tests drive), instead
    // of ASan's allocation-size-too-big abort.
    let mut asan =
        "detect_leaks=0:halt_on_error=1:abort_on_error=0:allocator_may_return_null=1".to_owned();
    if cfg!(target_os = "linux") {
        // The shared ASan runtime is linked, not preloaded.
        asan.push_str(":verify_asan_link_order=0");
    }
    cmd.env("ASAN_OPTIONS", asan)
        .env("UBSAN_OPTIONS", "print_stacktrace=1:halt_on_error=0")
        .env("OT_REF_SANITIZE", "1")
        .env("CARGO_TARGET_DIR", TARGET_DIR);
}

/// Waits for `child`, killing it after `limit`. Returns its exit success.
fn wait_with_deadline(mut child: Child, limit: Duration, what: &str) -> Result<bool> {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status.success());
        }
        if start.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            bail!(
                "{what} did not finish within {}s and was killed",
                limit.as_secs()
            );
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn runner_cmd(root: &Path, exe: &Path, input: &Path, out: &Path, tag: &str) -> Command {
    let mut cmd = Command::new(exe);
    cmd.current_dir(root)
        .arg("--data")
        .arg(root.join("data"))
        .arg("--userdir")
        .arg(out.join(format!("{tag}-user")))
        .arg("--input")
        .arg(input)
        .arg("--trace-out")
        .arg(out.join(format!("{tag}.jsonl")));
    cmd
}

/// `runtime error:` lines keyed by `file:line:col`, with one sample message.
fn collect_ubsan(stderr: &str, into: &mut BTreeMap<String, (String, usize)>) {
    for line in stderr.lines() {
        let Some(idx) = line.find(": runtime error: ") else {
            continue;
        };
        let loc = line[..idx].to_owned();
        let entry = into
            .entry(loc)
            .or_insert_with(|| (line[idx + 2..].to_owned(), 0));
        entry.1 += 1;
    }
}

pub fn run(root: &Path) -> Result<()> {
    if !root.join("data/tyrian1.lvl").is_file() {
        bail!("data/ is missing; run `cargo xtask fetch-data`");
    }
    let out_dir = root.join("target/sanitize");
    if out_dir.exists() {
        std::fs::remove_dir_all(&out_dir)?;
    }
    std::fs::create_dir_all(&out_dir)?;

    eprintln!("sanitize: building the ASan/UBSan oracle into {TARGET_DIR}");
    let mut build = Command::new("cargo");
    build
        .current_dir(root)
        .args(["build", "-p", "rustyrian-oracle", "--bins", "--tests"]);
    sanitizer_env(&mut build);
    if !build.status()?.success() {
        bail!("sanitized oracle build failed");
    }
    let exe = root
        .join(TARGET_DIR)
        .join(format!("debug/ot-ref-run{}", std::env::consts::EXE_SUFFIX));

    // Preflight: the sanitized binary must start and finish a short run.
    let idle = out_dir.join("preflight.ots");
    std::fs::write(&idle, "ots 1\n")?;
    let mut pre = runner_cmd(root, &exe, &idle, &out_dir, "preflight");
    pre.args(["--frames", "5"])
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    sanitizer_env(&mut pre);
    if !wait_with_deadline(pre.spawn()?, Duration::from_secs(60), "preflight run")? {
        bail!("preflight run of the sanitized oracle failed");
    }
    eprintln!("sanitize: preflight OK");

    let mut ubsan: BTreeMap<String, (String, usize)> = BTreeMap::new();
    let mut asan_failures = Vec::new();
    let mut names: Vec<PathBuf> = std::fs::read_dir(root.join("tests/scenarios"))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "ots"))
        .collect();
    names.sort();
    for input in names {
        let name = input
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let sc = scenario::load(root, &name)?;
        if sc.net.as_deref() == Some("udp") {
            continue; // needs a second peer; run by `crossplay`
        }
        let stderr_path = out_dir.join(format!("{name}.stderr"));
        let mut cmd = runner_cmd(root, &exe, &input, &out_dir, &name);
        if let Some(ms) = sc.until_ms {
            cmd.args(["--until-ms", &ms.to_string()]);
        }
        if let Some(n) = sc.frames {
            cmd.args(["--frames", &n.to_string()]);
        }
        if let Some(c) = &sc.clock {
            cmd.args(["--clock", c]);
        }
        cmd.arg("--")
            .args(&sc.args)
            .stdout(Stdio::null())
            .stderr(std::fs::File::create(&stderr_path)?);
        sanitizer_env(&mut cmd);
        eprintln!("sanitize: scenario {name}");
        let ok = wait_with_deadline(cmd.spawn()?, Duration::from_secs(3600), &name)?;
        let stderr = std::fs::read_to_string(&stderr_path).unwrap_or_default();
        collect_ubsan(&stderr, &mut ubsan);
        if stderr.contains("ERROR: AddressSanitizer") || !ok {
            asan_failures.push(format!("{name} (see {})", stderr_path.display()));
        }
    }

    eprintln!("sanitize: oracle tests");
    let mut tests = Command::new("cargo");
    tests
        .current_dir(root)
        .args(["nextest", "run", "-p", "rustyrian-oracle", "--no-fail-fast"])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    sanitizer_env(&mut tests);
    let tests_ok = wait_with_deadline(tests.spawn()?, Duration::from_secs(3600), "oracle tests")?;

    let mut report = String::from("# UBSan reports from upstream C (location, count, sample)\n");
    for (loc, (msg, n)) in &ubsan {
        report.push_str(&format!("{loc}\t{n}\t{msg}\n"));
    }
    std::fs::write(out_dir.join("ubsan.txt"), report)?;
    eprintln!(
        "sanitize: {} distinct UBSan locations (target/sanitize/ubsan.txt)",
        ubsan.len()
    );

    if !asan_failures.is_empty() {
        bail!("AddressSanitizer errors: {}", asan_failures.join(", "));
    }
    if !tests_ok {
        bail!("sanitized oracle tests failed");
    }
    eprintln!("sanitize: OK");
    Ok(())
}
