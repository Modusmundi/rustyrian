//! Builds the upstream C sources (unmodified, through the generated wrapper
//! TUs) into `libotref.a`, the C shims into `libotshim.a`, and Rust bindings.
//!
//! Each wrapper TU is compiled in two stages so that function-local statics can
//! be exported without patching upstream: clang emits LLVM IR with no
//! optimisation passes, `build.rs` appends `ot_sv_*` getter functions returning
//! the address of each local static named in `generated/local_statics.tsv`, and
//! clang then optimises and compiles the IR. Because the getters reference the
//! globals in IR, the optimiser treats them as escaped.

use std::collections::BTreeMap;
use std::env;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

fn run(cmd: &mut Command) {
    let status = cmd
        .status()
        .unwrap_or_else(|e| panic!("failed to spawn {cmd:?}: {e}"));
    assert!(status.success(), "{cmd:?} exited with {status}");
}

fn flag(name: &str) -> bool {
    println!("cargo:rerun-if-env-changed={name}");
    env::var(name).is_ok_and(|v| v == "1")
}

/// Links the first clang runtime library found in the compiler's runtime and
/// resource directories.
fn link_runtime(cc: &str, candidates: &[&str]) {
    let query = |arg: &str| -> Option<PathBuf> {
        let out = Command::new(cc).arg(arg).output().ok()?;
        let s = String::from_utf8(out.stdout).ok()?;
        let p = PathBuf::from(s.trim());
        p.is_dir().then_some(p)
    };
    let mut dirs: Vec<PathBuf> = Vec::new();
    dirs.extend(query("--print-runtime-dir"));
    if let Some(res) = query("-print-resource-dir") {
        dirs.push(res.join("lib/darwin"));
        dirs.push(res.join("lib/linux"));
    }
    for dir in &dirs {
        for name in candidates {
            if !dir.join(name).is_file() {
                continue;
            }
            let stem = name.trim_start_matches("lib");
            let (kind, lib) = match stem.rsplit_once('.') {
                Some((lib, "a")) => ("static", lib),
                Some((lib, _)) => ("dylib", lib),
                None => ("dylib", stem),
            };
            println!("cargo:rustc-link-search=native={}", dir.display());
            println!("cargo:rustc-link-lib={kind}={lib}");
            if kind == "dylib" {
                println!("cargo:rustc-link-arg=-Wl,-rpath,{}", dir.display());
            }
            return;
        }
    }
    panic!("none of {candidates:?} found under {dirs:?}");
}

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let root = manifest
        .join("../..")
        .canonicalize()
        .expect("repository root");
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let gen_dir = manifest.join("generated");
    let shim = manifest.join("shim");
    let upstream = root.join("vendor/opentyrian/src");

    for p in [&gen_dir, &shim, &upstream] {
        println!("cargo:rerun-if-changed={}", p.display());
    }
    println!("cargo:rerun-if-env-changed=OT_REF_CC");
    println!("cargo:rerun-if-env-changed=CC");

    let sdl = pkg_config::Config::new()
        .cargo_metadata(false)
        .probe("sdl2")
        .expect("pkg-config could not find sdl2");
    for p in &sdl.link_paths {
        println!("cargo:rustc-link-search=native={}", p.display());
    }
    for fw in &sdl.frameworks {
        println!("cargo:rustc-link-lib=framework={fw}");
    }
    println!("cargo:rustc-link-lib=SDL2");

    let cc = env::var("OT_REF_CC")
        .or_else(|_| env::var("CC"))
        .unwrap_or_else(|_| "clang".to_owned());
    let release = env::var("PROFILE").is_ok_and(|p| p == "release");
    let sanitize = flag("OT_REF_SANITIZE");
    let coverage = flag("OT_REF_COVERAGE");
    println!("cargo:rustc-check-cfg=cfg(ot_ref_sanitize)");
    if sanitize {
        println!("cargo:rustc-cfg=ot_ref_sanitize");
    }

    let mut common: Vec<String> = [
        "-std=c99",
        "-g",
        "-w",
        "-Wno-unused-command-line-argument",
        "-DTARGET_UNIX",
        "-DNDEBUG",
        "-DWITH_NETWORK",
        "-DTYRIAN_DIR=\"/nonexistent\"",
        "-DOPENTYRIAN_VERSION=\"rustyrian-oracle\"",
        "-fwrapv",
        "-fno-strict-aliasing",
        "-fsigned-char",
        "-ffp-contract=off",
        "-ftrivial-auto-var-init=zero",
        "-fPIC",
        "-Dmain=ot_c_main",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    // The shipped version string; upstream shows it on the title screen, so
    // the Rust engine uses the same one (`opentyr::OPENTYRIAN_VERSION`).
    common.push(format!(
        "-DOPENTYRIAN_VERSION=\"rustyrian-{}\"",
        env!("CARGO_PKG_VERSION")
    ));
    common.push(if release { "-O2" } else { "-O1" }.to_owned());
    common.push("-include".to_owned());
    common.push(gen_dir.join("ot_shim.h").display().to_string());
    common.push(format!("-I{}", shim.join("include").display()));
    for p in &sdl.include_paths {
        common.push(format!("-I{}", p.display()));
    }
    for (k, v) in &sdl.defines {
        common.push(match v {
            Some(v) => format!("-D{k}={v}"),
            None => format!("-D{k}"),
        });
    }
    // rustc links with `-nodefaultlibs`, so the clang driver does not add the
    // sanitizer or profile runtimes; link them explicitly.
    if sanitize {
        // Fortified libc calls (`__strcpy_chk`, on by default in the macOS SDK
        // when optimising) bypass ASan's interceptors; use the plain ones.
        common.extend(
            [
                "-fsanitize=address,undefined",
                "-fno-omit-frame-pointer",
                "-U_FORTIFY_SOURCE",
                "-D_FORTIFY_SOURCE=0",
            ]
            .map(String::from),
        );
        // Known upstream defects on valid game paths (each names its UF row).
        let ignorelist = manifest.join("asan.ignorelist");
        println!("cargo:rerun-if-changed={}", ignorelist.display());
        common.push(format!("-fsanitize-ignorelist={}", ignorelist.display()));
        link_runtime(
            &cc,
            &[
                "libclang_rt.asan_osx_dynamic.dylib",
                "libclang_rt.asan.so",
                "libclang_rt.asan-x86_64.so",
                "libclang_rt.asan-aarch64.so",
            ],
        );
        // Homebrew's sdl2-compat loads SDL3 through @rpath at process start
        // and blocks on a modal error dialog if it cannot. Under the ASan
        // runtime its own @loader_path rpath does not resolve, so give the
        // executable an rpath to SDL3 as well.
        if env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "macos") {
            let dir = Command::new("pkg-config")
                .args(["--variable=libdir", "sdl3"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
                .filter(|d| !d.is_empty())
                .expect(
                    "OT_REF_SANITIZE on macOS needs `pkg-config --variable=libdir sdl3`; \
                     without that rpath, sdl2-compat blocks on a modal dialog at startup",
                );
            println!("cargo:rustc-link-arg=-Wl,-rpath,{dir}");
        }
    }
    if coverage {
        common.push("--coverage".to_owned());
        link_runtime(
            &cc,
            &[
                "libclang_rt.profile_osx.a",
                "libclang_rt.profile.a",
                "libclang_rt.profile-x86_64.a",
                "libclang_rt.profile-aarch64.a",
            ],
        );
    }

    // Local statics per TU.
    let mut lsv: BTreeMap<String, Vec<(String, String, String)>> = BTreeMap::new();
    let tsv =
        std::fs::read_to_string(gen_dir.join("local_statics.tsv")).expect("local_statics.tsv");
    for line in tsv.lines().filter(|l| !l.starts_with('#') && !l.is_empty()) {
        let cols: Vec<&str> = line.split('\t').collect();
        assert_eq!(cols.len(), 4, "bad local_statics.tsv line: {line}");
        lsv.entry(cols[0].to_owned()).or_default().push((
            cols[1].to_owned(),
            cols[2].to_owned(),
            cols[3].to_owned(),
        ));
    }

    let tu_dir = gen_dir.join("tu");
    let mut tus: Vec<PathBuf> = std::fs::read_dir(&tu_dir)
        .expect("generated/tu")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "c"))
        .collect();
    tus.sort();

    let obj_dir = out.join("otref");
    std::fs::create_dir_all(&obj_dir).expect("create obj dir");
    let objects: Vec<PathBuf> = std::thread::scope(|s| {
        let handles: Vec<_> = tus
            .iter()
            .map(|tu| {
                let (cc, common, obj_dir, lsv, root) = (&cc, &common, &obj_dir, &lsv, &root);
                s.spawn(move || compile_tu(cc, common, root, obj_dir, tu, lsv))
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("compile thread panicked"))
            .collect()
    });

    let lib = out.join("libotref.a");
    let _ = std::fs::remove_file(&lib);
    let ar = cc::Build::new().get_archiver();
    let mut ar = Command::new(ar.get_program());
    ar.arg("crs").arg(&lib).args(&objects);
    run(&mut ar);
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=otref");

    let mut shim_build = cc::Build::new();
    shim_build
        .compiler(&cc)
        .files([shim.join("ot_shim_va.c"), shim.join("ot_guard.c")])
        .include(&gen_dir)
        .include(shim.join("include"))
        .includes(&sdl.include_paths)
        .flag("-std=c99")
        .warnings(false);
    if sanitize {
        shim_build.flag("-fsanitize=address,undefined");
    }
    shim_build.compile("otshim");

    let mut bg = bindgen::Builder::default()
        .header(shim.join("wrapper.h").display().to_string())
        .clang_arg(format!("-I{}", upstream.display()))
        .clang_arg(format!("-I{}", gen_dir.display()))
        .clang_arg(format!("-I{}", shim.join("include").display()))
        .clang_args(["-DTARGET_UNIX", "-DNDEBUG", "-DWITH_NETWORK", "-fsigned-char"])
        .allowlist_file(".*/vendor/opentyrian/src/.*\\.h")
        .allowlist_file(".*/oracle/generated/.*\\.h")
        .allowlist_file(".*/oracle/shim/wrapper\\.h")
        .allowlist_type("SDL_Event|SDL_EventType|SDL_WindowEventID|SDL_LogCategory|SDL_LogPriority|SDL_AudioSpec|SDL_AudioCVT|SDL_Rect|SDL_Surface|SDL_Color|SDL_LogOutputFunction|UDPpacket|IPaddress|SDL_Keysym")
        .allowlist_var("SDL_.*|AUDIO_.*|KMOD_.*")
        .default_enum_style(bindgen::EnumVariation::Consts)
        .derive_default(true)
        .layout_tests(false)
        .generate_comments(false)
        .rust_edition(bindgen::RustEdition::Edition2024)
        .wrap_unsafe_ops(true);
    for p in &sdl.include_paths {
        bg = bg.clang_arg(format!("-I{}", p.display()));
    }
    let bindings = bg.generate().expect("bindgen failed");
    bindings
        .write_to_file(out.join("bindings.rs"))
        .expect("write bindings.rs");
}

/// Compiles one wrapper TU. Runs from the repository root so debug info and
/// gcov notes name sources as `vendor/opentyrian/src/<file>.c`.
fn compile_tu(
    cc: &str,
    common: &[String],
    root: &Path,
    obj_dir: &Path,
    tu: &Path,
    lsv: &BTreeMap<String, Vec<(String, String, String)>>,
) -> PathBuf {
    let stem = tu
        .file_stem()
        .expect("tu stem")
        .to_string_lossy()
        .into_owned();
    let ll = obj_dir.join(format!("{stem}.ll"));
    let obj = obj_dir.join(format!("{stem}.o"));
    run(Command::new(cc)
        .current_dir(root)
        .args(common)
        .args(["-Xclang", "-disable-llvm-passes", "-S", "-emit-llvm", "-o"])
        .arg(&ll)
        .arg(tu));
    if let Some(statics) = lsv.get(&stem) {
        let mut ir = std::fs::read_to_string(&ll).expect("read .ll");
        let mut extra = String::new();
        for (func, var, getter) in statics {
            let prefix = format!("@{func}.{var}");
            let global = ir
                .lines()
                .filter_map(|l| l.split_once(" = "))
                .map(|(name, _)| name)
                .find(|name| {
                    *name == prefix
                        || name.strip_prefix(&prefix).is_some_and(|rest| {
                            rest.strip_prefix('.')
                                .is_some_and(|n| n.chars().all(|c| c.is_ascii_digit()))
                        })
                })
                .unwrap_or_else(|| panic!("{stem}: no IR global for local static {func}.{var}"))
                .to_owned();
            writeln!(extra, "\ndefine ptr @{getter}() {{\n  ret ptr {global}\n}}").expect("fmt");
        }
        ir.push_str(&extra);
        std::fs::write(&ll, ir).expect("write .ll");
    }
    run(Command::new(cc)
        .current_dir(root)
        .args(common)
        .arg("-c")
        .arg("-o")
        .arg(&obj)
        .arg(&ll));
    obj
}
