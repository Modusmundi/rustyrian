//! Differential tests for `mtrand.c` / `mtrand.h`.

use proptest::prelude::*;
use rustyrian_core::mtrand::{self, MT_RAND_N, MtState};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::mtrand::{mt_from_c, mt_to_c};
use std::ffi::c_ulong;

/// Any reachable generator state: 32-bit words, `p1`/`pm` derived from `p0`.
fn any_state() -> impl Strategy<Value = MtState> {
    (prop::collection::vec(any::<u32>(), MT_RAND_N), 0..MT_RAND_N).prop_map(|(x, p0)| {
        let mut s = MtState::default();
        for (d, v) in s.x.iter_mut().zip(x) {
            *d = u64::from(v);
        }
        s.p0 = p0;
        s.p1 = (p0 + 1) % MT_RAND_N;
        s.pm = (p0 + 397) % MT_RAND_N;
        s
    })
}

proptest! {
    #[test]
    fn mt_srand_then_10000_mt_rand_match(seed: u64) {
        let mut d = Diff::new();
        // SAFETY: plain C calls on the test thread.
        unsafe { b::ot_i_mtrand_mt_srand(seed as c_ulong) };
        mtrand::mt_srand(&mut d.game, seed);
        prop_assert_eq!(&diff::load::<mtrand::State>(), &d.game.mtrand);
        for _ in 0..10_000 {
            // SAFETY: as above.
            let c = unsafe { b::ot_i_mtrand_mt_rand() };
            prop_assert_eq!(c, mtrand::mt_rand(&mut d.game));
        }
        prop_assert_eq!(&diff::load::<mtrand::State>(), &d.game.mtrand);
        prop_assert!(d.c_calls().is_empty() && d.rust_calls().is_empty());
    }

    #[test]
    fn mt_rand_1_and_lt1_match_bitwise(seed: u64) {
        let mut d = Diff::new();
        // SAFETY: plain C calls on the test thread.
        unsafe { b::ot_i_mtrand_mt_srand(seed as c_ulong) };
        mtrand::mt_srand(&mut d.game, seed);
        for _ in 0..1000 {
            // SAFETY: as above.
            let (c1, c2) = unsafe { (b::ot_i_mtrand_mt_rand_1(), b::ot_i_mtrand_mt_rand_lt1()) };
            prop_assert_eq!(c1.to_bits(), mtrand::mt_rand_1(&mut d.game).to_bits());
            prop_assert_eq!(c2.to_bits(), mtrand::mt_rand_lt1(&mut d.game).to_bits());
        }
        prop_assert_eq!(&diff::load::<mtrand::State>(), &d.game.mtrand);
    }

    #[test]
    fn mt_srand_r_from_any_state(start in any_state(), seed: u64) {
        let mut c = b::MtState::default();
        mt_to_c(&start, &mut c);
        // SAFETY: `c` is a valid MtState.
        unsafe { b::mt_srand_r(&raw mut c, seed as c_ulong) };
        let mut r = start;
        mtrand::mt_srand_r(&mut r, seed);
        prop_assert_eq!(mt_from_c(&c), r);
    }

    #[test]
    fn mt_rand_r_variants_from_any_state(start in any_state()) {
        let mut c = b::MtState::default();
        mt_to_c(&start, &mut c);
        let mut r = start;
        for _ in 0..(2 * MT_RAND_N) {
            // SAFETY: `c` is a valid MtState.
            let (a, f1, f2) = unsafe {
                (b::mt_rand_r(&raw mut c), b::mt_rand_1_r(&raw mut c), b::mt_rand_lt1_r(&raw mut c))
            };
            prop_assert_eq!(a, mtrand::mt_rand_r(&mut r));
            prop_assert_eq!(f1.to_bits(), mtrand::mt_rand_1_r(&mut r).to_bits());
            prop_assert_eq!(f2.to_bits(), mtrand::mt_rand_lt1_r(&mut r).to_bits());
        }
        prop_assert_eq!(mt_from_c(&c), r);
    }
}
