//! Differential tests for `opl.c` / `opl.h`. The globals live in `OplState`
//! inside `loudness::AudioState` (see `rustyrian_core::loudness`).

use rustyrian_core::opl::{
    MAXOPERATORS, OF_TYPE_OFF, OP_ACT_OFF, OplState, State, VibvalTable, WAVEFORM, WAVEMASK,
};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::opl::{opl_from_c, opl_to_c};

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(opl_from_c(), OplState::default());
    assert_eq!(diff::load::<State>(), State::default());
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let mut s = OplState {
        int_samplerate: 22050,
        recipsamp: 1.0 / 22050.0,
        vibval1: VibvalTable::Const,
        vibval4: VibvalTable::Var2,
        adlib_init_initfirstime: 1,
        ..OplState::default()
    };
    s.op[17].cur_wform = Some(3071);
    s.op[0].cur_wform = Some(0);
    s.op[5].a3 = -0.125;
    s.wavtable[3071] = -32768;
    opl_to_c(&s);
    assert_eq!(opl_from_c(), s);
}

/// Checks the hand-declared `op_type` layout against values C writes itself.
#[test]
fn adlib_init_state_loads_as_expected() {
    let _d = Diff::new();
    // SAFETY: plain C call on the test thread.
    unsafe { b::adlib_init(44100) };
    let s = opl_from_c();
    assert_eq!(s.int_samplerate, 44100);
    assert_eq!(s.adlib_init_initfirstime, 1);
    for op in &s.op {
        assert_eq!(op.op_state, OF_TYPE_OFF);
        assert_eq!(op.act_state, OP_ACT_OFF);
        assert_eq!(op.cur_wmask, WAVEMASK[0]);
        assert_eq!(op.cur_wform, Some(WAVEFORM[0] as usize));
        assert_eq!(op.freq_high, 0);
        assert_eq!(op.env_step_skip_a, 0);
    }
    assert_eq!(s.op.len(), MAXOPERATORS);
    assert!(s.recipsamp > 0.0 && s.wavtable.iter().any(|&w| w != 0));
    // A round trip of the C-built state is exact.
    opl_to_c(&s);
    assert_eq!(opl_from_c(), s);
}
