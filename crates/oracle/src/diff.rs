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
use rustyrian_harness::{CallLog, test_session};
use std::ffi::{CString, c_char};

/// The two engines of one differential test.
pub struct Diff {
    /// The Rust engine.
    pub game: Box<Game>,
    c_log: CallLog,
    rust_log: CallLog,
}

impl Diff {
    /// Installs a fresh C session and builds a fresh Rust game. Both start in
    /// the state of a freshly started process.
    pub fn new() -> Diff {
        let (c, c_log) = test_session();
        session::install(c);
        let (r, rust_log) = test_session();
        Diff {
            game: Game::new(Box::new(r)),
            c_log,
            rust_log,
        }
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
