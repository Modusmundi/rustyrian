//! Differential tests for `swar.h` (no mutable globals).

use rustyrian_core::swar::{SWAR8_LANES, State, Swar8};
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let s = State::default();
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}

#[test]
fn lanes_match_swar8_width() {
    assert_eq!(SWAR8_LANES, size_of::<Swar8>());
}

proptest::proptest! {
    #[test]
    fn swar8_read_matches(src in proptest::prelude::any::<[u8; 8]>()) {
        // SAFETY: `src` holds SWAR8_LANES bytes.
        let c = unsafe { rustyrian_oracle::bindings::ot_i_swar_swar8Read(src.as_ptr()) };
        proptest::prop_assert_eq!(c, rustyrian_core::swar::swar8_read(&src));
    }

    #[test]
    fn swar8_write_matches(dst in proptest::prelude::any::<[u8; 10]>(), v: u64, at in 0usize..3) {
        let (mut c, mut r) = (dst, dst);
        // SAFETY: `c[at..]` holds at least SWAR8_LANES bytes.
        unsafe { rustyrian_oracle::bindings::ot_i_swar_swar8Write(c[at..].as_mut_ptr(), v) };
        rustyrian_core::swar::swar8_write(&mut r[at..], v);
        proptest::prop_assert_eq!(c, r);
    }
}

#[test]
fn swar8_replicates() {
    assert_eq!(rustyrian_core::swar::swar8(0x7F), 0x7F7F_7F7F_7F7F_7F7F);
}
