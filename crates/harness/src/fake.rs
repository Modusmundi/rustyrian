// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) rustyrian contributors

//! `FakePlatform`: deterministic fakes for every platform call, shared by the
//! C oracle's shim and the Rust port's headless mode.

use crate::scancode_names::SCANCODE_NAMES;
use rustyrian_core::platform::{
    AudioCallback, AudioCvt, AudioDeviceId, AudioSpec, Event, FrameView, IpAddress, JoystickChange,
    JoystickId, LogOutputFunction, Platform, PlatformProbe, Rect, RendererId, TextureId, Tm,
    UdpPacket, UdpSocketId, WindowId,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::net::{Ipv4Addr, SocketAddrV4, ToSocketAddrs, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// The constant returned by `time()`: 2001-09-09T01:46:40Z.
pub const FAKE_TIME: i64 = 1_000_000_000;

/// Consecutive `get_ticks` calls without a delay or present that abort the run.
pub const STALL_LIMIT: u32 = 1_000_000;

const AUDIO_S8: u16 = 0x8008;
const AUDIO_S16LSB: u16 = 0x8010;
const AUDIO_S16MSB: u16 = 0x9010;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clock {
    Virtual,
    Real,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetBackend {
    Loopback,
    Udp,
}

#[derive(Clone, Debug)]
pub struct FakeConfig {
    pub clock: Clock,
    pub net: NetBackend,
    /// Returned by `get_base_path`; must end in a path separator.
    pub base_path: Option<String>,
    /// Echo log calls to stderr, as SDL's default log output does.
    pub echo_logs: bool,
}

impl Default for FakeConfig {
    fn default() -> Self {
        FakeConfig {
            clock: Clock::Virtual,
            net: NetBackend::Loopback,
            base_path: None,
            echo_logs: false,
        }
    }
}

struct Window {
    w: i32,
    h: i32,
}

struct Texture {
    format: u32,
    access: i32,
    w: i32,
    h: i32,
    pixels: Vec<u8>,
    locked: bool,
}

#[derive(Default)]
struct Joystick {
    name: String,
    axes: Vec<i16>,
    buttons: Vec<u8>,
    hats: Vec<u8>,
}

struct Audio {
    callback: AudioCallback,
    paused: bool,
    freq: i32,
}

/// One datagram in the loopback network.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Datagram {
    pub socket: UdpSocketId,
    pub address: IpAddress,
    pub data: Vec<u8>,
}

/// `SDLNET_MAX_UDPCHANNELS`.
const MAX_UDP_CHANNELS: i32 = 32;
/// `SDLNET_MAX_UDPADDRESSES`.
const MAX_UDP_ADDRESSES: usize = 4;

#[derive(Default)]
struct Sockets {
    udp: BTreeMap<u32, Option<UdpSocket>>,
    /// Addresses bound to `(socket, channel)`, oldest first.
    channels: BTreeMap<(u32, i32), Vec<IpAddress>>,
}

/// Test-side control of a `FakePlatform` that stays reachable after the
/// platform is boxed into a `Game` (injected failures, loopback datagrams).
#[derive(Clone, Debug, Default)]
pub struct FakeHandle(Arc<Mutex<Shared>>);

#[derive(Debug, Default)]
struct Shared {
    /// Per SDL name, countdowns of successful calls before an injected failure.
    failures: BTreeMap<String, Vec<u32>>,
    inbox: VecDeque<Datagram>,
    outbox: Vec<Datagram>,
}

impl FakeHandle {
    fn lock(&self) -> std::sync::MutexGuard<'_, Shared> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Makes the next call to the named SDL function fail.
    pub fn fail_next(&self, sdl_name: &str) {
        self.fail_after(sdl_name, 0);
    }

    /// Makes the call to the named SDL function after `successes` more
    /// successful calls fail.
    pub fn fail_after(&self, sdl_name: &str, successes: u32) {
        self.lock()
            .failures
            .entry(sdl_name.to_owned())
            .or_default()
            .push(successes);
    }

    /// Queues a datagram for `net_udp_recv` (loopback backend).
    pub fn push_datagram(&self, d: Datagram) {
        self.lock().inbox.push_back(d);
    }

    /// Removes and returns the datagrams sent so far (loopback backend).
    pub fn take_sent(&self) -> Vec<Datagram> {
        std::mem::take(&mut self.lock().outbox)
    }

    /// Whether this call to `sdl_name` should fail; advances the countdowns.
    fn take_failure(&self, sdl_name: &str) -> bool {
        let mut s = self.lock();
        let Some(counts) = s.failures.get_mut(sdl_name) else {
            return false;
        };
        let due = counts.iter().position(|c| *c == 0);
        for c in counts.iter_mut() {
            *c = c.saturating_sub(1);
        }
        if let Some(i) = due {
            counts.remove(i);
        }
        if counts.is_empty() {
            s.failures.remove(sdl_name);
        }
        due.is_some()
    }
}

pub struct FakePlatform {
    config: FakeConfig,
    start: Instant,
    virtual_ms: u32,
    ticks_since_progress: u32,
    error: String,
    control: FakeHandle,
    next_id: u32,
    windows: BTreeMap<u32, Window>,
    renderers: BTreeSet<u32>,
    textures: BTreeMap<u32, Texture>,
    events: VecDeque<Event>,
    text_input: bool,
    cursor_shown: bool,
    relative_mouse: bool,
    joysticks: Vec<Joystick>,
    open_joysticks: BTreeMap<u32, usize>,
    pending_joystick: VecDeque<JoystickChange>,
    audio: Option<Audio>,
    audio_generated: u64,
    last_audio: Option<Vec<u8>>,
    sockets: Sockets,
}

impl FakePlatform {
    pub fn new(config: FakeConfig) -> Self {
        let mut events = VecDeque::new();
        // NDEBUG builds pause while unfocused (tyrian2.c:2065-2069).
        events.push_back(Event::WindowFocusGained);
        FakePlatform {
            config,
            start: Instant::now(),
            virtual_ms: 0,
            ticks_since_progress: 0,
            error: String::new(),
            control: FakeHandle::default(),
            next_id: 1,
            windows: BTreeMap::new(),
            renderers: BTreeSet::new(),
            textures: BTreeMap::new(),
            events,
            text_input: true,
            cursor_shown: true,
            relative_mouse: false,
            joysticks: Vec::new(),
            open_joysticks: BTreeMap::new(),
            pending_joystick: VecDeque::new(),
            audio: None,
            audio_generated: 0,
            last_audio: None,
            sockets: Sockets::default(),
        }
    }

    /// Makes the next call to the named SDL function fail.
    pub fn fail_next(&mut self, sdl_name: &str) {
        self.control.fail_next(sdl_name);
    }

    /// A handle for controlling this platform after it is boxed.
    pub fn handle(&self) -> FakeHandle {
        self.control.clone()
    }

    fn fails(&mut self, sdl_name: &str) -> bool {
        if self.control.take_failure(sdl_name) {
            self.error = format!("injected failure: {sdl_name}");
            true
        } else {
            false
        }
    }

    fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn now(&self) -> u32 {
        match self.config.clock {
            Clock::Virtual => self.virtual_ms,
            Clock::Real => {
                u32::try_from(self.start.elapsed().as_millis() & 0xFFFF_FFFF).unwrap_or(0)
            }
        }
    }

    fn progress(&mut self) {
        self.ticks_since_progress = 0;
    }

    fn apply_joystick(&mut self, change: JoystickChange) {
        match change {
            JoystickChange::Declare {
                name,
                axes,
                buttons,
                hats,
            } => self.joysticks.push(Joystick {
                name,
                axes: vec![0; usize::try_from(axes).unwrap_or(0)],
                buttons: vec![0; usize::try_from(buttons).unwrap_or(0)],
                hats: vec![0; usize::try_from(hats).unwrap_or(0)],
            }),
            JoystickChange::Axis {
                device,
                axis,
                value,
            } => {
                if let Some(v) = self.joystick_slot(device, |j| &mut j.axes, axis) {
                    *v = value;
                }
            }
            JoystickChange::Button {
                device,
                button,
                pressed,
            } => {
                if let Some(v) = self.joystick_slot(device, |j| &mut j.buttons, button) {
                    *v = u8::from(pressed);
                }
            }
            JoystickChange::Hat { device, hat, value } => {
                if let Some(v) = self.joystick_slot(device, |j| &mut j.hats, hat) {
                    *v = value;
                }
            }
        }
    }

    fn joystick_slot<T>(
        &mut self,
        device: i32,
        field: impl FnOnce(&mut Joystick) -> &mut Vec<T>,
        index: i32,
    ) -> Option<&mut T> {
        let j = self.joysticks.get_mut(usize::try_from(device).ok()?)?;
        field(j).get_mut(usize::try_from(index).ok()?)
    }

    fn pump_joysticks(&mut self) {
        while let Some(c) = self.pending_joystick.pop_front() {
            self.apply_joystick(c);
        }
    }

    fn open_joystick(&self, j: JoystickId) -> Option<&Joystick> {
        self.open_joysticks
            .get(&j.0)
            .and_then(|&i| self.joysticks.get(i))
    }

    fn generate_audio(&mut self) {
        let now = self.now();
        let Some(audio) = self.audio.as_mut() else {
            self.last_audio = None;
            return;
        };
        let target = u64::from(now) * u64::try_from(audio.freq).unwrap_or(0) / 1000;
        let count = target.saturating_sub(self.audio_generated);
        self.audio_generated = self.audio_generated.max(target);
        let mut buf = vec![0u8; usize::try_from(count * 2).unwrap_or(0)];
        if !audio.paused && !buf.is_empty() {
            (audio.callback)(&mut buf);
        }
        self.last_audio = Some(buf);
    }

    fn udp(&mut self, s: UdpSocketId) -> Option<&UdpSocket> {
        self.sockets.udp.get(&s.0).and_then(Option::as_ref)
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

/// Days-from-civil inverse (Howard Hinnant), for `localtime` in UTC.
pub fn utc_tm(t: i64) -> Tm {
    let days = t.div_euclid(86_400);
    let secs = t.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    let is_leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let cumulative = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let yday =
        cumulative[usize::try_from(m - 1).unwrap_or(0)] + d - 1 + i64::from(is_leap && m > 2);
    let to_i32 = |v: i64| i32::try_from(v).unwrap_or(0);
    Tm {
        sec: to_i32(secs % 60),
        min: to_i32(secs / 60 % 60),
        hour: to_i32(secs / 3600),
        mday: to_i32(d),
        mon: to_i32(m - 1),
        year: to_i32(year - 1900),
        wday: to_i32((days + 4).rem_euclid(7)),
        yday: to_i32(yday),
        isdst: 0,
    }
}

impl PlatformProbe for FakePlatform {
    fn probe_now_ms(&mut self) -> u32 {
        self.now()
    }

    fn probe_inject_event(&mut self, event: Event) {
        // SDL drops text events while text input is stopped.
        if matches!(event, Event::TextInput(_)) && !self.text_input {
            return;
        }
        self.events.push_back(event);
    }

    fn probe_joystick(&mut self, change: JoystickChange) {
        match change {
            JoystickChange::Declare { .. } => self.apply_joystick(change),
            other => self.pending_joystick.push_back(other),
        }
    }

    fn probe_locked_texture(&self, texture: TextureId) -> Option<&[u8]> {
        self.textures.get(&texture.0).map(|t| t.pixels.as_slice())
    }

    fn probe_take_audio(&mut self) -> Option<Vec<u8>> {
        self.last_audio.take()
    }
}

impl Platform for FakePlatform {
    fn init(&mut self, _flags: u32) -> i32 {
        if self.fails("SDL_Init") { -1 } else { 0 }
    }

    fn init_sub_system(&mut self, _flags: u32) -> i32 {
        if self.fails("SDL_InitSubSystem") {
            -1
        } else {
            0
        }
    }

    fn quit_sub_system(&mut self, _flags: u32) {}

    fn quit(&mut self) {}

    fn set_hint(&mut self, _name: &str, _value: &str) -> bool {
        !self.fails("SDL_SetHint")
    }

    fn get_error(&mut self) -> String {
        self.error.clone()
    }

    fn get_base_path(&mut self) -> Option<String> {
        if self.fails("SDL_GetBasePath") {
            return None;
        }
        self.config.base_path.clone()
    }

    fn get_ticks(&mut self) -> u32 {
        if self.config.clock == Clock::Virtual {
            self.ticks_since_progress += 1;
            assert!(
                self.ticks_since_progress < STALL_LIMIT,
                "virtual clock stall: {STALL_LIMIT} SDL_GetTicks calls without SDL_Delay or present"
            );
        }
        self.now()
    }

    fn delay(&mut self, ms: u32) {
        self.progress();
        match self.config.clock {
            Clock::Virtual => self.virtual_ms = self.virtual_ms.wrapping_add(ms),
            Clock::Real => std::thread::sleep(std::time::Duration::from_millis(u64::from(ms))),
        }
    }

    fn log(&mut self, message: &str) {
        if self.config.echo_logs {
            eprintln!("INFO: {message}");
        }
    }

    fn log_debug(&mut self, _category: i32, message: &str) {
        if self.config.echo_logs {
            eprintln!("DEBUG: {message}");
        }
    }

    fn log_warn(&mut self, _category: i32, message: &str) {
        if self.config.echo_logs {
            eprintln!("WARN: {message}");
        }
    }

    fn log_error(&mut self, _category: i32, message: &str) {
        if self.config.echo_logs {
            eprintln!("ERROR: {message}");
        }
    }

    fn log_get_output_function(&mut self) -> Option<LogOutputFunction> {
        Some(LogOutputFunction(1))
    }

    fn log_output(
        &mut self,
        _function: LogOutputFunction,
        _category: i32,
        _priority: i32,
        message: &str,
    ) {
        if self.config.echo_logs {
            eprint!("CRITICAL: {message}");
        }
    }

    fn show_simple_message_box(
        &mut self,
        _flags: u32,
        _title: &str,
        _message: &str,
        _window: Option<WindowId>,
    ) -> i32 {
        if self.fails("SDL_ShowSimpleMessageBox") {
            -1
        } else {
            0
        }
    }

    fn create_window(
        &mut self,
        _title: &str,
        _x: i32,
        _y: i32,
        w: i32,
        h: i32,
        _flags: u32,
    ) -> Option<WindowId> {
        if self.fails("SDL_CreateWindow") {
            return None;
        }
        let id = self.alloc_id();
        self.windows.insert(id, Window { w, h });
        Some(WindowId(id))
    }

    fn destroy_window(&mut self, window: WindowId) {
        self.windows.remove(&window.0);
    }

    fn get_window_size(&mut self, window: WindowId, w: Option<&mut i32>, h: Option<&mut i32>) {
        let (ww, wh) = self.windows.get(&window.0).map_or((0, 0), |x| (x.w, x.h));
        if let Some(w) = w {
            *w = ww;
        }
        if let Some(h) = h {
            *h = wh;
        }
    }

    fn set_window_size(&mut self, window: WindowId, w: i32, h: i32) {
        if let Some(x) = self.windows.get_mut(&window.0) {
            x.w = w;
            x.h = h;
        }
    }

    fn set_window_position(&mut self, _window: WindowId, _x: i32, _y: i32) {}

    fn set_window_fullscreen(&mut self, _window: WindowId, _flags: u32) -> i32 {
        if self.fails("SDL_SetWindowFullscreen") {
            -1
        } else {
            0
        }
    }

    fn show_window(&mut self, _window: WindowId) {}

    fn get_window_display_index(&mut self, _window: WindowId) -> i32 {
        if self.fails("SDL_GetWindowDisplayIndex") {
            -1
        } else {
            0
        }
    }

    fn get_num_video_displays(&mut self) -> i32 {
        if self.fails("SDL_GetNumVideoDisplays") {
            -1
        } else {
            1
        }
    }

    fn get_display_bounds(&mut self, display_index: i32, rect: &mut Rect) -> i32 {
        if display_index != 0 || self.fails("SDL_GetDisplayBounds") {
            self.error = "displayIndex must be in the range 0 - 0".to_owned();
            return -1;
        }
        *rect = Rect {
            x: 0,
            y: 0,
            w: 1920,
            h: 1080,
        };
        0
    }

    fn create_renderer(
        &mut self,
        _window: WindowId,
        _index: i32,
        _flags: u32,
    ) -> Option<RendererId> {
        if self.fails("SDL_CreateRenderer") {
            return None;
        }
        let id = self.alloc_id();
        self.renderers.insert(id);
        Some(RendererId(id))
    }

    fn destroy_renderer(&mut self, renderer: RendererId) {
        self.renderers.remove(&renderer.0);
    }

    fn set_render_draw_color(
        &mut self,
        _renderer: RendererId,
        _r: u8,
        _g: u8,
        _b: u8,
        _a: u8,
    ) -> i32 {
        0
    }

    fn render_clear(&mut self, _renderer: RendererId) -> i32 {
        0
    }

    fn render_copy(
        &mut self,
        _renderer: RendererId,
        _texture: TextureId,
        _src: Option<Rect>,
        _dst: Option<Rect>,
    ) -> i32 {
        0
    }

    fn render_flush(&mut self, _renderer: RendererId) -> i32 {
        0
    }

    fn render_present(&mut self, _renderer: RendererId, _view: &FrameView<'_>) {
        self.progress();
        self.generate_audio();
    }

    fn create_texture(
        &mut self,
        _renderer: RendererId,
        format: u32,
        access: i32,
        w: i32,
        h: i32,
    ) -> Option<TextureId> {
        if self.fails("SDL_CreateTexture") {
            return None;
        }
        let bpp = (format & 0xFF) as usize;
        let size = usize::try_from(w).unwrap_or(0) * usize::try_from(h).unwrap_or(0) * bpp;
        let id = self.alloc_id();
        self.textures.insert(
            id,
            Texture {
                format,
                access,
                w,
                h,
                pixels: vec![0; size],
                locked: false,
            },
        );
        Some(TextureId(id))
    }

    fn destroy_texture(&mut self, texture: TextureId) {
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
        let Some(t) = self.textures.get(&texture.0) else {
            self.error = "Parameter 'texture' is invalid".into();
            return -1;
        };
        if let Some(f) = format {
            *f = t.format;
        }
        if let Some(a) = access {
            *a = t.access;
        }
        if let Some(x) = w {
            *x = t.w;
        }
        if let Some(x) = h {
            *x = t.h;
        }
        0
    }

    fn lock_texture(&mut self, texture: TextureId, rect: Option<Rect>) -> Option<(&mut [u8], i32)> {
        if self.fails("SDL_LockTexture") {
            return None;
        }
        let t = self.textures.get_mut(&texture.0)?;
        let bpp = (t.format & 0xFF) as i32;
        let pitch = t.w * bpp;
        let offset = rect.map_or(0, |r| r.y * pitch + r.x * bpp);
        t.locked = true;
        let offset = usize::try_from(offset).ok()?;
        Some((t.pixels.get_mut(offset..)?, pitch))
    }

    fn unlock_texture(&mut self, texture: TextureId) {
        if let Some(t) = self.textures.get_mut(&texture.0) {
            t.locked = false;
        }
    }

    fn poll_event(&mut self) -> Option<Event> {
        self.pump_joysticks();
        self.events.pop_front()
    }

    fn push_event(&mut self, event: &Event) -> i32 {
        if self.fails("SDL_PushEvent") {
            return -1;
        }
        self.events.push_back(event.clone());
        1
    }

    fn start_text_input(&mut self) {
        self.text_input = true;
    }

    fn stop_text_input(&mut self) {
        self.text_input = false;
    }

    fn show_cursor(&mut self, toggle: i32) -> i32 {
        // SDL_QUERY = -1, SDL_DISABLE = 0, SDL_ENABLE = 1.
        if toggle >= 0 {
            self.cursor_shown = toggle != 0;
        }
        i32::from(self.cursor_shown)
    }

    fn set_relative_mouse_mode(&mut self, enabled: bool) -> i32 {
        self.relative_mouse = enabled;
        0
    }

    fn get_scancode_name(&mut self, scancode: i32) -> String {
        usize::try_from(scancode)
            .ok()
            .and_then(|i| SCANCODE_NAMES.get(i).copied().flatten())
            .unwrap_or("")
            .to_owned()
    }

    fn get_scancode_from_name(&mut self, name: &str) -> i32 {
        if name.is_empty() {
            return 0;
        }
        SCANCODE_NAMES
            .iter()
            .position(|n| n.is_some_and(|n| n.eq_ignore_ascii_case(name)))
            .and_then(|i| i32::try_from(i).ok())
            .unwrap_or(0)
    }

    fn num_joysticks(&mut self) -> i32 {
        i32::try_from(self.joysticks.len()).unwrap_or(i32::MAX)
    }

    fn joystick_open(&mut self, device_index: i32) -> Option<JoystickId> {
        let index = usize::try_from(device_index).ok()?;
        if index >= self.joysticks.len() || self.fails("SDL_JoystickOpen") {
            return None;
        }
        let id = self.alloc_id();
        self.open_joysticks.insert(id, index);
        Some(JoystickId(id))
    }

    fn joystick_close(&mut self, joystick: JoystickId) {
        self.open_joysticks.remove(&joystick.0);
    }

    fn joystick_name(&mut self, joystick: JoystickId) -> Option<String> {
        self.open_joystick(joystick).map(|j| j.name.clone())
    }

    fn joystick_num_axes(&mut self, joystick: JoystickId) -> i32 {
        self.open_joystick(joystick)
            .map_or(-1, |j| i32::try_from(j.axes.len()).unwrap_or(-1))
    }

    fn joystick_num_buttons(&mut self, joystick: JoystickId) -> i32 {
        self.open_joystick(joystick)
            .map_or(-1, |j| i32::try_from(j.buttons.len()).unwrap_or(-1))
    }

    fn joystick_num_hats(&mut self, joystick: JoystickId) -> i32 {
        self.open_joystick(joystick)
            .map_or(-1, |j| i32::try_from(j.hats.len()).unwrap_or(-1))
    }

    fn joystick_get_axis(&mut self, joystick: JoystickId, axis: i32) -> i16 {
        let i = usize::try_from(axis).ok();
        self.open_joystick(joystick)
            .and_then(|j| j.axes.get(i?).copied())
            .unwrap_or(0)
    }

    fn joystick_get_button(&mut self, joystick: JoystickId, button: i32) -> u8 {
        let i = usize::try_from(button).ok();
        self.open_joystick(joystick)
            .and_then(|j| j.buttons.get(i?).copied())
            .unwrap_or(0)
    }

    fn joystick_get_hat(&mut self, joystick: JoystickId, hat: i32) -> u8 {
        let i = usize::try_from(hat).ok();
        self.open_joystick(joystick)
            .and_then(|j| j.hats.get(i?).copied())
            .unwrap_or(0)
    }

    fn joystick_update(&mut self) {
        self.pump_joysticks();
    }

    fn joystick_event_state(&mut self, state: i32) -> i32 {
        // SDL_QUERY returns the current state; SDL_IGNORE/SDL_ENABLE return the new one.
        if state == -1 { 1 } else { state }
    }

    fn open_audio_device(
        &mut self,
        _device: Option<&str>,
        _is_capture: i32,
        _desired: &AudioSpec,
        callback: AudioCallback,
        obtained: &mut AudioSpec,
        _allowed_changes: i32,
    ) -> AudioDeviceId {
        if self.fails("SDL_OpenAudioDevice") {
            return 0;
        }
        *obtained = AudioSpec {
            freq: 44_100,
            format: AUDIO_S16LSB,
            channels: 1,
            silence: 0,
            samples: 1024,
            size: 2048,
        };
        self.audio = Some(Audio {
            callback,
            paused: true,
            freq: obtained.freq,
        });
        self.audio_generated = u64::from(self.now()) * 44_100 / 1000;
        2
    }

    fn pause_audio_device(&mut self, _device: AudioDeviceId, pause_on: i32) {
        if let Some(a) = self.audio.as_mut() {
            a.paused = pause_on != 0;
        }
    }

    fn lock_audio_device(&mut self, _device: AudioDeviceId) {}

    fn unlock_audio_device(&mut self, _device: AudioDeviceId) {}

    fn close_audio_device(&mut self, _device: AudioDeviceId) {
        self.audio = None;
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
        let supported = src_format == AUDIO_S8
            && (dst_format == AUDIO_S16LSB || dst_format == AUDIO_S16MSB)
            && src_channels == 1
            && dst_channels == 1
            && src_rate == 11_025
            && dst_rate > 0
            && dst_rate % 11_025 == 0;
        if !supported || self.fails("SDL_BuildAudioCVT") {
            self.error = "fake SDL_BuildAudioCVT supports only S8 mono 11025 Hz to S16 mono at multiples of 11025 Hz".into();
            return -1;
        }
        let factor = dst_rate / 11_025;
        *cvt = AudioCvt {
            needed: 1,
            src_format,
            dst_format,
            rate_incr: f64::from(dst_rate) / f64::from(src_rate),
            len: 0,
            len_cvt: 0,
            len_mult: 2 * factor,
            len_ratio: f64::from(2 * factor),
            handle: u32::try_from(factor).unwrap_or(0),
        };
        1
    }

    fn convert_audio(&mut self, cvt: &mut AudioCvt, buf: &mut [u8]) -> i32 {
        if self.fails("SDL_ConvertAudio") {
            return -1;
        }
        let factor = usize::try_from(cvt.handle).unwrap_or(0);
        let len = usize::try_from(cvt.len).unwrap_or(0);
        let out_len = len * 2 * factor;
        if factor == 0 || buf.len() < out_len {
            self.error = "fake SDL_ConvertAudio: buffer too small".into();
            return -1;
        }
        // Zero-order hold, in place from the back.
        for i in (0..len).rev() {
            let s = i16::from(buf[i] as i8) << 8;
            let bytes = if cvt.dst_format == AUDIO_S16MSB {
                s.to_be_bytes()
            } else {
                s.to_le_bytes()
            };
            for k in 0..factor {
                let o = (i * factor + k) * 2;
                buf[o..o + 2].copy_from_slice(&bytes);
            }
        }
        cvt.len_cvt = i32::try_from(out_len).unwrap_or(i32::MAX);
        0
    }

    fn net_init(&mut self) -> i32 {
        if self.fails("SDLNet_Init") { -1 } else { 0 }
    }

    fn net_quit(&mut self) {}

    fn net_get_error(&mut self) -> String {
        self.error.clone()
    }

    fn net_resolve_host(&mut self, address: &mut IpAddress, host: Option<&str>, port: u16) -> i32 {
        if self.fails("SDLNet_ResolveHost") {
            return -1;
        }
        // SDL_net 2.2: `inet_addr`, then the resolver; on failure the host is
        // INADDR_NONE, the port is still stored, and no error text is set.
        let mut ret = 0;
        address.host = match host {
            None => 0,
            Some(h) => {
                let ip = h
                    .parse::<Ipv4Addr>()
                    .ok()
                    .filter(|ip| *ip != Ipv4Addr::BROADCAST)
                    .or_else(|| {
                        (h, 0u16).to_socket_addrs().ok().and_then(|mut it| {
                            it.find_map(|a| match a {
                                std::net::SocketAddr::V4(v4) => Some(*v4.ip()),
                                std::net::SocketAddr::V6(_) => None,
                            })
                        })
                    });
                match ip {
                    Some(ip) => u32::from_ne_bytes(ip.octets()),
                    None => {
                        ret = -1;
                        u32::MAX
                    }
                }
            }
        };
        address.port = u16::from_ne_bytes(port.to_be_bytes());
        ret
    }

    fn net_udp_open(&mut self, port: u16) -> Option<UdpSocketId> {
        if self.fails("SDLNet_UDP_Open") {
            return None;
        }
        let socket = match self.config.net {
            NetBackend::Loopback => None,
            NetBackend::Udp => {
                match UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port)) {
                    Ok(s) => {
                        if s.set_nonblocking(true).is_err() {
                            return None;
                        }
                        // SDL_net enables broadcasts and joins 224.0.0.1,
                        // ignoring failures.
                        let _ = s.set_broadcast(true);
                        let _ = s.join_multicast_v4(
                            &Ipv4Addr::new(224, 0, 0, 1),
                            &Ipv4Addr::UNSPECIFIED,
                        );
                        Some(s)
                    }
                    Err(_) => {
                        self.error = "Couldn't bind to local port".into();
                        return None;
                    }
                }
            }
        };
        let id = self.alloc_id();
        self.sockets.udp.insert(id, socket);
        Some(UdpSocketId(id))
    }

    fn net_udp_bind(&mut self, socket: UdpSocketId, channel: i32, address: &IpAddress) -> i32 {
        if !self.sockets.udp.contains_key(&socket.0) {
            self.error = "Passed a NULL socket".into();
            return -1;
        }
        if self.fails("SDLNet_UDP_Bind") {
            return -1;
        }
        let bound = |c: i32| {
            self.sockets
                .channels
                .get(&(socket.0, c))
                .map_or(0, Vec::len)
        };
        let channel = if channel == -1 {
            // The first channel with room (SDL_net leaves `channel` at
            // SDLNET_MAX_UDPCHANNELS when none has room; that binding is out
            // of bounds there, so report no room).
            match (0..MAX_UDP_CHANNELS).find(|c| bound(*c) < MAX_UDP_ADDRESSES) {
                Some(c) => c,
                None => {
                    self.error = "No room for new addresses".into();
                    return -1;
                }
            }
        } else if (0..MAX_UDP_CHANNELS).contains(&channel) {
            channel
        } else {
            self.error = "Invalid channel".into();
            return -1;
        };
        if bound(channel) == MAX_UDP_ADDRESSES {
            self.error = "No room for new addresses".into();
            return -1;
        }
        self.sockets
            .channels
            .entry((socket.0, channel))
            .or_default()
            .push(*address);
        channel
    }

    fn net_udp_send(&mut self, socket: UdpSocketId, channel: i32, packet: &mut UdpPacket) -> i32 {
        if self.fails("SDLNet_UDP_Send") {
            return 0;
        }
        packet.channel = channel;
        if !self.sockets.udp.contains_key(&socket.0) {
            self.error = "Passed a NULL socket".into();
            return 0;
        }
        let len = usize::try_from(packet.len)
            .unwrap_or(0)
            .min(packet.data.len());
        // A channel < 0 sends to the packet's address; otherwise to every
        // address bound to the channel, most recent first. (A channel >= 32
        // is out of bounds in SDL_net; it sends nowhere here.)
        let dests: Vec<IpAddress> = if channel < 0 {
            vec![packet.address]
        } else {
            self.sockets
                .channels
                .get(&(socket.0, channel))
                .map(|v| v.iter().rev().copied().collect())
                .unwrap_or_default()
        };
        let data = packet.data[..len].to_vec();
        let mut sent = 0;
        for dest in dests {
            let status = match self.config.net {
                NetBackend::Loopback => {
                    self.control.lock().outbox.push(Datagram {
                        socket,
                        address: dest,
                        data: data.clone(),
                    });
                    Some(len)
                }
                NetBackend::Udp => self
                    .udp(socket)
                    .and_then(|s| s.send_to(&data, to_socket_addr(&dest)).ok()),
            };
            if let Some(n) = status {
                packet.status = i32::try_from(n).unwrap_or(i32::MAX);
                sent += 1;
            }
        }
        sent
    }

    fn net_udp_recv(&mut self, socket: UdpSocketId, packet: &mut UdpPacket) -> i32 {
        if self.fails("SDLNet_UDP_Recv") {
            return -1;
        }
        let maxlen = usize::try_from(packet.maxlen)
            .unwrap_or(0)
            .min(packet.data.len());
        let received = match self.config.net {
            NetBackend::Loopback => {
                let mut shared = self.control.lock();
                let pos = shared.inbox.iter().position(|d| d.socket == socket);
                pos.and_then(|p| shared.inbox.remove(p))
                    .map(|d| Ok((d.address, d.data)))
            }
            NetBackend::Udp => {
                let mut buf = vec![0u8; maxlen.max(1)];
                match self.udp(socket).map(|s| s.recv_from(&mut buf)) {
                    Some(Ok((n, std::net::SocketAddr::V4(from)))) => {
                        buf.truncate(n.min(maxlen));
                        Some(Ok((from_socket_addr(from), buf)))
                    }
                    Some(Err(e)) if e.kind() != std::io::ErrorKind::WouldBlock => Some(Err(())),
                    _ => None,
                }
            }
        };
        let (from, data) = match received {
            None => return 0,
            // A ready socket whose recvfrom fails (e.g. ICMP port unreachable):
            // SDL_net sets status -1 and len 0 and receives nothing.
            Some(Err(())) => {
                packet.status = -1;
                packet.len = 0;
                return 0;
            }
            Some(Ok(r)) => r,
        };
        let n = data.len().min(maxlen);
        packet.data[..n].copy_from_slice(&data[..n]);
        packet.status = i32::try_from(n).unwrap_or(i32::MAX);
        packet.len = packet.status;
        packet.address = from;
        // Highest channel, most recent address first, as SDL_net searches.
        packet.channel = (0..MAX_UDP_CHANNELS)
            .rev()
            .find(|c| {
                self.sockets
                    .channels
                    .get(&(socket.0, *c))
                    .is_some_and(|v| v.contains(&from))
            })
            .unwrap_or(-1);
        1
    }

    fn net_alloc_packet(&mut self, size: i32) -> Option<UdpPacket> {
        if self.fails("SDLNet_AllocPacket") {
            return None;
        }
        Some(UdpPacket {
            channel: 0,
            data: vec![0; usize::try_from(size).ok()?],
            len: 0,
            maxlen: size,
            status: 0,
            address: IpAddress::default(),
        })
    }

    fn net_free_packet(&mut self, _packet: UdpPacket) {}

    fn time(&mut self) -> i64 {
        FAKE_TIME
    }

    fn localtime(&mut self, t: i64) -> Tm {
        utc_tm(t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_tm_matches_known_dates() {
        let tm = utc_tm(FAKE_TIME);
        assert_eq!(
            (
                tm.year + 1900,
                tm.mon + 1,
                tm.mday,
                tm.hour,
                tm.min,
                tm.sec,
                tm.wday,
                tm.yday
            ),
            (2001, 9, 9, 1, 46, 40, 0, 251)
        );
        let tm = utc_tm(951_782_400); // 2000-02-29T00:00:00Z
        assert_eq!(
            (tm.year + 1900, tm.mon + 1, tm.mday, tm.yday, tm.wday),
            (2000, 2, 29, 59, 2)
        );
    }

    #[test]
    fn convert_audio_is_zero_order_hold() {
        let mut p = FakePlatform::new(FakeConfig::default());
        let mut cvt = AudioCvt::default();
        assert_eq!(
            p.build_audio_cvt(&mut cvt, AUDIO_S8, 1, 11_025, AUDIO_S16LSB, 1, 44_100),
            1
        );
        assert_eq!(cvt.len_mult, 8);
        let mut buf = vec![0u8; 2 * 8];
        buf[0] = 1;
        buf[1] = 0x80;
        cvt.len = 2;
        assert_eq!(p.convert_audio(&mut cvt, &mut buf), 0);
        assert_eq!(cvt.len_cvt, 16);
        let samples: Vec<i16> = buf
            .chunks(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect();
        assert_eq!(
            samples,
            [256, 256, 256, 256, -32768, -32768, -32768, -32768]
        );
    }

    #[test]
    fn udp_channels_follow_sdl_net_2_2() {
        let mut p = FakePlatform::new(FakeConfig::default());
        let s = p.net_udp_open(1333).expect("socket");
        let addr = |n: u8| IpAddress {
            host: u32::from_ne_bytes([10, 0, 0, n]),
            port: 1333u16.to_be(),
        };
        // Channel -1 fills channel 0 with up to four addresses, then moves on.
        for n in 1..=4 {
            assert_eq!(p.net_udp_bind(s, -1, &addr(n)), 0);
        }
        assert_eq!(p.net_udp_bind(s, -1, &addr(5)), 1);
        assert_eq!(p.net_udp_bind(s, 0, &addr(6)), -1);
        assert_eq!(p.net_get_error(), "No room for new addresses");
        assert_eq!(p.net_udp_bind(s, 32, &addr(6)), -1);
        assert_eq!(p.net_get_error(), "Invalid channel");

        // Sending on a channel reaches every bound address, most recent first.
        let mut pkt = p.net_alloc_packet(16).expect("packet");
        pkt.data[..3].copy_from_slice(b"abc");
        pkt.len = 3;
        pkt.channel = 7;
        assert_eq!(p.net_udp_send(s, 0, &mut pkt), 4);
        assert_eq!((pkt.channel, pkt.status), (0, 3));
        let order: Vec<IpAddress> = p.handle().take_sent().iter().map(|d| d.address).collect();
        assert_eq!(order, [addr(4), addr(3), addr(2), addr(1)]);

        // A sender bound on several channels is reported on the highest one.
        assert_eq!(p.net_udp_bind(s, 1, &addr(2)), 1);
        p.handle().push_datagram(Datagram {
            socket: s,
            address: addr(2),
            data: b"xy".to_vec(),
        });
        let mut rx = p.net_alloc_packet(16).expect("packet");
        assert_eq!(p.net_udp_recv(s, &mut rx), 1);
        assert_eq!((rx.channel, rx.len, rx.status), (1, 2, 2));
        assert_eq!(&rx.data[..2], b"xy");

        // A failed resolve still stores the port and sets the host to INADDR_NONE.
        let mut a = IpAddress::default();
        assert_eq!(
            p.net_resolve_host(&mut a, Some("no.such.host.invalid"), 1333),
            -1
        );
        assert_eq!((a.host, a.port), (u32::MAX, 1333u16.to_be()));
    }
}
