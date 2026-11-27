//! Differential tests for `logging.c` / `logging.h`.

use proptest::prelude::*;
use rustyrian_core::logging::{self, log_fatal};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};

proptest! {
    /// Short messages fit C's 256-byte stack buffer; longer ones take the
    /// `malloc` path. Both log at critical priority and show a message box.
    #[test]
    fn log_fatal_matches(message in "[ -~]{0,600}") {
        let mut d = Diff::new();
        let c = diff::cstring(&message);
        // SAFETY: "%s" with one C string argument.
        unsafe { b::logFatal(c"%s".as_ptr(), c.as_ptr()) };
        log_fatal(&mut d.game, &message);
        let (cc, rc) = (d.c_calls(), d.rust_calls());
        prop_assert_eq!(cc.len(), 3);
        prop_assert_eq!(cc, rc);
    }
}

#[test]
fn initial_state_matches_c() {
    let d = Diff::new();
    assert_eq!(diff::load::<logging::State>(), d.game.logging);
}

#[test]
fn mirror_round_trip() {
    let d = Diff::new();
    d.store_to_c();
    assert_eq!(diff::load::<logging::State>(), d.game.logging);
}
