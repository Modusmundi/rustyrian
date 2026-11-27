//! `porting/inventory.json`: the authoritative list of upstream C functions,
//! mutable globals and intercepted platform calls, produced by `gen-oracle`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Inventory {
    pub upstream: String,
    pub functions: Vec<Function>,
    pub globals: Vec<Global>,
    pub platform_calls: Vec<String>,
    pub libc_calls: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Function {
    /// Upstream file name, e.g. `tyrian2.c` or `sprite.h`.
    pub file: String,
    pub name: String,
    #[serde(rename = "static")]
    pub is_static: bool,
    pub inline_header: bool,
    pub begin_line: u32,
    pub end_line: u32,
    pub c_sha: String,
    /// Upstream project functions referenced by the body (callees and
    /// address-taken functions), sorted.
    pub calls: Vec<String>,
}

impl Function {
    /// Manifest key: `file::name`.
    pub fn key(&self) -> String {
        format!("{}::{}", self.file, self.name)
    }

    /// The Rust module this function is ported into (rule R1).
    pub fn module(&self) -> String {
        module_of(&self.file)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Global {
    pub file: String,
    pub name: String,
    /// Enclosing function for function-local statics.
    pub function: Option<String>,
    pub line: u32,
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(rename = "static")]
    pub is_static: bool,
}

/// Rule R1: one Rust module per C file, lowercase, `.c`/`.h` stripped.
pub fn module_of(file: &str) -> String {
    let stem = file
        .strip_suffix(".c")
        .or_else(|| file.strip_suffix(".h"))
        .unwrap_or(file);
    stem.to_lowercase()
}

pub fn path(root: &Path) -> std::path::PathBuf {
    root.join("porting/inventory.json")
}

pub fn load(root: &Path) -> Result<Inventory> {
    let p = path(root);
    let text = std::fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()))?;
    Ok(serde_json::from_str(&text)?)
}
