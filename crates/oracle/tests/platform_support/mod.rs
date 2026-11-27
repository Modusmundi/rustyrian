//! Shared setup for the platform-module differential tests (`video`,
//! `video_scale`, `keyboard`, `mouse`, `joystick`).

#![allow(dead_code)]

use rustyrian_core::Game;
use rustyrian_core::game::{Halt, Snapshot};
use rustyrian_core::platform::{JoystickChange, Platform, PlatformProbe};
use rustyrian_harness::trace::PlatformCall;
use rustyrian_harness::{CallLog, Session, test_session};
use rustyrian_oracle::{bindings as b, guard_call, mirror, session};

/// A C engine and a Rust engine whose fake platforms fail the next call to
/// each of the given SDL functions.
pub struct Both {
    pub game: Box<Game>,
    c_log: CallLog,
    r_log: CallLog,
}

impl Both {
    pub fn new(fail: &[&str]) -> Both {
        let (mut c, c_log) = test_session();
        let (mut r, r_log) = test_session();
        for f in fail.iter().filter(|f| !f.is_empty()) {
            c.inner_mut().fail_next(f);
            r.inner_mut().fail_next(f);
        }
        session::install(c);
        let both = Both {
            game: Game::new(Box::new(r)),
            c_log,
            r_log,
        };
        // Reset every C global to its initial value (earlier proptest cases
        // in this process changed them).
        both.store();
        both
    }

    /// Copies the Rust state into C.
    pub fn store(&self) {
        mirror::store_game(&self.game);
    }

    /// Platform calls made by C since the last call.
    pub fn c_calls(&self) -> Vec<PlatformCall> {
        self.c_log.take()
    }

    /// Platform calls made by Rust since the last call.
    pub fn rust_calls(&self) -> Vec<PlatformCall> {
        self.r_log.take()
    }

    /// Forgets the calls made so far.
    pub fn clear_calls(&self) {
        self.c_log.take();
        self.r_log.take();
    }

    /// The whole C state, as core types.
    pub fn c_state(&self) -> Snapshot {
        let (p, _) = test_session();
        let mut g = Game::new(Box::new(p));
        mirror::load_game(&mut g);
        g.snapshot()
    }

    /// Asserts equal platform calls and equal states.
    pub fn assert_same(&self) {
        assert_eq!(self.c_calls(), self.rust_calls(), "platform calls differ");
        assert_eq!(self.c_state(), self.game.snapshot(), "states differ");
    }

    /// Runs the same platform call on both fake platforms (recorded; use
    /// `clear_calls` afterwards).
    pub fn both<R: PartialEq + std::fmt::Debug>(
        &mut self,
        f: impl Fn(&mut dyn Platform) -> R,
    ) -> R {
        let c = session::with(|s| f(s));
        let r = f(&mut *self.game.platform);
        assert_eq!(c, r);
        r
    }

    /// Applies a scripted joystick change to both fake platforms.
    pub fn joystick(&mut self, change: JoystickChange) {
        session::with(|s| s.inner_mut().probe_joystick(change.clone()));
        probe(&mut self.game).inner_mut().probe_joystick(change);
    }

    /// Queues an OS event on both fake platforms.
    pub fn inject(&mut self, event: rustyrian_core::platform::Event) {
        session::with(|s| s.inner_mut().probe_inject_event(event.clone()));
        probe(&mut self.game).inner_mut().probe_inject_event(event);
    }

    /// Runs `init_video` on both engines; returns the exit code, if any.
    pub fn init_video(&mut self) -> Option<i32> {
        // SAFETY: plain C call on the test thread.
        let c = unsafe { guard_call(|| b::init_video()) };
        let r = rustyrian_core::video::init_video(&mut self.game)
            .err()
            .map(|Halt { code }| code);
        assert_eq!(c, r, "init_video exit codes differ");
        c
    }
}

/// The Rust side's probes. The `Game` platform of a [`Both`] is a test
/// `Session`; the probe methods are not part of `Platform`, so they are
/// reached through a pointer cast of the known concrete type.
fn probe(g: &mut Game) -> &mut Session {
    let p: *mut dyn Platform = &mut *g.platform;
    // SAFETY: `Both::new` boxed a `Session` as the game's platform and
    // nothing replaces it; the data pointer of the trait object is that
    // `Session`.
    unsafe { &mut *p.cast::<Session>() }
}

/// Makes the next call to `name` fail on both fake platforms.
pub fn fail_next(both: &mut Both, name: &str) {
    if name.is_empty() {
        return;
    }
    session::with(|s| s.inner_mut().fail_next(name));
    probe(&mut both.game).inner_mut().fail_next(name);
}

/// Makes the next call to `name` fail on the Rust side only (for paths where
/// C has undefined behaviour).
pub fn fail_next_rust(both: &mut Both, name: &str) {
    probe(&mut both.game).inner_mut().fail_next(name);
}
