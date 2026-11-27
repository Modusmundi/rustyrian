//! Differential tests for `video_scale.c` / `video_scale.h`.

use rustyrian_core::video_scale::{SCALERS, SCALERS_COUNT, State};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;
use std::ffi::CStr;

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let s = State { scaler: 7 };
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}

#[test]
fn scalers_table_matches_c() {
    // SAFETY: C constants.
    let (n, c) = unsafe {
        let n = b::scalers_count;
        (
            n,
            std::slice::from_raw_parts((&raw const b::scalers).cast::<b::Scalers>(), n as usize),
        )
    };
    assert_eq!(n, SCALERS_COUNT);
    for (r, c) in SCALERS.iter().zip(c) {
        assert_eq!((r.width, r.height), (c.width, c.height));
        assert_eq!(r.scaler16.is_some(), c.scaler16.is_some());
        assert_eq!(r.scaler32.is_some(), c.scaler32.is_some());
        // SAFETY: C string constant.
        assert_eq!(r.name, unsafe { CStr::from_ptr(c.name) }.to_str().unwrap());
    }
}
