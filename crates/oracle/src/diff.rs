//! Helpers for differential tests (`crates/oracle/tests/<module>.rs`).
//!
//! Each test process (nextest runs one test per process) gets a C side — the
//! upstream globals plus an installed fake-platform session — and a Rust side —
//! a `Game` on its own fake-platform session. Both sessions record every
//! platform call so tests can compare them.

use crate::mirror::{self, Mirror};
use crate::session;
use rustyrian_core::Game;
use rustyrian_harness::trace::PlatformCall;
use rustyrian_harness::{CallLog, FakeHandle, test_session, test_session_with_handle};
use std::ffi::{CString, c_char};

/// The two engines of one differential test.
pub struct Diff {
    /// The Rust engine.
    pub game: Box<Game>,
    /// Controls of the C side's fake platform (failures, loopback datagrams).
    pub c_fake: FakeHandle,
    /// Controls of the Rust side's fake platform.
    pub rust_fake: FakeHandle,
    c_log: CallLog,
    rust_log: CallLog,
}

impl Diff {
    /// Installs a fresh C session and builds a fresh Rust game. Both start in
    /// the state of a freshly started process.
    pub fn new() -> Diff {
        let (c, c_log, c_fake) = test_session_with_handle();
        session::install(c);
        let (r, rust_log, rust_fake) = test_session_with_handle();
        Diff {
            game: Game::new(Box::new(r)),
            c_fake,
            rust_fake,
            c_log,
            rust_log,
        }
    }

    /// Injects the same failure into both sides (see `FakeHandle::fail_after`).
    pub fn fail_after(&self, sdl_name: &str, successes: u32) {
        self.c_fake.fail_after(sdl_name, successes);
        self.rust_fake.fail_after(sdl_name, successes);
    }

    /// Platform calls made by C since the last call to this method.
    pub fn c_calls(&self) -> Vec<PlatformCall> {
        self.c_log.take()
    }

    /// Platform calls made by Rust since the last call to this method.
    pub fn rust_calls(&self) -> Vec<PlatformCall> {
        self.rust_log.take()
    }

    /// Copies the whole Rust state into C (before calling the C function).
    pub fn store_to_c(&self) {
        mirror::store_game(&self.game);
    }

    /// The whole C state, as core types.
    pub fn c_game_state(&self) -> rustyrian_core::game::Snapshot {
        let (r, _) = test_session();
        let mut g = Game::new(Box::new(r));
        mirror::load_game(&mut g);
        g.snapshot()
    }
}

impl Default for Diff {
    fn default() -> Self {
        Diff::new()
    }
}

/// Loads one module state from C.
pub fn load<S: Mirror + Default>() -> S {
    let mut s = S::default();
    s.load_from_c();
    s
}

/// The C oracle's game state at `frame` of `scenario`, as captured by
/// `cargo xtask fixtures` (listed in `porting/fixtures.toml`).
///
/// # Panics
/// If the fixture file is missing or does not decode (rerun
/// `cargo xtask fixtures` after changing state types).
pub fn fixture(scenario: &str, frame: u64) -> rustyrian_core::game::Snapshot {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/fixtures")
        .join(format!("{scenario}@{frame}.postcard"));
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("{}: {e}; run `cargo xtask fixtures`", path.display()));
    postcard::from_bytes(&bytes)
        .unwrap_or_else(|e| panic!("{}: {e}; rerun `cargo xtask fixtures`", path.display()))
}

impl Diff {
    /// Puts both engines in the fixture's state: the Rust game is restored
    /// from it and the C globals are stored from the Rust game.
    pub fn from_fixture(scenario: &str, frame: u64) -> Diff {
        let mut d = Diff::new();
        d.game.restore(fixture(scenario, frame));
        d.store_to_c();
        d
    }
}

/// A NUL-terminated copy of `s` for passing to C. Keep it alive across the
/// call.
pub fn cstring(s: &str) -> CString {
    CString::new(s).expect("no interior NUL")
}

/// `argv`-style pointer array (NULL-terminated) over `args`, which must
/// outlive its use.
pub fn argv(args: &[CString]) -> Vec<*mut c_char> {
    let mut v: Vec<*mut c_char> = args.iter().map(|a| a.as_ptr().cast_mut()).collect();
    v.push(std::ptr::null_mut());
    v
}

/// A proptest runner that runs exactly `cases` cases, whatever
/// `PROPTEST_CASES` says. The `proptest!` macro re-reads that variable and
/// replaces an explicit `cases`, so tests that run a whole screen, game or
/// script per case (nightly sets 10 000) use this instead:
/// `capped_runner(32).run(&strategy, |args| { … Ok(()) })`.
pub fn capped_runner(cases: u32) -> proptest::test_runner::TestRunner {
    proptest::test_runner::TestRunner::new(proptest::test_runner::Config {
        cases,
        ..proptest::test_runner::Config::default()
    })
}

/// The first field where two snapshots differ, as `path: c-value / rust-value`
/// (values shortened), or `None` when they are equal. Snapshot assertions use
/// it so a failing comparison names the field instead of printing two whole
/// games. Equality is `Snapshot`'s own; the JSON walk only locates the field,
/// skipping what that equality ignores (an open stream's path).
pub fn first_difference(
    c: &rustyrian_core::game::Snapshot,
    rust: &rustyrian_core::game::Snapshot,
) -> Option<String> {
    if c == rust {
        return None;
    }
    let cv = serde_json::to_value(c).expect("snapshots serialize to JSON (plain data)");
    let rv = serde_json::to_value(rust).expect("snapshots serialize to JSON (plain data)");
    Some(
        first_value_difference(&cv, &rv, String::new())
            .unwrap_or_else(|| "a field the JSON form does not show".to_owned()),
    )
}

/// Whether a JSON object is a serialized `file::Stream`, whose equality
/// ignores the path it was opened with.
fn is_stream(o: &serde_json::Map<String, serde_json::Value>) -> bool {
    ["path", "mode", "position", "pending", "error", "eof"]
        .iter()
        .all(|k| o.contains_key(*k))
}

fn first_value_difference(
    c: &serde_json::Value,
    r: &serde_json::Value,
    path: String,
) -> Option<String> {
    use serde_json::Value;
    if c == r {
        return None;
    }
    match (c, r) {
        (Value::Object(a), Value::Object(b)) => {
            let skip_path = is_stream(a) && is_stream(b);
            for (k, v) in a {
                if skip_path && k == "path" {
                    continue;
                }
                let sub = format!("{path}.{k}");
                match b.get(k) {
                    Some(w) => {
                        if let Some(d) = first_value_difference(v, w, sub) {
                            return Some(d);
                        }
                    }
                    None => return Some(format!("{sub}: only in C")),
                }
            }
            b.keys()
                .find(|k| !a.contains_key(*k))
                .map(|k| format!("{path}.{k}: only in Rust"))
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => a
            .iter()
            .zip(b)
            .enumerate()
            .find_map(|(i, (v, w))| first_value_difference(v, w, format!("{path}[{i}]"))),
        _ => {
            let short = |v: &Value| {
                let s = v.to_string();
                if s.len() > 120 {
                    format!("{}…", s.chars().take(120).collect::<String>())
                } else {
                    s
                }
            };
            Some(format!("{path}: c {} / rust {}", short(c), short(r)))
        }
    }
}
