//! The process-wide session behind the C shim, and C exit handling.
//!
//! C globals are process-wide, so each runner or differential test owns one
//! process (nextest runs every test in its own process). All C code runs on the
//! thread that installed the session; the audio callback runs inside
//! `SDL_RenderPresent` on that same thread.

use rustyrian_core::crand;
use rustyrian_harness::{End, Session};
use std::cell::{Cell, RefCell};

thread_local! {
    static SESSION: RefCell<Option<Session>> = const { RefCell::new(None) };
    static ATEXIT: RefCell<Vec<unsafe extern "C" fn()>> = const { RefCell::new(Vec::new()) };
    static RUNNER: Cell<bool> = const { Cell::new(false) };
    static LAST_EXIT: Cell<Option<i32>> = const { Cell::new(None) };
    static CRAND: RefCell<crand::State> = RefCell::new(crand::State::default());
}

/// Installs the session that the C shim adapters delegate to.
pub fn install(session: Session) {
    SESSION.with(|s| *s.borrow_mut() = Some(session));
}

/// Removes and returns the installed session.
pub fn take() -> Option<Session> {
    SESSION.with(|s| s.borrow_mut().take())
}

/// Runs `f` on the installed session.
///
/// # Panics
/// If no session is installed or a platform call re-enters the shim.
pub fn with<R>(f: impl FnOnce(&mut Session) -> R) -> R {
    SESSION.with(|s| {
        let mut guard = s
            .try_borrow_mut()
            .expect("re-entrant platform call from C while the session is busy");
        f(guard.as_mut().expect("no oracle session installed"))
    })
}

/// In a runner, `exit()` runs the registered atexit handlers and finishes the
/// trace; in a test, it only records the code (see `ot_guard_call`).
pub fn set_runner_mode(on: bool) {
    RUNNER.with(|r| r.set(on));
}

/// Exit code of the last `exit()` call made by C, if any.
pub fn last_exit() -> Option<i32> {
    LAST_EXIT.with(Cell::get)
}

pub(crate) fn register_atexit(f: unsafe extern "C" fn()) {
    ATEXIT.with(|a| a.borrow_mut().push(f));
}

/// Number of handlers C registered with `atexit` (only `SDL_Quit` upstream).
pub fn atexit_len() -> usize {
    ATEXIT.with(|a| a.borrow().len())
}

pub(crate) fn crand_next() -> i32 {
    CRAND.with(|c| crand::rand(&mut c.borrow_mut()))
}

/// The C side's `rand()` state, for mirrors.
pub fn crand_state() -> crand::State {
    CRAND.with(|c| c.borrow().clone())
}

pub fn set_crand_state(state: crand::State) {
    CRAND.with(|c| *c.borrow_mut() = state);
}

/// Runs the atexit handlers in reverse registration order, as `exit` does,
/// then finishes the trace with `code`.
pub fn run_exit(code: i32) {
    loop {
        let next = ATEXIT.with(|a| a.borrow_mut().pop());
        match next {
            // SAFETY: registered by C through `atexit`; takes no arguments.
            Some(f) => unsafe { f() },
            None => break,
        }
    }
    with(|s| s.finish(End::Exit(code)));
}

pub(crate) fn on_exit(code: i32, guarded: bool) {
    LAST_EXIT.with(|c| c.set(Some(code)));
    if !guarded && RUNNER.with(Cell::get) {
        run_exit(code);
    }
}
