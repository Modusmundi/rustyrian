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
    /// `--sim-digest` on every run.
    #[serde(default)]
    pub sim_digest: bool,
    /// Further runner processes, in order, on the same user directory (for
    /// example a restart that must see the settings of the first run).
    #[serde(default)]
    pub then: Vec<Step>,
    /// The two netplay peers, run together by `cargo xtask crossplay`. Such a
    /// scenario has no `<name>.ots` of its own.
    #[serde(default)]
    pub peers: Vec<Peer>,
}

/// A further run of a scenario (`then = [...]`).
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Step {
    /// Script, relative to `tests/scenarios/`.
    pub input: String,
    pub until_ms: Option<u32>,
    pub frames: Option<u64>,
    #[serde(default)]
    pub args: Vec<String>,
}

/// One netplay peer (`peers = [...]`): its script and game arguments.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Peer {
    /// Script, relative to `tests/scenarios/`.
    pub input: String,
    #[serde(default)]
    pub args: Vec<String>,
}

/// One runner process: script, limits, runner options and game arguments.
#[derive(Debug, Clone)]
pub struct RunSpec {
    pub input: PathBuf,
    pub until_ms: Option<u32>,
    pub frames: Option<u64>,
    pub clock: Option<String>,
    pub net: Option<String>,
    pub sim_digest: bool,
    pub args: Vec<String>,
}

fn scenarios_dir(root: &Path) -> PathBuf {
    root.join("tests/scenarios")
}

impl Scenario {
    fn spec(&self, input: PathBuf, args: &[String]) -> RunSpec {
        RunSpec {
            input,
            until_ms: self.until_ms,
            frames: self.frames,
            clock: self.clock.clone(),
            net: self.net.clone(),
            sim_digest: self.sim_digest,
            args: args.to_vec(),
        }
    }

    /// The runner processes of a scenario, in order: its own script, then
    /// each `then` step. They share one user directory.
    pub fn runs(&self, root: &Path, name: &str) -> Vec<RunSpec> {
        let dir = scenarios_dir(root);
        let mut v = vec![self.spec(dir.join(format!("{name}.ots")), &self.args)];
        for s in &self.then {
            let mut r = self.spec(dir.join(&s.input), &s.args);
            r.until_ms = s.until_ms;
            r.frames = s.frames;
            v.push(r);
        }
        v
    }

    /// The runs of the netplay peers, player 1 first.
    pub fn peer_runs(&self, root: &Path) -> Vec<RunSpec> {
        let dir = scenarios_dir(root);
        self.peers
            .iter()
            .map(|p| self.spec(dir.join(&p.input), &p.args))
            .collect()
    }
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
    let dir = scenarios_dir(root);
    if sc.peers.is_empty() {
        if !dir.join(format!("{name}.ots")).is_file() {
            bail!("tests/scenarios/{name}.ots does not exist");
        }
    } else if sc.peers.len() != 2 || !sc.then.is_empty() {
        bail!("scenario `{name}`: a netplay scenario has exactly two peers and no `then`");
    }
    for s in &sc.then {
        if s.until_ms.is_none() && s.frames.is_none() {
            bail!(
                "scenario `{name}`: run {} needs until_ms or frames",
                s.input
            );
        }
    }
    let inputs = sc.then.iter().map(|s| &s.input);
    for input in inputs.chain(sc.peers.iter().map(|p| &p.input)) {
        if !dir.join(input).is_file() {
            bail!("scenario `{name}`: tests/scenarios/{input} does not exist");
        }
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
    // `root.join` keeps an absolute CARGO_TARGET_DIR as is.
    let target =
        std::env::var_os("CARGO_TARGET_DIR").map_or_else(|| root.join("target"), |d| root.join(d));
    Ok(target
        .join(if release { "release" } else { "debug" })
        .join(exe))
}

/// Outputs of one engine run.
pub struct RunOut {
    pub trace: PathBuf,
    pub log: PathBuf,
    pub userdir: PathBuf,
}

/// File name of run `k` (1-based) of a scenario for engine `tag`:
/// `<tag><ext>` for the first run, `<tag>.<k><ext>` for later ones.
pub fn out_name(tag: &str, k: usize, ext: &str) -> String {
    if k <= 1 {
        format!("{tag}{ext}")
    } else {
        format!("{tag}.{k}{ext}")
    }
}

/// A fresh user directory `dir/<tag>-user`, seeded from
/// `tests/scenarios/<name>.userdir` when that exists (copied as is, so a seed
/// for both engines holds `opentyrian/` and `rustyrian/`).
pub fn fresh_userdir(root: &Path, name: &str, dir: &Path, tag: &str) -> Result<PathBuf> {
    let userdir = dir.join(format!("{tag}-user"));
    if userdir.exists() {
        std::fs::remove_dir_all(&userdir)?;
    }
    std::fs::create_dir_all(&userdir)?;
    let seed = root.join(format!("tests/scenarios/{name}.userdir"));
    if seed.is_dir() {
        copy_tree(&seed, &userdir)?;
    }
    Ok(userdir)
}

/// The runner command line for one run. `extra` goes before `--`.
pub fn command(
    root: &Path,
    exe: &Path,
    spec: &RunSpec,
    userdir: &Path,
    trace: &Path,
    log: Option<&Path>,
    extra: &[String],
) -> Command {
    let mut cmd = Command::new(exe);
    cmd.current_dir(root)
        .arg("--data")
        .arg(root.join("data"))
        .arg("--userdir")
        .arg(userdir)
        .arg("--input")
        .arg(&spec.input)
        .arg("--trace-out")
        .arg(trace);
    if let Some(log) = log {
        cmd.arg("--log-out").arg(log);
    }
    if let Some(ms) = spec.until_ms {
        cmd.args(["--until-ms", &ms.to_string()]);
    }
    if let Some(n) = spec.frames {
        cmd.args(["--frames", &n.to_string()]);
    }
    if let Some(c) = &spec.clock {
        cmd.args(["--clock", c]);
    }
    if let Some(n) = &spec.net {
        cmd.args(["--net", n]);
    }
    if spec.sim_digest {
        cmd.arg("--sim-digest");
    }
    cmd.args(extra);
    cmd.arg("--").args(&spec.args);
    cmd
}

fn invoke(mut cmd: Command, exe: &Path, input: &Path) -> Result<()> {
    let status = cmd
        .status()
        .with_context(|| format!("running {}", exe.display()))?;
    if !status.success() {
        bail!(
            "{} on {} exited with {status}",
            exe.display(),
            input.display()
        );
    }
    Ok(())
}

/// Runs one engine on the first run of a scenario, writing into `dir/<tag>.*`.
pub fn run(
    root: &Path,
    exe: &Path,
    name: &str,
    sc: &Scenario,
    dir: &Path,
    tag: &str,
    extra: &[String],
) -> Result<RunOut> {
    let userdir = fresh_userdir(root, name, dir, tag)?;
    let spec = &sc.runs(root, name)[0];
    let out = RunOut {
        trace: dir.join(out_name(tag, 1, ".jsonl")),
        log: dir.join(out_name(tag, 1, ".log.jsonl")),
        userdir,
    };
    let cmd = command(
        root,
        exe,
        spec,
        &out.userdir,
        &out.trace,
        Some(&out.log),
        extra,
    );
    invoke(cmd, exe, &spec.input)?;
    Ok(out)
}

/// Runs one engine on every run of a scenario (one user directory), writing
/// run `k` into `dir/<out_name(tag, k, ..)>`.
pub fn run_all(
    root: &Path,
    exe: &Path,
    name: &str,
    sc: &Scenario,
    dir: &Path,
    tag: &str,
) -> Result<Vec<RunOut>> {
    let userdir = fresh_userdir(root, name, dir, tag)?;
    let mut outs = Vec::new();
    for (i, spec) in sc.runs(root, name).iter().enumerate() {
        let out = RunOut {
            trace: dir.join(out_name(tag, i + 1, ".jsonl")),
            log: dir.join(out_name(tag, i + 1, ".log.jsonl")),
            userdir: userdir.clone(),
        };
        let cmd = command(
            root,
            exe,
            spec,
            &out.userdir,
            &out.trace,
            Some(&out.log),
            &[],
        );
        invoke(cmd, exe, &spec.input)?;
        outs.push(out);
    }
    Ok(outs)
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

/// `cargo xtask ref-trace <scenario>`: the C oracle's traces for a scenario
/// (one per run: `c.jsonl`, then `c.2.jsonl`, ... for `then` steps).
pub fn ref_trace(root: &Path, name: &str, out: Option<&Path>, release: bool) -> Result<()> {
    let sc = load(root, name)?;
    if !sc.peers.is_empty() {
        bail!("`{name}` is a netplay scenario; run it with `cargo xtask crossplay`");
    }
    let exe = build_runner(root, "ot-ref-run", release)?;
    let dir = out.map_or_else(|| root.join("target/parity").join(name), Path::to_path_buf);
    for r in run_all(root, &exe, name, &sc, &dir, "c")? {
        eprintln!("wrote {}", r.trace.display());
    }
    Ok(())
}
