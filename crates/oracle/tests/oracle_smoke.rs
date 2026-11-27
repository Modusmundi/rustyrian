//! Smoke tests for the oracle itself: `exit()` capture through `guard_call`,
//! and `ot-ref-run` trace determinism.

use rustyrian_harness::trace::Arg;
use rustyrian_oracle::{bindings, guard_call, session};
use std::ffi::{CString, c_char, c_int};
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn guard_call_captures_c_exit_and_platform_calls() {
    session::install(rustyrian_harness::test_session());
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

    let calls = session::with(|s| std::mem::take(&mut s.recorder.recent_calls));
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
    let root = root();
    let trace = dir.join("trace.jsonl");
    let status = Command::new(env!("CARGO_BIN_EXE_ot-ref-run"))
        .arg("--data")
        .arg(root.join("data"))
        .arg("--userdir")
        .arg(dir.join("user"))
        .arg("--input")
        .arg(root.join("tests/scenarios/demo-cycle.ots"))
        .arg("--trace-out")
        .arg(&trace)
        .args(["--frames", "60"])
        .status()
        .expect("spawn ot-ref-run");
    assert!(status.success(), "ot-ref-run exited with {status}");
    std::fs::read_to_string(trace).expect("trace")
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
