// SPDX-License-Identifier: BSD-3-Clause
// Copyright (C) 1997--2004, Makoto Matsumoto, Takuji Nishimura, and Eric Landry
// Copyright (C) rustyrian contributors
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions
// are met:
//
//   1. Redistributions of source code must retain the above copyright
//      notice, this list of conditions and the following disclaimer.
//
//   2. Redistributions in binary form must reproduce the above copyright
//      notice, this list of conditions and the following disclaimer
//      in the documentation and/or other materials provided with the
//      distribution.
//
//   3. The names of its contributors may not be used to endorse or
//      promote products derived from this software without specific
//      prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
// "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
// LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED.  IN NO EVENT SHALL THE COPYRIGHT
// OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
// SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
// LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
// DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
// THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//
// Reference: M. Matsumoto and T. Nishimura, "Mersenne Twister:
// A 623-Dimensionally Equidistributed Uniform Pseudo-Random Number
// Generator", ACM Transactions on Modeling and Computer Simulation,
// Vol. 8, No. 1, January 1998, pp 3--30.

//! Port of upstream `mtrand.c` / `mtrand.h`: the Mersenne Twister behind
//! every `mt_rand()` draw (rule R7).

use crate::Game;
use serde::{Deserialize, Serialize};

/// `MT_RAND_MAX`.
pub const MT_RAND_MAX: u64 = 0xffff_ffff;
/// `MT_RAND_N`.
pub const MT_RAND_N: usize = 624;

/// Period parameters.
const M: usize = 397;
const MATRIX_A: u64 = 0x9908_b0df;
const UPPER_MASK: u64 = 0x8000_0000;
const LOWER_MASK: u64 = 0x7fff_ffff;

/// `MtState`. `unsigned long` is `u64` (rule R4).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MtState {
    #[serde(with = "crate::serde_big::boxed")]
    pub x: Box<[u64; MT_RAND_N]>,
    pub p0: usize,
    pub p1: usize,
    pub pm: usize,
}

impl Default for MtState {
    fn default() -> Self {
        MtState {
            x: Box::new([0; MT_RAND_N]),
            p0: 0,
            p1: 0,
            pm: 0,
        }
    }
}

/// Mutable globals of `mtrand.c` (rule R3).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub mt_global: MtState,
}

pub fn mt_srand_r(mt: &mut MtState, s: u64) {
    mt.x[0] = s & 0xffff_ffff;
    for i in 1..MT_RAND_N {
        let prev = mt.x[i - 1];
        mt.x[i] = 1_812_433_253u64
            .wrapping_mul(prev ^ (prev >> 30))
            .wrapping_add(i as u64)
            & 0xffff_ffff;
    }
    mt.p0 = 0;
    mt.p1 = 1;
    mt.pm = M;
}

/// Generates a random number on the interval [0, 0xffffffff].
pub fn mt_rand_r(mt: &mut MtState) -> u64 {
    // Twisted feedback.
    let x1 = mt.x[mt.p1];
    let mut y = mt.x[mt.pm]
        ^ (((mt.x[mt.p0] & UPPER_MASK) | (x1 & LOWER_MASK)) >> 1)
        ^ ((!(x1 & 1)).wrapping_add(1) & MATRIX_A);
    mt.x[mt.p0] = y;
    mt.p0 = mt.p1;
    mt.p1 += 1;
    mt.pm += 1;
    if mt.pm == MT_RAND_N {
        mt.pm = 0;
    }
    if mt.p1 == MT_RAND_N {
        mt.p1 = 0;
    }
    // Temper.
    y ^= y >> 11;
    y ^= (y << 7) & 0x9d2c_5680;
    y ^= (y << 15) & 0xefc6_0000;
    y ^= y >> 18;
    y
}

/// Generates a random number on the interval [0, 1].
pub fn mt_rand_1_r(mt: &mut MtState) -> f32 {
    mt_rand_r(mt) as f32 / MT_RAND_MAX as f32
}

/// Generates a random number on the interval [0, 1).
pub fn mt_rand_lt1_r(mt: &mut MtState) -> f32 {
    // MT_RAND_MAX must be a float before adding one to it!
    mt_rand_r(mt) as f32 / (MT_RAND_MAX as f32 + 1.0)
}

pub fn mt_srand(g: &mut Game, s: u64) {
    mt_srand_r(&mut g.mtrand.mt_global, s);
}

pub fn mt_rand(g: &mut Game) -> u64 {
    mt_rand_r(&mut g.mtrand.mt_global)
}

pub fn mt_rand_1(g: &mut Game) -> f32 {
    mt_rand_1_r(&mut g.mtrand.mt_global)
}

pub fn mt_rand_lt1(g: &mut Game) -> f32 {
    mt_rand_lt1_r(&mut g.mtrand.mt_global)
}
