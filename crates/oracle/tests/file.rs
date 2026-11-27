//! Differential tests for `file.c` / `file.h`.

use proptest::prelude::*;
use rustyrian_core::Game;
use rustyrian_core::file::{self as f, File};
use rustyrian_harness::trace::PlatformCall;
use rustyrian_harness::{
    CallLog, FakeConfig, FakePlatform, InstrumentConfig, Instrumented, Session,
};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;
use rustyrian_oracle::mirror::file::{file_from_c, file_to_c};
use rustyrian_oracle::session;
use std::ffi::{CStr, CString, c_char, c_void};
use std::path::{Path, PathBuf};

/// A scratch directory for this test process.
fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir()
        .join(format!("rustyrian-file-{}", std::process::id()))
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("scratch dir");
    d
}

fn cs(p: impl AsRef<[u8]>) -> CString {
    CString::new(p.as_ref().to_vec()).expect("no NUL")
}

fn bytes(p: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    p.as_os_str().as_bytes().to_vec()
}

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data")
        .canonicalize()
        .expect("data/ (run `cargo xtask fetch-data`)")
}

/// `(open, errnum, error, position)` of a C `File`.
fn c_view(file: &b::File) -> (bool, i32, bool, i64) {
    let pos = if file.f.is_null() {
        -1
    } else {
        // SAFETY: an open stream.
        unsafe { libc::ftell(file.f.cast()) }
    };
    (!file.f.is_null(), file.errnum, file.error, pos)
}

/// The same view of a Rust `File`.
fn r_view(file: &File) -> (bool, i32, bool, i64) {
    let pos = file.f.as_ref().map_or(-1, |s| s.tell().unwrap_or(-1));
    (file.f.is_some(), file.errnum, file.error, pos)
}

/// A C session and a Rust game on fake platforms with `base_path`, plus their
/// call logs.
fn sessions(base_path: Option<String>) -> (Box<Game>, CallLog, CallLog) {
    let make = |log: &CallLog| -> Session {
        let mut s = Instrumented::new(
            FakePlatform::new(FakeConfig {
                base_path: base_path.clone(),
                ..FakeConfig::default()
            }),
            InstrumentConfig::default(),
        )
        .expect("session");
        s.recorder.log = Some(log.clone());
        s
    };
    let (c_log, r_log) = (CallLog::default(), CallLog::default());
    session::install(make(&c_log));
    let g = Game::new(Box::new(make(&r_log)));
    // Fresh C `file.c` state too (earlier cases in this process changed it).
    g.file.store_to_c();
    (g, c_log, r_log)
}

fn load_c() -> f::State {
    diff::load::<f::State>()
}

#[test]
fn initial_state_matches_c() {
    let d = Diff::new();
    assert_eq!(load_c(), d.game.file);
}

#[test]
fn mirror_round_trip() {
    let mut d = Diff::new();
    d.game.file = f::State {
        custom_data_dir_path: Some(b"/x/data".to_vec()),
        data_dir_path: Some(b"/y".to_vec()),
        data_dir_path_len: 2,
        user_dir_path: Some(b"/home/u/.config/z".to_vec()),
        user_dir_path_len: 17,
        find_data_files_base_data_dir_path: Some(b"/base/data".to_vec()),
    };
    d.store_to_c();
    assert_eq!(load_c(), d.game.file);
}

#[test]
fn file_mirror_helpers_round_trip() {
    let dir = tmp("helpers");
    let p = dir.join("a.bin");
    std::fs::write(&p, b"0123456789").expect("write");
    let mut r = f::file_open(&bytes(&p), "rb");
    f::file_set_position(&mut r, 7);
    let mut c = b::File::default();
    // SAFETY: `c` is a plain struct; the stream is ours.
    unsafe { file_to_c(&r, &mut c) };
    assert_eq!(c_view(&c), r_view(&r));
    // SAFETY: `c.f` is open.
    let back = unsafe { file_from_c(&c) };
    assert_eq!(back, r);
    assert_eq!(r_view(&back), (true, 0, false, 7));
    let closed = File {
        f: None,
        errnum: -1,
        error: true,
    };
    // SAFETY: as above.
    unsafe { file_to_c(&closed, &mut c) };
    // SAFETY: `c.f` is NULL.
    assert_eq!(unsafe { file_from_c(&c) }, closed);
}

#[test]
fn file_exists_matches() {
    let dir = tmp("exists");
    std::fs::write(dir.join("a"), b"x").expect("write");
    for p in [
        dir.join("a"),
        dir.join("missing"),
        dir.clone(),
        PathBuf::new(),
    ] {
        let c = cs(bytes(&p));
        // SAFETY: a valid C string.
        let cv = unsafe { b::ot_s_file_fileExists(c.as_ptr()) };
        assert_eq!(cv, f::file_exists(&bytes(&p)), "{}", p.display());
    }
}

#[test]
fn file_open_matches() {
    let dir = tmp("open");
    let modes = [
        "rb", "r", "wb", "w", "ab", "a", "r+b", "rb+", "w+", "a+", "x", "",
    ];
    for (k, mode) in modes.iter().enumerate() {
        for kind in ["existing", "missing", "dir"] {
            let path_for = |side: &str| {
                let p = dir.join(format!("{k}-{kind}-{side}"));
                match kind {
                    "existing" => std::fs::write(&p, b"hello").expect("write"),
                    "dir" => std::fs::create_dir_all(&p).expect("mkdir"),
                    _ => {}
                }
                p
            };
            let (pc, pr) = (path_for("c"), path_for("r"));
            let (cp, cm) = (cs(bytes(&pc)), cs(mode));
            // SAFETY: valid C strings.
            let mut c = unsafe { b::ot_s_file_fileOpen(cp.as_ptr(), cm.as_ptr()) };
            let mut r = f::file_open(&bytes(&pr), mode);
            assert_eq!(c_view(&c), r_view(&r), "mode {mode:?} on {kind}");
            // SAFETY: `c` came from fileOpen.
            unsafe { b::fileClose(&raw mut c) };
            f::file_close(&mut r);
            assert_eq!(c_view(&c), r_view(&r));
            assert_eq!(std::fs::read(&pc).ok(), std::fs::read(&pr).ok());
        }
    }
}

fn find(g: &mut Game) -> (bool, bool) {
    // SAFETY: plain C call.
    let c = unsafe { b::findDataFiles() };
    (c, f::find_data_files(g))
}

fn calls_eq(c: &CallLog, r: &CallLog) -> Vec<PlatformCall> {
    let (cc, rc) = (c.take(), r.take());
    assert_eq!(cc, rc);
    cc
}

#[test]
fn find_data_files_custom_dir() {
    for (dir, expect) in [(data_dir(), true), (tmp("nodata"), false)] {
        let (mut g, cl, rl) = sessions(None);
        g.file.custom_data_dir_path = Some(bytes(&dir));
        g.file.store_to_c();
        let (c, r) = find(&mut g);
        assert_eq!((c, r), (expect, expect));
        assert_eq!(load_c(), g.file);
        assert!(calls_eq(&cl, &rl).is_empty());
    }
}

#[test]
fn find_data_files_base_path() {
    // `<base>data` holds the data: found next to the executable.
    let root = data_dir().parent().expect("parent").to_path_buf();
    let (mut g, cl, rl) = sessions(Some(format!("{}/", root.display())));
    let (c, r) = find(&mut g);
    assert!(c && r);
    assert_eq!(load_c(), g.file);
    assert_eq!(calls_eq(&cl, &rl).len(), 1);
    // The base path is looked up once.
    let (c, r) = find(&mut g);
    assert!(c && r);
    assert!(calls_eq(&cl, &rl).is_empty());
    assert_eq!(load_c(), g.file);
}

#[test]
fn find_data_files_not_found() {
    // No data next to the executable, none in TYRIAN_DIR, none in the cwd.
    let empty = tmp("nobase");
    let (mut g, cl, rl) = sessions(Some(format!("{}/", empty.display())));
    let (c, r) = find(&mut g);
    assert_eq!((c, r), (false, false));
    assert_eq!(load_c(), g.file);
    assert_eq!(calls_eq(&cl, &rl).len(), 1);

    let (mut g, cl, rl) = sessions(None);
    let (c, r) = find(&mut g);
    assert_eq!((c, r), (false, false));
    assert_eq!(load_c(), g.file);
    assert_eq!(g.file.data_dir_path, Some(Vec::new()));
    assert_eq!(calls_eq(&cl, &rl).len(), 1);
}

#[test]
fn data_file_open_and_exists_match() {
    // First call finds the data directory (custom), later calls reuse it.
    let (mut g, cl, rl) = sessions(None);
    g.file.custom_data_dir_path = Some(bytes(&data_dir()));
    g.file.store_to_c();
    for name in ["tyrian1.lvl", "tyrian.hdt", "missing.dat", ""] {
        let (cn, cm) = (cs(name), cs("rb"));
        // SAFETY: valid C strings.
        let mut c = unsafe { b::dataFileOpen(cn.as_ptr(), cm.as_ptr()) };
        let mut r = f::data_file_open(&mut g, name, "rb");
        assert_eq!(c_view(&c), r_view(&r), "{name}");
        // SAFETY: from dataFileOpen.
        unsafe { b::fileClose(&raw mut c) };
        f::file_close(&mut r);
        // SAFETY: a valid C string.
        let ce = unsafe { b::dataFileExists(cn.as_ptr()) };
        assert_eq!(ce, f::data_file_exists(&mut g, name), "{name}");
        assert_eq!(load_c(), g.file);
    }
    assert!(calls_eq(&cl, &rl).is_empty());

    // Not found anywhere: files open relative to the working directory.
    let (mut g, cl, rl) = sessions(None);
    let (cn, cm) = (cs("Cargo.toml"), cs("rb"));
    // SAFETY: valid C strings.
    let c = unsafe { b::dataFileOpen(cn.as_ptr(), cm.as_ptr()) };
    let r = f::data_file_open(&mut g, "Cargo.toml", "rb");
    assert_eq!(c_view(&c), r_view(&r));
    assert_eq!(load_c(), g.file);
    calls_eq(&cl, &rl);
}

/// Sets an environment variable (`None` removes it).
fn set_env(name: &str, value: Option<&Path>) {
    // SAFETY: tests run one per process, on one thread.
    unsafe {
        match value {
            Some(v) => std::env::set_var(name, v),
            None => std::env::remove_var(name),
        }
    }
}

/// The C path with its last component `opentyrian`/`OpenTyrian` replaced by
/// `rustyrian` (divergence D-PRODUCT-1).
fn product_path(c: &[u8]) -> Vec<u8> {
    for name in [&b"opentyrian"[..], b"OpenTyrian"] {
        if c.ends_with(name) {
            let mut p = c[..c.len() - name.len()].to_vec();
            p.extend_from_slice(b"rustyrian");
            return p;
        }
    }
    c.to_vec()
}

#[test]
fn determine_user_dir_path() {
    let dir = tmp("userdir");
    // (base path, XDG_CONFIG_HOME, HOME, whether the C path gets renamed)
    type Case = (Option<String>, Option<PathBuf>, Option<PathBuf>, bool);
    let cases: [Case; 4] = [
        (None, Some(dir.join("xdg")), Some(dir.join("home")), true),
        (None, None, Some(dir.join("home")), true),
        (None, None, None, false),
        (
            Some(format!("{}/", dir.join("portable").display())),
            Some(dir.join("xdg")),
            None,
            false,
        ),
    ];
    std::fs::create_dir_all(dir.join("portable")).expect("mkdir");
    std::fs::write(dir.join("portable/opentyrian.cfg"), b"").expect("write");
    for (base, xdg, home, renamed) in cases {
        let (mut g, cl, rl) = sessions(base.clone());
        set_env("XDG_CONFIG_HOME", xdg.as_deref());
        set_env("HOME", home.as_deref());
        // SAFETY: plain C call.
        unsafe { b::ot_s_file_determineUserDirPath() };
        f::determine_user_dir_path(&mut g);
        let c = load_c();
        let c_path = c.user_dir_path.clone().expect("set");
        let expected = if renamed {
            product_path(&c_path)
        } else {
            c_path.clone()
        };
        assert_eq!(
            g.file.user_dir_path.as_deref(),
            Some(&expected[..]),
            "{base:?} {xdg:?} {home:?}"
        );
        assert_eq!(g.file.user_dir_path_len, expected.len());
        assert_eq!(c.user_dir_path_len, c_path.len());
        calls_eq(&cl, &rl);
        // Called again (non-empty path is freed and recomputed).
        // SAFETY: plain C call.
        unsafe { b::ot_s_file_determineUserDirPath() };
        f::determine_user_dir_path(&mut g);
        assert_eq!(load_c().user_dir_path, Some(c_path));
        assert_eq!(g.file.user_dir_path.as_deref(), Some(&expected[..]));
        calls_eq(&cl, &rl);
    }
}

#[test]
fn user_file_open_and_exists_match() {
    let dir = tmp("userfiles");
    set_env("XDG_CONFIG_HOME", Some(&dir));
    let (mut g, cl, rl) = sessions(None);
    for (name, mode) in [
        ("a.cfg", "rb"),
        ("a.cfg", "wb"),
        ("a.cfg", "rb"),
        ("sub/b", "wb"),
    ] {
        let (cn, cm) = (cs(name), cs(mode));
        // SAFETY: valid C strings.
        let ce = unsafe { b::userFileExists(cn.as_ptr()) };
        assert_eq!(ce, f::user_file_exists(&mut g, name), "{name}");
        // SAFETY: valid C strings.
        let mut c = unsafe { b::userFileOpen(cn.as_ptr(), cm.as_ptr()) };
        let mut r = f::user_file_open(&mut g, name, mode);
        assert_eq!(c_view(&c), r_view(&r), "{name} {mode}");
        // SAFETY: from userFileOpen.
        unsafe { b::fileClose(&raw mut c) };
        f::file_close(&mut r);
    }
    use std::os::unix::fs::PermissionsExt;
    for sub in ["opentyrian", "rustyrian"] {
        let m = std::fs::metadata(dir.join(sub)).expect("user dir created");
        assert_eq!(m.permissions().mode() & 0o777, 0o700);
    }
    assert_eq!(
        std::fs::read(dir.join("opentyrian/a.cfg")).ok(),
        std::fs::read(dir.join("rustyrian/a.cfg")).ok()
    );
    calls_eq(&cl, &rl);

    // No user directory at all: relative to the working directory.
    set_env("XDG_CONFIG_HOME", None);
    set_env("HOME", None);
    let (mut g, cl, rl) = sessions(None);
    let (cn, cm) = (cs("Cargo.toml"), cs("rb"));
    // SAFETY: valid C strings.
    let c = unsafe { b::userFileOpen(cn.as_ptr(), cm.as_ptr()) };
    let r = f::user_file_open(&mut g, "Cargo.toml", "rb");
    assert_eq!(c_view(&c), r_view(&r));
    calls_eq(&cl, &rl);
}

proptest! {
    #[test]
    fn file_get_error_matches(errnum in -1i32..120, error: bool) {
        let mut c = b::File { f: std::ptr::null_mut(), errnum, error };
        // SAFETY: fileGetError only reads `errnum`/`error`.
        let cv = unsafe { CStr::from_ptr(b::fileGetError(&raw mut c)) };
        let r = File { f: None, errnum, error };
        prop_assert_eq!(cv.to_string_lossy(), f::file_get_error(&r));
    }
}

/// One file operation, applied to both engines.
#[derive(Clone, Debug)]
enum Op {
    ReadExactly(usize),
    ReadAtMost(usize),
    SetPosition(i64),
    GetPosition,
    GetLength,
    Write(Vec<u8>),
    WriteU8(u8),
    WriteU8Array(Vec<u8>),
    WriteCharArray(Vec<u8>),
    Flush,
    U8,
    U16,
    U16Be,
    U32,
    Bool,
    Char,
    S8,
    S16,
    S32,
    U8Array(usize),
    U16Array(usize),
    U16BeArray(usize),
    U32Array(usize),
    BoolArray(usize),
    CharArray(usize),
    S8Array(usize),
    S16Array(usize),
    S32Array(usize),
}

fn any_op() -> impl Strategy<Value = Op> {
    let n = 0usize..24;
    let v = prop::collection::vec(any::<u8>(), 0..12);
    prop_oneof![
        n.clone().prop_map(Op::ReadExactly),
        n.clone().prop_map(Op::ReadAtMost),
        (-3i64..60).prop_map(Op::SetPosition),
        Just(Op::GetPosition),
        Just(Op::GetLength),
        v.clone().prop_map(Op::Write),
        any::<u8>().prop_map(Op::WriteU8),
        v.clone().prop_map(Op::WriteU8Array),
        v.prop_map(Op::WriteCharArray),
        Just(Op::Flush),
        Just(Op::U8),
        Just(Op::U16),
        Just(Op::U16Be),
        Just(Op::U32),
        Just(Op::Bool),
        Just(Op::Char),
        Just(Op::S8),
        Just(Op::S16),
        Just(Op::S32),
        n.clone().prop_map(Op::U8Array),
        n.clone().prop_map(Op::U16Array),
        n.clone().prop_map(Op::U16BeArray),
        n.clone().prop_map(Op::U32Array),
        n.clone().prop_map(Op::BoolArray),
        n.clone().prop_map(Op::CharArray),
        n.clone().prop_map(Op::S8Array),
        n.clone().prop_map(Op::S16Array),
        n.prop_map(Op::S32Array),
    ]
}

fn is_write(op: &Op) -> bool {
    matches!(
        op,
        Op::Write(_) | Op::WriteU8(_) | Op::WriteU8Array(_) | Op::WriteCharArray(_)
    )
}

fn is_read(op: &Op) -> bool {
    !is_write(op)
        && !matches!(
            op,
            Op::SetPosition(_) | Op::GetPosition | Op::GetLength | Op::Flush
        )
}

/// Bytes of a value slice.
fn raw<T: Copy>(v: &[T]) -> Vec<u8> {
    // SAFETY: `T` is a plain integer type (or bool, whose bytes are 0/1).
    unsafe {
        std::slice::from_raw_parts(v.as_ptr().cast::<u8>(), std::mem::size_of_val(v)).to_vec()
    }
}

/// Applies `op` to both files and returns each side's observable result.
fn apply(op: &Op, c: &mut b::File, r: &mut File) -> (Vec<u8>, Vec<u8>) {
    let c = &raw mut *c;
    macro_rules! scalar {
        ($cf:path, $rf:path) => {{
            // SAFETY: `c` is a valid File.
            let cv = unsafe { $cf(c) };
            (raw(&[cv]), raw(&[$rf(r)]))
        }};
    }
    macro_rules! array {
        ($n:expr, $t:ty, $cf:path, $rf:path) => {{
            let mut cv = vec![<$t>::default(); $n];
            // SAFETY: `cv` holds `n` elements.
            unsafe { $cf(c, cv.as_mut_ptr().cast(), $n as _) };
            let mut rv = vec![<$t>::default(); $n];
            $rf(r, &mut rv);
            (raw(&cv), raw(&rv))
        }};
    }
    // SAFETY (every block below): `c` is a valid File and the buffers hold
    // the sizes passed.
    match op {
        Op::ReadExactly(n) => {
            let (mut cv, mut rv) = (vec![0xaa; *n], vec![0xaa; *n]);
            // SAFETY: see the comment above the match.
            unsafe { b::fileReadExactly(c, cv.as_mut_ptr().cast::<c_void>(), *n) };
            f::file_read_exactly(r, &mut rv);
            (cv, rv)
        }
        Op::ReadAtMost(n) => {
            let (mut cv, mut rv) = (vec![0xaa; *n], vec![0xaa; *n]);
            // SAFETY: see the comment above the match.
            let cn = unsafe { b::fileReadAtMost(c, cv.as_mut_ptr().cast::<c_void>(), *n) };
            let rn = f::file_read_at_most(r, &mut rv);
            cv.extend(raw(&[cn]));
            rv.extend(raw(&[rn]));
            (cv, rv)
        }
        Op::SetPosition(p) => {
            // SAFETY: see the comment above the match.
            unsafe { b::fileSetPosition(c, *p as _) };
            f::file_set_position(r, *p);
            (vec![], vec![])
        }
        Op::GetPosition => {
            // SAFETY: see the comment above the match.
            let cv = unsafe { b::fileGetPosition(c) } as i64;
            (raw(&[cv]), raw(&[f::file_get_position(r)]))
        }
        Op::GetLength => {
            // SAFETY: see the comment above the match.
            let cv = unsafe { b::fileGetLength(c) } as i64;
            (raw(&[cv]), raw(&[f::file_get_length(r)]))
        }
        Op::Write(v) => {
            // SAFETY: see the comment above the match.
            unsafe { b::fileWrite(c, v.as_ptr().cast::<c_void>(), v.len()) };
            f::file_write(r, v);
            (vec![], vec![])
        }
        Op::WriteU8(v) => {
            // SAFETY: see the comment above the match.
            unsafe { b::ot_i_file_fileWriteU8(c, *v) };
            f::file_write_u8(r, *v);
            (vec![], vec![])
        }
        Op::WriteU8Array(v) => {
            let mut cv = v.clone();
            // SAFETY: see the comment above the match.
            unsafe { b::ot_i_file_fileWriteU8Array(c, cv.as_mut_ptr(), v.len() as _) };
            f::file_write_u8_array(r, v);
            (vec![], vec![])
        }
        Op::WriteCharArray(v) => {
            let mut cv = v.clone();
            // SAFETY: see the comment above the match.
            unsafe {
                b::ot_i_file_fileWriteCharArray(c, cv.as_mut_ptr().cast::<c_char>(), v.len() as _)
            };
            f::file_write_char_array(r, v);
            (vec![], vec![])
        }
        Op::Flush => {
            // SAFETY: see the comment above the match.
            unsafe { b::fileFlush(c) };
            f::file_flush(r);
            (vec![], vec![])
        }
        Op::U8 => scalar!(b::ot_i_file_fileReadU8, f::file_read_u8),
        Op::U16 => scalar!(b::ot_i_file_fileReadU16, f::file_read_u16),
        Op::U16Be => scalar!(b::ot_i_file_fileReadU16BE, f::file_read_u16be),
        Op::U32 => scalar!(b::ot_i_file_fileReadU32, f::file_read_u32),
        Op::Bool => scalar!(b::ot_i_file_fileReadBool, f::file_read_bool),
        Op::Char => scalar!(b::ot_i_file_fileReadChar, f::file_read_char),
        Op::S8 => scalar!(b::ot_i_file_fileReadS8, f::file_read_s8),
        Op::S16 => scalar!(b::ot_i_file_fileReadS16, f::file_read_s16),
        Op::S32 => scalar!(b::ot_i_file_fileReadS32, f::file_read_s32),
        Op::U8Array(n) => array!(*n, u8, b::ot_i_file_fileReadU8Array, f::file_read_u8_array),
        Op::U16Array(n) => array!(
            *n,
            u16,
            b::ot_i_file_fileReadU16Array,
            f::file_read_u16_array
        ),
        Op::U16BeArray(n) => array!(
            *n,
            u16,
            b::ot_i_file_fileReadU16BEArray,
            f::file_read_u16be_array
        ),
        Op::U32Array(n) => array!(
            *n,
            u32,
            b::ot_i_file_fileReadU32Array,
            f::file_read_u32_array
        ),
        Op::BoolArray(n) => array!(
            *n,
            bool,
            b::ot_i_file_fileReadBoolArray,
            f::file_read_bool_array
        ),
        Op::CharArray(n) => array!(
            *n,
            u8,
            b::ot_i_file_fileReadCharArray,
            f::file_read_char_array
        ),
        Op::S8Array(n) => array!(*n, i8, b::ot_i_file_fileReadS8Array, f::file_read_s8_array),
        Op::S16Array(n) => array!(
            *n,
            i16,
            b::ot_i_file_fileReadS16Array,
            f::file_read_s16_array
        ),
        Op::S32Array(n) => array!(
            *n,
            i32,
            b::ot_i_file_fileReadS32Array,
            f::file_read_s32_array
        ),
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, ..ProptestConfig::default() })]

    /// Any sequence of operations on a file opened in any mode: results,
    /// errors, positions and the final file contents match.
    #[test]
    fn file_operations_match(
        content in prop::collection::vec(any::<u8>(), 0..48),
        // Append streams are excluded: upstream never opens one, and stdio's
        // cached offset after a seek on them is libc-specific.
        mode in prop::sample::select(vec!["rb", "r+b", "wb", "w+b"]),
        ops in prop::collection::vec(any_op(), 0..24),
        missing: bool,
    ) {
        let dir = tmp("ops");
        let (pc, pr) = (dir.join("c.bin"), dir.join("r.bin"));
        if !missing {
            std::fs::write(&pc, &content).expect("write");
            std::fs::write(&pr, &content).expect("write");
        }
        let (cp, cm) = (cs(bytes(&pc)), cs(mode));
        // SAFETY: valid C strings.
        let mut c = unsafe { b::ot_s_file_fileOpen(cp.as_ptr(), cm.as_ptr()) };
        let mut r = f::file_open(&bytes(&pr), mode);
        prop_assert_eq!(c_view(&c), r_view(&r));
        let update = mode.contains('+');
        let mut last: Option<bool> = None; // whether the last data op wrote
        for op in &ops {
            // C requires a seek between reading and writing on update streams.
            if update && !c.error && !r.error {
                let switching = (is_write(op) && last == Some(false)) || (is_read(op) && last == Some(true));
                if switching {
                    let pos = f::file_get_position(&mut r);
                    // SAFETY: valid File.
                    let cpos = unsafe { b::fileGetPosition(&raw mut c) } as i64;
                    prop_assert_eq!(cpos, pos);
                    apply(&Op::SetPosition(pos), &mut c, &mut r);
                }
            }
            if is_write(op) {
                last = Some(true);
            } else if is_read(op) {
                last = Some(false);
            }
            let (cv, rv) = apply(op, &mut c, &mut r);
            prop_assert_eq!(cv, rv, "{:?}", op);
            prop_assert_eq!(c_view(&c), r_view(&r), "{:?}", op);
            // SAFETY: fileGetError only reads the struct.
            let ce = unsafe { CStr::from_ptr(b::fileGetError(&raw mut c)) }.to_string_lossy().into_owned();
            prop_assert_eq!(ce, f::file_get_error(&r));
        }
        // SAFETY: from fileOpen.
        unsafe { b::fileClose(&raw mut c) };
        f::file_close(&mut r);
        prop_assert_eq!(c_view(&c), r_view(&r));
        // Closing again is a no-op.
        // SAFETY: as above.
        unsafe { b::fileClose(&raw mut c) };
        f::file_close(&mut r);
        prop_assert_eq!(c_view(&c), r_view(&r));
        prop_assert_eq!(std::fs::read(&pc).ok(), std::fs::read(&pr).ok());
    }
}

#[test]
fn file_get_error_unknown_error_matches() {
    for (errnum, error) in [
        (0, true),
        (0, false),
        (-1, true),
        (2, true),
        (99_999, false),
    ] {
        let mut c = b::File {
            f: std::ptr::null_mut(),
            errnum,
            error,
        };
        // SAFETY: fileGetError only reads `errnum`/`error`.
        let cv = unsafe { CStr::from_ptr(b::fileGetError(&raw mut c)) };
        let r = File {
            f: None,
            errnum,
            error,
        };
        assert_eq!(
            cv.to_string_lossy(),
            f::file_get_error(&r),
            "{errnum} {error}"
        );
    }
}

/// `fseek` to a negative position fails with `EINVAL` (the error branch of
/// `fileSetPosition`), and the file then stays in error.
#[test]
fn negative_seek_matches() {
    let dir = tmp("negseek");
    let p = dir.join("n.bin");
    std::fs::write(&p, b"abc").expect("write");
    let (cp, cm) = (cs(bytes(&p)), cs("rb"));
    // SAFETY: valid C strings.
    let mut c = unsafe { b::ot_s_file_fileOpen(cp.as_ptr(), cm.as_ptr()) };
    let mut r = f::file_open(&bytes(&p), "rb");
    for op in [
        Op::SetPosition(-1),
        Op::GetPosition,
        Op::U8,
        Op::SetPosition(1),
    ] {
        let (cv, rv) = apply(&op, &mut c, &mut r);
        assert_eq!(cv, rv, "{op:?}");
        assert_eq!(c_view(&c), r_view(&r), "{op:?}");
    }
    assert!(r.error);
}

#[test]
fn reading_a_directory_matches() {
    let dir = tmp("readdir");
    let p = cs(bytes(&dir));
    let m = cs("rb");
    // SAFETY: valid C strings.
    let mut c = unsafe { b::ot_s_file_fileOpen(p.as_ptr(), m.as_ptr()) };
    let mut r = f::file_open(&bytes(&dir), "rb");
    for op in [Op::ReadAtMost(4), Op::GetLength, Op::ReadExactly(4), Op::U8] {
        let (cv, rv) = apply(&op, &mut c, &mut r);
        assert_eq!(cv, rv, "{op:?}");
        assert_eq!(c_view(&c), r_view(&r), "{op:?}");
    }
    // SAFETY: valid C strings.
    let mut c = unsafe { b::ot_s_file_fileOpen(p.as_ptr(), m.as_ptr()) };
    let mut r = f::file_open(&bytes(&dir), "rb");
    let (cv, rv) = apply(&Op::ReadExactly(4), &mut c, &mut r);
    assert_eq!((cv, c_view(&c)), (rv, r_view(&r)));
    // SAFETY: valid File.
    let ce = unsafe { CStr::from_ptr(b::fileGetError(&raw mut c)) };
    assert_eq!(ce.to_string_lossy(), f::file_get_error(&r));
}

#[test]
fn file_state_serde_round_trip() {
    let dir = tmp("serde");
    let p = dir.join("s.bin");
    std::fs::write(&p, b"abcdefgh").expect("write");
    let mut r = f::file_open(&bytes(&p), "rb");
    f::file_read_u16(&mut r);
    let json = serde_json::to_string(&r).expect("serialize");
    let mut back: File = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, r);
    assert_eq!(f::file_read_u8(&mut back), b'c');
    let mut cloned = r.clone();
    assert_eq!(cloned, r);
    assert_eq!(f::file_read_u8(&mut cloned), b'c');
    assert_eq!(r_view(&r).3, 2, "a clone is independent");
}
