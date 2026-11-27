//! Mirror of `rustyrian_core::mouse::State` <-> the C globals of `mouse.c`.

use super::video::sv;
use super::{Mirror, global, global_mut};
use crate::bindings as b;
use rustyrian_core::mouse::{MOUSE_GRAB_SHAPE_LEN, State};

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make; each
        // static is read through its own C type.
        unsafe {
            self.has_mouse = *global(&raw const b::has_mouse);
            self.mouse_has_three_buttons = *global(&raw const b::mouse_has_three_buttons);
            self.mouse_inactive = *global(&raw const b::mouseInactive);
            self.mouse_cursor = *global(&raw const b::mouseCursor);
            self.mouse_grab_x = *sv::<u16>(b::ot_sv_mouse_mouseGrabX);
            self.mouse_grab_y = *sv::<u16>(b::ot_sv_mouse_mouseGrabY);
            self.mouse_grab_shape =
                *sv::<[u8; MOUSE_GRAB_SHAPE_LEN]>(b::ot_sv_mouse_mouseGrabShape);
        }
    }

    fn store_to_c(&self) {
        // SAFETY: as in `load_from_c`.
        unsafe {
            *global_mut(&raw mut b::has_mouse) = self.has_mouse;
            *global_mut(&raw mut b::mouse_has_three_buttons) = self.mouse_has_three_buttons;
            *global_mut(&raw mut b::mouseInactive) = self.mouse_inactive;
            *global_mut(&raw mut b::mouseCursor) = self.mouse_cursor;
            *sv::<u16>(b::ot_sv_mouse_mouseGrabX) = self.mouse_grab_x;
            *sv::<u16>(b::ot_sv_mouse_mouseGrabY) = self.mouse_grab_y;
            *sv::<[u8; MOUSE_GRAB_SHAPE_LEN]>(b::ot_sv_mouse_mouseGrabShape) =
                self.mouse_grab_shape;
        }
    }
}
