//! `ot_shim_*`: every intercepted SDL/SDL_net/libc function the upstream C
//! calls, adapted onto the installed `Session` (the same `Platform` the Rust
//! port's headless mode uses). Signatures are checked at compile time against
//! the C declarations by `generated/shim_check.rs`.
//!
//! Variadic logging, `exit` and `malloc` are implemented in `shim/*.c`.
#![allow(non_snake_case, clippy::missing_safety_doc)]

use crate::bindings as b;
use crate::session::{self, with};
use rustyrian_core::platform::{
    AudioCvt, AudioSpec, Event, FrameView, IpAddress, JoystickId, LogOutputFunction, Platform,
    Rect, RendererId, TextureId, UdpPacket, UdpSocketId, WindowId,
};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::{CStr, CString, c_char, c_int, c_void};

include!("../generated/shim_check.rs");

thread_local! {
    /// Strings handed to C as `const char *`. The map owns them for the life
    /// of the thread and never removes entries, so a pointer stays valid after
    /// later calls (C may hold several scancode names at once); a `CString`'s
    /// heap buffer does not move when the map rebalances.
    static INTERNED: RefCell<BTreeMap<Vec<u8>, CString>> = const { RefCell::new(BTreeMap::new()) };
    static TM: RefCell<b::tm> = RefCell::new(b::tm::default());
}

fn intern(s: &str) -> *const c_char {
    let bytes = s.as_bytes().to_vec();
    INTERNED.with(|m| {
        m.borrow_mut()
            .entry(bytes.clone())
            .or_insert_with(|| CString::new(bytes).unwrap_or_default())
            .as_ptr()
    })
}

/// # Safety
/// `p` is null or a NUL-terminated string.
unsafe fn str_arg(p: *const c_char) -> Option<String> {
    if p.is_null() {
        None
    } else {
        // SAFETY: non-null and NUL-terminated per the caller.
        Some(unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned())
    }
}

fn handle<T>(id: u32) -> *mut T {
    id as usize as *mut T
}

fn id_of<T>(p: *mut T) -> u32 {
    assert!(
        !p.is_null(),
        "C passed NULL where the shim expects a handle"
    );
    u32::try_from(p as usize).expect("fake handle out of range")
}

/// # Safety
/// `r` is null or points to a valid `SDL_Rect`.
unsafe fn rect_arg(r: *const b::SDL_Rect) -> Option<Rect> {
    // SAFETY: per the caller.
    unsafe { r.as_ref() }.map(|r| Rect {
        x: r.x,
        y: r.y,
        w: r.w,
        h: r.h,
    })
}

/// # Safety
/// `p` is null or valid for writes.
unsafe fn out<'a, T>(p: *mut T) -> Option<&'a mut T> {
    // SAFETY: per the caller.
    unsafe { p.as_mut() }
}

// --- SDL core ---

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_Init(flags: u32) -> c_int {
    with(|s| s.init(flags))
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_InitSubSystem(flags: u32) -> c_int {
    with(|s| s.init_sub_system(flags))
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_QuitSubSystem(flags: u32) {
    with(|s| s.quit_sub_system(flags));
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_Quit() {
    with(|s| s.quit());
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ot_shim_SDL_SetHint(
    name: *const c_char,
    value: *const c_char,
) -> b::SDL_bool {
    // SAFETY: C passes NUL-terminated strings.
    let (name, value) = unsafe { (str_arg(name), str_arg(value)) };
    let r = with(|s| s.set_hint(&name.unwrap_or_default(), &value.unwrap_or_default()));
    b::SDL_bool::from(r)
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_GetError() -> *const c_char {
    intern(&with(|s| s.get_error()))
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_GetBasePath() -> *mut c_char {
    match with(|s| s.get_base_path()) {
        None => std::ptr::null_mut(),
        Some(p) => {
            let c = CString::new(p).unwrap_or_default();
            let bytes = c.as_bytes_with_nul();
            // SAFETY: allocating with libc malloc so that C's SDL_free (libc
            // free through SDL's default allocator) can release it.
            unsafe {
                let m = libc::malloc(bytes.len()).cast::<u8>();
                assert!(!m.is_null(), "out of memory");
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), m, bytes.len());
                m.cast()
            }
        }
    }
}

// --- timing ---

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_GetTicks() -> u32 {
    with(|s| s.get_ticks())
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_Delay(ms: u32) {
    with(|s| s.delay(ms));
}

// --- logging ---

/// Called by `shim/ot_shim_va.c` with the formatted message.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ot_shim_rs_log(
    name: *const c_char,
    category: c_int,
    message: *const c_char,
) {
    // SAFETY: C passes NUL-terminated strings.
    let (name, message) = unsafe {
        (
            str_arg(name).unwrap_or_default(),
            str_arg(message).unwrap_or_default(),
        )
    };
    with(|s| match name.as_str() {
        "SDL_Log" => s.log(&message),
        "SDL_LogDebug" => s.log_debug(category, &message),
        "SDL_LogWarn" => s.log_warn(category, &message),
        "SDL_LogError" => s.log_error(category, &message),
        other => panic!("unknown log shim {other}"),
    });
}

unsafe extern "C" fn log_output_trampoline(
    userdata: *mut c_void,
    category: c_int,
    priority: b::SDL_LogPriority,
    message: *const c_char,
) {
    // SAFETY: SDL log output functions receive a NUL-terminated message.
    let message = unsafe { str_arg(message) }.unwrap_or_default();
    let function = LogOutputFunction(u32::try_from(userdata as usize).unwrap_or(0));
    let priority = i32::try_from(priority).unwrap_or(i32::MAX);
    with(|s| s.log_output(function, category, priority, &message));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ot_shim_SDL_LogGetOutputFunction(
    callback: *mut b::SDL_LogOutputFunction,
    userdata: *mut *mut c_void,
) {
    let r = with(|s| s.log_get_output_function());
    // SAFETY: C passes valid out-pointers or NULL.
    unsafe {
        if let Some(cb) = out(callback) {
            *cb = r.map(|_| log_output_trampoline as unsafe extern "C" fn(_, _, _, _));
        }
        if let Some(ud) = out(userdata) {
            *ud = r.map_or(std::ptr::null_mut(), |f| f.0 as usize as *mut c_void);
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ot_shim_SDL_ShowSimpleMessageBox(
    flags: u32,
    title: *const c_char,
    message: *const c_char,
    window: *mut b::SDL_Window,
) -> c_int {
    // SAFETY: C passes NUL-terminated strings.
    let (title, message) = unsafe {
        (
            str_arg(title).unwrap_or_default(),
            str_arg(message).unwrap_or_default(),
        )
    };
    let window = (!window.is_null()).then(|| WindowId(id_of(window)));
    with(|s| s.show_simple_message_box(flags, &title, &message, window))
}

// --- windows and displays ---

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ot_shim_SDL_CreateWindow(
    title: *const c_char,
    x: c_int,
    y: c_int,
    w: c_int,
    h: c_int,
    flags: u32,
) -> *mut b::SDL_Window {
    // SAFETY: C passes a NUL-terminated title.
    let title = unsafe { str_arg(title) }.unwrap_or_default();
    with(|s| s.create_window(&title, x, y, w, h, flags))
        .map_or(std::ptr::null_mut(), |w| handle(w.0))
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_DestroyWindow(window: *mut b::SDL_Window) {
    with(|s| s.destroy_window(WindowId(id_of(window))));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ot_shim_SDL_GetWindowSize(
    window: *mut b::SDL_Window,
    w: *mut c_int,
    h: *mut c_int,
) {
    // SAFETY: C passes valid out-pointers or NULL.
    let (w, h) = unsafe { (out(w), out(h)) };
    with(|s| s.get_window_size(WindowId(id_of(window)), w, h));
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_SetWindowSize(window: *mut b::SDL_Window, w: c_int, h: c_int) {
    with(|s| s.set_window_size(WindowId(id_of(window)), w, h));
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_SetWindowPosition(window: *mut b::SDL_Window, x: c_int, y: c_int) {
    with(|s| s.set_window_position(WindowId(id_of(window)), x, y));
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_SetWindowFullscreen(window: *mut b::SDL_Window, flags: u32) -> c_int {
    with(|s| s.set_window_fullscreen(WindowId(id_of(window)), flags))
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_ShowWindow(window: *mut b::SDL_Window) {
    with(|s| s.show_window(WindowId(id_of(window))));
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_GetWindowDisplayIndex(window: *mut b::SDL_Window) -> c_int {
    with(|s| s.get_window_display_index(WindowId(id_of(window))))
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_GetNumVideoDisplays() -> c_int {
    with(|s| s.get_num_video_displays())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ot_shim_SDL_GetDisplayBounds(
    display_index: c_int,
    rect: *mut b::SDL_Rect,
) -> c_int {
    // SAFETY: C passes a valid rect.
    let rect = unsafe { out(rect) }.expect("SDL_GetDisplayBounds with NULL rect");
    let mut r = Rect {
        x: rect.x,
        y: rect.y,
        w: rect.w,
        h: rect.h,
    };
    let ret = with(|s| s.get_display_bounds(display_index, &mut r));
    *rect = b::SDL_Rect {
        x: r.x,
        y: r.y,
        w: r.w,
        h: r.h,
    };
    ret
}

// --- renderer and textures ---

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_CreateRenderer(
    window: *mut b::SDL_Window,
    index: c_int,
    flags: u32,
) -> *mut b::SDL_Renderer {
    with(|s| s.create_renderer(WindowId(id_of(window)), index, flags))
        .map_or(std::ptr::null_mut(), |r| handle(r.0))
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_DestroyRenderer(renderer: *mut b::SDL_Renderer) {
    with(|s| s.destroy_renderer(RendererId(id_of(renderer))));
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_SetRenderDrawColor(
    renderer: *mut b::SDL_Renderer,
    r: u8,
    g: u8,
    bl: u8,
    a: u8,
) -> c_int {
    with(|s| s.set_render_draw_color(RendererId(id_of(renderer)), r, g, bl, a))
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_RenderClear(renderer: *mut b::SDL_Renderer) -> c_int {
    with(|s| s.render_clear(RendererId(id_of(renderer))))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ot_shim_SDL_RenderCopy(
    renderer: *mut b::SDL_Renderer,
    texture: *mut b::SDL_Texture,
    src: *const b::SDL_Rect,
    dst: *const b::SDL_Rect,
) -> c_int {
    // SAFETY: C passes valid rects or NULL.
    let (src, dst) = unsafe { (rect_arg(src), rect_arg(dst)) };
    with(|s| {
        s.render_copy(
            RendererId(id_of(renderer)),
            TextureId(id_of(texture)),
            src,
            dst,
        )
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_RenderFlush(renderer: *mut b::SDL_Renderer) -> c_int {
    with(|s| s.render_flush(RendererId(id_of(renderer))))
}

/// The whole C game state as a postcard `game::Snapshot`, for `--snapshot-at`.
fn snapshot_c_game() -> Vec<u8> {
    let (p, _) = rustyrian_harness::test_session();
    let mut g = rustyrian_core::Game::new(Box::new(p));
    crate::mirror::load_game(&mut g);
    postcard::to_stdvec(&g.snapshot()).expect("a snapshot always serialises")
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_RenderPresent(renderer: *mut b::SDL_Renderer) {
    // SAFETY: reading upstream globals on the only thread running C; the
    // surface is upstream's 320x200 8-bit `VGAScreen`.
    let (screen, palette) = unsafe {
        let surface = &*b::VGAScreen;
        let len = usize::try_from(surface.pitch * surface.h).unwrap_or(0);
        let screen = std::slice::from_raw_parts(surface.pixels.cast::<u8>(), len);
        let palette = (&raw const b::rgb_palette)
            .as_ref()
            .expect("address of a static is non-null");
        (screen, palette)
    };
    let view = FrameView {
        screen,
        palette,
        sim: None,
        snapshot: Some(&snapshot_c_game),
    };
    with(|s| s.render_present(RendererId(id_of(renderer)), &view));
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_CreateTexture(
    renderer: *mut b::SDL_Renderer,
    format: u32,
    access: c_int,
    w: c_int,
    h: c_int,
) -> *mut b::SDL_Texture {
    with(|s| s.create_texture(RendererId(id_of(renderer)), format, access, w, h))
        .map_or(std::ptr::null_mut(), |t| handle(t.0))
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_DestroyTexture(texture: *mut b::SDL_Texture) {
    with(|s| s.destroy_texture(TextureId(id_of(texture))));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ot_shim_SDL_QueryTexture(
    texture: *mut b::SDL_Texture,
    format: *mut u32,
    access: *mut c_int,
    w: *mut c_int,
    h: *mut c_int,
) -> c_int {
    // SAFETY: C passes valid out-pointers or NULL.
    let (format, access, w, h) = unsafe { (out(format), out(access), out(w), out(h)) };
    with(|s| s.query_texture(TextureId(id_of(texture)), format, access, w, h))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ot_shim_SDL_LockTexture(
    texture: *mut b::SDL_Texture,
    rect: *const b::SDL_Rect,
    pixels: *mut *mut c_void,
    pitch: *mut c_int,
) -> c_int {
    // SAFETY: C passes a valid rect or NULL.
    let rect = unsafe { rect_arg(rect) };
    let locked = with(|s| {
        s.lock_texture(TextureId(id_of(texture)), rect)
            .map(|(buf, p)| (buf.as_mut_ptr(), p))
    });
    match locked {
        // SAFETY: C passes valid out-pointers; the buffer is owned by the
        // session's texture and outlives the lock.
        Some((ptr, p)) => unsafe {
            *pixels = ptr.cast();
            *pitch = p;
            0
        },
        None => -1,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_UnlockTexture(texture: *mut b::SDL_Texture) {
    with(|s| s.unlock_texture(TextureId(id_of(texture))));
}

// --- events ---

fn write_text(dst: &mut [c_char; 32], text: &str) {
    let bytes = text.as_bytes();
    let n = bytes.len().min(31);
    for (d, s) in dst.iter_mut().zip(&bytes[..n]) {
        *d = *s as c_char;
    }
    dst[n] = 0;
}

/// # Safety
/// `ev` points to a writable `SDL_Event`.
unsafe fn write_event(ev: &mut b::SDL_Event, e: &Event) {
    // SAFETY: zero-initialising a plain-data union.
    *ev = unsafe { std::mem::zeroed() };
    match e {
        Event::WindowFocusGained | Event::WindowFocusLost | Event::WindowResized { .. } => {
            let mut w = b::SDL_WindowEvent {
                type_: b::SDL_EventType_SDL_WINDOWEVENT,
                windowID: 1,
                ..Default::default()
            };
            match e {
                Event::WindowFocusGained => {
                    w.event = b::SDL_WindowEventID_SDL_WINDOWEVENT_FOCUS_GAINED as u8
                }
                Event::WindowFocusLost => {
                    w.event = b::SDL_WindowEventID_SDL_WINDOWEVENT_FOCUS_LOST as u8
                }
                Event::WindowResized { w: dw, h: dh } => {
                    w.event = b::SDL_WindowEventID_SDL_WINDOWEVENT_RESIZED as u8;
                    w.data1 = *dw;
                    w.data2 = *dh;
                }
                _ => {}
            }
            ev.window = w;
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
            ev.key = b::SDL_KeyboardEvent {
                type_: if down {
                    b::SDL_EventType_SDL_KEYDOWN
                } else {
                    b::SDL_EventType_SDL_KEYUP
                },
                windowID: 1,
                state: if down {
                    b::SDL_PRESSED as u8
                } else {
                    b::SDL_RELEASED as u8
                },
                repeat: u8::from(*repeat),
                keysym: b::SDL_Keysym {
                    scancode: b::SDL_Scancode::from(*scancode),
                    sym: *keycode,
                    mod_: *modifiers,
                    unused: 0,
                },
                ..Default::default()
            };
        }
        Event::TextInput(text) => {
            let mut t = b::SDL_TextInputEvent {
                type_: b::SDL_EventType_SDL_TEXTINPUT,
                windowID: 1,
                ..Default::default()
            };
            write_text(&mut t.text, text);
            ev.text = t;
        }
        Event::MouseMotion { x, y, xrel, yrel } => {
            ev.motion = b::SDL_MouseMotionEvent {
                type_: b::SDL_EventType_SDL_MOUSEMOTION,
                windowID: 1,
                x: *x,
                y: *y,
                xrel: *xrel,
                yrel: *yrel,
                ..Default::default()
            };
        }
        Event::MouseButtonDown { button, x, y } | Event::MouseButtonUp { button, x, y } => {
            let down = matches!(e, Event::MouseButtonDown { .. });
            ev.button = b::SDL_MouseButtonEvent {
                type_: if down {
                    b::SDL_EventType_SDL_MOUSEBUTTONDOWN
                } else {
                    b::SDL_EventType_SDL_MOUSEBUTTONUP
                },
                windowID: 1,
                button: *button,
                state: if down {
                    b::SDL_PRESSED as u8
                } else {
                    b::SDL_RELEASED as u8
                },
                clicks: 1,
                x: *x,
                y: *y,
                ..Default::default()
            };
        }
        Event::Quit => ev.type_ = b::SDL_EventType_SDL_QUIT,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ot_shim_SDL_PollEvent(ev: *mut b::SDL_Event) -> c_int {
    assert!(
        !ev.is_null(),
        "SDL_PollEvent(NULL) is not supported by the shim"
    );
    match with(|s| s.poll_event()) {
        Some(e) => {
            // SAFETY: non-null, points to C's SDL_Event.
            unsafe { write_event(&mut *ev, &e) };
            1
        }
        None => 0,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ot_shim_SDL_PushEvent(ev: *mut b::SDL_Event) -> c_int {
    // SAFETY: C passes a valid event.
    let ev = unsafe { &*ev };
    // SAFETY: `type_` is valid for every event; `key` is read only for key events.
    let e = unsafe {
        match ev.type_ {
            t if t == b::SDL_EventType_SDL_KEYDOWN || t == b::SDL_EventType_SDL_KEYUP => {
                let k = ev.key;
                let scancode = u16::try_from(k.keysym.scancode).expect("scancode out of range");
                if t == b::SDL_EventType_SDL_KEYDOWN {
                    Event::KeyDown {
                        scancode,
                        keycode: k.keysym.sym,
                        modifiers: k.keysym.mod_,
                        repeat: k.repeat != 0,
                    }
                } else {
                    Event::KeyUp {
                        scancode,
                        keycode: k.keysym.sym,
                        modifiers: k.keysym.mod_,
                        repeat: k.repeat != 0,
                    }
                }
            }
            t if t == b::SDL_EventType_SDL_QUIT => Event::Quit,
            t => panic!("SDL_PushEvent of event type {t:#x} is not supported by the shim"),
        }
    };
    with(|s| s.push_event(&e))
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_StartTextInput() {
    with(|s| s.start_text_input());
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_StopTextInput() {
    with(|s| s.stop_text_input());
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_ShowCursor(toggle: c_int) -> c_int {
    with(|s| s.show_cursor(toggle))
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_SetRelativeMouseMode(enabled: b::SDL_bool) -> c_int {
    with(|s| s.set_relative_mouse_mode(enabled != 0))
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_GetScancodeName(scancode: b::SDL_Scancode) -> *const c_char {
    let sc = i32::try_from(scancode).unwrap_or(-1);
    intern(&with(|s| s.get_scancode_name(sc)))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ot_shim_SDL_GetScancodeFromName(name: *const c_char) -> b::SDL_Scancode {
    // SAFETY: C passes a NUL-terminated string or NULL.
    let name = unsafe { str_arg(name) }.unwrap_or_default();
    b::SDL_Scancode::try_from(with(|s| s.get_scancode_from_name(&name))).unwrap_or(0)
}

// --- joysticks ---

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_NumJoysticks() -> c_int {
    with(|s| s.num_joysticks())
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_JoystickOpen(index: c_int) -> *mut b::SDL_Joystick {
    with(|s| s.joystick_open(index)).map_or(std::ptr::null_mut(), |j| handle(j.0))
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_JoystickClose(j: *mut b::SDL_Joystick) {
    with(|s| s.joystick_close(JoystickId(id_of(j))));
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_JoystickName(j: *mut b::SDL_Joystick) -> *const c_char {
    with(|s| s.joystick_name(JoystickId(id_of(j)))).map_or(std::ptr::null(), |n| intern(&n))
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_JoystickNumAxes(j: *mut b::SDL_Joystick) -> c_int {
    with(|s| s.joystick_num_axes(JoystickId(id_of(j))))
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_JoystickNumButtons(j: *mut b::SDL_Joystick) -> c_int {
    with(|s| s.joystick_num_buttons(JoystickId(id_of(j))))
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_JoystickNumHats(j: *mut b::SDL_Joystick) -> c_int {
    with(|s| s.joystick_num_hats(JoystickId(id_of(j))))
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_JoystickGetAxis(j: *mut b::SDL_Joystick, axis: c_int) -> i16 {
    with(|s| s.joystick_get_axis(JoystickId(id_of(j)), axis))
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_JoystickGetButton(j: *mut b::SDL_Joystick, button: c_int) -> u8 {
    with(|s| s.joystick_get_button(JoystickId(id_of(j)), button))
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_JoystickGetHat(j: *mut b::SDL_Joystick, hat: c_int) -> u8 {
    with(|s| s.joystick_get_hat(JoystickId(id_of(j)), hat))
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_JoystickUpdate() {
    with(|s| s.joystick_update());
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_JoystickEventState(state: c_int) -> c_int {
    with(|s| s.joystick_event_state(state))
}

// --- audio ---

struct CAudioCallback {
    f: unsafe extern "C" fn(*mut c_void, *mut u8, c_int),
    userdata: *mut c_void,
}

// SAFETY: the fake platform calls the callback only on the thread running C.
unsafe impl Send for CAudioCallback {}

impl CAudioCallback {
    fn call(&self, buf: &mut [u8]) {
        let len = c_int::try_from(buf.len()).expect("audio buffer too large");
        // SAFETY: upstream's audio callback writes `len` bytes into `buf`.
        unsafe { (self.f)(self.userdata, buf.as_mut_ptr(), len) }
    }
}

fn spec_from(s: &b::SDL_AudioSpec) -> AudioSpec {
    AudioSpec {
        freq: s.freq,
        format: s.format,
        channels: s.channels,
        silence: s.silence,
        samples: s.samples,
        size: s.size,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ot_shim_SDL_OpenAudioDevice(
    device: *const c_char,
    is_capture: c_int,
    desired: *const b::SDL_AudioSpec,
    obtained: *mut b::SDL_AudioSpec,
    allowed_changes: c_int,
) -> b::SDL_AudioDeviceID {
    // SAFETY: C passes a valid desired spec and a NUL-terminated name or NULL.
    let (device, desired) = unsafe { (str_arg(device), &*desired) };
    let cb = CAudioCallback {
        f: desired
            .callback
            .expect("SDL_OpenAudioDevice without a callback"),
        userdata: desired.userdata,
    };
    let mut got = AudioSpec::default();
    let id = with(|s| {
        s.open_audio_device(
            device.as_deref(),
            is_capture,
            &spec_from(desired),
            Box::new(move |buf: &mut [u8]| cb.call(buf)),
            &mut got,
            allowed_changes,
        )
    });
    // SAFETY: C passes a valid out-spec or NULL.
    if let Some(o) = unsafe { out(obtained) } {
        *o = b::SDL_AudioSpec {
            freq: got.freq,
            format: got.format,
            channels: got.channels,
            silence: got.silence,
            samples: got.samples,
            padding: 0,
            size: got.size,
            callback: desired.callback,
            userdata: desired.userdata,
        };
    }
    id
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_PauseAudioDevice(dev: b::SDL_AudioDeviceID, pause_on: c_int) {
    with(|s| s.pause_audio_device(dev, pause_on));
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_LockAudioDevice(dev: b::SDL_AudioDeviceID) {
    with(|s| s.lock_audio_device(dev));
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_UnlockAudioDevice(dev: b::SDL_AudioDeviceID) {
    with(|s| s.unlock_audio_device(dev));
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDL_CloseAudioDevice(dev: b::SDL_AudioDeviceID) {
    with(|s| s.close_audio_device(dev));
}

fn cvt_from(c: &b::SDL_AudioCVT) -> AudioCvt {
    AudioCvt {
        needed: c.needed,
        src_format: c.src_format,
        dst_format: c.dst_format,
        rate_incr: c.rate_incr,
        len: c.len,
        len_cvt: c.len_cvt,
        len_mult: c.len_mult,
        len_ratio: c.len_ratio,
        handle: u32::try_from(c.filter_index).unwrap_or(0),
    }
}

fn cvt_into(c: &mut b::SDL_AudioCVT, r: &AudioCvt) {
    c.needed = r.needed;
    c.src_format = r.src_format;
    c.dst_format = r.dst_format;
    c.rate_incr = r.rate_incr;
    c.len = r.len;
    c.len_cvt = r.len_cvt;
    c.len_mult = r.len_mult;
    c.len_ratio = r.len_ratio;
    // The fake's converter state travels in `filter_index`; upstream never reads it.
    c.filter_index = c_int::try_from(r.handle).unwrap_or(0);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ot_shim_SDL_BuildAudioCVT(
    cvt: *mut b::SDL_AudioCVT,
    src_format: b::SDL_AudioFormat,
    src_channels: u8,
    src_rate: c_int,
    dst_format: b::SDL_AudioFormat,
    dst_channels: u8,
    dst_rate: c_int,
) -> c_int {
    // SAFETY: C passes a valid converter (packed; accessed by value).
    let mut c = unsafe { cvt.read_unaligned() };
    let mut r = cvt_from(&c);
    let ret = with(|s| {
        s.build_audio_cvt(
            &mut r,
            src_format,
            src_channels,
            src_rate,
            dst_format,
            dst_channels,
            dst_rate,
        )
    });
    cvt_into(&mut c, &r);
    // SAFETY: as above.
    unsafe { cvt.write_unaligned(c) };
    ret
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ot_shim_SDL_ConvertAudio(cvt: *mut b::SDL_AudioCVT) -> c_int {
    // SAFETY: C passes a valid converter whose `buf` holds `len * len_mult` bytes.
    let mut c = unsafe { cvt.read_unaligned() };
    let mut r = cvt_from(&c);
    let size = usize::try_from(c.len).unwrap_or(0) * usize::try_from(c.len_mult).unwrap_or(0);
    // SAFETY: as above.
    let buf = unsafe { std::slice::from_raw_parts_mut(c.buf, size) };
    let ret = with(|s| s.convert_audio(&mut r, buf));
    cvt_into(&mut c, &r);
    // SAFETY: as above.
    unsafe { cvt.write_unaligned(c) };
    ret
}

// --- SDL_net ---

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDLNet_Init() -> c_int {
    with(|s| s.net_init())
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDLNet_Quit() {
    with(|s| s.net_quit());
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDLNet_GetError() -> *const c_char {
    intern(&with(|s| s.net_get_error()))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ot_shim_SDLNet_ResolveHost(
    address: *mut b::IPaddress,
    host: *const c_char,
    port: u16,
) -> c_int {
    // SAFETY: C passes a valid address and a NUL-terminated host or NULL.
    let (address, host) = unsafe { (&mut *address, str_arg(host)) };
    let mut a = IpAddress {
        host: address.host,
        port: address.port,
    };
    let r = with(|s| s.net_resolve_host(&mut a, host.as_deref(), port));
    address.host = a.host;
    address.port = a.port;
    r
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDLNet_UDP_Open(port: u16) -> b::UDPsocket {
    with(|s| s.net_udp_open(port)).map_or(std::ptr::null_mut(), |u| handle(u.0))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ot_shim_SDLNet_UDP_Bind(
    sock: b::UDPsocket,
    channel: c_int,
    address: *const b::IPaddress,
) -> c_int {
    // SAFETY: C passes a valid address.
    let a = unsafe { &*address };
    let a = IpAddress {
        host: a.host,
        port: a.port,
    };
    with(|s| s.net_udp_bind(UdpSocketId(id_of(sock)), channel, &a))
}

/// # Safety
/// `p` is a packet allocated by `ot_shim_SDLNet_AllocPacket`.
unsafe fn packet_from(p: &b::UDPpacket) -> UdpPacket {
    let maxlen = usize::try_from(p.maxlen).unwrap_or(0);
    // SAFETY: `data` holds `maxlen` bytes.
    let data = unsafe { std::slice::from_raw_parts(p.data, maxlen) }.to_vec();
    UdpPacket {
        channel: p.channel,
        data,
        len: p.len,
        maxlen: p.maxlen,
        status: p.status,
        address: IpAddress {
            host: p.address.host,
            port: p.address.port,
        },
    }
}

/// # Safety
/// As `packet_from`.
unsafe fn packet_into(p: &mut b::UDPpacket, r: &UdpPacket) {
    let n = r.data.len().min(usize::try_from(p.maxlen).unwrap_or(0));
    // SAFETY: `data` holds `maxlen` bytes.
    unsafe { std::ptr::copy_nonoverlapping(r.data.as_ptr(), p.data, n) };
    p.channel = r.channel;
    p.len = r.len;
    p.status = r.status;
    p.address.host = r.address.host;
    p.address.port = r.address.port;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ot_shim_SDLNet_UDP_Send(
    sock: b::UDPsocket,
    channel: c_int,
    packet: *mut b::UDPpacket,
) -> c_int {
    // SAFETY: C passes a packet from SDLNet_AllocPacket.
    let p = unsafe { &mut *packet };
    // SAFETY: as above.
    let mut r = unsafe { packet_from(p) };
    let ret = with(|s| s.net_udp_send(UdpSocketId(id_of(sock)), channel, &mut r));
    // SAFETY: as above.
    unsafe { packet_into(p, &r) };
    ret
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ot_shim_SDLNet_UDP_Recv(
    sock: b::UDPsocket,
    packet: *mut b::UDPpacket,
) -> c_int {
    // SAFETY: C passes a packet from SDLNet_AllocPacket.
    let p = unsafe { &mut *packet };
    // SAFETY: as above.
    let mut r = unsafe { packet_from(p) };
    let ret = with(|s| s.net_udp_recv(UdpSocketId(id_of(sock)), &mut r));
    // SAFETY: as above.
    unsafe { packet_into(p, &r) };
    ret
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_SDLNet_AllocPacket(size: c_int) -> *mut b::UDPpacket {
    let Some(r) = with(|s| s.net_alloc_packet(size)) else {
        return std::ptr::null_mut();
    };
    // SAFETY: allocating C memory for a packet the C code owns until
    // SDLNet_FreePacket; calloc zero-fills both.
    unsafe {
        let p = libc::calloc(1, std::mem::size_of::<b::UDPpacket>()).cast::<b::UDPpacket>();
        let data = libc::calloc(1, r.data.len().max(1)).cast::<u8>();
        assert!(!p.is_null() && !data.is_null(), "out of memory");
        (*p).data = data;
        (*p).maxlen = r.maxlen;
        packet_into(&mut *p, &r);
        p
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ot_shim_SDLNet_FreePacket(packet: *mut b::UDPpacket) {
    if packet.is_null() {
        return;
    }
    // SAFETY: allocated by ot_shim_SDLNet_AllocPacket.
    let r = unsafe { packet_from(&*packet) };
    with(|s| s.net_free_packet(r));
    // SAFETY: as above.
    unsafe {
        libc::free((*packet).data.cast());
        libc::free(packet.cast());
    }
}

// --- libc ---

static ALLOCS: std::sync::Mutex<BTreeMap<usize, usize>> = std::sync::Mutex::new(BTreeMap::new());

fn allocs() -> std::sync::MutexGuard<'static, BTreeMap<usize, usize>> {
    ALLOCS.lock().unwrap_or_else(|e| e.into_inner())
}

/// Called by the C allocator shims (`shim/ot_guard.c`).
#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_rs_alloc(ptr: *mut c_void, size: usize) {
    allocs().insert(ptr as usize, size);
}

/// Called by the C allocator shims; `usize::MAX` for an unknown block.
#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_rs_forget(ptr: *mut c_void) -> usize {
    allocs().remove(&(ptr as usize)).unwrap_or(usize::MAX)
}

/// Requested size of a live block upstream obtained from `malloc`, `calloc`
/// or `realloc`; `None` for anything else (static data, SDL memory, freed
/// blocks, interior pointers).
pub fn alloc_size(ptr: *const c_void) -> Option<usize> {
    allocs().get(&(ptr as usize)).copied()
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_rand() -> c_int {
    session::crand_next()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ot_shim_time(t: *mut b::time_t) -> b::time_t {
    let now = with(|s| s.time());
    // SAFETY: C passes a valid pointer or NULL.
    if let Some(t) = unsafe { out(t) } {
        *t = now;
    }
    now
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ot_shim_localtime(t: *const b::time_t) -> *mut b::tm {
    // SAFETY: C passes a valid time.
    let t = unsafe { *t };
    let r = with(|s| s.localtime(t));
    TM.with(|tm| {
        let mut tm = tm.borrow_mut();
        tm.tm_sec = r.sec;
        tm.tm_min = r.min;
        tm.tm_hour = r.hour;
        tm.tm_mday = r.mday;
        tm.tm_mon = r.mon;
        tm.tm_year = r.year;
        tm.tm_wday = r.wday;
        tm.tm_yday = r.yday;
        tm.tm_isdst = r.isdst;
        &raw mut *tm
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_atexit(f: Option<unsafe extern "C" fn()>) -> c_int {
    match f {
        Some(f) => {
            session::register_atexit(f);
            0
        }
        None => -1,
    }
}

/// Called by `shim/ot_guard.c` before `exit` unwinds or terminates.
#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_rs_on_exit(code: c_int, guarded: c_int) {
    session::on_exit(code, guarded != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_sinf(x: f32) -> f32 {
    libm::sinf(x)
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_cosf(x: f32) -> f32 {
    libm::cosf(x)
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_sin(x: f64) -> f64 {
    libm::sin(x)
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_pow(x: f64, y: f64) -> f64 {
    libm::pow(x, y)
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_powf(x: f32, y: f32) -> f32 {
    libm::powf(x, y)
}

#[unsafe(no_mangle)]
pub extern "C" fn ot_shim_atanf(x: f32) -> f32 {
    libm::atanf(x)
}
