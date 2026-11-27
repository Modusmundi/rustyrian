//! Differential tests for `video_scale.c` / `video_scale.h`.

use proptest::prelude::*;
use rustyrian_core::Game;
use rustyrian_core::platform::TextureId;
use rustyrian_core::sdl::{PIXELFORMAT_RGB565, PIXELFORMAT_RGB888};
use rustyrian_core::video::{SDL_TEXTUREACCESS_STREAMING, ScreenId};
use rustyrian_core::video_scale::{self as vs, SCALERS, SCALERS_COUNT, ScalerFunction, State};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;
use rustyrian_oracle::mirror::video::{handle_to_c, scaler_to_c, screen_to_c};
use std::ffi::CStr;

mod platform_support;
use platform_support::{Both, fail_next_rust};

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let s = State { scaler: 7 };
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}

#[test]
fn scalers_table_matches_c() {
    // SAFETY: C constants.
    let (n, c) = unsafe {
        let n = b::scalers_count;
        (
            n,
            std::slice::from_raw_parts((&raw const b::scalers).cast::<b::Scalers>(), n as usize),
        )
    };
    assert_eq!(n, SCALERS_COUNT);
    for (r, c) in SCALERS.iter().zip(c) {
        assert_eq!((r.width, r.height), (c.width, c.height));
        assert_eq!(r.scaler16.is_some(), c.scaler16.is_some());
        assert_eq!(r.scaler32.is_some(), c.scaler32.is_some());
        // SAFETY: C string constant.
        assert_eq!(r.name, unsafe { CStr::from_ptr(c.name) }.to_str().unwrap());
    }
}

#[test]
fn set_scaler_by_name_matches_c() {
    let mut d = Both::new(&[]);
    let mut names: Vec<&[u8]> = SCALERS.iter().map(|s| s.name.as_bytes()).collect();
    names.extend([&b""[..], b"none", b"Scale2x ", b"hq", b"4x\0junk"]);
    for (i, name) in names.into_iter().enumerate() {
        d.game.video_scale.scaler = 100 + i as u32;
        d.store();
        let n = std::ffi::CString::new(name.split(|&c| c == 0).next().unwrap()).unwrap();
        // SAFETY: plain C call with a C string.
        unsafe { b::set_scaler_by_name(n.as_ptr()) };
        vs::set_scaler_by_name(&mut d.game, name);
        d.assert_same();
    }
}

type RustScaler = fn(&mut Game, ScreenId, TextureId);

/// The non-hq scalers: Rust function, scale factor and destination format.
const CASES: [(ScalerFunction, RustScaler, i32, u32); 12] = [
    (ScalerFunction::Nn32, vs::nn_32, 1, PIXELFORMAT_RGB888),
    (ScalerFunction::Nn32, vs::nn_32, 2, PIXELFORMAT_RGB888),
    (ScalerFunction::Nn32, vs::nn_32, 3, PIXELFORMAT_RGB888),
    (ScalerFunction::Nn32, vs::nn_32, 4, PIXELFORMAT_RGB888),
    (ScalerFunction::Nn16, vs::nn_16, 1, PIXELFORMAT_RGB565),
    (ScalerFunction::Nn16, vs::nn_16, 2, PIXELFORMAT_RGB565),
    (ScalerFunction::Nn16, vs::nn_16, 3, PIXELFORMAT_RGB565),
    (ScalerFunction::Nn16, vs::nn_16, 4, PIXELFORMAT_RGB565),
    (
        ScalerFunction::Scale2x32,
        vs::scale2x_32,
        2,
        PIXELFORMAT_RGB888,
    ),
    (
        ScalerFunction::Scale2x16,
        vs::scale2x_16,
        2,
        PIXELFORMAT_RGB565,
    ),
    (
        ScalerFunction::Scale3x32,
        vs::scale3x_32,
        3,
        PIXELFORMAT_RGB888,
    ),
    (
        ScalerFunction::Scale3x16,
        vs::scale3x_16,
        3,
        PIXELFORMAT_RGB565,
    ),
];

/// Two engines with an initialised video system.
fn video_both() -> Both {
    let mut d = Both::new(&[]);
    assert_eq!(d.init_video(), None);
    d.clear_calls();
    d
}

/// Runs case `case` on `src` (whose pixels and `rgb_palette` are already set
/// in Rust) with both engines and compares textures, calls and state.
fn run_case(d: &mut Both, case: usize, src: ScreenId) -> Result<(), TestCaseError> {
    let (f, rust, scale, format) = CASES[case];
    let renderer = d.game.video.main_window_renderer.expect("renderer");
    let tex = d
        .both(|p| {
            p.create_texture(
                renderer,
                format,
                SDL_TEXTUREACCESS_STREAMING,
                scale * 320,
                scale * 200,
            )
        })
        .expect("texture");
    d.store();
    d.clear_calls();

    let c = scaler_to_c(Some(f)).expect("C scaler");
    // SAFETY: a live surface and texture handle.
    unsafe { c(screen_to_c(Some(src)), handle_to_c(Some(tex.0))) };
    rust(&mut d.game, src, tex);
    prop_assert_eq!(d.c_calls(), d.rust_calls());
    prop_assert_eq!(&d.c_state(), &d.game.snapshot());

    let pixels = d.both(|p| {
        p.lock_texture(tex, None)
            .map(|(px, pitch)| (px.to_vec(), pitch))
    });
    // `both` asserted the two textures equal.
    prop_assert!(pixels.is_some(), "lockable texture");
    d.both(|p| p.destroy_texture(tex));
    d.clear_calls();
    Ok(())
}

fn small_palette() -> impl Strategy<Value = Vec<u32>> {
    // Few distinct colors (with RGB565-truncation collisions) so every
    // Scale2x/Scale3x equality branch is reached.
    prop::collection::vec(
        prop_oneof![
            Just(0u32),
            Just(0x00FF_FFFF),
            Just(0x0001_0000),
            Just(0x0012_3456),
            any::<u32>()
        ],
        256,
    )
}

proptest! {
    #[test]
    fn scalers_match_c_on_random_images(
        case in 0..CASES.len(),
        palette in small_palette(),
        pixels in prop::collection::vec(0u8..6, 64000),
        screen in 0usize..3,
    ) {
        let mut d = video_both();
        let src = ScreenId::ALL[screen];
        d.game.palette.rgb_palette.copy_from_slice(&palette);
        let s = d.game.video.screen_mut(src);
        let pitch = s.pitch as usize;
        for (y, row) in pixels.chunks(320).enumerate() {
            s.pixels[y * pitch..y * pitch + 320].copy_from_slice(row);
        }
        run_case(&mut d, case, src)?;
    }
}

#[test]
fn scalers_match_c_on_fixture_frames() {
    for frame in [60, 500, 2000] {
        let fx = diff::fixture("demo-cycle", frame);
        for case in 0..CASES.len() {
            let mut d = video_both();
            d.game.palette.rgb_palette = fx.palette.rgb_palette;
            for id in ScreenId::ALL {
                if let Some(s) = &fx.video.screens[id as usize] {
                    d.game
                        .video
                        .screen_mut(id)
                        .pixels
                        .copy_from_slice(&s.pixels);
                }
            }
            let src = fx.video.vga_screen.expect("VGAScreen");
            run_case(&mut d, case, src)
                .unwrap_or_else(|e| panic!("frame {frame}, case {case}: {e}"));
        }
    }
}

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data")
        .canonicalize()
        .expect("data/ (run `cargo xtask fetch-data`)")
}

/// Every picture of `tyrian.pic`, decoded by C's `JE_loadPic` into
/// `VGAScreen` with its palette set by C.
#[test]
fn scalers_match_c_on_every_picture() {
    let mut d = video_both();
    {
        use std::os::unix::ffi::OsStrExt;
        d.game.file.custom_data_dir_path = Some(data_dir().as_os_str().as_bytes().to_vec());
    }
    d.store();
    // SAFETY: `loadPals` may exit, which the guard catches.
    let exit = unsafe { rustyrian_oracle::guard_call(|| b::loadPals()) };
    assert_eq!(exit, None);
    let src = d.game.video.vga_screen_id();
    for id in 1..=b::PCX_NUM as u8 {
        // SAFETY: C's VGAScreen is live; may exit, which the guard catches.
        let exit = unsafe {
            rustyrian_oracle::guard_call(|| b::JE_loadPic(screen_to_c(Some(src)), id, true))
        };
        assert_eq!(exit, None, "picture {id}");
        let c = d.c_state();
        d.game.restore(c);
        d.clear_calls();
        for case in 0..CASES.len() {
            run_case(&mut d, case, src)
                .unwrap_or_else(|e| panic!("picture {id}, case {case}: {e}"));
        }
    }
}

/// D-UB-SCALER-LOCK: when `SDL_LockTexture` fails (C would write through the
/// unset pixel pointer), the Rust scalers skip the pixel writes and still
/// unlock.
#[test]
fn scalers_skip_writes_when_lock_fails() {
    for (f, rust, scale, format) in CASES {
        let mut d = video_both();
        let renderer = d.game.video.main_window_renderer.expect("renderer");
        let tex = d
            .both(|p| {
                p.create_texture(
                    renderer,
                    format,
                    SDL_TEXTUREACCESS_STREAMING,
                    scale * 320,
                    scale * 200,
                )
            })
            .expect("texture");
        d.game.palette.rgb_palette = [0x00AB_CDEF; 256];
        d.clear_calls();
        fail_next_rust(&mut d, "SDL_LockTexture");
        rust(&mut d.game, ScreenId::VgaScreenSeg, tex);
        let calls: Vec<&str> = d.rust_calls().into_iter().map(|c| c.name).collect();
        assert!(
            calls.ends_with(&["SDL_LockTexture", "SDL_UnlockTexture"]),
            "{f:?}: {calls:?}"
        );
        let pixels = d.both(|p| p.lock_texture(tex, None).map(|(px, _)| px.to_vec()));
        assert!(pixels.expect("lockable").iter().all(|&b| b == 0), "{f:?}");
    }
}
