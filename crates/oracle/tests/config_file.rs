//! Differential tests for `config_file.c` / `config_file.h`.

use proptest::prelude::*;
use rustyrian_core::config_file::{self as cf, Config, ConfigBoolStyle, ConfigString};
use rustyrian_core::file::Stream;
use rustyrian_core::game::Halt;
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::config_file::{
    config_from_c, config_to_c, string_from_c, string_to_c,
};
use std::ffi::{CStr, CString, c_char};
use std::path::PathBuf;

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir()
        .join(format!("rustyrian-config-{}", std::process::id()))
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("scratch dir");
    d
}

fn cs(s: &[u8]) -> CString {
    CString::new(s.to_vec()).expect("no NUL")
}

/// Bytes of a C string pointer (`None` for `NULL`).
fn cbytes(p: *const c_char) -> Option<Vec<u8>> {
    // SAFETY: NULL or a NUL-terminated string owned by C.
    (!p.is_null()).then(|| unsafe { CStr::from_ptr(p) }.to_bytes().to_vec())
}

fn c_fopen(p: &std::path::Path, mode: &str) -> *mut b::FILE {
    use std::os::unix::ffi::OsStrExt;
    let (p, m) = (cs(p.as_os_str().as_bytes()), cs(mode.as_bytes()));
    // SAFETY: valid C strings.
    let f = unsafe { libc::fopen(p.as_ptr(), m.as_ptr()) };
    assert!(!f.is_null());
    f.cast()
}

/// Parses `text` with both engines; returns both configs.
fn parse_both(text: &[u8]) -> (b::Config, Config) {
    let dir = tmp("parse");
    let p = dir.join("in.cfg");
    std::fs::write(&p, text).expect("write");
    let mut c = b::Config::default();
    let fp = c_fopen(&p, "rb");
    // SAFETY: valid config and stream.
    let cok = unsafe { b::config_parse(&raw mut c, fp) };
    // SAFETY: from fopen.
    unsafe { libc::fclose(fp.cast()) };
    let mut r = Config::default();
    let mut s = Stream::open(p, "rb").expect("open");
    let rok = cf::config_parse(&mut r, &mut s);
    assert_eq!(cok, rok);
    (c, r)
}

/// Writes both configs and returns the bytes written by each.
fn write_both(c: &b::Config, r: &Config) -> (Vec<u8>, Vec<u8>) {
    let dir = tmp("write");
    let (pc, pr) = (dir.join("c.cfg"), dir.join("r.cfg"));
    let fp = c_fopen(&pc, "wb");
    // SAFETY: valid config and stream.
    unsafe {
        b::config_write(c, fp);
        libc::fclose(fp.cast());
    }
    let mut s = Stream::open(pr.clone(), "wb").expect("open");
    cf::config_write(r, &mut s);
    drop(s);
    (
        std::fs::read(pc).expect("read"),
        std::fs::read(pr).expect("read"),
    )
}

fn any_line() -> impl Strategy<Value = Vec<u8>> {
    let token = prop::sample::select(vec![
        "section",
        "item",
        "list",
        "sectionx",
        " ",
        "\t",
        "  ",
        "a",
        "key",
        "k2",
        "'q'",
        "\"dq\"",
        "'a b'",
        "'\\t\\n\\r\\\\\\''",
        "'\\x41\\x7e\\xff'",
        "'\\xg1'",
        "'\\q'",
        "'unterminated",
        "#",
        "'it''s'",
        "\"\\\"\"",
        "abcdefghijklmnopqrstu",
        "'abcdefghijklmnopqrstu'",
        "\u{e9}",
        "''",
        "x#y",
        "'\\x0a'",
        "\x01",
        "=",
    ]);
    (
        prop::collection::vec(token, 0..6),
        prop::sample::select(vec!["\n", "\r\n", "\r", ""]),
    )
        .prop_map(|(t, end)| {
            let mut v = t.concat().into_bytes();
            v.extend_from_slice(end.as_bytes());
            v
        })
}

fn any_text() -> impl Strategy<Value = Vec<u8>> {
    let structured = prop::collection::vec(
        prop_oneof![
            "(section|item|list)[ \t]+('[ -~]{0,20}'|[a-z]{1,20})([ \t]+('[ -~]{0,20}'|[a-z0-9]{1,8}))?[ \t]*\n"
                .prop_map(String::into_bytes),
            any_line(),
        ],
        0..40,
    )
    .prop_map(|v| v.concat());
    prop_oneof![structured, prop::collection::vec(any::<u8>(), 0..300)]
}

proptest! {
    /// Parsing any text (random lines, long lines that grow the buffer, raw
    /// bytes) gives the same configuration, and writing it gives the same
    /// bytes.
    #[test]
    fn config_parse_and_write_match(text in any_text()) {
        let (c, r) = parse_both(&text);
        // SAFETY: a parsed config.
        prop_assert_eq!(unsafe { config_from_c(&c) }, r.clone());
        let (cw, rw) = write_both(&c, &r);
        prop_assert_eq!(cw, rw);
    }

    #[test]
    fn config_parse_long_lines_match(len in 100usize..1200, k in 0usize..3) {
        let mut text = b"section 'type'\n".to_vec();
        for i in 0..k + 1 {
            text.extend_from_slice(format!("item k{i} '").as_bytes());
            text.extend(std::iter::repeat_n(b'v', len));
            text.extend_from_slice(b"'\n");
        }
        let (c, r) = parse_both(&text);
        // SAFETY: a parsed config.
        prop_assert_eq!(unsafe { config_from_c(&c) }, r.clone());
        let (cw, rw) = write_both(&c, &r);
        prop_assert_eq!(cw, rw);
    }
}

/// A shipped-style configuration (as upstream writes `opentyrian.cfg`).
#[test]
fn config_parse_sample_matches() {
    let text = b"section 'video'\n\titem 'fullscreen' '-1'\n\titem 'scaler' 'Scale2x'\n\n\
section 'keyboard'\n\tlist 'up' 'Up'\n\tlist 'up' 'Keypad 8'\n\tlist 'down' 'Down'\n\n\
section 'joystick' 'Fake Pad'\n\titem 'analog' 'no'\n\titem 'sensitivity' '5'\n\n";
    let (c, r) = parse_both(text);
    // SAFETY: a parsed config.
    assert_eq!(unsafe { config_from_c(&c) }, r);
    assert_eq!(r.sections.len(), 3);
    let (cw, rw) = write_both(&c, &r);
    assert_eq!(cw, rw);
    assert_eq!(&cw[..], &text[..]);
}

const KEYS: [&[u8]; 6] = [b"a", b"b", b"ab", b"key x", b"abcdefghijklmnopq", b""];
const VALUES: [&[u8]; 22] = [
    b"",
    b"0",
    b"1",
    b"yes",
    b"no",
    b"on",
    b"off",
    b"true",
    b"false",
    b"x",
    b" 12",
    b"-5",
    b"+7",
    b"0x1F",
    b"0X",
    b"010",
    b"08",
    b"4294967296",
    b"-4294967297",
    b"99999999999999999999",
    b"12 ",
    b"a long value of more than 16 bytes",
];

/// One accessor/manipulator call.
#[derive(Clone, Debug)]
enum Op {
    AddSection(usize, Option<usize>, bool),
    FindSection(usize, Option<usize>),
    FindOrAddSection(usize, Option<usize>),
    FindSections(usize),
    SetOption(usize, usize, Option<usize>, bool),
    GetOrSetOption(usize, usize, Option<usize>, bool),
    SetStringOption(usize, usize, Option<usize>, bool),
    GetStringOption(usize, usize),
    GetOrSetStringOption(usize, usize, Option<usize>),
    SetBool(usize, usize, bool, u32),
    GetBool(usize, usize),
    GetOrSetBool(usize, usize, bool, u32),
    SetInt(usize, usize, i32),
    GetInt(usize, usize),
    GetOrSetInt(usize, usize, i32),
    SetUint(usize, usize, u32),
    GetUint(usize, usize),
    GetOrSetUint(usize, usize, u32),
    SetValue(usize, usize, Option<usize>, bool),
    AddValue(usize, usize, usize, bool),
    RemoveValue(usize, usize, u32),
    Inspect(usize, usize),
}

fn any_op() -> impl Strategy<Value = Op> {
    let s = 0usize..4;
    let k = 0usize..KEYS.len();
    let v = 0usize..VALUES.len();
    let ov = prop::option::of(v.clone());
    let t = 0usize..3;
    let on = prop::option::of(0usize..3);
    prop_oneof![
        (t.clone(), on.clone(), any::<bool>()).prop_map(|(a, b, c)| Op::AddSection(a, b, c)),
        (t.clone(), on.clone()).prop_map(|(a, b)| Op::FindSection(a, b)),
        (t.clone(), on).prop_map(|(a, b)| Op::FindOrAddSection(a, b)),
        t.prop_map(Op::FindSections),
        (s.clone(), k.clone(), ov.clone(), any::<bool>())
            .prop_map(|(a, b, c, d)| Op::SetOption(a, b, c, d)),
        (s.clone(), k.clone(), ov.clone(), any::<bool>())
            .prop_map(|(a, b, c, d)| Op::GetOrSetOption(a, b, c, d)),
        (s.clone(), k.clone(), ov.clone(), any::<bool>())
            .prop_map(|(a, b, c, d)| Op::SetStringOption(a, b, c, d)),
        (s.clone(), k.clone()).prop_map(|(a, b)| Op::GetStringOption(a, b)),
        (s.clone(), k.clone(), ov.clone()).prop_map(|(a, b, c)| Op::GetOrSetStringOption(a, b, c)),
        (s.clone(), k.clone(), any::<bool>(), 0u32..4)
            .prop_map(|(a, b, c, d)| Op::SetBool(a, b, c, d)),
        (s.clone(), k.clone()).prop_map(|(a, b)| Op::GetBool(a, b)),
        (s.clone(), k.clone(), any::<bool>(), 0u32..4)
            .prop_map(|(a, b, c, d)| Op::GetOrSetBool(a, b, c, d)),
        (s.clone(), k.clone(), any::<i32>()).prop_map(|(a, b, c)| Op::SetInt(a, b, c)),
        (s.clone(), k.clone()).prop_map(|(a, b)| Op::GetInt(a, b)),
        (s.clone(), k.clone(), any::<i32>()).prop_map(|(a, b, c)| Op::GetOrSetInt(a, b, c)),
        (s.clone(), k.clone(), any::<u32>()).prop_map(|(a, b, c)| Op::SetUint(a, b, c)),
        (s.clone(), k.clone()).prop_map(|(a, b)| Op::GetUint(a, b)),
        (s.clone(), k.clone(), any::<u32>()).prop_map(|(a, b, c)| Op::GetOrSetUint(a, b, c)),
        (s.clone(), k.clone(), ov, any::<bool>()).prop_map(|(a, b, c, d)| Op::SetValue(a, b, c, d)),
        (s.clone(), k.clone(), v, any::<bool>()).prop_map(|(a, b, c, d)| Op::AddValue(a, b, c, d)),
        (s.clone(), k.clone(), 0u32..4).prop_map(|(a, b, c)| Op::RemoveValue(a, b, c)),
        (s, k).prop_map(|(a, b)| Op::Inspect(a, b)),
    ]
}

const TYPES: [&[u8]; 3] = [b"video", b"keyboard", b"a type longer than 16"];
const NAMES: [&[u8]; 3] = [b"one", b"two", b"a name longer than 16 bytes"];

fn style(s: u32) -> ConfigBoolStyle {
    [
        ConfigBoolStyle::ZeroOne,
        ConfigBoolStyle::NoYes,
        ConfigBoolStyle::OffOn,
        ConfigBoolStyle::FalseTrue,
    ][s as usize]
}

/// Index of `p` in the C section array.
fn sec_index(c: &b::Config, p: *mut b::ConfigSection) -> Option<usize> {
    (!p.is_null())
        .then(|| (p as usize - c.sections as usize) / std::mem::size_of::<b::ConfigSection>())
}

/// Index of `p` in the C option array of section `s`.
fn opt_index(c: &b::Config, s: usize, p: *mut b::ConfigOption) -> Option<usize> {
    // SAFETY: `s` is a valid section index.
    let base = unsafe { (*c.sections.add(s)).options } as usize;
    (!p.is_null()).then(|| (p as usize - base) / std::mem::size_of::<b::ConfigOption>())
}

fn ptr_of(v: &Option<CString>) -> *const c_char {
    v.as_ref().map_or(std::ptr::null(), |s| s.as_ptr())
}

/// Applies `op` to both configs; returns each side's observable result.
#[allow(clippy::too_many_lines)]
fn apply(op: &Op, c: &mut b::Config, r: &mut Config) -> (String, String) {
    let n = r.sections.len();
    // SAFETY (every C call below): `c` mirrors `r`, so every section/option
    // index valid for `r` is valid for `c`; strings are NUL-terminated.
    // SAFETY: `s` is a valid section index of `c` (see below).
    let sec = |c: &mut b::Config, s: usize| unsafe { c.sections.add(s) };
    match op.clone() {
        Op::AddSection(t, nm, inline) => {
            let (ct, cn) = (cs(TYPES[t]), nm.map(|i| cs(NAMES[i])));
            // SAFETY: as stated at the top of `apply`.
            let p = unsafe {
                if inline {
                    b::ot_i_config_file_config_add_section(c, ct.as_ptr(), ptr_of(&cn))
                } else {
                    b::config_add_section_len(
                        c,
                        ct.as_ptr(),
                        TYPES[t].len(),
                        ptr_of(&cn),
                        nm.map_or(0, |i| NAMES[i].len()),
                    )
                }
            };
            let ci = sec_index(c, p);
            if inline {
                cf::config_add_section(r, TYPES[t], nm.map(|i| NAMES[i]));
            } else {
                cf::config_add_section_len(r, TYPES[t], nm.map(|i| NAMES[i]));
            }
            (
                format!("{ci:?}"),
                format!("{:?}", Some(r.sections.len() - 1)),
            )
        }
        Op::FindSection(t, nm) => {
            let (ct, cn) = (cs(TYPES[t]), nm.map(|i| cs(NAMES[i])));
            // SAFETY: as stated at the top of `apply`.
            let p = unsafe { b::config_find_section(c, ct.as_ptr(), ptr_of(&cn)) };
            let base = r.sections.as_ptr() as usize;
            let ri = cf::config_find_section(r, TYPES[t], nm.map(|i| NAMES[i])).map(|s| {
                (s as *const _ as usize - base) / std::mem::size_of::<cf::ConfigSection>()
            });
            (format!("{:?}", sec_index(c, p)), format!("{ri:?}"))
        }
        Op::FindOrAddSection(t, nm) => {
            let (ct, cn) = (cs(TYPES[t]), nm.map(|i| cs(NAMES[i])));
            // SAFETY: as stated at the top of `apply`.
            let p = unsafe { b::config_find_or_add_section(c, ct.as_ptr(), ptr_of(&cn)) };
            let ci = sec_index(c, p);
            let s = cf::config_find_or_add_section(r, TYPES[t], nm.map(|i| NAMES[i]))
                as *const cf::ConfigSection;
            let ri = (s as usize - r.sections.as_ptr() as usize)
                / std::mem::size_of::<cf::ConfigSection>();
            (format!("{ci:?}"), format!("{:?}", Some(ri)))
        }
        Op::FindSections(t) => {
            let ct = cs(TYPES[t]);
            let (mut cv, mut rv) = (vec![], vec![]);
            let mut save: *mut b::ConfigSection = std::ptr::null_mut();
            loop {
                // SAFETY: as stated at the top of `apply`.
                let p = unsafe { b::config_find_sections(c, ct.as_ptr(), &raw mut save) };
                match sec_index(c, p) {
                    Some(i) => cv.push(i),
                    None => break,
                }
                // SAFETY: `save` points into the section array (or its end).
                save = unsafe { save.add(1) };
            }
            let mut cursor: Option<usize> = None;
            loop {
                let found = cf::config_find_sections(r, TYPES[t], Some(&mut cursor)).is_some();
                if !found {
                    break;
                }
                let i = cursor.expect("set");
                rv.push(i);
                cursor = Some(i + 1);
            }
            (format!("{cv:?}"), format!("{rv:?}"))
        }
        _ if n == 0 => (String::new(), String::new()),
        Op::SetOption(s, k, v, inline)
        | Op::GetOrSetOption(s, k, v, inline)
        | Op::SetStringOption(s, k, v, inline) => {
            let s = s % n;
            let (ck, cv) = (cs(KEYS[k]), v.map(|i| cs(VALUES[i])));
            let (rk, rv) = (KEYS[k], v.map(|i| VALUES[i]));
            let vlen = v.map_or(0, |i| VALUES[i].len());
            // SAFETY: as stated at the top of `apply`.
            let cp = unsafe {
                match (op, inline) {
                    (Op::SetOption(..), false) => b::config_set_option_len(
                        sec(c, s),
                        ck.as_ptr(),
                        rk.len(),
                        ptr_of(&cv),
                        vlen,
                    ),
                    (Op::SetOption(..), true) => {
                        b::ot_i_config_file_config_set_option(sec(c, s), ck.as_ptr(), ptr_of(&cv))
                    }
                    (Op::GetOrSetOption(..), false) => b::config_get_or_set_option_len(
                        sec(c, s),
                        ck.as_ptr(),
                        rk.len(),
                        ptr_of(&cv),
                        vlen,
                    ),
                    (Op::GetOrSetOption(..), true) => b::ot_i_config_file_config_get_or_set_option(
                        sec(c, s),
                        ck.as_ptr(),
                        ptr_of(&cv),
                    ),
                    (_, false) => {
                        b::config_set_string_option_len(
                            sec(c, s),
                            ck.as_ptr(),
                            rk.len(),
                            ptr_of(&cv),
                            vlen,
                        );
                        std::ptr::null_mut()
                    }
                    (_, true) => {
                        b::ot_i_config_file_config_set_string_option(
                            sec(c, s),
                            ck.as_ptr(),
                            ptr_of(&cv),
                        );
                        std::ptr::null_mut()
                    }
                }
            };
            let ci = opt_index(c, s, cp);
            let section = &mut r.sections[s];
            let base = section.options.as_ptr() as usize;
            let rp: Option<*const cf::ConfigOption> = match (op, inline) {
                (Op::SetOption(..), false) => Some(cf::config_set_option_len(section, rk, rv)),
                (Op::SetOption(..), true) => Some(cf::config_set_option(section, rk, rv)),
                (Op::GetOrSetOption(..), false) => {
                    Some(cf::config_get_or_set_option_len(section, rk, rv))
                }
                (Op::GetOrSetOption(..), true) => {
                    Some(cf::config_get_or_set_option(section, rk, rv))
                }
                (_, false) => {
                    cf::config_set_string_option_len(section, rk, rv);
                    None
                }
                (_, true) => {
                    cf::config_set_string_option(section, rk, rv);
                    None
                }
            };
            // Options may have moved on append; index from the new base.
            let base = if rp.is_some() {
                section.options.as_ptr() as usize
            } else {
                base
            };
            let ri = rp.map(|p| (p as usize - base) / std::mem::size_of::<cf::ConfigOption>());
            (format!("{ci:?}"), format!("{ri:?}"))
        }
        Op::GetStringOption(s, k) => {
            let s = s % n;
            let ck = cs(KEYS[k]);
            let mut out: *const c_char = std::ptr::null();
            // SAFETY: as stated at the top of `apply`.
            let ok = unsafe { b::config_get_string_option(sec(c, s), ck.as_ptr(), &raw mut out) };
            let mut rout = None;
            let rok = cf::config_get_string_option(&r.sections[s], KEYS[k], &mut rout);
            (
                format!("{ok} {:?}", cbytes(out)),
                format!("{rok} {:?}", rout.map(<[u8]>::to_vec)),
            )
        }
        Op::GetOrSetStringOption(s, k, v) => {
            let s = s % n;
            let (ck, cv) = (cs(KEYS[k]), v.map(|i| cs(VALUES[i])));
            // SAFETY: as stated at the top of `apply`.
            let p =
                unsafe { b::config_get_or_set_string_option(sec(c, s), ck.as_ptr(), ptr_of(&cv)) };
            let rv = cf::config_get_or_set_string_option(
                &mut r.sections[s],
                KEYS[k],
                v.map(|i| VALUES[i]),
            );
            (format!("{:?}", cbytes(p)), format!("{rv:?}"))
        }
        Op::SetBool(s, k, v, st) => {
            let s = s % n;
            let ck = cs(KEYS[k]);
            // SAFETY: as stated at the top of `apply`.
            unsafe { b::config_set_bool_option(sec(c, s), ck.as_ptr(), v, st) };
            cf::config_set_bool_option(&mut r.sections[s], KEYS[k], v, style(st));
            (String::new(), String::new())
        }
        Op::GetBool(s, k) => {
            let s = s % n;
            let ck = cs(KEYS[k]);
            let (mut cv, mut rv) = (true, true);
            // SAFETY: as stated at the top of `apply`.
            let ok = unsafe { b::config_get_bool_option(sec(c, s), ck.as_ptr(), &raw mut cv) };
            let rok = cf::config_get_bool_option(&r.sections[s], KEYS[k], &mut rv);
            (format!("{ok} {cv}"), format!("{rok} {rv}"))
        }
        Op::GetOrSetBool(s, k, v, st) => {
            let s = s % n;
            let ck = cs(KEYS[k]);
            // SAFETY: as stated at the top of `apply`.
            let cv = unsafe { b::config_get_or_set_bool_option(sec(c, s), ck.as_ptr(), v, st) };
            let rv = cf::config_get_or_set_bool_option(&mut r.sections[s], KEYS[k], v, style(st));
            (format!("{cv}"), format!("{rv}"))
        }
        Op::SetInt(s, k, v) => {
            let s = s % n;
            let ck = cs(KEYS[k]);
            // SAFETY: as stated at the top of `apply`.
            unsafe { b::config_set_int_option(sec(c, s), ck.as_ptr(), v) };
            cf::config_set_int_option(&mut r.sections[s], KEYS[k], v);
            (String::new(), String::new())
        }
        Op::GetInt(s, k) => {
            let s = s % n;
            let ck = cs(KEYS[k]);
            let (mut cv, mut rv) = (-77, -77);
            // SAFETY: as stated at the top of `apply`.
            let ok = unsafe { b::config_get_int_option(sec(c, s), ck.as_ptr(), &raw mut cv) };
            let rok = cf::config_get_int_option(&r.sections[s], KEYS[k], &mut rv);
            (format!("{ok} {cv}"), format!("{rok} {rv}"))
        }
        Op::GetOrSetInt(s, k, v) => {
            let s = s % n;
            let ck = cs(KEYS[k]);
            // SAFETY: as stated at the top of `apply`.
            let cv = unsafe { b::config_get_or_set_int_option(sec(c, s), ck.as_ptr(), v) };
            let rv = cf::config_get_or_set_int_option(&mut r.sections[s], KEYS[k], v);
            (format!("{cv}"), format!("{rv}"))
        }
        Op::SetUint(s, k, v) => {
            let s = s % n;
            let ck = cs(KEYS[k]);
            // SAFETY: as stated at the top of `apply`.
            unsafe { b::config_set_uint_option(sec(c, s), ck.as_ptr(), v) };
            cf::config_set_uint_option(&mut r.sections[s], KEYS[k], v);
            (String::new(), String::new())
        }
        Op::GetUint(s, k) => {
            let s = s % n;
            let ck = cs(KEYS[k]);
            let (mut cv, mut rv) = (77, 77);
            // SAFETY: as stated at the top of `apply`.
            let ok = unsafe { b::config_get_uint_option(sec(c, s), ck.as_ptr(), &raw mut cv) };
            let rok = cf::config_get_uint_option(&r.sections[s], KEYS[k], &mut rv);
            (format!("{ok} {cv}"), format!("{rok} {rv}"))
        }
        Op::GetOrSetUint(s, k, v) => {
            let s = s % n;
            let ck = cs(KEYS[k]);
            // SAFETY: as stated at the top of `apply`.
            let cv = unsafe { b::config_get_or_set_uint_option(sec(c, s), ck.as_ptr(), v) };
            let rv = cf::config_get_or_set_uint_option(&mut r.sections[s], KEYS[k], v);
            (format!("{cv}"), format!("{rv}"))
        }
        Op::SetValue(s, k, _, _)
        | Op::AddValue(s, k, _, _)
        | Op::RemoveValue(s, k, _)
        | Op::Inspect(s, k) => {
            let s = s % n;
            let ck = cs(KEYS[k]);
            // SAFETY: as stated at the top of `apply`.
            let co = unsafe { b::config_get_option(sec(c, s), ck.as_ptr()) };
            let section = &mut r.sections[s];
            let present = cf::config_get_option(section, KEYS[k]).is_some();
            assert_eq!(co.is_null(), !present);
            let Some(ro) = cf::config_get_option_mut(section, KEYS[k]) else {
                // SAFETY: NULL is accepted.
                let cvv = cbytes(unsafe { b::config_get_value(co) });
                return (
                    format!("absent {cvv:?}"),
                    format!("absent {:?}", cf::config_get_value(None)),
                );
            };
            match op.clone() {
                Op::SetValue(_, _, v, inline) => {
                    let cv = v.map(|i| cs(VALUES[i]));
                    let rv = v.map(|i| VALUES[i]);
                    // SAFETY: as stated at the top of `apply`.
                    let p = unsafe {
                        if inline {
                            b::ot_i_config_file_config_set_value(co, ptr_of(&cv))
                        } else {
                            b::config_set_value_len(
                                co,
                                ptr_of(&cv),
                                v.map_or(0, |i| VALUES[i].len()),
                            )
                        }
                    };
                    if inline {
                        cf::config_set_value(ro, rv);
                    } else {
                        cf::config_set_value_len(ro, rv);
                    }
                    (format!("{}", p == co), "true".into())
                }
                Op::AddValue(_, _, v, inline) => {
                    let cv = cs(VALUES[v]);
                    // SAFETY: as stated at the top of `apply`.
                    let p = unsafe {
                        if inline {
                            b::ot_i_config_file_config_add_value(co, cv.as_ptr())
                        } else {
                            b::config_add_value_len(co, cv.as_ptr(), VALUES[v].len())
                        }
                    };
                    if inline {
                        cf::config_add_value(ro, VALUES[v]);
                    } else {
                        cf::config_add_value_len(ro, VALUES[v]);
                    }
                    (format!("{}", p == co), "true".into())
                }
                Op::RemoveValue(_, _, i) => {
                    // SAFETY: as stated at the top of `apply`.
                    let p = unsafe { b::config_remove_value(co, i) };
                    let rp = cf::config_remove_value(ro, i).is_some();
                    (format!("{}", !p.is_null()), format!("{rp}"))
                }
                _ => {
                    // SAFETY: as stated at the top of `apply`.
                    let cv = unsafe {
                        (
                            cbytes(b::config_get_value(co)),
                            b::ot_i_config_file_config_is_value_list(co),
                            b::ot_i_config_file_config_get_value_count(co),
                        )
                    };
                    let rv = (
                        cf::config_get_value(Some(ro)).map(<[u8]>::to_vec),
                        cf::config_is_value_list(ro),
                        cf::config_get_value_count(ro),
                    );
                    (format!("{cv:?}"), format!("{rv:?}"))
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, ..ProptestConfig::default() })]

    /// Any sequence of accessor/manipulator calls on a configuration (empty,
    /// or parsed from text): results and the configuration match.
    #[test]
    fn config_operations_match(
        start in prop_oneof![Just(Vec::new()), any_text()],
        ops in prop::collection::vec(any_op(), 0..40),
    ) {
        let (mut c, mut r) = parse_both(&start);
        for op in &ops {
            let (cv, rv) = apply(op, &mut c, &mut r);
            prop_assert_eq!(cv, rv, "{:?}", op);
            // SAFETY: a valid config.
            prop_assert_eq!(unsafe { config_from_c(&c) }, r.clone(), "{:?}", op);
        }
        let (cw, rw) = write_both(&c, &r);
        prop_assert_eq!(cw, rw);
        // SAFETY: as above.
        unsafe { b::config_deinit(&raw mut c) };
        cf::config_deinit(&mut r);
        // SAFETY: as above.
        prop_assert_eq!(unsafe { config_from_c(&c) }, r.clone());
        // SAFETY: as above.
        unsafe { b::config_init(&raw mut c) };
        cf::config_init(&mut r);
        // SAFETY: as above.
        prop_assert_eq!(unsafe { config_from_c(&c) }, r);
    }

    /// `config_to_c` then `config_from_c` is the identity, and C can free the
    /// result.
    #[test]
    fn config_mirror_round_trip(text in any_text()) {
        let (_, r) = parse_both(&text);
        let mut c = b::Config::default();
        // SAFETY: `c` is writable.
        unsafe { config_to_c(&r, &mut c) };
        // SAFETY: a valid config.
        prop_assert_eq!(unsafe { config_from_c(&c) }, r);
        // SAFETY: allocated with malloc.
        unsafe { b::config_deinit(&raw mut c) };
    }

    #[test]
    fn string_helpers_match(s in prop::option::of(prop::collection::vec(1u8..=255, 0..24)), t in prop::collection::vec(1u8..=255, 0..24)) {
        let cs_ = s.as_ref().map(|v| cs(v));
        // SAFETY: NULL or a buffer of the given length.
        let mut c = unsafe { b::ot_s_config_file_string_init_len(ptr_of(&cs_), s.as_ref().map_or(0, Vec::len) as _) };
        let r = cf::string_init_len(s.as_deref());
        // SAFETY: an initialised ConfigString.
        prop_assert_eq!(unsafe { string_from_c(&c) }, r.clone());
        // SAFETY: as above.
        let cstr = cbytes(unsafe { b::ot_i_config_file_config_string_to_cstr(&raw const c) });
        prop_assert_eq!(cstr, cf::config_string_to_cstr(&r).map(<[u8]>::to_vec));
        if s.is_some() {
            let ct = cs(&t);
            // SAFETY: as above.
            let ce = unsafe { b::ot_s_config_file_string_equal_len(&raw mut c, ct.as_ptr(), t.len() as _) };
            prop_assert_eq!(ce, cf::string_equal_len(&r, &t));
            let tail = &t[..t.len().min(s.as_ref().map_or(0, Vec::len))];
            let ct = cs(tail);
            // SAFETY: as above.
            let ce = unsafe { b::ot_s_config_file_string_equal_len(&raw mut c, ct.as_ptr(), tail.len() as _) };
            prop_assert_eq!(ce, cf::string_equal_len(&r, tail));
        }
        let mut r = r;
        // SAFETY: as above.
        unsafe { b::ot_s_config_file_string_deinit(&raw mut c) };
        cf::string_deinit(&mut r);
        // SAFETY: deinit leaves a valid ConfigString.
        prop_assert_eq!(unsafe { string_from_c(&c) }, r.clone());
        let back = string_to_c(&r);
        // SAFETY: as above.
        prop_assert_eq!(unsafe { string_from_c(&back) }, r);
    }

    /// The parser primitives on arbitrary buffers.
    #[test]
    fn parser_primitives_match(buf in prop::collection::vec(1u8..=255, 0..24), start in 0usize..24) {
        let start = start.min(buf.len());
        for ch in [buf.first().copied().unwrap_or(0), 0, b' ', b'\t', b'\n', b'\r', b'x'] {
            // SAFETY: plain C calls.
            unsafe {
                prop_assert_eq!(b::ot_s_config_file_is_whitespace(ch as c_char), cf::is_whitespace(ch));
                prop_assert_eq!(b::ot_s_config_file_is_end(ch as c_char), cf::is_end(ch));
                prop_assert_eq!(b::ot_s_config_file_is_whitespace_or_end(ch as c_char), cf::is_whitespace_or_end(ch));
            }
        }
        let mut cbuf = buf.clone();
        cbuf.push(0);
        let mut rbuf = cbuf.clone();

        let (mut ci, mut ri) = (start as u64, start);
        // SAFETY: NUL-terminated buffer.
        let cd = unsafe { b::ot_s_config_file_match_directive(cbuf.as_ptr().cast(), &raw mut ci) };
        let rd = cf::match_directive(&rbuf, &mut ri);
        prop_assert_eq!((cd, ci as usize), (rd as i32, ri));

        let (mut ci, mut ri, mut cl, mut rl) = (start as u64, start, 0u64, 0usize);
        // SAFETY: as above.
        let cm = unsafe { b::ot_s_config_file_match_nonquote_field(cbuf.as_ptr().cast(), &raw mut ci, &raw mut cl) };
        let rm = cf::match_nonquote_field(&rbuf, &mut ri, &mut rl);
        prop_assert_eq!((cm, ci as usize), (rm, ri));
        if cm {
            prop_assert_eq!(cl as usize, rl);
        }

        let (mut ci, mut ri, mut cst, mut rst, mut cl, mut rl) = (start as u64, start, 0u64, 0usize, 0u64, 0usize);
        // SAFETY: as above.
        let cp = unsafe { b::ot_s_config_file_parse_field(cbuf.as_mut_ptr().cast(), &raw mut ci, &raw mut cst, &raw mut cl) };
        let rp = cf::parse_field(&mut rbuf, &mut ri, &mut rst, &mut rl);
        prop_assert_eq!((cp, ci as usize, cst as usize), (rp, ri, rst));
        if cp {
            prop_assert_eq!(cl as usize, rl);
        }
        prop_assert_eq!(&cbuf, &rbuf);

        if buf.get(start).is_some_and(|&q| q == b'\'' || q == b'"') {
            let (mut cbuf, mut rbuf) = (cbuf.clone(), rbuf.clone());
            let (mut ci, mut ri, mut cl, mut rl) = (start as u64, start, 0u64, 0usize);
            // SAFETY: as above.
            let cq = unsafe { b::ot_s_config_file_parse_quote_field(cbuf.as_mut_ptr().cast(), &raw mut ci, &raw mut cl) };
            let rq = cf::parse_quote_field(&mut rbuf, &mut ri, &mut rl);
            prop_assert_eq!(cq, rq);
            if cq {
                prop_assert_eq!((ci as usize, cl as usize), (ri, rl));
            }
            prop_assert_eq!(cbuf, rbuf);
        }
    }

    /// `sscanf("%i%n")`/`("%u%n")` through the integer getters on any string.
    #[test]
    fn integer_option_parsing_matches(v in prop_oneof![
        "[ \t]{0,2}[+-]?(0[xX]?)?[0-9a-fA-F]{0,12}[ g]?",
        "[ -~]{0,8}",
    ]) {
        let text = format!("section 's'\nitem 'k' '{}'\n", v.replace('\\', "\\\\").replace('\'', "\\'"));
        let (c, r) = parse_both(text.as_bytes());
        let ck = cs(b"k");
        let (mut ci, mut ri, mut cu, mut ru) = (-1, -1, 1u32, 1u32);
        // SAFETY: a parsed config with one section.
        let (ok_i, ok_u) = unsafe {
            (
                b::config_get_int_option(c.sections, ck.as_ptr(), &raw mut ci),
                b::config_get_uint_option(c.sections, ck.as_ptr(), &raw mut cu),
            )
        };
        prop_assert_eq!(ok_i, cf::config_get_int_option(&r.sections[0], b"k", &mut ri));
        prop_assert_eq!(ci, ri);
        prop_assert_eq!(ok_u, cf::config_get_uint_option(&r.sections[0], b"k", &mut ru));
        prop_assert_eq!(cu, ru);
    }
}

/// Deterministic paths the random sequences can miss: removing from 'list'
/// options (middle, last, down to empty), writing every escape class, and a
/// bad second hex digit.
#[test]
fn remove_value_and_escapes_match() {
    let text = b"section 's'\nlist 'k' 'a'\nlist 'k' 'b'\nlist 'k' 'c'\n\
item 'e' '\\t\\n\\r\\'\\\\\\x01\\x7f\\xff\\xAb'\nitem 'bad' '\\x4g'\nitem 'bad2' '\\xg4'\n";
    let (c, mut r) = parse_both(text);
    // SAFETY: a parsed config.
    assert_eq!(unsafe { config_from_c(&c) }, r);
    // 'k' is option 0 of section 0 on both sides.
    for (i, expect) in [(1u32, true), (5, false), (1, true), (0, true), (0, false)] {
        // SAFETY: a valid section/option of a parsed config.
        let cp = unsafe { b::config_remove_value((*c.sections).options, i) };
        let rp = cf::config_remove_value(&mut r.sections[0].options[0], i).is_some();
        assert_eq!((!cp.is_null(), rp), (expect, expect), "remove {i}");
        // SAFETY: as above.
        assert_eq!(unsafe { config_from_c(&c) }, r, "remove {i}");
    }
    let (cw, rw) = write_both(&c, &r);
    assert_eq!(cw, rw);
    assert!(rw.windows(4).any(|w| w == b"\\xff"));
    assert!(rw.windows(2).any(|w| w == b"\\n"));
}

/// The internal section/option initialisers and finalisers.
#[test]
fn internal_init_deinit_match() {
    let mut c = b::Config::default();
    let mut r = Config::default();
    let (t, nm) = (cs(b"a section type over 16"), cs(b"name"));
    // SAFETY: plain C calls on valid structures.
    unsafe {
        b::config_add_section_len(&raw mut c, t.as_ptr(), 22, std::ptr::null(), 0);
        b::ot_s_config_file_init_section(c.sections, t.as_ptr(), 22, nm.as_ptr(), 4);
        let k = cs(b"key");
        let v = cs(b"value");
        b::ot_s_config_file_append_option(c.sections, k.as_ptr(), 3, v.as_ptr(), 5);
        b::ot_s_config_file_append_option(c.sections, v.as_ptr(), 5, std::ptr::null(), 0);
        let found = b::ot_s_config_file_get_option_len(c.sections, k.as_ptr(), 3);
        assert_eq!(found, (*c.sections).options);
        b::ot_s_config_file_init_option_value(found, v.as_ptr(), 2);
        b::ot_s_config_file_init_option((*c.sections).options.add(1), k.as_ptr(), 2, v.as_ptr(), 1);
    }
    cf::config_add_section_len(&mut r, b"a section type over 16", None);
    cf::init_section(&mut r.sections[0], b"a section type over 16", Some(b"name"));
    cf::append_option(&mut r.sections[0], b"key", Some(b"value"));
    cf::append_option(&mut r.sections[0], b"value", None);
    assert_eq!(cf::get_option_len(&r.sections[0], b"key"), Some(0));
    cf::init_option_value(&mut r.sections[0].options[0], Some(b"va"));
    cf::init_option(&mut r.sections[0].options[1], b"ke", Some(b"v"));
    // SAFETY: a valid config.
    assert_eq!(unsafe { config_from_c(&c) }, r);

    // SAFETY: as above.
    unsafe {
        b::ot_s_config_file_deinit_option_value((*c.sections).options);
        b::ot_s_config_file_deinit_option((*c.sections).options.add(1));
    }
    cf::deinit_option_value(&mut r.sections[0].options[0]);
    cf::deinit_option(&mut r.sections[0].options[1]);
    // SAFETY: as above (the deinitialised short strings stay readable).
    assert_eq!(unsafe { config_from_c(&c) }, r);
    // SAFETY: as above.
    unsafe { b::ot_s_config_file_deinit_section(c.sections) };
    cf::deinit_section(&mut r.sections[0]);
    // SAFETY: as above.
    assert_eq!(unsafe { config_from_c(&c) }, r);
}

unsafe extern "C" {
    /// Declared only inside `config_file.c` (and its macro users).
    fn config_oom();
}

#[test]
fn config_oom_matches() {
    let mut d = Diff::new();
    // SAFETY: config_oom calls exit, which the guard catches.
    let c = unsafe { rustyrian_oracle::guard_call(|| config_oom()) };
    let r = cf::config_oom(&mut d.game);
    assert_eq!(c, Some(1));
    assert_eq!(r, Err(Halt { code: 1 }));
    assert_eq!(d.c_calls(), d.rust_calls());
}

#[test]
fn initial_state_matches_c() {
    let d = Diff::new();
    assert_eq!(diff::load::<cf::State>(), d.game.config_file);
}

#[test]
fn mirror_round_trip() {
    let d = Diff::new();
    d.store_to_c();
    assert_eq!(diff::load::<cf::State>(), d.game.config_file);
    let _ = ConfigString::default();
}

/// A list option inspected before and after removing a value, so the list
/// branch of `config_get_value_count` runs in every test run, not only when
/// the random operations happen to build a list.
#[test]
fn list_options_match() {
    let (mut c, mut r) = parse_both(b"");
    let ops = [
        Op::AddSection(0, None, false),
        Op::SetOption(0, 0, Some(1), false),
        Op::AddValue(0, 0, 2, false),
        Op::AddValue(0, 0, 3, true),
        Op::Inspect(0, 0),
        Op::RemoveValue(0, 0, 0),
        Op::Inspect(0, 0),
    ];
    for op in &ops {
        let (cv, rv) = apply(op, &mut c, &mut r);
        assert_eq!(cv, rv, "{op:?}");
        // SAFETY: a valid config.
        assert_eq!(unsafe { config_from_c(&c) }, r, "{op:?}");
    }
    assert!(
        cf::config_is_value_list(&r.sections[0].options[0]),
        "a list option"
    );
    // SAFETY: as above.
    unsafe { b::config_deinit(&raw mut c) };
    cf::config_deinit(&mut r);
}
