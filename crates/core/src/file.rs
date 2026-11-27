// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `file.c` / `file.h`: data/user directory lookup and the
//! `File` wrapper over C stdio with sticky errors.
//!
//! C's `FILE *` is [`Stream`], a small stdio emulation over [`std::fs::File`]
//! with C's observable behaviour: `fopen` mode strings, positions, short reads
//! at end of file, the `ferror`/`feof` indicators and `errno` values. Its read
//! buffering (like stdio's) is not observable.
//!
//! The user directory follows divergence `D-PRODUCT-1` (rule R14):
//! `rustyrian` instead of upstream's `opentyrian`/`OpenTyrian`.

use crate::Game;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

/// `ERRNUM_EOF`: `File::errnum` after a short `fileReadExactly` at end of file.
pub const ERRNUM_EOF: i32 = -1;

/// `EINVAL` (identical on every supported OS).
const EINVAL: i32 = 22;

/// `EBADF` (identical on every supported OS).
const EBADF: i32 = 9;

/// `ENOMEM` (12 on every supported OS): the `errnum` of a read whose buffer
/// cannot be allocated (`D-UB-PCX-1`, `D-UB-SPRITE-2`).
pub const ENOMEM: i32 = 12;

/// The installation data directory configured at build time (`TYRIAN_DIR`
/// in upstream's Makefile); unset unless the build sets it.
pub const TYRIAN_DIR: Option<&str> = option_env!("TYRIAN_DIR");

/// A parsed `fopen` mode string.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenMode {
    pub read: bool,
    pub write: bool,
    pub append: bool,
    pub create: bool,
    pub truncate: bool,
}

impl OpenMode {
    /// Parses `r`, `w`, `a`, each optionally followed by `b` and/or `+`
    /// (`"rb"`, `"r+b"`, `"rb+"`, …), as C's `fopen` does; `None` where `fopen`
    /// fails with `EINVAL`.
    pub fn parse(mode: &str) -> Option<OpenMode> {
        let b = mode.as_bytes();
        let plus = b.get(1) == Some(&b'+') || (b.get(1) == Some(&b'b') && b.get(2) == Some(&b'+'));
        let m = match b.first()? {
            b'r' => OpenMode {
                read: true,
                ..OpenMode::default()
            },
            b'w' => OpenMode {
                write: true,
                create: true,
                truncate: true,
                ..OpenMode::default()
            },
            b'a' => OpenMode {
                write: true,
                append: true,
                create: true,
                ..OpenMode::default()
            },
            _ => return None,
        };
        Some(if plus {
            OpenMode {
                read: true,
                write: true,
                ..m
            }
        } else {
            m
        })
    }

    fn options(&self) -> std::fs::OpenOptions {
        let mut o = std::fs::OpenOptions::new();
        o.read(self.read)
            .write(self.write && !self.append)
            .append(self.append)
            .create(self.create)
            .truncate(self.truncate);
        o
    }
}

/// `errno` of an I/O error (0 if it has none).
fn errno_of(e: &std::io::Error) -> i32 {
    let code = e.raw_os_error().unwrap_or(0);
    // std reports Win32 error codes; upstream's `fopen`/`fread` on Windows
    // see the C runtime's `errno`, which the CRT derives from them.
    #[cfg(windows)]
    let code = if code == 0 { 0 } else { crt_errno(code) };
    code
}

/// The C runtime's `errno` for a Win32 error code: the table of the
/// Microsoft CRT's `_dosmaperr` (`EINVAL` for codes it does not list).
#[cfg(any(windows, test))]
fn crt_errno(win32: i32) -> i32 {
    const ENOENT: i32 = 2;
    const E2BIG: i32 = 7;
    const ENOEXEC: i32 = 8;
    const ECHILD: i32 = 10;
    const EAGAIN: i32 = 11;
    const EACCES: i32 = 13;
    const EEXIST: i32 = 17;
    const EXDEV: i32 = 18;
    const EMFILE: i32 = 24;
    const ENOSPC: i32 = 28;
    const EPIPE: i32 = 32;
    const ENOTEMPTY: i32 = 41;
    match win32 {
        2 | 3 | 15 | 18 | 53 | 67 | 161 | 206 => ENOENT,
        4 => EMFILE,
        5 | 16 | 65 | 82 | 83 | 108 | 132 | 158 | 167 => EACCES,
        19..=36 => EACCES,
        6 | 114 | 130 => EBADF,
        7..=9 | 1816 => ENOMEM,
        10 => E2BIG,
        11 => ENOEXEC,
        188..=202 => ENOEXEC,
        17 => EXDEV,
        80 | 183 => EEXIST,
        89 | 164 | 215 => EAGAIN,
        109 => EPIPE,
        112 => ENOSPC,
        128 | 129 => ECHILD,
        145 => ENOTEMPTY,
        _ => EINVAL,
    }
}

/// A file path from C string bytes.
pub fn path_from_bytes(b: &[u8]) -> PathBuf {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        std::ffi::OsStr::from_bytes(b).into()
    }
    #[cfg(not(unix))]
    {
        String::from_utf8_lossy(b).into_owned().into()
    }
}

/// Bytes of an environment variable, as C's `getenv` returns them.
fn getenv(name: &str) -> Option<Vec<u8>> {
    let v = std::env::var_os(name)?;
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        Some(v.into_vec())
    }
    #[cfg(not(unix))]
    {
        Some(v.to_string_lossy().into_owned().into_bytes())
    }
}

const STREAM_BUFFER_SIZE: usize = 4096;

/// C's `FILE`: an open stdio stream.
///
/// Fully buffered like stdio: reads go through a read-ahead buffer and
/// writes are held until the buffer fills, `fflush`, a seek, a read or
/// `fclose`, so write errors surface where C's do.
///
/// Holds the path and mode it was opened with (C does not expose these), so a
/// clone or a deserialised snapshot can reopen the same file at the same
/// position as an independent stream. Clone, equality and serialisation cover
/// only what C can observe: position, `ferror` and `feof`.
pub struct Stream {
    file: std::fs::File,
    /// Read-ahead: `buf[pos..]` are bytes already read from `file` but not yet
    /// returned (stdio's buffer).
    buf: Vec<u8>,
    pos: usize,
    /// Written bytes not yet passed to the OS. Empty whenever `buf[pos..]`
    /// is not.
    pending: Vec<u8>,
    error: bool,
    eof: bool,
    path: PathBuf,
    mode: OpenMode,
}

impl Drop for Stream {
    fn drop(&mut self) {
        let _ = self.write_out();
    }
}

impl Stream {
    /// `fopen(path, mode)`; `Err(errno)` where it returns `NULL`.
    pub fn open(path: PathBuf, mode: &str) -> Result<Stream, i32> {
        let mode = OpenMode::parse(mode).ok_or(EINVAL)?;
        let mut file = mode.options().open(&path).map_err(|e| errno_of(&e))?;
        // stdio positions append streams at the end when opening them (BSD
        // libc for "a" and "a+", glibc only for "a").
        if mode.append && (cfg!(not(target_os = "linux")) || !mode.read) {
            file.seek(SeekFrom::End(0)).map_err(|e| errno_of(&e))?;
        }
        Ok(Stream::from_std(file, path, mode))
    }

    /// Wraps an already open file positioned where the stream should start.
    pub fn from_std(file: std::fs::File, path: PathBuf, mode: OpenMode) -> Stream {
        Stream {
            file,
            buf: Vec::new(),
            pos: 0,
            pending: Vec::new(),
            error: false,
            eof: false,
            path,
            mode,
        }
    }

    /// The path the stream was opened with.
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Sets the `ferror`/`feof` indicators (for mirrors restoring a C
    /// stream's state).
    pub fn set_indicators(&mut self, error: bool, eof: bool) {
        self.error = error;
        self.eof = eof;
    }

    /// The mode the stream was opened with.
    pub fn mode(&self) -> OpenMode {
        self.mode
    }

    /// `ferror(f) != 0`.
    pub fn ferror(&self) -> bool {
        self.error
    }

    /// `feof(f) != 0`.
    pub fn feof(&self) -> bool {
        self.eof
    }

    /// `fread(data, 1, data.len(), f)`: the count read and `errno` afterwards
    /// (0 unless an error occurred).
    pub fn read(&mut self, data: &mut [u8]) -> (usize, i32) {
        if let Err(e) = self.write_out() {
            self.error = true;
            return (0, e);
        }
        let mut n = 0;
        while n < data.len() {
            if self.pos < self.buf.len() {
                let k = (self.buf.len() - self.pos).min(data.len() - n);
                data[n..n + k].copy_from_slice(&self.buf[self.pos..self.pos + k]);
                self.pos += k;
                n += k;
                continue;
            }
            let want = data.len() - n;
            let r = if want >= STREAM_BUFFER_SIZE {
                self.file.read(&mut data[n..])
            } else {
                self.buf.resize(STREAM_BUFFER_SIZE, 0);
                self.pos = 0;
                let r = self.file.read(&mut self.buf);
                self.buf.truncate(*r.as_ref().unwrap_or(&0));
                r
            };
            match r {
                Ok(0) => {
                    self.eof = true;
                    return (n, 0);
                }
                Ok(k) => {
                    if want >= STREAM_BUFFER_SIZE {
                        n += k;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => {
                    self.error = true;
                    return (n, errno_of(&e));
                }
            }
        }
        (n, 0)
    }

    /// Drops the read-ahead, moving the OS offset back to the stream
    /// position.
    fn drop_read_ahead(&mut self) -> Result<(), i32> {
        let unread = self.buf.len() - self.pos;
        self.buf.clear();
        self.pos = 0;
        if unread > 0 {
            self.file
                .seek(SeekFrom::Current(-(unread as i64)))
                .map_err(|e| errno_of(&e))?;
        }
        Ok(())
    }

    /// Passes the pending writes to the OS; `Err(errno)` on failure. After
    /// a failure Apple/BSD stdio keeps the unwritten rest buffered, while
    /// glibc and the MSVC CRT discard it (so `ftell` drops back to the OS
    /// offset).
    fn write_out(&mut self) -> Result<(), i32> {
        let mut n = 0;
        let result = loop {
            if n == self.pending.len() {
                break Ok(());
            }
            match self.file.write(&self.pending[n..]) {
                Ok(0) => break Err(0),
                Ok(k) => n += k,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => break Err(errno_of(&e)),
            }
        };
        const KEEPS_UNWRITTEN: bool = cfg!(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "freebsd"
        ));
        if result.is_err() && !KEEPS_UNWRITTEN {
            self.pending.clear();
        } else {
            self.pending.drain(..n);
        }
        result
    }

    /// `fwrite(data, 1, data.len(), f)`: the count written and `errno`
    /// afterwards (0 unless an error occurred). Data that fits stays in the
    /// buffer; a full buffer is passed on, then whole buffer-sized blocks go
    /// straight to the OS and the rest is buffered.
    pub fn write(&mut self, data: &[u8]) -> (usize, i32) {
        if data.is_empty() {
            return (0, 0);
        }
        if !self.mode.write {
            self.error = true;
            return (0, EBADF);
        }
        if let Err(e) = self.drop_read_ahead() {
            self.error = true;
            return (0, e);
        }
        let space = STREAM_BUFFER_SIZE - self.pending.len();
        if data.len() <= space {
            self.pending.extend_from_slice(data);
            return (data.len(), 0);
        }
        self.pending.extend_from_slice(&data[..space]);
        if let Err(e) = self.write_out() {
            self.error = true;
            return (space, e);
        }
        let rest = &data[space..];
        let direct = rest.len() / STREAM_BUFFER_SIZE * STREAM_BUFFER_SIZE;
        let mut n = 0;
        while n < direct {
            match self.file.write(&rest[n..direct]) {
                Ok(0) => {
                    self.error = true;
                    return (space + n, 0);
                }
                Ok(k) => n += k,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => {
                    self.error = true;
                    return (space + n, errno_of(&e));
                }
            }
        }
        self.pending.extend_from_slice(&rest[direct..]);
        (data.len(), 0)
    }

    /// `fputc(c, f)`.
    pub fn putc(&mut self, c: u8) {
        self.write(&[c]);
    }

    /// `fputs(s, f)`.
    pub fn puts(&mut self, s: &[u8]) {
        self.write(s);
    }

    fn seek(&mut self, to: SeekFrom) -> Result<(), i32> {
        if let Err(e) = self.write_out() {
            self.error = true;
            return Err(e);
        }
        self.buf.clear();
        self.pos = 0;
        self.file.seek(to).map_err(|e| errno_of(&e))?;
        self.eof = false;
        Ok(())
    }

    /// `fseek(f, position, SEEK_SET)`; `Err(errno)` where it returns −1.
    pub fn seek_set(&mut self, position: i64) -> Result<(), i32> {
        if position < 0 {
            return Err(EINVAL);
        }
        self.seek(SeekFrom::Start(position as u64))
    }

    /// `fseek(f, 0, SEEK_END)`.
    pub fn seek_end(&mut self) -> Result<(), i32> {
        self.seek(SeekFrom::End(0))
    }

    /// `ftell(f)`; `Err(errno)` where it returns −1.
    pub fn tell(&self) -> Result<i64, i32> {
        let mut f = &self.file;
        let p = f.stream_position().map_err(|e| errno_of(&e))?;
        Ok(p as i64 - (self.buf.len() - self.pos) as i64 + self.pending.len() as i64)
    }

    /// `fflush(f)`; `Err(errno)` where it returns `EOF`.
    pub fn flush(&mut self) -> Result<(), i32> {
        self.write_out().inspect_err(|_| self.error = true)
    }

    /// `fclose(f)`: flushes, then closes even if flushing failed.
    pub fn close(mut self) -> Result<(), i32> {
        let result = self.write_out();
        self.pending.clear();
        result
    }

    /// Written bytes not yet passed to the OS.
    pub fn pending(&self) -> &[u8] {
        &self.pending
    }

    /// An independent stream on the same file at the same position, holding
    /// the same pending writes.
    fn reopen(&self, position: i64) -> std::io::Result<Stream> {
        let m = OpenMode {
            create: false,
            truncate: false,
            ..self.mode
        };
        let mut file = m.options().open(&self.path)?;
        let os_position = position - self.pending.len() as i64;
        file.seek(SeekFrom::Start(os_position.max(0) as u64))?;
        let mut s = Stream::from_std(file, self.path.clone(), self.mode);
        s.pending = self.pending.clone();
        s.error = self.error;
        s.eof = self.eof;
        Ok(s)
    }
}

impl Clone for Stream {
    /// An independent stream reopened at the same position, so a snapshot
    /// does not move with the original. Falls back to a duplicated handle
    /// (sharing the OS offset, like a C struct copy sharing the `FILE *`).
    fn clone(&self) -> Stream {
        let position = self.tell().unwrap_or(0);
        if let Ok(s) = self.reopen(position) {
            return s;
        }
        let file = self
            .file
            .try_clone()
            .expect("cannot duplicate an open file handle");
        Stream {
            file,
            buf: self.buf.clone(),
            pos: self.pos,
            pending: self.pending.clone(),
            error: self.error,
            eof: self.eof,
            path: self.path.clone(),
            mode: self.mode,
        }
    }
}

impl PartialEq for Stream {
    fn eq(&self, other: &Stream) -> bool {
        self.tell() == other.tell() && self.error == other.error && self.eof == other.eof
    }
}

impl std::fmt::Debug for Stream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stream")
            .field("path", &self.path)
            .field("position", &self.tell())
            .field("pending", &self.pending.len())
            .field("error", &self.error)
            .field("eof", &self.eof)
            .finish()
    }
}

#[derive(Serialize, Deserialize)]
struct StreamRepr {
    path: PathBuf,
    mode: OpenMode,
    position: i64,
    pending: Vec<u8>,
    error: bool,
    eof: bool,
}

impl Serialize for Stream {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        StreamRepr {
            path: self.path.clone(),
            mode: self.mode,
            position: self.tell().unwrap_or(-1),
            pending: self.pending.clone(),
            error: self.error,
            eof: self.eof,
        }
        .serialize(s)
    }
}

impl<'de> Deserialize<'de> for Stream {
    /// Reopens the file (without creating or truncating it) at the recorded
    /// position, with the recorded writes pending.
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Stream, D::Error> {
        let r = StreamRepr::deserialize(d)?;
        let file = r.mode.options().create(false).truncate(false).open(&r.path);
        let mut file = file.map_err(|e| D::Error::custom(format!("{}: {e}", r.path.display())))?;
        let os_position = r.position - r.pending.len() as i64;
        file.seek(SeekFrom::Start(os_position.max(0) as u64))
            .map_err(D::Error::custom)?;
        let mut s = Stream::from_std(file, r.path, r.mode);
        s.pending = r.pending;
        s.error = r.error;
        s.eof = r.eof;
        Ok(s)
    }
}

/// `File`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct File {
    /// `FILE *f`; `None` is `NULL`.
    pub f: Option<Stream>,
    pub errnum: i32,
    /// Indicates that an operation failed and no further operations (except
    /// close) will be performed.
    pub error: bool,
}

/// Mutable globals of `file.c` (rule R3). C `char *` paths are owned bytes
/// without the NUL; `None` is `NULL`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// `customDataDirPath` (set from `--data`).
    pub custom_data_dir_path: Option<Vec<u8>>,
    pub data_dir_path: Option<Vec<u8>>,
    pub data_dir_path_len: usize,
    pub user_dir_path: Option<Vec<u8>>,
    pub user_dir_path_len: usize,
    /// `findDataFiles`' `static char *baseDataDirPath`.
    pub find_data_files_base_data_dir_path: Option<Vec<u8>>,
}

#[doc(alias = "fileExists")]
pub fn file_exists(path: &[u8]) -> bool {
    Stream::open(path_from_bytes(path), "rb").is_ok()
}

#[doc(alias = "fileOpen")]
pub fn file_open(path: &[u8], mode: &str) -> File {
    match Stream::open(path_from_bytes(path), mode) {
        Ok(f) => File {
            f: Some(f),
            errnum: 0,
            error: false,
        },
        Err(errnum) => File {
            f: None,
            errnum,
            error: true,
        },
    }
}

/// `strlen` of an owned C string (which holds no NUL).
fn cstr(b: &Option<Vec<u8>>) -> &[u8] {
    b.as_deref().unwrap_or(&[])
}

#[doc(alias = "findDataFiles")]
pub fn find_data_files(g: &mut Game) -> bool {
    g.file.data_dir_path = None;
    g.file.data_dir_path_len = 0;

    let filename = "tyrian1.lvl";

    if let Some(custom) = g.file.custom_data_dir_path.clone() {
        g.file.data_dir_path_len = custom.len();
        g.file.data_dir_path = Some(custom);

        return data_file_exists(g, filename);
    }

    // A "data" directory next to the executable (or inside the app bundle's
    // Resources on macOS), so a self-contained distribution runs from any cwd.
    if g.file.find_data_files_base_data_dir_path.is_none()
        && let Some(base_path) = g.platform.get_base_path()
    {
        let mut p = base_path.into_bytes();
        p.extend_from_slice(b"data");
        g.file.find_data_files_base_data_dir_path = Some(p);
    }

    let data_dir_paths = [
        g.file.find_data_files_base_data_dir_path.clone(),
        TYRIAN_DIR.map(|d| d.as_bytes().to_vec()),
    ];

    for p in data_dir_paths.into_iter().flatten() {
        g.file.data_dir_path_len = p.len();
        g.file.data_dir_path = Some(p);

        if data_file_exists(g, filename) {
            return true;
        }
    }

    g.file.data_dir_path = Some(Vec::new());
    g.file.data_dir_path_len = 0;

    file_exists(filename.as_bytes())
}

#[doc(alias = "dataFileExists")]
pub fn data_file_exists(g: &mut Game, filename: &str) -> bool {
    let mut file = data_file_open(g, filename, "rb");

    let result = !file.error;

    file_close(&mut file);

    result
}

#[doc(alias = "userFileExists")]
pub fn user_file_exists(g: &mut Game, filename: &str) -> bool {
    let mut file = user_file_open(g, filename, "rb");

    let result = !file.error;

    file_close(&mut file);

    result
}

/// `dir/filename`, where `dir` is the first `dir_len` bytes of `dir` (C's
/// `snprintf("%s/%s")` into `dir_len + 1 + strlen(filename) + 1` bytes).
fn join(dir: &[u8], dir_len: usize, filename: &str) -> Vec<u8> {
    let mut path = Vec::with_capacity(dir_len + 1 + filename.len());
    path.extend_from_slice(dir);
    path.push(b'/');
    path.extend_from_slice(filename.as_bytes());
    path.truncate(dir_len + 1 + filename.len());
    path
}

#[doc(alias = "dataFileOpen")]
pub fn data_file_open(g: &mut Game, filename: &str, mode: &str) -> File {
    if g.file.data_dir_path.is_none() {
        find_data_files(g);
    }

    if g.file.data_dir_path_len == 0 {
        return file_open(filename.as_bytes(), mode);
    }

    let path = join(
        cstr(&g.file.data_dir_path),
        g.file.data_dir_path_len,
        filename,
    );

    file_open(&path, mode)
}

/// Upstream's user directory name, replaced per `D-PRODUCT-1`.
const USER_DIR_NAME: &str = "rustyrian";

#[doc(alias = "determineUserDirPath")]
pub fn determine_user_dir_path(g: &mut Game) {
    if g.file.user_dir_path_len != 0 {
        g.file.user_dir_path = None;
        g.file.user_dir_path_len = 0;
    }

    if let Some(base_path) = g.platform.get_base_path() {
        // If a certain file exists in the base path, store user files there.
        let filename = "opentyrian.cfg";

        let mut file_path = base_path.clone().into_bytes();
        file_path.extend_from_slice(filename.as_bytes());

        let portable = file_exists(&file_path);

        if portable {
            // Trim trailing slash. (SDL's base path always ends in one; C
            // would underflow on an empty one.)
            let len = base_path.len().saturating_sub(1);
            g.file.user_dir_path_len = len;
            g.file.user_dir_path = Some(base_path.as_bytes()[..len].to_vec());
            return;
        }
    }

    let suffixed = |base: Vec<u8>, suffix: &str| {
        let mut p = base;
        p.extend_from_slice(suffix.as_bytes());
        p
    };

    #[cfg(windows)]
    {
        if let Some(app_data) = getenv("APPDATA") {
            let p = suffixed(app_data, &format!("/{USER_DIR_NAME}"));
            g.file.user_dir_path_len = p.len();
            g.file.user_dir_path = Some(p);
            return;
        }
    }
    #[cfg(not(windows))]
    {
        if let Some(xdg_config_home) = getenv("XDG_CONFIG_HOME") {
            let p = suffixed(xdg_config_home, &format!("/{USER_DIR_NAME}"));
            g.file.user_dir_path_len = p.len();
            g.file.user_dir_path = Some(p);
            return;
        }

        if let Some(home) = getenv("HOME") {
            let p = suffixed(home, &format!("/.config/{USER_DIR_NAME}"));
            g.file.user_dir_path_len = p.len();
            g.file.user_dir_path = Some(p);
            return;
        }
    }

    g.file.user_dir_path = Some(Vec::new());
    g.file.user_dir_path_len = 0;
}

#[doc(alias = "userFileOpen")]
pub fn user_file_open(g: &mut Game, filename: &str, mode: &str) -> File {
    if g.file.user_dir_path.is_none() {
        determine_user_dir_path(g);
    }

    if g.file.user_dir_path_len == 0 {
        return file_open(filename.as_bytes(), mode);
    }

    let dir = path_from_bytes(cstr(&g.file.user_dir_path));
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        let _ = std::fs::DirBuilder::new().mode(0o700).create(&dir);
    }
    #[cfg(not(unix))]
    {
        let _ = std::fs::create_dir(&dir);
    }

    let path = join(
        cstr(&g.file.user_dir_path),
        g.file.user_dir_path_len,
        filename,
    );

    file_open(&path, mode)
}

/// The open stream of a file without an error. C dereferences `file->f`
/// unchecked after `file->error` is false, which implies an open stream.
fn stream(file: &mut File) -> &mut Stream {
    file.f
        .as_mut()
        .expect("a File without an error has an open stream")
}

#[doc(alias = "fileSetPosition")]
pub fn file_set_position(file: &mut File, position: i64) {
    if file.error {
        return;
    }

    if let Err(errnum) = stream(file).seek_set(position) {
        file.errnum = errnum;
        file.error = true;
    }
}

#[doc(alias = "fileGetPosition")]
pub fn file_get_position(file: &mut File) -> i64 {
    if file.error {
        return 0;
    }

    match stream(file).tell() {
        Ok(position) if position >= 0 => position,
        Ok(_) => {
            file.errnum = 0;
            file.error = true;
            0
        }
        Err(errnum) => {
            file.errnum = errnum;
            file.error = true;
            0
        }
    }
}

#[doc(alias = "fileGetLength")]
pub fn file_get_length(file: &mut File) -> i64 {
    if file.error {
        return 0;
    }

    let f = stream(file);
    let result = (|| {
        let position = f.tell()?;
        f.seek_end()?;
        let length = f.tell()?;
        f.seek_set(position)?;
        Ok(length)
    })();
    match result {
        Ok(length) => length,
        Err(errnum) => {
            file.errnum = errnum;
            file.error = true;
            0
        }
    }
}

/// `fileReadAtMost(file, data, size)` with `size = data.len()`.
#[doc(alias = "fileReadAtMost")]
pub fn file_read_at_most(file: &mut File, data: &mut [u8]) -> usize {
    if file.error {
        return 0;
    }

    let f = stream(file);
    let (read, errnum) = f.read(data);
    if read == data.len() {
        return read;
    }

    let (ferror, feof) = (f.ferror(), f.feof());
    file.errnum = errnum;
    file.error = ferror;
    debug_assert!(file.error || feof);

    read
}

/// `fileReadExactly(file, data, size)` with `size = data.len()`.
#[doc(alias = "fileReadExactly")]
pub fn file_read_exactly(file: &mut File, data: &mut [u8]) {
    if file.error {
        data.fill(0);
        return;
    }

    let (read, errnum) = stream(file).read(data);
    if read == data.len() {
        return;
    }

    file.errnum = errnum;
    file.error = true;

    if file.errnum == 0 {
        file.errnum = ERRNUM_EOF;
    }

    data[read..].fill(0);
}

/// Reads `values.len()` `W`-byte elements as one `fileReadExactly` would
/// (through a fixed chunk buffer; chunking is unobservable because a short
/// read sets `error`, after which every read yields zeros).
fn read_array<T, const W: usize>(file: &mut File, values: &mut [T], conv: impl Fn([u8; W]) -> T) {
    let mut chunk = [0u8; 512];
    let per = chunk.len() / W;
    for vs in values.chunks_mut(per) {
        let bytes = &mut chunk[..vs.len() * W];
        file_read_exactly(file, bytes);
        for (v, b) in vs.iter_mut().zip(bytes.as_chunks::<W>().0) {
            *v = conv(*b);
        }
    }
}

#[doc(alias = "fileReadU8")]
pub fn file_read_u8(file: &mut File) -> u8 {
    let mut value = [0u8; 1];
    file_read_exactly(file, &mut value);
    value[0]
}

#[doc(alias = "fileReadU8Array")]
pub fn file_read_u8_array(file: &mut File, values: &mut [u8]) {
    file_read_exactly(file, values);
}

#[doc(alias = "fileReadU16")]
pub fn file_read_u16(file: &mut File) -> u16 {
    let mut value = [0u8; 2];
    file_read_exactly(file, &mut value);
    u16::from_le_bytes(value)
}

#[doc(alias = "fileReadU16Array")]
pub fn file_read_u16_array(file: &mut File, values: &mut [u16]) {
    read_array(file, values, u16::from_le_bytes);
}

#[doc(alias = "fileReadU16BE")]
pub fn file_read_u16be(file: &mut File) -> u16 {
    let mut value = [0u8; 2];
    file_read_exactly(file, &mut value);
    u16::from_be_bytes(value)
}

#[doc(alias = "fileReadU16BEArray")]
pub fn file_read_u16be_array(file: &mut File, values: &mut [u16]) {
    read_array(file, values, u16::from_be_bytes);
}

#[doc(alias = "fileReadU32")]
pub fn file_read_u32(file: &mut File) -> u32 {
    let mut value = [0u8; 4];
    file_read_exactly(file, &mut value);
    u32::from_le_bytes(value)
}

#[doc(alias = "fileReadU32Array")]
pub fn file_read_u32_array(file: &mut File, values: &mut [u32]) {
    read_array(file, values, u32::from_le_bytes);
}

#[doc(alias = "fileReadBool")]
pub fn file_read_bool(file: &mut File) -> bool {
    file_read_u8(file) != 0
}

#[doc(alias = "fileReadBoolArray")]
pub fn file_read_bool_array(file: &mut File, values: &mut [bool]) {
    for v in values.iter_mut() {
        *v = file_read_bool(file);
    }
}

/// `char` is signed in the oracle (rule R4).
#[doc(alias = "fileReadChar")]
pub fn file_read_char(file: &mut File) -> i8 {
    file_read_u8(file) as i8
}

/// `char` buffers are bytes (rule R4).
#[doc(alias = "fileReadCharArray")]
pub fn file_read_char_array(file: &mut File, values: &mut [u8]) {
    file_read_exactly(file, values);
}

#[doc(alias = "fileReadS8")]
pub fn file_read_s8(file: &mut File) -> i8 {
    file_read_u8(file) as i8
}

#[doc(alias = "fileReadS8Array")]
pub fn file_read_s8_array(file: &mut File, values: &mut [i8]) {
    read_array(file, values, |b: [u8; 1]| b[0] as i8);
}

#[doc(alias = "fileReadS16")]
pub fn file_read_s16(file: &mut File) -> i16 {
    file_read_u16(file) as i16
}

#[doc(alias = "fileReadS16Array")]
pub fn file_read_s16_array(file: &mut File, values: &mut [i16]) {
    read_array(file, values, i16::from_le_bytes);
}

#[doc(alias = "fileReadS32")]
pub fn file_read_s32(file: &mut File) -> i32 {
    file_read_u32(file) as i32
}

#[doc(alias = "fileReadS32Array")]
pub fn file_read_s32_array(file: &mut File, values: &mut [i32]) {
    read_array(file, values, i32::from_le_bytes);
}

/// `fileWrite(file, data, size)` with `size = data.len()`.
#[doc(alias = "fileWrite")]
pub fn file_write(file: &mut File, data: &[u8]) {
    if file.error {
        return;
    }

    let (written, errnum) = stream(file).write(data);
    if written == data.len() {
        return;
    }

    file.errnum = errnum;
    file.error = true;
}

#[doc(alias = "fileWriteU8")]
pub fn file_write_u8(file: &mut File, value: u8) {
    file_write(file, &[value]);
}

#[doc(alias = "fileWriteU8Array")]
pub fn file_write_u8_array(file: &mut File, values: &[u8]) {
    file_write(file, values);
}

#[doc(alias = "fileWriteCharArray")]
pub fn file_write_char_array(file: &mut File, values: &[u8]) {
    file_write(file, values);
}

#[doc(alias = "fileFlush")]
pub fn file_flush(file: &mut File) {
    if file.error {
        return;
    }

    if let Err(errnum) = stream(file).flush() {
        file.errnum = errnum;
        file.error = true;
    }
}

#[doc(alias = "fileClose")]
pub fn file_close(file: &mut File) {
    let Some(f) = file.f.take() else {
        return;
    };

    if let Err(errnum) = f.close() {
        file.errnum = errnum;
        file.error = true;
    }
}

/// `strerror(errnum)`: the OS message without std's ` (os error N)` suffix.
#[cfg(not(windows))]
pub fn strerror(errnum: i32) -> String {
    let s = std::io::Error::from_raw_os_error(errnum).to_string();
    match s.rfind(" (os error ") {
        Some(i) => s[..i].to_owned(),
        None => s,
    }
}

/// `strerror(errnum)`: the C runtime's message for an `errno` value (see
/// [`errno_of`]), not the Win32 one std would give.
#[cfg(windows)]
pub fn strerror(errnum: i32) -> String {
    crt_strerror(errnum).to_owned()
}

/// The Microsoft CRT's `strerror` table (`_sys_errlist`).
#[cfg(any(windows, test))]
fn crt_strerror(errnum: i32) -> &'static str {
    const MESSAGES: [&str; 43] = [
        "No error",
        "Operation not permitted",
        "No such file or directory",
        "No such process",
        "Interrupted function call",
        "Input/output error",
        "No such device or address",
        "Arg list too long",
        "Exec format error",
        "Bad file descriptor",
        "No child processes",
        "Resource temporarily unavailable",
        "Not enough space",
        "Permission denied",
        "Bad address",
        "Unknown error",
        "Resource device",
        "File exists",
        "Improper link",
        "No such device",
        "Not a directory",
        "Is a directory",
        "Invalid argument",
        "Too many open files in system",
        "Too many open files",
        "Inappropriate I/O control operation",
        "Unknown error",
        "File too large",
        "No space left on device",
        "Invalid seek",
        "Read-only file system",
        "Too many links",
        "Broken pipe",
        "Domain error",
        "Result too large",
        "Unknown error",
        "Resource deadlock avoided",
        "Unknown error",
        "Filename too long",
        "No locks available",
        "Function not implemented",
        "Directory not empty",
        "Illegal byte sequence",
    ];
    usize::try_from(errnum)
        .ok()
        .and_then(|i| MESSAGES.get(i))
        .copied()
        .unwrap_or("Unknown error")
}

#[doc(alias = "fileGetError")]
pub fn file_get_error(file: &File) -> String {
    match file.errnum {
        ERRNUM_EOF => "Unexpected end of file".to_owned(),
        0 if file.error => "Unknown error".to_owned(),
        n => strerror(n),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A missing file reports `ENOENT` and C's message on every OS
    /// (on Windows through the CRT's mapping of the Win32 error).
    #[test]
    fn missing_file_errno_and_message_match_c() {
        let dir = std::env::temp_dir().join(format!("rr-errno-{}", std::process::id()));
        let missing = dir.join("absent").join("tyrian.cfg");
        let file = file_open(missing.to_string_lossy().as_bytes(), "rb");
        assert!(file.error);
        assert_eq!(file.errnum, 2);
        assert_eq!(file_get_error(&file), "No such file or directory");
    }

    #[test]
    fn crt_tables_map_the_common_win32_errors() {
        // ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND, ERROR_ACCESS_DENIED,
        // ERROR_FILE_EXISTS, ERROR_DISK_FULL, ERROR_INVALID_PARAMETER.
        let got: Vec<_> = [2, 3, 5, 80, 112, 87]
            .map(|w| crt_strerror(crt_errno(w)))
            .to_vec();
        assert_eq!(
            got,
            [
                "No such file or directory",
                "No such file or directory",
                "Permission denied",
                "File exists",
                "No space left on device",
                "Invalid argument",
            ]
        );
        assert_eq!(crt_strerror(-1), "Unknown error");
        assert_eq!(crt_strerror(43), "Unknown error");
    }
}
