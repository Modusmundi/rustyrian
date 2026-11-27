//! Mirror of `rustyrian_core::video::State` <-> the C globals of `video.c`.
//!
//! Surfaces: C `SDL_Surface *` values map to `ScreenId` by comparing them
//! with the current C values of `VGAScreenSeg`, `VGAScreen2` and
//! `game_screen` (the only three surfaces upstream creates). Pixels, size,
//! pitch and clip rectangle are copied; the palette is not (upstream never
//! reads an 8-bit surface's palette). When Rust has a surface C lacks, the
//! mirror creates it with the real `SDL_CreateRGBSurface`, as `init_video`
//! does; when C has one Rust lacks, it is freed.
//!
//! Handles (`main_window`, renderer, texture) are the fake platform's ids,
//! which the shim hands to C as pointer values. `main_window_tex_format` is
//! mirrored by contents (`SDL_AllocFormat`/`SDL_FreeFormat` on store).

use super::{Mirror, global, global_mut};
use crate::bindings as b;
use rustyrian_core::platform::{Rect, RendererId, TextureId, WindowId};
use rustyrian_core::sdl::{PixelFormat, Surface};
use rustyrian_core::video::{ScalingMode, ScreenId, State};
use rustyrian_core::video_scale::{SCALERS, ScalerFunction};
use std::ffi::{c_int, c_void};

unsafe extern "C" {
    fn SDL_CreateRGBSurface(
        flags: u32,
        width: c_int,
        height: c_int,
        depth: c_int,
        rmask: u32,
        gmask: u32,
        bmask: u32,
        amask: u32,
    ) -> *mut b::SDL_Surface;
    fn SDL_FreeSurface(surface: *mut b::SDL_Surface);
    fn SDL_AllocFormat(pixel_format: u32) -> *mut b::SDL_PixelFormat;
    fn SDL_FreeFormat(format: *mut b::SDL_PixelFormat);
}

/// The C surface pointers, indexed by `ScreenId as usize`, as currently held
/// by `VGAScreenSeg`, `VGAScreen2` and `game_screen`.
pub fn c_screens() -> [*mut b::SDL_Surface; 3] {
    // SAFETY: C runs only on this thread, inside calls we make.
    unsafe {
        [
            *global(&raw const b::VGAScreenSeg),
            *global(&raw const b::VGAScreen2),
            *global(&raw const b::game_screen),
        ]
    }
}

/// Maps a C `SDL_Surface *` to its `ScreenId` (rule R8).
///
/// # Panics
/// If `p` is neither NULL nor one of the three upstream surfaces.
pub fn screen_from_c(p: *const b::SDL_Surface) -> Option<ScreenId> {
    if p.is_null() {
        return None;
    }
    let screens = c_screens();
    let i = screens
        .iter()
        .position(|&s| std::ptr::eq(s, p))
        .expect("SDL_Surface pointer is not one of the three upstream surfaces");
    Some(ScreenId::ALL[i])
}

/// The C `SDL_Surface *` for `id` (NULL for `None` or a surface C lacks).
pub fn screen_to_c(id: Option<ScreenId>) -> *mut b::SDL_Surface {
    id.map_or(std::ptr::null_mut(), |id| c_screens()[id as usize])
}

/// A handle id from a fake-platform pointer value.
pub fn handle_from_c<T>(p: *const T) -> Option<u32> {
    (!p.is_null()).then(|| u32::try_from(p as usize).expect("fake handle out of range"))
}

/// A fake-platform pointer value from a handle id.
pub fn handle_to_c<T>(id: Option<u32>) -> *mut T {
    id.map_or(std::ptr::null_mut(), |id| id as usize as *mut T)
}

fn rect_from_c(r: &b::SDL_Rect) -> Rect {
    Rect {
        x: r.x,
        y: r.y,
        w: r.w,
        h: r.h,
    }
}

fn rect_to_c(r: &Rect) -> b::SDL_Rect {
    b::SDL_Rect {
        x: r.x,
        y: r.y,
        w: r.w,
        h: r.h,
    }
}

/// Copies a C surface.
///
/// # Safety
/// `s` is NULL or a valid surface.
unsafe fn surface_from_c(s: *const b::SDL_Surface) -> Option<Surface> {
    // SAFETY: per the caller.
    let s = unsafe { s.as_ref() }?;
    let len = usize::try_from(s.pitch * s.h).expect("surface size");
    // SAFETY: a valid surface owns `pitch * h` pixel bytes.
    let pixels = unsafe { std::slice::from_raw_parts(s.pixels.cast::<u8>(), len) }.to_vec();
    Some(Surface {
        w: s.w,
        h: s.h,
        pitch: s.pitch,
        pixels,
        clip_rect: rect_from_c(&s.clip_rect),
    })
}

/// Makes `*slot` a C surface equal to `r` (creating, recreating or freeing
/// it as needed).
///
/// # Safety
/// `*slot` is NULL or a valid surface not referenced elsewhere for freeing.
unsafe fn surface_to_c(r: Option<&Surface>, slot: &mut *mut b::SDL_Surface) {
    let Some(r) = r else {
        if !slot.is_null() {
            // SAFETY: per the caller.
            unsafe { SDL_FreeSurface(*slot) };
            *slot = std::ptr::null_mut();
        }
        return;
    };
    // SAFETY: per the caller.
    let fits = unsafe { slot.as_ref() }.is_some_and(|c| c.w == r.w && c.h == r.h);
    if !fits {
        if !slot.is_null() {
            // SAFETY: per the caller.
            unsafe { SDL_FreeSurface(*slot) };
        }
        // SAFETY: plain SDL call.
        *slot = unsafe { SDL_CreateRGBSurface(0, r.w, r.h, 8, 0, 0, 0, 0) };
        assert!(!slot.is_null(), "SDL_CreateRGBSurface failed");
    }
    // SAFETY: `*slot` is a valid surface now.
    let c = unsafe { &mut **slot };
    assert_eq!(c.pitch, r.pitch, "surface pitch differs from SDL's");
    assert_eq!(
        r.pixels.len(),
        usize::try_from(c.pitch * c.h).expect("size")
    );
    // SAFETY: the surface owns `pitch * h` pixel bytes, checked above.
    unsafe { std::slice::from_raw_parts_mut(c.pixels.cast::<u8>(), r.pixels.len()) }
        .copy_from_slice(&r.pixels);
    c.clip_rect = rect_to_c(&r.clip_rect);
}

/// The C `scalers` table.
fn c_scalers() -> &'static [b::Scalers] {
    // SAFETY: `scalers` has `scalers_count` entries; both are C constants.
    unsafe {
        let n = *global(&raw const b::scalers_count) as usize;
        std::slice::from_raw_parts((&raw const b::scalers).cast::<b::Scalers>(), n)
    }
}

type CScaler = unsafe extern "C" fn(*mut b::SDL_Surface, *mut b::SDL_Texture);

/// The C function pointer for `f`, found through the `scalers` table.
pub fn scaler_to_c(f: Option<ScalerFunction>) -> b::ScalerFunction {
    let f = f?;
    let c = c_scalers();
    for (r, c) in SCALERS.iter().zip(c) {
        if r.scaler16 == Some(f) {
            return c.scaler16;
        }
        if r.scaler32 == Some(f) {
            return c.scaler32;
        }
    }
    unreachable!("every ScalerFunction appears in SCALERS")
}

/// The `ScalerFunction` for a C function pointer.
pub fn scaler_from_c(p: b::ScalerFunction) -> Option<ScalerFunction> {
    let p = p? as CScaler as usize;
    let c = c_scalers();
    for (r, c) in SCALERS.iter().zip(c) {
        if c.scaler16.map(|f| f as CScaler as usize) == Some(p) {
            return r.scaler16;
        }
        if c.scaler32.map(|f| f as CScaler as usize) == Some(p) {
            return r.scaler32;
        }
    }
    panic!("unknown scaler function pointer");
}

/// Pointer to a static C global from its `ot_sv_*` getter.
///
/// # Safety
/// `T` is the C type of the global.
pub unsafe fn sv<'a, T>(getter: unsafe extern "C" fn() -> *mut c_void) -> &'a mut T {
    // SAFETY: the getter returns the address of a static of type `T`.
    unsafe { getter().cast::<T>().as_mut() }.expect("address of a C static")
}

unsafe extern "C" {
    fn ot_shim_calloc(count: usize, size: usize) -> *mut c_void;
    fn ot_shim_free(ptr: *mut c_void);
}

/// Number of `T` entries in a live upstream heap block (allocated through
/// the `malloc` shim, which records every block's size). `None` for NULL or
/// for a pointer that is not a live block (freed and left dangling).
pub fn c_heap_len<T>(p: *const T) -> Option<usize> {
    if p.is_null() {
        return None;
    }
    crate::shim::alloc_size(p.cast()).map(|n| n / std::mem::size_of::<T>().max(1))
}

/// Frees an upstream heap block through the shim (NULL and non-live
/// pointers are left alone), keeping the shim's block registry consistent.
///
/// # Safety
/// `p` is NULL, dangling, or a live block referenced nowhere else.
pub unsafe fn c_heap_free<T>(p: *mut T) {
    if c_heap_len(p).is_some() {
        // SAFETY: a live block from the shim allocator, per the caller.
        unsafe { ot_shim_free(p.cast()) };
    }
}

/// A fresh zeroed upstream heap block holding `items`, allocated as
/// upstream's `malloc(len * size)` is (so an empty slice gives what
/// `malloc(0)` gives).
pub fn c_heap_from<T: Copy>(items: &[T]) -> *mut T {
    let bytes = std::mem::size_of_val(items);
    // SAFETY: the shim allocator returns zeroed memory of `bytes` bytes,
    // which we fill in bounds.
    unsafe {
        let p = ot_shim_calloc(1, bytes).cast::<T>();
        assert!(bytes == 0 || !p.is_null(), "out of memory");
        if !p.is_null() {
            std::ptr::copy_nonoverlapping(items.as_ptr(), p, items.len());
        }
        p
    }
}

impl Mirror for State {
    fn load_from_c(&mut self) {
        // SAFETY: C runs only on this thread, inside calls we make; each
        // reference is dropped before the next is taken.
        unsafe {
            self.fullscreen_display = *global(&raw const b::fullscreen_display);
            self.scaling_mode = ScalingMode::from_c(*global(&raw const b::scaling_mode))
                .expect("valid ScalingMode");
            self.last_output_rect = rect_from_c(sv::<b::SDL_Rect>(b::ot_sv_video_last_output_rect));
            self.next_output_rect = rect_from_c(sv::<b::SDL_Rect>(b::ot_sv_video_next_output_rect));

            let screens = c_screens();
            for (d, s) in self.screens.iter_mut().zip(screens) {
                *d = surface_from_c(s);
            }
            self.vga_screen = screen_from_c(*global(&raw const b::VGAScreen));
            self.vga_screen_seg = screen_from_c(screens[0]);
            self.vga_screen2 = screen_from_c(screens[1]);
            self.game_screen = screen_from_c(screens[2]);

            self.main_window = handle_from_c(*global(&raw const b::main_window)).map(WindowId);
            self.main_window_renderer =
                handle_from_c(*sv::<*mut c_void>(b::ot_sv_video_main_window_renderer))
                    .map(RendererId);
            self.main_window_texture =
                handle_from_c(*sv::<*mut c_void>(b::ot_sv_video_main_window_texture))
                    .map(TextureId);
            self.main_window_tex_format = global(&raw const b::main_window_tex_format)
                .as_ref()
                .map(|f| PixelFormat {
                    format: f.format,
                    bits_per_pixel: f.BitsPerPixel,
                    bytes_per_pixel: f.BytesPerPixel,
                });
            self.scaler_function =
                scaler_from_c(*sv::<b::ScalerFunction>(b::ot_sv_video_scaler_function));
        }
    }

    fn store_to_c(&self) {
        // SAFETY: as in `load_from_c`.
        unsafe {
            *global_mut(&raw mut b::fullscreen_display) = self.fullscreen_display;
            *global_mut(&raw mut b::scaling_mode) = self.scaling_mode.to_c();
            *sv::<b::SDL_Rect>(b::ot_sv_video_last_output_rect) = rect_to_c(&self.last_output_rect);
            *sv::<b::SDL_Rect>(b::ot_sv_video_next_output_rect) = rect_to_c(&self.next_output_rect);

            // Reconcile the three surfaces, then point the globals at them.
            let mut slots = c_screens();
            for i in 1..3 {
                if slots[..i].contains(&slots[i]) {
                    slots[i] = std::ptr::null_mut();
                }
            }
            for (r, slot) in self.screens.iter().zip(slots.iter_mut()) {
                surface_to_c(r.as_ref(), slot);
            }
            let ptr =
                |id: Option<ScreenId>| id.map_or(std::ptr::null_mut(), |id| slots[id as usize]);
            *global_mut(&raw mut b::VGAScreen) = ptr(self.vga_screen);
            *global_mut(&raw mut b::VGAScreenSeg) = ptr(self.vga_screen_seg);
            *global_mut(&raw mut b::VGAScreen2) = ptr(self.vga_screen2);
            *global_mut(&raw mut b::game_screen) = ptr(self.game_screen);

            *global_mut(&raw mut b::main_window) = handle_to_c(self.main_window.map(|h| h.0));
            *sv::<*mut c_void>(b::ot_sv_video_main_window_renderer) =
                handle_to_c(self.main_window_renderer.map(|h| h.0));
            *sv::<*mut c_void>(b::ot_sv_video_main_window_texture) =
                handle_to_c(self.main_window_texture.map(|h| h.0));

            let fmt = global_mut(&raw mut b::main_window_tex_format);
            let same = match (fmt.as_ref(), &self.main_window_tex_format) {
                (Some(c), Some(r)) => c.format == r.format,
                (None, None) => true,
                _ => false,
            };
            if !same {
                if !fmt.is_null() {
                    SDL_FreeFormat(*fmt);
                }
                *fmt = match &self.main_window_tex_format {
                    Some(r) => {
                        let p = SDL_AllocFormat(r.format);
                        assert!(!p.is_null(), "SDL_AllocFormat failed");
                        p
                    }
                    None => std::ptr::null_mut(),
                };
            }

            *sv::<b::ScalerFunction>(b::ot_sv_video_scaler_function) =
                scaler_to_c(self.scaler_function);
        }
    }
}
