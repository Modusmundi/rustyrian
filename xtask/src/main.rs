//! Repository automation for rustyrian. See `porting/RULES.md` and the README.

mod ast;
mod bench;
mod check_trace;
mod coverage;
mod crossplay;
mod fetch_data;
mod fuzz_seed;
mod gen_oracle;
mod inventory;
mod manifest;
mod parity;
mod sanitize;
mod scenario;
mod tools;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xtask", about = "rustyrian repository automation")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Download, verify, and extract the freeware Tyrian 2.1 data into `data/`.
    FetchData,
    /// Regenerate `porting/inventory.json` and the oracle's generated C/headers.
    GenOracle,
    /// Sync `porting/functions.toml` with `porting/inventory.json`.
    ManifestSync,
    /// Print the next `todo` functions in callee-first order.
    Next {
        #[arg(long, default_value_t = 5)]
        n: usize,
    },
    /// Traceability gate; writes `porting/STATUS.md`.
    CheckTrace,
    /// ASan/UBSan oracle on every scenario and the oracle tests.
    Sanitize,
    /// Snapshot the C oracle at each fixture in `porting/fixtures.toml`.
    Fixtures,
    /// Per-function C line coverage into `porting/coverage.json`.
    Coverage,
    /// Copy the freeware data files into `fuzz/corpus/<target>/`.
    FuzzSeed,
    /// Run the C oracle on a scenario and write its trace.
    RefTrace {
        scenario: String,
        /// Output directory (default `target/parity/<scenario>`).
        #[arg(long)]
        out: Option<std::path::PathBuf>,
        #[arg(long)]
        release: bool,
    },
    /// Compare the C oracle and the Rust runner on a scenario (traces and user
    /// directories); diagnose the first differing frame.
    Parity {
        scenario: String,
        /// Copy the C traces to `tests/golden/`.
        #[arg(long)]
        bless: bool,
        /// Rust runner binary (default: `rs-run` of the oracle crate).
        #[arg(long)]
        rust_runner: Option<std::path::PathBuf>,
        /// Test only: the Rust side runs this script instead of the
        /// scenario's first one.
        #[arg(long, hide = true)]
        rust_script: Option<std::path::PathBuf>,
        #[arg(long)]
        release: bool,
    },
    /// Rust/C headless throughput on `demo-cycle`; fails below 0.90.
    BenchRatio {
        /// Rust runner binary (default: `rs-run` of the oracle crate).
        #[arg(long)]
        rust_runner: Option<std::path::PathBuf>,
    },
    /// C (player 1) and Rust (player 2) netplay over UDP on 127.0.0.1.
    Crossplay {
        /// Rust runner binary (default: `rs-run` of the oracle crate).
        #[arg(long)]
        rust_runner: Option<std::path::PathBuf>,
        #[arg(long)]
        release: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = tools::repo_root()?;
    match cli.cmd {
        Cmd::FetchData => fetch_data::run(&root),
        Cmd::GenOracle => gen_oracle::run(&root),
        Cmd::ManifestSync => manifest::sync(&root),
        Cmd::Next { n } => manifest::next(&root, n),
        Cmd::CheckTrace => check_trace::run(&root),
        Cmd::Sanitize => sanitize::run(&root),
        Cmd::Fixtures => scenario::fixtures(&root),
        Cmd::Coverage => coverage::run(&root),
        Cmd::FuzzSeed => fuzz_seed::run(&root),
        Cmd::RefTrace {
            scenario,
            out,
            release,
        } => scenario::ref_trace(&root, &scenario, out.as_deref(), release),
        Cmd::Parity {
            scenario,
            bless,
            rust_runner,
            rust_script,
            release,
        } => {
            let opts = parity::Options {
                scenario,
                bless,
                rust_runner: runner_path(rust_runner)?,
                rust_script: rust_script.map(std::path::absolute).transpose()?,
                release,
            };
            if !parity::run(&root, &opts)? {
                std::process::exit(1);
            }
            Ok(())
        }
        Cmd::BenchRatio { rust_runner } => bench::run(&root, runner_path(rust_runner)?.as_deref()),
        Cmd::Crossplay {
            rust_runner,
            release,
        } => crossplay::run(&root, runner_path(rust_runner)?.as_deref(), release),
    }
}

/// `--rust-runner`, made absolute: runners start in the repository root.
fn runner_path(p: Option<std::path::PathBuf>) -> Result<Option<std::path::PathBuf>> {
    p.map(|p| {
        std::fs::canonicalize(&p).map_err(|e| anyhow::anyhow!("--rust-runner {}: {e}", p.display()))
    })
    .transpose()
}
