//! Differential tests for `animlib.c` / `animlib.h` (no mutable globals).

use proptest::prelude::*;
use rustyrian_core::animlib::{self as a, FileHeader, PageDescriptor, State};
use rustyrian_core::file::{self as f, File};
use rustyrian_core::memreader::MemReader;
use rustyrian_core::memwriter::MemWriter;
use rustyrian_core::palette::Color;
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;
use std::ffi::CString;
use std::path::{Path, PathBuf};

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let s = State::default();
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}

/// `animlib.c`'s file-local `FileHeader`.
#[repr(C)]
#[derive(Default)]
struct CFileHeader {
    page_count: u16,
    record_count: u32,
}

/// `animlib.c`'s file-local `PageDescriptor`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CPageDescriptor {
    first_record: u16,
    record_count: u16,
    records_size: u16,
}

fn anm() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/tyrend.anm")
        .canonicalize()
        .expect("data/tyrend.anm (run `cargo xtask fetch-data`)")
}

fn tmp_file(name: &str, contents: &[u8]) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rustyrian-animlib-{}", std::process::id()));
    std::fs::create_dir_all(&d).expect("scratch dir");
    let p = d.join(name);
    std::fs::write(&p, contents).expect("write");
    p
}

fn bytes(p: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    p.as_os_str().as_bytes().to_vec()
}

/// The same file opened by both engines.
fn open_both(p: &Path) -> (b::File, File) {
    let (cp, cm) = (
        CString::new(bytes(p)).expect("no NUL"),
        CString::new("rb").expect("no NUL"),
    );
    // SAFETY: valid C strings.
    let c = unsafe { b::ot_s_file_fileOpen(cp.as_ptr(), cm.as_ptr()) };
    (c, f::file_open(&bytes(p), "rb"))
}

/// Reads the three headers of `p` in both engines and compares them and the
/// file states.
fn check_headers(p: &Path) {
    let _d = Diff::new();
    let (mut c, mut r) = open_both(p);

    let mut ch = CFileHeader::default();
    let mut rh = FileHeader::default();
    // SAFETY: `ch` has the C layout; `c` is an open File.
    unsafe { b::ot_s_animlib_readFileHeader((&raw mut ch).cast(), &raw mut c) };
    a::read_file_header(&mut rh, &mut r);
    assert_eq!(
        (ch.page_count, ch.record_count),
        (rh.page_count, rh.record_count)
    );
    assert_eq!((c.errnum, c.error), (r.errnum, r.error));

    let mut cp = [b::SDL_Color::default(); 256];
    let mut rp = [Color::default(); 256];
    // SAFETY: 256 colors; an open File.
    unsafe { b::ot_s_animlib_readPalette(cp.as_mut_ptr(), &raw mut c) };
    a::read_palette(&mut rp, &mut r);
    for (x, y) in cp.iter().zip(&rp) {
        // `a` is not written by either engine.
        assert_eq!((x.r, x.g, x.b), (y.r, y.g, y.b));
    }
    assert_eq!((c.errnum, c.error), (r.errnum, r.error));

    let mut cd = [CPageDescriptor::default(); 256];
    let mut rd = [PageDescriptor::default(); 256];
    // SAFETY: 256 descriptors with the C layout; an open File.
    unsafe { b::ot_s_animlib_readPageDescriptors(cd.as_mut_ptr().cast(), &raw mut c) };
    a::read_page_descriptors(&mut rd, &mut r);
    for (x, y) in cd.iter().zip(&rd) {
        assert_eq!(
            (x.first_record, x.record_count, x.records_size),
            (y.first_record, y.record_count, y.records_size)
        );
    }
    assert_eq!((c.errnum, c.error), (r.errnum, r.error));
    // SAFETY: from fileOpen.
    unsafe { b::fileClose(&raw mut c) };
}

#[test]
fn headers_match_on_real_data_and_truncated_files() {
    check_headers(&anm());
    let full = std::fs::read(anm()).expect("tyrend.anm");
    for n in [0, 100, 256, 600, 256 + 1024, 2000, 256 + 1024 + 1536 - 1] {
        check_headers(&tmp_file(&format!("t{n}.anm"), &full[..n]));
    }
}

/// Decodes `body` onto `image` in both engines and compares the images and
/// the reader and writer states.
fn check_decode(body: &[u8], image: &mut Vec<u8>, writer_len: usize) {
    let mut cimage = image.clone();
    let mut cw = b::MemWriter {
        data: cimage.as_mut_ptr(),
        size: writer_len,
        error: false,
    };
    let mut cr = b::MemReader {
        data: body.as_ptr(),
        size: body.len(),
        error: false,
    };
    // SAFETY: the writer and reader cover live buffers of their sizes.
    unsafe { b::ot_s_animlib_decodeRunSkipDump(&raw mut cw, &raw mut cr) };

    let (mut rw, mut rr) = (
        MemWriter::new(&mut image[..writer_len]),
        MemReader::new(body),
    );
    a::decode_run_skip_dump(&mut rw, &mut rr);
    assert_eq!((cw.size, cw.error), (rw.size(), rw.error));
    assert_eq!((cr.size, cr.error), (rr.size(), rr.error));
    assert!(cimage == *image, "images differ");
}

/// Every record of `tyrend.anm`, decoded in page order onto one image as the
/// player does.
#[test]
fn decode_run_skip_dump_matches_every_record() {
    let full = std::fs::read(anm()).expect("tyrend.anm");
    let u16_at = |p: usize| u16::from_le_bytes([full[p], full[p + 1]]);
    let page_count = usize::from(u16_at(6));
    let mut image = vec![0u8; 320 * 200];
    let mut records = 0;
    for i in 0..page_count {
        let d = 256 + 1024 + i * 6;
        let (count, size) = (usize::from(u16_at(d + 2)), usize::from(u16_at(d + 4)));
        let page = 0xB00 + (i << 16);
        let sizes = page + 8;
        let mut rec = sizes + 2 * count;
        for k in 0..count {
            let n = usize::from(u16_at(sizes + 2 * k));
            let end = (rec + n).min(full.len()).min(page + 8 + 2 * count + size);
            let body = &full[(rec + 4).min(end)..end];
            check_decode(body, &mut image, 320 * 200);
            rec += n;
            records += 1;
        }
    }
    assert!(records > 50, "{records}");
}

proptest! {
    #[test]
    fn decode_run_skip_dump_matches_random(
        body in prop::collection::vec(any::<u8>(), 0..600),
        writer_len in 0usize..2000,
        seed in any::<u8>(),
    ) {
        let mut image: Vec<u8> = (0..2000).map(|i| (i as u8).wrapping_mul(seed)).collect();
        check_decode(&body, &mut image, writer_len);
    }

    /// Structured op streams: every opcode kind with sizes around the
    /// writer's end.
    #[test]
    fn decode_run_skip_dump_matches_ops(
        ops in prop::collection::vec((0u8..7, any::<u16>(), any::<u8>()), 0..30),
        writer_len in 0usize..3000,
    ) {
        let mut body = Vec::new();
        for (kind, n, v) in ops {
            match kind {
                0 => body.extend([0, n as u8, v]),
                1 => body.push(0x81 | (n as u8 & 0x7f)),
                2 => { let k = (n as u8 & 0x7f).max(1); body.push(k); body.extend((0..k).map(|i| i ^ v)); }
                3 => { body.push(0x80); body.extend((0xC000 | (n & 0x3fff)).to_le_bytes()); body.push(v); }
                4 => { body.push(0x80); body.extend((n & 0x7fff).max(1).to_le_bytes()); }
                5 => { let k = n & 0x3ff; body.push(0x80); body.extend((0x8000 | k).to_le_bytes()); body.extend((0..k).map(|i| i as u8 ^ v)); }
                _ => body.extend([0x80, 0, 0]),
            }
        }
        let mut image = vec![0x55u8; 3000];
        check_decode(&body, &mut image, writer_len);
    }
}
