//! `cargo xtask parity <scenario> [--bless] [--rust-runner PATH]`: runs the C
//! oracle (`ot-ref-run`) and the Rust runner (`rs-run`) on a scenario with
//! identically seeded user directories, then compares, run by run, the traces
//! and the user directories byte for byte.
//!
//! At the first differing frame N it reruns both engines up to N−1 and N with
//! `--snapshot-at` and `--log-out` and writes, under
//! `target/parity/<scenario>/`:
//! - `c_<N>.png`, `rust_<N>.png` and `diff_<N>.png` (differing pixels in
//!   magenta over a dimmed C frame);
//! - `state_diff_<N>.txt`: the field paths of the mirrored game state
//!   (`rustyrian_core::game::Snapshot`) that differ at N−1 and at N;
//! - `first_call_<N>.txt`: the first differing `PlatformCall`;
//!
//! and exits 1. `--bless` copies the C traces to `tests/golden/`.
//!
//! C keeps its files in `<userdir>/opentyrian`, Rust in `<userdir>/rustyrian`
//! (rule R14); the user-directory comparison maps one onto the other.

use crate::scenario::{self, RunSpec};
use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

/// `cargo xtask parity` options.
pub struct Options {
    pub scenario: String,
    pub bless: bool,
    /// Rust runner binary (default: `rs-run` built from the oracle crate).
    pub rust_runner: Option<PathBuf>,
    /// Test only: run the Rust side's first run on this script instead of the
    /// scenario's (proves that a divergence is caught and diagnosed).
    pub rust_script: Option<PathBuf>,
    pub release: bool,
}

/// The two sides of a comparison.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    C,
    Rust,
}

impl Side {
    fn tag(self) -> &'static str {
        match self {
            Side::C => "c",
            Side::Rust => "rust",
        }
    }

    /// The engine's directory under the user directory: C uses `opentyrian`,
    /// Rust `rustyrian`; either side may be a C runner (`--rust-runner
    /// ot-ref-run`), so the other name is the fallback.
    fn engine_dir(self, userdir: &Path) -> Option<PathBuf> {
        let order = match self {
            Side::C => ["opentyrian", "rustyrian"],
            Side::Rust => ["rustyrian", "opentyrian"],
        };
        order.iter().map(|n| userdir.join(n)).find(|p| p.is_dir())
    }
}

/// Runs `cargo xtask parity`. `Ok(false)` means the engines differ (the
/// diagnosis has been written).
pub fn run(root: &Path, opts: &Options) -> Result<bool> {
    let name = opts.scenario.as_str();
    let sc = scenario::load(root, name)?;
    if !sc.peers.is_empty() {
        bail!("`{name}` is a netplay scenario; run it with `cargo xtask crossplay`");
    }
    let c_exe = scenario::build_runner(root, "ot-ref-run", opts.release)?;
    let dir = root.join("target/parity").join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).with_context(|| format!("clearing {}", dir.display()))?;
    }
    std::fs::create_dir_all(&dir)?;

    let c_runs = sc.runs(root, name);
    let c = run_side(root, &c_exe, name, &dir, Side::C, &c_runs)?;
    if opts.bless {
        bless(root, name, &c)?;
    }

    let rust_exe = match &opts.rust_runner {
        Some(p) => p.clone(),
        None => scenario::build_runner(root, "rs-run", opts.release)?,
    };
    let mut rust_runs = c_runs.clone();
    if let Some(script) = &opts.rust_script {
        rust_runs[0].input = script.clone();
    }
    let rust = run_side(root, &rust_exe, name, &dir, Side::Rust, &rust_runs)?;

    for (k, (c_out, r_out)) in c.iter().zip(&rust).enumerate() {
        let run_no = k + 1;
        let ct = read_trace(&c_out.trace)?;
        let rt = read_trace(&r_out.trace)?;
        if let Some(d) = first_difference(&ct, &rt) {
            eprintln!(
                "parity: {name} run {run_no}: traces differ at frame {} ({})",
                d.frame, d.detail
            );
            let ctx = Diagnosis {
                root,
                name,
                dir: &dir,
                run_no,
                frame: d.frame,
                exes: [
                    (Side::C, &c_exe, &c_runs),
                    (Side::Rust, &rust_exe, &rust_runs),
                ],
            };
            ctx.diagnose(&d)?;
            return Ok(false);
        }
        let tree = diff_trees(
            Side::C.engine_dir(&c_out.userdir).as_deref(),
            Side::Rust.engine_dir(&r_out.userdir).as_deref(),
        )?;
        if !tree.is_empty() {
            let report = dir.join(format!("userdir_diff_{run_no}.txt"));
            std::fs::write(&report, tree.join("\n") + "\n")?;
            eprintln!(
                "parity: {name} run {run_no}: user directories differ:\n  {}\n  (written to {})",
                tree.join("\n  "),
                report.display()
            );
            return Ok(false);
        }
        eprintln!(
            "parity: {name} run {run_no}: {} frames identical, user directories identical",
            ct.iter().filter(|l| l.get("frame").is_some()).count()
        );
    }
    println!("parity: {name} OK");
    Ok(true)
}

/// Outputs of run `k` of one side.
struct SideRun {
    trace: PathBuf,
    /// Copy of the user directory as it was after this run.
    userdir: PathBuf,
}

/// Runs every run of a scenario on one engine, in a fresh user directory.
/// A non-zero exit is not an error here: its trace records it.
fn run_side(
    root: &Path,
    exe: &Path,
    name: &str,
    dir: &Path,
    side: Side,
    runs: &[RunSpec],
) -> Result<Vec<SideRun>> {
    let tag = side.tag();
    let userdir = scenario::fresh_userdir(root, name, dir, tag)?;
    let mut outs = Vec::new();
    for (i, spec) in runs.iter().enumerate() {
        let k = i + 1;
        let trace = dir.join(scenario::out_name(tag, k, ".jsonl"));
        let log = dir.join(scenario::out_name(tag, k, ".log.jsonl"));
        let mut cmd = scenario::command(root, exe, spec, &userdir, &trace, Some(&log), &[]);
        eprintln!("parity: {name} run {k}: {tag} ({})", exe.display());
        let status = cmd
            .status()
            .with_context(|| format!("running {}", exe.display()))?;
        if !trace.is_file() {
            bail!("{} wrote no trace ({status})", exe.display());
        }
        let after = if runs.len() > 1 {
            let snap = dir.join(format!("{tag}-user@{k}"));
            copy_tree(&userdir, &snap)?;
            snap
        } else {
            userdir.clone()
        };
        outs.push(SideRun {
            trace,
            userdir: after,
        });
    }
    Ok(outs)
}

fn bless(root: &Path, name: &str, c: &[SideRun]) -> Result<()> {
    let golden = root.join("tests/golden");
    std::fs::create_dir_all(&golden)?;
    for (i, r) in c.iter().enumerate() {
        let dst = golden.join(scenario::out_name(name, i + 1, ".jsonl"));
        std::fs::copy(&r.trace, &dst)
            .with_context(|| format!("copying {} to {}", r.trace.display(), dst.display()))?;
        eprintln!("parity: blessed {}", dst.display());
    }
    Ok(())
}

pub fn read_trace(path: &Path) -> Result<Vec<Value>> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    text.lines()
        .enumerate()
        .map(|(i, l)| {
            serde_json::from_str(l).with_context(|| format!("{}:{}", path.display(), i + 1))
        })
        .collect()
}

/// Where two traces first differ.
#[derive(Debug, PartialEq)]
pub struct Divergence {
    /// The first differing frame N (or the frame count, when only the final
    /// lines differ).
    pub frame: u64,
    /// The two differing lines' fields, for the report.
    pub detail: String,
}

fn without_engine(v: &Value) -> Value {
    let mut v = v.clone();
    if let Some(m) = v.as_object_mut() {
        m.remove("engine");
    }
    v
}

fn frame_of(v: Option<&Value>) -> Option<u64> {
    v?.get("frame")?.as_u64()
}

/// The first difference between two traces; the header's `engine` field is
/// ignored.
pub fn first_difference(c: &[Value], rust: &[Value]) -> Option<Divergence> {
    if c.first().map(without_engine) != rust.first().map(without_engine) {
        return Some(Divergence {
            frame: 0,
            detail: format!(
                "headers differ: c {} / rust {}",
                show(c.first()),
                show(rust.first())
            ),
        });
    }
    for i in 1..c.len().max(rust.len()) {
        let (a, b) = (c.get(i), rust.get(i));
        if a == b {
            continue;
        }
        let frame = frame_of(a)
            .or_else(|| frame_of(b))
            .unwrap_or((i - 1) as u64);
        let detail = match (a, b) {
            (Some(Value::Object(x)), Some(Value::Object(y))) => {
                let mut keys: Vec<&String> = x.keys().chain(y.keys()).collect();
                keys.sort();
                keys.dedup();
                keys.into_iter()
                    .filter(|k| x.get(*k) != y.get(*k))
                    .map(|k| format!("{k}: c {} / rust {}", show(x.get(k)), show(y.get(k))))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
            _ => format!("c {} / rust {}", show(a), show(b)),
        };
        return Some(Divergence { frame, detail });
    }
    None
}

fn show(v: Option<&Value>) -> String {
    let s = v.map_or_else(|| "(missing)".to_owned(), Value::to_string);
    if s.len() > 120 {
        let cut = (0..=117)
            .rev()
            .find(|&i| s.is_char_boundary(i))
            .unwrap_or(0);
        format!("{}...", &s[..cut])
    } else {
        s
    }
}

/// Rerun context for the first differing frame of run `run_no`.
struct Diagnosis<'a> {
    root: &'a Path,
    name: &'a str,
    dir: &'a Path,
    run_no: usize,
    frame: u64,
    exes: [(Side, &'a PathBuf, &'a Vec<RunSpec>); 2],
}

/// Files of one side's rerun at frame `n`.
struct Rerun {
    snapshot: PathBuf,
    log: PathBuf,
    frames: PathBuf,
}

impl Diagnosis<'_> {
    fn diagnose(&self, d: &Divergence) -> Result<()> {
        let n = self.frame;
        let rerun_dir = self.dir.join("rerun");
        std::fs::create_dir_all(&rerun_dir)?;

        // Both sides, frames N and N-1, all at once.
        let mut children: Vec<(String, Child)> = Vec::new();
        let mut at_n = Vec::new();
        let mut at_prev = Vec::new();
        for (side, exe, runs) in self.exes {
            let tag = side.tag();
            let base = self.replay_before(side, exe, runs, &rerun_dir)?;
            let spec = &runs[self.run_no - 1];
            let (cmd, files) = self.snapshot_cmd(exe, spec, &base, &rerun_dir, tag, n, true)?;
            children.push((format!("{tag} @{n}"), spawn(cmd, exe)?));
            at_n.push(files);
            if n > 0 {
                let (cmd, files) =
                    self.snapshot_cmd(exe, spec, &base, &rerun_dir, tag, n - 1, false)?;
                children.push((format!("{tag} @{}", n - 1), spawn(cmd, exe)?));
                at_prev.push(files);
            }
        }
        for (what, mut child) in children {
            let status = child.wait()?;
            eprintln!("parity: rerun {what}: {status}");
        }

        let mut summary = vec![format!(
            "{} run {}: first differing frame {n}: {}",
            self.name, self.run_no, d.detail
        )];

        // Frame images.
        let c_png = at_n[0].frames.join(format!("frame_{n}.png"));
        let r_png = at_n[1].frames.join(format!("frame_{n}.png"));
        let c_out = self.dir.join(format!("c_{n}.png"));
        let r_out = self.dir.join(format!("rust_{n}.png"));
        let diff_out = self.dir.join(format!("diff_{n}.png"));
        if c_png.is_file() && r_png.is_file() {
            std::fs::copy(&c_png, &c_out)?;
            std::fs::copy(&r_png, &r_out)?;
            let differing = diff_png(&c_png, &r_png, &diff_out)?;
            summary.push(format!(
                "frame {n}: {differing} differing pixels; {} {} {}",
                c_out.display(),
                r_out.display(),
                diff_out.display()
            ));
        } else {
            summary.push(format!(
                "frame {n} was not presented by both engines (no images)"
            ));
        }

        // State.
        let mut state = String::new();
        let mut first_fields = Vec::new();
        let mut pairs = vec![(n, &at_n[0].snapshot, &at_n[1].snapshot)];
        if n > 0 {
            pairs.insert(0, (n - 1, &at_prev[0].snapshot, &at_prev[1].snapshot));
        }
        for (frame, c_snap, r_snap) in pairs {
            writeln!(state, "== frame {frame}")?;
            match (load_snapshot(c_snap), load_snapshot(r_snap)) {
                (Ok(c), Ok(r)) => {
                    let (expected, diffs): (Vec<String>, Vec<String>) = diff_values(&c, &r)
                        .into_iter()
                        .partition(|d| is_engine_field(field_of(d)));
                    let grouped = group_diffs(&diffs);
                    writeln!(state, "{} differing fields", diffs.len())?;
                    for f in grouped.iter().take(MAX_FIELDS) {
                        writeln!(state, "{f}")?;
                    }
                    if grouped.len() > MAX_FIELDS {
                        writeln!(state, "... and {} more", grouped.len() - MAX_FIELDS)?;
                    }
                    if !expected.is_empty() {
                        writeln!(
                            state,
                            "per-engine by design (rule R14: each engine's own user directory):"
                        )?;
                        for f in &expected {
                            writeln!(state, "  {f}")?;
                        }
                    }
                    if first_fields.is_empty() {
                        first_fields = grouped.iter().take(8).cloned().collect();
                    }
                }
                (c, r) => {
                    for (tag, res) in [("c", c), ("rust", r)] {
                        if let Err(e) = res {
                            writeln!(state, "{tag}: no snapshot: {e:#}")?;
                        }
                    }
                }
            }
        }
        let state_out = self.dir.join(format!("state_diff_{n}.txt"));
        std::fs::write(&state_out, &state)?;
        summary.push(format!("state diff: {}", state_out.display()));
        for f in first_fields {
            summary.push(format!("  {f}"));
        }

        // Platform calls.
        let call = first_call_difference(&at_n[0].log, &at_n[1].log)?;
        let call_out = self.dir.join(format!("first_call_{n}.txt"));
        std::fs::write(&call_out, &call)?;
        summary.push(format!("first differing call: {}", call_out.display()));
        summary.push(format!("  {}", call.lines().next().unwrap_or_default()));

        eprintln!("parity: {}", summary.join("\nparity: "));
        Ok(())
    }

    /// A user directory in the state it had before run `run_no` (seed, then
    /// the earlier runs replayed).
    fn replay_before(
        &self,
        side: Side,
        exe: &Path,
        runs: &[RunSpec],
        rerun_dir: &Path,
    ) -> Result<PathBuf> {
        let tag = side.tag();
        let base =
            scenario::fresh_userdir(self.root, self.name, rerun_dir, &format!("{tag}-base"))?;
        for (i, spec) in runs.iter().enumerate().take(self.run_no - 1) {
            let trace = rerun_dir.join(scenario::out_name(
                &format!("{tag}-replay"),
                i + 1,
                ".jsonl",
            ));
            let mut cmd = scenario::command(self.root, exe, spec, &base, &trace, None, &[]);
            cmd.status()
                .with_context(|| format!("replaying run {} on {}", i + 1, exe.display()))?;
        }
        Ok(base)
    }

    /// The rerun of one side up to `frame`, writing its snapshot, and (for the
    /// differing frame itself) its frame image and platform-call log.
    #[allow(clippy::too_many_arguments)]
    fn snapshot_cmd(
        &self,
        exe: &Path,
        spec: &RunSpec,
        base: &Path,
        rerun_dir: &Path,
        tag: &str,
        frame: u64,
        full: bool,
    ) -> Result<(Command, Rerun)> {
        let stem = format!("{tag}@{frame}");
        let userdir = rerun_dir.join(format!("{stem}-user"));
        if userdir.exists() {
            std::fs::remove_dir_all(&userdir)?;
        }
        copy_tree(base, &userdir)?;
        let files = Rerun {
            snapshot: rerun_dir.join(format!("{stem}.postcard")),
            log: rerun_dir.join(format!("{stem}.log.jsonl")),
            frames: rerun_dir.join(format!("{stem}-frames")),
        };
        let mut spec = spec.clone();
        spec.frames = Some(frame + 1);
        let mut extra = vec![
            "--snapshot-at".to_owned(),
            frame.to_string(),
            "--snapshot-out".to_owned(),
            files.snapshot.display().to_string(),
        ];
        if full {
            extra.extend([
                "--dump-frames".to_owned(),
                files.frames.display().to_string(),
                "--dump-every".to_owned(),
                frame.max(1).to_string(),
            ]);
        }
        let trace = rerun_dir.join(format!("{stem}.jsonl"));
        let log = full.then_some(files.log.as_path());
        let cmd = scenario::command(self.root, exe, &spec, &userdir, &trace, log, &extra);
        Ok((cmd, files))
    }
}

const MAX_FIELDS: usize = 200;

/// Fields that differ between the engines by design: the user directory
/// path ends in `opentyrian` for C and `rustyrian` for Rust (rule R14), and
/// reruns use different directories.
const ENGINE_FIELDS: [&str; 2] = ["file.user_dir_path", "file.user_dir_path_len"];

/// The field path of a `diff_values` line.
fn field_of(line: &str) -> &str {
    line.split_once(": c ").map_or(line, |(p, _)| p)
}

fn is_engine_field(path: &str) -> bool {
    ENGINE_FIELDS.iter().any(|f| {
        path.strip_prefix(f)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('['))
    })
}

/// Collapses runs of more than three differing elements of one array into a
/// single line (frame buffers, sample buffers).
pub fn group_diffs(diffs: &[String]) -> Vec<String> {
    let base = |line: &str| -> String {
        let path = field_of(line);
        match path.strip_suffix(']').and_then(|p| p.rsplit_once('[')) {
            Some((b, i)) if i.bytes().all(|c| c.is_ascii_digit()) => b.to_owned(),
            _ => path.to_owned(),
        }
    };
    let mut out = Vec::new();
    let mut i = 0;
    while i < diffs.len() {
        let b = base(&diffs[i]);
        let n = diffs[i..].iter().take_while(|d| base(d) == b).count();
        if n > 3 {
            out.push(format!("{b}[..]: {n} elements differ; first {}", diffs[i]));
        } else {
            out.extend(diffs[i..i + n].iter().cloned());
        }
        i += n;
    }
    out
}

fn spawn(mut cmd: Command, exe: &Path) -> Result<Child> {
    cmd.spawn()
        .with_context(|| format!("spawning {}", exe.display()))
}

/// A `--snapshot-out` file as JSON (through `rustyrian_core::game::Snapshot`,
/// the format `rustyrian_oracle::diff::fixture` reads).
pub fn load_snapshot(path: &Path) -> Result<Value> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let snap: rustyrian_core::game::Snapshot =
        postcard::from_bytes(&bytes).with_context(|| format!("decoding {}", path.display()))?;
    Ok(serde_json::to_value(&snap)?)
}

/// Every leaf path where two JSON values differ, as `path: c <v> / rust <v>`.
pub fn diff_values(c: &Value, rust: &Value) -> Vec<String> {
    let mut out = Vec::new();
    diff_at(String::new(), c, rust, &mut out);
    out
}

fn diff_at(path: String, c: &Value, rust: &Value, out: &mut Vec<String>) {
    match (c, rust) {
        (Value::Object(a), Value::Object(b)) => {
            let mut keys: Vec<&String> = a.keys().chain(b.keys()).collect();
            keys.sort();
            keys.dedup();
            for k in keys {
                let p = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                match (a.get(k), b.get(k)) {
                    (Some(x), Some(y)) => diff_at(p, x, y, out),
                    (x, y) => out.push(format!("{p}: c {} / rust {}", show(x), show(y))),
                }
            }
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                diff_at(format!("{path}[{i}]"), x, y, out);
            }
        }
        _ if c != rust => out.push(format!(
            "{path}: c {} / rust {}",
            show(Some(c)),
            show(Some(rust))
        )),
        _ => {}
    }
}

fn read_png(path: &Path) -> Result<(u32, u32, Vec<u8>)> {
    let file = std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let decoder = png::Decoder::new(std::io::BufReader::new(file));
    let mut reader = decoder.read_info()?;
    let mut buf = vec![0; reader.output_buffer_size().context("PNG too large")?];
    let info = reader.next_frame(&mut buf)?;
    if info.color_type != png::ColorType::Rgb || info.bit_depth != png::BitDepth::Eight {
        bail!("{}: expected 8-bit RGB", path.display());
    }
    buf.truncate(info.buffer_size());
    Ok((info.width, info.height, buf))
}

/// Writes `out`: the C frame dimmed to a quarter, with every pixel that
/// differs in the Rust frame in magenta. Returns the number of differing
/// pixels.
pub fn diff_png(c: &Path, rust: &Path, out: &Path) -> Result<usize> {
    let (w, h, a) = read_png(c)?;
    let (w2, h2, b) = read_png(rust)?;
    if (w, h) != (w2, h2) {
        bail!("frame sizes differ: {w}x{h} / {w2}x{h2}");
    }
    let mut img = Vec::with_capacity(a.len());
    let mut differing = 0;
    for (pa, pb) in a.as_chunks::<3>().0.iter().zip(b.as_chunks::<3>().0) {
        if pa == pb {
            img.extend(pa.iter().map(|v| v / 4));
        } else {
            differing += 1;
            img.extend_from_slice(&[255, 0, 255]);
        }
    }
    let file = std::fs::File::create(out)?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(&img)?;
    Ok(differing)
}

/// The first differing `PlatformCall` of two `--log-out` files, with the
/// frame it belongs to (presents before it) and the calls just before.
pub fn first_call_difference(c: &Path, rust: &Path) -> Result<String> {
    let read = |p: &Path| -> Result<Vec<String>> {
        Ok(std::fs::read_to_string(p)
            .with_context(|| format!("reading {}", p.display()))?
            .lines()
            .map(str::to_owned)
            .collect())
    };
    let (a, b) = (read(c)?, read(rust)?);
    let Some(i) = (0..a.len().max(b.len())).find(|&i| a.get(i) != b.get(i)) else {
        return Ok("no differing platform call\n".to_owned());
    };
    let frame = a[..i]
        .iter()
        .filter(|l| l.contains("\"SDL_RenderPresent\""))
        .count();
    let none = "(none)".to_owned();
    let mut s = format!(
        "call #{i} (frame {frame}): c {} / rust {}\n",
        a.get(i).unwrap_or(&none),
        b.get(i).unwrap_or(&none)
    );
    writeln!(s, "preceding calls:")?;
    for l in &a[i.saturating_sub(5)..i] {
        writeln!(s, "  {l}")?;
    }
    Ok(s)
}

/// Byte-for-byte comparison of two engine directories (either may be
/// absent). Returns one line per difference.
pub fn diff_trees(c: Option<&Path>, rust: Option<&Path>) -> Result<Vec<String>> {
    let (a, b) = (files(c)?, files(rust)?);
    let mut out = Vec::new();
    let mut names: Vec<&String> = a.keys().chain(b.keys()).collect();
    names.sort();
    names.dedup();
    for n in names {
        match (a.get(n), b.get(n)) {
            (Some(x), Some(y)) if x == y => {}
            (Some(x), Some(y)) => {
                let at = x
                    .iter()
                    .zip(y)
                    .position(|(p, q)| p != q)
                    .unwrap_or(x.len().min(y.len()));
                out.push(format!(
                    "{n}: differs at byte {at} (c {} bytes, rust {} bytes)",
                    x.len(),
                    y.len()
                ));
            }
            (Some(_), None) => out.push(format!("{n}: only written by C")),
            (None, Some(_)) => out.push(format!("{n}: only written by Rust")),
            (None, None) => {}
        }
    }
    Ok(out)
}

fn files(dir: Option<&Path>) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut map = BTreeMap::new();
    if let Some(dir) = dir {
        collect(dir, dir, &mut map)?;
    }
    Ok(map)
}

fn collect(base: &Path, dir: &Path, map: &mut BTreeMap<String, Vec<u8>>) -> Result<()> {
    for e in std::fs::read_dir(dir)? {
        let e = e?;
        let p = e.path();
        if e.file_type()?.is_dir() {
            collect(base, &p, map)?;
        } else {
            let rel = p.strip_prefix(base)?.to_string_lossy().replace('\\', "/");
            map.insert(rel, std::fs::read(&p)?);
        }
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn trace(frames: &[Value], end: Value) -> Vec<Value> {
        let mut v = vec![json!({"trace": 1, "engine": "c", "scenario": "s"})];
        v.extend(frames.iter().cloned());
        v.push(end);
        v
    }

    fn frame(n: u64, idx: &str) -> Value {
        json!({"frame": n, "ms": n * 28, "idx": idx, "calls": "00"})
    }

    #[test]
    fn identical_traces_ignore_the_engine() {
        let c = trace(
            &[frame(0, "a"), frame(1, "b")],
            json!({"end": "limit", "code": 0}),
        );
        let mut r = c.clone();
        r[0]["engine"] = json!("rust");
        assert_eq!(first_difference(&c, &r), None);
    }

    #[test]
    fn first_differing_frame_and_fields() {
        let end = json!({"end": "limit", "code": 0});
        let c = trace(&[frame(0, "a"), frame(1, "b"), frame(2, "c")], end.clone());
        let r = trace(&[frame(0, "a"), frame(1, "x"), frame(2, "y")], end);
        let d = first_difference(&c, &r).expect("differs");
        assert_eq!(d.frame, 1);
        assert_eq!(d.detail, r#"idx: c "b" / rust "x""#);
    }

    #[test]
    fn early_exit_is_the_frame_after_the_last_common_one() {
        let c = trace(
            &[frame(0, "a"), frame(1, "b")],
            json!({"end": "limit", "code": 0}),
        );
        let r = trace(&[frame(0, "a")], json!({"end": "exit", "code": 1}));
        assert_eq!(first_difference(&c, &r).expect("differs").frame, 1);
        let c2 = trace(&[frame(0, "a")], json!({"end": "limit", "code": 0}));
        assert_eq!(first_difference(&c2, &r).expect("differs").frame, 1);
    }

    #[test]
    fn field_paths() {
        let c = json!({"player": {"player": [{"cash": 10000}, {"cash": 0}]}, "x": [1, 2]});
        let r = json!({"player": {"player": [{"cash": 9750}, {"cash": 0}]}, "x": [1, 2, 3]});
        assert_eq!(
            diff_values(&c, &r),
            vec![
                "player.player[0].cash: c 10000 / rust 9750".to_owned(),
                "x: c [1,2] / rust [1,2,3]".to_owned(),
            ]
        );
    }

    #[test]
    fn grouped_arrays_and_engine_fields() {
        let mut diffs: Vec<String> = (0..5)
            .map(|i| format!("video.screens[0].pixels[{i}]: c 1 / rust 2"))
            .collect();
        diffs.push("varz.cur_loc: c 3 / rust 4".to_owned());
        assert_eq!(
            group_diffs(&diffs),
            vec![
                "video.screens[0].pixels[..]: 5 elements differ; first video.screens[0].pixels[0]: c 1 / rust 2"
                    .to_owned(),
                "varz.cur_loc: c 3 / rust 4".to_owned(),
            ]
        );
        assert!(is_engine_field("file.user_dir_path[3]"));
        assert!(is_engine_field("file.user_dir_path_len"));
        assert!(!is_engine_field("file.user_dir_paths"));
    }

    #[test]
    fn trees_and_images() -> Result<()> {
        let tmp = std::env::temp_dir().join(format!("xtask-parity-{}", std::process::id()));
        let (a, b) = (tmp.join("a/opentyrian"), tmp.join("b/rustyrian"));
        std::fs::create_dir_all(&a)?;
        std::fs::create_dir_all(&b)?;
        std::fs::write(a.join("tyrian.cfg"), [1, 2, 3])?;
        std::fs::write(b.join("tyrian.cfg"), [1, 9, 3])?;
        std::fs::write(a.join("tyrian.sav"), [0])?;
        assert_eq!(
            Side::Rust.engine_dir(&tmp.join("b")).as_deref(),
            Some(b.as_path())
        );
        assert_eq!(
            diff_trees(Some(&a), Some(&b))?,
            vec![
                "tyrian.cfg: differs at byte 1 (c 3 bytes, rust 3 bytes)".to_owned(),
                "tyrian.sav: only written by C".to_owned(),
            ]
        );
        assert!(diff_trees(Some(&a), Some(&a))?.is_empty());

        let write = |p: &Path, px: &[u8]| -> Result<()> {
            let mut enc = png::Encoder::new(std::fs::File::create(p)?, 2, 1);
            enc.set_color(png::ColorType::Rgb);
            enc.set_depth(png::BitDepth::Eight);
            enc.write_header()?.write_image_data(px)?;
            Ok(())
        };
        write(&tmp.join("c.png"), &[8, 8, 8, 1, 2, 3])?;
        write(&tmp.join("r.png"), &[8, 8, 8, 1, 2, 4])?;
        let n = diff_png(&tmp.join("c.png"), &tmp.join("r.png"), &tmp.join("d.png"))?;
        assert_eq!(n, 1);
        assert_eq!(read_png(&tmp.join("d.png"))?.2, vec![2, 2, 2, 255, 0, 255]);

        std::fs::write(
            tmp.join("c.log"),
            "{\"f\":\"A\"}\n{\"f\":\"SDL_RenderPresent\"}\n{\"f\":\"B\"}\n",
        )?;
        std::fs::write(
            tmp.join("r.log"),
            "{\"f\":\"A\"}\n{\"f\":\"SDL_RenderPresent\"}\n{\"f\":\"C\"}\n",
        )?;
        let call = first_call_difference(&tmp.join("c.log"), &tmp.join("r.log"))?;
        assert!(call.starts_with(r#"call #2 (frame 1): c {"f":"B"} / rust {"f":"C"}"#));
        std::fs::remove_dir_all(&tmp)?;
        Ok(())
    }
}
