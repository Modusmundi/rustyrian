// SPDX-License-Identifier: GPL-2.0-or-later OR MPL-2.0
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `config_file.c` / `config_file.h`: the
//! `section`/`item`/`list` text configuration format.
//!
//! Representation (rule R8):
//! - `ConfigString` (C: inline/heap union) holds the bytes passed to
//!   `string_init_len` (`None` is `NULL`); C reads it as a NUL-terminated
//!   string ([`config_string_to_cstr`]).
//! - Counted C arrays (`sections`/`sections_count`, `options`/`options_count`,
//!   `values`/`values_count`) are `Vec`s; the option value union is
//!   [`ConfigValue`].
//! - Pointers returned into those arrays are references (public functions) or
//!   indices (file-static helpers). Allocation failure aborts in Rust, so the
//!   `NULL`-on-`realloc`-failure results and their `config_oom()` calls cannot
//!   occur; `config_remove_value`'s out-of-range `NULL` remains an `Option`.
//! - `const char *` parameters are byte slices; NUL-terminated ones are read
//!   up to their first NUL, `_len` ones are taken whole. C's `FILE *` is
//!   [`crate::file::Stream`].

use crate::Game;
use crate::file::Stream;
use crate::game::{EXIT_FAILURE, Flow, Halt};
use serde::{Deserialize, Serialize};

/// `ConfigString`. `None` is `NULL`. The default is the all-zero C value,
/// the empty string.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigString(pub Option<Vec<u8>>);

impl Default for ConfigString {
    fn default() -> Self {
        ConfigString(Some(Vec::new()))
    }
}

/// `short_buf` size; strings at least this long are heap-allocated in C.
const SHORT_BUF: usize = 16;

/// The value union of `ConfigOption`, tagged by `values_count`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigValue {
    /// `values_count == 0`: `v.value`, an 'item' value or, if `NULL`, an
    /// empty 'list'.
    Value(ConfigString),
    /// `values_count > 0`: `v.values`, a non-empty 'list'.
    Values(Vec<ConfigString>),
}

impl Default for ConfigValue {
    fn default() -> Self {
        ConfigValue::Value(ConfigString::default())
    }
}

/// `ConfigOption`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigOption {
    /// The key of the option.
    pub key: ConfigString,
    /// `values_count` and `v`.
    pub v: ConfigValue,
}

impl ConfigOption {
    /// `values_count`.
    pub fn values_count(&self) -> u32 {
        match &self.v {
            ConfigValue::Value(_) => 0,
            ConfigValue::Values(v) => v.len() as u32,
        }
    }
}

/// `ConfigSection`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigSection {
    /// The type of the section.
    pub r#type: ConfigString,
    /// The optional name of the section (may be `NULL`).
    pub name: ConfigString,
    /// `options`, `options_count`.
    pub options: Vec<ConfigOption>,
}

/// `Config`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// `sections`, `sections_count`.
    pub sections: Vec<ConfigSection>,
}

/// `ConfigBoolStyle`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u32)]
pub enum ConfigBoolStyle {
    ZeroOne = 0,
    NoYes = 1,
    OffOn = 2,
    FalseTrue = 3,
}

pub const ZERO_ONE: ConfigBoolStyle = ConfigBoolStyle::ZeroOne;
pub const NO_YES: ConfigBoolStyle = ConfigBoolStyle::NoYes;
pub const OFF_ON: ConfigBoolStyle = ConfigBoolStyle::OffOn;
pub const FALSE_TRUE: ConfigBoolStyle = ConfigBoolStyle::FalseTrue;

/// Mutable globals of this module (rule R3): none.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {}

/// A `const char *` argument read as C does: up to its first NUL.
fn c(s: &[u8]) -> &[u8] {
    match s.iter().position(|&b| b == 0) {
        Some(i) => &s[..i],
        None => s,
    }
}

/// `strcmp(a, b) == 0` for C strings.
fn streq(a: &[u8], b: &[u8]) -> bool {
    c(a) == c(b)
}

pub fn config_oom(g: &mut Game) -> Flow {
    crate::logging::log_fatal(g, "Out of memory.");
    Err(Halt { code: EXIT_FAILURE })
}

/* string manipulators */

/// `string_init_len(s, n)` with `n = s.len()`.
pub fn string_init_len(s: Option<&[u8]>) -> ConfigString {
    ConfigString(s.map(<[u8]>::to_vec))
}

pub fn string_deinit(string: &mut ConfigString) {
    let is_long = string.0.as_ref().is_none_or(|s| s.len() >= SHORT_BUF);

    if is_long {
        string.0 = None;
    }
}

/// `string_equal_len(string, s, n)` with `n = s.len()`.
pub fn string_equal_len(string: &ConfigString, s: &[u8]) -> bool {
    // C reads the stored buffer, including its terminator; reading past the
    // terminator (possible only with a NUL inside `s`) yields 0 here.
    let raw = string.0.as_deref().unwrap_or(&[]);
    let at = |i: usize| raw.get(i).copied().unwrap_or(0);
    // strncmp(cstr, s, n) == 0
    for (i, &b) in s.iter().enumerate() {
        let a = at(i);
        if a != b {
            return false;
        }
        if a == 0 {
            break;
        }
    }
    at(s.len()) == 0
}

pub fn config_string_to_cstr(string: &ConfigString) -> Option<&[u8]> {
    string.0.as_deref().map(c)
}

/* config manipulators */

pub fn config_init(config: &mut Config) {
    config.sections = Vec::new();
}

pub fn config_deinit(config: &mut Config) {
    for section in config.sections.iter_mut() {
        deinit_section(section);
    }

    config.sections = Vec::new();
}

/* config section manipulators -- internal */

pub fn init_section(section: &mut ConfigSection, r#type: &[u8], name: Option<&[u8]>) {
    section.r#type = string_init_len(Some(r#type));
    section.name = string_init_len(name);
    section.options = Vec::new();
}

pub fn deinit_section(section: &mut ConfigSection) {
    for option in section.options.iter_mut() {
        deinit_option(option);
    }

    string_deinit(&mut section.r#type);
    string_deinit(&mut section.name);

    section.options = Vec::new();
}

/* config section accessors/manipulators -- by type, name */

/// `config_add_section_len(config, type, type_len, name, name_len)`.
pub fn config_add_section_len<'a>(
    config: &'a mut Config,
    r#type: &[u8],
    name: Option<&[u8]>,
) -> &'a mut ConfigSection {
    config.sections.push(ConfigSection::default());
    let section = config.sections.last_mut().expect("just pushed");

    init_section(section, r#type, name);

    section
}

pub fn config_add_section<'a>(
    config: &'a mut Config,
    r#type: &[u8],
    name: Option<&[u8]>,
) -> &'a mut ConfigSection {
    config_add_section_len(config, c(r#type), name.map(c))
}

/// `config_find_sections(config, type, save)`. The cursor `*save` is an index
/// (`Some(sections_count)` is the end pointer); `None` is `NULL`.
pub fn config_find_sections<'a>(
    config: &'a mut Config,
    r#type: &[u8],
    save: Option<&mut Option<usize>>,
) -> Option<&'a mut ConfigSection> {
    let sections_end = config.sections.len();

    let start = match &save {
        Some(Some(s)) => *s,
        _ => 0,
    };

    let mut section = start;
    while section < sections_end {
        if streq(
            config_string_to_cstr(&config.sections[section].r#type).unwrap_or(b""),
            r#type,
        ) {
            break;
        }
        section += 1;
    }

    if let Some(save) = save {
        *save = Some(section);
    }

    if section < sections_end {
        Some(&mut config.sections[section])
    } else {
        None
    }
}

fn find_section_index(config: &Config, r#type: &[u8], name: Option<&[u8]>) -> Option<usize> {
    config.sections.iter().position(|section| {
        if !streq(
            config_string_to_cstr(&section.r#type).unwrap_or(b""),
            r#type,
        ) {
            return false;
        }
        let section_name = config_string_to_cstr(&section.name);
        match (section_name, name) {
            (Some(a), Some(b)) => streq(a, b),
            (a, b) => a.is_none() && b.is_none(),
        }
    })
}

pub fn config_find_section<'a>(
    config: &'a mut Config,
    r#type: &[u8],
    name: Option<&[u8]>,
) -> Option<&'a mut ConfigSection> {
    let i = find_section_index(config, r#type, name)?;
    Some(&mut config.sections[i])
}

pub fn config_find_or_add_section<'a>(
    config: &'a mut Config,
    r#type: &[u8],
    name: Option<&[u8]>,
) -> &'a mut ConfigSection {
    if let Some(i) = find_section_index(config, r#type, name) {
        return &mut config.sections[i];
    }

    config_add_section(config, r#type, name)
}

/* config option manipulators -- internal */

pub fn init_option_value(option: &mut ConfigOption, value: Option<&[u8]>) {
    option.v = ConfigValue::Value(string_init_len(value));
}

pub fn deinit_option_value(option: &mut ConfigOption) {
    match &mut option.v {
        ConfigValue::Values(_) => {
            // The strings are freed with the array; C leaves `v.values` NULL
            // (so `v.value` reads as NULL) and `values_count` unchanged.
            option.v = ConfigValue::Value(ConfigString(None));
        }
        ConfigValue::Value(v) => string_deinit(v),
    }
}

pub fn init_option(option: &mut ConfigOption, key: &[u8], value: Option<&[u8]>) {
    option.key = string_init_len(Some(key));
    init_option_value(option, value);
}

pub fn deinit_option(option: &mut ConfigOption) {
    string_deinit(&mut option.key);
    deinit_option_value(option);
}

pub fn append_option<'a>(
    section: &'a mut ConfigSection,
    key: &[u8],
    value: Option<&[u8]>,
) -> &'a mut ConfigOption {
    section.options.push(ConfigOption::default());
    let option = section.options.last_mut().expect("just pushed");

    init_option(option, key, value);

    option
}

/// `get_option_len(section, key, key_len)`, returning the option's index.
pub fn get_option_len(section: &ConfigSection, key: &[u8]) -> Option<usize> {
    section
        .options
        .iter()
        .position(|option| string_equal_len(&option.key, key))
}

/* config option accessors/manipulators -- by key */

pub fn config_set_option_len<'a>(
    section: &'a mut ConfigSection,
    key: &[u8],
    value: Option<&[u8]>,
) -> &'a mut ConfigOption {
    if let Some(i) = get_option_len(section, key) {
        return config_set_value_len(&mut section.options[i], value);
    }

    append_option(section, key, value)
}

pub fn config_set_option<'a>(
    section: &'a mut ConfigSection,
    key: &[u8],
    value: Option<&[u8]>,
) -> &'a mut ConfigOption {
    config_set_option_len(section, c(key), value.map(c))
}

fn get_option_index(section: &ConfigSection, key: &[u8]) -> Option<usize> {
    section
        .options
        .iter()
        .position(|option| streq(config_string_to_cstr(&option.key).unwrap_or(b""), key))
}

pub fn config_get_option<'a>(section: &'a ConfigSection, key: &[u8]) -> Option<&'a ConfigOption> {
    get_option_index(section, key).map(|i| &section.options[i])
}

/// `config_get_option` for callers that modify the option (C returns a
/// non-const pointer from a const section).
pub fn config_get_option_mut<'a>(
    section: &'a mut ConfigSection,
    key: &[u8],
) -> Option<&'a mut ConfigOption> {
    get_option_index(section, key).map(|i| &mut section.options[i])
}

pub fn config_get_or_set_option_len<'a>(
    section: &'a mut ConfigSection,
    key: &[u8],
    value: Option<&[u8]>,
) -> &'a mut ConfigOption {
    if let Some(i) = get_option_len(section, key) {
        return &mut section.options[i];
    }

    append_option(section, key, value)
}

pub fn config_get_or_set_option<'a>(
    section: &'a mut ConfigSection,
    key: &[u8],
    value: Option<&[u8]>,
) -> &'a mut ConfigOption {
    config_get_or_set_option_len(section, c(key), value.map(c))
}

pub fn config_set_string_option_len(section: &mut ConfigSection, key: &[u8], value: Option<&[u8]>) {
    config_set_option_len(section, key, value);
}

pub fn config_set_string_option(section: &mut ConfigSection, key: &[u8], value: Option<&[u8]>) {
    config_set_string_option_len(section, c(key), value.map(c));
}

/// `config_get_string_option(section, key, out_value)`; `*out_value` is
/// written only on success.
pub fn config_get_string_option<'a>(
    section: &'a ConfigSection,
    key: &[u8],
    out_value: &mut Option<&'a [u8]>,
) -> bool {
    if let Some(option) = config_get_option(section, key)
        && let Some(value) = config_get_value(Some(option))
    {
        *out_value = Some(value);
        return true;
    }

    false
}

/// Returns a copy of the string C returns (which points either into the
/// section or at `value`).
pub fn config_get_or_set_string_option(
    section: &mut ConfigSection,
    key: &[u8],
    value: Option<&[u8]>,
) -> Option<Vec<u8>> {
    let mut found = value;
    if config_get_string_option(section, key, &mut found) {
        return found.map(<[u8]>::to_vec);
    }
    config_set_string_option_len(section, c(key), value.map(c));
    value.map(|v| c(v).to_vec())
}

const BOOL_VALUES: [[&[u8]; 2]; 4] = [
    [b"0", b"1"],
    [b"no", b"yes"],
    [b"off", b"on"],
    [b"false", b"true"],
];

pub fn config_set_bool_option(
    section: &mut ConfigSection,
    key: &[u8],
    value: bool,
    style: ConfigBoolStyle,
) {
    config_set_option(
        section,
        key,
        Some(BOOL_VALUES[style as usize][if value { 1 } else { 0 }]),
    );
}

pub fn config_get_bool_option(section: &ConfigSection, key: &[u8], out_value: &mut bool) -> bool {
    let mut value = None;
    if config_get_string_option(section, key, &mut value) {
        let value = value.unwrap_or(b"");
        for pair in BOOL_VALUES.iter() {
            for (j, v) in pair.iter().enumerate() {
                if value == *v {
                    *out_value = j != 0;
                    return true;
                }
            }
        }
    }

    false
}

pub fn config_get_or_set_bool_option(
    section: &mut ConfigSection,
    key: &[u8],
    mut value: bool,
    style: ConfigBoolStyle,
) -> bool {
    if !config_get_bool_option(section, key, &mut value) {
        config_set_bool_option(section, key, value, style);
    }
    value
}

pub fn config_set_int_option(section: &mut ConfigSection, key: &[u8], value: i32) {
    let buffer = format!("{value}");

    config_set_option_len(section, c(key), Some(buffer.as_bytes()));
}

/// The C (Apple/FreeBSD libc) `sscanf` integer scanner for `%i` (`base`
/// 0) and `%u` (`base` 10): the converted value (`strtoimax`/`strtoumax`
/// semantics, saturating at 64 bits, as two's-complement bits) and the number
/// of characters consumed, or `None` on input or matching failure.
pub fn scan_integer(s: &[u8], base: u32, unsigned: bool) -> Option<(u64, usize)> {
    const WIDTH: usize = 512; // BUF - 1
    let at = |i: usize| s.get(i).copied().unwrap_or(0);

    let mut i = 0;
    while matches!(at(i), b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r') {
        i += 1;
    }
    if at(i) == 0 {
        return None; // input failure: EOF
    }
    let start = i;

    let mut base = base;
    let mut sign_ok = true;
    let mut have_sign = false;
    let mut ndigits = true;
    let mut nzdigits = true;
    let mut pfx_ok = false;
    let mut p = 0; // characters accepted
    while p < WIDTH {
        let ch = at(start + p);
        let accept = match ch {
            b'0' => {
                if base == 0 {
                    base = 8;
                    pfx_ok = true;
                }
                if nzdigits {
                    sign_ok = false;
                    nzdigits = false;
                    ndigits = false;
                } else {
                    sign_ok = false;
                    pfx_ok = false;
                    ndigits = false;
                }
                true
            }
            b'1'..=b'7' => {
                if base == 0 {
                    base = 10;
                }
                sign_ok = false;
                pfx_ok = false;
                ndigits = false;
                true
            }
            b'8' | b'9' => {
                if base == 0 {
                    base = 10;
                }
                if base <= 8 {
                    false
                } else {
                    sign_ok = false;
                    pfx_ok = false;
                    ndigits = false;
                    true
                }
            }
            b'A'..=b'F' | b'a'..=b'f' => {
                if base <= 10 {
                    false
                } else {
                    sign_ok = false;
                    pfx_ok = false;
                    ndigits = false;
                    true
                }
            }
            b'+' | b'-' => {
                if sign_ok {
                    sign_ok = false;
                    have_sign = true;
                    true
                } else {
                    false
                }
            }
            b'x' | b'X' if pfx_ok && p == 1 + usize::from(have_sign) => {
                base = 16;
                pfx_ok = false;
                true
            }
            _ => false,
        };
        if !accept {
            break;
        }
        p += 1;
    }

    if ndigits {
        return None; // matching failure
    }
    // Apple/FreeBSD libc pushes a trailing "x" of a bare "0x" back (the
    // number is "0"); glibc keeps it consumed.
    if cfg!(not(target_os = "linux")) && matches!(at(start + p - 1), b'x' | b'X') {
        p -= 1;
    }

    let digits = &s[start..start + p];
    Some((strto(digits, base, unsigned), start + p))
}

/// `strtoimax`/`strtoumax(s, NULL, base)` on an already validated number.
fn strto(s: &[u8], base: u32, unsigned: bool) -> u64 {
    let mut i = 0;
    let mut neg = false;
    if matches!(s.first(), Some(b'+' | b'-')) {
        neg = s[0] == b'-';
        i = 1;
    }
    if base == 16 && s.get(i) == Some(&b'0') && matches!(s.get(i + 1), Some(b'x' | b'X')) {
        i += 2;
    }
    let mut acc: u64 = 0;
    let mut overflow = false;
    for &ch in &s[i..] {
        let d = match ch {
            b'0'..=b'9' => u64::from(ch - b'0'),
            b'a'..=b'f' => u64::from(ch - b'a' + 10),
            b'A'..=b'F' => u64::from(ch - b'A' + 10),
            _ => break,
        };
        match acc
            .checked_mul(u64::from(base))
            .and_then(|a| a.checked_add(d))
        {
            Some(a) => acc = a,
            None => overflow = true,
        }
    }
    if unsigned {
        if overflow {
            u64::MAX
        } else if neg {
            acc.wrapping_neg()
        } else {
            acc
        }
    } else if neg {
        if overflow || acc > 1 << 63 {
            i64::MIN as u64
        } else {
            acc.wrapping_neg()
        }
    } else if overflow || acc > i64::MAX as u64 {
        i64::MAX as u64
    } else {
        acc
    }
}

pub fn config_get_int_option(section: &ConfigSection, key: &[u8], out_value: &mut i32) -> bool {
    let mut value = None;
    if config_get_string_option(section, key, &mut value) {
        let value = value.unwrap_or(b"");
        if let Some((i, n)) = scan_integer(value, 0, false)
            && n == value.len()
        {
            // must be entire string
            *out_value = i as i32;
            return true;
        }
    }

    false
}

pub fn config_get_or_set_int_option(
    section: &mut ConfigSection,
    key: &[u8],
    mut value: i32,
) -> i32 {
    if !config_get_int_option(section, key, &mut value) {
        config_set_int_option(section, key, value);
    }
    value
}

pub fn config_set_uint_option(section: &mut ConfigSection, key: &[u8], value: u32) {
    let buffer = format!("{value}");

    config_set_option_len(section, c(key), Some(buffer.as_bytes()));
}

pub fn config_get_uint_option(section: &ConfigSection, key: &[u8], out_value: &mut u32) -> bool {
    let mut value = None;
    if config_get_string_option(section, key, &mut value) {
        let value = value.unwrap_or(b"");
        if let Some((u, n)) = scan_integer(value, 10, true)
            && n == value.len()
        {
            // must be entire string
            *out_value = u as u32;
            return true;
        }
    }

    false
}

pub fn config_get_or_set_uint_option(
    section: &mut ConfigSection,
    key: &[u8],
    mut value: u32,
) -> u32 {
    if !config_get_uint_option(section, key, &mut value) {
        config_set_uint_option(section, key, value);
    }
    value
}

/* config option accessors/manipulators -- by reference */

pub fn config_set_value_len<'a>(
    option: &'a mut ConfigOption,
    value: Option<&[u8]>,
) -> &'a mut ConfigOption {
    deinit_option_value(option);

    init_option_value(option, value);

    option
}

pub fn config_set_value<'a>(
    option: &'a mut ConfigOption,
    value: Option<&[u8]>,
) -> &'a mut ConfigOption {
    config_set_value_len(option, value.map(c))
}

pub fn config_add_value_len<'a>(
    option: &'a mut ConfigOption,
    value: &[u8],
) -> &'a mut ConfigOption {
    match &mut option.v {
        ConfigValue::Value(v) if config_string_to_cstr(v).is_some() => {
            // convert 'item' to 'list'
            let option_value = v.clone();
            option.v = ConfigValue::Values(vec![option_value, string_init_len(Some(value))]);
        }
        ConfigValue::Value(_) => {
            option.v = ConfigValue::Values(vec![string_init_len(Some(value))]);
        }
        ConfigValue::Values(values) => values.push(string_init_len(Some(value))),
    }

    option
}

pub fn config_add_value<'a>(option: &'a mut ConfigOption, value: &[u8]) -> &'a mut ConfigOption {
    config_add_value_len(option, c(value))
}

pub fn config_remove_value(option: &mut ConfigOption, i: u32) -> Option<&mut ConfigOption> {
    if !config_is_value_list(option) {
        if i > 0 {
            return None;
        }

        config_set_value_len(option, None);
    } else {
        let ConfigValue::Values(values) = &mut option.v else {
            // An empty 'list' (`values_count == 0`, `NULL` value).
            return None;
        };
        let i = i as usize;
        if i >= values.len() {
            return None;
        }

        values.remove(i);

        if values.is_empty() {
            option.v = ConfigValue::Value(string_init_len(None));
        }
    }

    Some(option)
}

pub fn config_get_value(option: Option<&ConfigOption>) -> Option<&[u8]> {
    match option {
        Some(ConfigOption {
            v: ConfigValue::Value(v),
            ..
        }) => config_string_to_cstr(v),
        _ => None,
    }
}

pub fn config_is_value_list(option: &ConfigOption) -> bool {
    match &option.v {
        ConfigValue::Values(_) => true,
        ConfigValue::Value(v) => config_string_to_cstr(v).is_none(),
    }
}

pub fn config_get_value_count(option: &ConfigOption) -> u32 {
    match &option.v {
        ConfigValue::Values(v) => v.len() as u32,
        ConfigValue::Value(v) => u32::from(config_string_to_cstr(v).is_some()),
    }
}

/* config parser */

fn at(buffer: &[u8], i: usize) -> u8 {
    buffer.get(i).copied().unwrap_or(0)
}

pub fn is_whitespace(c: u8) -> bool {
    c == b'\t' || c == b' '
}

pub fn is_end(c: u8) -> bool {
    c == b'\0' || c == b'\n' || c == b'\r'
}

pub fn is_whitespace_or_end(c: u8) -> bool {
    is_whitespace(c) || is_end(c)
}

/// `Directive`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Directive {
    InvalidDirective = 0,
    SectionDirective,
    ItemDirective,
    ListDirective,
}

pub fn match_directive(buffer: &[u8], index: &mut usize) -> Directive {
    let mut i = *index;

    while is_whitespace(at(buffer, i)) {
        i += 1;
    }

    let starts = |w: &[u8]| (0..w.len()).all(|k| at(buffer, i + k) == w[k]);

    let directive;

    if starts(b"section") {
        directive = Directive::SectionDirective;
        i += 7;
    } else if starts(b"item") {
        directive = Directive::ItemDirective;
        i += 4;
    } else if starts(b"list") {
        directive = Directive::ListDirective;
        i += 4;
    } else {
        return Directive::InvalidDirective;
    }

    if !is_whitespace_or_end(at(buffer, i)) {
        return Directive::InvalidDirective;
    }

    *index = i;

    directive
}

pub fn match_nonquote_field(buffer: &[u8], index: &mut usize, length: &mut usize) -> bool {
    let mut i = *index;

    loop {
        let c = at(buffer, i);

        if is_whitespace_or_end(c) {
            break;
        } else if c <= b' ' || c > b'~' || c == b'#' || c == b'\'' || c == b'"' {
            return false;
        }
        i += 1;
    }

    *length = i - *index;
    *index = i;

    *length > 0
}

/// A hex digit's offset as C computes `m` (0 for a non-digit).
fn hex_base(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => b'0',
        b'a'..=b'f' => b'a' - 10,
        b'A'..=b'F' => b'A' - 10,
        _ => 0,
    }
}

pub fn parse_quote_field(buffer: &mut [u8], index: &mut usize, length: &mut usize) -> bool {
    let mut i = *index;
    let mut o = *index;

    let quote = at(buffer, i);

    loop {
        i += 1;
        let mut c = at(buffer, i);

        if c == quote {
            i += 1;
            break;
        } else if c == b'\\' {
            i += 1;
            c = at(buffer, i);
            if c == quote {
                buffer[o] = quote;
                o += 1;
            } else {
                let out = match c {
                    b't' => b'\t',
                    b'n' => b'\n',
                    b'r' => b'\r',
                    b'\\' => b'\\',
                    b'x' => {
                        // parse two hex digits
                        i += 1;
                        c = at(buffer, i);
                        let m = hex_base(c);
                        if m == 0 {
                            return false;
                        }
                        let h = c - m;
                        i += 1;
                        c = at(buffer, i);
                        let m = hex_base(c);
                        if m == 0 {
                            return false;
                        }
                        (h << 4) | (c - m)
                    }
                    _ => return false,
                };
                buffer[o] = out;
                o += 1;
            }
        } else if (b' '..=b'~').contains(&c) {
            buffer[o] = c;
            o += 1;
        } else {
            return false;
        }
    }

    *length = o - *index;
    *index = i;

    true
}

pub fn parse_field(
    buffer: &mut [u8],
    index: &mut usize,
    start: &mut usize,
    length: &mut usize,
) -> bool {
    let mut i = *index;

    while is_whitespace(at(buffer, i)) {
        i += 1;
    }

    *start = i;

    if at(buffer, i) == b'"' || at(buffer, i) == b'\'' {
        if !parse_quote_field(buffer, &mut i, length) {
            return false;
        }
    } else if !match_nonquote_field(buffer, &mut i, length) {
        return false;
    }

    if !is_whitespace_or_end(at(buffer, i)) {
        return false; // C: `return INVALID_DIRECTIVE;`
    }

    *index = i;

    true
}

pub fn config_parse(config: &mut Config, file: &mut Stream) -> bool {
    config_init(config);

    // `section` and `option` point into `config`: kept as indices.
    let mut section: Option<usize> = None;
    let mut option: Option<usize> = None;

    let mut buffer_cap: usize = 128;
    let mut buffer = vec![0u8; buffer_cap];
    let mut buffer_end: usize = 1;
    buffer[buffer_end - 1] = b'\0';

    let mut line: usize;
    let mut next_line: usize = 0;
    loop {
        // `for (line = 0, next_line = 0; ; line = next_line)`; `continue`
        // runs the increment too.
        line = next_line;
        /* find beginning of next line */
        while next_line < buffer_end {
            let ch = buffer[next_line];

            if ch == b'\0' && next_line == buffer_end - 1 {
                if line > 0 {
                    /* shift to front */
                    buffer.copy_within(line..buffer_end, 0);
                    buffer_end -= line;
                    next_line -= line;
                    line = 0;
                } else if buffer_end > 1 {
                    /* need larger capacity */
                    buffer_cap *= 2;
                    buffer.resize(buffer_cap, 0);
                }

                let (read, _) = file.read(&mut buffer[buffer_end - 1..buffer_cap - 1]);
                if read == 0 {
                    break;
                }

                buffer_end += read;
                buffer[buffer_end - 1] = b'\0';
            } else {
                next_line += 1;

                if ch == b'\n' || ch == b'\r' {
                    break;
                }
            }
        }

        /* if at end of file */
        if next_line == line {
            break;
        }

        let mut i = line;

        let directive = match_directive(&buffer, &mut i);

        match directive {
            Directive::InvalidDirective => continue,
            Directive::SectionDirective => {
                let mut type_start = 0;
                let mut type_length = 0;

                if !parse_field(&mut buffer, &mut i, &mut type_start, &mut type_length) {
                    continue;
                }

                let mut name_start = 0;
                let mut name_length = 0;

                let has_name = parse_field(&mut buffer, &mut i, &mut name_start, &mut name_length);

                config_add_section_len(
                    config,
                    &buffer[type_start..type_start + type_length],
                    if has_name {
                        Some(&buffer[name_start..name_start + name_length])
                    } else {
                        None
                    },
                );
                section = Some(config.sections.len() - 1);
                option = None;
            }
            Directive::ItemDirective | Directive::ListDirective => {
                let Some(s) = section else {
                    continue;
                };

                let mut key_start = 0;
                let mut key_length = 0;

                if !parse_field(&mut buffer, &mut i, &mut key_start, &mut key_length) {
                    continue;
                }

                let mut value_start = 0;
                let mut value_length = 0;

                if !parse_field(&mut buffer, &mut i, &mut value_start, &mut value_length) {
                    continue;
                }

                let key = &buffer[key_start..key_start + key_length];
                let value = &buffer[value_start..value_start + value_length];
                let section = &mut config.sections[s];

                if directive == Directive::ItemDirective {
                    config_set_option_len(section, key, Some(value));
                    option = get_option_len(section, key);
                } else {
                    let same =
                        option.is_some_and(|o| string_equal_len(&section.options[o].key, key));
                    if !same {
                        config_get_or_set_option_len(section, key, None);
                        option = get_option_len(section, key);
                    }
                    if let Some(o) = option {
                        config_add_value_len(&mut section.options[o], value);
                    }
                }
            }
        }

        debug_assert!(i <= next_line);
    }

    true
}

/* config writer */

pub fn write_field(field: &ConfigString, file: &mut Stream) {
    file.putc(b'\'');

    let mut buffer = [0u8; 128];
    let mut o = 0;

    for &ch in config_string_to_cstr(field).unwrap_or(b"") {
        let l = match ch {
            b'\t' | b'\n' | b'\r' | b'\'' | b'\\' => 2,
            _ => {
                if (b' '..=b'~').contains(&ch) {
                    1
                } else {
                    4
                }
            }
        };

        if o + l > buffer.len() {
            file.write(&buffer[..o]);
            o = 0;
        }

        match l {
            1 => {
                buffer[o] = ch;
                o += 1;
            }
            2 => {
                buffer[o] = b'\\';
                buffer[o + 1] = match ch {
                    b'\t' => b't',
                    b'\n' => b'n',
                    b'\r' => b'r',
                    _ => ch, // '\'' and '\\'
                };
                o += 2;
            }
            _ => {
                let hex = |n: u8| if n < 10 { b'0' + n } else { b'a' - 10 + n };
                buffer[o] = b'\\';
                buffer[o + 1] = b'x';
                buffer[o + 2] = hex((ch >> 4) & 0x0f);
                buffer[o + 3] = hex(ch & 0x0f);
                o += 4;
            }
        }
    }

    if o > 0 {
        file.write(&buffer[..o]);
    }

    file.putc(b'\'');
}

pub fn config_write(config: &Config, file: &mut Stream) {
    for section in &config.sections {
        file.puts(b"section ");
        write_field(&section.r#type, file);
        if config_string_to_cstr(&section.name).is_some() {
            file.putc(b' ');
            write_field(&section.name, file);
        }
        file.putc(b'\n');

        for option in &section.options {
            match &option.v {
                ConfigValue::Value(v) if config_string_to_cstr(v).is_some() => {
                    file.puts(b"\titem ");
                    write_field(&option.key, file);
                    file.putc(b' ');
                    write_field(v, file);
                    file.putc(b'\n');
                }
                ConfigValue::Value(_) => {}
                ConfigValue::Values(values) => {
                    for value in values {
                        file.puts(b"\tlist ");
                        write_field(&option.key, file);
                        file.putc(b' ');
                        write_field(value, file);
                        file.putc(b'\n');
                    }
                }
            }
        }

        file.putc(b'\n');
    }
}
