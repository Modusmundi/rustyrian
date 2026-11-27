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
    e.raw_os_error().unwrap_or(0)
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
    error: bool,
    eof: bool,
    path: PathBuf,
    mode: OpenMode,
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

    /// `fwrite(data, 1, data.len(), f)`: the count written and `errno`
    /// afterwards (0 unless an error occurred).
    pub fn write(&mut self, data: &[u8]) -> (usize, i32) {
        if let Err(e) = self.drop_read_ahead() {
            self.error = true;
            return (0, e);
        }
        let mut n = 0;
        while n < data.len() {
            match self.file.write(&data[n..]) {
                Ok(0) => {
                    self.error = true;
                    return (n, 0);
                }
                Ok(k) => n += k,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => {
                    self.error = true;
                    return (n, errno_of(&e));
                }
            }
        }
        (n, 0)
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
        Ok(p as i64 - (self.buf.len() - self.pos) as i64)
    }

    /// `fflush(f)`: writes are unbuffered, so it always succeeds.
    pub fn flush(&mut self) -> Result<(), i32> {
        Ok(())
    }

    /// An independent stream on the same file at the same position.
    fn reopen(&self, position: i64) -> std::io::Result<Stream> {
        let m = OpenMode {
            create: false,
            truncate: false,
            ..self.mode
        };
        let mut file = m.options().open(&self.path)?;
        file.seek(SeekFrom::Start(position.max(0) as u64))?;
        Ok(Stream {
            error: self.error,
            eof: self.eof,
            ..Stream::from_std(file, self.path.clone(), self.mode)
        })
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
    error: bool,
    eof: bool,
}

impl Serialize for Stream {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        StreamRepr {
            path: self.path.clone(),
            mode: self.mode,
            position: self.tell().unwrap_or(-1),
            error: self.error,
            eof: self.eof,
        }
        .serialize(s)
    }
}

impl<'de> Deserialize<'de> for Stream {
    /// Reopens the file (without creating or truncating it) at the recorded
    /// position.
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Stream, D::Error> {
        let r = StreamRepr::deserialize(d)?;
        let file = r.mode.options().create(false).truncate(false).open(&r.path);
        let mut file = file.map_err(|e| D::Error::custom(format!("{}: {e}", r.path.display())))?;
        file.seek(SeekFrom::Start(r.position.max(0) as u64))
            .map_err(D::Error::custom)?;
        Ok(Stream {
            error: r.error,
            eof: r.eof,
            ..Stream::from_std(file, r.path, r.mode)
        })
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

/// `fileClose`. Writes are unbuffered, so closing cannot fail.
#[doc(alias = "fileClose")]
pub fn file_close(file: &mut File) {
    if file.f.is_none() {
        return;
    }

    file.f = None;
}

/// `strerror(errnum)`: the OS message without std's ` (os error N)` suffix.
pub fn strerror(errnum: i32) -> String {
    let s = std::io::Error::from_raw_os_error(errnum).to_string();
    match s.rfind(" (os error ") {
        Some(i) => s[..i].to_owned(),
        None => s,
    }
}

#[doc(alias = "fileGetError")]
pub fn file_get_error(file: &File) -> String {
    match file.errnum {
        ERRNUM_EOF => "Unexpected end of file".to_owned(),
        0 if file.error => "Unknown error".to_owned(),
        n => strerror(n),
    }
}
