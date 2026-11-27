//! Shared setup for the differential tests of the netplay and wait loops
//! (`network`, `waits`): two engines on scripted, recorded fake platforms with
//! loopback networking, the freeware data and separate user directories.

#![allow(dead_code)]

use rustyrian_core::Game;
use rustyrian_core::game::{Flow, Halt};
use rustyrian_core::platform::{IpAddress, Platform, UdpSocketId};
use rustyrian_core::{config, episodes, keyboard, network, palette, sprite, video};
use rustyrian_harness::fake::{Datagram, FakeConfig, FakeHandle, FakePlatform};
use rustyrian_harness::trace::PlatformCall;
use rustyrian_harness::{CallLog, InstrumentConfig, Instrumented, Session, script, test_session};
use rustyrian_oracle::mirror::Mirror;
use rustyrian_oracle::{bindings as b, diff, guard_call, mirror, session};
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

/// The opponent: the address channel 0 is bound to by [`Rig::net`].
pub const PEER: IpAddress = IpAddress {
    host: 0x0100_007f,
    port: 0x3605,
};

/// An input script header plus a RETURN tap every second from `from_ms` to
/// `until_ms`, so every wait for input ends.
pub fn taps(from_ms: u32, until_ms: u32) -> String {
    let mut s = String::from("ots 1\n");
    let mut t = from_ms;
    while t <= until_ms {
        s.push_str(&format!(
            "@{t} key-down RETURN\n@{} key-up RETURN\n",
            t + 45
        ));
        t += 1000;
    }
    s
}

fn scripted_session(script: &str) -> (Session, CallLog, FakeHandle) {
    let script = script::parse(script).expect("valid input script");
    let mut s = Instrumented::new(
        FakePlatform::new(FakeConfig::default()),
        InstrumentConfig {
            script: Some(script),
            ..InstrumentConfig::default()
        },
    )
    .expect("a session without outputs cannot fail to open");
    let log = CallLog::default();
    s.recorder.log = Some(log.clone());
    let handle = s.inner().handle();
    (s, log, handle)
}

pub fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data")
        .canonicalize()
        .expect("data/ (run `cargo xtask fetch-data`)")
}

/// A fresh scratch directory.
pub fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir()
        .join(format!("rustyrian-net-{}", std::process::id()))
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("scratch dir");
    d
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
    pub c_fake: FakeHandle,
    pub r_fake: FakeHandle,
    c_dir: PathBuf,
    r_dir: PathBuf,
}

/// A C call's result, or its exit code.
pub type Outcome<T> = Result<T, i32>;

impl Rig {
    /// Fresh engines on `script`, with game data in `data` and separate user
    /// directories.
    pub fn with_data(script: &str, data: &Path) -> Rig {
        let (c, c_log, c_fake) = scripted_session(script);
        let (r, r_log, r_fake) = scripted_session(script);
        session::install(c);
        let mut rig = Rig {
            g: Game::new(Box::new(r)),
            c_log,
            r_log,
            c_fake,
            r_fake,
            c_dir: tmp("user-c"),
            r_dir: tmp("user-r"),
        };
        rig.g.file.custom_data_dir_path = Some(data.as_os_str().as_bytes().to_vec());
        let f = &mut rig.g.file;
        set_dir(&mut f.user_dir_path, &mut f.user_dir_path_len, &rig.r_dir);
        rig.store();
        rig
    }

    pub fn new(script: &str) -> Rig {
        Rig::with_data(script, &data_dir())
    }

    /// Copies the Rust state into C (C keeps its own user directory).
    pub fn store(&mut self) {
        let f = &mut self.g.file;
        set_dir(&mut f.user_dir_path, &mut f.user_dir_path_len, &self.c_dir);
        mirror::store_game(&self.g);
        let f = &mut self.g.file;
        set_dir(&mut f.user_dir_path, &mut f.user_dir_path_len, &self.r_dir);
    }

    /// Forgets the calls and datagrams so far.
    pub fn clear(&self) {
        self.c_log.take();
        self.r_log.take();
        self.c_fake.take_sent();
        self.r_fake.take_sent();
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

    /// `network_init` on both engines, channel 0 bound to [`PEER`]; returns
    /// the socket.
    pub fn net(&mut self) -> UdpSocketId {
        // SAFETY: plain C call.
        let c = unsafe { b::network_init() };
        assert_eq!(c, network::network_init(&mut self.g));
        assert_eq!(c, 0);
        let socket = self.g.network.socket.expect("socket");
        self.both(|p| p.net_udp_bind(socket, 0, &PEER));
        self.clear();
        socket
    }

    /// Queues `data` from [`PEER`] on both engines.
    pub fn push(&self, data: &[u8]) {
        self.push_from(PEER, data);
    }

    pub fn push_from(&self, address: IpAddress, data: &[u8]) {
        let socket = self.g.network.socket.expect("socket");
        for f in [&self.c_fake, &self.r_fake] {
            f.push_datagram(Datagram {
                socket,
                address,
                data: data.to_vec(),
            });
        }
    }

    /// Asserts equal calls and sent datagrams; returns the datagrams.
    pub fn check_io(&self) -> Vec<Datagram> {
        assert_eq!(self.c_calls(), self.rust_calls(), "platform calls differ");
        let sent = self.c_fake.take_sent();
        assert_eq!(sent, self.r_fake.take_sent(), "sent datagrams differ");
        sent
    }

    /// After a run that returned: equal calls, datagrams and whole states.
    pub fn check(&mut self) {
        self.check_io();
        let (p, _) = test_session();
        let mut c = Game::new(Box::new(p));
        mirror::load_game(&mut c);
        c.file.user_dir_path = self.g.file.user_dir_path.clone();
        c.file.user_dir_path_len = self.g.file.user_dir_path_len;
        if let Some(d) = diff::first_difference(&c.snapshot(), &self.g.snapshot()) {
            panic!("states differ at {d}");
        }
    }

    /// After a run that exited through `JE_tyrianHalt` (C's freed surfaces
    /// cannot be loaded): equal calls, datagrams, saved files and the states
    /// that stay valid.
    pub fn check_halted(&mut self) {
        forget_freed_c();
        self.check_io();
        assert_eq!(diff::load::<network::State>(), self.g.network);
        assert_eq!(diff::load::<palette::State>(), self.g.palette);
        assert_eq!(diff::load::<keyboard::State>(), self.g.keyboard);
        assert_eq!(diff::load::<config::State>(), self.g.config);
        assert_eq!(diff::load::<episodes::State>(), self.g.episodes);
        for f in ["tyrian.sav", "tyrian.cfg", "opentyrian.cfg"] {
            assert_eq!(
                std::fs::read(self.c_dir.join(f)).ok(),
                std::fs::read(self.r_dir.join(f)).ok(),
                "{f}"
            );
        }
    }
}

/// `JE_tyrianHalt` frees these blocks but leaves the pointers set; clearing
/// them keeps the next test case's `store_game` from freeing them again.
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

/// Stores `s` as the network state of both engines.
pub fn store_net(rig: &mut Rig, s: network::State) {
    s.store_to_c();
    rig.g.network = s;
}
