//! Differential tests for `opentyr.c` / `opentyr.h`.

use proptest::prelude::*;
use rustyrian_core::opentyr::{self, State};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;
use std::ffi::{CStr, c_char};

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let s = State {
        opentyrian_str: b"Other".to_vec(),
        opentyrian_version: b"v9.9".to_vec(),
    };
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
    State::default().store_to_c();
    assert_eq!(diff::load::<State>(), State::default());
}

/// The display picker's item count, with the display query succeeding and
/// failing (SDL returns a negative count).
#[test]
fn display_picker_items_count_matches() {
    for fail in [false, true] {
        let mut d = Diff::new();
        if fail {
            d.fail_after("SDL_GetNumVideoDisplays", 0);
        }
        // SAFETY: plain C call.
        let c = unsafe { b::ot_s_opentyr_getDisplayPickerItemsCount() };
        assert_eq!(
            c as usize,
            opentyr::get_display_picker_items_count(&mut d.game)
        );
        assert_eq!(d.c_calls(), d.rust_calls());
    }
}

/// C's picker item `i` into a `len`-byte buffer: the returned text and the
/// buffer.
fn c_item(
    f: unsafe extern "C" fn(u64, *mut c_char, u64) -> *const c_char,
    i: usize,
    len: usize,
) -> (Vec<u8>, Vec<u8>) {
    let mut buffer = vec![0x55 as c_char; len];
    // SAFETY: `buffer` holds `len` bytes; the result is a NUL-terminated
    // string (a literal or `buffer`).
    let text = unsafe {
        let p = f(i as u64, buffer.as_mut_ptr(), len as u64);
        CStr::from_ptr(p).to_bytes().to_vec()
    };
    (text, buffer.into_iter().map(|c| c as u8).collect())
}

proptest! {
    /// Display names ("Window", "Display N") formatted into buffers of any
    /// size, truncated as `snprintf` does.
    #[test]
    fn display_picker_item_matches(i in prop_oneof![0usize..10, any::<u32>().prop_map(|v| v as usize)], len in 1usize..24) {
        let _d = Diff::new();
        let (c_text, c_buffer) = c_item(b::ot_s_opentyr_getDisplayPickerItem, i, len);
        let mut buffer = vec![0x55u8; len];
        let r_text = opentyr::get_display_picker_item(i, &mut buffer);
        prop_assert_eq!(c_text, r_text);
        prop_assert_eq!(c_buffer, buffer);
    }
}

/// Every scaler and scaling-mode name, and both counts.
#[test]
fn scaler_and_scaling_mode_pickers_match() {
    let _d = Diff::new();
    // SAFETY: plain C calls.
    let (scalers, modes) = unsafe {
        (
            b::ot_s_opentyr_getScalerPickerItemsCount(),
            b::ot_s_opentyr_getScalingModePickerItemsCount(),
        )
    };
    assert_eq!(scalers as usize, opentyr::get_scaler_picker_items_count());
    assert_eq!(
        modes as usize,
        opentyr::get_scaling_mode_picker_items_count()
    );
    for i in 0..scalers as usize {
        let (c, _) = c_item(b::ot_s_opentyr_getScalerPickerItem, i, 4);
        assert_eq!(c, opentyr::get_scaler_picker_item(i, &mut [0; 4]));
    }
    for i in 0..modes as usize {
        let (c, _) = c_item(b::ot_s_opentyr_getScalingModePickerItem, i, 4);
        assert_eq!(c, opentyr::get_scaling_mode_picker_item(i, &mut [0; 4]));
    }
}
