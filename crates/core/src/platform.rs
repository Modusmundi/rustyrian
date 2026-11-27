// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) rustyrian contributors

//! The platform boundary: one method per SDL2/SDL2_net function that upstream
//! OpenTyrian calls and the oracle intercepts (see `crates/oracle/generated/
//! intercepted.txt`), plus `time`/`localtime` (libc) and `log_output` (the
//! function pointer returned by `SDL_LogGetOutputFunction`).
//!
//! Method names drop the `SDL_`/`SDLNet_` prefix and are snake_case
//! (`SDL_GetWindowSize` → `get_window_size`, `SDLNet_UDP_Send` →
//! `net_udp_send`). Semantics and integer types follow SDL; opaque SDL pointers
//! become handles, and C out-parameters stay `&mut` out-parameters so that
//! "not written on failure" behaves as in C.
//!
//! `cargo xtask check-trace` fails if this trait's method set drifts from the
//! inventory.

use serde::{Deserialize, Serialize};

macro_rules! handle {
    ($(#[$m:meta])* $name:ident) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub struct $name(pub u32);
    };
}

handle!(
    /// `SDL_Window *`.
    WindowId
);
handle!(
    /// `SDL_Renderer *`.
    RendererId
);
handle!(
    /// `SDL_Texture *`.
    TextureId
);
handle!(
    /// `SDL_Joystick *`.
    JoystickId
);
handle!(
    /// `UDPsocket`.
    UdpSocketId
);
handle!(
    /// Opaque `SDL_LogOutputFunction` + userdata pair.
    LogOutputFunction
);

/// `SDL_AudioDeviceID`; 0 means failure.
pub type AudioDeviceId = u32;

/// `SDL_Rect`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// The subset of `SDL_Event` that upstream handles or pushes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    WindowFocusGained,
    WindowFocusLost,
    WindowResized {
        w: i32,
        h: i32,
    },
    KeyDown {
        scancode: u16,
        keycode: i32,
        modifiers: u16,
        repeat: bool,
    },
    KeyUp {
        scancode: u16,
        keycode: i32,
        modifiers: u16,
        repeat: bool,
    },
    /// UTF-8 text, at most 31 bytes (`SDL_TEXTINPUTEVENT_TEXT_SIZE` − 1).
    TextInput(String),
    MouseMotion {
        x: i32,
        y: i32,
        xrel: i32,
        yrel: i32,
    },
    MouseButtonDown {
        button: u8,
        x: i32,
        y: i32,
    },
    MouseButtonUp {
        button: u8,
        x: i32,
        y: i32,
    },
    Quit,
}

/// `SDL_AudioSpec` without the callback (passed separately).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioSpec {
    pub freq: i32,
    pub format: u16,
    pub channels: u8,
    pub silence: u8,
    pub samples: u16,
    pub size: u32,
}

/// The audio callback; receives the output buffer in bytes.
pub type AudioCallback = Box<dyn FnMut(&mut [u8]) + Send>;

/// The fields of `SDL_AudioCVT` that callers read or write; `handle` names
/// the platform's converter state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AudioCvt {
    pub needed: i32,
    pub src_format: u16,
    pub dst_format: u16,
    pub rate_incr: f64,
    pub len: i32,
    pub len_cvt: i32,
    pub len_mult: i32,
    pub len_ratio: f64,
    pub handle: u32,
}

/// `IPaddress`: both fields in network byte order, as in SDL_net.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IpAddress {
    pub host: u32,
    pub port: u16,
}

/// `UDPpacket`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UdpPacket {
    pub channel: i32,
    /// `maxlen` bytes.
    pub data: Vec<u8>,
    pub len: i32,
    pub maxlen: i32,
    pub status: i32,
    pub address: IpAddress,
}

/// `struct tm`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tm {
    pub sec: i32,
    pub min: i32,
    pub hour: i32,
    pub mday: i32,
    pub mon: i32,
    pub year: i32,
    pub wday: i32,
    pub yday: i32,
    pub isdst: i32,
}

/// What the engine is showing at a present; read only by instrumentation.
pub struct FrameView<'a> {
    /// The 320×200 8-bit indices of the current `VGAScreen`.
    pub screen: &'a [u8],
    /// `rgb_palette`: colors mapped to the texture format (`SDL_PIXELFORMAT_RGB888`,
    /// `0x00RRGGBB`).
    pub palette: &'a [u32; 256],
    /// Serialised simulation state for `--sim-digest`.
    pub sim: Option<&'a dyn Fn() -> Vec<u8>>,
}

/// One method per intercepted platform function.
pub trait Platform {
    // --- SDL core ---
    fn init(&mut self, flags: u32) -> i32;
    fn init_sub_system(&mut self, flags: u32) -> i32;
    fn quit_sub_system(&mut self, flags: u32);
    fn quit(&mut self);
    fn set_hint(&mut self, name: &str, value: &str) -> bool;
    fn get_error(&mut self) -> String;
    fn get_base_path(&mut self) -> Option<String>;

    // --- timing ---
    fn get_ticks(&mut self) -> u32;
    fn delay(&mut self, ms: u32);

    // --- logging ---
    fn log(&mut self, message: &str);
    fn log_debug(&mut self, category: i32, message: &str);
    fn log_warn(&mut self, category: i32, message: &str);
    fn log_error(&mut self, category: i32, message: &str);
    fn log_get_output_function(&mut self) -> Option<LogOutputFunction>;
    fn log_output(
        &mut self,
        function: LogOutputFunction,
        category: i32,
        priority: i32,
        message: &str,
    );
    fn show_simple_message_box(
        &mut self,
        flags: u32,
        title: &str,
        message: &str,
        window: Option<WindowId>,
    ) -> i32;

    // --- windows and displays ---
    fn create_window(
        &mut self,
        title: &str,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        flags: u32,
    ) -> Option<WindowId>;
    fn destroy_window(&mut self, window: WindowId);
    fn get_window_size(&mut self, window: WindowId, w: Option<&mut i32>, h: Option<&mut i32>);
    fn set_window_size(&mut self, window: WindowId, w: i32, h: i32);
    fn set_window_position(&mut self, window: WindowId, x: i32, y: i32);
    fn set_window_fullscreen(&mut self, window: WindowId, flags: u32) -> i32;
    fn show_window(&mut self, window: WindowId);
    fn get_window_display_index(&mut self, window: WindowId) -> i32;
    fn get_num_video_displays(&mut self) -> i32;
    fn get_display_bounds(&mut self, display_index: i32, rect: &mut Rect) -> i32;

    // --- renderer and textures ---
    fn create_renderer(&mut self, window: WindowId, index: i32, flags: u32) -> Option<RendererId>;
    fn destroy_renderer(&mut self, renderer: RendererId);
    fn set_render_draw_color(&mut self, renderer: RendererId, r: u8, g: u8, b: u8, a: u8) -> i32;
    fn render_clear(&mut self, renderer: RendererId) -> i32;
    fn render_copy(
        &mut self,
        renderer: RendererId,
        texture: TextureId,
        src: Option<Rect>,
        dst: Option<Rect>,
    ) -> i32;
    fn render_flush(&mut self, renderer: RendererId) -> i32;
    fn render_present(&mut self, renderer: RendererId, view: &FrameView<'_>);
    fn create_texture(
        &mut self,
        renderer: RendererId,
        format: u32,
        access: i32,
        w: i32,
        h: i32,
    ) -> Option<TextureId>;
    fn destroy_texture(&mut self, texture: TextureId);
    fn query_texture(
        &mut self,
        texture: TextureId,
        format: Option<&mut u32>,
        access: Option<&mut i32>,
        w: Option<&mut i32>,
        h: Option<&mut i32>,
    ) -> i32;
    /// Returns the writable pixels and the pitch, or `None` where SDL returns −1.
    fn lock_texture(&mut self, texture: TextureId, rect: Option<Rect>) -> Option<(&mut [u8], i32)>;
    fn unlock_texture(&mut self, texture: TextureId);

    // --- events, keyboard, mouse ---
    fn poll_event(&mut self) -> Option<Event>;
    fn push_event(&mut self, event: &Event) -> i32;
    fn start_text_input(&mut self);
    fn stop_text_input(&mut self);
    fn show_cursor(&mut self, toggle: i32) -> i32;
    fn set_relative_mouse_mode(&mut self, enabled: bool) -> i32;
    fn get_scancode_name(&mut self, scancode: i32) -> String;
    fn get_scancode_from_name(&mut self, name: &str) -> i32;

    // --- joysticks ---
    fn num_joysticks(&mut self) -> i32;
    fn joystick_open(&mut self, device_index: i32) -> Option<JoystickId>;
    fn joystick_close(&mut self, joystick: JoystickId);
    fn joystick_name(&mut self, joystick: JoystickId) -> Option<String>;
    fn joystick_num_axes(&mut self, joystick: JoystickId) -> i32;
    fn joystick_num_buttons(&mut self, joystick: JoystickId) -> i32;
    fn joystick_num_hats(&mut self, joystick: JoystickId) -> i32;
    fn joystick_get_axis(&mut self, joystick: JoystickId, axis: i32) -> i16;
    fn joystick_get_button(&mut self, joystick: JoystickId, button: i32) -> u8;
    fn joystick_get_hat(&mut self, joystick: JoystickId, hat: i32) -> u8;
    fn joystick_update(&mut self);
    fn joystick_event_state(&mut self, state: i32) -> i32;

    // --- audio ---
    #[allow(clippy::too_many_arguments)]
    fn open_audio_device(
        &mut self,
        device: Option<&str>,
        is_capture: i32,
        desired: &AudioSpec,
        callback: AudioCallback,
        obtained: &mut AudioSpec,
        allowed_changes: i32,
    ) -> AudioDeviceId;
    fn pause_audio_device(&mut self, device: AudioDeviceId, pause_on: i32);
    fn lock_audio_device(&mut self, device: AudioDeviceId);
    fn unlock_audio_device(&mut self, device: AudioDeviceId);
    fn close_audio_device(&mut self, device: AudioDeviceId);
    #[allow(clippy::too_many_arguments)]
    fn build_audio_cvt(
        &mut self,
        cvt: &mut AudioCvt,
        src_format: u16,
        src_channels: u8,
        src_rate: i32,
        dst_format: u16,
        dst_channels: u8,
        dst_rate: i32,
    ) -> i32;
    /// `buf` holds `cvt.len * cvt.len_mult` bytes; the first `cvt.len` are input.
    fn convert_audio(&mut self, cvt: &mut AudioCvt, buf: &mut [u8]) -> i32;

    // --- SDL_net ---
    fn net_init(&mut self) -> i32;
    fn net_quit(&mut self);
    fn net_get_error(&mut self) -> String;
    fn net_resolve_host(&mut self, address: &mut IpAddress, host: Option<&str>, port: u16) -> i32;
    fn net_udp_open(&mut self, port: u16) -> Option<UdpSocketId>;
    fn net_udp_bind(&mut self, socket: UdpSocketId, channel: i32, address: &IpAddress) -> i32;
    fn net_udp_send(&mut self, socket: UdpSocketId, channel: i32, packet: &mut UdpPacket) -> i32;
    fn net_udp_recv(&mut self, socket: UdpSocketId, packet: &mut UdpPacket) -> i32;
    fn net_alloc_packet(&mut self, size: i32) -> Option<UdpPacket>;
    fn net_free_packet(&mut self, packet: UdpPacket);

    // --- libc ---
    fn time(&mut self) -> i64;
    fn localtime(&mut self, t: i64) -> Tm;
}

/// Unrecorded hooks the instrumentation layer needs from a platform.
pub trait PlatformProbe {
    /// The clock that script timestamps refer to, in ms; not recorded.
    fn probe_now_ms(&mut self) -> u32;
    /// Queues an event as if it came from the OS.
    fn probe_inject_event(&mut self, event: Event);
    /// Applies a scripted joystick state change.
    fn probe_joystick(&mut self, change: JoystickChange);
    /// Bytes of a texture currently locked, before it is unlocked.
    fn probe_locked_texture(&self, texture: TextureId) -> Option<&[u8]>;
    /// Audio samples generated by the last `render_present`, if captured.
    fn probe_take_audio(&mut self) -> Option<Vec<u8>>;
}

/// A scripted virtual-joystick declaration or state change.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum JoystickChange {
    Declare {
        name: String,
        axes: i32,
        buttons: i32,
        hats: i32,
    },
    Axis {
        device: i32,
        axis: i32,
        value: i16,
    },
    Button {
        device: i32,
        button: i32,
        pressed: bool,
    },
    Hat {
        device: i32,
        hat: i32,
        value: u8,
    },
}
