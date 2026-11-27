// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) rustyrian contributors

//! C's float-to-`unsigned int` conversion where upstream reaches it with a
//! value outside the target range (undefined behaviour in C). Upstream's
//! result then depends on the CPU, so these mirror the build's target, as the
//! oracle shows (divergences D-UB-DESTRUCT-2, D-UB-OPL-1):
//! - x86-64: clang converts through a 64-bit `cvttss2si`/`cvttsd2si` and keeps
//!   the low 32 bits; NaN and values outside `[-2^63, 2^63)` give `i64::MIN`,
//!   so `+inf` becomes 0 and `-1.0` becomes `u32::MAX`.
//! - other targets (arm64 `fcvtzu`): saturating, which is Rust's `as`.
//!
//! In range both agree with `as`.

/// `2^63`, the bound of the x86-64 64-bit conversion.
const I64_LIMIT: f64 = 9_223_372_036_854_775_808.0;

/// `(unsigned int)f` for a C `double`.
pub fn f64_to_u32(f: f64) -> u32 {
    if cfg!(target_arch = "x86_64") {
        let i = if f.is_nan() || !(-I64_LIMIT..I64_LIMIT).contains(&f) {
            i64::MIN
        } else {
            f as i64
        };
        i as u32
    } else {
        f as u32
    }
}

/// `(unsigned int)f` for a C `float`.
pub fn f32_to_u32(f: f32) -> u32 {
    f64_to_u32(f64::from(f))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_range_values_convert_like_as() {
        for f in [0.0f64, 1.0, 1.9, 4_294_967_295.0, 12_345.678] {
            assert_eq!(f64_to_u32(f), f as u32);
        }
    }

    #[test]
    fn out_of_range_values_follow_the_target() {
        let (inf, neg, nan) = (
            f64_to_u32(f64::INFINITY),
            f64_to_u32(-1.0),
            f64_to_u32(f64::NAN),
        );
        if cfg!(target_arch = "x86_64") {
            assert_eq!((inf, neg, nan), (0, u32::MAX, 0));
        } else {
            assert_eq!((inf, neg, nan), (u32::MAX, 0, 0));
        }
        assert_eq!(f32_to_u32(-1.0), f64_to_u32(-1.0));
    }
}
