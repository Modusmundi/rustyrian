//! Mirror of `rustyrian_core::arg_parse::State` <-> the function-local
//! statics of `arg_parse.c`.

use super::Mirror;
use crate::bindings as b;
use rustyrian_core::arg_parse::State;

fn sv<T>(getter: unsafe extern "C" fn() -> *mut std::ffi::c_void) -> *mut T {
    // SAFETY: the generated getters return the static's address.
    unsafe { getter() }.cast()
}

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make; the
        // statics have the declared C types (`int`, `bool`, `int`, `size_t`).
        unsafe {
            self.parse_args_argn = *sv::<i32>(b::ot_sv_arg_parse_parse_args_argn);
            self.parse_args_no_more_options =
                *sv::<bool>(b::ot_sv_arg_parse_parse_args_no_more_options);
            self.parse_args_first_nonopt = *sv::<i32>(b::ot_sv_arg_parse_parse_args_first_nonopt);
            self.parse_short_opt_offset = *sv::<usize>(b::ot_sv_arg_parse_parse_short_opt_offset);
        }
    }

    fn store_to_c(&self) {
        // SAFETY: as above.
        unsafe {
            *sv::<i32>(b::ot_sv_arg_parse_parse_args_argn) = self.parse_args_argn;
            *sv::<bool>(b::ot_sv_arg_parse_parse_args_no_more_options) =
                self.parse_args_no_more_options;
            *sv::<i32>(b::ot_sv_arg_parse_parse_args_first_nonopt) = self.parse_args_first_nonopt;
            *sv::<usize>(b::ot_sv_arg_parse_parse_short_opt_offset) = self.parse_short_opt_offset;
        }
    }
}
