//! Smoke tests for the oracle itself: `exit()` capture through `guard_call`,
//! `ot-ref-run` trace determinism, and the runners' `--sim-digest`.

use rustyrian_harness::trace::Arg;
use rustyrian_oracle::{bindings, guard_call, session};
use std::ffi::{CString, c_char, c_int};
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn guard_call_captures_c_exit_and_platform_calls() {
    let (s, log) = rustyrian_harness::test_session();
    session::install(s);
    let argv: Vec<CString> = ["opentyrian", "--bogus"]
        .into_iter()
        .map(|s| CString::new(s).expect("no NUL"))
        .collect();
    let mut ptrs: Vec<*mut c_char> = argv.iter().map(|c| c.as_ptr().cast_mut()).collect();
    ptrs.push(std::ptr::null_mut());
    let argc = c_int::try_from(argv.len()).expect("argc");
    let argv_ptr = ptrs.as_mut_ptr();

    // SAFETY: argv outlives the call; the closure captures only Copy values.
    let code = unsafe { guard_call(move || bindings::JE_paramCheck(argc, argv_ptr)) };
    assert_eq!(
        code,
        Some(1),
        "an unknown option makes upstream exit(EXIT_FAILURE)"
    );
    assert_eq!(session::last_exit(), Some(1));

    let calls = log.take();
    let errors: Vec<&Arg> = calls
        .iter()
        .filter(|c| c.name == "SDL_LogError")
        .map(|c| &c.args[1])
        .collect();
    assert_eq!(
        errors,
        [
            &Arg::Str("opentyrian: unrecognized option '--bogus'".into()),
            &Arg::Str("Try 'opentyrian --help' for more information.".into()),
        ]
    );
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run_ref(dir: &Path) -> String {
    run(
        env!("CARGO_BIN_EXE_ot-ref-run"),
        dir,
        "demo-cycle",
        &["--frames", "60"],
    )
}

/// Runs a runner binary on `tests/scenarios/<scenario>.ots` and returns its
/// trace.
fn run(exe: &str, dir: &Path, scenario: &str, extra: &[&str]) -> String {
    let root = root();
    let trace = dir.join("trace.jsonl");
    let status = Command::new(exe)
        .arg("--data")
        .arg(root.join("data"))
        .arg("--userdir")
        .arg(dir.join("user"))
        .arg("--input")
        .arg(root.join(format!("tests/scenarios/{scenario}.ots")))
        .arg("--trace-out")
        .arg(&trace)
        .args(extra)
        .status()
        .unwrap_or_else(|e| panic!("spawn {exe}: {e}"));
    assert!(status.success(), "{exe} exited with {status}");
    std::fs::read_to_string(trace).expect("trace")
}

/// The `sim` value of every frame line of a trace (`None` where absent).
fn sims(trace: &str) -> Vec<Option<String>> {
    trace
        .lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).expect("JSON line"))
        .filter(|l| l.get("frame").is_some())
        .map(|l| l.get("sim").and_then(|s| s.as_str()).map(str::to_owned))
        .collect()
}

/// `--sim-digest` (what `xtask crossplay` compares) is written by both
/// runners on every frame, and equal frame by frame through a demo, where
/// the simulation state it covers changes.
#[test]
fn sim_digest_is_written_by_both_runners_and_matches() {
    let tmp = std::env::temp_dir().join(format!("ot-sim-{}", std::process::id()));
    let args = ["--until-ms", "16000", "--sim-digest"];
    let c = run(
        env!("CARGO_BIN_EXE_ot-ref-run"),
        &tmp.join("c"),
        "demo-menu",
        &args,
    );
    let rust = run(
        env!("CARGO_BIN_EXE_rs-run"),
        &tmp.join("rust"),
        "demo-menu",
        &args,
    );
    let _ = std::fs::remove_dir_all(&tmp);
    let (c, rust) = (sims(&c), sims(&rust));
    assert!(c.iter().all(Option::is_some), "a C frame without `sim`");
    assert!(
        rust.iter().all(Option::is_some),
        "a Rust frame without `sim`"
    );
    assert_eq!(c, rust, "the sim digests differ");
    let distinct: std::collections::BTreeSet<_> = c.iter().collect();
    assert!(
        distinct.len() > 100,
        "only {} distinct digests: the demo did not run",
        distinct.len()
    );
}

#[test]
fn ot_ref_run_traces_are_deterministic_and_complete() {
    let tmp = std::env::temp_dir().join(format!("ot-smoke-{}", std::process::id()));
    let a = run_ref(&tmp.join("a"));
    let b = run_ref(&tmp.join("b"));
    let _ = std::fs::remove_dir_all(&tmp);
    assert_eq!(a, b, "two runs with different user directories differ");

    let lines: Vec<serde_json::Value> = a
        .lines()
        .map(|l| serde_json::from_str(l).expect("JSON line"))
        .collect();
    assert_eq!(lines.len(), 1 + 60 + 1, "header, 60 frames, end");
    assert_eq!(lines[0]["engine"], "c");
    assert_eq!(lines[0]["scenario"], "demo-cycle");
    for (i, l) in lines[1..61].iter().enumerate() {
        assert_eq!(l["frame"], i);
    }
    assert_eq!(lines[61]["end"], "limit");
}
