//! Scenario runs: `tests/scenarios/<name>.ots` plus its parameters in
//! `tests/scenarios/scenarios.toml`; `cargo xtask ref-trace`.

use crate::tools::run_status;
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub until_ms: Option<u32>,
    pub frames: Option<u64>,
    #[serde(default)]
    pub args: Vec<String>,
    pub clock: Option<String>,
    pub net: Option<String>,
}

pub fn load(root: &Path, name: &str) -> Result<Scenario> {
    let path = root.join("tests/scenarios/scenarios.toml");
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let all: BTreeMap<String, Scenario> = toml::from_str(&text)?;
    let sc = all
        .get(name)
        .cloned()
        .with_context(|| format!("scenario `{name}` is not in scenarios.toml"))?;
    if sc.until_ms.is_none() && sc.frames.is_none() {
        bail!("scenario `{name}` needs until_ms or frames");
    }
    if !root.join(format!("tests/scenarios/{name}.ots")).is_file() {
        bail!("tests/scenarios/{name}.ots does not exist");
    }
    Ok(sc)
}

/// Builds a runner binary of the oracle crate and returns its path.
pub fn build_runner(root: &Path, bin: &str, release: bool) -> Result<PathBuf> {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(root)
        .args(["build", "--quiet", "-p", "rustyrian-oracle", "--bin", bin]);
    if release {
        cmd.arg("--release");
    }
    run_status(&mut cmd)?;
    let exe = format!("{bin}{}", std::env::consts::EXE_SUFFIX);
    Ok(root
        .join("target")
        .join(if release { "release" } else { "debug" })
        .join(exe))
}

/// Outputs of one engine run.
pub struct RunOut {
    pub trace: PathBuf,
    pub log: PathBuf,
    pub userdir: PathBuf,
}

/// Runs one engine on a scenario, writing into `dir/<tag>.*`.
pub fn run(
    root: &Path,
    exe: &Path,
    name: &str,
    sc: &Scenario,
    dir: &Path,
    tag: &str,
    extra: &[String],
) -> Result<RunOut> {
    std::fs::create_dir_all(dir)?;
    let userdir = dir.join(format!("{tag}-user"));
    if userdir.exists() {
        std::fs::remove_dir_all(&userdir)?;
    }
    let seed = root.join(format!("tests/scenarios/{name}.userdir"));
    if seed.is_dir() {
        copy_tree(&seed, &userdir)?;
    }
    let out = RunOut {
        trace: dir.join(format!("{tag}.jsonl")),
        log: dir.join(format!("{tag}.log.jsonl")),
        userdir,
    };
    let mut cmd = Command::new(exe);
    cmd.current_dir(root)
        .arg("--data")
        .arg(root.join("data"))
        .arg("--userdir")
        .arg(&out.userdir)
        .arg("--input")
        .arg(root.join(format!("tests/scenarios/{name}.ots")))
        .arg("--trace-out")
        .arg(&out.trace)
        .arg("--log-out")
        .arg(&out.log);
    if let Some(ms) = sc.until_ms {
        cmd.args(["--until-ms", &ms.to_string()]);
    }
    if let Some(n) = sc.frames {
        cmd.args(["--frames", &n.to_string()]);
    }
    if let Some(c) = &sc.clock {
        cmd.args(["--clock", c]);
    }
    if let Some(n) = &sc.net {
        cmd.args(["--net", n]);
    }
    cmd.args(extra);
    cmd.arg("--").args(&sc.args);
    let status = cmd
        .status()
        .with_context(|| format!("running {}", exe.display()))?;
    if !status.success() {
        bail!("{} on {name} exited with {status}", exe.display());
    }
    Ok(out)
}

fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let dst = to.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_tree(&e.path(), &dst)?;
        } else {
            std::fs::copy(e.path(), dst)?;
        }
    }
    Ok(())
}

#[derive(Deserialize)]
struct FixtureList {
    #[serde(default)]
    fixture: Vec<Fixture>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    scenario: String,
    frame: u64,
}

/// `cargo xtask fixtures`: the C oracle's mirrored game state at each
/// `[[fixture]]` of `porting/fixtures.toml`, written to
/// `target/fixtures/<scenario>@<frame>.postcard` (read by
/// `rustyrian_oracle::diff::fixture`).
pub fn fixtures(root: &Path) -> Result<()> {
    let text = std::fs::read_to_string(root.join("porting/fixtures.toml"))?;
    let list: FixtureList = toml::from_str(&text)?;
    if list.fixture.is_empty() {
        eprintln!("fixtures: porting/fixtures.toml lists no fixtures");
        return Ok(());
    }
    let exe = build_runner(root, "ot-ref-run", false)?;
    let dir = root.join("target/fixtures");
    std::fs::create_dir_all(&dir)?;
    for f in &list.fixture {
        let mut sc = load(root, &f.scenario)?;
        sc.frames = Some(f.frame + 1);
        let out = dir.join(format!("{}@{}.postcard", f.scenario, f.frame));
        let extra = vec![
            "--snapshot-at".to_owned(),
            f.frame.to_string(),
            "--snapshot-out".to_owned(),
            out.display().to_string(),
        ];
        let work = root
            .join("target/fixtures/runs")
            .join(format!("{}@{}", f.scenario, f.frame));
        run(root, &exe, &f.scenario, &sc, &work, "c", &extra)?;
        if !out.is_file() {
            bail!(
                "{}: the run ended before frame {} (no snapshot written)",
                f.scenario,
                f.frame
            );
        }
        eprintln!("fixtures: wrote {}", out.display());
    }
    Ok(())
}

/// `cargo xtask ref-trace <scenario>`: the C oracle's trace for a scenario.
pub fn ref_trace(root: &Path, name: &str, out: Option<&Path>, release: bool) -> Result<()> {
    let sc = load(root, name)?;
    let exe = build_runner(root, "ot-ref-run", release)?;
    let dir = out.map_or_else(|| root.join("target/parity").join(name), Path::to_path_buf);
    let r = run(root, &exe, name, &sc, &dir, "c", &[])?;
    eprintln!("wrote {}", r.trace.display());
    Ok(())
}
