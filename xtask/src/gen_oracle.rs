//! `cargo xtask gen-oracle`: inventory the pinned upstream C sources through the
//! clang AST and generate every oracle wrapper file from it.

use crate::ast::{self, FnFact, TuFacts};
use crate::inventory::{Function, Global, Inventory};
use crate::tools::{UPSTREAM_COMMIT, run_capture, sha256_hex, upstream_src, write_if_changed};
use anyhow::{Context, Result, bail};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// SDL functions that are pure memory helpers; linked from the real SDL2.
pub const REAL_SDL: &[&str] = &[
    "SDL_CreateRGBSurface",
    "SDL_FreeSurface",
    "SDL_FillRect",
    "SDL_UpperBlit",
    "SDL_AllocFormat",
    "SDL_FreeFormat",
    "SDL_MapRGB",
    "SDL_GetPixelFormatName",
    "SDL_strlcpy",
    "SDL_free",
];

/// libc functions redirected to the shim.
pub const LIBC_INTERCEPTED: &[&str] = &[
    "rand",
    "time",
    "localtime",
    "malloc",
    "exit",
    "atexit",
    "sinf",
    "cosf",
    "sin",
    "pow",
    "powf",
    "atanf",
];

/// Intercepted functions whose shim is written in C (`crates/oracle/shim/*.c`)
/// rather than Rust: `exit` must `longjmp`, `malloc` is `calloc`, and Rust
/// cannot define C-variadic functions.
const C_IMPLEMENTED_LIBC: &[&str] = &["exit", "malloc"];

/// libm functions whose results are exact or correctly rounded on every
/// platform (rule R6); all other libm functions must be intercepted.
const EXACT_LIBM: &[&str] = &[
    "roundf", "round", "floorf", "floor", "fabsf", "fabs", "sqrtf", "sqrt", "ceilf", "ceil",
    "truncf", "trunc", "fmodf", "fmod",
];

const TRANSCENDENTAL_LIBM: &[&str] = &[
    "sin", "cos", "tan", "asin", "acos", "atan", "atan2", "exp", "exp2", "log", "log2", "log10",
    "pow", "sinh", "cosh", "tanh", "hypot", "cbrt", "sinf", "cosf", "tanf", "asinf", "acosf",
    "atanf", "atan2f", "expf", "exp2f", "logf", "log2f", "log10f", "powf", "sinhf", "coshf",
    "tanhf", "hypotf", "cbrtf",
];

pub fn clang_args(root: &Path) -> Result<Vec<String>> {
    let mut args: Vec<String> = [
        "-std=c99",
        "-DTARGET_UNIX",
        "-DNDEBUG",
        "-DWITH_NETWORK",
        "-DTYRIAN_DIR=\"/nonexistent\"",
        "-DOPENTYRIAN_VERSION=\"rustyrian-oracle\"",
        "-fsigned-char",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    args.push(format!(
        "-I{}",
        root.join("crates/oracle/shim/include").display()
    ));
    let cflags = run_capture(Command::new("pkg-config").args(["--cflags", "sdl2"]))?;
    args.extend(
        String::from_utf8(cflags)?
            .split_whitespace()
            .map(str::to_owned),
    );
    Ok(args)
}

fn dump_tu(root: &Path, clang: &str, args: &[String], rel: &str) -> Result<TuFacts> {
    let mut child = Command::new(clang)
        .current_dir(root)
        .args(["-fsyntax-only", "-Xclang", "-ast-dump=json"])
        .args(args)
        .arg(rel)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("spawning {clang}"))?;
    let stdout = child.stdout.take().context("no stdout")?;
    let facts = ast::walk(stdout).with_context(|| format!("parsing AST of {rel}"))?;
    let out = child.wait_with_output()?;
    if !out.status.success() {
        bail!(
            "clang failed on {rel}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(facts)
}

fn is_const_type(t: &str) -> bool {
    let mut t = t.trim();
    while t.ends_with(']') {
        match t.rfind('[') {
            Some(i) => t = t[..i].trim_end(),
            None => break,
        }
    }
    if t.contains('*') {
        t.ends_with("const")
    } else {
        t.starts_with("const ") || t.contains(" const")
    }
}

fn split_return_type(qual_type: &str) -> Option<String> {
    let bytes = qual_type.as_bytes();
    if !qual_type.ends_with(')') {
        return None;
    }
    let mut depth = 0i32;
    for i in (0..bytes.len()).rev() {
        match bytes[i] {
            b')' => depth += 1,
            b'(' => {
                depth -= 1;
                if depth == 0 {
                    let ret = qual_type[..i].trim_end();
                    if ret.contains('(') {
                        return None;
                    }
                    return Some(ret.to_owned());
                }
            }
            _ => {}
        }
    }
    None
}

fn identifiers(t: &str) -> impl Iterator<Item = &str> {
    t.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|s| !s.is_empty())
}

fn is_local_type(t: &str, local: &BTreeSet<String>) -> bool {
    t.contains("(unnamed") || t.contains("(anonymous") || identifiers(t).any(|i| local.contains(i))
}

fn collapse_ws(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_ws = false;
    for c in text.chars() {
        if c.is_whitespace() {
            if !in_ws {
                out.push(' ');
            }
            in_ws = true;
        } else {
            out.push(c);
            in_ws = false;
        }
    }
    out
}

fn fn_sha(source: &str, f: &FnFact) -> Result<String> {
    let begin = usize::try_from(f.range.begin.offset)?;
    let end = usize::try_from(f.range.end.offset + f.range.end.tok_len)?;
    let text = source
        .get(begin..end)
        .with_context(|| format!("bad range for {}", f.name))?;
    Ok(sha256_hex(collapse_ws(text).as_bytes())[..12].to_owned())
}

fn is_sdl_header(file: &str) -> bool {
    file.contains("SDL2/") || file.ends_with("SDL_net.h")
}

struct Thunk {
    name: String,
    target: String,
    ret: String,
    params: Vec<String>,
    /// Prototype for `statics.h`, absent when a by-value type is file-local.
    proto: Option<String>,
}

fn make_thunk(
    name: String,
    f: &FnFact,
    local: &BTreeSet<String>,
    enums: &BTreeSet<String>,
) -> Option<Thunk> {
    if f.qual_type.contains("...") {
        return None;
    }
    let ret = split_return_type(&f.qual_type)?;
    let decl_ty = |t: &str| -> Option<String> {
        if !is_local_type(t, local) {
            Some(format!("__typeof__({t})"))
        } else if t.contains('*') {
            Some("void *".to_owned())
        } else if identifiers(t)
            .filter(|i| local.contains(*i))
            .all(|i| enums.contains(i))
        {
            // File-local enum passed by value: ABI-identical to `int`.
            Some("int".to_owned())
        } else {
            None
        }
    };
    let proto = (|| {
        let r = decl_ty(&ret)?;
        let ps: Option<Vec<String>> = f.params.iter().map(|p| decl_ty(p)).collect();
        let ps = ps?;
        let plist = if ps.is_empty() {
            "void".to_owned()
        } else {
            ps.join(", ")
        };
        Some(format!("{r} {name}({plist});"))
    })();
    Some(Thunk {
        name,
        target: f.name.clone(),
        ret,
        params: f.params.clone(),
        proto,
    })
}

fn thunk_def(t: &Thunk) -> String {
    let params: Vec<String> = t
        .params
        .iter()
        .enumerate()
        .map(|(i, p)| format!("__typeof__({p}) a{i}"))
        .collect();
    let plist = if params.is_empty() {
        "void".to_owned()
    } else {
        params.join(", ")
    };
    let args: Vec<String> = (0..t.params.len()).map(|i| format!("a{i}")).collect();
    let call = format!("{}({})", t.target, args.join(", "));
    if t.ret == "void" {
        format!("void {}({plist}) {{ {call}; }}\n", t.name)
    } else {
        format!(
            "__typeof__({}) {}({plist}) {{ return {call}; }}\n",
            t.ret, t.name
        )
    }
}

pub fn run(root: &Path) -> Result<()> {
    let head = run_capture(
        Command::new("git")
            .current_dir(root.join("vendor/opentyrian"))
            .args(["rev-parse", "HEAD"]),
    )?;
    let head = String::from_utf8(head)?.trim().to_owned();
    if head != UPSTREAM_COMMIT {
        bail!("vendor/opentyrian is at {head}, expected pinned {UPSTREAM_COMMIT}");
    }

    let src = upstream_src(root);
    let mut c_files: Vec<String> = std::fs::read_dir(&src)?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".c"))
        .collect();
    c_files.sort();

    let clang = std::env::var("CLANG").unwrap_or_else(|_| "clang".to_owned());
    let args = clang_args(root)?;

    // Parse all TUs in parallel; each walk runs on a large stack.
    let results: Vec<(String, Result<TuFacts>)> = std::thread::scope(|s| {
        let handles: Vec<_> = c_files
            .iter()
            .map(|f| {
                let rel = format!("vendor/opentyrian/src/{f}");
                let (clang, args) = (&clang, &args);
                let h = std::thread::Builder::new()
                    .stack_size(512 << 20)
                    .spawn_scoped(s, move || dump_tu(root, clang, args, &rel))
                    .expect("spawn thread");
                (f.clone(), h)
            })
            .collect();
        handles
            .into_iter()
            .map(|(f, h)| {
                let r = h
                    .join()
                    .unwrap_or_else(|_| Err(anyhow::anyhow!("walker thread panicked")));
                (f, r)
            })
            .collect()
    });
    let mut tus: BTreeMap<String, TuFacts> = BTreeMap::new();
    for (f, r) in results {
        tus.insert(f, r?);
    }

    let prefix = "vendor/opentyrian/src/";
    let mut sources: BTreeMap<String, String> = BTreeMap::new();
    let mut source_of = |file: &str| -> Result<String> {
        if let Some(s) = sources.get(file) {
            return Ok(s.clone());
        }
        let s = std::fs::read_to_string(src.join(file))?;
        sources.insert(file.to_owned(), s.clone());
        Ok(s)
    };

    // SDL declarations: name -> (has_body, variadic).
    let mut sdl: BTreeMap<String, (bool, bool)> = BTreeMap::new();
    for facts in tus.values() {
        for f in &facts.functions {
            if is_sdl_header(&f.file) {
                let e = sdl.entry(f.name.clone()).or_insert((false, false));
                e.0 |= f.has_body;
                e.1 |= f.qual_type.contains("...");
            }
        }
    }

    let mut defs: Vec<Def> = Vec::new();
    let mut header_seen: BTreeSet<(String, String)> = BTreeSet::new();
    for (cfile, facts) in &tus {
        let main = format!("{prefix}{cfile}");
        for f in &facts.functions {
            if !f.has_body {
                continue;
            }
            if f.file == main {
                defs.push(Def {
                    file: cfile.clone(),
                    fact: f,
                    header: false,
                });
            } else if let Some(h) = f.file.strip_prefix(prefix)
                && h.ends_with(".h")
                && !h.contains('/')
                && header_seen.insert((h.to_owned(), f.name.clone()))
            {
                defs.push(Def {
                    file: h.to_owned(),
                    fact: f,
                    header: true,
                });
            }
        }
    }

    // Name resolution for call edges: same-file static first, then global
    // definitions, then header inlines.
    let mut by_name: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, d) in defs.iter().enumerate() {
        by_name.entry(d.fact.name.as_str()).or_default().push(i);
    }
    let resolve = |from_file: &str, name: &str| -> Option<String> {
        let cands = by_name.get(name)?;
        let pick = cands
            .iter()
            .find(|&&i| defs[i].file == from_file && defs[i].fact.storage_static)
            .or_else(|| cands.iter().find(|&&i| !defs[i].fact.storage_static))
            .or_else(|| cands.iter().find(|&&i| defs[i].header))
            .or_else(|| cands.first())?;
        Some(format!("{}::{}", defs[*pick].file, defs[*pick].fact.name))
    };

    let mut functions = Vec::new();
    let mut platform: BTreeSet<String> = BTreeSet::new();
    let mut libc: BTreeSet<String> = BTreeSet::new();
    let mut globals = Vec::new();
    for d in &defs {
        let source = source_of(&d.file)?;
        let mut calls: Vec<String> = Vec::new();
        for r in &d.fact.refs {
            if let Some(k) = resolve(&d.file, r) {
                calls.push(k);
            } else if let Some(&(has_body, _)) = sdl.get(r) {
                if !has_body && !REAL_SDL.contains(&r.as_str()) {
                    platform.insert(r.clone());
                }
            } else {
                libc.insert(r.clone());
            }
        }
        calls.sort();
        calls.dedup();
        functions.push(Function {
            file: d.file.clone(),
            name: d.fact.name.clone(),
            is_static: d.fact.storage_static,
            inline_header: d.header,
            begin_line: d.fact.range.begin.line,
            end_line: d.fact.range.end.line,
            c_sha: fn_sha(&source, d.fact)?,
            calls,
        });
        for v in &d.fact.local_statics {
            if is_const_type(&v.qual_type) {
                continue;
            }
            globals.push(Global {
                file: d.file.clone(),
                name: v.name.clone(),
                function: Some(d.fact.name.clone()),
                line: v.line,
                ty: v.qual_type.clone(),
                is_static: true,
            });
        }
    }
    for (cfile, facts) in &tus {
        let main = format!("{prefix}{cfile}");
        for v in &facts.vars {
            if v.file != main
                || v.storage_class.as_deref() == Some("extern")
                || is_const_type(&v.qual_type)
            {
                continue;
            }
            globals.push(Global {
                file: cfile.clone(),
                name: v.name.clone(),
                function: None,
                line: v.line,
                ty: v.qual_type.clone(),
                is_static: v.storage_class.as_deref() == Some("static"),
            });
        }
    }
    functions.sort_by(|a, b| (&a.file, a.begin_line).cmp(&(&b.file, b.begin_line)));
    globals.sort_by(|a, b| (&a.file, a.line, &a.name).cmp(&(&b.file, b.line, &b.name)));

    for name in &libc {
        if TRANSCENDENTAL_LIBM.contains(&name.as_str())
            && !LIBC_INTERCEPTED.contains(&name.as_str())
        {
            bail!("upstream calls libm `{name}`, which is neither exact nor intercepted (rule R6)");
        }
    }
    for name in LIBC_INTERCEPTED {
        if !libc.contains(*name) {
            bail!("intercepted libc function `{name}` is no longer used upstream");
        }
    }
    let _ = EXACT_LIBM;

    let inventory = Inventory {
        upstream: UPSTREAM_COMMIT.to_owned(),
        functions,
        globals,
        platform_calls: platform.iter().cloned().collect(),
        libc_calls: libc.iter().cloned().collect(),
    };
    let json = serde_json::to_string_pretty(&inventory)? + "\n";
    write_if_changed(&crate::inventory::path(root), json.as_bytes())?;

    let n_c = inventory
        .functions
        .iter()
        .filter(|f| !f.inline_header)
        .count();
    let n_static = inventory
        .functions
        .iter()
        .filter(|f| !f.inline_header && f.is_static)
        .count();
    let n_inline = inventory
        .functions
        .iter()
        .filter(|f| f.inline_header)
        .count();
    eprintln!(
        "inventory: {n_c} .c functions ({n_static} static), {n_inline} header inlines, {} globals ({} function-local), {} platform calls",
        inventory.globals.len(),
        inventory
            .globals
            .iter()
            .filter(|g| g.function.is_some())
            .count(),
        inventory.platform_calls.len()
    );

    generate(root, &tus, &defs, &inventory, &sdl)?;
    check_adapters(root, &inventory, &sdl)
}

struct Def<'a> {
    file: String,
    fact: &'a FnFact,
    header: bool,
}

fn generate(
    root: &Path,
    tus: &BTreeMap<String, TuFacts>,
    defs: &[Def],
    inv: &Inventory,
    sdl: &BTreeMap<String, (bool, bool)>,
) -> Result<()> {
    let gen_dir = root.join("crates/oracle/generated");
    let tu_dir = gen_dir.join("tu");
    std::fs::create_dir_all(&tu_dir)?;
    let header =
        "/* Generated by `cargo xtask gen-oracle` from upstream OpenTyrian; do not edit. */\n";
    let up = "../../../../vendor/opentyrian/src";

    let mut statics_h = String::from(header);
    statics_h.push_str("#ifndef OT_STATICS_H\n#define OT_STATICS_H\n\n");
    let mut lsv = String::from("# tu\tfunction\tvariable\tgetter\n");
    let mut expected_tus: BTreeSet<PathBuf> = BTreeSet::new();

    for (cfile, facts) in tus {
        let stem = cfile.trim_end_matches(".c");
        let main = format!("vendor/opentyrian/src/{cfile}");
        let local = facts.type_files.get(&main).cloned().unwrap_or_default();
        let enums = facts.enum_types.get(&main).cloned().unwrap_or_default();
        let mut tu = String::from(header);
        writeln!(tu, "#include \"{up}/{cfile}\"\n")?;
        writeln!(statics_h, "/* {cfile} */")?;
        for d in defs.iter().filter(|d| !d.header && d.file == *cfile) {
            if !d.fact.storage_static {
                continue;
            }
            let name = format!("ot_s_{stem}_{}", d.fact.name);
            match make_thunk(name, d.fact, &local, &enums) {
                Some(t) => {
                    tu.push_str(&thunk_def(&t));
                    match &t.proto {
                        Some(p) => writeln!(statics_h, "{p}")?,
                        None => writeln!(statics_h, "/* {}: file-local by-value type */", t.name)?,
                    }
                }
                None => writeln!(
                    statics_h,
                    "/* {}: no thunk (variadic or complex return) */",
                    d.fact.name
                )?,
            }
        }
        for g in inv.globals.iter().filter(|g| g.file == *cfile) {
            match &g.function {
                None => {
                    let getter = format!("ot_sv_{stem}_{}", g.name);
                    writeln!(tu, "void *{getter}(void) {{ return (void *)&{}; }}", g.name)?;
                    writeln!(statics_h, "void *{getter}(void);")?;
                }
                Some(func) => {
                    let getter = format!("ot_sv_{stem}_{func}_{}", g.name);
                    writeln!(lsv, "{stem}\t{func}\t{}\t{getter}", g.name)?;
                    writeln!(statics_h, "void *{getter}(void);")?;
                }
            }
        }
        writeln!(statics_h)?;
        let path = tu_dir.join(format!("{stem}.c"));
        write_if_changed(&path, tu.as_bytes())?;
        expected_tus.insert(path);
    }

    // Header static inline thunks.
    let mut it = String::from(header);
    let mut hdrs: BTreeSet<&str> = BTreeSet::new();
    for d in defs.iter().filter(|d| d.header) {
        hdrs.insert(&d.file);
    }
    for h in &hdrs {
        writeln!(it, "#include \"{up}/{h}\"")?;
    }
    it.push('\n');
    writeln!(statics_h, "/* header inlines */")?;
    let empty = BTreeSet::new();
    for d in defs.iter().filter(|d| d.header) {
        let stem = d.file.trim_end_matches(".h");
        let name = format!("ot_i_{stem}_{}", d.fact.name);
        match make_thunk(name, d.fact, &empty, &empty) {
            Some(t) => {
                it.push_str(&thunk_def(&t));
                if let Some(p) = &t.proto {
                    writeln!(statics_h, "{p}")?;
                }
            }
            None => writeln!(statics_h, "/* {}: no thunk */", d.fact.name)?,
        }
    }
    statics_h.push_str("\n#endif\n");
    let it_path = tu_dir.join("inline_thunks.c");
    write_if_changed(&it_path, it.as_bytes())?;
    expected_tus.insert(it_path);

    // Remove stale generated TUs.
    for e in std::fs::read_dir(&tu_dir)? {
        let p = e?.path();
        if !expected_tus.contains(&p) {
            std::fs::remove_file(&p)?;
        }
    }

    write_if_changed(&gen_dir.join("statics.h"), statics_h.as_bytes())?;
    write_if_changed(&gen_dir.join("local_statics.tsv"), lsv.as_bytes())?;

    // Interception header and signature typedefs.
    let intercepted: Vec<&str> = inv
        .platform_calls
        .iter()
        .map(String::as_str)
        .chain(LIBC_INTERCEPTED.iter().copied())
        .collect();
    let mut shim = String::from(header);
    shim.push_str(
        "#ifndef OT_SHIM_H\n#define OT_SHIM_H\n\n#include <stdlib.h>\n#include <math.h>\n#include <time.h>\n#include <SDL.h>\n#include <SDL_net.h>\n\n",
    );
    let mut types = String::from(header);
    types.push_str("#ifndef OT_SHIM_TYPES_H\n#define OT_SHIM_TYPES_H\n\n#include <stdlib.h>\n#include <math.h>\n#include <time.h>\n#include <SDL.h>\n#include <SDL_net.h>\n\n");
    for name in &intercepted {
        writeln!(shim, "__typeof__({name}) ot_shim_{name};")?;
        writeln!(shim, "#define {name} ot_shim_{name}")?;
        writeln!(types, "typedef __typeof__({name}) *ot_shim_t_{name};")?;
    }
    shim.push_str("\n#endif\n");
    types.push_str("\n#endif\n");
    write_if_changed(&gen_dir.join("ot_shim.h"), shim.as_bytes())?;
    write_if_changed(&gen_dir.join("shim_types.h"), types.as_bytes())?;

    let mut list = String::new();
    for name in &intercepted {
        writeln!(list, "{name}")?;
    }
    write_if_changed(&gen_dir.join("intercepted.txt"), list.as_bytes())?;

    // Compile-time signature checks for the Rust-implemented shims.
    let mut check = String::from("// Generated by `cargo xtask gen-oracle`; do not edit.\n");
    for name in &intercepted {
        if c_implemented(name, sdl) {
            continue;
        }
        writeln!(
            check,
            "const _: crate::bindings::ot_shim_t_{name} = Some(crate::shim::ot_shim_{name});"
        )?;
    }
    write_if_changed(&gen_dir.join("shim_check.rs"), check.as_bytes())?;
    Ok(())
}

fn c_implemented(name: &str, sdl: &BTreeMap<String, (bool, bool)>) -> bool {
    C_IMPLEMENTED_LIBC.contains(&name) || sdl.get(name).is_some_and(|&(_, variadic)| variadic)
}

fn check_adapters(
    root: &Path,
    inv: &Inventory,
    sdl: &BTreeMap<String, (bool, bool)>,
) -> Result<()> {
    let shim_rs =
        std::fs::read_to_string(root.join("crates/oracle/src/shim.rs")).unwrap_or_default();
    let mut shim_c = String::new();
    let shim_dir = root.join("crates/oracle/shim");
    if let Ok(rd) = std::fs::read_dir(&shim_dir) {
        for e in rd.flatten() {
            if e.path().extension().is_some_and(|x| x == "c") {
                shim_c.push_str(&std::fs::read_to_string(e.path())?);
            }
        }
    }
    let mut missing = Vec::new();
    for name in inv
        .platform_calls
        .iter()
        .map(String::as_str)
        .chain(LIBC_INTERCEPTED.iter().copied())
    {
        let needle = format!("ot_shim_{name}(");
        let found = if c_implemented(name, sdl) {
            shim_c.contains(&needle)
        } else {
            shim_rs.contains(&format!("fn ot_shim_{name}("))
        };
        if !found {
            missing.push(name.to_owned());
        }
    }
    if !missing.is_empty() {
        bail!(
            "intercepted functions without a shim adapter: {}",
            missing.join(", ")
        );
    }
    Ok(())
}
