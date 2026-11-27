//! Mirror of `rustyrian_core::keyboard::State` <-> the C globals of
//! `keyboard.c`.

use super::video::sv;
use super::{Mirror, global, global_mut};
use crate::bindings as b;
use rustyrian_core::keyboard::{KeyboardInput, MouseInput, State};

fn kin_from_c(c: &b::KeyboardInput) -> KeyboardInput {
    KeyboardInput {
        sym: c.sym,
        scancode: c.scancode,
        r#mod: c.mod_,
        ch: c.ch,
    }
}

fn kin_to_c(r: &KeyboardInput) -> b::KeyboardInput {
    b::KeyboardInput {
        sym: r.sym,
        scancode: r.scancode,
        mod_: r.r#mod,
        ch: r.ch,
    }
}

fn min_from_c(c: &b::MouseInput) -> MouseInput {
    MouseInput {
        x: c.x,
        y: c.y,
        button: c.button,
    }
}

fn min_to_c(r: &MouseInput) -> b::MouseInput {
    b::MouseInput {
        x: r.x,
        y: r.y,
        button: r.button,
    }
}

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make; each
        // static is read through its own C type.
        unsafe {
            self.esc_pressed = *global(&raw const b::ESCPressed);
            self.window_has_focus = *global(&raw const b::windowHasFocus);
            self.keysactive = *global(&raw const b::keysactive);
            self.lord_key_syms_down = *global(&raw const b::lordKeySymsDown);
            let ki = sv::<[b::KeyboardInput; 32]>(b::ot_sv_keyboard_keyboardInputs);
            for (d, s) in self.keyboard_inputs.iter_mut().zip(ki.iter()) {
                *d = kin_from_c(s);
            }
            self.keyboard_inputs_front = *sv::<usize>(b::ot_sv_keyboard_keyboardInputsFront);
            self.keyboard_inputs_back = *sv::<usize>(b::ot_sv_keyboard_keyboardInputsBack);
            self.keyboard_inputs_count = *sv::<usize>(b::ot_sv_keyboard_keyboardInputsCount);
            self.mouse_x = *global(&raw const b::mouseX);
            self.mouse_y = *global(&raw const b::mouseY);
            self.mouse_buttons_down = *global(&raw const b::mouseButtonsDown);
            let mi = sv::<[b::MouseInput; 4]>(b::ot_sv_keyboard_mouseInputs);
            for (d, s) in self.mouse_inputs.iter_mut().zip(mi.iter()) {
                *d = min_from_c(s);
            }
            self.mouse_inputs_front = *sv::<usize>(b::ot_sv_keyboard_mouseInputsFront);
            self.mouse_inputs_back = *sv::<usize>(b::ot_sv_keyboard_mouseInputsBack);
            self.mouse_inputs_count = *sv::<usize>(b::ot_sv_keyboard_mouseInputsCount);
            self.mouse_has_motion_input = *sv::<bool>(b::ot_sv_keyboard_mouseHasMotionInput);
            self.mouse_relative_enabled = *sv::<bool>(b::ot_sv_keyboard_mouseRelativeEnabled);
            self.mouse_window_x_relative = *sv::<i32>(b::ot_sv_keyboard_mouseWindowXRelative);
            self.mouse_window_y_relative = *sv::<i32>(b::ot_sv_keyboard_mouseWindowYRelative);
        }
    }

    fn store_to_c(&self) {
        // SAFETY: as in `load_from_c`.
        unsafe {
            *global_mut(&raw mut b::ESCPressed) = self.esc_pressed;
            *global_mut(&raw mut b::windowHasFocus) = self.window_has_focus;
            *global_mut(&raw mut b::keysactive) = self.keysactive;
            *global_mut(&raw mut b::lordKeySymsDown) = self.lord_key_syms_down;
            let ki = sv::<[b::KeyboardInput; 32]>(b::ot_sv_keyboard_keyboardInputs);
            for (d, s) in ki.iter_mut().zip(self.keyboard_inputs.iter()) {
                *d = kin_to_c(s);
            }
            *sv::<usize>(b::ot_sv_keyboard_keyboardInputsFront) = self.keyboard_inputs_front;
            *sv::<usize>(b::ot_sv_keyboard_keyboardInputsBack) = self.keyboard_inputs_back;
            *sv::<usize>(b::ot_sv_keyboard_keyboardInputsCount) = self.keyboard_inputs_count;
            *global_mut(&raw mut b::mouseX) = self.mouse_x;
            *global_mut(&raw mut b::mouseY) = self.mouse_y;
            *global_mut(&raw mut b::mouseButtonsDown) = self.mouse_buttons_down;
            let mi = sv::<[b::MouseInput; 4]>(b::ot_sv_keyboard_mouseInputs);
            for (d, s) in mi.iter_mut().zip(self.mouse_inputs.iter()) {
                *d = min_to_c(s);
            }
            *sv::<usize>(b::ot_sv_keyboard_mouseInputsFront) = self.mouse_inputs_front;
            *sv::<usize>(b::ot_sv_keyboard_mouseInputsBack) = self.mouse_inputs_back;
            *sv::<usize>(b::ot_sv_keyboard_mouseInputsCount) = self.mouse_inputs_count;
            *sv::<bool>(b::ot_sv_keyboard_mouseHasMotionInput) = self.mouse_has_motion_input;
            *sv::<bool>(b::ot_sv_keyboard_mouseRelativeEnabled) = self.mouse_relative_enabled;
            *sv::<i32>(b::ot_sv_keyboard_mouseWindowXRelative) = self.mouse_window_x_relative;
            *sv::<i32>(b::ot_sv_keyboard_mouseWindowYRelative) = self.mouse_window_y_relative;
        }
    }
}
