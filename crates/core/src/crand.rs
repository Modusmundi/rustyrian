// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) rustyrian contributors

//! The libc `rand()` used by upstream (`opl.c`, `destruct.c`, `xmas.c`),
//! fixed to the ANSI C example LCG so C and Rust draw identical streams on
//! every OS (rule R7). The oracle's `rand` shim uses this same function.

use serde::{Deserialize, Serialize};

/// `RAND_MAX` for this generator.
pub const RAND_MAX: i32 = 32767;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    pub next: u32,
}

impl Default for State {
    fn default() -> Self {
        // As if `srand(1)`, the C default.
        State { next: 1 }
    }
}

/// `next = next * 1103515245 + 12345; return (next / 65536) % 32768`.
pub fn rand(s: &mut State) -> i32 {
    s.next = s.next.wrapping_mul(1_103_515_245).wrapping_add(12_345);
    ((s.next / 65_536) % 32_768) as i32
}

/// `rand()` on the shared stream (`Game::crand`), holding the lock only for
/// the draw.
pub fn rand_shared(s: &std::sync::Mutex<State>) -> i32 {
    rand(&mut s.lock().unwrap_or_else(|e| e.into_inner()))
}

#[cfg(test)]
mod tests {
    #[test]
    fn matches_the_ansi_example_sequence() {
        let mut s = super::State::default();
        let first: Vec<i32> = (0..5).map(|_| super::rand(&mut s)).collect();
        assert_eq!(first, [16838, 5758, 10113, 17515, 31051]);
    }
}
