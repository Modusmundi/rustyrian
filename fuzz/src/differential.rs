//! Differential fuzzing against the C oracle (plan step 9): the same input
//! through the upstream C function and its Rust port; their exit codes,
//! platform calls and whole game states must match.
//!
//! The C side runs in a child process forked per input, so a C crash ends
//! only the child, and every input starts from the C globals of a fresh
//! process (C allocations and statics never leak from one input into the
//! next). Built with `OT_REF_SANITIZE=1`, the oracle is AddressSanitizer- and
//! UBSan-instrumented; an input the sanitizers flag (the child dies) is
//! skipped, as are inputs on which C does not finish within
//! [`CHILD_DEADLINE`]. Everything else must agree; a difference panics in the
//! fuzzer process, which reports the input.
//!
//! Run with the sanitized oracle (Linux, as the nightly `fuzz-diff` job does):
//! `OT_REF_SANITIZE=1 ASAN_OPTIONS=detect_leaks=0:allocator_may_return_null=1:verify_asan_link_order=0
//! UBSAN_OPTIONS=halt_on_error=1 cargo +nightly fuzz run -s none --features differential
//! diff_<target> -- -timeout=5 -malloc_limit_mb=1048576`.
//! `-s none`: the oracle links clang's shared ASan runtime, which cannot
//! share the process with rustc's; the Rust side is safe code. The malloc
//! limit is lifted because libFuzzer's malloc hook would otherwise report
//! the C side's huge (NULL-returning) requests from the child.
//! Without `OT_REF_SANITIZE` (for example on macOS, where `build.rs` gives the
//! binaries their SDL3 rpath), `cargo +nightly fuzz run --features
//! differential diff_<target> -- -timeout=5` compares without sanitizers;
//! only C crashes are skipped then.

use rustyrian_core::Game;
use rustyrian_core::game::{Flow, Halt, Snapshot};
use rustyrian_harness::script::{self, Script};
use rustyrian_harness::{
    CallLog, FakeConfig, FakePlatform, InstrumentConfig, Instrumented, Session,
};
use rustyrian_oracle::{diff, mirror, session};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::ffi::{c_int, c_void};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// How long the C side of one input may run.
pub const CHILD_DEADLINE: Duration = Duration::from_millis(1500);

/// The child's exit status when its Rust harness code panicked (as opposed
/// to a sanitizer report, which exits with 1, or a signal).
const CHILD_PANICKED: c_int = 3;

static COMPARED: AtomicU64 = AtomicU64::new(0);
static FLAGGED: AtomicU64 = AtomicU64::new(0);
static TIMED_OUT: AtomicU64 = AtomicU64::new(0);

/// Why the C side produced no result.
#[derive(Debug)]
pub enum Skip {
    /// The child died: a sanitizer report or a crash.
    Flagged,
    /// The child did not finish within [`CHILD_DEADLINE`].
    TimedOut,
}

fn count(skip: Option<&Skip>) {
    let counter = match skip {
        None => &COMPARED,
        Some(Skip::Flagged) => &FLAGGED,
        Some(Skip::TimedOut) => &TIMED_OUT,
    };
    let n = counter.fetch_add(1, Ordering::Relaxed) + 1;
    if n.is_power_of_two() {
        eprintln!(
            "differential: {} compared, {} skipped (C flagged by a sanitizer or crashed), {} skipped (C timed out)",
            COMPARED.load(Ordering::Relaxed),
            FLAGGED.load(Ordering::Relaxed),
            TIMED_OUT.load(Ordering::Relaxed),
        );
    }
}

/// In the child: report panics as [`CHILD_PANICKED`] instead of through the
/// fuzzer's hook, and keep a sanitizer report from running libFuzzer's death
/// callback (which would save the input as a crash of the parent).
fn disarm_fuzzer() {
    std::panic::set_hook(Box::new(|info| {
        eprintln!("differential: C side harness panicked: {info}");
        // SAFETY: ends the child without unwinding into libFuzzer.
        unsafe { libc::_exit(CHILD_PANICKED) }
    }));
    let name = c"__sanitizer_set_death_callback";
    // SAFETY: a symbol lookup in the loaded images.
    let f = unsafe { libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr()) };
    if !f.is_null() {
        // SAFETY: the sanitizer interface's
        // `void __sanitizer_set_death_callback(void (*)(void))`.
        let set: unsafe extern "C" fn(Option<extern "C" fn()>) = unsafe { std::mem::transmute(f) };
        // SAFETY: clearing the callback is always valid.
        unsafe { set(None) };
    }
}

fn write_all(fd: c_int, mut bytes: &[u8]) {
    while !bytes.is_empty() {
        // SAFETY: `bytes` is a valid buffer of its length.
        let n = unsafe { libc::write(fd, bytes.as_ptr().cast::<c_void>(), bytes.len()) };
        if n <= 0 {
            // SAFETY: ends the child.
            unsafe { libc::_exit(CHILD_PANICKED) }
        }
        bytes = &bytes[n as usize..];
    }
}

/// Reads `fd` to its end, or gives up at `deadline`.
fn read_until(fd: c_int, deadline: Instant) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut buf = [0u8; 1 << 16];
    loop {
        let left = deadline.checked_duration_since(Instant::now())?;
        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let ms = c_int::try_from(left.as_millis())
            .unwrap_or(c_int::MAX)
            .max(1);
        // SAFETY: one valid pollfd.
        let ready = unsafe { libc::poll(&raw mut pfd, 1, ms) };
        if ready < 0 {
            if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return None;
        }
        if ready == 0 {
            continue;
        }
        // SAFETY: `buf` is a valid buffer of its length.
        let n = unsafe { libc::read(fd, buf.as_mut_ptr().cast::<c_void>(), buf.len()) };
        match n {
            0 => return Some(out),
            n if n > 0 => out.extend_from_slice(&buf[..n as usize]),
            _ if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted => {}
            _ => return None,
        }
    }
}

/// Runs `f` in a forked child and returns its result.
///
/// # Panics
/// If the child's harness code panicked (a bug in the comparison or the
/// mirrors, or C state the mirrors cannot represent).
pub fn in_child<T: Serialize + DeserializeOwned>(f: impl FnOnce() -> T) -> Result<T, Skip> {
    let mut fds = [0 as c_int; 2];
    // SAFETY: a valid two-element array.
    assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0, "pipe");
    let [rd, wr] = fds;
    // SAFETY: the child only runs `f` and exits with `_exit`.
    let pid = unsafe { libc::fork() };
    assert!(pid >= 0, "fork");
    if pid == 0 {
        // SAFETY: the child's copy of the read end.
        unsafe { libc::close(rd) };
        disarm_fuzzer();
        let bytes = postcard::to_stdvec(&f()).expect("the result serializes");
        write_all(wr, &bytes);
        // SAFETY: ends the child without running the fuzzer's exit handlers.
        unsafe { libc::_exit(0) }
    }
    // SAFETY: the parent's copy of the write end.
    unsafe { libc::close(wr) };
    let bytes = read_until(rd, Instant::now() + CHILD_DEADLINE);
    // SAFETY: the read end, no longer used.
    unsafe { libc::close(rd) };
    if bytes.is_none() {
        // SAFETY: our child.
        unsafe { libc::kill(pid, libc::SIGKILL) };
    }
    let mut status: c_int = 0;
    // SAFETY: waits for our child.
    while unsafe { libc::waitpid(pid, &raw mut status, 0) } < 0 {}
    let result = match bytes {
        None => Err(Skip::TimedOut),
        Some(_) if libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == CHILD_PANICKED => {
            panic!("the C side's harness code panicked (its message is above)")
        }
        Some(b) if libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0 => {
            Ok(postcard::from_bytes(&b).expect("the child's result decodes"))
        }
        Some(_) => Err(Skip::Flagged),
    };
    count(result.as_ref().err());
    result
}

/// A recorded fake-platform session, optionally driven by an input script.
pub fn session(script: Option<&Script>) -> (Session, CallLog) {
    let mut s = Instrumented::new(
        FakePlatform::new(FakeConfig::default()),
        InstrumentConfig {
            script: script.cloned(),
            ..InstrumentConfig::default()
        },
    )
    .expect("a session without outputs cannot fail to open");
    let log = CallLog::default();
    s.recorder.log = Some(log.clone());
    (s, log)
}

/// Parses an input script known to be valid.
pub fn parse_script(text: &str) -> Script {
    script::parse(text).expect("a valid input script")
}

/// What one engine did with an input.
#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Outcome {
    /// The exit code, if the engine exited.
    pub exit: Option<i32>,
    /// Every platform call, as JSON.
    pub calls: Vec<String>,
    pub state: Snapshot,
}

fn calls_json(log: &CallLog) -> Vec<String> {
    log.take()
        .iter()
        .map(|c| serde_json::to_string(c).expect("calls serialize"))
        .collect()
}

/// The differential check of one input: `setup` builds the starting state of
/// both engines (stored into C's globals), then `c` (the C calls, `exit`
/// caught) and `rust` (the port) run on it. Exit codes, platform calls and
/// whole states must be equal.
///
/// `c` runs between `setjmp` and a possible `longjmp`: it must hold no
/// values with destructors (capture raw pointers and `Copy` data).
pub fn check_game(
    script: Option<&Script>,
    setup: impl Fn(&mut Game),
    c: impl FnOnce(),
    rust: impl FnOnce(&mut Game) -> Flow,
) {
    let from_c = in_child(|| {
        let (s, log) = session(script);
        session::install(s);
        let (start, _) = session(None);
        let mut g = Game::new(Box::new(start));
        setup(&mut g);
        mirror::store_game(&g);
        // SAFETY: `c` holds no values with destructors (see above).
        let exit = unsafe { rustyrian_oracle::guard_call(c) };
        let calls = calls_json(&log);
        let (empty, _) = session(None);
        let mut state = Game::new(Box::new(empty));
        mirror::load_game(&mut state);
        Outcome {
            exit,
            calls,
            state: state.snapshot(),
        }
    });
    let Ok(c) = from_c else {
        return;
    };

    let (s, log) = session(script);
    let mut g = Game::new(Box::new(s));
    setup(&mut g);
    let exit = rust(&mut g).err().map(|Halt { code }| code);
    let r = Outcome {
        exit,
        calls: calls_json(&log),
        state: g.snapshot(),
    };

    assert_eq!(c.exit, r.exit, "exit codes differ (C, Rust)");
    if let Some(i) =
        (0..c.calls.len().max(r.calls.len())).find(|&i| c.calls.get(i) != r.calls.get(i))
    {
        panic!(
            "platform call {i} differs:\n  C:    {:?}\n  Rust: {:?}",
            c.calls.get(i),
            r.calls.get(i)
        );
    }
    if let Some(d) = diff::first_difference(&c.state, &r.state) {
        panic!("states differ at {d}");
    }
}
