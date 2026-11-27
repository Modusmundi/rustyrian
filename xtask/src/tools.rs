//! Small shared helpers: repository paths, hashing, process execution.

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Pinned upstream commit; `gen-oracle` refuses to run against anything else.
pub const UPSTREAM_COMMIT: &str = "26e40a2012903812f16aae5dd6af2be50b96b5cb";

pub fn repo_root() -> Result<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest
        .parent()
        .context("xtask has no parent directory")?
        .to_path_buf();
    Ok(root)
}

pub fn upstream_src(root: &Path) -> PathBuf {
    root.join("vendor/opentyrian/src")
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// Runs a command, failing with its stderr if it exits non-zero; returns stdout.
pub fn run_capture(cmd: &mut Command) -> Result<Vec<u8>> {
    let out = cmd
        .output()
        .with_context(|| format!("failed to spawn {cmd:?}"))?;
    if !out.status.success() {
        bail!(
            "{cmd:?} exited with {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(out.stdout)
}

/// Runs a command with inherited stdio, failing on non-zero exit.
pub fn run_status(cmd: &mut Command) -> Result<()> {
    let status = cmd
        .status()
        .with_context(|| format!("failed to spawn {cmd:?}"))?;
    if !status.success() {
        bail!("{cmd:?} exited with {status}");
    }
    Ok(())
}

pub fn write_if_changed(path: &Path, contents: &[u8]) -> Result<bool> {
    if std::fs::read(path).ok().as_deref() == Some(contents) {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, contents).with_context(|| format!("writing {}", path.display()))?;
    Ok(true)
}
