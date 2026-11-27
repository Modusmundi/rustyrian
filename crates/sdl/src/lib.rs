// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) rustyrian contributors

//! `SdlPlatform`: the real implementation of
//! [`rustyrian_core::platform::Platform`] for windowed runs.
//!
//! Every SDL2 method calls the matching raw `sdl2-sys` function (SDL 2.32,
//! bundled and statically linked) with the same argument values, so the
//! semantics — return codes, out-parameters left untouched on failure, the
//! `SDL_GetError` text — are SDL's own:
//!
//! - core, timing, logging, message boxes, windows, displays, renderers,
//!   textures, events, keyboard, mouse, joysticks and audio are real SDL;
//! - opaque SDL pointers (`SDL_Window *`, `SDL_Renderer *`, `SDL_Texture *`,
//!   `SDL_Joystick *`) live in tables keyed by the core's handle ids; an
//!   unknown id is passed to SDL as `NULL`, so SDL reports the error;
//! - the audio callback is boxed and handed to SDL as the userdata of a C
//!   trampoline; SDL runs it on its audio thread until the device is closed
//!   (or the audio subsystem is shut down);
//! - `SDL_BuildAudioCVT`/`SDL_ConvertAudio` use SDL's own converters; each
//!   built `SDL_AudioCVT` (with its filter chain) is kept in a table keyed by
//!   [`AudioCvt::handle`];
//! - `poll_event` returns only the event kinds upstream handles
//!   ([`Event`]); every other SDL event is consumed and skipped, exactly as
//!   upstream's `switch` ignores it.
//!
//! SDL_net is not linked. The `net_*` methods reimplement SDL_net 2.2's UDP
//! layer over [`std::net::UdpSocket`] (non-blocking, IPv4): addresses stay in
//! network byte order, each socket has 32 channels of up to 4 addresses,
//! `net_udp_send` sends to every address bound to the channel (or to the
//! packet's address for channel −1) and returns the number of successful
//! sends, and `net_udp_recv` fills `len`/`status`/`address` and the highest
//! channel whose bound addresses include the sender (−1 if none). Errors go
//! through `SDL_SetError`, which is what SDL_net 2.2's `SDLNet_GetError`
//! reads.
//!
//! `time`/`localtime` are the OS clock and the OS local-time breakdown
//! (`localtime_r`, or `localtime_s` on Windows).
//!
//! # Instrumentation ([`PlatformProbe`])
//!
//! `rustyrian_harness::Instrumented<SdlPlatform>` drives windowed runs with
//! input scripts:
//! - `probe_now_ms` is `SDL_GetTicks`, so script timestamps are real time;
//! - `probe_inject_event` is `SDL_PushEvent` of the converted event (text
//!   events are dropped while text input is inactive, as SDL drops them);
//! - `probe_joystick` drives SDL virtual joysticks
//!   (`SDL_JoystickAttachVirtualEx`, `SDL_JoystickSetVirtual{Axis,Button,Hat}`).
//!   Declared joysticks are attached whenever the joystick subsystem is
//!   initialised (and re-attached after it is restarted), keeping the
//!   scripted state; script device *n* is the *n*-th declared virtual
//!   joystick, whatever its SDL device index;
//! - `probe_locked_texture` returns the bytes of a currently locked texture;
//! - `probe_take_audio` returns `None` (audio goes to the real device).

use rustyrian_core::platform::{
    AudioCallback, AudioCvt, AudioDeviceId, AudioSpec, Event, FrameView, IpAddress, JoystickChange,
    JoystickId, LogOutputFunction, Platform, PlatformProbe, Rect, RendererId, TextureId, Tm,
    UdpPacket, UdpSocketId, WindowId,
};
use sdl2_sys as sys;
use std::collections::BTreeMap;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::io::ErrorKind;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, ToSocketAddrs, UdpSocket};
use std::ptr;

/// `SDLNET_MAX_UDPCHANNELS`.
const MAX_UDP_CHANNELS: usize = 32;
/// `SDLNET_MAX_UDPADDRESSES`.
const MAX_UDP_ADDRESSES: usize = 4;
/// `INADDR_NONE`.
const INADDR_NONE: u32 = u32::MAX;

// `SDL_Scancode` is a C enum (an `int`); sdl2-sys binds it as a Rust enum,
// which cannot carry the out-of-range values C callers may pass or SDL may
// return. These declarations bind the same symbols with the C-level `int`.
unsafe extern "C" {
    #[link_name = "SDL_GetScancodeName"]
    fn sdl_get_scancode_name(scancode: c_int) -> *const c_char;
    #[link_name = "SDL_GetScancodeFromName"]
    fn sdl_get_scancode_from_name(name: *const c_char) -> c_int;
}

type LogOutputFn = unsafe extern "C" fn(*mut c_void, c_int, sys::SDL_LogPriority, *const c_char);

struct Renderer {
    ptr: *mut sys::SDL_Renderer,
    window: u32,
}

struct Texture {
    ptr: *mut sys::SDL_Texture,
    renderer: u32,
    format: u32,
    h: i32,
    /// The pixels and length of the current lock, if locked.
    locked: Option<(*mut u8, usize)>,
}

struct VirtualJoystick {
    name: CString,
    axes: Vec<i16>,
    buttons: Vec<u8>,
    hats: Vec<u8>,
    /// Our own open handle while attached; null otherwise.
    handle: *mut sys::SDL_Joystick,
}

struct NetSocket {
    socket: UdpSocket,
    bindings: [Vec<IpAddress>; MAX_UDP_CHANNELS],
}

/// The real SDL2 platform.
pub struct SdlPlatform {
    next_id: u32,
    windows: BTreeMap<u32, *mut sys::SDL_Window>,
    renderers: BTreeMap<u32, Renderer>,
    textures: BTreeMap<u32, Texture>,
    joysticks: BTreeMap<u32, *mut sys::SDL_Joystick>,
    virtual_joysticks: Vec<VirtualJoystick>,
    log_outputs: BTreeMap<u32, (LogOutputFn, *mut c_void)>,
    /// Boxed callbacks of open audio devices (from `Box::into_raw`).
    audio: BTreeMap<AudioDeviceId, *mut AudioCallback>,
    cvts: BTreeMap<u32, Box<sys::SDL_AudioCVT>>,
    sockets: BTreeMap<u32, NetSocket>,
}

impl Default for SdlPlatform {
    fn default() -> Self {
        Self::new()
    }
}

/// `s` up to its first NUL, as C would see it.
fn c_string(s: &str) -> CString {
    let bytes = s.as_bytes();
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    CString::new(&bytes[..end]).unwrap_or_default()
}

/// # Safety
/// `p` is null or a NUL-terminated string.
unsafe fn string_from(p: *const c_char) -> Option<String> {
    if p.is_null() {
        return None;
    }
    // SAFETY: non-null and NUL-terminated per the contract.
    Some(unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned())
}

fn set_error(message: &str) {
    let message = c_string(message);
    // SAFETY: "%s" with one NUL-terminated string argument.
    unsafe { sys::SDL_SetError(c"%s".as_ptr(), message.as_ptr()) };
}

fn was_init(flags: u32) -> bool {
    // SAFETY: SDL_WasInit only reads SDL's subsystem reference counts.
    unsafe { sys::SDL_WasInit(flags) & flags != 0 }
}

fn sdl_rect(r: Rect) -> sys::SDL_Rect {
    sys::SDL_Rect {
        x: r.x,
        y: r.y,
        w: r.w,
        h: r.h,
    }
}

fn sdl_bool(b: bool) -> sys::SDL_bool {
    if b {
        sys::SDL_bool::SDL_TRUE
    } else {
        sys::SDL_bool::SDL_FALSE
    }
}

fn log_priority(p: i32) -> Option<sys::SDL_LogPriority> {
    use sys::SDL_LogPriority as P;
    Some(match p {
        1 => P::SDL_LOG_PRIORITY_VERBOSE,
        2 => P::SDL_LOG_PRIORITY_DEBUG,
        3 => P::SDL_LOG_PRIORITY_INFO,
        4 => P::SDL_LOG_PRIORITY_WARN,
        5 => P::SDL_LOG_PRIORITY_ERROR,
        6 => P::SDL_LOG_PRIORITY_CRITICAL,
        7 => P::SDL_NUM_LOG_PRIORITIES,
        _ => return None,
    })
}

fn spec_from(s: &sys::SDL_AudioSpec) -> AudioSpec {
    AudioSpec {
        freq: s.freq,
        format: s.format,
        channels: s.channels,
        silence: s.silence,
        samples: s.samples,
        size: s.size,
    }
}

/// Runs the boxed [`AudioCallback`] passed as SDL userdata.
///
/// # Safety
/// `userdata` comes from `Box::into_raw(Box<AudioCallback>)` and stays alive
/// until the device is closed; `stream` holds `len` writable bytes.
unsafe extern "C" fn audio_trampoline(userdata: *mut c_void, stream: *mut u8, len: c_int) {
    // SAFETY: SDL never runs a device's callback concurrently with itself,
    // and the platform touches the box only after closing the device.
    let callback = unsafe { &mut *userdata.cast::<AudioCallback>() };
    let len = usize::try_from(len).unwrap_or(0);
    if stream.is_null() || len == 0 {
        return;
    }
    // SAFETY: SDL passes a buffer of `len` bytes, exclusively ours for the call.
    let buf = unsafe { std::slice::from_raw_parts_mut(stream, len) };
    callback(buf);
}

fn cvt_from(c: &sys::SDL_AudioCVT, handle: u32) -> AudioCvt {
    AudioCvt {
        needed: c.needed,
        src_format: c.src_format,
        dst_format: c.dst_format,
        rate_incr: c.rate_incr,
        len: c.len,
        len_cvt: c.len_cvt,
        len_mult: c.len_mult,
        len_ratio: c.len_ratio,
        handle,
    }
}

fn to_socket_addr(a: &IpAddress) -> SocketAddrV4 {
    SocketAddrV4::new(
        Ipv4Addr::from(a.host.to_ne_bytes()),
        u16::from_be_bytes(a.port.to_ne_bytes()),
    )
}

fn from_socket_addr(a: SocketAddrV4) -> IpAddress {
    IpAddress {
        host: u32::from_ne_bytes(a.ip().octets()),
        port: u16::from_ne_bytes(a.port().to_be_bytes()),
    }
}

/// `inet_addr`, then `gethostbyname`, as SDL_net 2.2's `SDLNet_ResolveHost`.
fn resolve_ipv4(host: &str) -> Option<Ipv4Addr> {
    if let Ok(ip) = host.parse::<Ipv4Addr>() {
        return Some(ip);
    }
    (host, 0).to_socket_addrs().ok()?.find_map(|a| match a {
        SocketAddr::V4(v4) => Some(*v4.ip()),
        SocketAddr::V6(_) => None,
    })
}

/// Converts a polled SDL event; `None` for event kinds upstream ignores.
fn event_from_sdl(ev: &sys::SDL_Event) -> Option<Event> {
    use sys::SDL_EventType as T;
    use sys::SDL_WindowEventID as W;
    // SAFETY: `type_` is valid for every event; each arm reads only integer
    // fields of the union member selected by the type (the scancode, a C
    // enum, through its `u32` representation).
    unsafe {
        let t = ev.type_;
        if t == T::SDL_WINDOWEVENT as u32 {
            let w = ev.window;
            match w.event {
                e if e == W::SDL_WINDOWEVENT_FOCUS_GAINED as u8 => Some(Event::WindowFocusGained),
                e if e == W::SDL_WINDOWEVENT_FOCUS_LOST as u8 => Some(Event::WindowFocusLost),
                e if e == W::SDL_WINDOWEVENT_RESIZED as u8 => Some(Event::WindowResized {
                    w: w.data1,
                    h: w.data2,
                }),
                _ => None,
            }
        } else if t == T::SDL_KEYDOWN as u32 || t == T::SDL_KEYUP as u32 {
            let raw = ptr::addr_of!(ev.key.keysym.scancode).cast::<u32>().read();
            let scancode = u16::try_from(raw).unwrap_or(0);
            let keycode = ev.key.keysym.sym;
            let modifiers = ev.key.keysym.mod_;
            let repeat = ev.key.repeat != 0;
            Some(if t == T::SDL_KEYDOWN as u32 {
                Event::KeyDown {
                    scancode,
                    keycode,
                    modifiers,
                    repeat,
                }
            } else {
                Event::KeyUp {
                    scancode,
                    keycode,
                    modifiers,
                    repeat,
                }
            })
        } else if t == T::SDL_TEXTINPUT as u32 {
            let text = ev.text.text;
            let bytes: Vec<u8> = text
                .iter()
                .take_while(|&&c| c != 0)
                .map(|&c| u8::from_ne_bytes(c.to_ne_bytes()))
                .collect();
            Some(Event::TextInput(
                String::from_utf8_lossy(&bytes).into_owned(),
            ))
        } else if t == T::SDL_MOUSEMOTION as u32 {
            let m = ev.motion;
            Some(Event::MouseMotion {
                x: m.x,
                y: m.y,
                xrel: m.xrel,
                yrel: m.yrel,
            })
        } else if t == T::SDL_MOUSEBUTTONDOWN as u32 || t == T::SDL_MOUSEBUTTONUP as u32 {
            let b = ev.button;
            Some(if t == T::SDL_MOUSEBUTTONDOWN as u32 {
                Event::MouseButtonDown {
                    button: b.button,
                    x: b.x,
                    y: b.y,
                }
            } else {
                Event::MouseButtonUp {
                    button: b.button,
                    x: b.x,
                    y: b.y,
                }
            })
        } else if t == T::SDL_QUIT as u32 {
            Some(Event::Quit)
        } else {
            None
        }
    }
}

/// Builds the SDL event for `e`, the inverse of [`event_from_sdl`] (and the
/// same layout as the oracle shim's `write_event`).
fn event_to_sdl(e: &Event, window_id: u32) -> sys::SDL_Event {
    use sys::SDL_EventType as T;
    use sys::SDL_WindowEventID as W;
    // SAFETY: an all-zero `SDL_Event` is valid (every member is plain data and
    // the scancode enum has a zero variant).
    let mut ev: sys::SDL_Event = unsafe { std::mem::zeroed() };
    match e {
        Event::WindowFocusGained | Event::WindowFocusLost | Event::WindowResized { .. } => {
            let (event, data1, data2) = match *e {
                Event::WindowFocusGained => (W::SDL_WINDOWEVENT_FOCUS_GAINED, 0, 0),
                Event::WindowFocusLost => (W::SDL_WINDOWEVENT_FOCUS_LOST, 0, 0),
                Event::WindowResized { w, h } => (W::SDL_WINDOWEVENT_RESIZED, w, h),
                _ => (W::SDL_WINDOWEVENT_NONE, 0, 0),
            };
            ev.window = sys::SDL_WindowEvent {
                type_: T::SDL_WINDOWEVENT as u32,
                timestamp: 0,
                windowID: window_id,
                event: event as u8,
                padding1: 0,
                padding2: 0,
                padding3: 0,
                data1,
                data2,
            };
        }
        Event::KeyDown {
            scancode,
            keycode,
            modifiers,
            repeat,
        }
        | Event::KeyUp {
            scancode,
            keycode,
            modifiers,
            repeat,
        } => {
            let down = matches!(e, Event::KeyDown { .. });
            ev.key = sys::SDL_KeyboardEvent {
                type_: if down {
                    T::SDL_KEYDOWN as u32
                } else {
                    T::SDL_KEYUP as u32
                },
                timestamp: 0,
                windowID: window_id,
                state: if down {
                    sys::SDL_PRESSED as u8
                } else {
                    sys::SDL_RELEASED as u8
                },
                repeat: u8::from(*repeat),
                padding2: 0,
                padding3: 0,
                keysym: sys::SDL_Keysym {
                    scancode: sys::SDL_Scancode::SDL_SCANCODE_UNKNOWN,
                    sym: *keycode,
                    mod_: *modifiers,
                    unused: 0,
                },
            };
            // SAFETY: the scancode is a C enum stored as `u32`; writing its raw
            // value through the union keeps any value C could hold.
            unsafe {
                ptr::addr_of_mut!(ev.key.keysym.scancode)
                    .cast::<u32>()
                    .write(u32::from(*scancode));
            }
        }
        Event::TextInput(text) => {
            let mut buf: [c_char; 32] = [0; 32];
            let bytes = text.as_bytes();
            let n = bytes.len().min(31);
            for (d, s) in buf.iter_mut().zip(&bytes[..n]) {
                *d = c_char::from_ne_bytes([*s]);
            }
            ev.text = sys::SDL_TextInputEvent {
                type_: T::SDL_TEXTINPUT as u32,
                timestamp: 0,
                windowID: window_id,
                text: buf,
            };
        }
        Event::MouseMotion { x, y, xrel, yrel } => {
            ev.motion = sys::SDL_MouseMotionEvent {
                type_: T::SDL_MOUSEMOTION as u32,
                timestamp: 0,
                windowID: window_id,
                which: 0,
                state: 0,
                x: *x,
                y: *y,
                xrel: *xrel,
                yrel: *yrel,
            };
        }
        Event::MouseButtonDown { button, x, y } | Event::MouseButtonUp { button, x, y } => {
            let down = matches!(e, Event::MouseButtonDown { .. });
            ev.button = sys::SDL_MouseButtonEvent {
                type_: if down {
                    T::SDL_MOUSEBUTTONDOWN as u32
                } else {
                    T::SDL_MOUSEBUTTONUP as u32
                },
                timestamp: 0,
                windowID: window_id,
                which: 0,
                button: *button,
                state: if down {
                    sys::SDL_PRESSED as u8
                } else {
                    sys::SDL_RELEASED as u8
                },
                clicks: 1,
                padding1: 0,
                x: *x,
                y: *y,
            };
        }
        Event::Quit => ev.type_ = T::SDL_QUIT as u32,
    }
    ev
}

fn tm_from(t: &libc::tm) -> Tm {
    Tm {
        sec: t.tm_sec,
        min: t.tm_min,
        hour: t.tm_hour,
        mday: t.tm_mday,
        mon: t.tm_mon,
        year: t.tm_year,
        wday: t.tm_wday,
        yday: t.tm_yday,
        isdst: t.tm_isdst,
    }
}

impl SdlPlatform {
    pub fn new() -> Self {
        SdlPlatform {
            next_id: 1,
            windows: BTreeMap::new(),
            renderers: BTreeMap::new(),
            textures: BTreeMap::new(),
            joysticks: BTreeMap::new(),
            virtual_joysticks: Vec::new(),
            log_outputs: BTreeMap::new(),
            audio: BTreeMap::new(),
            cvts: BTreeMap::new(),
            sockets: BTreeMap::new(),
        }
    }

    fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        id
    }

    fn window(&self, w: WindowId) -> *mut sys::SDL_Window {
        self.windows.get(&w.0).copied().unwrap_or(ptr::null_mut())
    }

    fn renderer(&self, r: RendererId) -> *mut sys::SDL_Renderer {
        self.renderers.get(&r.0).map_or(ptr::null_mut(), |r| r.ptr)
    }

    fn texture(&self, t: TextureId) -> *mut sys::SDL_Texture {
        self.textures.get(&t.0).map_or(ptr::null_mut(), |t| t.ptr)
    }

    fn joystick(&self, j: JoystickId) -> *mut sys::SDL_Joystick {
        self.joysticks.get(&j.0).copied().unwrap_or(ptr::null_mut())
    }

    /// The SDL id of the first open window, for injected events.
    fn event_window_id(&self) -> u32 {
        self.windows.values().next().map_or(0, |&w| {
            // SAFETY: `w` is a live window from SDL_CreateWindow.
            unsafe { sys::SDL_GetWindowID(w) }
        })
    }

    /// Frees the callback box of an audio device SDL has closed.
    fn free_audio(&mut self, device: AudioDeviceId) {
        if let Some(cb) = self.audio.remove(&device) {
            // SAFETY: from Box::into_raw in open_audio_device; SDL has closed
            // the device, so its audio thread no longer uses the box.
            drop(unsafe { Box::from_raw(cb) });
        }
    }

    /// Drops handles that SDL freed when a subsystem shut down.
    fn forget_quit_subsystems(&mut self) {
        if !was_init(sys::SDL_INIT_VIDEO) {
            // SDL_VideoQuit destroys every window, and with it each
            // renderer's textures (the renderers stay allocated).
            self.windows.clear();
            self.textures.clear();
        }
        if !was_init(sys::SDL_INIT_JOYSTICK) {
            // SDL_JoystickQuit closes every joystick and detaches virtual ones.
            self.joysticks.clear();
            for vj in &mut self.virtual_joysticks {
                vj.handle = ptr::null_mut();
            }
        }
        if !was_init(sys::SDL_INIT_AUDIO) {
            // SDL_AudioQuit closes every device.
            let devices: Vec<_> = self.audio.keys().copied().collect();
            for d in devices {
                self.free_audio(d);
            }
        }
    }

    /// Attaches declared virtual joysticks while the joystick subsystem runs.
    fn sync_virtual_joysticks(&mut self) {
        if !was_init(sys::SDL_INIT_JOYSTICK) {
            return;
        }
        for vj in &mut self.virtual_joysticks {
            if !vj.handle.is_null() {
                continue;
            }
            // SAFETY: an all-zero descriptor is valid (null pointers, no
            // callbacks); `name` outlives the call (SDL copies it).
            let mut desc: sys::SDL_VirtualJoystickDesc = unsafe { std::mem::zeroed() };
            desc.version = sys::SDL_VIRTUAL_JOYSTICK_DESC_VERSION as u16;
            desc.type_ = sys::SDL_JoystickType::SDL_JOYSTICK_TYPE_UNKNOWN as u16;
            desc.naxes = u16::try_from(vj.axes.len()).unwrap_or(u16::MAX);
            desc.nbuttons = u16::try_from(vj.buttons.len()).unwrap_or(u16::MAX);
            desc.nhats = u16::try_from(vj.hats.len()).unwrap_or(u16::MAX);
            desc.name = vj.name.as_ptr();
            // SAFETY: `desc` is a valid, versioned descriptor.
            let index = unsafe { sys::SDL_JoystickAttachVirtualEx(&desc) };
            if index < 0 {
                continue;
            }
            // SAFETY: `index` is the device index SDL just returned.
            vj.handle = unsafe { sys::SDL_JoystickOpen(index) };
            if vj.handle.is_null() {
                continue;
            }
            for (i, &v) in (0..).zip(&vj.axes) {
                // SAFETY: `handle` is an open virtual joystick with these many
                // axes (and, below, buttons and hats).
                unsafe { sys::SDL_JoystickSetVirtualAxis(vj.handle, i, v) };
            }
            for (i, &v) in (0..).zip(&vj.buttons) {
                // SAFETY: as above.
                unsafe { sys::SDL_JoystickSetVirtualButton(vj.handle, i, v) };
            }
            for (i, &v) in (0..).zip(&vj.hats) {
                // SAFETY: as above.
                unsafe { sys::SDL_JoystickSetVirtualHat(vj.handle, i, v) };
            }
        }
    }

    fn virtual_joystick(&mut self, device: i32) -> Option<&mut VirtualJoystick> {
        self.virtual_joysticks
            .get_mut(usize::try_from(device).ok()?)
    }
}

impl Drop for SdlPlatform {
    fn drop(&mut self) {
        let devices: Vec<_> = self.audio.keys().copied().collect();
        for d in devices {
            // SAFETY: closing stops the audio thread before the box is freed;
            // an id SDL no longer knows only sets an error.
            unsafe { sys::SDL_CloseAudioDevice(d) };
            self.free_audio(d);
        }
    }
}

impl PlatformProbe for SdlPlatform {
    fn probe_now_ms(&mut self) -> u32 {
        // SAFETY: SDL_GetTicks has no preconditions.
        unsafe { sys::SDL_GetTicks() }
    }

    fn probe_inject_event(&mut self, event: Event) {
        // SDL drops text events while text input is inactive.
        // SAFETY: SDL_IsTextInputActive has no preconditions.
        let text_active = unsafe { sys::SDL_IsTextInputActive() } == sys::SDL_bool::SDL_TRUE;
        if matches!(event, Event::TextInput(_)) && !text_active {
            return;
        }
        let mut ev = event_to_sdl(&event, self.event_window_id());
        // SAFETY: `ev` is a fully initialised event.
        unsafe { sys::SDL_PushEvent(&mut ev) };
    }

    fn probe_joystick(&mut self, change: JoystickChange) {
        let len = |n: i32| usize::try_from(n).unwrap_or(0);
        match change {
            JoystickChange::Declare {
                name,
                axes,
                buttons,
                hats,
            } => {
                self.virtual_joysticks.push(VirtualJoystick {
                    name: c_string(&name),
                    axes: vec![0; len(axes)],
                    buttons: vec![0; len(buttons)],
                    hats: vec![0; len(hats)],
                    handle: ptr::null_mut(),
                });
                self.sync_virtual_joysticks();
            }
            JoystickChange::Axis {
                device,
                axis,
                value,
            } => {
                if let Some(vj) = self.virtual_joystick(device)
                    && let Some(slot) = usize::try_from(axis).ok().and_then(|i| vj.axes.get_mut(i))
                {
                    *slot = value;
                    if !vj.handle.is_null() {
                        // SAFETY: `handle` is an open virtual joystick.
                        unsafe { sys::SDL_JoystickSetVirtualAxis(vj.handle, axis, value) };
                    }
                }
            }
            JoystickChange::Button {
                device,
                button,
                pressed,
            } => {
                if let Some(vj) = self.virtual_joystick(device)
                    && let Some(slot) = usize::try_from(button)
                        .ok()
                        .and_then(|i| vj.buttons.get_mut(i))
                {
                    *slot = u8::from(pressed);
                    if !vj.handle.is_null() {
                        // SAFETY: `handle` is an open virtual joystick.
                        unsafe {
                            sys::SDL_JoystickSetVirtualButton(vj.handle, button, u8::from(pressed))
                        };
                    }
                }
            }
            JoystickChange::Hat { device, hat, value } => {
                if let Some(vj) = self.virtual_joystick(device)
                    && let Some(slot) = usize::try_from(hat).ok().and_then(|i| vj.hats.get_mut(i))
                {
                    *slot = value;
                    if !vj.handle.is_null() {
                        // SAFETY: `handle` is an open virtual joystick.
                        unsafe { sys::SDL_JoystickSetVirtualHat(vj.handle, hat, value) };
                    }
                }
            }
        }
    }

    fn probe_locked_texture(&self, texture: TextureId) -> Option<&[u8]> {
        let (pixels, len) = self.textures.get(&texture.0)?.locked?;
        // SAFETY: SDL keeps the locked buffer valid until SDL_UnlockTexture,
        // which clears `locked` first; `len` bytes lie inside it.
        Some(unsafe { std::slice::from_raw_parts(pixels, len) })
    }

    fn probe_take_audio(&mut self) -> Option<Vec<u8>> {
        None
    }
}

impl Platform for SdlPlatform {
    fn init(&mut self, flags: u32) -> i32 {
        // SAFETY: SDL_Init accepts any flags.
        let r = unsafe { sys::SDL_Init(flags) };
        self.sync_virtual_joysticks();
        r
    }

    fn init_sub_system(&mut self, flags: u32) -> i32 {
        // SAFETY: SDL_InitSubSystem accepts any flags.
        let r = unsafe { sys::SDL_InitSubSystem(flags) };
        self.sync_virtual_joysticks();
        r
    }

    fn quit_sub_system(&mut self, flags: u32) {
        // SAFETY: SDL_QuitSubSystem accepts any flags.
        unsafe { sys::SDL_QuitSubSystem(flags) };
        self.forget_quit_subsystems();
    }

    fn quit(&mut self) {
        // SAFETY: SDL_Quit has no preconditions.
        unsafe { sys::SDL_Quit() };
        self.forget_quit_subsystems();
    }

    fn set_hint(&mut self, name: &str, value: &str) -> bool {
        let (name, value) = (c_string(name), c_string(value));
        // SAFETY: two NUL-terminated strings.
        unsafe { sys::SDL_SetHint(name.as_ptr(), value.as_ptr()) == sys::SDL_bool::SDL_TRUE }
    }

    fn get_error(&mut self) -> String {
        // SAFETY: SDL_GetError returns a NUL-terminated string.
        unsafe { string_from(sys::SDL_GetError()) }.unwrap_or_default()
    }

    fn get_base_path(&mut self) -> Option<String> {
        // SAFETY: SDL_GetBasePath returns NULL or an SDL_malloc'd string,
        // which is copied before SDL_free.
        unsafe {
            let p = sys::SDL_GetBasePath();
            let s = string_from(p);
            if !p.is_null() {
                sys::SDL_free(p.cast());
            }
            s
        }
    }

    fn get_ticks(&mut self) -> u32 {
        // SAFETY: SDL_GetTicks has no preconditions.
        unsafe { sys::SDL_GetTicks() }
    }

    fn delay(&mut self, ms: u32) {
        // SAFETY: SDL_Delay has no preconditions.
        unsafe { sys::SDL_Delay(ms) }
    }

    fn log(&mut self, message: &str) {
        let m = c_string(message);
        // SAFETY: "%s" with one NUL-terminated string argument.
        unsafe { sys::SDL_Log(c"%s".as_ptr(), m.as_ptr()) }
    }

    fn log_debug(&mut self, category: i32, message: &str) {
        let m = c_string(message);
        // SAFETY: as in `log`.
        unsafe { sys::SDL_LogDebug(category, c"%s".as_ptr(), m.as_ptr()) }
    }

    fn log_warn(&mut self, category: i32, message: &str) {
        let m = c_string(message);
        // SAFETY: as in `log`.
        unsafe { sys::SDL_LogWarn(category, c"%s".as_ptr(), m.as_ptr()) }
    }

    fn log_error(&mut self, category: i32, message: &str) {
        let m = c_string(message);
        // SAFETY: as in `log`.
        unsafe { sys::SDL_LogError(category, c"%s".as_ptr(), m.as_ptr()) }
    }

    fn log_get_output_function(&mut self) -> Option<LogOutputFunction> {
        let mut callback: sys::SDL_LogOutputFunction = None;
        let mut userdata = ptr::null_mut();
        // SAFETY: two valid out-pointers.
        unsafe { sys::SDL_LogGetOutputFunction(&mut callback, &mut userdata) };
        let f = callback?;
        if let Some((&id, _)) = self
            .log_outputs
            .iter()
            .find(|(_, (g, u))| ptr::fn_addr_eq(*g, f) && *u == userdata)
        {
            return Some(LogOutputFunction(id));
        }
        let id = self.alloc_id();
        self.log_outputs.insert(id, (f, userdata));
        Some(LogOutputFunction(id))
    }

    fn log_output(
        &mut self,
        function: LogOutputFunction,
        category: i32,
        priority: i32,
        message: &str,
    ) {
        let Some(&(f, userdata)) = self.log_outputs.get(&function.0) else {
            return;
        };
        let Some(priority) = log_priority(priority) else {
            return;
        };
        let m = c_string(message);
        // SAFETY: the pair SDL_LogGetOutputFunction returned, called with a
        // valid priority and a NUL-terminated message.
        unsafe { f(userdata, category, priority, m.as_ptr()) }
    }

    fn show_simple_message_box(
        &mut self,
        flags: u32,
        title: &str,
        message: &str,
        window: Option<WindowId>,
    ) -> i32 {
        let (title, message) = (c_string(title), c_string(message));
        let window = window.map_or(ptr::null_mut(), |w| self.window(w));
        // SAFETY: NUL-terminated strings and a live window or NULL.
        unsafe { sys::SDL_ShowSimpleMessageBox(flags, title.as_ptr(), message.as_ptr(), window) }
    }

    fn create_window(
        &mut self,
        title: &str,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        flags: u32,
    ) -> Option<WindowId> {
        let title = c_string(title);
        // SAFETY: a NUL-terminated title; SDL validates the rest.
        let p = unsafe { sys::SDL_CreateWindow(title.as_ptr(), x, y, w, h, flags) };
        if p.is_null() {
            return None;
        }
        let id = self.alloc_id();
        self.windows.insert(id, p);
        Some(WindowId(id))
    }

    fn destroy_window(&mut self, window: WindowId) {
        // SAFETY: a live window or NULL (SDL reports the error).
        unsafe { sys::SDL_DestroyWindow(self.window(window)) };
        self.windows.remove(&window.0);
        // SDL destroys the window's renderer resources, textures included.
        let renderers: Vec<u32> = self
            .renderers
            .iter()
            .filter(|(_, r)| r.window == window.0)
            .map(|(&id, _)| id)
            .collect();
        self.textures
            .retain(|_, t| !renderers.contains(&t.renderer));
    }

    fn get_window_size(&mut self, window: WindowId, w: Option<&mut i32>, h: Option<&mut i32>) {
        let w = w.map_or(ptr::null_mut(), ptr::from_mut);
        let h = h.map_or(ptr::null_mut(), ptr::from_mut);
        // SAFETY: a live window or NULL; valid out-pointers or NULL.
        unsafe { sys::SDL_GetWindowSize(self.window(window), w, h) }
    }

    fn set_window_size(&mut self, window: WindowId, w: i32, h: i32) {
        // SAFETY: a live window or NULL.
        unsafe { sys::SDL_SetWindowSize(self.window(window), w, h) }
    }

    fn set_window_position(&mut self, window: WindowId, x: i32, y: i32) {
        // SAFETY: a live window or NULL.
        unsafe { sys::SDL_SetWindowPosition(self.window(window), x, y) }
    }

    fn set_window_fullscreen(&mut self, window: WindowId, flags: u32) -> i32 {
        // SAFETY: a live window or NULL.
        unsafe { sys::SDL_SetWindowFullscreen(self.window(window), flags) }
    }

    fn show_window(&mut self, window: WindowId) {
        // SAFETY: a live window or NULL.
        unsafe { sys::SDL_ShowWindow(self.window(window)) }
    }

    fn get_window_display_index(&mut self, window: WindowId) -> i32 {
        // SAFETY: a live window or NULL.
        unsafe { sys::SDL_GetWindowDisplayIndex(self.window(window)) }
    }

    fn get_num_video_displays(&mut self) -> i32 {
        // SAFETY: no preconditions.
        unsafe { sys::SDL_GetNumVideoDisplays() }
    }

    fn get_display_bounds(&mut self, display_index: i32, rect: &mut Rect) -> i32 {
        let mut r = sdl_rect(*rect);
        // SAFETY: a valid out-rect, written only on success.
        let ret = unsafe { sys::SDL_GetDisplayBounds(display_index, &mut r) };
        *rect = Rect {
            x: r.x,
            y: r.y,
            w: r.w,
            h: r.h,
        };
        ret
    }

    fn create_renderer(&mut self, window: WindowId, index: i32, flags: u32) -> Option<RendererId> {
        // SAFETY: a live window or NULL.
        let p = unsafe { sys::SDL_CreateRenderer(self.window(window), index, flags) };
        if p.is_null() {
            return None;
        }
        let id = self.alloc_id();
        self.renderers.insert(
            id,
            Renderer {
                ptr: p,
                window: window.0,
            },
        );
        Some(RendererId(id))
    }

    fn destroy_renderer(&mut self, renderer: RendererId) {
        // SAFETY: a live renderer or NULL.
        unsafe { sys::SDL_DestroyRenderer(self.renderer(renderer)) };
        self.renderers.remove(&renderer.0);
        // SDL destroys the renderer's textures.
        self.textures.retain(|_, t| t.renderer != renderer.0);
    }

    fn set_render_draw_color(&mut self, renderer: RendererId, r: u8, g: u8, b: u8, a: u8) -> i32 {
        // SAFETY: a live renderer or NULL.
        unsafe { sys::SDL_SetRenderDrawColor(self.renderer(renderer), r, g, b, a) }
    }

    fn render_clear(&mut self, renderer: RendererId) -> i32 {
        // SAFETY: a live renderer or NULL.
        unsafe { sys::SDL_RenderClear(self.renderer(renderer)) }
    }

    fn render_copy(
        &mut self,
        renderer: RendererId,
        texture: TextureId,
        src: Option<Rect>,
        dst: Option<Rect>,
    ) -> i32 {
        let src = src.map(sdl_rect);
        let dst = dst.map(sdl_rect);
        let src_p = src.as_ref().map_or(ptr::null(), ptr::from_ref);
        let dst_p = dst.as_ref().map_or(ptr::null(), ptr::from_ref);
        // SAFETY: live handles or NULL; rects valid or NULL.
        unsafe { sys::SDL_RenderCopy(self.renderer(renderer), self.texture(texture), src_p, dst_p) }
    }

    fn render_flush(&mut self, renderer: RendererId) -> i32 {
        // SAFETY: a live renderer or NULL.
        unsafe { sys::SDL_RenderFlush(self.renderer(renderer)) }
    }

    fn render_present(&mut self, renderer: RendererId, _view: &FrameView<'_>) {
        // SAFETY: a live renderer or NULL.
        unsafe { sys::SDL_RenderPresent(self.renderer(renderer)) }
    }

    fn create_texture(
        &mut self,
        renderer: RendererId,
        format: u32,
        access: i32,
        w: i32,
        h: i32,
    ) -> Option<TextureId> {
        // SAFETY: a live renderer or NULL; SDL validates the rest.
        let p = unsafe { sys::SDL_CreateTexture(self.renderer(renderer), format, access, w, h) };
        if p.is_null() {
            return None;
        }
        let (mut real_format, mut real_h) = (format, h);
        // SAFETY: `p` is the texture just created; valid out-pointers.
        unsafe {
            sys::SDL_QueryTexture(
                p,
                &mut real_format,
                ptr::null_mut(),
                ptr::null_mut(),
                &mut real_h,
            )
        };
        let id = self.alloc_id();
        self.textures.insert(
            id,
            Texture {
                ptr: p,
                renderer: renderer.0,
                format: real_format,
                h: real_h,
                locked: None,
            },
        );
        Some(TextureId(id))
    }

    fn destroy_texture(&mut self, texture: TextureId) {
        // SAFETY: a live texture or NULL.
        unsafe { sys::SDL_DestroyTexture(self.texture(texture)) };
        self.textures.remove(&texture.0);
    }

    fn query_texture(
        &mut self,
        texture: TextureId,
        format: Option<&mut u32>,
        access: Option<&mut i32>,
        w: Option<&mut i32>,
        h: Option<&mut i32>,
    ) -> i32 {
        let format = format.map_or(ptr::null_mut(), ptr::from_mut);
        let access = access.map_or(ptr::null_mut(), ptr::from_mut);
        let w = w.map_or(ptr::null_mut(), ptr::from_mut);
        let h = h.map_or(ptr::null_mut(), ptr::from_mut);
        // SAFETY: a live texture or NULL; valid out-pointers or NULL.
        unsafe { sys::SDL_QueryTexture(self.texture(texture), format, access, w, h) }
    }

    fn lock_texture(&mut self, texture: TextureId, rect: Option<Rect>) -> Option<(&mut [u8], i32)> {
        let r = rect.map(sdl_rect);
        let r_p = r.as_ref().map_or(ptr::null(), ptr::from_ref);
        let mut pixels: *mut c_void = ptr::null_mut();
        let mut pitch: c_int = 0;
        // SAFETY: a live texture or NULL; valid rect or NULL; out-pointers.
        let ret =
            unsafe { sys::SDL_LockTexture(self.texture(texture), r_p, &mut pixels, &mut pitch) };
        if ret != 0 || pixels.is_null() {
            return None;
        }
        let t = self.textures.get_mut(&texture.0)?;
        let row = usize::try_from(pitch).unwrap_or(0);
        let len = match rect {
            None => row * usize::try_from(t.h).unwrap_or(0),
            Some(r) => {
                // Planar (FourCC) formats report the first plane's pitch.
                let bpp = if (t.format >> 28) & 0x0F == 1 {
                    (t.format & 0xFF) as usize
                } else {
                    1
                };
                let rows = usize::try_from(r.h).unwrap_or(0);
                let cols = usize::try_from(r.w).unwrap_or(0);
                if rows == 0 {
                    0
                } else {
                    row * (rows - 1) + cols * bpp
                }
            }
        };
        let pixels = pixels.cast::<u8>();
        t.locked = Some((pixels, len));
        // SAFETY: SDL's lock buffer holds `len` writable bytes until
        // SDL_UnlockTexture; the slice borrows `self` mutably, so no unlock or
        // destroy can happen while it lives.
        Some((
            unsafe { std::slice::from_raw_parts_mut(pixels, len) },
            pitch,
        ))
    }

    fn unlock_texture(&mut self, texture: TextureId) {
        if let Some(t) = self.textures.get_mut(&texture.0) {
            t.locked = None;
        }
        // SAFETY: a live texture or NULL.
        unsafe { sys::SDL_UnlockTexture(self.texture(texture)) }
    }

    fn poll_event(&mut self) -> Option<Event> {
        loop {
            // SAFETY: an all-zero event is valid storage for SDL to fill.
            let mut ev: sys::SDL_Event = unsafe { std::mem::zeroed() };
            // SAFETY: a valid out-event.
            if unsafe { sys::SDL_PollEvent(&mut ev) } == 0 {
                return None;
            }
            if let Some(e) = event_from_sdl(&ev) {
                return Some(e);
            }
        }
    }

    fn push_event(&mut self, event: &Event) -> i32 {
        let mut ev = event_to_sdl(event, self.event_window_id());
        // SAFETY: a fully initialised event.
        unsafe { sys::SDL_PushEvent(&mut ev) }
    }

    fn start_text_input(&mut self) {
        // SAFETY: no preconditions.
        unsafe { sys::SDL_StartTextInput() }
    }

    fn stop_text_input(&mut self) {
        // SAFETY: no preconditions.
        unsafe { sys::SDL_StopTextInput() }
    }

    fn show_cursor(&mut self, toggle: i32) -> i32 {
        // SAFETY: no preconditions.
        unsafe { sys::SDL_ShowCursor(toggle) }
    }

    fn set_relative_mouse_mode(&mut self, enabled: bool) -> i32 {
        // SAFETY: no preconditions.
        unsafe { sys::SDL_SetRelativeMouseMode(sdl_bool(enabled)) }
    }

    fn get_scancode_name(&mut self, scancode: i32) -> String {
        // SAFETY: SDL range-checks the scancode and returns a NUL-terminated
        // string ("" for unnamed or invalid scancodes).
        unsafe { string_from(sdl_get_scancode_name(scancode)) }.unwrap_or_default()
    }

    fn get_scancode_from_name(&mut self, name: &str) -> i32 {
        let name = c_string(name);
        // SAFETY: a NUL-terminated string.
        unsafe { sdl_get_scancode_from_name(name.as_ptr()) }
    }

    fn num_joysticks(&mut self) -> i32 {
        // SAFETY: no preconditions.
        unsafe { sys::SDL_NumJoysticks() }
    }

    fn joystick_open(&mut self, device_index: i32) -> Option<JoystickId> {
        // SAFETY: SDL range-checks the index.
        let p = unsafe { sys::SDL_JoystickOpen(device_index) };
        if p.is_null() {
            return None;
        }
        let id = self.alloc_id();
        self.joysticks.insert(id, p);
        Some(JoystickId(id))
    }

    fn joystick_close(&mut self, joystick: JoystickId) {
        // SAFETY: an open joystick or NULL.
        unsafe { sys::SDL_JoystickClose(self.joystick(joystick)) };
        self.joysticks.remove(&joystick.0);
    }

    fn joystick_name(&mut self, joystick: JoystickId) -> Option<String> {
        // SAFETY: an open joystick or NULL; NULL or a NUL-terminated name.
        unsafe { string_from(sys::SDL_JoystickName(self.joystick(joystick))) }
    }

    fn joystick_num_axes(&mut self, joystick: JoystickId) -> i32 {
        // SAFETY: an open joystick or NULL.
        unsafe { sys::SDL_JoystickNumAxes(self.joystick(joystick)) }
    }

    fn joystick_num_buttons(&mut self, joystick: JoystickId) -> i32 {
        // SAFETY: an open joystick or NULL.
        unsafe { sys::SDL_JoystickNumButtons(self.joystick(joystick)) }
    }

    fn joystick_num_hats(&mut self, joystick: JoystickId) -> i32 {
        // SAFETY: an open joystick or NULL.
        unsafe { sys::SDL_JoystickNumHats(self.joystick(joystick)) }
    }

    fn joystick_get_axis(&mut self, joystick: JoystickId, axis: i32) -> i16 {
        // SAFETY: an open joystick or NULL; SDL range-checks the axis.
        unsafe { sys::SDL_JoystickGetAxis(self.joystick(joystick), axis) }
    }

    fn joystick_get_button(&mut self, joystick: JoystickId, button: i32) -> u8 {
        // SAFETY: an open joystick or NULL; SDL range-checks the button.
        unsafe { sys::SDL_JoystickGetButton(self.joystick(joystick), button) }
    }

    fn joystick_get_hat(&mut self, joystick: JoystickId, hat: i32) -> u8 {
        // SAFETY: an open joystick or NULL; SDL range-checks the hat.
        unsafe { sys::SDL_JoystickGetHat(self.joystick(joystick), hat) }
    }

    fn joystick_update(&mut self) {
        // SAFETY: no preconditions.
        unsafe { sys::SDL_JoystickUpdate() }
    }

    fn joystick_event_state(&mut self, state: i32) -> i32 {
        // SAFETY: no preconditions.
        unsafe { sys::SDL_JoystickEventState(state) }
    }

    fn open_audio_device(
        &mut self,
        device: Option<&str>,
        is_capture: i32,
        desired: &AudioSpec,
        callback: AudioCallback,
        obtained: &mut AudioSpec,
        allowed_changes: i32,
    ) -> AudioDeviceId {
        let device = device.map(c_string);
        let device_p = device.as_ref().map_or(ptr::null(), |d| d.as_ptr());
        let userdata = Box::into_raw(Box::new(callback));
        let want = sys::SDL_AudioSpec {
            freq: desired.freq,
            format: desired.format,
            channels: desired.channels,
            silence: desired.silence,
            samples: desired.samples,
            padding: 0,
            size: desired.size,
            callback: Some(audio_trampoline),
            userdata: userdata.cast(),
        };
        let mut got = sys::SDL_AudioSpec {
            freq: obtained.freq,
            format: obtained.format,
            channels: obtained.channels,
            silence: obtained.silence,
            samples: obtained.samples,
            padding: 0,
            size: obtained.size,
            callback: None,
            userdata: ptr::null_mut(),
        };
        // SAFETY: valid specs; the trampoline's userdata is the boxed
        // callback, freed only after the device is closed.
        let id = unsafe {
            sys::SDL_OpenAudioDevice(device_p, is_capture, &want, &mut got, allowed_changes)
        };
        *obtained = spec_from(&got);
        if id == 0 {
            // SAFETY: SDL failed to open the device and never saw the box run.
            drop(unsafe { Box::from_raw(userdata) });
        } else {
            self.audio.insert(id, userdata);
        }
        id
    }

    fn pause_audio_device(&mut self, device: AudioDeviceId, pause_on: i32) {
        // SAFETY: SDL validates the id.
        unsafe { sys::SDL_PauseAudioDevice(device, pause_on) }
    }

    fn lock_audio_device(&mut self, device: AudioDeviceId) {
        // SAFETY: SDL validates the id.
        unsafe { sys::SDL_LockAudioDevice(device) }
    }

    fn unlock_audio_device(&mut self, device: AudioDeviceId) {
        // SAFETY: SDL validates the id.
        unsafe { sys::SDL_UnlockAudioDevice(device) }
    }

    fn close_audio_device(&mut self, device: AudioDeviceId) {
        // SAFETY: SDL validates the id; closing joins the audio thread.
        unsafe { sys::SDL_CloseAudioDevice(device) };
        self.free_audio(device);
    }

    fn build_audio_cvt(
        &mut self,
        cvt: &mut AudioCvt,
        src_format: u16,
        src_channels: u8,
        src_rate: i32,
        dst_format: u16,
        dst_channels: u8,
        dst_rate: i32,
    ) -> i32 {
        // SAFETY: an all-zero converter is valid (SDL zeroes it anyway).
        let mut c: Box<sys::SDL_AudioCVT> = Box::new(unsafe { std::mem::zeroed() });
        // SAFETY: a valid converter to fill.
        let ret = unsafe {
            sys::SDL_BuildAudioCVT(
                &mut *c,
                src_format,
                src_channels,
                src_rate,
                dst_format,
                dst_channels,
                dst_rate,
            )
        };
        let handle = if cvt.handle != 0 && self.cvts.contains_key(&cvt.handle) {
            cvt.handle
        } else {
            self.alloc_id()
        };
        *cvt = cvt_from(&c, handle);
        self.cvts.insert(handle, c);
        ret
    }

    fn convert_audio(&mut self, cvt: &mut AudioCvt, buf: &mut [u8]) -> i32 {
        let Some(c) = self.cvts.get_mut(&cvt.handle) else {
            set_error("No buffer allocated for conversion");
            return -1;
        };
        let needed =
            usize::try_from(cvt.len).unwrap_or(0) * usize::try_from(cvt.len_mult).unwrap_or(0);
        if cvt.len < 0 || cvt.len_mult < 0 || buf.len() < needed {
            set_error("No buffer allocated for conversion");
            return -1;
        }
        c.needed = cvt.needed;
        c.src_format = cvt.src_format;
        c.dst_format = cvt.dst_format;
        c.rate_incr = cvt.rate_incr;
        c.len = cvt.len;
        c.len_cvt = cvt.len_cvt;
        c.len_mult = cvt.len_mult;
        c.len_ratio = cvt.len_ratio;
        c.buf = buf.as_mut_ptr();
        // SAFETY: a converter built by SDL_BuildAudioCVT whose buffer holds
        // `len * len_mult` bytes.
        let ret = unsafe { sys::SDL_ConvertAudio(&mut **c) };
        c.buf = ptr::null_mut();
        *cvt = cvt_from(c, cvt.handle);
        ret
    }

    fn net_init(&mut self) -> i32 {
        // std::net initialises Winsock itself; SDL_net's SIGPIPE handling
        // concerns TCP only.
        0
    }

    fn net_quit(&mut self) {}

    fn net_get_error(&mut self) -> String {
        // SDL_net 2.2: SDLNet_GetError is SDL_GetError.
        self.get_error()
    }

    fn net_resolve_host(&mut self, address: &mut IpAddress, host: Option<&str>, port: u16) -> i32 {
        let mut ret = 0;
        match host {
            None => address.host = 0,
            Some(h) => {
                let h = c_string(h);
                match resolve_ipv4(&h.to_string_lossy()) {
                    Some(ip) => address.host = u32::from_ne_bytes(ip.octets()),
                    None => {
                        address.host = INADDR_NONE;
                        ret = -1;
                    }
                }
            }
        }
        address.port = u16::from_ne_bytes(port.to_be_bytes());
        ret
    }

    fn net_udp_open(&mut self, port: u16) -> Option<UdpSocketId> {
        let socket = match UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port)) {
            Ok(s) => s,
            Err(_) => {
                set_error("Couldn't bind to local port");
                return None;
            }
        };
        if socket.set_nonblocking(true).is_err() {
            set_error("Couldn't create socket");
            return None;
        }
        // As SDL_net: allow LAN broadcasts and receive 224.0.0.1 multicasts;
        // failures are ignored there too.
        let _ = socket.set_broadcast(true);
        let _ = socket.join_multicast_v4(&Ipv4Addr::new(224, 0, 0, 1), &Ipv4Addr::UNSPECIFIED);
        let id = self.alloc_id();
        self.sockets.insert(
            id,
            NetSocket {
                socket,
                bindings: Default::default(),
            },
        );
        Some(UdpSocketId(id))
    }

    fn net_udp_bind(&mut self, socket: UdpSocketId, channel: i32, address: &IpAddress) -> i32 {
        let Some(s) = self.sockets.get_mut(&socket.0) else {
            set_error("Passed a NULL socket");
            return -1;
        };
        let index = if channel == -1 {
            // The first channel with room; if none, SDL checks the last one.
            s.bindings
                .iter()
                .position(|b| b.len() < MAX_UDP_ADDRESSES)
                .unwrap_or(MAX_UDP_CHANNELS - 1)
        } else {
            match usize::try_from(channel) {
                Ok(c) if c < MAX_UDP_CHANNELS => c,
                _ => {
                    set_error("Invalid channel");
                    return -1;
                }
            }
        };
        let binding = &mut s.bindings[index];
        if binding.len() == MAX_UDP_ADDRESSES {
            set_error("No room for new addresses");
            return -1;
        }
        binding.push(*address);
        i32::try_from(index).unwrap_or(-1)
    }

    fn net_udp_send(&mut self, socket: UdpSocketId, channel: i32, packet: &mut UdpPacket) -> i32 {
        packet.channel = channel;
        let Some(s) = self.sockets.get(&socket.0) else {
            set_error("Passed a NULL socket");
            return 0;
        };
        let destinations: Vec<IpAddress> = match usize::try_from(channel) {
            Err(_) => vec![packet.address],
            // Most recently bound first, as SDL_net iterates.
            Ok(c) => s
                .bindings
                .get(c)
                .map(|b| b.iter().rev().copied().collect())
                .unwrap_or_default(),
        };
        // A negative length makes C's `sendto` fail; a length past the buffer
        // is clamped (C would read past it).
        let Ok(len) = usize::try_from(packet.len) else {
            return 0;
        };
        let data = &packet.data[..len.min(packet.data.len())];
        let mut sent = 0;
        let mut status = None;
        for d in &destinations {
            if let Ok(n) = s.socket.send_to(data, to_socket_addr(d)) {
                status = Some(i32::try_from(n).unwrap_or(i32::MAX));
                sent += 1;
            }
        }
        if let Some(st) = status {
            packet.status = st;
        }
        sent
    }

    fn net_udp_recv(&mut self, socket: UdpSocketId, packet: &mut UdpPacket) -> i32 {
        let Some(s) = self.sockets.get(&socket.0) else {
            return 0;
        };
        let cap = usize::try_from(packet.maxlen)
            .unwrap_or(0)
            .min(packet.data.len());
        loop {
            match s.socket.recv_from(&mut packet.data[..cap]) {
                Ok((n, SocketAddr::V4(from))) => {
                    packet.status = i32::try_from(n).unwrap_or(i32::MAX);
                    packet.len = packet.status;
                    packet.address = from_socket_addr(from);
                    packet.channel = (0..MAX_UDP_CHANNELS)
                        .rev()
                        .find(|&c| s.bindings[c].contains(&packet.address))
                        .map_or(-1, |c| i32::try_from(c).unwrap_or(-1));
                    return 1;
                }
                // An IPv4 socket never reports an IPv6 peer.
                Ok((_, SocketAddr::V6(_))) => return 0,
                Err(e) if e.kind() == ErrorKind::WouldBlock => return 0,
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                // SDL_net: a failed recvfrom marks the packet and keeps reading
                // while the socket is ready.
                Err(_) => {
                    packet.status = -1;
                    packet.len = 0;
                }
            }
        }
    }

    fn net_alloc_packet(&mut self, size: i32) -> Option<UdpPacket> {
        let Ok(n) = usize::try_from(size) else {
            set_error("Out of memory");
            return None;
        };
        Some(UdpPacket {
            channel: 0,
            data: vec![0; n],
            len: 0,
            maxlen: size,
            status: 0,
            address: IpAddress::default(),
        })
    }

    fn net_free_packet(&mut self, _packet: UdpPacket) {}

    fn time(&mut self) -> i64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(d) => i64::try_from(d.as_secs()).unwrap_or(i64::MAX),
            Err(e) => i64::try_from(e.duration().as_secs()).map_or(i64::MIN, |s| -s),
        }
    }

    fn localtime(&mut self, t: i64) -> Tm {
        // C converts the value to `time_t` as is.
        let t = t as libc::time_t;
        // SAFETY: an all-zero `tm` is valid (integers and a null zone pointer).
        let mut out: libc::tm = unsafe { std::mem::zeroed() };
        #[cfg(unix)]
        {
            // SAFETY: valid pointers to a time and an output struct.
            if unsafe { libc::localtime_r(&t, &mut out) }.is_null() {
                return Tm::default();
            }
        }
        #[cfg(windows)]
        {
            // SAFETY: valid pointers to an output struct and a time.
            if unsafe { libc::localtime_s(&mut out, &t) } != 0 {
                return Tm::default();
            }
        }
        tm_from(&out)
    }
}
