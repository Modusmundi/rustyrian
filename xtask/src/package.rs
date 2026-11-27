//! `cargo xtask package --target <triple> [--binary PATH]... [--out DIR]`:
//! release archives in the layout of upstream's release workflows.
//!
//! |Target|Archive|Contents|
//! |---|---|---|
//! |`*-linux-*`|`rustyrian-<version>-linux-<arch>.tar.gz`|`rustyrian/{rustyrian, data/, COPYING, licenses/}`|
//! |`*-windows-*`|`rustyrian-<version>-windows-<arch>.zip`|`rustyrian/{rustyrian.exe, data/, COPYING.txt, licenses/}`|
//! |`*-apple-darwin`|`rustyrian-<version>-macos-<arch>.zip`|`Rustyrian.app` (below)|
//!
//! `universal-apple-darwin` joins an `aarch64` and an `x86_64` binary with
//! `lipo` into `rustyrian-<version>-macos-universal.zip`. The app bundle
//! follows upstream `make_macos.sh`: `Contents/Info.plist`,
//! `Contents/MacOS/rustyrian`, and in `Contents/Resources` the data (found
//! through `SDL_GetBasePath`), `COPYING.txt`, `licenses/` and the icon (the
//! 128 px Linux icon as an `ic07` PNG entry). SDL2 is linked in statically
//! (`sdl2-sys` features `bundled,static-link`), so no framework travels with
//! it. On a macOS host the bundle is ad-hoc signed, as upstream does.
//!
//! Without `--binary`, the target is built with `cargo auditable build
//! --release -p rustyrian` (installing cargo-auditable if it is missing).
//! Archives are reproducible: fixed timestamps, sorted entries.

use crate::tools::run_status;
use anyhow::{Context, Result, bail};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The pseudo-triple of the macOS universal package.
pub const UNIVERSAL: &str = "universal-apple-darwin";
const MAC_TRIPLES: [&str; 2] = ["aarch64-apple-darwin", "x86_64-apple-darwin"];

pub struct Options {
    pub target: String,
    pub binaries: Vec<PathBuf>,
    pub out: Option<PathBuf>,
}

/// Source files of a package, outside the binary.
pub struct Inputs {
    /// `data/` as `cargo xtask fetch-data` leaves it.
    pub data: PathBuf,
    pub copying: PathBuf,
    /// `licenses/` (`Tyrian.txt`: upstream `doc/tyrian-freeware-license.txt`;
    /// `SDL2.txt`: SDL2's zlib license).
    pub licenses: PathBuf,
    /// Upstream `linux/icons/tyrian-128.png`.
    pub icon: PathBuf,
}

impl Inputs {
    pub fn of(root: &Path) -> Inputs {
        Inputs {
            data: root.join("data"),
            copying: root.join("COPYING"),
            licenses: root.join("licenses"),
            icon: root.join("vendor/opentyrian/linux/icons/tyrian-128.png"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    TarGz,
    Zip,
    App,
}

/// Archive kind and platform name (`linux-x86_64`, `macos-universal`, ...).
pub fn platform(target: &str) -> Result<(Kind, String)> {
    if target == UNIVERSAL {
        return Ok((Kind::App, "macos-universal".to_owned()));
    }
    let arch = target.split('-').next().unwrap_or_default();
    if target.contains("-linux-") {
        Ok((Kind::TarGz, format!("linux-{arch}")))
    } else if target.contains("-windows-") {
        Ok((Kind::Zip, format!("windows-{arch}")))
    } else if target.ends_with("-apple-darwin") {
        Ok((Kind::App, format!("macos-{arch}")))
    } else {
        bail!("no package layout for target {target}")
    }
}

/// The archive file name for a version and target.
pub fn archive_name(version: &str, target: &str) -> Result<String> {
    let (kind, platform) = platform(target)?;
    let ext = if kind == Kind::TarGz { "tar.gz" } else { "zip" };
    Ok(format!("rustyrian-{version}-{platform}.{ext}"))
}

/// The workspace version (`[workspace.package] version`).
pub fn version(root: &Path) -> Result<String> {
    let text = std::fs::read_to_string(root.join("Cargo.toml"))?;
    let doc: toml::Table = toml::from_str(&text)?;
    doc.get("workspace")
        .and_then(|w| w.get("package"))
        .and_then(|p| p.get("version"))
        .and_then(|v| v.as_str())
        .map(str::to_owned)
        .context("Cargo.toml has no [workspace.package] version")
}

pub fn run(root: &Path, opts: &Options) -> Result<()> {
    let version = version(root)?;
    let (kind, _) = platform(&opts.target)?;
    let binary = binary(root, opts)?;
    let out_dir = opts
        .out
        .clone()
        .unwrap_or_else(|| root.join("target/package"));
    let archive = out_dir.join(archive_name(&version, &opts.target)?);
    let stage = out_dir.join("stage").join(&opts.target);
    if stage.exists() {
        std::fs::remove_dir_all(&stage)?;
    }
    let inputs = Inputs::of(root);
    match kind {
        Kind::TarGz | Kind::Zip => {
            let windows = opts.target.contains("-windows-");
            stage_plain(&inputs, &binary, &stage, windows)?;
        }
        Kind::App => {
            let app = stage_app(&inputs, &binary, &stage, &version)?;
            if cfg!(target_os = "macos") {
                run_status(
                    Command::new("codesign")
                        .args(["--force", "-s", "-"])
                        .arg(&app),
                )?;
            }
        }
    }
    match kind {
        Kind::TarGz => write_tar_gz(&stage, &archive)?,
        Kind::Zip | Kind::App => write_zip(&stage, &archive)?,
    }
    let listing = list_archive(&archive)?;
    if !listing.iter().any(|(p, m)| is_executable(p) && *m == 0o755) {
        bail!("{}: no executable entry", archive.display());
    }
    eprintln!("package: {} entries", listing.len());
    println!("{}", archive.display());
    Ok(())
}

/// The executable to package: `--binary`, the `lipo` of two for the
/// universal target, or a fresh `cargo auditable` release build.
fn binary(root: &Path, opts: &Options) -> Result<PathBuf> {
    let universal = opts.target == UNIVERSAL;
    let binaries = if !opts.binaries.is_empty() {
        opts.binaries.clone()
    } else if universal {
        MAC_TRIPLES
            .iter()
            .map(|t| build(root, t))
            .collect::<Result<_>>()?
    } else {
        vec![build(root, &opts.target)?]
    };
    match (universal, binaries.as_slice()) {
        (false, [one]) => Ok(one.clone()),
        (true, [a, b]) => {
            let out = root.join("target/package/rustyrian-universal");
            std::fs::create_dir_all(out.parent().context("parent")?)?;
            run_status(
                Command::new("lipo")
                    .arg("-create")
                    .arg("-output")
                    .arg(&out)
                    .arg(a)
                    .arg(b),
            )?;
            Ok(out)
        }
        (false, _) => bail!("--target {} takes one --binary", opts.target),
        (true, _) => bail!("--target {UNIVERSAL} takes two --binary (aarch64 and x86_64)"),
    }
}

/// `cargo auditable build --release -p rustyrian --target <triple>`.
fn build(root: &Path, target: &str) -> Result<PathBuf> {
    let has_auditable = Command::new("cargo")
        .args(["auditable", "--version"])
        .output()
        .is_ok_and(|o| o.status.success());
    if !has_auditable {
        run_status(Command::new("cargo").args(["install", "--locked", "cargo-auditable"]))?;
    }
    run_status(Command::new("cargo").current_dir(root).args([
        "auditable",
        "build",
        "--release",
        "-p",
        "rustyrian",
        "--target",
        target,
    ]))?;
    let target_dir =
        std::env::var_os("CARGO_TARGET_DIR").map_or_else(|| root.join("target"), |d| root.join(d));
    let exe = if target.contains("-windows-") {
        "rustyrian.exe"
    } else {
        "rustyrian"
    };
    Ok(target_dir.join(target).join("release").join(exe))
}

/// `stage/rustyrian/`: the binary, `data/`, `COPYING` (`COPYING.txt` on
/// Windows, as upstream) and `licenses/`.
pub fn stage_plain(inputs: &Inputs, binary: &Path, stage: &Path, windows: bool) -> Result<()> {
    let top = stage.join("rustyrian");
    std::fs::create_dir_all(&top)?;
    let exe = if windows {
        "rustyrian.exe"
    } else {
        "rustyrian"
    };
    copy(binary, &top.join(exe))?;
    let copying = if windows { "COPYING.txt" } else { "COPYING" };
    copy(&inputs.copying, &top.join(copying))?;
    copy_licenses(inputs, &top.join("licenses"))?;
    copy_data(&inputs.data, &top.join("data"))
}

/// `stage/Rustyrian.app`, modeled on upstream `make_macos.sh`. Returns the
/// bundle path.
pub fn stage_app(inputs: &Inputs, binary: &Path, stage: &Path, version: &str) -> Result<PathBuf> {
    let app = stage.join("Rustyrian.app");
    let contents = app.join("Contents");
    let resources = contents.join("Resources");
    std::fs::create_dir_all(contents.join("MacOS"))?;
    std::fs::create_dir_all(&resources)?;
    std::fs::write(contents.join("Info.plist"), info_plist(version))?;
    copy(binary, &contents.join("MacOS/rustyrian"))?;
    copy_data(&inputs.data, &resources.join("data"))?;
    copy(&inputs.copying, &resources.join("COPYING.txt"))?;
    copy_licenses(inputs, &resources.join("licenses"))?;
    let png = std::fs::read(&inputs.icon)
        .with_context(|| format!("reading {}", inputs.icon.display()))?;
    std::fs::write(resources.join("Rustyrian.icns"), icns(&png)?)?;
    Ok(app)
}

/// `Info.plist` of the app bundle (upstream `make_macos.sh`, renamed).
pub fn info_plist(version: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>      <string>rustyrian</string>
    <key>CFBundleIdentifier</key>      <string>io.github.modusmundi.Rustyrian</string>
    <key>CFBundleName</key>            <string>Rustyrian</string>
    <key>CFBundleDisplayName</key>     <string>Rustyrian</string>
    <key>CFBundlePackageType</key>     <string>APPL</string>
    <key>CFBundleShortVersionString</key> <string>{version}</string>
    <key>CFBundleIconFile</key>        <string>Rustyrian</string>
    <key>LSMinimumSystemVersion</key>  <string>10.13</string>
    <key>NSHighResolutionCapable</key> <true/>
</dict>
</plist>
"#
    )
}

/// An `.icns` holding one 128×128 PNG (`ic07`).
pub fn icns(png: &[u8]) -> Result<Vec<u8>> {
    const SIG: &[u8] = b"\x89PNG\r\n\x1a\n";
    if !png.starts_with(SIG) || png.len() < 24 || &png[12..16] != b"IHDR" {
        bail!("the icon is not a PNG");
    }
    let (w, h) = (
        u32::from_be_bytes([png[16], png[17], png[18], png[19]]),
        u32::from_be_bytes([png[20], png[21], png[22], png[23]]),
    );
    if (w, h) != (128, 128) {
        bail!("the icon is {w}x{h}, ic07 needs 128x128");
    }
    let entry = u32::try_from(8 + png.len())?;
    let total = 8 + entry;
    let mut v = Vec::with_capacity(total as usize);
    v.extend_from_slice(b"icns");
    v.extend_from_slice(&total.to_be_bytes());
    v.extend_from_slice(b"ic07");
    v.extend_from_slice(&entry.to_be_bytes());
    v.extend_from_slice(png);
    Ok(v)
}

fn copy(from: &Path, to: &Path) -> Result<()> {
    std::fs::copy(from, to)
        .with_context(|| format!("copying {} to {}", from.display(), to.display()))?;
    Ok(())
}

fn copy_licenses(inputs: &Inputs, dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    for name in ["Tyrian.txt", "SDL2.txt"] {
        copy(&inputs.licenses.join(name), &dir.join(name))?;
    }
    Ok(())
}

/// The data files (not `fetch-data`'s dot-file stamp).
fn copy_data(data: &Path, dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let mut n = 0;
    for e in std::fs::read_dir(data)
        .with_context(|| format!("{} (run `cargo xtask fetch-data`)", data.display()))?
    {
        let e = e?;
        let name = e.file_name();
        if name.to_string_lossy().starts_with('.') || !e.path().is_file() {
            continue;
        }
        copy(&e.path(), &dir.join(&name))?;
        n += 1;
    }
    if n == 0 {
        bail!("{} holds no data files", data.display());
    }
    Ok(())
}

/// Every file and directory under `stage`, sorted, as `(relative path with
/// '/', is_dir)`.
fn entries(stage: &Path) -> Result<Vec<(String, PathBuf, bool)>> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, PathBuf, bool)>) -> Result<()> {
        let mut items: Vec<_> = std::fs::read_dir(dir)?.collect::<Result<_, _>>()?;
        items.sort_by_key(std::fs::DirEntry::file_name);
        for e in items {
            let p = e.path();
            let rel = p.strip_prefix(base)?.to_string_lossy().replace('\\', "/");
            let is_dir = e.file_type()?.is_dir();
            out.push((rel, p.clone(), is_dir));
            if is_dir {
                walk(base, &p, out)?;
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(stage, stage, &mut out)?;
    Ok(out)
}

/// Executables: the binary in every layout.
fn is_executable(rel: &str) -> bool {
    matches!(rel, "rustyrian/rustyrian" | "rustyrian/rustyrian.exe")
        || rel.ends_with(".app/Contents/MacOS/rustyrian")
}

fn mode(rel: &str, is_dir: bool) -> u32 {
    if is_dir || is_executable(rel) {
        0o755
    } else {
        0o644
    }
}

pub fn write_tar_gz(stage: &Path, archive: &Path) -> Result<()> {
    if let Some(dir) = archive.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let gz = flate2::write::GzEncoder::new(File::create(archive)?, flate2::Compression::best());
    let mut tar = tar::Builder::new(gz);
    for (rel, path, is_dir) in entries(stage)? {
        let mut h = tar::Header::new_gnu();
        h.set_mode(mode(&rel, is_dir));
        h.set_mtime(0);
        h.set_uid(0);
        h.set_gid(0);
        if is_dir {
            h.set_entry_type(tar::EntryType::Directory);
            h.set_size(0);
            tar.append_data(&mut h, format!("{rel}/"), std::io::empty())?;
        } else {
            let bytes = std::fs::read(&path)?;
            h.set_entry_type(tar::EntryType::Regular);
            h.set_size(bytes.len() as u64);
            tar.append_data(&mut h, &rel, bytes.as_slice())?;
        }
    }
    tar.into_inner()?.finish()?.flush()?;
    Ok(())
}

pub fn write_zip(stage: &Path, archive: &Path) -> Result<()> {
    use zip::write::SimpleFileOptions;
    if let Some(dir) = archive.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut zip = zip::ZipWriter::new(File::create(archive)?);
    for (rel, path, is_dir) in entries(stage)? {
        // Unix entries on every host: the zip crate's default on Windows is
        // DOS, which drops the modes (and makes archives host-dependent).
        let opts = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .system(zip::System::Unix)
            .unix_permissions(mode(&rel, is_dir));
        if is_dir {
            zip.add_directory(format!("{rel}/"), opts)?;
        } else {
            zip.start_file(&rel, opts)?;
            zip.write_all(&std::fs::read(&path)?)?;
        }
    }
    zip.finish()?;
    Ok(())
}

/// `(path, unix mode)` of every entry of a package, in archive order.
pub fn list_archive(archive: &Path) -> Result<Vec<(String, u32)>> {
    let name = archive.to_string_lossy();
    if name.ends_with(".tar.gz") {
        let gz = flate2::read::GzDecoder::new(File::open(archive)?);
        let mut tar = tar::Archive::new(gz);
        tar.entries()?
            .map(|e| {
                let e = e?;
                Ok((e.path()?.to_string_lossy().into_owned(), e.header().mode()?))
            })
            .collect()
    } else {
        let mut zip = zip::ZipArchive::new(File::open(archive)?)?;
        (0..zip.len())
            .map(|i| {
                let f = zip.by_index(i)?;
                Ok((f.name()?.into_owned(), f.unix_mode().unwrap_or(0) & 0o777))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny 128×128 PNG header (enough for `icns`'s checks).
    fn png_128() -> Vec<u8> {
        let mut v = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        v.extend_from_slice(&128u32.to_be_bytes());
        v.extend_from_slice(&128u32.to_be_bytes());
        v.extend_from_slice(&[8, 6, 0, 0, 0]);
        v
    }

    /// Package inputs in a scratch directory, and a dummy binary.
    fn fixture(name: &str) -> Result<(PathBuf, Inputs, PathBuf)> {
        let tmp = std::env::temp_dir().join(format!("xtask-package-{}-{name}", std::process::id()));
        if tmp.exists() {
            std::fs::remove_dir_all(&tmp)?;
        }
        let root = tmp.join("root");
        std::fs::create_dir_all(root.join("data"))?;
        std::fs::create_dir_all(root.join("licenses"))?;
        std::fs::write(root.join("data/tyrian1.lvl"), b"lvl")?;
        std::fs::write(root.join("data/music.mus"), b"mus")?;
        std::fs::write(root.join("data/.sha256"), b"stamp")?;
        std::fs::write(root.join("COPYING"), b"GPL")?;
        std::fs::write(root.join("licenses/Tyrian.txt"), b"freeware")?;
        std::fs::write(root.join("licenses/SDL2.txt"), b"zlib")?;
        std::fs::write(root.join("icon.png"), png_128())?;
        let inputs = Inputs {
            data: root.join("data"),
            copying: root.join("COPYING"),
            licenses: root.join("licenses"),
            icon: root.join("icon.png"),
        };
        let bin = tmp.join("dummy-binary");
        std::fs::write(&bin, b"\x7fELF dummy")?;
        Ok((tmp, inputs, bin))
    }

    #[test]
    fn names() -> Result<()> {
        assert_eq!(
            archive_name("1.0.0", "x86_64-unknown-linux-gnu")?,
            "rustyrian-1.0.0-linux-x86_64.tar.gz"
        );
        assert_eq!(
            archive_name("1.0.0", "aarch64-pc-windows-msvc")?,
            "rustyrian-1.0.0-windows-aarch64.zip"
        );
        assert_eq!(
            archive_name("1.0.0", UNIVERSAL)?,
            "rustyrian-1.0.0-macos-universal.zip"
        );
        assert!(platform("wasm32-unknown-unknown").is_err());
        Ok(())
    }

    #[test]
    fn linux_tarball_layout() -> Result<()> {
        let (tmp, inputs, bin) = fixture("linux")?;
        let stage = tmp.join("stage");
        stage_plain(&inputs, &bin, &stage, false)?;
        let archive = tmp.join("out.tar.gz");
        write_tar_gz(&stage, &archive)?;
        assert_eq!(
            list_archive(&archive)?,
            vec![
                ("rustyrian/".to_owned(), 0o755),
                ("rustyrian/COPYING".to_owned(), 0o644),
                ("rustyrian/data/".to_owned(), 0o755),
                ("rustyrian/data/music.mus".to_owned(), 0o644),
                ("rustyrian/data/tyrian1.lvl".to_owned(), 0o644),
                ("rustyrian/licenses/".to_owned(), 0o755),
                ("rustyrian/licenses/SDL2.txt".to_owned(), 0o644),
                ("rustyrian/licenses/Tyrian.txt".to_owned(), 0o644),
                ("rustyrian/rustyrian".to_owned(), 0o755),
            ]
        );
        // Reproducible.
        let first = std::fs::read(&archive)?;
        write_tar_gz(&stage, &archive)?;
        assert_eq!(std::fs::read(&archive)?, first);
        std::fs::remove_dir_all(&tmp)?;
        Ok(())
    }

    #[test]
    fn windows_zip_layout() -> Result<()> {
        let (tmp, inputs, bin) = fixture("windows")?;
        let stage = tmp.join("stage");
        stage_plain(&inputs, &bin, &stage, true)?;
        let archive = tmp.join("out.zip");
        write_zip(&stage, &archive)?;
        let names: Vec<String> = list_archive(&archive)?.into_iter().map(|e| e.0).collect();
        assert_eq!(
            names,
            [
                "rustyrian/",
                "rustyrian/COPYING.txt",
                "rustyrian/data/",
                "rustyrian/data/music.mus",
                "rustyrian/data/tyrian1.lvl",
                "rustyrian/licenses/",
                "rustyrian/licenses/SDL2.txt",
                "rustyrian/licenses/Tyrian.txt",
                "rustyrian/rustyrian.exe",
            ]
        );
        std::fs::remove_dir_all(&tmp)?;
        Ok(())
    }

    #[test]
    fn macos_app_layout() -> Result<()> {
        let (tmp, inputs, bin) = fixture("macos")?;
        let stage = tmp.join("stage");
        let app = stage_app(&inputs, &bin, &stage, "1.2.3")?;
        assert!(
            std::fs::read_to_string(app.join("Contents/Info.plist"))?
                .contains("<key>CFBundleShortVersionString</key> <string>1.2.3</string>")
        );
        let icns = std::fs::read(app.join("Contents/Resources/Rustyrian.icns"))?;
        assert_eq!(&icns[..4], b"icns");
        assert_eq!(&icns[8..12], b"ic07");
        assert_eq!(
            u32::from_be_bytes(icns[4..8].try_into()?) as usize,
            icns.len()
        );
        let archive = tmp.join("out.zip");
        write_zip(&stage, &archive)?;
        assert_eq!(
            list_archive(&archive)?,
            vec![
                ("Rustyrian.app/".to_owned(), 0o755),
                ("Rustyrian.app/Contents/".to_owned(), 0o755),
                ("Rustyrian.app/Contents/Info.plist".to_owned(), 0o644),
                ("Rustyrian.app/Contents/MacOS/".to_owned(), 0o755),
                ("Rustyrian.app/Contents/MacOS/rustyrian".to_owned(), 0o755),
                ("Rustyrian.app/Contents/Resources/".to_owned(), 0o755),
                (
                    "Rustyrian.app/Contents/Resources/COPYING.txt".to_owned(),
                    0o644
                ),
                (
                    "Rustyrian.app/Contents/Resources/Rustyrian.icns".to_owned(),
                    0o644
                ),
                ("Rustyrian.app/Contents/Resources/data/".to_owned(), 0o755),
                (
                    "Rustyrian.app/Contents/Resources/data/music.mus".to_owned(),
                    0o644
                ),
                (
                    "Rustyrian.app/Contents/Resources/data/tyrian1.lvl".to_owned(),
                    0o644
                ),
                (
                    "Rustyrian.app/Contents/Resources/licenses/".to_owned(),
                    0o755
                ),
                (
                    "Rustyrian.app/Contents/Resources/licenses/SDL2.txt".to_owned(),
                    0o644
                ),
                (
                    "Rustyrian.app/Contents/Resources/licenses/Tyrian.txt".to_owned(),
                    0o644
                ),
            ]
        );
        std::fs::remove_dir_all(&tmp)?;
        Ok(())
    }

    #[test]
    fn icon_must_be_a_128_png() {
        assert!(icns(b"not a png").is_err());
        let mut small = png_128();
        small[16..20].copy_from_slice(&64u32.to_be_bytes());
        assert!(icns(&small).is_err());
    }
}
