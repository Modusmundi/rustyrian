// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) The OpenTyrian Development Team
// Copyright (C) rustyrian contributors

//! Port of upstream `joystick.c` / `joystick.h`.

use crate::Game;
use crate::config_file::{
    ConfigString, ConfigValue, NO_YES, config_add_value, config_find_or_add_section,
    config_find_section, config_get_bool_option, config_get_option, config_get_or_set_int_option,
    config_set_bool_option, config_set_int_option, config_set_option, config_set_value,
    config_string_to_cstr, scan_integer,
};
use crate::logging::SDL_LOG_CATEGORY_APPLICATION;
use crate::platform::{Event, JoystickId};
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

/// `SDL_INIT_JOYSTICK`.
pub const SDL_INIT_JOYSTICK: u32 = 0x0000_0200;
/// `SDL_IGNORE`.
pub const SDL_IGNORE: i32 = 0;
/// `SDL_HAT_UP`.
pub const SDL_HAT_UP: i32 = 0x01;
/// `SDL_HAT_RIGHT`.
pub const SDL_HAT_RIGHT: i32 = 0x02;
/// `SDL_HAT_DOWN`.
pub const SDL_HAT_DOWN: i32 = 0x04;
/// `SDL_HAT_LEFT`.
pub const SDL_HAT_LEFT: i32 = 0x08;
/// `SDL_HAT_CENTERED`.
pub const SDL_HAT_CENTERED: i32 = 0x00;

const SDL_SCANCODE_RETURN: u16 = 40;
const SDL_SCANCODE_ESCAPE: u16 = 41;
const SDL_SCANCODE_RIGHT: u16 = 79;
const SDL_SCANCODE_LEFT: u16 = 80;
const SDL_SCANCODE_DOWN: u16 = 81;
const SDL_SCANCODE_UP: u16 = 82;

/// The `joystick` array; upstream indexes it only after `init_joysticks`.
fn sticks(g: &Game) -> &[Joystick] {
    g.joystick
        .joystick
        .as_deref()
        .expect("joystick used before init_joysticks")
}

fn sticks_mut(g: &mut Game) -> &mut [Joystick] {
    g.joystick
        .joystick
        .as_deref_mut()
        .expect("joystick used before init_joysticks")
}

/// C passes `joystick[j].handle` to SDL even when NULL; the fake platform's
/// handle for NULL is id 0.
fn handle_of(j: &Joystick) -> JoystickId {
    j.handle.unwrap_or(JoystickId(0))
}

/// Eliminates axis movement below the threshold.
pub fn joystick_axis_threshold(g: &mut Game, j: i32, mut value: i32) -> i32 {
    debug_assert!(j < g.joystick.joysticks);

    let negative = value < 0;
    if negative {
        value = value.wrapping_neg();
    }

    let threshold = sticks(g)[j as usize].threshold.wrapping_mul(1000);
    if value <= threshold {
        return 0;
    }

    value = value.wrapping_sub(threshold);

    if negative {
        value.wrapping_neg()
    } else {
        value
    }
}

/// Converts joystick axis to sane Tyrian-usable value (based on sensitivity).
pub fn joystick_axis_reduce(g: &mut Game, j: i32, mut value: i32) -> i32 {
    debug_assert!(j < g.joystick.joysticks);

    value = joystick_axis_threshold(g, j, value);

    if value == 0 {
        return 0;
    }

    let divisor = 3000i32.wrapping_sub(200i32.wrapping_mul(sticks(g)[j as usize].sensitivity));
    // C divides by zero for a configured sensitivity of 15 (D-UB-JOYSTICK-SENSITIVITY).
    if divisor == 0 {
        return 0;
    }
    value.wrapping_div(divisor)
}

/// Converts analog joystick axes to an angle; returns false if axes are
/// centered (there is no angle).
pub fn joystick_analog_angle(g: &mut Game, j: i32, angle: &mut f32) -> bool {
    debug_assert!(j < g.joystick.joysticks);

    let (jx, jy) = {
        let s = &sticks(g)[j as usize];
        (s.x, s.y)
    };
    let x = joystick_axis_threshold(g, j, jx) as f32;
    let y = joystick_axis_threshold(g, j, jy) as f32;

    if x != 0.0 {
        *angle += libm::atanf(-y / x);
        *angle = (f64::from(*angle)
            + if x < 0.0 {
                -std::f64::consts::FRAC_PI_2
            } else {
                std::f64::consts::FRAC_PI_2
            }) as f32;
        return true;
    } else if y != 0.0 {
        *angle = (f64::from(*angle) + if y < 0.0 { std::f64::consts::PI } else { 0.0 }) as f32;
        return true;
    }

    false
}

/// Gives back value 0..joystick_analog_max indicating that one of the
/// assigned buttons has been pressed or that one of the assigned axes/hats
/// has been moved in the assigned direction.
pub fn check_assigned(
    g: &mut Game,
    joystick_handle: JoystickId,
    assignment: &[JoystickAssignment; 2],
) -> i32 {
    let mut result = 0;

    for a in assignment {
        let mut temp: i32;

        match a.r#type {
            JoystickAssignmentTypes::None => continue,
            JoystickAssignmentTypes::Axis => {
                temp = i32::from(g.platform.joystick_get_axis(joystick_handle, a.num));

                if a.negative_axis {
                    temp = -temp;
                }
            }
            JoystickAssignmentTypes::Button => {
                temp = if g.platform.joystick_get_button(joystick_handle, a.num) == 1 {
                    JOYSTICK_ANALOG_MAX
                } else {
                    0
                };
            }
            JoystickAssignmentTypes::Hat => {
                temp = i32::from(g.platform.joystick_get_hat(joystick_handle, a.num));

                if a.x_axis {
                    temp &= SDL_HAT_LEFT | SDL_HAT_RIGHT;
                } else {
                    temp &= SDL_HAT_UP | SDL_HAT_DOWN;
                }

                if a.negative_axis {
                    temp &= SDL_HAT_LEFT | SDL_HAT_UP;
                } else {
                    temp &= SDL_HAT_RIGHT | SDL_HAT_DOWN;
                }

                temp = if temp != 0 { JOYSTICK_ANALOG_MAX } else { 0 };
            }
        }

        if temp > result {
            result = temp;
        }
    }

    result
}

/// Updates joystick state.
pub fn poll_joystick(g: &mut Game, j: i32) {
    debug_assert!(j < g.joystick.joysticks);
    let ju = j as usize;

    let Some(handle) = sticks(g)[ju].handle else {
        return;
    };

    g.platform.joystick_update();

    // indicates that a direction/action was pressed since last poll
    sticks_mut(g)[ju].input_pressed = false;

    // indicates that an direction/action has been held long enough to fake a repeat press
    let repeat = sticks(g)[ju].joystick_delay < g.platform.get_ticks();

    // update direction state
    for d in 0..4 {
        let old = sticks(g)[ju].direction[d];

        let assignment = sticks(g)[ju].assignment[d];
        let analog = check_assigned(g, handle, &assignment);
        let s = &mut g.joystick.joystick.as_deref_mut().expect("joystick")[ju];
        s.analog_direction[d] = analog;
        s.direction[d] = s.analog_direction[d] > (JOYSTICK_ANALOG_MAX / 2);
        g.joystick.joydown |= s.direction[d];

        s.direction_pressed[d] = s.direction[d] && (!old || repeat);
        s.input_pressed |= s.direction_pressed[d];
    }

    {
        let s = &mut sticks_mut(g)[ju];
        s.x = s.analog_direction[3]
            .wrapping_neg()
            .wrapping_add(s.analog_direction[1]);
        s.y = s.analog_direction[0]
            .wrapping_neg()
            .wrapping_add(s.analog_direction[2]);
    }

    // update action state
    for d in 0..6 {
        let old = sticks(g)[ju].action[d];

        let assignment = sticks(g)[ju].assignment[d + 4];
        let pressed = check_assigned(g, handle, &assignment) > (JOYSTICK_ANALOG_MAX / 2);
        let s = &mut g.joystick.joystick.as_deref_mut().expect("joystick")[ju];
        s.action[d] = pressed;
        g.joystick.joydown |= s.action[d];

        s.action_pressed[d] = s.action[d] && (!old || repeat);
        s.input_pressed |= s.action_pressed[d];
    }

    let s = &mut sticks_mut(g)[ju];
    s.confirm = s.action[0] || s.action[4];
    s.cancel = s.action[1] || s.action[5];

    // if new input, reset press-repeat delay
    if s.input_pressed {
        let delay = g.joystick.joystick_repeat_delay as u32;
        let ticks = g.platform.get_ticks();
        sticks_mut(g)[ju].joystick_delay = ticks.wrapping_add(delay);
    }
}

/// Updates all joystick states.
pub fn poll_joysticks(g: &mut Game) {
    g.joystick.joydown = false;

    for j in 0..g.joystick.joysticks {
        poll_joystick(g, j);
    }
}

/// Sends SDL KEYDOWN and KEYUP events for a key.
pub fn push_key(g: &mut Game, key: u16) {
    // `keysym` is zeroed; `repeat` is left uninitialised in C (zero in the
    // oracle).
    g.platform.push_event(&Event::KeyDown {
        scancode: key,
        keycode: 0,
        modifiers: 0,
        repeat: false,
    });

    g.platform.push_event(&Event::KeyUp {
        scancode: key,
        keycode: 0,
        modifiers: 0,
        repeat: false,
    });
}

/// Helps us be lazy by pretending joysticks are a keyboard (useful for menus).
pub fn push_joysticks_as_keyboard(g: &mut Game) {
    let confirm = SDL_SCANCODE_RETURN;
    let cancel = SDL_SCANCODE_ESCAPE;
    let direction = [
        SDL_SCANCODE_UP,
        SDL_SCANCODE_RIGHT,
        SDL_SCANCODE_DOWN,
        SDL_SCANCODE_LEFT,
    ];

    poll_joysticks(g);

    for j in 0..g.joystick.joysticks {
        let s = sticks(g)[j as usize];
        if !s.input_pressed {
            continue;
        }

        if s.confirm {
            push_key(g, confirm);
        }
        if s.cancel {
            push_key(g, cancel);
        }

        for (d, &pressed) in s.direction_pressed.iter().enumerate() {
            if pressed {
                push_key(g, direction[d]);
            }
        }
    }
}

/// Initializes SDL joystick system and loads assignments for joysticks found.
pub fn init_joysticks(g: &mut Game) {
    if g.joystick.ignore_joystick {
        return;
    }

    if g.platform.init_sub_system(SDL_INIT_JOYSTICK) != 0 {
        let e = g.platform.get_error();
        g.platform.log_warn(
            SDL_LOG_CATEGORY_APPLICATION,
            &format!("Failed to initialize SDL joystick: {e}"),
        );
        g.joystick.ignore_joystick = true;
        return;
    }

    g.platform.joystick_event_state(SDL_IGNORE);

    g.joystick.joysticks = g.platform.num_joysticks();
    // malloc(joysticks * sizeof *joystick): a negative count is a failed
    // allocation; malloc(0) is non-NULL.
    g.joystick.joystick = usize::try_from(g.joystick.joysticks)
        .ok()
        .map(|n| vec![Joystick::default(); n]);

    for j in 0..g.joystick.joysticks {
        sticks_mut(g)[j as usize] = Joystick::default();

        let handle = g.platform.joystick_open(j);
        sticks_mut(g)[j as usize].handle = handle;
        if let Some(h) = handle {
            let name = g.platform.joystick_name(h);
            let axes = g.platform.joystick_num_axes(h);
            let buttons = g.platform.joystick_num_buttons(h);
            let hats = g.platform.joystick_num_hats(h);
            g.platform.log(&format!(
                "Joystick detected: {} ({axes} axes, {buttons} buttons, {hats} hats)",
                name.as_deref().unwrap_or("(null)")
            ));

            if !load_joystick_assignments(g, j) {
                reset_joystick_assignments(g, j);
            }
        }
    }

    if g.joystick.joysticks == 0 {
        g.platform.log("No joysticks detected.");
    }
}

/// Deinitializes SDL joystick system and saves joystick assignments.
pub fn deinit_joysticks(g: &mut Game) {
    if g.joystick.ignore_joystick {
        return;
    }

    for j in 0..g.joystick.joysticks {
        if let Some(h) = sticks(g)[j as usize].handle {
            save_joystick_assignments(g, j);
            g.platform.joystick_close(h);
        }
    }

    // free(joystick); C leaves the pointer dangling.
    g.joystick.joystick = None;

    g.platform.quit_sub_system(SDL_INIT_JOYSTICK);
}

pub fn reset_joystick_assignments(g: &mut Game, j: i32) {
    debug_assert!(j < g.joystick.joysticks);
    let ju = j as usize;
    let handle = handle_of(&sticks(g)[ju]);

    // defaults: first 2 axes, first hat, first 6 buttons
    for a in 0..10usize {
        // clear assignments
        for i in 0..2 {
            sticks_mut(g)[ju].assignment[a][i].r#type = JoystickAssignmentTypes::None;
        }

        if a < 4 {
            if g.platform.joystick_num_axes(handle) >= 2 {
                let x = &mut sticks_mut(g)[ju].assignment[a][0];
                x.r#type = JoystickAssignmentTypes::Axis;
                x.num = ((a + 1) % 2) as i32;
                x.negative_axis = a == 0 || a == 3;
            }

            if g.platform.joystick_num_hats(handle) >= 1 {
                let x = &mut sticks_mut(g)[ju].assignment[a][1];
                x.r#type = JoystickAssignmentTypes::Hat;
                x.num = 0;
                x.x_axis = a == 1 || a == 3;
                x.negative_axis = a == 0 || a == 3;
            }
        } else if ((a - 4) as u32) < g.platform.joystick_num_buttons(handle) as u32 {
            let x = &mut sticks_mut(g)[ju].assignment[a][0];
            x.r#type = JoystickAssignmentTypes::Button;
            x.num = (a - 4) as i32;
        }
    }

    let s = &mut sticks_mut(g)[ju];
    s.analog = false;
    s.sensitivity = 5;
    s.threshold = 5;
}

/// `load_joystick_assignments(&opentyrian_config, j)`: upstream's only
/// config is `opentyrian_config`.
pub fn load_joystick_assignments(g: &mut Game, j: i32) -> bool {
    let ju = j as usize;
    let name = g.platform.joystick_name(handle_of(&sticks(g)[ju]));
    let Some(section) = config_find_section(
        &mut g.config.opentyrian_config,
        b"joystick",
        name.as_deref().map(str::as_bytes),
    ) else {
        return false;
    };
    let s = &mut g.joystick.joystick.as_deref_mut().expect("joystick")[ju];

    if !config_get_bool_option(section, b"analog", &mut s.analog) {
        s.analog = false;
    }

    s.sensitivity = config_get_or_set_int_option(section, b"sensitivity", 5);

    s.threshold = config_get_or_set_int_option(section, b"threshold", 5);

    for (a, key) in ASSIGNMENT_NAMES.iter().enumerate() {
        for i in 0..2 {
            s.assignment[a][i].r#type = JoystickAssignmentTypes::None;
        }

        let Some(option) = config_get_option(section, key.as_bytes()) else {
            continue;
        };

        // foreach_option_i_value: every value that is not NULL.
        let values: &[ConfigString] = match &option.v {
            ConfigValue::Value(v) => std::slice::from_ref(v),
            ConfigValue::Values(v) => v,
        };
        for (i, v) in values.iter().enumerate() {
            let Some(value) = config_string_to_cstr(v) else {
                continue;
            };
            if i >= 2 {
                continue;
            }

            code_to_assignment(&mut s.assignment[a][i], value);
        }
    }

    true
}

/// `save_joystick_assignments(&opentyrian_config, j)`. C exits on out of
/// memory, which cannot happen here.
pub fn save_joystick_assignments(g: &mut Game, j: i32) -> bool {
    let ju = j as usize;
    let name = g.platform.joystick_name(handle_of(&sticks(g)[ju]));
    let section = config_find_or_add_section(
        &mut g.config.opentyrian_config,
        b"joystick",
        name.as_deref().map(str::as_bytes),
    );
    let s = &g.joystick.joystick.as_deref().expect("joystick")[ju];
    let code_name = &mut g.joystick.assignment_to_code_name;

    config_set_bool_option(section, b"analog", s.analog, NO_YES);

    config_set_int_option(section, b"sensitivity", s.sensitivity);

    config_set_int_option(section, b"threshold", s.threshold);

    for (a, key) in ASSIGNMENT_NAMES.iter().enumerate() {
        let mut option = config_set_option(section, key.as_bytes(), None);

        option = config_set_value(option, None);

        for i in 0..2 {
            if s.assignment[a][i].r#type == JoystickAssignmentTypes::None {
                continue;
            }

            option = config_add_value(option, code_into(code_name, &s.assignment[a][i]));
        }
    }

    true
}

/// `snprintf(buffer, buffer_len, "%s", text)`: writes what fits plus a NUL
/// (nothing if `buffer_len` is 0) and returns the full length.
fn snprintf(buffer: &mut [u8], buffer_len: usize, text: &[u8]) -> usize {
    if buffer_len > 0 {
        let n = text.len().min(buffer_len - 1);
        buffer[..n].copy_from_slice(&text[..n]);
        buffer[n] = 0;
    }
    text.len()
}

/// Fills buffer with comma separated list of assigned joystick functions.
pub fn joystick_assignments_to_string(
    g: &mut Game,
    buffer: &mut [u8],
    mut buffer_len: usize,
    assignments: &[JoystickAssignment; 2],
) {
    // strncpy(buffer, "", buffer_len)
    buffer[..buffer_len].fill(0);

    let mut off = 0usize;
    let mut comma = false;
    for a in assignments {
        if a.r#type == JoystickAssignmentTypes::None {
            continue;
        }

        let code = assignment_to_code(g, a);
        let mut text = Vec::with_capacity(8);
        if comma {
            text.extend_from_slice(b", ");
        }
        text.extend_from_slice(&code[..code.iter().position(|&b| b == 0).unwrap_or(7)]);
        let len = snprintf(&mut buffer[off..], buffer_len, &text);
        off += len;
        buffer_len = buffer_len.wrapping_sub(len);

        comma = true;
    }
}

/// `sscanf` input position over a C string.
struct Scan<'a> {
    s: &'a [u8],
    i: usize,
}

impl Scan<'_> {
    fn at(&self) -> u8 {
        self.s.get(self.i).copied().unwrap_or(0)
    }

    /// A whitespace directive: skips any amount of white space.
    fn space(&mut self) {
        while matches!(self.at(), b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r') {
            self.i += 1;
        }
    }

    /// An ordinary character directive.
    fn lit(&mut self, c: u8) -> bool {
        if self.at() == c {
            self.i += 1;
            true
        } else {
            false
        }
    }

    /// `%d`.
    fn int(&mut self) -> Option<i32> {
        let (v, n) = scan_integer(&self.s[self.i..], 10, false)?;
        self.i += n;
        Some(v as i32)
    }

    /// `%c`.
    fn chr(&mut self) -> Option<u8> {
        let c = self.at();
        if c == 0 {
            return None;
        }
        self.i += 1;
        Some(c)
    }
}

/// `sscanf(buffer, " <word> %d%c...")` with `chars` trailing `%c`
/// conversions: the number of conversions and the converted values (each
/// stored only when converted).
fn scan_code(buffer: &[u8], word: &[u8], num: &mut i32, chars: &mut [&mut u8]) -> usize {
    let mut sc = Scan { s: buffer, i: 0 };
    sc.space();
    for &c in word {
        if !sc.lit(c) {
            return 0;
        }
    }
    sc.space();
    let Some(v) = sc.int() else {
        return 0;
    };
    *num = v;
    let mut count = 1;
    for c in chars.iter_mut() {
        let Some(v) = sc.chr() else {
            return count;
        };
        **c = v;
        count += 1;
    }
    count
}

/// Reverse of `assignment_to_code()`; `buffer` is a C string.
pub fn code_to_assignment(assignment: &mut JoystickAssignment, buffer: &[u8]) {
    let buffer = &buffer[..buffer.iter().position(|&b| b == 0).unwrap_or(buffer.len())];
    *assignment = JoystickAssignment::default();

    let mut axis: u8 = 0;
    let mut direction: u8 = 0;

    if scan_code(buffer, b"AX", &mut assignment.num, &mut [&mut direction]) == 2 {
        assignment.r#type = JoystickAssignmentTypes::Axis;
    } else if scan_code(buffer, b"BTN", &mut assignment.num, &mut []) == 1 {
        assignment.r#type = JoystickAssignmentTypes::Button;
    } else if scan_code(
        buffer,
        b"H",
        &mut assignment.num,
        &mut [&mut axis, &mut direction],
    ) == 3
    {
        assignment.r#type = JoystickAssignmentTypes::Hat;
    }

    if assignment.num == 0 {
        assignment.r#type = JoystickAssignmentTypes::None;
    } else {
        assignment.num = assignment.num.wrapping_sub(1);
    }

    assignment.x_axis = axis.eq_ignore_ascii_case(&b'X');
    assignment.negative_axis = direction == b'-';
}

/// `assignment_to_code` writing into its `static char name[7]`; returns
/// the C string (without its NUL).
fn code_into<'a>(name: &'a mut [u8; 7], assignment: &JoystickAssignment) -> &'a [u8] {
    let sign = |neg: bool| if neg { '-' } else { '+' };
    let text = match assignment.r#type {
        JoystickAssignmentTypes::None => String::new(),
        JoystickAssignmentTypes::Axis => format!(
            "AX {}{}",
            assignment.num.wrapping_add(1),
            sign(assignment.negative_axis)
        ),
        JoystickAssignmentTypes::Button => format!("BTN {}", assignment.num.wrapping_add(1)),
        JoystickAssignmentTypes::Hat => format!(
            "H {}{}{}",
            assignment.num.wrapping_add(1),
            if assignment.x_axis { 'X' } else { 'Y' },
            sign(assignment.negative_axis)
        ),
    };
    // strcpy / snprintf(name, sizeof(name), ...): bytes after the NUL keep
    // their previous values.
    let n = text.len().min(6);
    name[..n].copy_from_slice(&text.as_bytes()[..n]);
    name[n] = 0;
    &name[..n]
}

/// Gives the short (6 or less characters) identifier for a joystick
/// assignment; returns the whole `static char name[7]`.
pub fn assignment_to_code(g: &mut Game, assignment: &JoystickAssignment) -> [u8; 7] {
    code_into(&mut g.joystick.assignment_to_code_name, assignment);
    g.joystick.assignment_to_code_name
}

/// Compares relevant parts of joystick assignments for equality.
pub fn joystick_assignment_cmp(a: &JoystickAssignment, b: &JoystickAssignment) -> bool {
    if a.r#type == b.r#type {
        return match a.r#type {
            JoystickAssignmentTypes::None => true,
            JoystickAssignmentTypes::Axis => a.num == b.num && a.negative_axis == b.negative_axis,
            JoystickAssignmentTypes::Button => a.num == b.num,
            JoystickAssignmentTypes::Hat => {
                a.num == b.num && a.x_axis == b.x_axis && a.negative_axis == b.negative_axis
            }
        };
    }

    false
}
