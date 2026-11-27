// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `joystick.c` / `joystick.h`.

use crate::platform::JoystickId;
use serde::{Deserialize, Serialize};

/// `Joystick_assignment_types`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u32)]
pub enum JoystickAssignmentTypes {
    #[default]
    None = 0,
    Axis = 1,
    Button = 2,
    Hat = 3,
}

impl JoystickAssignmentTypes {
    /// The C enumerator value.
    pub fn to_c(self) -> u32 {
        self as u32
    }

    /// The enumerator with C value `v`, if any.
    pub fn from_c(v: u32) -> Option<JoystickAssignmentTypes> {
        match v {
            0 => Some(JoystickAssignmentTypes::None),
            1 => Some(JoystickAssignmentTypes::Axis),
            2 => Some(JoystickAssignmentTypes::Button),
            3 => Some(JoystickAssignmentTypes::Hat),
            _ => None,
        }
    }
}

/// `Joystick_assignment`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JoystickAssignment {
    pub r#type: JoystickAssignmentTypes,
    pub num: i32,
    /// If hat: x axis, else y axis.
    pub x_axis: bool,
    /// If hat or axis: negative, else positive.
    pub negative_axis: bool,
}

/// `Joystick`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Joystick {
    /// `handle` (`SDL_Joystick *`); `None` is NULL.
    pub handle: Option<JoystickId>,
    /// 0-3: directions, 4-9: actions.
    pub assignment: [[JoystickAssignment; 2]; 10],
    pub analog: bool,
    pub sensitivity: i32,
    pub threshold: i32,
    pub x: i32,
    pub y: i32,
    pub analog_direction: [i32; 4],
    /// Up, right, down, left.
    pub direction: [bool; 4],
    /// For emulating key presses.
    pub direction_pressed: [bool; 4],
    pub confirm: bool,
    pub cancel: bool,
    /// Fire, mode swap, left fire, right fire, menu, pause.
    pub action: [bool; 6],
    pub action_pressed: [bool; 6],
    pub joystick_delay: u32,
    pub input_pressed: bool,
}

/// `joystick_analog_max`.
pub const JOYSTICK_ANALOG_MAX: i32 = 32767;

/// `assignment_names`.
pub const ASSIGNMENT_NAMES: [&str; 10] = [
    "up",
    "right",
    "down",
    "left",
    "fire",
    "change fire",
    "left sidekick",
    "right sidekick",
    "menu",
    "pause",
];

/// Mutable globals of `joystick.c` (rule R3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// Milliseconds, repeat delay for buttons.
    pub joystick_repeat_delay: i32,
    /// Any joystick buttons down, updated by `poll_joysticks()`.
    pub joydown: bool,
    pub ignore_joystick: bool,
    pub joysticks: i32,
    /// `joystick`: the `malloc`ed array of `joysticks` entries; `None` is
    /// NULL. `Some(vec![])` is the non-NULL result of `malloc(0)`.
    pub joystick: Option<Vec<Joystick>>,
    /// `assignment_to_code`'s `static char name[7]`.
    pub assignment_to_code_name: [u8; 7],
}

impl Default for State {
    fn default() -> Self {
        State {
            joystick_repeat_delay: 300,
            joydown: false,
            ignore_joystick: false,
            joysticks: 0,
            joystick: None,
            assignment_to_code_name: [0; 7],
        }
    }
}
