//! `porting/functions.toml`: per-function port status (`manifest-sync`, `next`).

use crate::inventory::{self, Function, Inventory, module_of};
use anyhow::{Context, Result, bail};
use heck::ToSnakeCase;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use toml_edit::{Array, DocumentMut, Item, Table, value};

pub const STATUSES: &[&str] = &["todo", "ported", "verified", "replaced", "stale", "gone"];

/// Milestone module sets (plan step 7). Header inlines belong to the
/// milestone of their header's module.
pub const MILESTONES: &[(&str, &[&str])] = &[
    (
        "M1",
        &[
            "mtrand",
            "memreader",
            "memwriter",
            "file",
            "logging",
            "arg_parse",
            "params",
            "config_file",
            "helptext",
            "episodes",
            "lvllib",
            "lvlmast",
            "musmast",
            "pcxmast",
            "sndmast",
            "nortvars",
            "palette",
            "vga_palette",
            "picload",
            "pcxload",
            "animlib",
            "sprite",
            "vga256d",
            "font",
            "fonthand",
            "backgrnd",
            "starlib",
            "shots",
            "player",
            "config",
            "swar",
        ],
    ),
    ("M2", &["opl", "lds_play", "loudness", "nortsong"]),
    (
        "M3",
        &[
            "video",
            "video_scale",
            "video_scale_hqnx",
            "keyboard",
            "mouse",
            "joystick",
            "network",
        ],
    ),
    (
        "M4",
        &[
            "varz",
            "mainint",
            "tyrian2",
            "game_menu",
            "menus",
            "demo",
            "destruct",
            "editship",
            "xmas",
            "jukebox",
            "opentyr",
        ],
    ),
];

pub fn milestone_of(module: &str) -> Option<&'static str> {
    MILESTONES
        .iter()
        .find(|(_, mods)| mods.contains(&module))
        .map(|(m, _)| *m)
}

#[derive(Deserialize, Debug, Clone)]
pub struct Entry {
    pub c: String,
    pub c_sha: String,
    pub rust: String,
    pub status: String,
    #[serde(default)]
    pub verify: Vec<String>,
    #[serde(default)]
    pub waived_lines: Vec<Waiver>,
    #[serde(default)]
    pub notes: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Waiver {
    pub line: u32,
    pub reason: String,
}

#[derive(Deserialize)]
struct Manifest {
    #[serde(default, rename = "fn")]
    entries: Vec<Entry>,
}

pub fn path(root: &Path) -> PathBuf {
    root.join("porting/functions.toml")
}

pub fn load(root: &Path) -> Result<Vec<Entry>> {
    let p = path(root);
    let text = std::fs::read_to_string(&p).unwrap_or_default();
    let m: Manifest = toml::from_str(&text).with_context(|| format!("parsing {}", p.display()))?;
    for e in &m.entries {
        if !STATUSES.contains(&e.status.as_str()) {
            bail!("{}: unknown status `{}`", e.c, e.status);
        }
    }
    Ok(m.entries)
}

/// Rule R2 Rust path for a C function.
pub fn rust_path(f: &Function) -> String {
    format!("rustyrian_core::{}::{}", f.module(), f.name.to_snake_case())
}

pub fn sync(root: &Path) -> Result<()> {
    let inv = inventory::load(root)?;
    let p = path(root);
    let text = std::fs::read_to_string(&p).unwrap_or_else(|_| {
        "# Port status per upstream C function. Maintained by `cargo xtask manifest-sync`;\n# see porting/RULES.md for the meaning of each field.\n".to_owned()
    });
    let mut doc: DocumentMut = text.parse()?;
    if doc.get("fn").is_none() {
        doc.insert("fn", Item::ArrayOfTables(toml_edit::ArrayOfTables::new()));
    }
    let aot = doc
        .get_mut("fn")
        .and_then(Item::as_array_of_tables_mut)
        .context("`fn` is not an array of tables")?;

    let by_key: BTreeMap<String, &Function> = inv.functions.iter().map(|f| (f.key(), f)).collect();
    let mut seen = BTreeSet::new();
    let (mut stale, mut gone, mut added) = (0, 0, 0);
    for t in aot.iter_mut() {
        let key = t.get("c").and_then(Item::as_str).unwrap_or("").to_owned();
        seen.insert(key.clone());
        match by_key.get(&key) {
            Some(f) => {
                let old = t.get("c_sha").and_then(Item::as_str).unwrap_or("");
                if old != f.c_sha {
                    t["c_sha"] = value(f.c_sha.clone());
                    if t.get("status").and_then(Item::as_str) != Some("todo") {
                        t["status"] = value("stale");
                        stale += 1;
                    }
                }
                if t.get("status").and_then(Item::as_str) == Some("gone") {
                    t["status"] = value("todo");
                }
            }
            None => {
                if t.get("status").and_then(Item::as_str) != Some("gone") {
                    t["status"] = value("gone");
                    gone += 1;
                }
            }
        }
    }
    for f in &inv.functions {
        if seen.contains(&f.key()) {
            continue;
        }
        let mut t = Table::new();
        t["c"] = value(f.key());
        t["c_sha"] = value(f.c_sha.clone());
        t["rust"] = value(rust_path(f));
        t["status"] = value("todo");
        t["verify"] = value(Array::new());
        t["notes"] = value("");
        aot.push(t);
        added += 1;
    }
    std::fs::write(&p, doc.to_string())?;
    eprintln!("functions.toml: {added} added, {stale} stale, {gone} gone");
    Ok(())
}

/// Tarjan SCCs over the call graph; returned in callee-first order.
pub fn sccs(inv: &Inventory) -> Vec<Vec<String>> {
    let keys: Vec<String> = inv.functions.iter().map(Function::key).collect();
    let index_of: BTreeMap<&str, usize> = keys
        .iter()
        .enumerate()
        .map(|(i, k)| (k.as_str(), i))
        .collect();
    let edges: Vec<Vec<usize>> = inv
        .functions
        .iter()
        .map(|f| {
            f.calls
                .iter()
                .filter_map(|c| index_of.get(c.as_str()).copied())
                .collect()
        })
        .collect();

    struct T<'a> {
        edges: &'a [Vec<usize>],
        index: Vec<Option<usize>>,
        low: Vec<usize>,
        on_stack: Vec<bool>,
        stack: Vec<usize>,
        next: usize,
        out: Vec<Vec<usize>>,
    }
    fn strong(t: &mut T, v: usize) {
        t.index[v] = Some(t.next);
        t.low[v] = t.next;
        t.next += 1;
        t.stack.push(v);
        t.on_stack[v] = true;
        for i in 0..t.edges[v].len() {
            let w = t.edges[v][i];
            match t.index[w] {
                None => {
                    strong(t, w);
                    t.low[v] = t.low[v].min(t.low[w]);
                }
                Some(iw) if t.on_stack[w] => t.low[v] = t.low[v].min(iw),
                Some(_) => {}
            }
        }
        if Some(t.low[v]) == t.index[v] {
            let mut comp = Vec::new();
            while let Some(w) = t.stack.pop() {
                t.on_stack[w] = false;
                comp.push(w);
                if w == v {
                    break;
                }
            }
            comp.sort_unstable();
            t.out.push(comp);
        }
    }
    let n = keys.len();
    let mut t = T {
        edges: &edges,
        index: vec![None; n],
        low: vec![0; n],
        on_stack: vec![false; n],
        stack: Vec::new(),
        next: 0,
        out: Vec::new(),
    };
    for v in 0..n {
        if t.index[v].is_none() {
            strong(&mut t, v);
        }
    }
    t.out
        .into_iter()
        .map(|c| c.into_iter().map(|i| keys[i].clone()).collect())
        .collect()
}

pub fn next(root: &Path, n: usize) -> Result<()> {
    let inv = inventory::load(root)?;
    let entries = load(root)?;
    let status: BTreeMap<&str, &str> = entries
        .iter()
        .map(|e| (e.c.as_str(), e.status.as_str()))
        .collect();
    let file_of: BTreeMap<String, &Function> = inv.functions.iter().map(|f| (f.key(), f)).collect();
    let is_open = |k: &str| matches!(status.get(k).copied(), Some("todo" | "stale") | None);
    let mut ready: Vec<(&str, String)> = Vec::new();
    for comp in sccs(&inv) {
        if !comp.iter().any(|k| is_open(k)) {
            continue;
        }
        let members: BTreeSet<&str> = comp.iter().map(String::as_str).collect();
        let blocked = comp
            .iter()
            .flat_map(|k| file_of[k].calls.iter())
            .any(|c| !members.contains(c.as_str()) && is_open(c));
        if blocked {
            continue;
        }
        let ms = comp
            .iter()
            .map(|k| milestone_of(&module_of(&file_of[k].file)).unwrap_or("M9"))
            .max()
            .unwrap_or("M9");
        let names: Vec<String> = comp
            .iter()
            .map(|k| {
                let f = file_of[k];
                format!("{k} (lines {}-{})", f.begin_line, f.end_line)
            })
            .collect();
        ready.push((ms, names.join("  +  ")));
    }
    // Earliest milestone first; callee-first order within a milestone.
    ready.sort_by_key(|(ms, _)| *ms);
    let mut printed = 0;
    for (ms, line) in ready.iter().take(n) {
        println!("[{ms}] {line}");
        printed += 1;
    }
    if printed == 0 {
        println!("nothing ready: every function is ported or blocked");
    }
    Ok(())
}
