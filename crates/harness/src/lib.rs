// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) rustyrian contributors

//! Headless platform, input scripts, virtual clock and traces.
//!
//! `Session` (= `HeadlessPlatform`) is the single object behind both the C
//! oracle's SDL shim and the Rust port's headless mode, so both engines see the
//! same input, clock and platform return values and produce comparable
//! `PlatformCall` logs.
#![forbid(unsafe_code)]

pub mod fake;
pub mod instrumented;
pub mod keys;
pub mod scancode_names;
pub mod script;
pub mod trace;

pub use fake::{Clock, FakeConfig, FakePlatform, NetBackend};
pub use instrumented::{End, InstrumentConfig, Instrumented, Recorder};

/// A scripted, recorded fake platform.
pub type Session = Instrumented<FakePlatform>;

/// The Rust port's headless platform.
pub type HeadlessPlatform = Session;

/// A session with no script, outputs or limits, keeping every call record.
pub fn test_session() -> Session {
    let mut s = Instrumented::new(
        FakePlatform::new(FakeConfig::default()),
        InstrumentConfig::default(),
    )
    .expect("a session without outputs cannot fail to open");
    s.recorder.keep_calls = true;
    s
}
