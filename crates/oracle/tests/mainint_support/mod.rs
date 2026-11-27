//! Shared setup for the `mainint` differential tests: both engines on the
//! same timed input script, with the freeware data, separate user
//! directories and a per-engine trace, so calls, states, saved files and
//! every presented frame can be compared.

#![allow(dead_code)]

use rustyrian_core::Game;
use rustyrian_core::game::{Flow, Halt};
use rustyrian_core::platform::Platform;
use rustyrian_core::{sprite, video};
use rustyrian_harness::fake::{FakeConfig, FakePlatform};
use rustyrian_harness::trace::PlatformCall;
use rustyrian_harness::{CallLog, InstrumentConfig, Instrumented, Session, script, test_session};
use rustyrian_oracle::{bindings as b, diff, guard_call, mirror, session};
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

pub fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data")
        .canonicalize()
        .expect("data/ (run `cargo xtask fetch-data`)")
}

/// A fresh scratch directory.
pub fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir()
        .join(format!("rustyrian-mainint-{}", std::process::id()))
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("scratch dir");
    d
}

/// A script: the header, then `(ms, event)` lines.
pub fn script(events: &[(u32, &str)]) -> String {
    let mut s = String::from("ots 1\n");
    for (t, e) in events {
        s.push_str(&format!("@{t} {e}\n"));
    }
    s
}

/// A key tap at `ms` (down, then up 45 ms later).
pub fn tap(ms: u32, key: &str) -> [(u32, String); 2] {
    [
        (ms, format!("key-down {key}")),
        (ms + 45, format!("key-up {key}")),
    ]
}

/// A script of key taps, one per `(ms, key)`, plus extra raw events.
pub fn taps(keys: &[(u32, &str)], extra: &[(u32, &str)]) -> String {
    let mut events: Vec<(u32, String)> = keys.iter().flat_map(|&(t, k)| tap(t, k)).collect();
    events.extend(extra.iter().map(|&(t, e)| (t, e.to_owned())));
    events.sort_by_key(|e| e.0);
    let mut s = String::from("ots 1\n");
    for (t, e) in events {
        s.push_str(&format!("@{t} {e}\n"));
    }
    s
}

fn session_for(
    script: &str,
    engine: &str,
    trace: &Path,
    redact: Vec<(String, String)>,
) -> (Session, CallLog) {
    let script = script::parse(script).expect("valid input script");
    let mut s = Instrumented::new(
        FakePlatform::new(FakeConfig::default()),
        InstrumentConfig {
            engine: engine.to_owned(),
            script: Some(script),
            trace_out: Some(trace.to_owned()),
            redact,
            ..InstrumentConfig::default()
        },
    )
    .expect("trace file opens");
    let log = CallLog::default();
    s.recorder.log = Some(log.clone());
    (s, log)
}

fn set_dir(slot: &mut Option<Vec<u8>>, len: &mut usize, dir: &Path) {
    let bytes = dir.as_os_str().as_bytes().to_vec();
    *len = bytes.len();
    *slot = Some(bytes);
}

/// Two engines driven by the same input script.
pub struct Rig {
    pub g: Box<Game>,
    c_log: CallLog,
    r_log: CallLog,
    pub c_dir: PathBuf,
    pub r_dir: PathBuf,
    c_trace: PathBuf,
    r_trace: PathBuf,
}

/// A C call's result, or its exit code.
pub type Outcome<T> = Result<T, i32>;

impl Rig {
    pub fn new(script: &str) -> Rig {
        let base = tmp("rig");
        let (c_dir, r_dir) = (base.join("user-c"), base.join("user-r"));
        std::fs::create_dir_all(&c_dir).expect("user dir");
        std::fs::create_dir_all(&r_dir).expect("user dir");
        let (c_trace, r_trace) = (base.join("c.jsonl"), base.join("r.jsonl"));
        let redact = |dir: &Path| vec![(dir.display().to_string(), "<user>".to_owned())];
        let (c, c_log) = session_for(script, "c", &c_trace, redact(&c_dir));
        let (r, r_log) = session_for(script, "rust", &r_trace, redact(&r_dir));
        session::install(c);
        let mut rig = Rig {
            g: Game::new(Box::new(r)),
            c_log,
            r_log,
            c_dir,
            r_dir,
            c_trace,
            r_trace,
        };
        rig.g.file.custom_data_dir_path = Some(data_dir().as_os_str().as_bytes().to_vec());
        rig.store();
        rig
    }

    /// Both engines in the fixture's state, with a fresh window (scaler
    /// "None", so window and screen coordinates agree) and the main shape
    /// tables loaded.
    pub fn from_fixture(script: &str, frame: u64) -> Rig {
        let mut rig = Rig::new(script);
        rig.g.restore(diff::fixture("demo-cycle", frame));
        rig.g.file.custom_data_dir_path = Some(data_dir().as_os_str().as_bytes().to_vec());
        rig.g.video_scale.scaler = 0;
        // The fixture's audio device belongs to the oracle runner's session.
        rig.g.loudness.audio_disabled = true;
        rig.store();
        rig.video();
        rig
    }

    /// Copies the Rust state into C (C keeps its own user directory).
    pub fn store(&mut self) {
        let f = &mut self.g.file;
        set_dir(&mut f.user_dir_path, &mut f.user_dir_path_len, &self.c_dir);
        mirror::store_game(&self.g);
        let f = &mut self.g.file;
        set_dir(&mut f.user_dir_path, &mut f.user_dir_path_len, &self.r_dir);
    }

    /// Forgets the calls so far.
    pub fn clear(&self) {
        self.c_log.take();
        self.r_log.take();
    }

    pub fn c_calls(&self) -> Vec<PlatformCall> {
        self.c_log.take()
    }

    pub fn rust_calls(&self) -> Vec<PlatformCall> {
        self.r_log.take()
    }

    /// Runs the same platform call on both fake platforms.
    pub fn both<R: PartialEq + std::fmt::Debug>(
        &mut self,
        f: impl Fn(&mut dyn Platform) -> R,
    ) -> R {
        let c = session::with(|s| f(s));
        let r = f(&mut *self.g.platform);
        assert_eq!(c, r);
        r
    }

    /// Runs `c` (C, `exit` caught) and `r` (Rust) and checks that they agree.
    pub fn run<T: PartialEq + std::fmt::Debug>(
        &mut self,
        c: impl FnOnce() -> T,
        r: impl FnOnce(&mut Game) -> Flow<T>,
    ) -> Outcome<T> {
        let mut out = None;
        // SAFETY: C runs on the test thread; `exit` is caught by the guard.
        let code = unsafe { guard_call(|| out = Some(c())) };
        let c = match code {
            Some(code) => Err(code),
            None => Ok(out.expect("returned normally")),
        };
        let r = r(&mut self.g).map_err(|Halt { code }| code);
        assert_eq!(c, r, "results differ");
        r
    }

    /// `init_video` and the main shape tables on both engines.
    pub fn video(&mut self) {
        assert_eq!(
            // SAFETY: plain C call inside the guard.
            self.run(|| unsafe { b::init_video() }, video::init_video),
            Ok(())
        );
        let name = CString::new("tyrian.shp").expect("no NUL");
        // SAFETY: a valid C string.
        unsafe { b::JE_loadMainShapeTables(name.as_ptr()) };
        sprite::je_load_main_shape_tables(&mut self.g, "tyrian.shp").expect("shape tables");
        self.clear();
    }

    /// The whole C state, as core types (user directory as Rust's).
    pub fn c_state(&self) -> rustyrian_core::game::Snapshot {
        let (p, _) = test_session();
        let mut c = Game::new(Box::new(p));
        mirror::load_game(&mut c);
        c.file.user_dir_path = self.g.file.user_dir_path.clone();
        c.file.user_dir_path_len = self.g.file.user_dir_path_len;
        c.snapshot()
    }

    /// Equal calls, whole states and saved files.
    pub fn check(&mut self) {
        assert_eq!(self.c_calls(), self.rust_calls(), "platform calls differ");
        if let Some(d) = diff::first_difference(&self.c_state(), &self.g.snapshot()) {
            panic!("states differ at {d}");
        }
        self.check_files();
    }

    /// Equal calls and files, for runs that exited (C's freed surfaces cannot
    /// be loaded).
    pub fn check_halted(&mut self) {
        assert_eq!(self.c_calls(), self.rust_calls(), "platform calls differ");
        self.check_files();
    }

    pub fn check_files(&self) {
        for f in ["tyrian.sav", "tyrian.cfg", "opentyrian.cfg"] {
            assert_eq!(
                std::fs::read(self.c_dir.join(f)).ok(),
                std::fs::read(self.r_dir.join(f)).ok(),
                "{f}"
            );
        }
    }

    /// Ends both sessions and compares every presented frame (screen,
    /// palette, texture, audio and call hashes, and the clock); returns the
    /// number of frames.
    pub fn check_frames(self) -> usize {
        drop(session::take());
        let Rig {
            g,
            c_trace,
            r_trace,
            ..
        } = self;
        drop(g);
        let frames = |p: &Path| -> Vec<String> {
            std::fs::read_to_string(p)
                .expect("trace written")
                .lines()
                .filter(|l| l.starts_with("{\"frame\""))
                .map(str::to_owned)
                .collect()
        };
        let (c, r) = (frames(&c_trace), frames(&r_trace));
        assert_eq!(c.len(), r.len(), "frame counts differ");
        for (i, (c, r)) in c.iter().zip(&r).enumerate() {
            assert_eq!(c, r, "frame {i} differs");
        }
        c.len()
    }
}

/// `JE_tyrianHalt` frees these blocks but leaves the pointers set; clearing
/// them keeps a later `store_game` from freeing them again.
pub fn forget_freed_c() {
    // SAFETY: plain writes of C globals on the test thread.
    unsafe {
        *mirror::global_mut(&raw mut b::VGAScreen) = std::ptr::null_mut();
        *mirror::global_mut(&raw mut b::VGAScreenSeg) = std::ptr::null_mut();
        *mirror::global_mut(&raw mut b::VGAScreen2) = std::ptr::null_mut();
        *mirror::global_mut(&raw mut b::game_screen) = std::ptr::null_mut();
        *mirror::global_mut(&raw mut b::joystick) = std::ptr::null_mut();
        for p in mirror::global_mut(&raw mut b::soundSamples).iter_mut() {
            *p = std::ptr::null_mut();
        }
    }
}
