//! `cargo xtask fetch-data`: the freeware Tyrian 2.1 data, verified by hash.

use crate::tools::{run_status, sha256_hex};
use anyhow::{Context, Result, bail};
use std::io::Read;
use std::path::Path;
use std::process::Command;

const URL: &str = "https://camanis.net/tyrian/tyrian21.zip";
const SHA256: &str = "7790d09a2a3addcd33c66ef063d5900eb81cc9c342f4807eb8356364dd1d9277";

pub fn run(root: &Path) -> Result<()> {
    let data = root.join("data");
    let stamp = data.join(".sha256");
    if std::fs::read_to_string(&stamp).is_ok_and(|s| s.trim() == SHA256)
        && data.join("tyrian1.lvl").is_file()
    {
        eprintln!("data/ already matches {SHA256}");
        return Ok(());
    }

    let tmp = root.join("target/xtask");
    std::fs::create_dir_all(&tmp)?;
    let zip_path = tmp.join("tyrian21.zip");
    run_status(
        Command::new("curl")
            .args(["-fsSL", "--retry", "3", "-o"])
            .arg(&zip_path)
            .arg(URL),
    )?;
    let bytes = std::fs::read(&zip_path)?;
    let got = sha256_hex(&bytes);
    if got != SHA256 {
        bail!("tyrian21.zip SHA-256 mismatch: expected {SHA256}, got {got}");
    }

    std::fs::create_dir_all(&data)?;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
    let mut count = 0usize;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        if entry.is_dir() {
            continue;
        }
        let raw_name = entry.name()?.into_owned();
        let name = Path::new(&raw_name)
            .file_name()
            .context("zip entry without file name")?
            .to_string_lossy()
            .to_lowercase();
        let mut buf = Vec::with_capacity(usize::try_from(entry.size())?);
        entry.read_to_end(&mut buf)?;
        std::fs::write(data.join(&name), buf).with_context(|| format!("writing data/{name}"))?;
        count += 1;
    }
    if !data.join("tyrian1.lvl").is_file() {
        bail!("tyrian21.zip did not contain tyrian1.lvl");
    }
    std::fs::write(&stamp, format!("{SHA256}\n"))?;
    eprintln!("extracted {count} files into data/");
    Ok(())
}
