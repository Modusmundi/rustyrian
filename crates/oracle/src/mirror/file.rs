//! Mirror of `rustyrian_core::file::State` <-> the C globals of `file.c`, and
//! conversions between C `File` (`FILE *`) and the core's `File`.

use super::{Mirror, global, global_mut};
use crate::bindings as b;
use rustyrian_core::file::{File, OpenMode, State, Stream};
use std::ffi::{CStr, c_char};
use std::path::PathBuf;

/// A C string as owned bytes; `None` for `NULL`.
///
/// # Safety
/// `p` is `NULL` or NUL-terminated.
pub unsafe fn cstr_from_c(p: *const c_char) -> Option<Vec<u8>> {
    if p.is_null() {
        None
    } else {
        // SAFETY: per the caller.
        Some(unsafe { CStr::from_ptr(p) }.to_bytes().to_vec())
    }
}

/// A `malloc`ed NUL-terminated copy of `s` (so C may `free` it); `NULL` for
/// `None`.
pub fn cstr_to_c(s: &Option<Vec<u8>>) -> *mut c_char {
    let Some(s) = s else {
        return std::ptr::null_mut();
    };
    // SAFETY: allocating `len + 1` bytes and filling them.
    unsafe {
        let p = libc::malloc(s.len() + 1).cast::<u8>();
        assert!(!p.is_null(), "out of memory");
        std::ptr::copy_nonoverlapping(s.as_ptr(), p, s.len());
        *p.add(s.len()) = 0;
        p.cast()
    }
}

/// The path of an open descriptor.
fn fd_path(fd: i32) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let mut buf = vec![0u8; libc::PATH_MAX as usize];
        // SAFETY: F_GETPATH writes at most PATH_MAX bytes into `buf`.
        let r = unsafe { libc::fcntl(fd, libc::F_GETPATH, buf.as_mut_ptr()) };
        if r == -1 {
            return None;
        }
        let n = buf.iter().position(|&c| c == 0)?;
        buf.truncate(n);
        use std::os::unix::ffi::OsStringExt;
        Some(PathBuf::from(std::ffi::OsString::from_vec(buf)))
    }
    #[cfg(not(target_os = "macos"))]
    {
        std::fs::read_link(format!("/proc/self/fd/{fd}")).ok()
    }
}

/// Converts a C `File`: an open `FILE *` becomes an independent stream on the
/// same file at the same position, with the same `ferror`/`feof` indicators.
/// The C stream is flushed first, since its buffered writes cannot be read
/// portably; a flush failure shows in its `ferror`.
///
/// # Safety
/// `c.f` is `NULL` or an open stream.
pub unsafe fn file_from_c(c: &b::File) -> File {
    let f = if c.f.is_null() {
        None
    } else {
        let fp = c.f.cast::<libc::FILE>();
        // SAFETY: `fp` is an open stream per the caller.
        let (fd, position, error, eof) = unsafe {
            libc::fflush(fp);
            (
                libc::fileno(fp),
                libc::ftell(fp),
                libc::ferror(fp) != 0,
                libc::feof(fp) != 0,
            )
        };
        // SAFETY: `fd` is open.
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        let acc = flags & libc::O_ACCMODE;
        let mode = OpenMode {
            read: acc == libc::O_RDONLY || acc == libc::O_RDWR,
            write: acc == libc::O_WRONLY || acc == libc::O_RDWR,
            append: flags & libc::O_APPEND != 0,
            create: false,
            truncate: false,
        };
        let path = fd_path(fd).expect("path of an open C stream");
        let mut opts = std::fs::OpenOptions::new();
        opts.read(mode.read)
            .write(mode.write && !mode.append)
            .append(mode.append);
        let mut file = opts.open(&path).expect("reopen a C stream's file");
        use std::io::{Seek, SeekFrom};
        file.seek(SeekFrom::Start(position.max(0) as u64))
            .expect("seek a reopened C stream");
        let mut s = Stream::from_std(file, path, mode);
        s.set_indicators(error, eof);
        Some(s)
    };
    File {
        f,
        errnum: c.errnum,
        error: c.error,
    }
}

/// Writes `r` into `c`: an open stream becomes a new `FILE *` on the same
/// file at the same position, holding the same buffered writes (the previous
/// `c.f` is not closed). The `ferror` and `feof` indicators of the new stream
/// start clear.
///
/// # Safety
/// `c` is writable.
pub unsafe fn file_to_c(r: &File, c: &mut b::File) {
    c.errnum = r.errnum;
    c.error = r.error;
    c.f = match &r.f {
        None => std::ptr::null_mut(),
        Some(s) => {
            let m = s.mode();
            let (flags, mode) = match (m.read, m.write, m.append) {
                (true, true, true) => (libc::O_RDWR | libc::O_APPEND, c"a+b"),
                (true, true, false) => (libc::O_RDWR, c"r+b"),
                (false, _, true) => (libc::O_WRONLY | libc::O_APPEND, c"ab"),
                (false, true, false) => (libc::O_WRONLY, c"wb"),
                _ => (libc::O_RDONLY, c"rb"),
            };
            use std::os::unix::ffi::OsStrExt;
            let path =
                std::ffi::CString::new(s.path().as_os_str().as_bytes()).expect("path without NUL");
            // SAFETY: valid C strings; the descriptor is handed to the stream.
            unsafe {
                let fd = libc::open(path.as_ptr(), flags);
                assert!(fd >= 0, "reopen {}", s.path().display());
                let fp = libc::fdopen(fd, mode.as_ptr());
                assert!(!fp.is_null(), "fdopen");
                let pending = s.pending();
                let os_position = s.tell().unwrap_or(0) - pending.len() as i64;
                libc::fseek(fp, os_position as libc::c_long, libc::SEEK_SET);
                libc::fwrite(pending.as_ptr().cast(), 1, pending.len(), fp);
                fp.cast()
            }
        }
    };
}

/// Address of a `file.c` static.
fn sv<T>(getter: unsafe extern "C" fn() -> *mut std::ffi::c_void) -> *mut T {
    // SAFETY: the generated getters return the static's address.
    unsafe { getter() }.cast()
}

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make; the
        // statics have the declared C types.
        unsafe {
            self.custom_data_dir_path = cstr_from_c(*global(&raw const b::customDataDirPath));
            self.data_dir_path = cstr_from_c(*sv::<*const c_char>(b::ot_sv_file_dataDirPath));
            self.data_dir_path_len = *sv::<usize>(b::ot_sv_file_dataDirPathLen);
            self.user_dir_path = cstr_from_c(*sv::<*const c_char>(b::ot_sv_file_userDirPath));
            self.user_dir_path_len = *sv::<usize>(b::ot_sv_file_userDirPathLen);
            self.find_data_files_base_data_dir_path = cstr_from_c(*sv::<*const c_char>(
                b::ot_sv_file_findDataFiles_baseDataDirPath,
            ));
        }
    }

    fn store_to_c(&self) {
        // SAFETY: as above. Strings are `malloc`ed copies (C frees
        // `userDirPath`; the others are leaked, as C never frees them).
        unsafe {
            *global_mut(&raw mut b::customDataDirPath) = cstr_to_c(&self.custom_data_dir_path);
            *sv::<*mut c_char>(b::ot_sv_file_dataDirPath) = cstr_to_c(&self.data_dir_path);
            *sv::<usize>(b::ot_sv_file_dataDirPathLen) = self.data_dir_path_len;
            *sv::<*mut c_char>(b::ot_sv_file_userDirPath) = cstr_to_c(&self.user_dir_path);
            *sv::<usize>(b::ot_sv_file_userDirPathLen) = self.user_dir_path_len;
            *sv::<*mut c_char>(b::ot_sv_file_findDataFiles_baseDataDirPath) =
                cstr_to_c(&self.find_data_files_base_data_dir_path);
        }
    }
}
