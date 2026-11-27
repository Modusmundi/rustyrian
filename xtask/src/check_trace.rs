//! `cargo xtask check-trace`: the traceability gate. Writes `porting/STATUS.md`.

use crate::inventory::{self, Function, module_of};
use crate::manifest::{self, Entry, milestone_of};
use anyhow::{Context, Result, bail};
use heck::ToSnakeCase;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

/// Trait methods that are not a 1:1 SDL/SDL_net function: libc `time` and
/// `localtime`, the invocation of the function pointer returned by
/// `SDL_LogGetOutputFunction`, and `exit` (intercepted, not recorded).
pub const EXTRA_PLATFORM_METHODS: &[&str] = &["time", "localtime", "log_output", "exit"];

/// Rule: `SDL_GetWindowSize` -> `get_window_size`, `SDLNet_UDP_Send` -> `net_udp_send`.
pub fn platform_method_name(c: &str) -> String {
    if let Some(rest) = c.strip_prefix("SDLNet_") {
        format!("net_{}", rest.to_snake_case())
    } else if let Some(rest) = c.strip_prefix("SDL_") {
        rest.to_snake_case()
    } else {
        c.to_snake_case()
    }
}

fn doc_aliases_in(path: &Path, out: &mut BTreeSet<String>) -> Result<()> {
    let text = std::fs::read_to_string(path)?;
    let file = syn::parse_file(&text).with_context(|| format!("parsing {}", path.display()))?;
    struct V<'a>(&'a mut BTreeSet<String>);
    impl V<'_> {
        fn attrs(&mut self, attrs: &[syn::Attribute]) {
            for a in attrs {
                if !a.path().is_ident("doc") {
                    continue;
                }
                let _ = a.parse_nested_meta(|m| {
                    if m.path.is_ident("alias") {
                        let s: syn::LitStr = m.value()?.parse()?;
                        self.0.insert(s.value());
                    }
                    Ok(())
                });
            }
        }
    }
    impl<'ast> syn::visit::Visit<'ast> for V<'_> {
        fn visit_item_fn(&mut self, i: &'ast syn::ItemFn) {
            // A function named exactly like the C function needs no alias
            // (rustc rejects an alias equal to the item name).
            self.0.insert(i.sig.ident.to_string());
            self.attrs(&i.attrs);
            syn::visit::visit_item_fn(self, i);
        }
        fn visit_impl_item_fn(&mut self, i: &'ast syn::ImplItemFn) {
            self.0.insert(i.sig.ident.to_string());
            self.attrs(&i.attrs);
            syn::visit::visit_impl_item_fn(self, i);
        }
    }
    syn::visit::Visit::visit_file(&mut V(out), &file);
    Ok(())
}

fn module_aliases(root: &Path, module: &str) -> Result<BTreeSet<String>> {
    let mut out = BTreeSet::new();
    let src = root.join("crates/core/src");
    let file = src.join(format!("{module}.rs"));
    if file.is_file() {
        doc_aliases_in(&file, &mut out)?;
    }
    let dir = src.join(module);
    if dir.is_dir() {
        let mut stack = vec![dir];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d)? {
                let p = e?.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    doc_aliases_in(&p, &mut out)?;
                }
            }
        }
    }
    Ok(out)
}

fn platform_trait_methods(root: &Path) -> Result<BTreeSet<String>> {
    let p = root.join("crates/core/src/platform.rs");
    let text = std::fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()))?;
    let file = syn::parse_file(&text)?;
    for item in &file.items {
        if let syn::Item::Trait(t) = item
            && t.ident == "Platform"
        {
            return Ok(t
                .items
                .iter()
                .filter_map(|i| match i {
                    syn::TraitItem::Fn(f) => Some(f.sig.ident.to_string()),
                    _ => None,
                })
                .collect());
        }
    }
    bail!("no `trait Platform` in {}", p.display())
}

fn nextest_names(root: &Path) -> Result<BTreeSet<String>> {
    let out = Command::new("cargo")
        .current_dir(root)
        .args(["nextest", "list", "--workspace", "--message-format", "json"])
        .output()
        .context("running cargo nextest list")?;
    if !out.status.success() {
        bail!(
            "cargo nextest list failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout)?;
    let mut names = BTreeSet::new();
    if let Some(suites) = v.get("rust-suites").and_then(|s| s.as_object()) {
        for suite in suites.values() {
            let binary = suite
                .get("binary-name")
                .and_then(|b| b.as_str())
                .unwrap_or_default();
            if let Some(cases) = suite.get("testcases").and_then(|c| c.as_object()) {
                // `diff:<test binary>::<test name>`, e.g. `diff:mtrand::mt_rand_r_variants_from_any_state`.
                names.extend(cases.keys().map(|k| format!("{binary}::{k}")));
            }
        }
    }
    Ok(names)
}

fn word_in(haystack: &str, word: &str) -> bool {
    haystack.match_indices(word).any(|(i, _)| {
        let before = haystack[..i].chars().next_back();
        let after = haystack[i + word.len()..].chars().next();
        let is_ident = |c: Option<char>| c.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
        !is_ident(before) && !is_ident(after)
    })
}

pub fn run(root: &Path) -> Result<()> {
    let inv = inventory::load(root)?;
    let entries = manifest::load(root)?;
    let by_key: BTreeMap<&str, &Entry> = entries.iter().map(|e| (e.c.as_str(), e)).collect();
    let mut errors: Vec<String> = Vec::new();

    // 1. Every inventory function has a non-stale entry.
    for f in &inv.functions {
        match by_key.get(f.key().as_str()) {
            None => errors.push(format!("{}: no entry in functions.toml", f.key())),
            Some(e) if e.status == "stale" => {
                errors.push(format!("{}: stale (upstream source changed)", f.key()));
            }
            Some(e) if e.c_sha != f.c_sha => {
                errors.push(format!("{}: c_sha out of date; run manifest-sync", f.key()));
            }
            Some(_) => {}
        }
    }

    // 2. Ported/verified entries have a Rust function carrying the C name alias.
    let mut alias_cache: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let funcs: BTreeMap<String, &Function> = inv.functions.iter().map(|f| (f.key(), f)).collect();
    for e in &entries {
        if !matches!(e.status.as_str(), "ported" | "verified") {
            continue;
        }
        let Some(f) = funcs.get(&e.c) else { continue };
        let module = f.module();
        if !alias_cache.contains_key(&module) {
            alias_cache.insert(module.clone(), module_aliases(root, &module)?);
        }
        if !alias_cache[&module].contains(&f.name) {
            errors.push(format!(
                "{}: status {} but no fn with #[doc(alias = \"{}\")] in crates/core/src/{module}.rs",
                e.c, e.status, f.name
            ));
        }
        if e.status == "verified" && e.verify.is_empty() {
            errors.push(format!("{}: verified without `verify` tests", e.c));
        }
        if !e.rust.starts_with(&format!("rustyrian_core::{module}::")) {
            errors.push(format!(
                "{}: rust path `{}` is not in module `{module}` (rule R1)",
                e.c, e.rust
            ));
        }
    }
    for e in &entries {
        for w in &e.waived_lines {
            if w.reason.trim().is_empty() {
                errors.push(format!("{}: waived line {} has no reason", e.c, w.line));
            }
        }
    }
    for e in &entries {
        if e.status == "replaced" && (e.notes.trim().is_empty() || e.verify.is_empty()) {
            errors.push(format!("{}: replaced requires notes and verify", e.c));
        }
    }

    // 3. Listed tests exist.
    let needs_tests = entries
        .iter()
        .filter(|e| matches!(e.status.as_str(), "verified" | "replaced"))
        .any(|e| e.verify.iter().any(|v| v.starts_with("diff:")));
    let tests = if needs_tests {
        nextest_names(root)?
    } else {
        BTreeSet::new()
    };
    for e in entries
        .iter()
        .filter(|e| matches!(e.status.as_str(), "verified" | "replaced"))
    {
        for v in &e.verify {
            if let Some(t) = v.strip_prefix("diff:") {
                if !tests.contains(t) {
                    errors.push(format!("{}: test `{t}` not found by nextest", e.c));
                }
            } else if let Some(s) = v.strip_prefix("trace:") {
                if !root.join(format!("tests/scenarios/{s}.ots")).is_file() {
                    errors.push(format!(
                        "{}: scenario `{s}` has no tests/scenarios/{s}.ots",
                        e.c
                    ));
                }
            } else {
                errors.push(format!(
                    "{}: verify item `{v}` must start with diff: or trace:",
                    e.c
                ));
            }
        }
    }

    // 4. Coverage gate when coverage data is present.
    let cov_path = root.join("porting/coverage.json");
    if cov_path.is_file() {
        let cov: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&cov_path)?)?;
        for e in entries.iter().filter(|e| e.status == "verified") {
            let Some(c) = cov.get("functions").and_then(|m| m.get(&e.c)) else {
                errors.push(format!("{}: missing from coverage.json", e.c));
                continue;
            };
            let total = c.get("lines").and_then(|v| v.as_u64()).unwrap_or(0);
            let uncovered: BTreeSet<u64> = c
                .get("uncovered")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_u64()).collect())
                .unwrap_or_default();
            let waived: BTreeSet<u64> = e.waived_lines.iter().map(|w| u64::from(w.line)).collect();
            let effective_total = total.saturating_sub(waived.len() as u64);
            let missed = uncovered.difference(&waived).count() as u64;
            if effective_total > 0 {
                let pct = 100.0 * (effective_total - missed.min(effective_total)) as f64
                    / effective_total as f64;
                if pct < 95.0 {
                    errors.push(format!("{}: C line coverage {pct:.1}% < 95%", e.c));
                }
            }
        }
    }

    // 5. Mirrors reference every global of a file with verified functions.
    let mut verified_files: BTreeSet<&str> = BTreeSet::new();
    for e in entries.iter().filter(|e| e.status == "verified") {
        if let Some(f) = funcs.get(&e.c) {
            verified_files.insert(&f.file);
        }
    }
    for file in &verified_files {
        let module = module_of(file);
        let globals: Vec<_> = inv.globals.iter().filter(|g| g.file == *file).collect();
        if globals.is_empty() {
            continue;
        }
        let mirror_path = format!("crates/oracle/src/mirror/{module}.rs");
        let mirror = match std::fs::read_to_string(root.join(&mirror_path)) {
            Ok(text) => text,
            Err(e) => {
                errors.push(format!("{file}: cannot read {mirror_path}: {e}"));
                continue;
            }
        };
        let stem = file.trim_end_matches(".c").trim_end_matches(".h");
        for g in globals {
            let getter = match &g.function {
                Some(func) => format!("ot_sv_{stem}_{func}_{}", g.name),
                None => format!("ot_sv_{stem}_{}", g.name),
            };
            if !word_in(&mirror, &getter) && (g.function.is_some() || !word_in(&mirror, &g.name)) {
                errors.push(format!(
                    "{file}: global `{}` not referenced in crates/oracle/src/mirror/{module}.rs",
                    g.name
                ));
            }
        }
    }

    // 6. Platform trait method set.
    let expected: BTreeSet<String> = inv
        .platform_calls
        .iter()
        .map(|c| platform_method_name(c))
        .chain(EXTRA_PLATFORM_METHODS.iter().map(|s| (*s).to_owned()))
        .collect();
    match platform_trait_methods(root) {
        Ok(actual) => {
            for m in expected.difference(&actual) {
                errors.push(format!("Platform trait lacks `{m}`"));
            }
            for m in actual.difference(&expected) {
                errors.push(format!("Platform trait has unexpected method `{m}`"));
            }
        }
        Err(e) => errors.push(format!("{e:#}")),
    }

    write_status(root, &inv, &by_key)?;

    if errors.is_empty() {
        eprintln!("check-trace: OK ({} functions)", inv.functions.len());
        Ok(())
    } else {
        for e in &errors {
            eprintln!("check-trace: {e}");
        }
        bail!("check-trace: {} problem(s)", errors.len())
    }
}

fn write_status(
    root: &Path,
    inv: &inventory::Inventory,
    by_key: &BTreeMap<&str, &Entry>,
) -> Result<()> {
    let funcs = &inv.functions;
    let cols = ["todo", "ported", "verified", "replaced", "stale"];
    let mut per_file: BTreeMap<(String, String), BTreeMap<&str, usize>> = BTreeMap::new();
    let mut totals: BTreeMap<&str, usize> = BTreeMap::new();
    for f in funcs {
        let status = by_key
            .get(f.key().as_str())
            .map_or("todo", |e| e.status.as_str());
        let ms = milestone_of(&f.module()).unwrap_or("-").to_owned();
        *per_file
            .entry((ms, f.file.clone()))
            .or_default()
            .entry(status)
            .or_default() += 1;
        *totals.entry(status).or_default() += 1;
    }
    let total = funcs.len().max(1);
    let done =
        totals.get("verified").copied().unwrap_or(0) + totals.get("replaced").copied().unwrap_or(0);
    let mut s =
        String::from("# Port status\n\nGenerated by `cargo xtask check-trace`; do not edit.\n\n");
    let n_c = funcs.iter().filter(|f| !f.inline_header).count();
    writeln!(
        s,
        "Inventory (upstream `{}`, clang AST): {n_c} `.c` functions ({} static), {} header inlines, {} mutable globals ({} function-local), {} intercepted platform calls.\n",
        &inv.upstream[..7],
        funcs
            .iter()
            .filter(|f| !f.inline_header && f.is_static)
            .count(),
        funcs.len() - n_c,
        inv.globals.len(),
        inv.globals.iter().filter(|g| g.function.is_some()).count(),
        inv.platform_calls.len(),
    )?;
    writeln!(
        s,
        "Overall: {done}/{} functions verified or replaced ({:.1}%); {} ported, {} todo.\n",
        funcs.len(),
        100.0 * done as f64 / total as f64,
        totals.get("ported").copied().unwrap_or(0),
        totals.get("todo").copied().unwrap_or(0)
    )?;
    s.push_str("|Milestone|File|todo|ported|verified|replaced|stale|Done|\n|---|---|---|---|---|---|---|---|\n");
    for ((ms, file), counts) in &per_file {
        let n: usize = counts.values().sum();
        let d = counts.get("verified").copied().unwrap_or(0)
            + counts.get("replaced").copied().unwrap_or(0);
        write!(s, "|{ms}|{file}|")?;
        for c in cols {
            write!(s, "{}|", counts.get(c).copied().unwrap_or(0))?;
        }
        writeln!(s, "{:.0}%|", 100.0 * d as f64 / n.max(1) as f64)?;
    }
    std::fs::write(root.join("porting/STATUS.md"), s)?;
    Ok(())
}
