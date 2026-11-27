// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `swar.h`.

use serde::{Deserialize, Serialize};

/// `Swar8`: an unsigned integer with 8-bit lanes. The oracle builds on LP64
/// hosts (`UINTPTR_MAX >= UINT64_MAX`), so it is `uint64_t`.
pub type Swar8 = u64;

/// `SWAR8_LANES`.
pub const SWAR8_LANES: usize = 8;

/// `SWAR8(x)`: replicates `x` into every lane.
pub const fn swar8(x: u8) -> Swar8 {
    (x as Swar8).wrapping_mul(Swar8::MAX / 0xFF)
}

/// `swar8Read`: the `SWAR8_LANES` bytes at the start of `src`, in native byte
/// order (C's `memcpy` into the integer).
#[doc(alias = "swar8Read")]
pub fn swar8_read(src: &[u8]) -> Swar8 {
    let mut values = [0u8; SWAR8_LANES];
    values.copy_from_slice(&src[..SWAR8_LANES]);
    Swar8::from_ne_bytes(values)
}

/// `swar8Write`: stores `values` into the first `SWAR8_LANES` bytes of `dst`.
#[doc(alias = "swar8Write")]
pub fn swar8_write(dst: &mut [u8], values: Swar8) {
    dst[..SWAR8_LANES].copy_from_slice(&values.to_ne_bytes());
}

/// `swar.h` has no mutable globals.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {}
