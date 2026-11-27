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
    pub wavtable: Box<[i16; WAVEPREC as usize * 3]>,
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
