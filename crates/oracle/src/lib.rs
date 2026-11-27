//! The C reference oracle: upstream OpenTyrian, unmodified, running on the
//! rustyrian fake platform. See `porting/RULES.md` and plan step 2.

pub mod bindings;
pub mod diff;
pub mod mirror;
pub mod runner;
pub mod session;
pub mod shim;

use std::ffi::{c_char, c_int, c_void};

unsafe extern "C" {
    /// Upstream `main`, renamed by `-Dmain=ot_c_main`.
    pub fn ot_c_main(argc: c_int, argv: *mut *mut c_char) -> c_int;
}

unsafe extern "C" fn guard_trampoline<F: FnOnce()>(ctx: *mut c_void) {
    // SAFETY: `ctx` is the `Option<F>` owned by `guard_call`'s frame.
    let f = unsafe { &mut *ctx.cast::<Option<F>>() };
    if let Some(f) = f.take() {
        f();
    }
}

/// Calls `f`, which calls into C, and returns `Some(code)` if C called
/// `exit(code)` (unwound with `longjmp`), `None` if `f` returned normally.
///
/// # Safety
/// `f` must hold no values with destructors across the C call: `longjmp`
/// skips its frame. Capture only references and `Copy` data.
pub unsafe fn guard_call<F: FnOnce()>(f: F) -> Option<i32> {
    let mut slot = Some(f);
    let mut code: c_int = 0;
    // SAFETY: the trampoline matches `ot_guard_call`'s callback type and
    // receives a pointer to `slot`, which outlives the call.
    let exited = unsafe {
        bindings::ot_guard_call(
            Some(guard_trampoline::<F>),
            (&raw mut slot).cast::<c_void>(),
            &raw mut code,
        )
    };
    (exited != 0).then_some(code)
}
