# Porting rules

These rules apply to every function ported from upstream OpenTyrian
(`vendor/opentyrian`, pinned at `26e40a2`). The goal before sign-off is a
faithful port: same behaviour, same platform calls, same files, frame for frame.

## R1. Modules
One Rust module per C file, lowercase: `crates/core/src/<cfile>.rs`
(`video_scale_hqNx.c` → `video_scale_hqnx.rs`). Header `static inline`
functions go in the module of their header (`file.h` → `file.rs`).

## R2. Names
Functions and variables use `heck::ToSnakeCase` of the C name; types use
`ToUpperCamelCase`. Every ported function carries
`#[doc(alias = "<exact C name>")]`; `cargo xtask check-trace` finds ports by it.

## R3. State
Every C global, file `static`, and function-local `static` becomes a field of
`<module>::State`. Function-local statics are named `<fn>_<var>`.
`core::Game` aggregates every `State` plus `platform: Box<dyn Platform>` and
`audio: Arc<Mutex<AudioState>>`. Functions take `g: &mut Game`.

## R4. Types
|C type|Rust type|
|---|---|
|`Uint8`/`JE_byte`|`u8`|
|`Sint8`/`JE_shortint`|`i8`|
|`Uint16`/`JE_word`|`u16`|
|`Sint16`/`JE_integer`|`i16`|
|`Uint32`/`uint`/`unsigned`|`u32`|
|`Sint32`/`JE_longint`/`int`|`i32`|
|`long`|`i64`|
|`ulong`|`u64` (LP64, matching the oracle; upstream's Windows build has 32-bit `ulong`)|
|`size_t`|`usize`|
|`char` used arithmetically|`i8` (the oracle uses `-fsigned-char`)|
|`char` buffers|`[u8; N]`, kept NUL-terminated exactly as in C|
|`bool`/`JE_boolean`|`bool`|
|`float`/`JE_real`|`f32`|
|`double`|`f64`|

Fixed arrays stay `[T; N]`. Arrays over 4 KiB are `Box<[T; N]>`.

## R5. Integer semantics
- Reproduce C promotion explicitly: widen to `i32` where C promotes; truncate
  with `as` only where C stores.
- Use `wrapping_*` wherever C can wrap, unsigned or signed; the oracle is built
  with `-fwrapv`.
- `/`, `%` and `>>` on signed values already match C as built by clang and gcc.
- Debug overflow panics are bug detectors, never behaviour.

## R6. Floats
- Keep C's operation order and precision. Where C promotes to `double`
  (`M_PI` literals, `shots.c:140`, `varz.c:1112`, `destruct.c:939`), compute in
  `f64` and cast where C casts.
- Transcendentals use `libm::{sinf, cosf, sin, pow, powf, atanf}`, never `std`
  methods (the oracle routes C's calls to the same `libm`).
- `roundf`, `floorf`, `fabsf` and `sqrtf` are exact or correctly rounded
  everywhere, so `f32::round`, `floor`, `abs` and `sqrt` are used for them.
- Rust float-to-int `as` casts saturate. Where C converts an out-of-range value
  (undefined behaviour), treat it under R10.

## R7. RNG
Port `mt_rand*` exactly. Call order is behaviour: cosmetic draws share the
stream (`backgrnd.c:1053`, `varz.c:1112`, `tyrian2.c:518,1763`). libc `rand()`
calls use `core::crand::rand()` (the oracle's `rand` shim uses the same LCG).

## R8. Pointers
- Pointers into arrays become indices: `usize`, or `isize` when C forms a
  one-before-begin pointer (`tyrian2.c:693-695` `&mainmap[mapY][0] - 1`).
- `VGAScreen`/`VGAScreen2`/`VGAScreenSeg`/`game_screen` become `ScreenId` and
  `g.video.screens[ScreenId]`.
- `player[p].lives` (`mainint.c:1053`) becomes a method resolving to the same
  item field.
- `void *enemydatofs` becomes `Option<usize>`.
- Function pointers become enums with a `match`: scalers
  (`video_scale.c:44-56`), OPL `opfuncs` (`opl.c:459-461`), menu picker
  callbacks (`opentyr.c:132-133`).

## R9. Control flow
- `goto` labels become `loop { match label { … } }` with an enum whose variants
  are the C label names.
- `exit(code)` becomes `return Err(Halt { code })`, with
  `type Flow<T = ()> = Result<T, Halt>` propagated by `?`. `main` runs the
  `atexit` teardown and exits with `code`.

## R10. UB and invalid input
- Where valid data provably makes C read out of bounds or uninitialised memory,
  reproduce the observed deterministic value and record a `ub-guard` divergence.
- Where only malformed external data reaches UB (data files, saves, demos,
  packets), validate at the input boundary and return an error, recorded as
  `ub-guard`.
- Never panic on external input.

## R11. Configuration
Port the `NDEBUG` (release) behaviour. `assert` becomes `debug_assert!`.
`WITH_NETWORK` is always on. Only the OPL2 path is ported (`OPLTYPE_IS_OPL3` is
undefined upstream), recorded as `dead-config`.

## R12. Change scope
No refactors or features before sign-off. The only allowed behaviour
differences are R10 guards, `D-PRODUCT-1` (R14), and the `security`
divergences recorded in `porting/divergences.md` (netplay input validation).

## R13. License headers
Each module starts with the C file's SPDX license and copyright lines.

|Module|License|
|---|---|
|`opl.rs`|`LGPL-2.1-or-later`; DOSBox Team, Ken Silverman|
|`mtrand.rs`|`BSD-3-Clause`, notice verbatim|
|`config_file.rs`|`GPL-2.0-or-later OR MPL-2.0`|
|`video_scale.rs`|`GPL-2.0-or-later`; Scale2x notice|
|`video_scale_hqnx.rs`|`GPL-2.0-or-later`; MaxSt notice|
|all others|`GPL-2.0-or-later`, "Copyright (C) The OpenTyrian Development Team" plus "Copyright (C) rustyrian contributors"|

## R14. Paths and formats
File formats and names are identical to upstream (`opentyrian.cfg`,
`tyrian.cfg`, `tyrian.sav`, `demorec.N`, `newsh$.shp`), as is the data-directory
search (`file.c:66-119`). The user directory differs: `$XDG_CONFIG_HOME/rustyrian`,
then `$HOME/.config/rustyrian` on Unix, `%APPDATA%\rustyrian` on Windows.
Portable mode keeps upstream's rule (an `opentyrian.cfg` beside the executable
selects that directory). This is divergence `D-PRODUCT-1`.

The `determineUserDirPath` differential test asserts that the Rust path equals
the C path with its final component `opentyrian`/`OpenTyrian` replaced by
`rustyrian`. The shipped binary's `--userdir DIR` uses `DIR` directly; the
runners set `XDG_CONFIG_HOME` instead, so C uses `DIR/opentyrian` and Rust
`DIR/rustyrian`, and `xtask parity` seeds both identically.

# Per-function recipe

1. `cargo xtask next --n 5`; take the first group.
2. Port it into its module following R1–R14. Add new globals to `State` and to
   the module's mirror (`crates/oracle/src/mirror/<module>.rs`).
3. Write differential tests in `crates/oracle/tests/<module>.rs`. Each test
   builds the initial state, calls `store_to_c()`, calls the C function (direct
   extern, or an `ot_s_*` thunk for statics), snapshots the C state with
   `load_from_c()`, runs the Rust function on the initial state, and asserts
   equal return values, `Game` state, surfaces, and platform call log.

   |Function type|Test style|
   |---|---|
   |Pure or argument-driven|`proptest` over arguments and the state fields it reads|
   |Mid-game state|Fixture snapshots (`porting/fixtures.toml`) plus `proptest` perturbations|
   |Data-driven|Every real data file: all episodes' levels, items, shapes, songs, sounds|

   `PROPTEST_CASES=256` on PRs, 10,000 nightly.
4. Iterate until `cargo nextest run -p rustyrian-oracle -E 'test(/^<module>::/)'` passes.
5. Set `status = "verified"` and fill `verify` in `porting/functions.toml`;
   `cargo xtask check-trace` must pass.
6. Commit as `port(<module>): <C names>`.

## Definition of verified
- The listed differential or trace tests pass on macOS arm64 and Linux x86_64.
- C line coverage of the function body from those tests is at least 95%, net of
  `waived_lines` (each with a reason).
- clippy is clean; no `unsafe`.

Functions that never return per call (`main`, `JE_main`, `titleScreen`,
`JE_itemScreen`, `setupMenu`, the `network_*` handshake, `JE_destructMain`) use
`trace:` verification: frame-by-frame trace identity plus the coverage rule.

Platform-facing functions are verified the same way; their tests compare the
`PlatformCall` sequences and use `FakePlatform::fail_next` for error paths.

`replaced` is only for a C function whose whole body is a Rust standard
facility (the byte-swap inlines in `file.h`); it needs a `notes` justification
and a `verify` test covering its callers.

# Oracle notes

- Function-local statics are exported without patching upstream: `build.rs`
  compiles each wrapper TU to LLVM IR, appends an `ot_sv_*` getter returning the
  static's address, then optimises. The oracle therefore requires clang (also
  for coverage builds, which use clang's gcov-format `--coverage` with
  `llvm-cov gcov`).
- `Platform` has three methods that are not 1:1 SDL functions: `time` and
  `localtime` (libc) and `log_output` (the function pointer returned by
  `SDL_LogGetOutputFunction`).
- The trace's end line also carries `calls`, the hash of platform calls after
  the last present (atexit teardown).
