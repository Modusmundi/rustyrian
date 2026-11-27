// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) rustyrian contributors

//! Platform call records and the `.jsonl` trace format.
//!
//! Line 1: `{"trace":1,"engine":"c"|"rust","scenario":"<name>"}`.
//! One line per present:
//! `{"frame":N,"ms":T,"rgb":"<hex>","idx":"<hex>","pal":"<hex>","audio":"<hex>","samples":K,"calls":"<hex>"}`
//! (plus `"sim"` with `--sim-digest`). Last line:
//! `{"end":"limit"|"exit","code":N,"calls":"<hex>"}`, where `calls` covers the
//! platform calls after the last present (for example `atexit` teardown).
//! All hashes are xxh3-64, 16 lowercase hex digits.

use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Serialize, Serializer};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use xxhash_rust::xxh3::xxh3_64;

/// A logged argument or return value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Arg {
    Int(i64),
    Str(String),
    /// xxh3 of a byte buffer.
    Bytes(u64),
    /// `void`, `NULL`, or an absent optional.
    None,
    List(Vec<Arg>),
}

impl Serialize for Arg {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Arg::Int(v) => s.serialize_i64(*v),
            Arg::Str(v) => s.serialize_str(v),
            Arg::Bytes(h) => {
                let mut m = s.serialize_map(Some(1))?;
                m.serialize_entry("h", &hex(*h))?;
                m.end()
            }
            Arg::None => s.serialize_unit(),
            Arg::List(items) => {
                let mut q = s.serialize_seq(Some(items.len()))?;
                for i in items {
                    q.serialize_element(i)?;
                }
                q.end()
            }
        }
    }
}

macro_rules! arg_int {
    ($($t:ty),*) => {$(
        impl From<$t> for Arg {
            fn from(v: $t) -> Self { Arg::Int(i64::from(v)) }
        }
    )*};
}
arg_int!(i8, u8, i16, u16, i32, u32, i64, bool);

impl From<&str> for Arg {
    fn from(v: &str) -> Self {
        Arg::Str(v.to_owned())
    }
}

impl From<String> for Arg {
    fn from(v: String) -> Self {
        Arg::Str(v)
    }
}

impl From<()> for Arg {
    fn from((): ()) -> Self {
        Arg::None
    }
}

impl<T: Into<Arg>> From<Option<T>> for Arg {
    fn from(v: Option<T>) -> Self {
        v.map_or(Arg::None, Into::into)
    }
}

/// One intercepted platform call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformCall {
    /// The C function name (`SDL_GetTicks`, `time`, ...).
    pub name: &'static str,
    pub args: Vec<Arg>,
    pub ret: Arg,
}

impl Serialize for PlatformCall {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(Some(3))?;
        m.serialize_entry("f", self.name)?;
        m.serialize_entry("a", &self.args)?;
        m.serialize_entry("r", &self.ret)?;
        m.end()
    }
}

pub fn hex(h: u64) -> String {
    format!("{h:016x}")
}

pub fn hash(bytes: &[u8]) -> u64 {
    xxh3_64(bytes)
}

/// One trace frame line.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct FrameLine {
    pub frame: u64,
    pub ms: u32,
    pub rgb: String,
    pub idx: String,
    pub pal: String,
    pub audio: String,
    pub samples: u64,
    pub calls: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sim: Option<String>,
}

pub struct TraceWriter {
    out: BufWriter<File>,
}

impl TraceWriter {
    pub fn create(path: &Path, engine: &str, scenario: &str) -> std::io::Result<Self> {
        let mut out = BufWriter::new(File::create(path)?);
        let header = serde_json::json!({ "trace": 1, "engine": engine, "scenario": scenario });
        writeln!(out, "{header}")?;
        Ok(TraceWriter { out })
    }

    pub fn frame(&mut self, line: &FrameLine) -> std::io::Result<()> {
        serde_json::to_writer(&mut self.out, line)?;
        self.out.write_all(b"\n")
    }

    pub fn end(&mut self, kind: &str, code: i32, calls: &str) -> std::io::Result<()> {
        let end = serde_json::json!({ "end": kind, "code": code, "calls": calls });
        writeln!(self.out, "{end}")?;
        self.out.flush()
    }
}
