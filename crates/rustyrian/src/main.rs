// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! The shipped `rustyrian` binary: upstream's `main` on the SDL platform, or
//! headless on the fake platform, optionally instrumented (traces, frame
//! dumps, input scripts, perf log).
#![forbid(unsafe_code)]

use rustyrian::cli::{self, Options};
use rustyrian_core::game::Halt;
use rustyrian_core::platform::{Platform, PlatformProbe};
use rustyrian_core::{Game, opentyr};
use rustyrian_harness::{
    Clock, FakeConfig, FakePlatform, InstrumentConfig, Instrumented, NetBackend, script,
};
use rustyrian_sdl::SdlPlatform;
use std::io::Write;

/// Whether any instrumentation flag asks for the instrumented platform.
fn instrumented(o: &Options) -> bool {
    o.input_script.is_some()
        || o.trace_out.is_some()
        || o.log_out.is_some()
        || o.frames.is_some()
        || o.until_ms.is_some()
        || o.dump_frames.is_some()
        || o.perf_log.is_some()
}

/// The executable's directory, which `SDL_GetBasePath` reports (with a
/// trailing separator).
fn exe_dir() -> Option<String> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.display().to_string()))
}

/// The instrumentation the options ask for.
fn instrument_config(o: &Options) -> Result<InstrumentConfig, String> {
    let script = match &o.input_script {
        Some(path) => {
            let text = std::fs::read_to_string(path)
                .map_err(|e| format!("reading {}: {e}", path.display()))?;
            Some(script::parse(&text).map_err(|e| format!("{}:{e}", path.display()))?)
        }
        None => None,
    };
    let scenario = o
        .input_script
        .as_ref()
        .and_then(|p| p.file_stem())
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(InstrumentConfig {
        engine: "rust".to_owned(),
        scenario,
        script,
        trace_out: o.trace_out.clone(),
        log_out: o.log_out.clone(),
        perf_log: o.perf_log.clone(),
        frames: o.frames,
        until_ms: o.until_ms,
        dump_frames: o.dump_frames.clone().map(|d| (d, o.dump_every)),
        // The base path is recorded as `<base>`, as the oracle runners do,
        // so traces do not depend on where the binary is installed.
        redact: exe_dir()
            .map(|d| (d, "<base>".to_owned()))
            .into_iter()
            .collect(),
        ..InstrumentConfig::default()
    })
}

/// `inner`, wrapped in the instrumented platform when asked for.
fn wrap<P: Platform + PlatformProbe + 'static>(
    inner: P,
    o: &Options,
) -> Result<Box<dyn Platform>, String> {
    if instrumented(o) {
        let config = instrument_config(o)?;
        let p = Instrumented::new(inner, config).map_err(|e| e.to_string())?;
        Ok(Box::new(p))
    } else {
        Ok(Box::new(inner))
    }
}

/// The platform the options select: SDL, or the fake platform headless.
fn platform(o: &Options) -> Result<Box<dyn Platform>, String> {
    if o.headless {
        // The executable's directory, as SDL_GetBasePath reports it (portable
        // mode looks for `opentyrian.cfg` there).
        let base_path = exe_dir().map(|d| format!("{d}/"));
        let fake = FakePlatform::new(FakeConfig {
            clock: if o.clock_real {
                Clock::Real
            } else {
                Clock::Virtual
            },
            net: NetBackend::Udp,
            base_path,
            echo_logs: true,
        });
        wrap(fake, o)
    } else {
        wrap(SdlPlatform::new(), o)
    }
}

fn main() {
    let o = match cli::parse(std::env::args()) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("rustyrian: {e}");
            std::process::exit(1);
        }
    };
    if o.help {
        // Upstream prints its own options after these.
        let _ = std::io::stdout().write_all(cli::HELP.as_bytes());
    }

    let platform = match platform(&o) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("rustyrian: {e}");
            std::process::exit(1);
        }
    };
    let mut g = Game::new(platform);

    // D-PRODUCT-2: `--userdir DIR` is the user directory itself.
    if let Some(dir) = &o.userdir {
        let bytes = dir.as_os_str().as_encoded_bytes().to_vec();
        g.file.user_dir_path_len = bytes.len();
        g.file.user_dir_path = Some(bytes);
    }

    let code = match opentyr::main(&mut g, &o.game_args) {
        Ok(code) | Err(Halt { code }) => code,
    };
    opentyr::run_atexit(&mut g);
    g.platform.exit(code);
    std::process::exit(code);
}
