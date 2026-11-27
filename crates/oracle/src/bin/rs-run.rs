//! `rs-run`: runs the Rust port headless on the fake platform and writes a
//! trace, exactly as `ot-ref-run` does for the C engine. See
//! `rustyrian_oracle::runner` for the command line.

use rustyrian_core::game::Halt;
use rustyrian_core::{Game, opentyr};
use rustyrian_oracle::runner;

fn main() -> anyhow::Result<()> {
    let args = runner::parse_args(std::env::args().skip(1))?;
    let session = runner::prepare(&args, "rust")?;
    let mut g = Game::new(Box::new(session));

    let argv = runner::game_argv(&args, "rustyrian");
    // `exit()` anywhere in the game arrives here as `Halt`; a run limit ends
    // the process inside the session.
    let code = match opentyr::main(&mut g, &argv) {
        Ok(code) | Err(Halt { code }) => code,
    };
    opentyr::run_atexit(&mut g);
    g.platform.exit(code);
    std::process::exit(code);
}
