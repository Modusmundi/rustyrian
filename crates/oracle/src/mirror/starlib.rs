//! Mirror of `rustyrian_core::starlib::State` <-> the C globals of `starlib.c`
//! (all file statics).

use super::Mirror;
use super::video::sv;
use crate::bindings as b;
use rustyrian_core::starlib::{JeStarType, STARLIB_MAX_STARS, State};

/// `struct JE_StarType`, defined only in `starlib.c`.
#[repr(C)]
#[derive(Clone, Copy)]
struct CStar {
    sp_x: i16,
    sp_y: i16,
    sp_z: i16,
    last_x: i16,
    last_y: i16,
}

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make; each `sv`
        // type is the C type of that static.
        unsafe {
            self.temp_x = *sv::<i32>(b::ot_sv_starlib_tempX);
            self.temp_y = *sv::<i32>(b::ot_sv_starlib_tempY);
            self.run = *sv::<bool>(b::ot_sv_starlib_run);
            let star = sv::<[CStar; STARLIB_MAX_STARS]>(b::ot_sv_starlib_star);
            for (d, s) in self.star.iter_mut().zip(star.iter()) {
                *d = JeStarType {
                    sp_x: s.sp_x,
                    sp_y: s.sp_y,
                    sp_z: s.sp_z,
                    last_x: s.last_x,
                    last_y: s.last_y,
                };
            }
            self.setup = *sv::<u8>(b::ot_sv_starlib_setup);
            self.step_counter = *sv::<u16>(b::ot_sv_starlib_stepCounter);
            self.nsp2 = *sv::<u16>(b::ot_sv_starlib_nsp2);
            self.nsp_var2_inc = *sv::<i8>(b::ot_sv_starlib_nspVar2Inc);
            self.nsp = *sv::<f32>(b::ot_sv_starlib_nsp);
            self.nsp_var_inc = *sv::<f32>(b::ot_sv_starlib_nspVarInc);
            self.nsp_var_var_inc = *sv::<f32>(b::ot_sv_starlib_nspVarVarInc);
            self.change_time = *sv::<u16>(b::ot_sv_starlib_changeTime);
            self.do_change = *sv::<bool>(b::ot_sv_starlib_doChange);
            self.gray_b = *sv::<bool>(b::ot_sv_starlib_grayB);
            self.starlib_speed = *sv::<i16>(b::ot_sv_starlib_starlib_speed);
            self.speed_change = *sv::<i8>(b::ot_sv_starlib_speedChange);
            self.p_color = *sv::<u8>(b::ot_sv_starlib_pColor);
            self.je_starlib_init_initialized =
                *sv::<bool>(b::ot_sv_starlib_JE_starlib_init_initialized);
        }
    }

    fn store_to_c(&self) {
        // SAFETY: as above.
        unsafe {
            *sv::<i32>(b::ot_sv_starlib_tempX) = self.temp_x;
            *sv::<i32>(b::ot_sv_starlib_tempY) = self.temp_y;
            *sv::<bool>(b::ot_sv_starlib_run) = self.run;
            let star = sv::<[CStar; STARLIB_MAX_STARS]>(b::ot_sv_starlib_star);
            for (d, s) in star.iter_mut().zip(self.star.iter()) {
                *d = CStar {
                    sp_x: s.sp_x,
                    sp_y: s.sp_y,
                    sp_z: s.sp_z,
                    last_x: s.last_x,
                    last_y: s.last_y,
                };
            }
            *sv::<u8>(b::ot_sv_starlib_setup) = self.setup;
            *sv::<u16>(b::ot_sv_starlib_stepCounter) = self.step_counter;
            *sv::<u16>(b::ot_sv_starlib_nsp2) = self.nsp2;
            *sv::<i8>(b::ot_sv_starlib_nspVar2Inc) = self.nsp_var2_inc;
            *sv::<f32>(b::ot_sv_starlib_nsp) = self.nsp;
            *sv::<f32>(b::ot_sv_starlib_nspVarInc) = self.nsp_var_inc;
            *sv::<f32>(b::ot_sv_starlib_nspVarVarInc) = self.nsp_var_var_inc;
            *sv::<u16>(b::ot_sv_starlib_changeTime) = self.change_time;
            *sv::<bool>(b::ot_sv_starlib_doChange) = self.do_change;
            *sv::<bool>(b::ot_sv_starlib_grayB) = self.gray_b;
            *sv::<i16>(b::ot_sv_starlib_starlib_speed) = self.starlib_speed;
            *sv::<i8>(b::ot_sv_starlib_speedChange) = self.speed_change;
            *sv::<u8>(b::ot_sv_starlib_pColor) = self.p_color;
            *sv::<bool>(b::ot_sv_starlib_JE_starlib_init_initialized) =
                self.je_starlib_init_initialized;
        }
    }
}
