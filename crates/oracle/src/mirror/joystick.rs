//! Mirror of `rustyrian_core::joystick::State` <-> the C globals of
//! `joystick.c`.
//!
//! `joystick` is an upstream heap block: loading reads as many entries as
//! the live block holds (from the shim's allocation registry, so it is
//! independent of `joysticks`); storing frees C's block through the shim and
//! allocates a copy of the Rust one the same way. The pointer value itself
//! is not preserved. A dangling (freed) pointer loads as `None`.

use super::video::{c_heap_free, c_heap_from, c_heap_len, handle_from_c, handle_to_c, sv};
use super::{Mirror, global, global_mut};
use crate::bindings as b;
use rustyrian_core::joystick::{Joystick, JoystickAssignment, JoystickAssignmentTypes, State};
use rustyrian_core::platform::JoystickId;

fn assignment_from_c(c: &b::Joystick_assignment) -> JoystickAssignment {
    JoystickAssignment {
        r#type: JoystickAssignmentTypes::from_c(c.type_).expect("valid assignment type"),
        num: c.num,
        x_axis: c.x_axis,
        negative_axis: c.negative_axis,
    }
}

fn assignment_to_c(r: &JoystickAssignment) -> b::Joystick_assignment {
    b::Joystick_assignment {
        type_: r.r#type.to_c(),
        num: r.num,
        x_axis: r.x_axis,
        negative_axis: r.negative_axis,
    }
}

/// Converts a C `Joystick`.
pub fn joystick_from_c(c: &b::Joystick) -> Joystick {
    Joystick {
        handle: handle_from_c(c.handle).map(JoystickId),
        assignment: c.assignment.map(|a| a.map(|x| assignment_from_c(&x))),
        analog: c.analog,
        sensitivity: c.sensitivity,
        threshold: c.threshold,
        x: c.x,
        y: c.y,
        analog_direction: c.analog_direction,
        direction: c.direction,
        direction_pressed: c.direction_pressed,
        confirm: c.confirm,
        cancel: c.cancel,
        action: c.action,
        action_pressed: c.action_pressed,
        joystick_delay: c.joystick_delay,
        input_pressed: c.input_pressed,
    }
}

/// Converts a core `Joystick`.
pub fn joystick_to_c(r: &Joystick) -> b::Joystick {
    b::Joystick {
        handle: handle_to_c(r.handle.map(|h| h.0)),
        assignment: r.assignment.map(|a| a.map(|x| assignment_to_c(&x))),
        analog: r.analog,
        sensitivity: r.sensitivity,
        threshold: r.threshold,
        x: r.x,
        y: r.y,
        analog_direction: r.analog_direction,
        direction: r.direction,
        direction_pressed: r.direction_pressed,
        confirm: r.confirm,
        cancel: r.cancel,
        action: r.action,
        action_pressed: r.action_pressed,
        joystick_delay: r.joystick_delay,
        input_pressed: r.input_pressed,
    }
}

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make; a live
        // `joystick` block holds `c_heap_len` entries.
        unsafe {
            self.joystick_repeat_delay = *global(&raw const b::joystick_repeat_delay);
            self.joydown = *global(&raw const b::joydown);
            self.ignore_joystick = *global(&raw const b::ignore_joystick);
            self.joysticks = *global(&raw const b::joysticks);
            let p = *global(&raw const b::joystick);
            self.joystick = c_heap_len(p).map(|n| {
                std::slice::from_raw_parts(p, n)
                    .iter()
                    .map(joystick_from_c)
                    .collect()
            });
            self.assignment_to_code_name =
                *sv::<[u8; 7]>(b::ot_sv_joystick_assignment_to_code_name);
        }
    }

    fn store_to_c(&self) {
        // SAFETY: as in `load_from_c`; the C block came from the shim
        // allocator (upstream or this mirror).
        unsafe {
            *global_mut(&raw mut b::joystick_repeat_delay) = self.joystick_repeat_delay;
            *global_mut(&raw mut b::joydown) = self.joydown;
            *global_mut(&raw mut b::ignore_joystick) = self.ignore_joystick;
            *global_mut(&raw mut b::joysticks) = self.joysticks;
            let p = global_mut(&raw mut b::joystick);
            c_heap_free(*p);
            *p = match &self.joystick {
                None => std::ptr::null_mut(),
                Some(v) => c_heap_from(&v.iter().map(joystick_to_c).collect::<Vec<_>>()),
            };
            *sv::<[u8; 7]>(b::ot_sv_joystick_assignment_to_code_name) =
                self.assignment_to_code_name;
        }
    }
}
