//! Mirror of the C globals of `opl.c` (all file statics).
//!
//! They live in `rustyrian_core::opl::OplState`, owned by
//! `loudness::AudioState`, so the loudness `AudioState` mirror calls
//! [`opl_from_c`]/[`opl_to_c`]; `opl::State` is empty. `op_type` is defined
//! only in `opl.c` and is redeclared here.

use super::Mirror;
use super::video::sv;
use crate::bindings as b;
use rustyrian_core::opl::{
    BLOCKBUF_SIZE, Bits, Bitu, MAXOPERATORS, OpType, OplState, State, TREMTAB_SIZE, VIBTAB_SIZE,
    VibvalTable, WAVEPREC,
};

const WAVTABLE_LEN: usize = WAVEPREC as usize * 3;
type Block = [i32; BLOCKBUF_SIZE];

/// `op_type` (`struct operator_struct`, OPL2 configuration).
#[repr(C)]
#[derive(Clone, Copy)]
struct COp {
    cval: i32,
    lastcval: i32,
    tcount: u32,
    wfpos: u32,
    tinc: u32,
    amp: f64,
    step_amp: f64,
    vol: f64,
    sustain_level: f64,
    mfbi: i32,
    a0: f64,
    a1: f64,
    a2: f64,
    a3: f64,
    decaymul: f64,
    releasemul: f64,
    op_state: u32,
    toff: u32,
    freq_high: i32,
    cur_wform: *mut i16,
    cur_wmask: u32,
    act_state: u32,
    sus_keep: bool,
    vibrato: bool,
    tremolo: bool,
    generator_pos: u32,
    cur_env_step: Bits,
    env_step_a: Bits,
    env_step_d: Bits,
    env_step_r: Bits,
    step_skip_pos_a: u8,
    env_step_skip_a: Bits,
}

fn wavtable_base() -> *mut i16 {
    // SAFETY: the getter returns the address of `wavtable`.
    unsafe { b::ot_sv_opl_wavtable() }.cast()
}

fn op_from_c(c: &COp) -> OpType {
    let cur_wform = (!c.cur_wform.is_null()).then(|| {
        let d = (c.cur_wform as usize).wrapping_sub(wavtable_base() as usize);
        let i = d / size_of::<i16>();
        assert!(i <= WAVTABLE_LEN, "cur_wform points into wavtable");
        i
    });
    OpType {
        cval: c.cval,
        lastcval: c.lastcval,
        tcount: c.tcount,
        wfpos: c.wfpos,
        tinc: c.tinc,
        amp: c.amp,
        step_amp: c.step_amp,
        vol: c.vol,
        sustain_level: c.sustain_level,
        mfbi: c.mfbi,
        a0: c.a0,
        a1: c.a1,
        a2: c.a2,
        a3: c.a3,
        decaymul: c.decaymul,
        releasemul: c.releasemul,
        op_state: c.op_state,
        toff: c.toff,
        freq_high: c.freq_high,
        cur_wform,
        cur_wmask: c.cur_wmask,
        act_state: c.act_state,
        sus_keep: c.sus_keep,
        vibrato: c.vibrato,
        tremolo: c.tremolo,
        generator_pos: c.generator_pos,
        cur_env_step: c.cur_env_step,
        env_step_a: c.env_step_a,
        env_step_d: c.env_step_d,
        env_step_r: c.env_step_r,
        step_skip_pos_a: c.step_skip_pos_a,
        env_step_skip_a: c.env_step_skip_a,
    }
}

fn op_to_c(r: &OpType) -> COp {
    COp {
        cval: r.cval,
        lastcval: r.lastcval,
        tcount: r.tcount,
        wfpos: r.wfpos,
        tinc: r.tinc,
        amp: r.amp,
        step_amp: r.step_amp,
        vol: r.vol,
        sustain_level: r.sustain_level,
        mfbi: r.mfbi,
        a0: r.a0,
        a1: r.a1,
        a2: r.a2,
        a3: r.a3,
        decaymul: r.decaymul,
        releasemul: r.releasemul,
        op_state: r.op_state,
        toff: r.toff,
        freq_high: r.freq_high,
        cur_wform: r
            .cur_wform
            .map_or(std::ptr::null_mut(), |i| wavtable_base().wrapping_add(i)),
        cur_wmask: r.cur_wmask,
        act_state: r.act_state,
        sus_keep: r.sus_keep,
        vibrato: r.vibrato,
        tremolo: r.tremolo,
        generator_pos: r.generator_pos,
        cur_env_step: r.cur_env_step,
        env_step_a: r.env_step_a,
        env_step_d: r.env_step_d,
        env_step_r: r.env_step_r,
        step_skip_pos_a: r.step_skip_pos_a,
        env_step_skip_a: r.env_step_skip_a,
    }
}

/// The three tables a `vibval*` pointer can target.
fn vib_tables() -> [(VibvalTable, *mut i32); 3] {
    // SAFETY: the getters return the addresses of the statics.
    unsafe {
        [
            (VibvalTable::Const, b::ot_sv_opl_vibval_const().cast()),
            (VibvalTable::Var1, b::ot_sv_opl_vibval_var1().cast()),
            (VibvalTable::Var2, b::ot_sv_opl_vibval_var2().cast()),
        ]
    }
}

fn vibval_from_c(p: *mut i32) -> VibvalTable {
    if p.is_null() {
        return VibvalTable::Null;
    }
    vib_tables()
        .into_iter()
        .find(|&(_, t)| t == p)
        .map(|(v, _)| v)
        .expect("vibval pointer targets the start of a vibval table")
}

fn vibval_to_c(v: VibvalTable) -> *mut i32 {
    vib_tables()
        .into_iter()
        .find(|&(t, _)| t == v)
        .map_or(std::ptr::null_mut(), |(_, p)| p)
}

/// Every global of `opl.c`, from C.
pub fn opl_from_c() -> OplState {
    // SAFETY: C runs only on this thread, inside calls we make; each `sv`
    // type is the C type of that static.
    unsafe {
        let op = sv::<[COp; MAXOPERATORS]>(b::ot_sv_opl_op);
        OplState {
            op: op.map(|c| op_from_c(&c)),
            int_samplerate: *sv::<Bits>(b::ot_sv_opl_int_samplerate),
            status: *sv::<u8>(b::ot_sv_opl_status),
            opl_index: *sv::<u32>(b::ot_sv_opl_opl_index),
            adlibreg: *sv::<[u8; 256]>(b::ot_sv_opl_adlibreg),
            wave_sel: *sv::<[u8; 22]>(b::ot_sv_opl_wave_sel),
            vibtab_pos: *sv::<u32>(b::ot_sv_opl_vibtab_pos),
            vibtab_add: *sv::<u32>(b::ot_sv_opl_vibtab_add),
            tremtab_pos: *sv::<u32>(b::ot_sv_opl_tremtab_pos),
            tremtab_add: *sv::<u32>(b::ot_sv_opl_tremtab_add),
            generator_add: *sv::<u32>(b::ot_sv_opl_generator_add),
            recipsamp: *sv::<f64>(b::ot_sv_opl_recipsamp),
            wavtable: Box::new(*sv::<[i16; WAVTABLE_LEN]>(b::ot_sv_opl_wavtable)),
            vib_table: *sv::<[i32; VIBTAB_SIZE]>(b::ot_sv_opl_vib_table),
            trem_table: *sv::<[i32; TREMTAB_SIZE * 2]>(b::ot_sv_opl_trem_table),
            vibval_const: *sv::<Block>(b::ot_sv_opl_vibval_const),
            tremval_const: *sv::<Block>(b::ot_sv_opl_tremval_const),
            vibval_var1: *sv::<Block>(b::ot_sv_opl_vibval_var1),
            vibval_var2: *sv::<Block>(b::ot_sv_opl_vibval_var2),
            vibval1: vibval_from_c(*sv::<*mut i32>(b::ot_sv_opl_vibval1)),
            vibval2: vibval_from_c(*sv::<*mut i32>(b::ot_sv_opl_vibval2)),
            vibval3: vibval_from_c(*sv::<*mut i32>(b::ot_sv_opl_vibval3)),
            vibval4: vibval_from_c(*sv::<*mut i32>(b::ot_sv_opl_vibval4)),
            frqmul: *sv::<[f64; 16]>(b::ot_sv_opl_frqmul),
            kslev: *sv::<[[u8; 16]; 8]>(b::ot_sv_opl_kslev),
            adlib_init_initfirstime: *sv::<Bitu>(b::ot_sv_opl_adlib_init_initfirstime),
        }
    }
}

/// Writes every global of `opl.c`.
pub fn opl_to_c(r: &OplState) {
    // SAFETY: as in `opl_from_c`.
    unsafe {
        *sv::<[COp; MAXOPERATORS]>(b::ot_sv_opl_op) = r.op.map(|o| op_to_c(&o));
        *sv::<Bits>(b::ot_sv_opl_int_samplerate) = r.int_samplerate;
        *sv::<u8>(b::ot_sv_opl_status) = r.status;
        *sv::<u32>(b::ot_sv_opl_opl_index) = r.opl_index;
        *sv::<[u8; 256]>(b::ot_sv_opl_adlibreg) = r.adlibreg;
        *sv::<[u8; 22]>(b::ot_sv_opl_wave_sel) = r.wave_sel;
        *sv::<u32>(b::ot_sv_opl_vibtab_pos) = r.vibtab_pos;
        *sv::<u32>(b::ot_sv_opl_vibtab_add) = r.vibtab_add;
        *sv::<u32>(b::ot_sv_opl_tremtab_pos) = r.tremtab_pos;
        *sv::<u32>(b::ot_sv_opl_tremtab_add) = r.tremtab_add;
        *sv::<u32>(b::ot_sv_opl_generator_add) = r.generator_add;
        *sv::<f64>(b::ot_sv_opl_recipsamp) = r.recipsamp;
        *sv::<[i16; WAVTABLE_LEN]>(b::ot_sv_opl_wavtable) = *r.wavtable;
        *sv::<[i32; VIBTAB_SIZE]>(b::ot_sv_opl_vib_table) = r.vib_table;
        *sv::<[i32; TREMTAB_SIZE * 2]>(b::ot_sv_opl_trem_table) = r.trem_table;
        *sv::<Block>(b::ot_sv_opl_vibval_const) = r.vibval_const;
        *sv::<Block>(b::ot_sv_opl_tremval_const) = r.tremval_const;
        *sv::<Block>(b::ot_sv_opl_vibval_var1) = r.vibval_var1;
        *sv::<Block>(b::ot_sv_opl_vibval_var2) = r.vibval_var2;
        *sv::<*mut i32>(b::ot_sv_opl_vibval1) = vibval_to_c(r.vibval1);
        *sv::<*mut i32>(b::ot_sv_opl_vibval2) = vibval_to_c(r.vibval2);
        *sv::<*mut i32>(b::ot_sv_opl_vibval3) = vibval_to_c(r.vibval3);
        *sv::<*mut i32>(b::ot_sv_opl_vibval4) = vibval_to_c(r.vibval4);
        *sv::<[f64; 16]>(b::ot_sv_opl_frqmul) = r.frqmul;
        *sv::<[[u8; 16]; 8]>(b::ot_sv_opl_kslev) = r.kslev;
        *sv::<Bitu>(b::ot_sv_opl_adlib_init_initfirstime) = r.adlib_init_initfirstime;
    }
}

/// `opl::State` is empty; its globals are mirrored with `AudioState`.
impl Mirror for State {
    fn load_from_c(&mut self) {}

    fn store_to_c(&self) {}
}
