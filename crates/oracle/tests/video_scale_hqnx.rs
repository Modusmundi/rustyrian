//! Differential tests for `video_scale_hqNx.c`.
//!
//! The scalers are compared on the locked texture bytes both engines leave
//! behind, the `YUV1`/`YUV2` statics and the recorded platform calls, over:
//! the demo-cycle fixture frames, every picture of `tyrian.pic` decoded and
//! palettized by C, random images and palettes, and adversarial images whose
//! 3×3 neighbourhoods reach every `pattern` case and both arms of every
//! `diff` test inside the cases.

use proptest::prelude::*;
use proptest::strategy::ValueTree;
use proptest::test_runner::{TestCaseError, TestRunner};
use rustyrian_core::Game;
use rustyrian_core::platform::{Platform, RendererId, TextureId};
use rustyrian_core::sdl::{PIXELFORMAT_RGB888, Surface};
use rustyrian_core::video::{ScreenId, VGA_HEIGHT, VGA_WIDTH};
use rustyrian_core::video_scale_hqnx::{self as hq, State};
use rustyrian_harness::test_session;
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;
use rustyrian_oracle::session;
use std::cell::RefCell;
use std::path::{Path, PathBuf};

/// Upstream functions without a header prototype (defined in
/// `video_scale_hqNx.c` with external linkage), plus the real
/// `SDL_AllocFormat` the oracle links.
mod c {
    use rustyrian_oracle::bindings as b;
    use std::ffi::c_uint;

    unsafe extern "C" {
        pub fn interp1(pc: *mut u32, c1: u32, c2: u32);
        pub fn interp2(pc: *mut u32, c1: u32, c2: u32, c3: u32);
        pub fn interp3(pc: *mut u32, c1: u32, c2: u32);
        pub fn interp4(pc: *mut u32, c1: u32, c2: u32, c3: u32);
        pub fn interp5(pc: *mut u32, c1: u32, c2: u32);
        pub fn interp6(pc: *mut u32, c1: u32, c2: u32, c3: u32);
        pub fn interp7(pc: *mut u32, c1: u32, c2: u32, c3: u32);
        pub fn interp8(pc: *mut u32, c1: u32, c2: u32);
        pub fn interp9(pc: *mut u32, c1: u32, c2: u32, c3: u32);
        pub fn interp10(pc: *mut u32, c1: u32, c2: u32, c3: u32);
        pub fn diff(w1: c_uint, w2: c_uint) -> bool;
        pub fn hq2x_32(src_surface: *mut b::SDL_Surface, dst_texture: *mut b::SDL_Texture);
        pub fn hq3x_32(src_surface: *mut b::SDL_Surface, dst_texture: *mut b::SDL_Texture);
        pub fn hq4x_32(src_surface: *mut b::SDL_Surface, dst_texture: *mut b::SDL_Texture);
        pub fn SDL_AllocFormat(pixel_format: u32) -> *mut b::SDL_PixelFormat;
    }
}

const PIXELS: usize = (VGA_WIDTH * VGA_HEIGHT) as usize;
const SDL_TEXTUREACCESS_STREAMING: i32 = 1;

type CScaler = unsafe extern "C" fn(*mut b::SDL_Surface, *mut b::SDL_Texture);
type RustScaler = fn(&mut Game, ScreenId, TextureId);

/// `(factor, Rust, C)` for hq2x, hq3x, hq4x.
const SCALERS: [(i32, RustScaler, CScaler); 3] = [
    (2, hq::hq2x_32, c::hq2x_32),
    (3, hq::hq3x_32, c::hq3x_32),
    (4, hq::hq4x_32, c::hq4x_32),
];

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data")
        .canonicalize()
        .expect("data/ (run `cargo xtask fetch-data`)")
}

/// Both engines with one streaming texture per scaler, of the scaler's size,
/// created identically in both sessions.
struct Both {
    d: Diff,
    textures: [TextureId; 3],
}

impl Both {
    fn new(d: Diff) -> Both {
        let mut d = d;
        let textures = SCALERS.map(|(n, _, _)| {
            let (w, h) = (n * VGA_WIDTH, n * VGA_HEIGHT);
            let ct = session::with(|s| {
                s.create_texture(
                    RendererId(1),
                    PIXELFORMAT_RGB888,
                    SDL_TEXTUREACCESS_STREAMING,
                    w,
                    h,
                )
            })
            .expect("C texture");
            let rt = d
                .game
                .platform
                .create_texture(
                    RendererId(1),
                    PIXELFORMAT_RGB888,
                    SDL_TEXTUREACCESS_STREAMING,
                    w,
                    h,
                )
                .expect("Rust texture");
            assert_eq!(ct, rt);
            rt
        });
        d.c_calls();
        d.rust_calls();
        Both { d, textures }
    }

    /// Scales the 320×200 image `pixels` with every scaler in both engines,
    /// with the palettes currently in `self.d.game.palette` (stored to C here)
    /// and the `YUV1`/`YUV2` statics set to `yuv`. Returns the hq4x output.
    fn check(&mut self, pixels: &[u8], yuv: (i32, i32)) -> Result<(), TestCaseError> {
        assert_eq!(pixels.len(), PIXELS);
        let d = &mut self.d;
        d.game.palette.store_to_c();

        let mut surface = Surface::new_8bit(VGA_WIDTH, VGA_HEIGHT);
        surface.pixels.copy_from_slice(pixels);
        d.game.video.screens[ScreenId::VgaScreenSeg as usize] = Some(surface);

        let mut c_pixels = pixels.to_vec();
        // SAFETY: plain C struct; the scalers read only `pixels` and `pitch`.
        let mut c_surface: b::SDL_Surface = unsafe { std::mem::zeroed() };
        c_surface.w = VGA_WIDTH;
        c_surface.h = VGA_HEIGHT;
        c_surface.pitch = VGA_WIDTH;
        c_surface.pixels = c_pixels.as_mut_ptr().cast();

        for (&(n, rust, cfn), &t) in SCALERS.iter().zip(&self.textures) {
            d.game.video_scale_hqnx = State {
                yuv1: yuv.0,
                yuv2: yuv.1,
            };
            d.game.video_scale_hqnx.store_to_c();

            // SAFETY: the surface's pixels outlive the call; the texture
            // handle is a fake id the shim resolves.
            unsafe { cfn(&raw mut c_surface, t.0 as usize as *mut b::SDL_Texture) };
            rust(&mut d.game, ScreenId::VgaScreenSeg, t);

            prop_assert_eq!(d.c_calls(), d.rust_calls(), "hq{}x calls", n);

            // Read both textures back (the readback's own calls are dropped).
            let c_tex = session::with(|s| {
                let v = s.lock_texture(t, None).map(|(buf, _)| buf.to_vec());
                s.unlock_texture(t);
                v
            })
            .expect("C texture");
            let r_tex = d
                .game
                .platform
                .lock_texture(t, None)
                .map(|(buf, _)| buf.to_vec())
                .expect("Rust texture");
            d.game.platform.unlock_texture(t);
            d.c_calls();
            d.rust_calls();
            prop_assert_eq!(c_tex.len(), r_tex.len());
            if let Some(i) = (0..c_tex.len())
                .step_by(4)
                .find(|&i| c_tex[i..i + 4] != r_tex[i..i + 4])
            {
                let px = i / 4;
                let w = (n * VGA_WIDTH) as usize;
                return Err(TestCaseError::fail(format!(
                    "hq{n}x: first differing pixel ({}, {}) from source ({}, {}): C {:02x?} Rust {:02x?}",
                    px % w,
                    px / w,
                    px % w / n as usize,
                    px / w / n as usize,
                    &c_tex[i..i + 4],
                    &r_tex[i..i + 4],
                )));
            }
            prop_assert_eq!(
                diff::load::<State>(),
                d.game.video_scale_hqnx.clone(),
                "hq{}x statics",
                n
            );
        }
        Ok(())
    }
}

/// Runs `f` once per generated case on one shared pair of engines (the
/// textures are large; one per scaler is created up front).
///
/// Each case is a whole 320×200 image, i.e. 64,000 scaled neighbourhoods, so
/// these tests run an eighth of the configured case count (`PROPTEST_CASES`).
/// Shrinking is off: every shrink step would rescale whole images with all six
/// scalers, and the failure message already names the first differing pixel.
fn run_cases<S: Strategy>(
    strategy: S,
    f: impl Fn(&mut Both, S::Value) -> Result<(), TestCaseError>,
) {
    let both = RefCell::new(Both::new(Diff::new()));
    let config = ProptestConfig::default();
    TestRunner::new(ProptestConfig {
        cases: (config.cases / 8).max(1),
        max_shrink_iters: 0,
        ..config
    })
    .run(&strategy, |v| f(&mut both.borrow_mut(), v))
    .unwrap();
}

fn palette256(s: impl Strategy<Value = u32>) -> impl Strategy<Value = [u32; 256]> {
    prop::collection::vec(s, 256).prop_map(|v| v.try_into().unwrap())
}

/// A YUV value near `base`: each of the Y, U and V fields is moved by 0, by
/// exactly the threshold, by one past it, or by a random amount, in either
/// direction, so every `> tr*` comparison sees both outcomes and the
/// short-circuit reaches each of the three lines.
fn near_yuv(base: u32) -> impl Strategy<Value = u32> {
    fn field(base: u32, shift: u32, tr: u32) -> impl Strategy<Value = u32> {
        let v = (base >> shift) & 0xFF;
        let step = tr >> shift;
        prop_oneof![Just(0u32), Just(step), Just(step + 1), 0u32..=0xFF,].prop_flat_map(
            move |delta| {
                prop_oneof![
                    Just(v.saturating_add(delta).min(0xFF)),
                    Just(v.saturating_sub(delta))
                ]
            },
        )
    }
    let hi = base & 0xFF00_0000;
    (
        field(base, 16, hq::TR_Y as u32),
        field(base, 8, hq::TR_U as u32),
        field(base, 0, hq::TR_V as u32),
        prop_oneof![Just(hi), any::<u8>().prop_map(|h| u32::from(h) << 24)],
    )
        .prop_map(|(y, u, v, hi)| hi | y << 16 | u << 8 | v)
}

/// An adversarial case: a palette whose YUV entries cluster around one base
/// value (so neighbours may be "equal" by index, "close" or "distinct"), and an
/// image drawn from the first `colors` indices, so that every 3×3 pattern
/// occurs many times in each image.
fn adversarial() -> impl Strategy<Value = ([u32; 256], [u32; 256], Vec<u8>)> {
    (any::<u32>(), 2u8..=6).prop_flat_map(|(base, colors)| {
        (
            palette256(any::<u32>()),
            palette256(near_yuv(base)),
            prop::collection::vec(0..colors, PIXELS),
        )
    })
}

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let s = State {
        yuv1: 0x0012_3456,
        yuv2: -7,
    };
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}

proptest! {
    /// `interp1`…`interp10` over the whole `Uint32` domain (wrapping included).
    #[test]
    fn interp_matches_c(c1 in any::<u32>(), c2 in any::<u32>(), c3 in any::<u32>(), small in any::<bool>()) {
        let _d = Diff::new();
        // Also exercise palette-like values, where no product wraps.
        let (c1, c2, c3) = if small {
            (c1 & 0x00FC_FCFC, c2 & 0x00FC_FCFC, c3 & 0x00FC_FCFC)
        } else {
            (c1, c2, c3)
        };
        type C2 = unsafe extern "C" fn(*mut u32, u32, u32);
        type C3 = unsafe extern "C" fn(*mut u32, u32, u32, u32);
        type R2 = fn(&mut [u8], u32, u32);
        type R3 = fn(&mut [u8], u32, u32, u32);
        let two: [(&str, C2, R2); 4] = [
            ("interp1", c::interp1, hq::interp1),
            ("interp3", c::interp3, hq::interp3),
            ("interp5", c::interp5, hq::interp5),
            ("interp8", c::interp8, hq::interp8),
        ];
        let three: [(&str, C3, R3); 6] = [
            ("interp2", c::interp2, hq::interp2),
            ("interp4", c::interp4, hq::interp4),
            ("interp6", c::interp6, hq::interp6),
            ("interp7", c::interp7, hq::interp7),
            ("interp9", c::interp9, hq::interp9),
            ("interp10", c::interp10, hq::interp10),
        ];
        for (name, cf, rf) in two {
            let mut cv = 0u32;
            // SAFETY: `cv` is a valid `Uint32` out-pointer.
            unsafe { cf(&raw mut cv, c1, c2) };
            let mut rv = [0u8; 8];
            rf(&mut rv, c1, c2);
            prop_assert_eq!(&rv[4..], &[0u8; 4], "{} wrote past its pixel", name);
            prop_assert_eq!(u32::from_ne_bytes(rv[..4].try_into().unwrap()), cv, "{}", name);
        }
        for (name, cf, rf) in three {
            let mut cv = 0u32;
            // SAFETY: as above.
            unsafe { cf(&raw mut cv, c1, c2, c3) };
            let mut rv = [0u8; 8];
            rf(&mut rv, c1, c2, c3);
            prop_assert_eq!(&rv[4..], &[0u8; 4], "{} wrote past its pixel", name);
            prop_assert_eq!(u32::from_ne_bytes(rv[..4].try_into().unwrap()), cv, "{}", name);
        }
    }

    /// `diff` over arbitrary and clustered YUV palettes.
    #[test]
    fn diff_matches_c(
        yuv in prop_oneof![palette256(any::<u32>()), any::<u32>().prop_flat_map(|b| palette256(near_yuv(b)))],
        w1 in any::<u8>(),
        w2 in any::<u8>(),
    ) {
        let d = Diff::new();
        let mut pal = d.game.palette.clone();
        pal.yuv_palette = yuv;
        pal.store_to_c();
        // SAFETY: indices are below 256.
        let c = unsafe { c::diff(w1.into(), w2.into()) };
        prop_assert_eq!(hq::diff(&yuv, w1.into(), w2.into()), c);
        prop_assert_eq!(d.c_calls(), d.rust_calls());
    }
}

/// Random images and palettes over the whole value range.
#[test]
fn scalers_match_c_on_random_images() {
    run_cases(
        (
            palette256(any::<u32>()),
            palette256(any::<u32>()),
            prop::collection::vec(any::<u8>(), PIXELS),
            any::<(i32, i32)>(),
        ),
        |both, (rgb, yuv, pixels, statics)| {
            both.d.game.palette.rgb_palette = rgb;
            both.d.game.palette.yuv_palette = yuv;
            both.check(&pixels, statics)
        },
    );
}

/// Few-colour images with clustered YUV palettes: every `pattern` value and
/// both arms of each in-case `diff` test.
#[test]
fn scalers_match_c_on_adversarial_images() {
    run_cases(adversarial(), |both, (rgb, yuv, pixels)| {
        both.d.game.palette.rgb_palette = rgb;
        both.d.game.palette.yuv_palette = yuv;
        both.check(&pixels, (0, 0))
    });
}

/// The adversarial generator does reach all 256 patterns and both outcomes of
/// every neighbour comparison (checked on the Rust side's own inputs, with the
/// pattern computed as C computes it).
#[test]
fn adversarial_images_reach_every_pattern() {
    let mut seen = [false; 256];
    let mut runner = TestRunner::deterministic();
    for _ in 0..4 {
        let (_, yuv, pixels) = adversarial().new_tree(&mut runner).unwrap().current();
        let w = VGA_WIDTH as usize;
        let h = VGA_HEIGHT as usize;
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let at = |dx: usize, dy: usize| u32::from(pixels[(y + dy - 1) * w + x + dx - 1]);
                let mut pattern = 0;
                let mut flag = 1;
                for k in [
                    (0, 0),
                    (1, 0),
                    (2, 0),
                    (0, 1),
                    (2, 1),
                    (0, 2),
                    (1, 2),
                    (2, 2),
                ] {
                    if at(k.0, k.1) != at(1, 1) && hq::diff(&yuv, at(1, 1), at(k.0, k.1)) {
                        pattern |= flag;
                    }
                    flag <<= 1;
                }
                seen[pattern] = true;
            }
        }
    }
    let missing: Vec<usize> = (0..256).filter(|&p| !seen[p]).collect();
    assert!(missing.is_empty(), "patterns never generated: {missing:?}");
}

/// The frames of the demo-cycle fixtures (title screen, demo gameplay), with
/// the palettes C had at those frames.
#[test]
fn scalers_match_c_on_fixture_frames() {
    for frame in [60, 500, 2000] {
        let d = Diff::from_fixture("demo-cycle", frame);
        let pixels = d
            .game
            .video
            .screen(d.game.video.vga_screen_id())
            .pixels
            .clone();
        assert!(
            pixels.iter().any(|&p| p != pixels[0]),
            "frame {frame} is blank"
        );
        let mut both = Both::new(d);
        both.check(&pixels, (0, 0))
            .unwrap_or_else(|e| panic!("frame {frame}: {e}"));
    }
}

/// Every picture of `tyrian.pic`, decoded by C's `JE_loadPic` with its
/// palette applied by C's `set_palette` on an RGB888 texture format.
#[test]
fn scalers_match_c_on_every_picture() {
    let mut d = Diff::new();
    {
        use std::os::unix::ffi::OsStrExt;
        d.game.file.custom_data_dir_path = Some(data_dir().as_os_str().as_bytes().to_vec());
    }
    d.game.file.store_to_c();
    // SAFETY: real SDL allocation; C runs only on this thread.
    unsafe {
        *rustyrian_oracle::mirror::global_mut(&raw mut b::main_window_tex_format) =
            c::SDL_AllocFormat(PIXELFORMAT_RGB888);
    }
    // SAFETY: `loadPals` may exit, which the guard catches.
    let exit = unsafe { rustyrian_oracle::guard_call(|| b::loadPals()) };
    assert_eq!(exit, None);
    let mut both = Both::new(d);
    for id in 1..=b::PCX_NUM as u8 {
        let mut pixels = vec![0u8; PIXELS];
        // SAFETY: plain C struct; `JE_loadPic` reads w, h, pitch, pixels and
        // `format->BytesPerPixel`.
        let mut format: b::SDL_PixelFormat = unsafe { std::mem::zeroed() };
        format.BytesPerPixel = 1;
        format.BitsPerPixel = 8;
        // SAFETY: as above.
        let mut surface: b::SDL_Surface = unsafe { std::mem::zeroed() };
        surface.w = VGA_WIDTH;
        surface.h = VGA_HEIGHT;
        surface.pitch = VGA_WIDTH;
        surface.pixels = pixels.as_mut_ptr().cast();
        surface.format = &raw mut format;
        // SAFETY: the surface and its pixels outlive the call; may exit.
        let exit =
            unsafe { rustyrian_oracle::guard_call(|| b::JE_loadPic(&raw mut surface, id, true)) };
        assert_eq!(exit, None, "picture {id}");
        both.d.game.palette = diff::load();
        assert!(
            pixels.iter().any(|&p| p != pixels[0]),
            "picture {id} is blank"
        );
        both.d.c_calls();
        both.check(&pixels, (0, 0))
            .unwrap_or_else(|e| panic!("picture {id}: {e}"));
    }
}

/// D-UB-SCALER-LOCK: when `SDL_LockTexture` fails (C would write through the
/// unset pixel pointer), the Rust scalers skip the pixel writes and still
/// unlock.
#[test]
fn scalers_skip_writes_when_lock_fails() {
    for (i, &(n, rust, _)) in SCALERS.iter().enumerate() {
        let (mut session, log) = test_session();
        session.inner_mut().fail_next("SDL_LockTexture");
        let mut g = Game::new(Box::new(session));
        let t = g
            .platform
            .create_texture(
                RendererId(1),
                PIXELFORMAT_RGB888,
                SDL_TEXTUREACCESS_STREAMING,
                n * VGA_WIDTH,
                n * VGA_HEIGHT,
            )
            .expect("texture");
        let mut surface = Surface::new_8bit(VGA_WIDTH, VGA_HEIGHT);
        surface.pixels.fill(7);
        g.video.screens[ScreenId::VgaScreenSeg as usize] = Some(surface);
        g.palette.rgb_palette = [0x00FF_FFFF; 256];
        log.take();
        rust(&mut g, ScreenId::VgaScreenSeg, t);
        let names: Vec<&str> = log.take().into_iter().map(|c| c.name).collect();
        assert_eq!(
            names,
            ["SDL_LockTexture", "SDL_UnlockTexture"],
            "scaler {i}"
        );
        let after = g
            .platform
            .lock_texture(t, None)
            .map(|(buf, _)| buf.to_vec())
            .expect("texture");
        assert!(after.iter().all(|&b| b == 0), "scaler {i} wrote pixels");
        assert_eq!(g.video_scale_hqnx, State::default());
    }
}
