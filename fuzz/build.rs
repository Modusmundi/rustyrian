//! Link arguments of the `differential` targets that the oracle's build
//! script cannot pass on (`cargo:rustc-link-arg` applies only to the
//! binaries of the package that prints it):
//! - macOS: Homebrew's SDL2 is sdl2-compat, which loads SDL3 through
//!   `@rpath` when it initialises and blocks on a modal error dialog if it
//!   cannot; the binaries get an rpath to SDL3's libdir (as
//!   `crates/oracle/build.rs` gives its own binaries).
//! - `OT_REF_SANITIZE=1`: the oracle links clang's shared ASan runtime; the
//!   binaries get an rpath to clang's runtime directory.

use std::env;
use std::process::Command;

/// The trimmed stdout of a successful command, if it is not empty.
fn output(cmd: &str, args: &[&str]) -> Option<String> {
    Command::new(cmd)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .filter(|s| !s.is_empty())
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=OT_REF_SANITIZE");
    println!("cargo:rerun-if-env-changed=OT_REF_CC");
    println!("cargo:rerun-if-env-changed=CC");
    if env::var_os("CARGO_FEATURE_DIFFERENTIAL").is_none() {
        return;
    }

    if env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "macos") {
        let dir = output("pkg-config", &["--variable=libdir", "sdl3"]).expect(
            "the differential fuzz targets on macOS need `pkg-config --variable=libdir sdl3` \
             (brew install sdl3 pkg-config); without that rpath, sdl2-compat blocks on a \
             modal dialog at startup",
        );
        println!("cargo:rustc-link-arg=-Wl,-rpath,{dir}");
    }

    if env::var("OT_REF_SANITIZE").is_ok_and(|v| v == "1") {
        let cc = env::var("OT_REF_CC")
            .or_else(|_| env::var("CC"))
            .unwrap_or_else(|_| "clang".to_owned());
        let dir = output(&cc, &["--print-runtime-dir"])
            .unwrap_or_else(|| panic!("`{cc} --print-runtime-dir` (the ASan runtime's directory)"));
        println!("cargo:rustc-link-arg=-Wl,-rpath,{dir}");
    }
}
