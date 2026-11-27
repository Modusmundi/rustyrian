// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2002-2010 The DOSBox Team
// Copyright (C) 1998-2001 Ken Silverman
// Copyright (C) rustyrian contributors

//! Port of upstream `opl.c` / `opl.h`: the OPL2 emulator (DOSBox, originally
//! based on Ken Silverman's ADLIBEMU.C). Only the OPL2 configuration is ported
//! (`OPLTYPE_IS_OPL3` is undefined upstream; rule R11).
//!
//! Every global of `opl.c` is used by `adlib_getsample`, which the audio
//! callback calls, so they all live in [`OplState`], owned by
//! `loudness::AudioState` behind `Game::audio` (see `loudness.rs`). `State`
//! is empty.

use serde::{Deserialize, Serialize};

/// `Bitu` (`uintptr_t`).
pub type Bitu = usize;
/// `Bits` (`intptr_t`).
pub type Bits = isize;

/// `fltype`.
pub type Fltype = f64;

pub const NUM_CHANNELS: usize = 9;
pub const MAXOPERATORS: usize = NUM_CHANNELS * 2;

pub const FL05: Fltype = 0.5;
pub const FL2: Fltype = 2.0;
/// `PI` (`3.1415926535897932384626433832795` rounds to exactly this double).
pub const PI: Fltype = std::f64::consts::PI;

/// Fixed-point calculations using 16+16.
pub const FIXEDPT: i32 = 0x10000;
/// Fixed-point calculations using 8+24.
pub const FIXEDPT_LFO: i32 = 0x1000000;

/// Waveform precision (10 bits).
pub const WAVEPREC: u32 = 1024;

/// Clocking of the chip.
pub const INTFREQU: Fltype = 14318180.0 / 288.0;

pub const OF_TYPE_ATT: u32 = 0;
pub const OF_TYPE_DEC: u32 = 1;
pub const OF_TYPE_REL: u32 = 2;
pub const OF_TYPE_SUS: u32 = 3;
pub const OF_TYPE_SUS_NOKEEP: u32 = 4;
pub const OF_TYPE_OFF: u32 = 5;

pub const ARC_CONTROL: Bitu = 0x00;
pub const ARC_TVS_KSR_MUL: Bitu = 0x20;
pub const ARC_KSL_OUTLEV: Bitu = 0x40;
pub const ARC_ATTR_DECR: Bitu = 0x60;
pub const ARC_SUSL_RELR: Bitu = 0x80;
pub const ARC_FREQ_NUM: Bitu = 0xa0;
pub const ARC_KON_BNUM: Bitu = 0xb0;
pub const ARC_PERC_MODE: Bitu = 0xbd;
pub const ARC_FEEDBACK: Bitu = 0xc0;
pub const ARC_WAVE_SEL: Bitu = 0xe0;

/// Second operator set for OPL3.
pub const ARC_SECONDSET: Bitu = 0x100;
pub const ARC_SECONDSET_MASK: Bitu = 0;

pub const OP_ACT_OFF: u32 = 0x00;
/// Regular channel activated (bitmasked).
pub const OP_ACT_NORMAL: u32 = 0x01;
/// Percussion channel activated (bitmasked).
pub const OP_ACT_PERC: u32 = 0x02;

pub const BLOCKBUF_SIZE: usize = 512;

pub const VIBTAB_SIZE: usize = 8;
// `VIBFAC` is `70/50000` without braces: integer multiply, then divide.
pub const VIBFAC_MUL: i32 = 70;
pub const VIBFAC_DIV: i32 = 50000;

pub const TREMTAB_SIZE: usize = 53;
/// Tremolo at 3.7 Hz.
pub const TREM_FREQ: Fltype = 3.7;

/// `op_type` (`struct operator_struct`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct OpType {
    /// Current output.
    pub cval: i32,
    /// Last output (used for feedback).
    pub lastcval: i32,
    /// Time (position in waveform).
    pub tcount: u32,
    pub wfpos: u32,
    /// Time increment.
    pub tinc: u32,
    /// Amplification (envelope).
    pub amp: Fltype,
    pub step_amp: Fltype,
    pub vol: Fltype,
    pub sustain_level: Fltype,
    /// Feedback amount.
    pub mfbi: i32,
    /// Attack rate function coefficients.
    pub a0: Fltype,
    pub a1: Fltype,
    pub a2: Fltype,
    pub a3: Fltype,
    /// Decay/release rate functions.
    pub decaymul: Fltype,
    pub releasemul: Fltype,
    /// Current state of operator (attack/decay/sustain/release/off).
    pub op_state: u32,
    pub toff: u32,
    /// Highest three bits of the frequency, used for vibrato calculations.
    pub freq_high: i32,
    /// `Bit16s *cur_wform`, start of selected waveform: an index into
    /// `OplState::wavtable` (rule R8); `None` is NULL.
    pub cur_wform: Option<usize>,
    /// Mask for selected waveform.
    pub cur_wmask: u32,
    /// Activity state (regular, percussion).
    pub act_state: u32,
    /// Keep sustain level when decay finished.
    pub sus_keep: bool,
    pub vibrato: bool,
    pub tremolo: bool,
    /// For non-standard sample rates we need to determine how many samples
    /// have passed.
    pub generator_pos: u32,
    /// Current (standardized) sample position.
    pub cur_env_step: Bits,
    /// Number of std samples of one step (attack/decay/release mode).
    pub env_step_a: Bits,
    pub env_step_d: Bits,
    pub env_step_r: Bits,
    /// Position of 8-cyclic step skipping (always 2^x to check against mask).
    pub step_skip_pos_a: u8,
    /// Bitmask that determines if a step is skipped (bit zero then).
    pub env_step_skip_a: Bits,
}

/// Target of the `vibval1`..`vibval4` table pointers (rule R8).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum VibvalTable {
    /// NULL.
    #[default]
    Null,
    /// `vibval_const`.
    Const,
    /// `vibval_var1`.
    Var1,
    /// `vibval_var2`.
    Var2,
}

/// `optype_fptr` targets (rule R8).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpFunc {
    OperatorAttack,
    OperatorDecay,
    OperatorRelease,
    OperatorSustain,
    OperatorOff,
}

/// `opfuncs`, indexed by `op_state`.
pub const OPFUNCS: [OpFunc; 6] = [
    OpFunc::OperatorAttack,
    OpFunc::OperatorDecay,
    OpFunc::OperatorRelease,
    // sustain phase (keeping level)
    OpFunc::OperatorSustain,
    // sustain_nokeep phase (release-style)
    OpFunc::OperatorRelease,
    OpFunc::OperatorOff,
];

/// Key scale level lookup table (-> 0, 3, 1.5, 6 dB/oct).
pub const KSLMUL: [Fltype; 4] = [0.0, 0.5, 0.25, 1.0];

/// Frequency multiplicator lookup table.
pub const FRQMUL_TAB: [Fltype; 16] = [
    0.5, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 10.0, 12.0, 12.0, 15.0, 15.0,
];

/// Map a channel number to the register offset of the modulator (= register
/// base).
pub const MODULATORBASE: [u8; 9] = [0, 1, 2, 8, 9, 10, 16, 17, 18];

/// Map a register base to a modulator operator number.
pub const REGBASE2MODOP: [u8; 22] = [
    0, 1, 2, 0, 1, 2, 0, 0, 3, 4, 5, 3, 4, 5, 0, 0, 6, 7, 8, 6, 7, 8,
];
/// Map a register base to an operator number.
pub const REGBASE2OP: [u8; 22] = [
    0, 1, 2, 9, 10, 11, 0, 0, 3, 4, 5, 12, 13, 14, 0, 0, 6, 7, 8, 15, 16, 17,
];

/// Start of the waveform.
pub const WAVEFORM: [u32; 8] = [
    WAVEPREC,
    WAVEPREC >> 1,
    WAVEPREC,
    (WAVEPREC * 3) >> 2,
    0,
    0,
    (WAVEPREC * 5) >> 2,
    WAVEPREC << 1,
];

/// Length of the waveform as mask.
pub const WAVEMASK: [u32; 8] = [
    WAVEPREC - 1,
    WAVEPREC - 1,
    (WAVEPREC >> 1) - 1,
    (WAVEPREC >> 1) - 1,
    WAVEPREC - 1,
    ((WAVEPREC * 3) >> 2) - 1,
    WAVEPREC >> 1,
    WAVEPREC - 1,
];

/// Where the first entry resides.
pub const WAVESTART: [u32; 8] = [0, WAVEPREC >> 1, 0, WAVEPREC >> 2, 0, 0, 0, WAVEPREC >> 3];

/// Envelope generator function constants.
pub const ATTACKCONST: [Fltype; 4] = [1.0 / 2.82624, 1.0 / 2.25280, 1.0 / 1.88416, 1.0 / 1.59744];
pub const DECRELCONST: [Fltype; 4] = [
    1.0 / 39.28064,
    1.0 / 31.41608,
    1.0 / 26.17344,
    1.0 / 22.44608,
];

/// `wavtable`: the three waveforms.
pub type Wavtable = [i16; WAVEPREC as usize * 3];

/// Every global of `opl.c` (all file statics), plus `adlib_init`'s local
/// static `initfirstime`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OplState {
    pub op: [OpType; MAXOPERATORS],
    pub int_samplerate: Bits,
    pub status: u8,
    pub opl_index: u32,
    /// Adlib register set.
    #[serde(with = "crate::serde_big")]
    pub adlibreg: [u8; 256],
    /// Waveform selection.
    pub wave_sel: [u8; 22],
    pub vibtab_pos: u32,
    pub vibtab_add: u32,
    pub tremtab_pos: u32,
    pub tremtab_add: u32,
    /// Should be a chip parameter.
    pub generator_add: u32,
    /// Inverse of sampling rate.
    pub recipsamp: Fltype,
    /// Wave form table.
    #[serde(with = "crate::serde_big::boxed")]
    pub wavtable: Box<Wavtable>,
    pub vib_table: [i32; VIBTAB_SIZE],
    #[serde(with = "crate::serde_big")]
    pub trem_table: [i32; TREMTAB_SIZE * 2],
    #[serde(with = "crate::serde_big")]
    pub vibval_const: [i32; BLOCKBUF_SIZE],
    #[serde(with = "crate::serde_big")]
    pub tremval_const: [i32; BLOCKBUF_SIZE],
    #[serde(with = "crate::serde_big")]
    pub vibval_var1: [i32; BLOCKBUF_SIZE],
    #[serde(with = "crate::serde_big")]
    pub vibval_var2: [i32; BLOCKBUF_SIZE],
    pub vibval1: VibvalTable,
    pub vibval2: VibvalTable,
    pub vibval3: VibvalTable,
    pub vibval4: VibvalTable,
    /// Calculated frequency multiplication values (depend on sampling rate).
    pub frqmul: [Fltype; 16],
    /// Key scale levels.
    pub kslev: [[u8; 16]; 8],
    /// `adlib_init`'s local `static Bitu initfirstime`.
    pub adlib_init_initfirstime: Bitu,
}

impl Default for OplState {
    fn default() -> Self {
        OplState {
            op: [OpType::default(); MAXOPERATORS],
            int_samplerate: 0,
            status: 0,
            opl_index: 0,
            adlibreg: [0; 256],
            wave_sel: [0; 22],
            vibtab_pos: 0,
            vibtab_add: 0,
            tremtab_pos: 0,
            tremtab_add: 0,
            generator_add: 0,
            recipsamp: 0.0,
            wavtable: Box::new([0; WAVEPREC as usize * 3]),
            vib_table: [0; VIBTAB_SIZE],
            trem_table: [0; TREMTAB_SIZE * 2],
            vibval_const: [0; BLOCKBUF_SIZE],
            tremval_const: [0; BLOCKBUF_SIZE],
            vibval_var1: [0; BLOCKBUF_SIZE],
            vibval_var2: [0; BLOCKBUF_SIZE],
            vibval1: VibvalTable::Null,
            vibval2: VibvalTable::Null,
            vibval3: VibvalTable::Null,
            vibval4: VibvalTable::Null,
            frqmul: [0.0; 16],
            kslev: [[0; 16]; 8],
            adlib_init_initfirstime: 0,
        }
    }
}

/// Main-thread-only globals of `opl.c`: none (see the module docs).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {}

/// Runs the envelope function `opfuncs[op_pt.op_state]`.
#[inline(always)]
fn run_opfunc(op_pt: &mut OpType) {
    match OPFUNCS[op_pt.op_state as usize] {
        OpFunc::OperatorAttack => operator_attack(op_pt),
        OpFunc::OperatorDecay => operator_decay(op_pt),
        OpFunc::OperatorRelease => operator_release(op_pt),
        OpFunc::OperatorSustain => operator_sustain(op_pt),
        OpFunc::OperatorOff => operator_off(op_pt),
    }
}

/// `(Bit32s)(tinc)*vib/FIXEDPT`, added to `tcount` by the advance functions.
fn vib_step(tinc: u32, vib: i32) -> u32 {
    ((tinc as i32).wrapping_mul(vib) / FIXEDPT) as u32
}

/// `operator_advance(&op[op_pt], vib)`.
pub fn operator_advance(s: &mut OplState, op_pt: usize, vib: i32) {
    advance(&mut s.op[op_pt], s.generator_add, vib);
}

/// The body of `operator_advance` on one operator.
#[inline]
fn advance(o: &mut OpType, generator_add: u32, vib: i32) {
    o.wfpos = o.tcount;

    o.tcount = o.tcount.wrapping_add(o.tinc);
    o.tcount = o.tcount.wrapping_add(vib_step(o.tinc, vib));

    o.generator_pos = o.generator_pos.wrapping_add(generator_add);
}

/// `operator_advance_drums(&op[op_pt1], vib1, &op[op_pt2], vib2, &op[op_pt3],
/// vib3)`; `rand()` draws from the shared libc stream (rule R7).
#[allow(clippy::too_many_arguments)]
pub fn operator_advance_drums(
    s: &mut OplState,
    crand: &std::sync::Mutex<crate::crand::State>,
    op_pt1: usize,
    vib1: i32,
    op_pt2: usize,
    vib2: i32,
    op_pt3: usize,
    vib3: i32,
) {
    advance_drums(
        &mut s.op,
        s.generator_add,
        crand,
        [(op_pt1, vib1), (op_pt2, vib2), (op_pt3, vib3)],
    );
}

/// The body of `operator_advance_drums` on the operator array.
fn advance_drums(
    op: &mut [OpType; MAXOPERATORS],
    generator_add: u32,
    crand: &std::sync::Mutex<crate::crand::State>,
    [(op_pt1, vib1), (op_pt2, vib2), (op_pt3, vib3)]: [(usize, i32); 3],
) {
    const FX: u32 = FIXEDPT as u32;
    let c1 = op[op_pt1].tcount / FX;
    let c3 = op[op_pt3].tcount / FX;
    let phasebit: u32 = if (((c1 & 0x88) ^ ((c1 << 5) & 0x80)) | ((c3 ^ (c3 << 2)) & 0x20)) != 0 {
        0x02
    } else {
        0x00
    };

    let noisebit = (crate::crand::rand_shared(crand) & 1) as u32;

    let snare_phase_bit = ((op[op_pt1].tcount / FX) / 0x100) & 1;

    let advance = |o: &mut OpType, inttm: u32, vib: i32| {
        o.wfpos = inttm.wrapping_mul(FX);
        o.tcount = o.tcount.wrapping_add(o.tinc);
        o.tcount = o.tcount.wrapping_add(vib_step(o.tinc, vib));
        o.generator_pos = o.generator_pos.wrapping_add(generator_add);
    };

    // Hihat
    let inttm = (phasebit << 8) | (0x34 << (phasebit ^ (noisebit << 1)));
    advance(&mut op[op_pt1], inttm, vib1);

    // Snare
    let inttm = ((1 + snare_phase_bit) ^ noisebit) << 8;
    advance(&mut op[op_pt2], inttm, vib2);

    // Cymbal
    let inttm = (1 + phasebit) << 8;
    advance(&mut op[op_pt3], inttm, vib3);
}

/// `operator_output(&op[op_pt], modulator, trem)`.
pub fn operator_output(s: &mut OplState, op_pt: usize, modulator: i32, trem: i32) {
    output(&mut s.op[op_pt], &s.wavtable, modulator, trem);
}

/// The body of `operator_output` on one operator.
#[inline]
fn output(o: &mut OpType, wavtable: &Wavtable, modulator: i32, trem: i32) {
    if o.op_state != OF_TYPE_OFF {
        o.lastcval = o.cval;
        let i = o.wfpos.wrapping_add(modulator as u32) / FIXEDPT as u32;

        let wform = o.cur_wform.expect("adlib_init selects a waveform");
        let w = wavtable[wform + (i & o.cur_wmask) as usize];
        o.cval = (o.step_amp * o.vol * f64::from(w) * f64::from(trem) / 16.0) as i32;
    }
}

pub fn operator_off(op_pt: &mut OpType) {
    let _ = op_pt;
}

/// Number of standardised samples that passed, consumed from
/// `generator_pos` by the envelope functions.
fn env_steps(op_pt: &OpType) -> u32 {
    op_pt.generator_pos / FIXEDPT as u32
}

fn consume_env_steps(op_pt: &mut OpType, num_steps_add: u32) {
    op_pt.generator_pos = op_pt
        .generator_pos
        .wrapping_sub(num_steps_add.wrapping_mul(FIXEDPT as u32));
}

pub fn operator_sustain(op_pt: &mut OpType) {
    let num_steps_add = env_steps(op_pt);
    for _ in 0..num_steps_add {
        op_pt.cur_env_step = op_pt.cur_env_step.wrapping_add(1);
    }
    consume_env_steps(op_pt, num_steps_add);
}

pub fn operator_release(op_pt: &mut OpType) {
    if op_pt.amp > 0.00000001 {
        op_pt.amp *= op_pt.releasemul;
    }

    let num_steps_add = env_steps(op_pt);
    for _ in 0..num_steps_add {
        op_pt.cur_env_step = op_pt.cur_env_step.wrapping_add(1);
        if (op_pt.cur_env_step & op_pt.env_step_r) == 0 {
            if op_pt.amp <= 0.00000001 {
                op_pt.amp = 0.0;
                if op_pt.op_state == OF_TYPE_REL {
                    op_pt.op_state = OF_TYPE_OFF;
                }
            }
            op_pt.step_amp = op_pt.amp;
        }
    }
    consume_env_steps(op_pt, num_steps_add);
}

pub fn operator_decay(op_pt: &mut OpType) {
    if op_pt.amp > op_pt.sustain_level {
        op_pt.amp *= op_pt.decaymul;
    }

    let num_steps_add = env_steps(op_pt);
    for _ in 0..num_steps_add {
        op_pt.cur_env_step = op_pt.cur_env_step.wrapping_add(1);
        if (op_pt.cur_env_step & op_pt.env_step_d) == 0 {
            if op_pt.amp <= op_pt.sustain_level {
                if op_pt.sus_keep {
                    op_pt.op_state = OF_TYPE_SUS;
                    op_pt.amp = op_pt.sustain_level;
                } else {
                    op_pt.op_state = OF_TYPE_SUS_NOKEEP;
                }
            }
            op_pt.step_amp = op_pt.amp;
        }
    }
    consume_env_steps(op_pt, num_steps_add);
}

pub fn operator_attack(op_pt: &mut OpType) {
    op_pt.amp = ((op_pt.a3 * op_pt.amp + op_pt.a2) * op_pt.amp + op_pt.a1) * op_pt.amp + op_pt.a0;

    let num_steps_add = env_steps(op_pt);
    for _ in 0..num_steps_add {
        op_pt.cur_env_step = op_pt.cur_env_step.wrapping_add(1);
        if (op_pt.cur_env_step & op_pt.env_step_a) == 0 {
            if op_pt.amp > 1.0 {
                op_pt.op_state = OF_TYPE_DEC;
                op_pt.amp = 1.0;
                op_pt.step_amp = 1.0;
            }
            op_pt.step_skip_pos_a <<= 1;
            if op_pt.step_skip_pos_a == 0 {
                op_pt.step_skip_pos_a = 1;
            }
            if (op_pt.step_skip_pos_a as Bits & op_pt.env_step_skip_a) != 0 {
                op_pt.step_amp = op_pt.amp;
            }
        }
    }
    consume_env_steps(op_pt, num_steps_add);
}

/// `(1<<(steps<=12?12-steps:0))-1`.
fn env_step_mask(steps: Bits) -> Bits {
    let shift = if steps <= 12 { 12 - steps } else { 0 };
    ((1i32 << shift) - 1) as Bits
}

/// `change_attackrate(regbase, &op[op_pt])`.
pub fn change_attackrate(s: &mut OplState, regbase: Bitu, op_pt: usize) {
    let attackrate = Bits::from(s.adlibreg[ARC_ATTR_DECR + regbase] >> 4);
    let recipsamp = s.recipsamp;
    let o = &mut s.op[op_pt];
    if attackrate != 0 {
        let f = libm::pow(FL2, attackrate as Fltype + Fltype::from(o.toff >> 2) - 1.0)
            * ATTACKCONST[(o.toff & 3) as usize]
            * recipsamp;
        o.a0 = 0.0377 * f;
        o.a1 = 10.73 * f + 1.0;
        o.a2 = -17.57 * f;
        o.a3 = 7.42 * f;

        let step_skip = attackrate * 4 + o.toff as Bits;
        let steps = step_skip >> 2;
        o.env_step_a = env_step_mask(steps);

        let step_num = if step_skip <= 48 {
            4 - (step_skip & 3)
        } else {
            0
        };
        const STEP_SKIP_MASK: [u8; 5] = [0xff, 0xfe, 0xee, 0xba, 0xaa];
        o.env_step_skip_a = Bits::from(STEP_SKIP_MASK[step_num as usize]);

        if step_skip >= 62 {
            // An immediate transition to amp = 1.0.
            o.a0 = 2.0;
            o.a1 = 0.0;
            o.a2 = 0.0;
            o.a3 = 0.0;
        }
    } else {
        o.a0 = 0.0;
        o.a1 = 1.0;
        o.a2 = 0.0;
        o.a3 = 0.0;
        o.env_step_a = 0;
        o.env_step_skip_a = 0;
    }
}

/// `pow(FL2, f*pow(FL2, rate + (toff>>2)))` with
/// `f = -7.4493*decrelconst[toff&3]*recipsamp`.
fn decrel_mul(rate: Bits, toff: u32, recipsamp: Fltype) -> Fltype {
    let f = -7.4493 * DECRELCONST[(toff & 3) as usize] * recipsamp;
    libm::pow(
        FL2,
        f * libm::pow(FL2, (rate + (toff >> 2) as Bits) as Fltype),
    )
}

/// `change_decayrate(regbase, &op[op_pt])`.
pub fn change_decayrate(s: &mut OplState, regbase: Bitu, op_pt: usize) {
    let decayrate = Bits::from(s.adlibreg[ARC_ATTR_DECR + regbase] & 15);
    let recipsamp = s.recipsamp;
    let o = &mut s.op[op_pt];
    if decayrate != 0 {
        o.decaymul = decrel_mul(decayrate, o.toff, recipsamp);
        let steps = (decayrate * 4 + o.toff as Bits) >> 2;
        o.env_step_d = env_step_mask(steps);
    } else {
        o.decaymul = 1.0;
        o.env_step_d = 0;
    }
}

/// `change_releaserate(regbase, &op[op_pt])`.
pub fn change_releaserate(s: &mut OplState, regbase: Bitu, op_pt: usize) {
    let releaserate = Bits::from(s.adlibreg[ARC_SUSL_RELR + regbase] & 15);
    let recipsamp = s.recipsamp;
    let o = &mut s.op[op_pt];
    if releaserate != 0 {
        o.releasemul = decrel_mul(releaserate, o.toff, recipsamp);
        let steps = (releaserate * 4 + o.toff as Bits) >> 2;
        o.env_step_r = env_step_mask(steps);
    } else {
        o.releasemul = 1.0;
        o.env_step_r = 0;
    }
}

/// `change_sustainlevel(regbase, &op[op_pt])`.
pub fn change_sustainlevel(s: &mut OplState, regbase: Bitu, op_pt: usize) {
    let sustainlevel = Bits::from(s.adlibreg[ARC_SUSL_RELR + regbase] >> 4);
    let o = &mut s.op[op_pt];
    if sustainlevel < 15 {
        o.sustain_level = libm::pow(FL2, sustainlevel as Fltype * (-FL05));
    } else {
        o.sustain_level = 0.0;
    }
}

/// `change_waveform(regbase, &op[op_pt])`.
pub fn change_waveform(s: &mut OplState, regbase: Bitu, op_pt: usize) {
    let sel = s.wave_sel[regbase] as usize;
    let o = &mut s.op[op_pt];
    o.cur_wmask = WAVEMASK[sel];
    o.cur_wform = Some(WAVEFORM[sel] as usize);
}

/// `change_keepsustain(regbase, &op[op_pt])`.
pub fn change_keepsustain(s: &mut OplState, regbase: Bitu, op_pt: usize) {
    let sus_keep = (s.adlibreg[ARC_TVS_KSR_MUL + regbase] & 0x20) > 0;
    let o = &mut s.op[op_pt];
    o.sus_keep = sus_keep;
    if o.op_state == OF_TYPE_SUS {
        if !o.sus_keep {
            o.op_state = OF_TYPE_SUS_NOKEEP;
        }
    } else if o.op_state == OF_TYPE_SUS_NOKEEP && o.sus_keep {
        o.op_state = OF_TYPE_SUS;
    }
}

/// `change_vibrato(regbase, &op[op_pt])`.
pub fn change_vibrato(s: &mut OplState, regbase: Bitu, op_pt: usize) {
    let reg = s.adlibreg[ARC_TVS_KSR_MUL + regbase];
    let o = &mut s.op[op_pt];
    o.vibrato = (reg & 0x40) != 0;
    o.tremolo = (reg & 0x80) != 0;
}

/// `change_feedback(chanbase, &op[op_pt])`.
pub fn change_feedback(s: &mut OplState, chanbase: Bitu, op_pt: usize) {
    let feedback = Bits::from(s.adlibreg[ARC_FEEDBACK + chanbase] & 14);
    s.op[op_pt].mfbi = if feedback != 0 {
        libm::pow(FL2, ((feedback >> 1) + 8) as Fltype) as i32
    } else {
        0
    };
}

/// `change_frequency(chanbase, regbase, &op[op_pt])`.
pub fn change_frequency(s: &mut OplState, chanbase: Bitu, regbase: Bitu, op_pt: usize) {
    let kon_bnum = u32::from(s.adlibreg[ARC_KON_BNUM + chanbase]);
    let frn = ((kon_bnum & 3) << 8) + u32::from(s.adlibreg[ARC_FREQ_NUM + chanbase]);
    let oct = (kon_bnum >> 2) & 7;
    let note_sel = u32::from((s.adlibreg[8] >> 6) & 1);
    let tvs_ksr_mul = s.adlibreg[ARC_TVS_KSR_MUL + regbase];
    let ksl_outlev = s.adlibreg[ARC_KSL_OUTLEV + regbase];
    let frqmul = s.frqmul[(tvs_ksr_mul & 15) as usize];
    let kslev = s.kslev[oct as usize][(frn >> 6) as usize];

    let o = &mut s.op[op_pt];
    o.freq_high = ((frn >> 7) & 7) as i32;

    // keysplit
    o.toff = ((frn >> 9) & (note_sel ^ 1)) | ((frn >> 8) & note_sel);
    o.toff += oct << 1;

    // envelope scaling (KSR)
    if (tvs_ksr_mul & 0x10) == 0 {
        o.toff >>= 2;
    }

    o.tinc = (Fltype::from(frn << oct) * frqmul) as u32;
    let vol_in =
        Fltype::from(ksl_outlev & 63) + KSLMUL[(ksl_outlev >> 6) as usize] * Fltype::from(kslev);
    o.vol = libm::pow(FL2, vol_in * -0.125 - 14.0);

    change_attackrate(s, regbase, op_pt);
    change_decayrate(s, regbase, op_pt);
    change_releaserate(s, regbase, op_pt);
}

/// `enable_operator(regbase, &op[op_pt], act_type)`.
pub fn enable_operator(s: &mut OplState, regbase: Bitu, op_pt: usize, act_type: u32) {
    if s.op[op_pt].act_state == OP_ACT_OFF {
        let mut wselbase = regbase as Bits;
        if wselbase >= ARC_SECONDSET as Bits {
            wselbase -= (ARC_SECONDSET - 22) as Bits;
        }
        let start = WAVESTART[s.wave_sel[wselbase as usize] as usize];

        let o = &mut s.op[op_pt];
        o.tcount = start.wrapping_mul(FIXEDPT as u32);

        o.op_state = OF_TYPE_ATT;
        o.act_state |= act_type;
    }
}

/// `disable_operator(&op[op_pt], act_type)`.
pub fn disable_operator(s: &mut OplState, op_pt: usize, act_type: u32) {
    let o = &mut s.op[op_pt];
    if o.act_state != OP_ACT_OFF {
        o.act_state &= !act_type;
        if o.act_state == OP_ACT_OFF && o.op_state != OF_TYPE_OFF {
            o.op_state = OF_TYPE_REL;
        }
    }
}

pub fn adlib_init(s: &mut OplState, samplerate: u32) {
    s.int_samplerate = samplerate as Bits;

    // D-UB-OPL-1: a sample rate of 0 divides to infinity, whose conversion
    // depends on the CPU.
    s.generator_add =
        crate::cconv::f64_to_u32(INTFREQU * Fltype::from(FIXEDPT) / s.int_samplerate as Fltype);

    s.adlibreg = [0; 256];
    s.op = [OpType::default(); MAXOPERATORS];
    s.wave_sel = [0; 22];

    for o in s.op.iter_mut() {
        o.op_state = OF_TYPE_OFF;
        o.act_state = OP_ACT_OFF;
        o.cur_wmask = WAVEMASK[0];
        o.cur_wform = Some(WAVEFORM[0] as usize);
    }

    s.recipsamp = 1.0 / s.int_samplerate as Fltype;
    for i in (0..16).rev() {
        s.frqmul[i] =
            FRQMUL_TAB[i] * INTFREQU / Fltype::from(WAVEPREC) * Fltype::from(FIXEDPT) * s.recipsamp;
    }

    s.status = 0;
    s.opl_index = 0;

    s.vib_table[0] = 8;
    s.vib_table[1] = 4;
    s.vib_table[2] = 0;
    s.vib_table[3] = -4;
    for i in 4..VIBTAB_SIZE {
        s.vib_table[i] = -s.vib_table[i - 4];
    }

    // D-UB-OPL-1, as for `generator_add`.
    s.vibtab_add = crate::cconv::f64_to_u32(
        Fltype::from(VIBTAB_SIZE as i32 * FIXEDPT_LFO / 8192) * INTFREQU
            / s.int_samplerate as Fltype,
    );
    s.vibtab_pos = 0;

    s.vibval_const = [0; BLOCKBUF_SIZE];

    let mut trem_table_int = [0i32; TREMTAB_SIZE];
    for (i, t) in trem_table_int.iter_mut().enumerate() {
        let i = i as i32;
        *t = match i {
            0..14 => i - 13,
            14..41 => -i + 14,
            _ => i - 40 - 26,
        };
    }

    for (i, &t) in trem_table_int.iter().enumerate() {
        let trem_val1 = Fltype::from(t) * 4.8 / 26.0 / 6.0;
        let trem_val2 = Fltype::from(t / 4) * 1.2 / 6.0 / 6.0;
        s.trem_table[i] = (libm::pow(FL2, trem_val1) * Fltype::from(FIXEDPT)) as i32;
        s.trem_table[TREMTAB_SIZE + i] = (libm::pow(FL2, trem_val2) * Fltype::from(FIXEDPT)) as i32;
    }

    // D-UB-OPL-1, as for `generator_add`.
    s.tremtab_add = crate::cconv::f64_to_u32(
        TREMTAB_SIZE as Fltype * TREM_FREQ * Fltype::from(FIXEDPT_LFO) / s.int_samplerate as Fltype,
    );
    s.tremtab_pos = 0;

    s.tremval_const = [FIXEDPT; BLOCKBUF_SIZE];

    if s.adlib_init_initfirstime == 0 {
        s.adlib_init_initfirstime = 1;

        const WP: usize = WAVEPREC as usize;
        let sine = |k: usize| {
            (16384.0 * libm::sin(k as Fltype * PI * 2.0 / Fltype::from(WAVEPREC))) as i16
        };
        for i in 0..(WP >> 1) {
            s.wavtable[(i << 1) + WP] = sine(i << 1);
            s.wavtable[(i << 1) + 1 + WP] = sine((i << 1) + 1);
            s.wavtable[i] = s.wavtable[(i << 1) + WP];
        }
        for i in 0..(WP >> 3) {
            s.wavtable[i + (WP << 1)] = (i32::from(s.wavtable[i + (WP >> 3)]) - 16384) as i16;
            s.wavtable[i + ((WP * 17) >> 3)] =
                (i32::from(s.wavtable[i + (WP >> 2)]) + 16384) as i16;
        }

        s.kslev[7][..9].copy_from_slice(&[0, 24, 32, 37, 40, 43, 45, 47, 48]);
        for i in 9..16 {
            s.kslev[7][i] = (i + 41) as u8;
        }
        for j in (0..=6).rev() {
            for i in 0..16 {
                let mut oct = Bits::from(s.kslev[j + 1][i]) - 8;
                if oct < 0 {
                    oct = 0;
                }
                s.kslev[j][i] = oct as u8;
            }
        }
    }
}

pub fn adlib_write(s: &mut OplState, idx: Bitu, val: u8) {
    let second_set = idx & ARC_SECONDSET_MASK;
    s.adlibreg[idx] = val;

    match idx & 0xf0 {
        ARC_CONTROL => {
            // 0x02/0x03 timer counters, 0x08 CSW/note select: no action.
            if idx == 0x04 {
                // IRQ reset, timer mask/start
                if (val & 0x80) != 0 {
                    s.status &= !0x60;
                } else {
                    s.status = 0;
                }
            }
        }
        0x20 | 0x30 => {
            // tremolo/vibrato/sustain keeping; key scale rate; frequency multiplication
            let num = idx & 7;
            let base = idx.wrapping_sub(ARC_TVS_KSR_MUL) & 0xff;
            if num < 6 && base < 22 {
                let modop =
                    Bitu::from(REGBASE2MODOP[if second_set != 0 { base + 22 } else { base }]);
                let regbase = base + second_set;
                let chanbase = if second_set != 0 {
                    modop - 18 + ARC_SECONDSET
                } else {
                    modop
                };
                let op_ptr = modop + if num < 3 { 0 } else { 9 };
                change_keepsustain(s, regbase, op_ptr);
                change_vibrato(s, regbase, op_ptr);
                change_frequency(s, chanbase, base, op_ptr);
            }
        }
        0x40 | 0x50 => {
            // key scale level; output rate
            let num = idx & 7;
            let base = idx.wrapping_sub(ARC_KSL_OUTLEV) & 0xff;
            if num < 6 && base < 22 {
                let modop =
                    Bitu::from(REGBASE2MODOP[if second_set != 0 { base + 22 } else { base }]);
                let chanbase = if second_set != 0 {
                    modop - 18 + ARC_SECONDSET
                } else {
                    modop
                };
                let op_ptr = modop + if num < 3 { 0 } else { 9 };
                change_frequency(s, chanbase, base, op_ptr);
            }
        }
        0x60 | 0x70 => {
            // attack/decay rates
            let num = idx & 7;
            let base = idx.wrapping_sub(ARC_ATTR_DECR) & 0xff;
            if num < 6 && base < 22 {
                let regbase = base + second_set;
                let op_ptr =
                    usize::from(REGBASE2OP[if second_set != 0 { base + 22 } else { base }]);
                change_attackrate(s, regbase, op_ptr);
                change_decayrate(s, regbase, op_ptr);
            }
        }
        0x80 | 0x90 => {
            // sustain level; release rate
            let num = idx & 7;
            let base = idx.wrapping_sub(ARC_SUSL_RELR) & 0xff;
            if num < 6 && base < 22 {
                let regbase = base + second_set;
                let op_ptr =
                    usize::from(REGBASE2OP[if second_set != 0 { base + 22 } else { base }]);
                change_releaserate(s, regbase, op_ptr);
                change_sustainlevel(s, regbase, op_ptr);
            }
        }
        ARC_FREQ_NUM => {
            // 0xa0-0xa8 low8 frequency
            let base = idx.wrapping_sub(ARC_FREQ_NUM) & 0xff;
            if base < 9 {
                let opbase = if second_set != 0 { base + 18 } else { base };
                let modbase = Bitu::from(MODULATORBASE[base]) + second_set;
                let chanbase = base + second_set;
                change_frequency(s, chanbase, modbase, opbase);
                change_frequency(s, chanbase, modbase + 3, opbase + 9);
            }
        }
        ARC_KON_BNUM => {
            if idx == ARC_PERC_MODE {
                if (val & 0x30) == 0x30 {
                    // BassDrum active
                    enable_operator(s, 16, 6, OP_ACT_PERC);
                    change_frequency(s, 6, 16, 6);
                    enable_operator(s, 16 + 3, 6 + 9, OP_ACT_PERC);
                    change_frequency(s, 6, 16 + 3, 6 + 9);
                } else {
                    disable_operator(s, 6, OP_ACT_PERC);
                    disable_operator(s, 6 + 9, OP_ACT_PERC);
                }
                if (val & 0x28) == 0x28 {
                    // Snare active
                    enable_operator(s, 17 + 3, 16, OP_ACT_PERC);
                    change_frequency(s, 7, 17 + 3, 16);
                } else {
                    disable_operator(s, 16, OP_ACT_PERC);
                }
                if (val & 0x24) == 0x24 {
                    // TomTom active
                    enable_operator(s, 18, 8, OP_ACT_PERC);
                    change_frequency(s, 8, 18, 8);
                } else {
                    disable_operator(s, 8, OP_ACT_PERC);
                }
                if (val & 0x22) == 0x22 {
                    // Cymbal active
                    enable_operator(s, 18 + 3, 8 + 9, OP_ACT_PERC);
                    change_frequency(s, 8, 18 + 3, 8 + 9);
                } else {
                    disable_operator(s, 8 + 9, OP_ACT_PERC);
                }
                if (val & 0x21) == 0x21 {
                    // Hihat active
                    enable_operator(s, 17, 7, OP_ACT_PERC);
                    change_frequency(s, 7, 17, 7);
                } else {
                    disable_operator(s, 7, OP_ACT_PERC);
                }
                return;
            }
            // regular 0xb0-0xb8
            let base = idx.wrapping_sub(ARC_KON_BNUM) & 0xff;
            if base < 9 {
                let opbase = if second_set != 0 { base + 18 } else { base };
                let modbase = Bitu::from(MODULATORBASE[base]) + second_set;
                if (val & 32) != 0 {
                    enable_operator(s, modbase, opbase, OP_ACT_NORMAL);
                    enable_operator(s, modbase + 3, opbase + 9, OP_ACT_NORMAL);
                } else {
                    disable_operator(s, opbase, OP_ACT_NORMAL);
                    disable_operator(s, opbase + 9, OP_ACT_NORMAL);
                }
                let chanbase = base + second_set;
                change_frequency(s, chanbase, modbase, opbase);
                change_frequency(s, chanbase, modbase + 3, opbase + 9);
            }
        }
        ARC_FEEDBACK => {
            // 0xc0-0xc8 feedback/modulation type (AM/FM)
            let base = idx.wrapping_sub(ARC_FEEDBACK) & 0xff;
            if base < 9 {
                let opbase = if second_set != 0 { base + 18 } else { base };
                let chanbase = base + second_set;
                change_feedback(s, chanbase, opbase);
            }
        }
        0xe0 | 0xf0 => {
            let num = idx & 7;
            let base = idx.wrapping_sub(ARC_WAVE_SEL) & 0xff;
            if num < 6 && base < 22 && (s.adlibreg[0x01] & 0x20) != 0 {
                // wave selection enabled, change waveform
                s.wave_sel[base] = val & 3;
                let op_ptr = usize::from(REGBASE2MODOP[base]) + if num < 3 { 0 } else { 9 };
                change_waveform(s, base, op_ptr);
            }
        }
        _ => {}
    }
}

pub fn adlib_reg_read(s: &OplState, port: Bitu) -> Bitu {
    // opl2-detection routines require ret&6 to be 6
    if (port & 1) == 0 {
        return Bitu::from(s.status | 6);
    }
    0xff
}

pub fn adlib_write_index(s: &mut OplState, port: Bitu, val: u8) {
    let _ = port;
    s.opl_index = u32::from(val);
}

pub fn clipit16(ival: i32, outval: &mut i16) {
    if ival < 32768 {
        if ival > -32769 {
            *outval = ival as i16;
        } else {
            *outval = -32768;
        }
    } else {
        *outval = 32767;
    }
}

/// `(Bit32s)((vib_lut[i]*freq_high/8)*FIXEDPT*VIBFAC)`.
fn vibval_of(vib_lut: i32, freq_high: i32) -> i32 {
    (vib_lut.wrapping_mul(freq_high) / 8)
        .wrapping_mul(FIXEDPT)
        .wrapping_mul(VIBFAC_MUL)
        / VIBFAC_DIV
}

/// Points a `vibval*` pointer at `var` (filled from `vib_lut` for the
/// operator `op_pt`) when `enabled`, else at `vibval_const`.
fn select_vibval(
    s: &mut OplState,
    enabled: bool,
    var: VibvalTable,
    vib_lut: &[i32; BLOCKBUF_SIZE],
    op_pt: usize,
    endsamples: usize,
) -> VibvalTable {
    if !enabled {
        return VibvalTable::Const;
    }
    let freq_high = s.op[op_pt].freq_high;
    let table = match var {
        VibvalTable::Var1 => &mut s.vibval_var1,
        VibvalTable::Var2 => &mut s.vibval_var2,
        VibvalTable::Null | VibvalTable::Const => unreachable!("only the var tables are filled"),
    };
    for (v, &l) in table[..endsamples].iter_mut().zip(vib_lut) {
        *v = vibval_of(l, freq_high);
    }
    var
}

/// The tables the sample loops read through the `vibval*` pointers and the
/// `tremval*` choices, borrowed apart from the operators they advance.
struct Tables<'a> {
    generator_add: u32,
    wavtable: &'a Wavtable,
    vibval_const: &'a [i32; BLOCKBUF_SIZE],
    vibval_var1: &'a [i32; BLOCKBUF_SIZE],
    vibval_var2: &'a [i32; BLOCKBUF_SIZE],
    tremval_const: &'a [i32; BLOCKBUF_SIZE],
    trem_lut: &'a [i32; BLOCKBUF_SIZE],
}

impl<'a> Tables<'a> {
    /// Splits `s` into its operators and the tables (`trem_lut` is
    /// `adlib_getsample`'s local table).
    fn split(
        s: &'a mut OplState,
        trem_lut: &'a [i32; BLOCKBUF_SIZE],
    ) -> (&'a mut [OpType; MAXOPERATORS], Self) {
        let OplState {
            op,
            generator_add,
            wavtable,
            vibval_const,
            vibval_var1,
            vibval_var2,
            tremval_const,
            ..
        } = s;
        let tables = Tables {
            generator_add: *generator_add,
            wavtable,
            vibval_const,
            vibval_var1,
            vibval_var2,
            tremval_const,
            trem_lut,
        };
        (op, tables)
    }

    /// The first `n` entries of the table a `vibval*` pointer targets.
    fn vibval(&self, table: VibvalTable, n: usize) -> &'a [i32] {
        let t = match table {
            VibvalTable::Const => self.vibval_const,
            VibvalTable::Var1 => self.vibval_var1,
            VibvalTable::Var2 => self.vibval_var2,
            VibvalTable::Null => unreachable!("vibval pointers are set before use"),
        };
        &t[..n]
    }

    /// The first `n` entries of `tremvalN`: `trem_lut` when the operator's
    /// tremolo is enabled, else `tremval_const`.
    fn tremval(&self, use_lut: bool, n: usize) -> &'a [i32] {
        let t = if use_lut {
            self.trem_lut
        } else {
            self.tremval_const
        };
        &t[..n]
    }

    /// `operator_advance(o, vib); opfuncs[o->op_state](o);
    /// operator_output(o, modulator(o), trem)`.
    #[inline(always)]
    fn op_sample(&self, o: &mut OpType, vib: i32, modulator: fn(&OpType) -> i32, trem: i32) {
        advance(o, self.generator_add, vib);
        run_opfunc(o);
        let m = modulator(o);
        output(o, self.wavtable, m, trem);
    }
}

/// Operators `a` and `b` (`a < b`), borrowed together.
fn op_pair(op: &mut [OpType; MAXOPERATORS], a: usize, b: usize) -> (&mut OpType, &mut OpType) {
    let (lo, hi) = op.split_at_mut(b);
    (&mut lo[a], &mut hi[0])
}

/// No modulation.
fn no_modulator(_: &OpType) -> i32 {
    0
}

/// The self-feedback modulator `(lastcval+cval)*mfbi/2` of an operator.
fn feedback_of(o: &OpType) -> i32 {
    o.lastcval.wrapping_add(o.cval).wrapping_mul(o.mfbi) / 2
}

/// `adlib_getsample(sndptr, numsamples)`; `sndptr` holds at least
/// `numsamples` samples.
#[allow(clippy::needless_range_loop)] // `i` indexes several block buffers, as upstream
pub fn adlib_getsample(
    s: &mut OplState,
    crand: &std::sync::Mutex<crate::crand::State>,
    sndptr: &mut [i16],
    numsamples: Bits,
) {
    let mut out = 0usize;
    let mut outbufl = [0i32; BLOCKBUF_SIZE];
    let mut vib_lut = [0i32; BLOCKBUF_SIZE];
    let mut trem_lut = [0i32; BLOCKBUF_SIZE];

    let samples_to_process = numsamples;
    let mut cursmp: Bits = 0;
    while cursmp < samples_to_process {
        let mut endsamples = samples_to_process - cursmp;
        if endsamples > BLOCKBUF_SIZE as Bits {
            endsamples = BLOCKBUF_SIZE as Bits;
        }
        let n = endsamples as usize;

        outbufl[..n].fill(0);

        // vibrato/tremolo lookup tables
        let vib_tshift = if (s.adlibreg[ARC_PERC_MODE] & 0x40) == 0 {
            1
        } else {
            0
        };
        for i in 0..n {
            s.vibtab_pos = s.vibtab_pos.wrapping_add(s.vibtab_add);
            if s.vibtab_pos / FIXEDPT_LFO as u32 >= VIBTAB_SIZE as u32 {
                s.vibtab_pos = s
                    .vibtab_pos
                    .wrapping_sub((VIBTAB_SIZE as i32 * FIXEDPT_LFO) as u32);
            }
            vib_lut[i] = s.vib_table[(s.vibtab_pos / FIXEDPT_LFO as u32) as usize] >> vib_tshift;

            s.tremtab_pos = s.tremtab_pos.wrapping_add(s.tremtab_add);
            if s.tremtab_pos / FIXEDPT_LFO as u32 >= TREMTAB_SIZE as u32 {
                s.tremtab_pos = s
                    .tremtab_pos
                    .wrapping_sub((TREMTAB_SIZE as i32 * FIXEDPT_LFO) as u32);
            }
            let t = (s.tremtab_pos / FIXEDPT_LFO as u32) as usize;
            trem_lut[i] = if (s.adlibreg[ARC_PERC_MODE] & 0x80) != 0 {
                s.trem_table[t]
            } else {
                s.trem_table[TREMTAB_SIZE + t]
            };
        }

        let off = |s: &OplState, o: usize| s.op[o].op_state == OF_TYPE_OFF;
        let outbuf = &mut outbufl[..n];

        if (s.adlibreg[ARC_PERC_MODE] & 0x20) != 0 {
            // BassDrum (operators 6 and 15)
            let (c0, c9) = (6, 6 + 9);
            if (s.adlibreg[ARC_FEEDBACK + 6] & 1) != 0 {
                // additive synthesis
                if !off(s, c9) {
                    let en = s.op[c9].vibrato;
                    s.vibval1 = select_vibval(s, en, VibvalTable::Var1, &vib_lut, c9, n);
                    let tl1 = s.op[c9].tremolo;
                    let vt1 = s.vibval1;
                    let (op, t) = Tables::split(s, &trem_lut);
                    let (vib1, trem1) = (t.vibval(vt1, n), t.tremval(tl1, n));
                    let o = &mut op[c9];
                    for i in 0..n {
                        t.op_sample(o, vib1[i], no_modulator, trem1[i]);
                        let chanval = o.cval.wrapping_mul(2);
                        outbuf[i] = outbuf[i].wrapping_add(chanval);
                    }
                }
            } else {
                // frequency modulation
                if !off(s, c9) || !off(s, c0) {
                    let en = s.op[c0].vibrato && !off(s, c0);
                    s.vibval1 = select_vibval(s, en, VibvalTable::Var1, &vib_lut, c0, n);
                    let en = s.op[c9].vibrato && !off(s, c9);
                    s.vibval2 = select_vibval(s, en, VibvalTable::Var2, &vib_lut, c9, n);
                    let tl1 = s.op[c0].tremolo;
                    let tl2 = s.op[c9].tremolo;
                    let (vt1, vt2) = (s.vibval1, s.vibval2);
                    let (op, t) = Tables::split(s, &trem_lut);
                    let (vib1, vib2) = (t.vibval(vt1, n), t.vibval(vt2, n));
                    let (trem1, trem2) = (t.tremval(tl1, n), t.tremval(tl2, n));
                    let (m, c) = op_pair(op, c0, c9);
                    for i in 0..n {
                        two_op_fm_sample(&t, m, c, (vib1[i], vib2[i]), (trem1[i], trem2[i]));
                        let chanval = c.cval.wrapping_mul(2);
                        outbuf[i] = outbuf[i].wrapping_add(chanval);
                    }
                }
            }

            // TomTom (j=8)
            if !off(s, 8) {
                let c0 = 8;
                let en = s.op[c0].vibrato;
                s.vibval3 = select_vibval(s, en, VibvalTable::Var1, &vib_lut, c0, n);
                let tl3 = s.op[c0].tremolo;
                let vt3 = s.vibval3;
                let (op, t) = Tables::split(s, &trem_lut);
                let (vib3, trem3) = (t.vibval(vt3, n), t.tremval(tl3, n));
                let o = &mut op[c0];
                for i in 0..n {
                    t.op_sample(o, vib3[i], no_modulator, trem3[i]);
                    let chanval = o.cval.wrapping_mul(2);
                    outbuf[i] = outbuf[i].wrapping_add(chanval);
                }
            }

            // Snare/Hihat (j=7), Cymbal (j=8)
            if !off(s, 7) || !off(s, 16) || !off(s, 17) {
                let en = s.op[7].vibrato && !off(s, 7);
                s.vibval1 = select_vibval(s, en, VibvalTable::Var1, &vib_lut, 7, n);
                // Upstream tests for OF_TYPE_OFF here (sic).
                let en = s.op[16].vibrato && off(s, 16);
                s.vibval2 = select_vibval(s, en, VibvalTable::Var2, &vib_lut, 16, n);
                let tl1 = s.op[7].tremolo;
                let tl2 = s.op[16].tremolo;
                let en = s.op[17].vibrato && off(s, 17);
                s.vibval4 = select_vibval(s, en, VibvalTable::Var2, &vib_lut, 17, n);
                let tl4 = s.op[17].tremolo;

                let (vt1, vt2, vt4) = (s.vibval1, s.vibval2, s.vibval4);
                let (op, t) = Tables::split(s, &trem_lut);
                let (vib1, vib2, vib4) = (t.vibval(vt1, n), t.vibval(vt2, n), t.vibval(vt4, n));
                let (trem1, trem2, trem4) =
                    (t.tremval(tl1, n), t.tremval(tl2, n), t.tremval(tl4, n));
                for i in 0..n {
                    advance_drums(
                        op,
                        t.generator_add,
                        crand,
                        [(7, vib1[i]), (7 + 9, vib2[i]), (8 + 9, vib4[i])],
                    );
                    run_opfunc(&mut op[7]); // Hihat
                    output(&mut op[7], t.wavtable, 0, trem1[i]);
                    run_opfunc(&mut op[7 + 9]); // Snare
                    output(&mut op[7 + 9], t.wavtable, 0, trem2[i]);
                    run_opfunc(&mut op[8 + 9]); // Cymbal
                    output(&mut op[8 + 9], t.wavtable, 0, trem4[i]);

                    let chanval = op[7]
                        .cval
                        .wrapping_add(op[7 + 9].cval)
                        .wrapping_add(op[8 + 9].cval)
                        .wrapping_mul(2);
                    outbuf[i] = outbuf[i].wrapping_add(chanval);
                }
            }
        }

        let max_channel = NUM_CHANNELS as Bits;
        for cur_ch in (0..max_channel).rev() {
            // skip drum/percussion operators
            if (s.adlibreg[ARC_PERC_MODE] & 0x20) != 0 && (6..9).contains(&cur_ch) {
                continue;
            }

            let k = cur_ch as Bitu;
            let c0 = cur_ch as usize;
            let c9 = c0 + 9;

            // 2op additive synthesis or frequency modulation; skip if both off
            if off(s, c9) && off(s, c0) {
                continue;
            }
            let additive = (s.adlibreg[ARC_FEEDBACK + k] & 1) != 0;
            let en = s.op[c0].vibrato && !off(s, c0);
            s.vibval1 = select_vibval(s, en, VibvalTable::Var1, &vib_lut, c0, n);
            let en = s.op[c9].vibrato && !off(s, c9);
            s.vibval2 = select_vibval(s, en, VibvalTable::Var2, &vib_lut, c9, n);
            let tl1 = s.op[c0].tremolo;
            let tl2 = s.op[c9].tremolo;

            let (vt1, vt2) = (s.vibval1, s.vibval2);
            let (op, t) = Tables::split(s, &trem_lut);
            let (vib1, vib2) = (t.vibval(vt1, n), t.vibval(vt2, n));
            let (trem1, trem2) = (t.tremval(tl1, n), t.tremval(tl2, n));
            let (m, c) = op_pair(op, c0, c9);
            if additive {
                for i in 0..n {
                    // carrier1
                    t.op_sample(m, vib1[i], feedback_of, trem1[i]);
                    // carrier2
                    t.op_sample(c, vib2[i], no_modulator, trem2[i]);
                    let chanval = c.cval.wrapping_add(m.cval);
                    outbuf[i] = outbuf[i].wrapping_add(chanval);
                }
            } else {
                for i in 0..n {
                    two_op_fm_sample(&t, m, c, (vib1[i], vib2[i]), (trem1[i], trem2[i]));
                    outbuf[i] = outbuf[i].wrapping_add(c.cval);
                }
            }
        }

        // convert to 16bit samples
        for &v in outbuf.iter() {
            clipit16(v, &mut sndptr[out]);
            out += 1;
        }

        cursmp += endsamples;
    }
}

/// One sample of a 2op FM pair: modulator `m` (with self-feedback) into
/// carrier `c`; `vib` and `trem` are their `vibvalN[i]` and `tremvalN[i]`.
#[inline(always)]
fn two_op_fm_sample(
    t: &Tables<'_>,
    m: &mut OpType,
    c: &mut OpType,
    vib: (i32, i32),
    trem: (i32, i32),
) {
    // modulator
    t.op_sample(m, vib.0, feedback_of, trem.0);
    // carrier
    let modulator = m.cval.wrapping_mul(FIXEDPT);
    advance(c, t.generator_add, vib.1);
    run_opfunc(c);
    output(c, t.wavtable, modulator, trem.1);
}
