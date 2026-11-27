//! Repository automation for rustyrian. See `porting/RULES.md` and the README.

mod ast;
mod check_trace;
mod coverage;
mod fetch_data;
mod fuzz_seed;
mod gen_oracle;
mod inventory;
mod manifest;
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
    }
}
