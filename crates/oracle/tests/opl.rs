//! Differential tests for `opl.c` / `opl.h`. The globals live in `OplState`
//! inside `loudness::AudioState` (see `rustyrian_core::loudness`).

use proptest::prelude::*;
use rustyrian_core::opl::{
    self, Bitu, MAXOPERATORS, OF_TYPE_OFF, OP_ACT_OFF, OpType, OplState, State, VibvalTable,
    WAVEFORM, WAVEMASK,
};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::opl::{opl_from_c, opl_to_c};
use rustyrian_oracle::session;
use std::ffi::c_void;

// Non-static `opl.c` functions that no header declares.
unsafe extern "C" {
    fn operator_advance(op_pt: *mut c_void, vib: i32);
    fn operator_output(op_pt: *mut c_void, modulator: i32, trem: i32);
    fn operator_off(op_pt: *mut c_void);
    fn operator_sustain(op_pt: *mut c_void);
    fn operator_release(op_pt: *mut c_void);
    fn operator_decay(op_pt: *mut c_void);
    fn operator_attack(op_pt: *mut c_void);
    fn change_attackrate(regbase: Bitu, op_pt: *mut c_void);
    fn change_decayrate(regbase: Bitu, op_pt: *mut c_void);
    fn change_releaserate(regbase: Bitu, op_pt: *mut c_void);
    fn change_sustainlevel(regbase: Bitu, op_pt: *mut c_void);
    fn change_waveform(regbase: Bitu, op_pt: *mut c_void);
    fn change_keepsustain(regbase: Bitu, op_pt: *mut c_void);
    fn change_vibrato(regbase: Bitu, op_pt: *mut c_void);
    fn change_feedback(chanbase: Bitu, op_pt: *mut c_void);
    fn change_frequency(chanbase: Bitu, regbase: Bitu, op_pt: *mut c_void);
    fn enable_operator(regbase: Bitu, op_pt: *mut c_void, act_type: u32);
    fn disable_operator(op_pt: *mut c_void, act_type: u32);
}

/// `&op[0]` in C.
fn c_op0() -> *mut c_void {
    // SAFETY: the getter returns the address of `op`.
    unsafe { b::ot_sv_opl_op() }
}

fn c_init(rate: u32) {
    // SAFETY: plain C call on the test thread.
    unsafe { b::adlib_init(rate) };
}

fn c_write(reg: u8, val: u8) {
    // SAFETY: plain C call on the test thread.
    unsafe { b::adlib_write(Bitu::from(reg), val) };
}

/// A Rust `OplState` after `adlib_init(rate)`, with C in the same state.
/// Proptest runs many cases in one process, so C's globals are first reset
/// to the Rust initial state.
fn both_init(rate: u32) -> (Diff, OplState) {
    let d = Diff::new();
    let mut s = OplState::default();
    opl_to_c(&s);
    session::set_crand_state(d.game.crand.lock().unwrap().clone());
    c_init(rate);
    opl::adlib_init(&mut s, rate);
    (d, s)
}

const RATES: [u32; 5] = [11025, 22050, 44100, 48000, 8000];

/// Register writes biased towards the interesting ranges (key-on, rhythm
/// mode, wave select enable).
fn write_seq(max: usize) -> impl Strategy<Value = Vec<(u8, u8)>> {
    let reg = prop_oneof![
        any::<u8>(),
        Just(0x01u8),
        Just(0x08),
        Just(0x04),
        Just(0xbd),
        (0xb0u8..0xb9),
        (0xa0u8..0xa9),
        (0x20u8..0x36),
        (0x40u8..0x56),
        (0x60u8..0x76),
        (0x80u8..0x96),
        (0xc0u8..0xc9),
        (0xe0u8..0xf6),
    ];
    prop::collection::vec((reg, any::<u8>()), 0..max)
}

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
    c_init(44100);
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

proptest! {
    /// `adlib_init` at several rates, twice (the second call skips the
    /// one-time tables), from a dirty chip.
    #[test]
    fn adlib_init_matches(r1 in prop::sample::select(&RATES[..]), r2 in 4000u32..100_000, writes in write_seq(40)) {
        let (_d, mut s) = both_init(r1);
        prop_assert_eq!(opl_from_c(), s.clone());
        for &(reg, val) in &writes {
            c_write(reg, val);
            opl::adlib_write(&mut s, Bitu::from(reg), val);
        }
        c_init(r2);
        opl::adlib_init(&mut s, r2);
        prop_assert_eq!(opl_from_c(), s);
    }

    #[test]
    fn adlib_write_sequences_match(rate in prop::sample::select(&RATES[..]), writes in write_seq(300)) {
        let (_d, mut s) = both_init(rate);
        for &(reg, val) in &writes {
            c_write(reg, val);
            opl::adlib_write(&mut s, Bitu::from(reg), val);
        }
        prop_assert_eq!(opl_from_c(), s);
    }

    /// Random register programs, then samples: output, chip state and the
    /// libc `rand()` stream (rhythm-mode noise) all match.
    #[test]
    fn adlib_getsample_matches(
        rate in prop::sample::select(&RATES[..]),
        rounds in prop::collection::vec((write_seq(60), 0isize..1500), 1..5),
    ) {
        let (d, mut s) = both_init(rate);
        for (writes, n) in &rounds {
            for &(reg, val) in writes {
                c_write(reg, val);
                opl::adlib_write(&mut s, Bitu::from(reg), val);
            }
            let mut c_out = vec![0i16; *n as usize];
            // SAFETY: `c_out` holds `n` samples.
            unsafe { b::adlib_getsample(c_out.as_mut_ptr(), *n) };
            let mut r_out = vec![0i16; *n as usize];
            opl::adlib_getsample(&mut s, &d.game.crand, &mut r_out, *n);
            prop_assert_eq!(c_out, r_out);
            prop_assert_eq!(opl_from_c(), s.clone());
            prop_assert_eq!(session::crand_state(), d.game.crand.lock().unwrap().clone());
        }
    }

    #[test]
    fn adlib_reg_read_and_write_index_match(status: u8, port: Bitu, val: u8) {
        let _d = Diff::new();
        let s = OplState { status, ..OplState::default() };
        opl_to_c(&s);
        // SAFETY: plain C calls on the test thread.
        let c = unsafe { b::adlib_reg_read(port) };
        prop_assert_eq!(c, opl::adlib_reg_read(&s, port));
        let mut s = s;
        // SAFETY: as above.
        unsafe { b::adlib_write_index(port, val) };
        opl::adlib_write_index(&mut s, port, val);
        prop_assert_eq!(opl_from_c(), s);
    }

    #[test]
    fn clipit16_matches(ival: i32, near in -40_000i32..40_000) {
        for v in [ival, near] {
            let mut c = 0i16;
            // SAFETY: `c` is a valid out-pointer.
            unsafe { b::ot_s_opl_clipit16(v, &raw mut c) };
            let mut r = 0i16;
            opl::clipit16(v, &mut r);
            prop_assert_eq!(c, r);
        }
    }
}

/// Any operator state the emulator can hold (envelope coefficients from
/// realistic ranges; waveform pointers inside `wavtable`).
fn any_op() -> impl Strategy<Value = OpType> {
    let ints = (
        any::<i32>(),
        any::<i32>(),
        any::<u32>(),
        any::<u32>(),
        0u32..0x0100_0000,
        -20i32..20,
    );
    let amps = (
        0.0f64..1.2,
        0.0f64..1.2,
        1e-7f64..1e-4,
        0.0f64..1.0,
        0i32..0x8000,
    );
    let coef = (
        -1.0f64..2.5,
        -1.0f64..2.5,
        -1.0f64..1.0,
        -1.0f64..1.0,
        0.9f64..1.0,
        0.9f64..1.0,
    );
    let state = (
        0u32..6,
        0u32..16,
        0i32..8,
        0usize..8,
        0u32..3,
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
    );
    let env = (
        0u32..0x40000,
        any::<i32>(),
        0isize..4096,
        0isize..4096,
        0isize..4096,
        any::<u8>(),
        prop::sample::select(&[0isize, 0xff, 0xfe, 0xee, 0xba, 0xaa][..]),
    );
    (ints, amps, coef, state, env).prop_map(|(i, a, c, st, e)| OpType {
        cval: i.0,
        lastcval: i.1,
        tcount: i.2,
        wfpos: i.3,
        tinc: i.4,
        amp: a.0,
        step_amp: a.1,
        vol: a.2,
        sustain_level: a.3,
        mfbi: a.4,
        a0: c.0,
        a1: c.1,
        a2: c.2,
        a3: c.3,
        decaymul: c.4,
        releasemul: c.5,
        op_state: st.0,
        toff: st.1,
        freq_high: st.2,
        cur_wform: Some(WAVEFORM[st.3 % 4] as usize),
        cur_wmask: WAVEMASK[st.3 % 4],
        act_state: st.4,
        sus_keep: st.5,
        vibrato: st.6,
        tremolo: st.7,
        generator_pos: e.0,
        cur_env_step: e.1 as isize,
        env_step_a: e.2 & 0xfff,
        env_step_d: e.3 & 0xfff,
        env_step_r: e.4 & 0xfff,
        step_skip_pos_a: e.5,
        env_step_skip_a: e.6,
    })
}

/// Runs `c` on C's `op[0]` and `r` on Rust's, from state `s` with `op[0]`
/// replaced by `o`; compares the whole chip state.
fn op_case(
    mut s: OplState,
    o: OpType,
    c: impl FnOnce(*mut c_void),
    r: impl FnOnce(&mut OplState),
) -> Result<(), TestCaseError> {
    s.op[0] = o;
    opl_to_c(&s);
    c(c_op0());
    r(&mut s);
    prop_assert_eq!(opl_from_c(), s);
    Ok(())
}

/// Calls a C operator function from an `op_case` closure.
macro_rules! c_op {
    ($call:expr) => {
        // SAFETY: the closure's argument is `&op[0]` in C and every base
        // argument indexes the register file in bounds.
        unsafe { $call }
    };
}

proptest! {
    #[test]
    fn envelope_functions_match(o in any_op(), which in 0usize..5) {
        let (_d, s) = both_init(44100);
        match which {
            0 => op_case(s, o, |p| c_op!(operator_attack(p)), |s| opl::operator_attack(&mut s.op[0]))?,
            1 => op_case(s, o, |p| c_op!(operator_decay(p)), |s| opl::operator_decay(&mut s.op[0]))?,
            2 => op_case(s, o, |p| c_op!(operator_release(p)), |s| opl::operator_release(&mut s.op[0]))?,
            3 => op_case(s, o, |p| c_op!(operator_sustain(p)), |s| opl::operator_sustain(&mut s.op[0]))?,
            _ => op_case(s, o, |p| c_op!(operator_off(p)), |s| opl::operator_off(&mut s.op[0]))?,
        }
    }

    #[test]
    fn operator_advance_and_output_match(o in any_op(), vib in -1000i32..1000, modulator: i32, trem in 0i32..0x10001) {
        let (_d, s) = both_init(44100);
        op_case(s.clone(), o, |p| c_op!(operator_advance(p, vib)), |s| opl::operator_advance(s, 0, vib))?;
        op_case(s, o, |p| c_op!(operator_output(p, modulator, trem)), |s| opl::operator_output(s, 0, modulator, trem))?;
    }

    /// Every `change_*` function and operator enable/disable, for register
    /// bases of operator 0's set, over random register contents.
    #[test]
    fn change_functions_match(
        o in any_op(),
        regs in prop::collection::vec(any::<u8>(), 256),
        wave_sel in prop::collection::vec(0u8..4, 22),
        regbase in 0usize..22,
        chanbase in 0usize..9,
        act_type in 0u32..4,
        which in 0usize..11,
    ) {
        let (_d, mut s) = both_init(44100);
        s.adlibreg.copy_from_slice(&regs);
        s.wave_sel.copy_from_slice(&wave_sel);
        match which {
            0 => op_case(s, o, |p| c_op!(change_attackrate(regbase, p)), |s| opl::change_attackrate(s, regbase, 0))?,
            1 => op_case(s, o, |p| c_op!(change_decayrate(regbase, p)), |s| opl::change_decayrate(s, regbase, 0))?,
            2 => op_case(s, o, |p| c_op!(change_releaserate(regbase, p)), |s| opl::change_releaserate(s, regbase, 0))?,
            3 => op_case(s, o, |p| c_op!(change_sustainlevel(regbase, p)), |s| opl::change_sustainlevel(s, regbase, 0))?,
            4 => op_case(s, o, |p| c_op!(change_waveform(regbase, p)), |s| opl::change_waveform(s, regbase, 0))?,
            5 => op_case(s, o, |p| c_op!(change_keepsustain(regbase, p)), |s| opl::change_keepsustain(s, regbase, 0))?,
            6 => op_case(s, o, |p| c_op!(change_vibrato(regbase, p)), |s| opl::change_vibrato(s, regbase, 0))?,
            7 => op_case(s, o, |p| c_op!(change_feedback(chanbase, p)), |s| opl::change_feedback(s, chanbase, 0))?,
            8 => op_case(s, o, |p| c_op!(change_frequency(chanbase, regbase, p)), |s| opl::change_frequency(s, chanbase, regbase, 0))?,
            9 => op_case(s, o, |p| c_op!(enable_operator(regbase, p, act_type)), |s| opl::enable_operator(s, regbase, 0, act_type))?,
            _ => op_case(s, o, |p| c_op!(disable_operator(p, act_type)), |s| opl::disable_operator(s, 0, act_type))?,
        }
    }
}
