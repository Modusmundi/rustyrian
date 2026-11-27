//! `ot-ref-run`: runs upstream OpenTyrian (C) headless on the fake platform
//! and writes a trace. See `rustyrian_oracle::runner` for the command line.

use rustyrian_oracle::{ot_c_main, runner, session};
use std::ffi::{CString, c_char, c_int};

fn main() -> anyhow::Result<()> {
    let args = runner::parse_args(std::env::args().skip(1))?;
    let s = runner::prepare(&args, "c")?;
    session::install(s);
    session::set_runner_mode(true);

    let argv: Vec<CString> = runner::game_argv(&args, "opentyrian")
        .into_iter()
        .map(CString::new)
        .collect::<Result<_, _>>()?;
    let mut ptrs: Vec<*mut c_char> = argv.iter().map(|c| c.as_ptr().cast_mut()).collect();
    ptrs.push(std::ptr::null_mut());
    let argc = c_int::try_from(argv.len())?;
    // SAFETY: argv is a NULL-terminated array of NUL-terminated strings that
    // outlive the call; upstream main may call exit(), which ends the process
    // after finishing the trace.
    let code = unsafe { ot_c_main(argc, ptrs.as_mut_ptr()) };
    session::run_exit(code);
    std::process::exit(code);
}
