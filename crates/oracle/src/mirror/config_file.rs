//! Mirror of `rustyrian_core::config_file` types <-> their C layouts.
//! `config_file.c` has no globals; the `Config` conversions serve the mirrors
//! of modules holding a `Config` (`config.c`) and the differential tests.

use super::Mirror;
use crate::bindings as b;
use rustyrian_core::config_file::{
    Config, ConfigOption, ConfigSection, ConfigString, ConfigValue, State,
};
use std::ffi::{CStr, c_char};

impl Mirror for State {
    fn load_from_c(&mut self) {}

    fn store_to_c(&self) {}
}

/// `CONFIG_STRING_LONG_TAG`.
const LONG_TAG: usize = 15;

/// Converts a C `ConfigString`.
///
/// # Safety
/// `c` is a valid C `ConfigString` (initialised by `string_init_len`, or
/// all zero).
pub unsafe fn string_from_c(c: &b::ConfigString) -> ConfigString {
    // SAFETY: both union views are plain data; the tag byte selects one.
    unsafe {
        if c.short_buf[LONG_TAG] != 0 {
            if c.long_buf.is_null() {
                ConfigString(None)
            } else {
                ConfigString(Some(CStr::from_ptr(c.long_buf).to_bytes().to_vec()))
            }
        } else {
            ConfigString(Some(
                CStr::from_ptr(c.short_buf.as_ptr()).to_bytes().to_vec(),
            ))
        }
    }
}

/// A C `ConfigString` holding `r` as `string_init_len` would build it (long
/// strings are `malloc`ed, so C can `free` them).
pub fn string_to_c(r: &ConfigString) -> b::ConfigString {
    let mut c = b::ConfigString::default();
    match &r.0 {
        None => {
            // SAFETY: writing plain data into the union.
            unsafe {
                c.short_buf[LONG_TAG] = 1;
                c.long_buf = std::ptr::null_mut();
            }
        }
        Some(s) if s.len() >= 16 => {
            // SAFETY: allocating `len + 1` bytes and filling them.
            unsafe {
                let p = libc::malloc(s.len() + 1).cast::<u8>();
                assert!(!p.is_null(), "out of memory");
                std::ptr::copy_nonoverlapping(s.as_ptr(), p, s.len());
                *p.add(s.len()) = 0;
                c.long_buf = p.cast();
                c.short_buf[LONG_TAG] = 1;
            }
        }
        Some(s) => {
            // SAFETY: writing plain data into the union.
            unsafe {
                for (d, &v) in c.short_buf.iter_mut().zip(s.iter()) {
                    *d = v as c_char;
                }
                c.short_buf[s.len()] = 0;
                c.short_buf[LONG_TAG] = 0;
            }
        }
    }
    c
}

/// # Safety
/// `c` is a valid C `ConfigOption`.
unsafe fn option_from_c(c: &b::ConfigOption) -> ConfigOption {
    // SAFETY: per the caller; `values_count` tags the union.
    unsafe {
        ConfigOption {
            key: string_from_c(&c.key),
            v: if c.values_count == 0 {
                ConfigValue::Value(string_from_c(&c.v.value))
            } else {
                ConfigValue::Values(
                    std::slice::from_raw_parts(c.v.values, c.values_count as usize)
                        .iter()
                        .map(|s| string_from_c(s))
                        .collect(),
                )
            },
        }
    }
}

/// `malloc`ed copy of `items` converted with `f` (NULL when empty).
fn c_array<T, U>(items: &[T], f: impl Fn(&T) -> U) -> *mut U {
    if items.is_empty() {
        return std::ptr::null_mut();
    }
    // SAFETY: allocating and initialising `items.len()` elements.
    unsafe {
        let p = libc::malloc(items.len() * std::mem::size_of::<U>()).cast::<U>();
        assert!(!p.is_null(), "out of memory");
        for (i, item) in items.iter().enumerate() {
            p.add(i).write(f(item));
        }
        p
    }
}

fn option_to_c(r: &ConfigOption) -> b::ConfigOption {
    let mut c = b::ConfigOption {
        key: string_to_c(&r.key),
        ..Default::default()
    };
    match &r.v {
        ConfigValue::Value(v) => {
            c.values_count = 0;
            c.v.value = string_to_c(v);
        }
        ConfigValue::Values(vs) => {
            c.values_count = vs.len() as u32;
            c.v.values = c_array(vs, string_to_c);
        }
    }
    c
}

/// Converts a C `Config` (a `config_deinit`ed one, with `sections == NULL`,
/// reads as empty).
///
/// # Safety
/// `c` is a valid C `Config`.
pub unsafe fn config_from_c(c: &b::Config) -> Config {
    if c.sections.is_null() {
        return Config::default();
    }
    // SAFETY: per the caller, `sections` holds `sections_count` sections.
    unsafe {
        Config {
            sections: std::slice::from_raw_parts(c.sections, c.sections_count as usize)
                .iter()
                .map(|s| ConfigSection {
                    r#type: string_from_c(&s.type_),
                    name: string_from_c(&s.name),
                    options: if s.options.is_null() {
                        Vec::new()
                    } else {
                        std::slice::from_raw_parts(s.options, s.options_count as usize)
                            .iter()
                            .map(|o| option_from_c(o))
                            .collect()
                    },
                })
                .collect(),
        }
    }
}

/// Overwrites `c` with a `malloc`ed copy of `r` (the previous contents are not
/// freed).
///
/// # Safety
/// `c` is writable; C may later `config_deinit` it.
pub unsafe fn config_to_c(r: &Config, c: &mut b::Config) {
    c.sections_count = r.sections.len() as u32;
    c.sections = c_array(&r.sections, |s| b::ConfigSection {
        type_: string_to_c(&s.r#type),
        name: string_to_c(&s.name),
        options_count: s.options.len() as u32,
        options: c_array(&s.options, option_to_c),
    });
}
