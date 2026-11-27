# rustyrian

A faithful, function-by-function Rust port of
[OpenTyrian](https://github.com/opentyrian/opentyrian), verified against the
original C at every step.

Until the port is signed off, behaviour is meant to be identical to upstream
OpenTyrian (pinned at `26e40a2`), frame for frame, sample for sample, and file
for file; the only differences are recorded in
[`porting/divergences.md`](porting/divergences.md). Progress is tracked in
[`porting/STATUS.md`](porting/STATUS.md).

## Layout

|Path|Contents|
|---|---|
|`crates/core`|All ported game logic, one module per upstream C file; no `unsafe`, no SDL. Owns the `Platform` trait.|
|`crates/harness`|Fake platform, `.ots` input scripts, virtual clock, call recording and `.jsonl` traces.|
|`crates/oracle`|Upstream C built unmodified on the fake platform (`ot-ref-run`), bindings, mirrors and differential tests.|
|`xtask`|Repository automation (`cargo xtask --help`).|
|`porting/`|Inventory, per-function status, rules, divergences, upstream findings.|
|`tests/scenarios`|Input scripts and run parameters for trace comparisons.|
|`vendor/opentyrian`|Upstream source (git submodule, pinned).|

## Building

Prerequisites: Rust (the toolchain in `rust-toolchain.toml` installs
automatically through rustup), and for the oracle: clang, pkg-config and the
SDL2 development headers (`brew install sdl2-compat pkg-config` on macOS;
`apt install clang-18 libclang-18-dev libsdl2-dev` on Debian/Ubuntu).

```sh
git clone --recurse-submodules https://github.com/Modusmundi/rustyrian
cd rustyrian
cargo xtask fetch-data        # freeware Tyrian 2.1 data into data/, verified by SHA-256
cargo nextest run --workspace
```

Run the C reference on a scenario and inspect it:

```sh
cargo xtask ref-trace demo-cycle --release
cargo run --release -p rustyrian-oracle --bin ot-ref-run -- \
  --data data --userdir target/u --input tests/scenarios/demo-cycle.ots \
  --trace-out target/t.jsonl --until-ms 60000 --dump-frames target/frames --dump-every 100
```

## Process

See [`porting/RULES.md`](porting/RULES.md) for the porting rules, the
per-function recipe and the definition of "verified". `cargo xtask check-trace`
is the traceability gate run in CI.

## Licenses

rustyrian is GPL-2.0-or-later, like OpenTyrian; individual modules keep the
licenses of the files they port (see `porting/RULES.md`, R13). The Tyrian 2.1
data is freeware (`licenses/Tyrian.txt`); SDL2 is zlib-licensed
(`licenses/SDL2.txt`).
