// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) rustyrian contributors

//! `Instrumented<P>`: wraps any platform; injects scripted input, records every
//! platform call, and writes the trace, frame dumps and perf log at each
//! present.

use crate::keys;
use crate::script::{Action, Script};
use crate::trace::{Arg, FrameLine, PlatformCall, TraceWriter, hash, hex};
use rustyrian_core::platform::{
    AudioCallback, AudioCvt, AudioDeviceId, AudioSpec, Event, FrameView, IpAddress, JoystickChange,
    JoystickId, LogOutputFunction, Platform, PlatformProbe, Rect, RendererId, TextureId, Tm,
    UdpPacket, UdpSocketId, WindowId,
};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use xxhash_rust::xxh3::Xxh3;

#[derive(Clone, Debug, Default)]
pub struct InstrumentConfig {
    /// `"c"` or `"rust"`; written to the trace header.
    pub engine: String,
    pub scenario: String,
    pub script: Option<Script>,
    pub trace_out: Option<PathBuf>,
    pub log_out: Option<PathBuf>,
    pub perf_log: Option<PathBuf>,
    /// Stop after this many presents.
    pub frames: Option<u64>,
    /// Stop once the session clock reaches this many ms.
    pub until_ms: Option<u32>,
    pub dump_frames: Option<(PathBuf, u64)>,
    pub sim_digest: bool,
    /// Write `FrameView::snapshot` at this frame to this file.
    pub snapshot_at: Option<(u64, PathBuf)>,
    /// `(path, placeholder)` pairs replaced in recorded strings, so traces do
    /// not depend on where the user and data directories live.
    pub redact: Vec<(String, String)>,
}

/// How a run ended, for the trace's last line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    Limit,
    Exit(i32),
}

/// A shared, clonable list of recorded calls; differential tests keep a clone
/// and compare the C and Rust sessions' calls.
#[derive(Clone, Debug, Default)]
pub struct CallLog(Arc<Mutex<Vec<PlatformCall>>>);

impl CallLog {
    /// Removes and returns every call recorded so far.
    pub fn take(&self) -> Vec<PlatformCall> {
        std::mem::take(&mut *self.0.lock().unwrap_or_else(|e| e.into_inner()))
    }

    fn push(&self, call: PlatformCall) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).push(call);
    }
}

/// Call recording state, separate from `inner` so a call can be recorded
/// while a borrow of the inner platform (a locked texture) is live.
pub struct Recorder {
    calls: Xxh3,
    log_out: Option<BufWriter<File>>,
    /// Receives every call when set.
    pub log: Option<CallLog>,
    redact: Vec<(String, String)>,
}

fn redact_arg(arg: &mut Arg, redact: &[(String, String)]) {
    match arg {
        Arg::Str(s) => {
            for (from, to) in redact {
                if s.contains(from.as_str()) {
                    *s = s.replace(from.as_str(), to);
                }
            }
        }
        Arg::List(items) => items.iter_mut().for_each(|a| redact_arg(a, redact)),
        Arg::Int(_) | Arg::Bytes(_) | Arg::None => {}
    }
}

impl Recorder {
    fn rec(&mut self, name: &'static str, mut args: Vec<Arg>, mut ret: Arg) {
        if !self.redact.is_empty() {
            args.iter_mut().for_each(|a| redact_arg(a, &self.redact));
            redact_arg(&mut ret, &self.redact);
        }
        let call = PlatformCall { name, args, ret };
        let mut line = serde_json::to_vec(&call).unwrap_or_default();
        line.push(b'\n');
        self.calls.update(&line);
        if let Some(w) = self.log_out.as_mut() {
            let _ = w.write_all(&line);
        }
        if let Some(log) = &self.log {
            log.push(call);
        }
    }
}

pub struct Instrumented<P> {
    inner: P,
    config: InstrumentConfig,
    next_event: usize,
    held_mods: u16,
    mouse: (i32, i32),
    pub recorder: Recorder,
    trace: Option<TraceWriter>,
    perf: Option<BufWriter<File>>,
    frame: u64,
    last_rgb: u64,
    ended: bool,
    frame_start: Instant,
    render_start: Option<Instant>,
}

fn rect(r: Rect) -> Arg {
    Arg::List(vec![r.x.into(), r.y.into(), r.w.into(), r.h.into()])
}

fn spec(s: &AudioSpec) -> Arg {
    Arg::List(vec![
        s.freq.into(),
        s.format.into(),
        s.channels.into(),
        s.silence.into(),
        s.samples.into(),
        s.size.into(),
    ])
}

fn cvt_arg(c: &AudioCvt) -> Arg {
    Arg::List(vec![
        c.needed.into(),
        c.src_format.into(),
        c.dst_format.into(),
        Arg::Str(format!("{}", c.rate_incr)),
        c.len.into(),
        c.len_cvt.into(),
        c.len_mult.into(),
        Arg::Str(format!("{}", c.len_ratio)),
    ])
}

fn addr(a: &IpAddress) -> Arg {
    Arg::List(vec![a.host.into(), a.port.into()])
}

fn packet(p: &UdpPacket) -> Arg {
    let len = usize::try_from(p.len).unwrap_or(0).min(p.data.len());
    Arg::List(vec![
        p.channel.into(),
        p.len.into(),
        p.maxlen.into(),
        p.status.into(),
        addr(&p.address),
        Arg::Bytes(hash(&p.data[..len])),
    ])
}

fn event(e: &Event) -> Arg {
    Arg::Str(serde_json::to_string(e).unwrap_or_default())
}

fn tm(t: &Tm) -> Arg {
    Arg::List(vec![
        t.sec.into(),
        t.min.into(),
        t.hour.into(),
        t.mday.into(),
        t.mon.into(),
        t.year.into(),
        t.wday.into(),
        t.yday.into(),
        t.isdst.into(),
    ])
}

impl<P: Platform + PlatformProbe> Instrumented<P> {
    pub fn new(mut inner: P, config: InstrumentConfig) -> std::io::Result<Self> {
        if let Some(script) = &config.script {
            for j in &script.joysticks {
                inner.probe_joystick(JoystickChange::Declare {
                    name: j.name.clone(),
                    axes: j.axes,
                    buttons: j.buttons,
                    hats: j.hats,
                });
            }
        }
        let trace = match &config.trace_out {
            Some(p) => Some(TraceWriter::create(p, &config.engine, &config.scenario)?),
            None => None,
        };
        let log_out = match &config.log_out {
            Some(p) => Some(BufWriter::new(File::create(p)?)),
            None => None,
        };
        let perf = match &config.perf_log {
            Some(p) => {
                let mut w = BufWriter::new(File::create(p)?);
                writeln!(w, "frame,sim_us,render_us")?;
                Some(w)
            }
            None => None,
        };
        if let Some((dir, _)) = &config.dump_frames {
            std::fs::create_dir_all(dir)?;
        }
        let redact = config.redact.clone();
        Ok(Instrumented {
            inner,
            config,
            next_event: 0,
            held_mods: 0,
            mouse: (0, 0),
            recorder: Recorder {
                calls: Xxh3::new(),
                log_out,
                log: None,
                redact,
            },
            trace,
            perf,
            frame: 0,
            last_rgb: hash(&[]),
            ended: false,
            frame_start: Instant::now(),
            render_start: None,
        })
    }

    pub fn inner(&self) -> &P {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut P {
        &mut self.inner
    }

    /// Number of presents recorded so far.
    pub fn frames(&self) -> u64 {
        self.frame
    }

    fn rec(&mut self, name: &'static str, args: Vec<Arg>, ret: Arg) {
        self.recorder.rec(name, args, ret);
    }

    /// Writes the trace's last line and flushes every output. Idempotent.
    pub fn finish(&mut self, end: End) {
        if self.ended {
            return;
        }
        self.ended = true;
        let (kind, code) = match end {
            End::Limit => ("limit", 0),
            End::Exit(c) => ("exit", c),
        };
        if let Some(t) = self.trace.as_mut() {
            let calls = std::mem::replace(&mut self.recorder.calls, Xxh3::new()).digest();
            let _ = t.end(kind, code, &hex(calls));
        }
        if let Some(w) = self.recorder.log_out.as_mut() {
            let _ = w.flush();
        }
        if let Some(w) = self.perf.as_mut() {
            let _ = w.flush();
        }
    }

    fn limit_reached(&mut self) {
        self.finish(End::Limit);
        std::process::exit(0);
    }

    fn check_time_limit(&mut self) {
        if let Some(t) = self.config.until_ms
            && self.inner.probe_now_ms() >= t
        {
            self.limit_reached();
        }
    }

    fn inject_due(&mut self) {
        let Some(script) = self.config.script.as_ref() else {
            return;
        };
        let now = self.inner.probe_now_ms();
        let mut actions = Vec::new();
        while let Some(t) = script.events.get(self.next_event) {
            if t.at_ms > now {
                break;
            }
            actions.push(t.action.clone());
            self.next_event += 1;
        }
        for a in actions {
            match a {
                Action::KeyDown { scancode, keycode } => {
                    self.held_mods |= keys::modifier_bit(scancode);
                    self.inner.probe_inject_event(Event::KeyDown {
                        scancode,
                        keycode,
                        modifiers: self.held_mods,
                        repeat: false,
                    });
                }
                Action::KeyUp { scancode, keycode } => {
                    self.held_mods &= !keys::modifier_bit(scancode);
                    self.inner.probe_inject_event(Event::KeyUp {
                        scancode,
                        keycode,
                        modifiers: self.held_mods,
                        repeat: false,
                    });
                }
                Action::Text(s) => self.inner.probe_inject_event(Event::TextInput(s)),
                Action::MouseMove { x, y } => {
                    let (ox, oy) = self.mouse;
                    self.mouse = (x, y);
                    self.inner.probe_inject_event(Event::MouseMotion {
                        x,
                        y,
                        xrel: x - ox,
                        yrel: y - oy,
                    });
                }
                Action::MouseDown { button } => {
                    let (x, y) = self.mouse;
                    self.inner
                        .probe_inject_event(Event::MouseButtonDown { button, x, y });
                }
                Action::MouseUp { button } => {
                    let (x, y) = self.mouse;
                    self.inner
                        .probe_inject_event(Event::MouseButtonUp { button, x, y });
                }
                Action::JoyAxis {
                    device,
                    axis,
                    value,
                } => self.inner.probe_joystick(JoystickChange::Axis {
                    device,
                    axis,
                    value,
                }),
                Action::JoyButton {
                    device,
                    button,
                    pressed,
                } => self.inner.probe_joystick(JoystickChange::Button {
                    device,
                    button,
                    pressed,
                }),
                Action::JoyHat { device, hat, value } => self
                    .inner
                    .probe_joystick(JoystickChange::Hat { device, hat, value }),
                Action::FocusLost => self.inner.probe_inject_event(Event::WindowFocusLost),
                Action::FocusGained => self.inner.probe_inject_event(Event::WindowFocusGained),
                Action::Quit => self.inner.probe_inject_event(Event::Quit),
            }
        }
    }

    fn dump_frame(&self, view: &FrameView<'_>) -> std::io::Result<()> {
        let Some((dir, every)) = &self.config.dump_frames else {
            return Ok(());
        };
        if *every == 0 || !self.frame.is_multiple_of(*every) {
            return Ok(());
        }
        let mut rgb = Vec::with_capacity(320 * 200 * 3);
        for &i in view.screen.iter().take(320 * 200) {
            let c = view.palette[usize::from(i)].to_be_bytes();
            rgb.extend_from_slice(&c[1..]);
        }
        let file = File::create(dir.join(format!("frame_{}.png", self.frame)))?;
        let mut enc = png::Encoder::new(BufWriter::new(file), 320, 200);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().map_err(std::io::Error::other)?;
        w.write_image_data(&rgb).map_err(std::io::Error::other)?;
        Ok(())
    }
}

impl<P: Platform + PlatformProbe> Platform for Instrumented<P> {
    fn init(&mut self, flags: u32) -> i32 {
        let r = self.inner.init(flags);
        self.rec("SDL_Init", vec![flags.into()], r.into());
        r
    }

    fn init_sub_system(&mut self, flags: u32) -> i32 {
        let r = self.inner.init_sub_system(flags);
        self.rec("SDL_InitSubSystem", vec![flags.into()], r.into());
        r
    }

    fn quit_sub_system(&mut self, flags: u32) {
        self.inner.quit_sub_system(flags);
        self.rec("SDL_QuitSubSystem", vec![flags.into()], Arg::None);
    }

    fn quit(&mut self) {
        self.inner.quit();
        self.rec("SDL_Quit", vec![], Arg::None);
    }

    fn set_hint(&mut self, name: &str, value: &str) -> bool {
        let r = self.inner.set_hint(name, value);
        self.rec("SDL_SetHint", vec![name.into(), value.into()], r.into());
        r
    }

    fn get_error(&mut self) -> String {
        let r = self.inner.get_error();
        self.rec("SDL_GetError", vec![], r.clone().into());
        r
    }

    fn get_base_path(&mut self) -> Option<String> {
        let r = self.inner.get_base_path();
        self.rec("SDL_GetBasePath", vec![], r.clone().into());
        r
    }

    fn get_ticks(&mut self) -> u32 {
        let r = self.inner.get_ticks();
        self.rec("SDL_GetTicks", vec![], r.into());
        r
    }

    fn delay(&mut self, ms: u32) {
        self.inner.delay(ms);
        self.rec("SDL_Delay", vec![ms.into()], Arg::None);
        self.check_time_limit();
    }

    fn log(&mut self, message: &str) {
        self.inner.log(message);
        self.rec("SDL_Log", vec![message.into()], Arg::None);
    }

    fn log_debug(&mut self, category: i32, message: &str) {
        self.inner.log_debug(category, message);
        self.rec(
            "SDL_LogDebug",
            vec![category.into(), message.into()],
            Arg::None,
        );
    }

    fn log_warn(&mut self, category: i32, message: &str) {
        self.inner.log_warn(category, message);
        self.rec(
            "SDL_LogWarn",
            vec![category.into(), message.into()],
            Arg::None,
        );
    }

    fn log_error(&mut self, category: i32, message: &str) {
        self.inner.log_error(category, message);
        self.rec(
            "SDL_LogError",
            vec![category.into(), message.into()],
            Arg::None,
        );
    }

    fn log_get_output_function(&mut self) -> Option<LogOutputFunction> {
        let r = self.inner.log_get_output_function();
        self.rec("SDL_LogGetOutputFunction", vec![], r.map(|f| f.0).into());
        r
    }

    fn log_output(
        &mut self,
        function: LogOutputFunction,
        category: i32,
        priority: i32,
        message: &str,
    ) {
        self.inner.log_output(function, category, priority, message);
        self.rec(
            "SDL_LogOutputFunction",
            vec![
                function.0.into(),
                category.into(),
                priority.into(),
                message.into(),
            ],
            Arg::None,
        );
    }

    fn show_simple_message_box(
        &mut self,
        flags: u32,
        title: &str,
        message: &str,
        window: Option<WindowId>,
    ) -> i32 {
        let r = self
            .inner
            .show_simple_message_box(flags, title, message, window);
        self.rec(
            "SDL_ShowSimpleMessageBox",
            vec![
                flags.into(),
                title.into(),
                message.into(),
                window.map(|w| w.0).into(),
            ],
            r.into(),
        );
        r
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
        let r = self.inner.create_window(title, x, y, w, h, flags);
        self.rec(
            "SDL_CreateWindow",
            vec![
                title.into(),
                x.into(),
                y.into(),
                w.into(),
                h.into(),
                flags.into(),
            ],
            r.map(|v| v.0).into(),
        );
        r
    }

    fn destroy_window(&mut self, window: WindowId) {
        self.inner.destroy_window(window);
        self.rec("SDL_DestroyWindow", vec![window.0.into()], Arg::None);
    }

    fn get_window_size(&mut self, window: WindowId, w: Option<&mut i32>, h: Option<&mut i32>) {
        let (mut ww, mut hh) = (0, 0);
        let (want_w, want_h) = (w.is_some(), h.is_some());
        self.inner
            .get_window_size(window, want_w.then_some(&mut ww), want_h.then_some(&mut hh));
        if let Some(w) = w {
            *w = ww;
        }
        if let Some(h) = h {
            *h = hh;
        }
        self.rec(
            "SDL_GetWindowSize",
            vec![window.0.into()],
            Arg::List(vec![
                want_w.then_some(ww).into(),
                want_h.then_some(hh).into(),
            ]),
        );
    }

    fn set_window_size(&mut self, window: WindowId, w: i32, h: i32) {
        self.inner.set_window_size(window, w, h);
        self.rec(
            "SDL_SetWindowSize",
            vec![window.0.into(), w.into(), h.into()],
            Arg::None,
        );
    }

    fn set_window_position(&mut self, window: WindowId, x: i32, y: i32) {
        self.inner.set_window_position(window, x, y);
        self.rec(
            "SDL_SetWindowPosition",
            vec![window.0.into(), x.into(), y.into()],
            Arg::None,
        );
    }

    fn set_window_fullscreen(&mut self, window: WindowId, flags: u32) -> i32 {
        let r = self.inner.set_window_fullscreen(window, flags);
        self.rec(
            "SDL_SetWindowFullscreen",
            vec![window.0.into(), flags.into()],
            r.into(),
        );
        r
    }

    fn show_window(&mut self, window: WindowId) {
        self.inner.show_window(window);
        self.rec("SDL_ShowWindow", vec![window.0.into()], Arg::None);
    }

    fn get_window_display_index(&mut self, window: WindowId) -> i32 {
        let r = self.inner.get_window_display_index(window);
        self.rec("SDL_GetWindowDisplayIndex", vec![window.0.into()], r.into());
        r
    }

    fn get_num_video_displays(&mut self) -> i32 {
        let r = self.inner.get_num_video_displays();
        self.rec("SDL_GetNumVideoDisplays", vec![], r.into());
        r
    }

    fn get_display_bounds(&mut self, display_index: i32, out: &mut Rect) -> i32 {
        let r = self.inner.get_display_bounds(display_index, out);
        self.rec(
            "SDL_GetDisplayBounds",
            vec![display_index.into()],
            Arg::List(vec![r.into(), rect(*out)]),
        );
        r
    }

    fn create_renderer(&mut self, window: WindowId, index: i32, flags: u32) -> Option<RendererId> {
        let r = self.inner.create_renderer(window, index, flags);
        self.rec(
            "SDL_CreateRenderer",
            vec![window.0.into(), index.into(), flags.into()],
            r.map(|v| v.0).into(),
        );
        r
    }

    fn destroy_renderer(&mut self, renderer: RendererId) {
        self.inner.destroy_renderer(renderer);
        self.rec("SDL_DestroyRenderer", vec![renderer.0.into()], Arg::None);
    }

    fn set_render_draw_color(&mut self, renderer: RendererId, r: u8, g: u8, b: u8, a: u8) -> i32 {
        let ret = self.inner.set_render_draw_color(renderer, r, g, b, a);
        self.rec(
            "SDL_SetRenderDrawColor",
            vec![renderer.0.into(), r.into(), g.into(), b.into(), a.into()],
            ret.into(),
        );
        ret
    }

    fn render_clear(&mut self, renderer: RendererId) -> i32 {
        let r = self.inner.render_clear(renderer);
        self.rec("SDL_RenderClear", vec![renderer.0.into()], r.into());
        r
    }

    fn render_copy(
        &mut self,
        renderer: RendererId,
        texture: TextureId,
        src: Option<Rect>,
        dst: Option<Rect>,
    ) -> i32 {
        let r = self.inner.render_copy(renderer, texture, src, dst);
        self.rec(
            "SDL_RenderCopy",
            vec![
                renderer.0.into(),
                texture.0.into(),
                src.map_or(Arg::None, rect),
                dst.map_or(Arg::None, rect),
            ],
            r.into(),
        );
        r
    }

    fn render_flush(&mut self, renderer: RendererId) -> i32 {
        let r = self.inner.render_flush(renderer);
        self.rec("SDL_RenderFlush", vec![renderer.0.into()], r.into());
        r
    }

    fn render_present(&mut self, renderer: RendererId, view: &FrameView<'_>) {
        let present_start = Instant::now();
        self.inner.render_present(renderer, view);
        self.rec("SDL_RenderPresent", vec![renderer.0.into()], Arg::None);
        let audio = self.inner.probe_take_audio();
        let calls = std::mem::replace(&mut self.recorder.calls, Xxh3::new()).digest();
        let mut pal = Vec::with_capacity(1024);
        for c in view.palette {
            pal.extend_from_slice(&c.to_le_bytes());
        }
        let line = FrameLine {
            frame: self.frame,
            ms: self.inner.probe_now_ms(),
            rgb: hex(self.last_rgb),
            idx: hex(hash(view.screen)),
            pal: hex(hash(&pal)),
            audio: hex(hash(audio.as_deref().unwrap_or(&[]))),
            samples: audio.as_ref().map_or(0, |a| a.len() as u64 / 2),
            calls: hex(calls),
            sim: if self.config.sim_digest {
                view.sim.map(|f| hex(hash(&f())))
            } else {
                None
            },
        };
        if let Some(t) = self.trace.as_mut() {
            let _ = t.frame(&line);
        }
        let _ = self.dump_frame(view);
        if let Some((n, path)) = &self.config.snapshot_at
            && *n == self.frame
            && let Some(snap) = view.snapshot
        {
            let _ = std::fs::write(path, snap());
        }
        let now = Instant::now();
        if let Some(p) = self.perf.as_mut() {
            let render_start = self.render_start.unwrap_or(present_start);
            let sim = render_start.saturating_duration_since(self.frame_start);
            let render = now.saturating_duration_since(render_start);
            let _ = writeln!(
                p,
                "{},{},{}",
                self.frame,
                sim.as_micros(),
                render.as_micros()
            );
        }
        self.frame_start = now;
        self.render_start = None;
        self.frame += 1;
        if self.config.frames.is_some_and(|n| self.frame >= n) {
            self.limit_reached();
        }
        self.check_time_limit();
    }

    fn create_texture(
        &mut self,
        renderer: RendererId,
        format: u32,
        access: i32,
        w: i32,
        h: i32,
    ) -> Option<TextureId> {
        let r = self.inner.create_texture(renderer, format, access, w, h);
        self.rec(
            "SDL_CreateTexture",
            vec![
                renderer.0.into(),
                format.into(),
                access.into(),
                w.into(),
                h.into(),
            ],
            r.map(|v| v.0).into(),
        );
        r
    }

    fn destroy_texture(&mut self, texture: TextureId) {
        self.inner.destroy_texture(texture);
        self.rec("SDL_DestroyTexture", vec![texture.0.into()], Arg::None);
    }

    fn query_texture(
        &mut self,
        texture: TextureId,
        format: Option<&mut u32>,
        access: Option<&mut i32>,
        w: Option<&mut i32>,
        h: Option<&mut i32>,
    ) -> i32 {
        let (mut f, mut a, mut ww, mut hh) = (0u32, 0i32, 0i32, 0i32);
        let want = (format.is_some(), access.is_some(), w.is_some(), h.is_some());
        let r = self.inner.query_texture(
            texture,
            want.0.then_some(&mut f),
            want.1.then_some(&mut a),
            want.2.then_some(&mut ww),
            want.3.then_some(&mut hh),
        );
        if let Some(x) = format {
            *x = f;
        }
        if let Some(x) = access {
            *x = a;
        }
        if let Some(x) = w {
            *x = ww;
        }
        if let Some(x) = h {
            *x = hh;
        }
        self.rec(
            "SDL_QueryTexture",
            vec![texture.0.into()],
            Arg::List(vec![
                r.into(),
                want.0.then_some(f).into(),
                want.1.then_some(a).into(),
                want.2.then_some(ww).into(),
                want.3.then_some(hh).into(),
            ]),
        );
        r
    }

    fn lock_texture(&mut self, texture: TextureId, area: Option<Rect>) -> Option<(&mut [u8], i32)> {
        if self.render_start.is_none() {
            self.render_start = Some(Instant::now());
        }
        let locked = self.inner.lock_texture(texture, area);
        self.recorder.rec(
            "SDL_LockTexture",
            vec![texture.0.into(), area.map_or(Arg::None, rect)],
            locked.as_ref().map(|(_, p)| *p).into(),
        );
        locked
    }

    fn unlock_texture(&mut self, texture: TextureId) {
        if let Some(bytes) = self.inner.probe_locked_texture(texture) {
            self.last_rgb = hash(bytes);
        }
        self.inner.unlock_texture(texture);
        self.rec("SDL_UnlockTexture", vec![texture.0.into()], Arg::None);
    }

    fn poll_event(&mut self) -> Option<Event> {
        self.inject_due();
        let r = self.inner.poll_event();
        self.rec("SDL_PollEvent", vec![], r.as_ref().map_or(Arg::None, event));
        r
    }

    fn push_event(&mut self, e: &Event) -> i32 {
        let r = self.inner.push_event(e);
        self.rec("SDL_PushEvent", vec![event(e)], r.into());
        r
    }

    fn start_text_input(&mut self) {
        self.inner.start_text_input();
        self.rec("SDL_StartTextInput", vec![], Arg::None);
    }

    fn stop_text_input(&mut self) {
        self.inner.stop_text_input();
        self.rec("SDL_StopTextInput", vec![], Arg::None);
    }

    fn show_cursor(&mut self, toggle: i32) -> i32 {
        let r = self.inner.show_cursor(toggle);
        self.rec("SDL_ShowCursor", vec![toggle.into()], r.into());
        r
    }

    fn set_relative_mouse_mode(&mut self, enabled: bool) -> i32 {
        let r = self.inner.set_relative_mouse_mode(enabled);
        self.rec("SDL_SetRelativeMouseMode", vec![enabled.into()], r.into());
        r
    }

    fn get_scancode_name(&mut self, scancode: i32) -> String {
        let r = self.inner.get_scancode_name(scancode);
        self.rec(
            "SDL_GetScancodeName",
            vec![scancode.into()],
            r.clone().into(),
        );
        r
    }

    fn get_scancode_from_name(&mut self, name: &str) -> i32 {
        let r = self.inner.get_scancode_from_name(name);
        self.rec("SDL_GetScancodeFromName", vec![name.into()], r.into());
        r
    }

    fn num_joysticks(&mut self) -> i32 {
        let r = self.inner.num_joysticks();
        self.rec("SDL_NumJoysticks", vec![], r.into());
        r
    }

    fn joystick_open(&mut self, device_index: i32) -> Option<JoystickId> {
        let r = self.inner.joystick_open(device_index);
        self.rec(
            "SDL_JoystickOpen",
            vec![device_index.into()],
            r.map(|v| v.0).into(),
        );
        r
    }

    fn joystick_close(&mut self, joystick: JoystickId) {
        self.inner.joystick_close(joystick);
        self.rec("SDL_JoystickClose", vec![joystick.0.into()], Arg::None);
    }

    fn joystick_name(&mut self, joystick: JoystickId) -> Option<String> {
        let r = self.inner.joystick_name(joystick);
        self.rec(
            "SDL_JoystickName",
            vec![joystick.0.into()],
            r.clone().into(),
        );
        r
    }

    fn joystick_num_axes(&mut self, joystick: JoystickId) -> i32 {
        let r = self.inner.joystick_num_axes(joystick);
        self.rec("SDL_JoystickNumAxes", vec![joystick.0.into()], r.into());
        r
    }

    fn joystick_num_buttons(&mut self, joystick: JoystickId) -> i32 {
        let r = self.inner.joystick_num_buttons(joystick);
        self.rec("SDL_JoystickNumButtons", vec![joystick.0.into()], r.into());
        r
    }

    fn joystick_num_hats(&mut self, joystick: JoystickId) -> i32 {
        let r = self.inner.joystick_num_hats(joystick);
        self.rec("SDL_JoystickNumHats", vec![joystick.0.into()], r.into());
        r
    }

    fn joystick_get_axis(&mut self, joystick: JoystickId, axis: i32) -> i16 {
        let r = self.inner.joystick_get_axis(joystick, axis);
        self.rec(
            "SDL_JoystickGetAxis",
            vec![joystick.0.into(), axis.into()],
            r.into(),
        );
        r
    }

    fn joystick_get_button(&mut self, joystick: JoystickId, button: i32) -> u8 {
        let r = self.inner.joystick_get_button(joystick, button);
        self.rec(
            "SDL_JoystickGetButton",
            vec![joystick.0.into(), button.into()],
            r.into(),
        );
        r
    }

    fn joystick_get_hat(&mut self, joystick: JoystickId, hat: i32) -> u8 {
        let r = self.inner.joystick_get_hat(joystick, hat);
        self.rec(
            "SDL_JoystickGetHat",
            vec![joystick.0.into(), hat.into()],
            r.into(),
        );
        r
    }

    fn joystick_update(&mut self) {
        self.inject_due();
        self.inner.joystick_update();
        self.rec("SDL_JoystickUpdate", vec![], Arg::None);
    }

    fn joystick_event_state(&mut self, state: i32) -> i32 {
        let r = self.inner.joystick_event_state(state);
        self.rec("SDL_JoystickEventState", vec![state.into()], r.into());
        r
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
        let r = self.inner.open_audio_device(
            device,
            is_capture,
            desired,
            callback,
            obtained,
            allowed_changes,
        );
        self.rec(
            "SDL_OpenAudioDevice",
            vec![
                device.into(),
                is_capture.into(),
                spec(desired),
                allowed_changes.into(),
            ],
            Arg::List(vec![r.into(), spec(obtained)]),
        );
        r
    }

    fn pause_audio_device(&mut self, device: AudioDeviceId, pause_on: i32) {
        self.inner.pause_audio_device(device, pause_on);
        self.rec(
            "SDL_PauseAudioDevice",
            vec![device.into(), pause_on.into()],
            Arg::None,
        );
    }

    fn lock_audio_device(&mut self, device: AudioDeviceId) {
        self.inner.lock_audio_device(device);
        self.rec("SDL_LockAudioDevice", vec![device.into()], Arg::None);
    }

    fn unlock_audio_device(&mut self, device: AudioDeviceId) {
        self.inner.unlock_audio_device(device);
        self.rec("SDL_UnlockAudioDevice", vec![device.into()], Arg::None);
    }

    fn close_audio_device(&mut self, device: AudioDeviceId) {
        self.inner.close_audio_device(device);
        self.rec("SDL_CloseAudioDevice", vec![device.into()], Arg::None);
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
        let r = self.inner.build_audio_cvt(
            cvt,
            src_format,
            src_channels,
            src_rate,
            dst_format,
            dst_channels,
            dst_rate,
        );
        self.rec(
            "SDL_BuildAudioCVT",
            vec![
                src_format.into(),
                src_channels.into(),
                src_rate.into(),
                dst_format.into(),
                dst_channels.into(),
                dst_rate.into(),
            ],
            Arg::List(vec![r.into(), cvt_arg(cvt)]),
        );
        r
    }

    fn convert_audio(&mut self, cvt: &mut AudioCvt, buf: &mut [u8]) -> i32 {
        let len = usize::try_from(cvt.len).unwrap_or(0).min(buf.len());
        let input = hash(&buf[..len]);
        let r = self.inner.convert_audio(cvt, buf);
        let out_len = usize::try_from(cvt.len_cvt).unwrap_or(0).min(buf.len());
        self.rec(
            "SDL_ConvertAudio",
            vec![cvt_arg(cvt), Arg::Bytes(input)],
            Arg::List(vec![r.into(), Arg::Bytes(hash(&buf[..out_len]))]),
        );
        r
    }

    fn net_init(&mut self) -> i32 {
        let r = self.inner.net_init();
        self.rec("SDLNet_Init", vec![], r.into());
        r
    }

    fn net_quit(&mut self) {
        self.inner.net_quit();
        self.rec("SDLNet_Quit", vec![], Arg::None);
    }

    fn net_get_error(&mut self) -> String {
        let r = self.inner.net_get_error();
        self.rec("SDLNet_GetError", vec![], r.clone().into());
        r
    }

    fn net_resolve_host(&mut self, address: &mut IpAddress, host: Option<&str>, port: u16) -> i32 {
        let r = self.inner.net_resolve_host(address, host, port);
        self.rec(
            "SDLNet_ResolveHost",
            vec![host.into(), port.into()],
            Arg::List(vec![r.into(), addr(address)]),
        );
        r
    }

    fn net_udp_open(&mut self, port: u16) -> Option<UdpSocketId> {
        let r = self.inner.net_udp_open(port);
        self.rec("SDLNet_UDP_Open", vec![port.into()], r.map(|v| v.0).into());
        r
    }

    fn net_udp_bind(&mut self, socket: UdpSocketId, channel: i32, address: &IpAddress) -> i32 {
        let r = self.inner.net_udp_bind(socket, channel, address);
        self.rec(
            "SDLNet_UDP_Bind",
            vec![socket.0.into(), channel.into(), addr(address)],
            r.into(),
        );
        r
    }

    fn net_udp_send(&mut self, socket: UdpSocketId, channel: i32, p: &mut UdpPacket) -> i32 {
        let before = packet(p);
        let r = self.inner.net_udp_send(socket, channel, p);
        self.rec(
            "SDLNet_UDP_Send",
            vec![socket.0.into(), channel.into(), before],
            Arg::List(vec![r.into(), p.status.into()]),
        );
        r
    }

    fn net_udp_recv(&mut self, socket: UdpSocketId, p: &mut UdpPacket) -> i32 {
        let r = self.inner.net_udp_recv(socket, p);
        let after = if r > 0 { packet(p) } else { Arg::None };
        self.rec(
            "SDLNet_UDP_Recv",
            vec![socket.0.into()],
            Arg::List(vec![r.into(), after]),
        );
        r
    }

    fn net_alloc_packet(&mut self, size: i32) -> Option<UdpPacket> {
        let r = self.inner.net_alloc_packet(size);
        self.rec(
            "SDLNet_AllocPacket",
            vec![size.into()],
            r.as_ref().map(|_| 1).into(),
        );
        r
    }

    fn net_free_packet(&mut self, p: UdpPacket) {
        self.inner.net_free_packet(p);
        self.rec("SDLNet_FreePacket", vec![], Arg::None);
    }

    fn time(&mut self) -> i64 {
        let r = self.inner.time();
        self.rec("time", vec![], r.into());
        r
    }

    fn localtime(&mut self, t: i64) -> Tm {
        let r = self.inner.localtime(t);
        self.rec("localtime", vec![t.into()], tm(&r));
        r
    }
}
