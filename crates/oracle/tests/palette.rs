//! Differential tests for `palette.c` / `palette.h`.

use proptest::prelude::*;
use rustyrian_core::game::Halt;
use rustyrian_core::palette::{self as pl, Color, Palette, State};
use rustyrian_core::sdl::{PIXELFORMAT_RGB565, PIXELFORMAT_RGB888, alloc_format};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff::{self, Diff};
use rustyrian_oracle::mirror::Mirror;
use rustyrian_oracle::mirror::palette::{palette_from_c, palette_to_c};
use std::path::{Path, PathBuf};

#[test]
fn initial_state_matches_c() {
    let _d = Diff::new();
    assert_eq!(diff::load::<State>(), State::default());
}

fn color(i: usize, salt: usize) -> Color {
    let v = i.wrapping_mul(31).wrapping_add(salt);
    Color {
        r: v as u8,
        g: (v >> 1) as u8,
        b: (v >> 2) as u8,
        a: (v >> 3) as u8,
    }
}

#[test]
fn mirror_round_trip() {
    let _d = Diff::new();
    let mut s = State::default();
    for (p, pal) in s.palettes.iter_mut().enumerate() {
        for (i, c) in pal.iter_mut().enumerate() {
            *c = color(i, p);
        }
    }
    s.palettes_count = 23;
    for i in 0..256 {
        s.palette[i] = color(i, 100);
        s.colors[i] = color(i, 200);
        s.rgb_palette[i] = (i as u32) * 0x0101_0101;
        s.yuv_palette[i] = !(i as u32);
        s.fade_palette_diff[i] = [i as i32, -(i as i32), 7];
        s.fade_solid_diff[i] = [-1, i as i32 * 3, -(i as i32)];
    }
    s.store_to_c();
    assert_eq!(diff::load::<State>(), s);
}

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data")
        .canonicalize()
        .expect("data/ (run `cargo xtask fetch-data`)")
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir()
        .join(format!("rustyrian-palette-{}", std::process::id()))
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("scratch dir");
    d
}

fn load_pals_in(dir: &Path) -> (Diff, Option<i32>, Result<(), Halt>) {
    use std::os::unix::ffi::OsStrExt;
    let mut d = Diff::new();
    d.game.file.custom_data_dir_path = Some(dir.as_os_str().as_bytes().to_vec());
    d.store_to_c();
    // SAFETY: loadPals may exit, which the guard catches.
    let c = unsafe { rustyrian_oracle::guard_call(|| b::loadPals()) };
    let r = pl::load_pals(&mut d.game);
    (d, c, r)
}

#[test]
fn load_pals_matches_on_real_data() {
    let (d, c, r) = load_pals_in(&data_dir());
    assert_eq!((c, r), (None, Ok(())));
    assert_eq!(diff::load::<State>(), d.game.palette);
    assert_eq!(d.game.palette.palettes_count, pl::PALETTES_MAX);
    assert_eq!(d.c_calls(), d.rust_calls());
}

/// Missing, short, truncated and oversized `palette.dat`.
#[test]
fn load_pals_other_files_match() {
    let full = std::fs::read(data_dir().join("palette.dat")).expect("palette.dat");
    let (d, c, r) = load_pals_in(&tmp("missing"));
    assert_eq!(c.map(|code| Halt { code }), r.err());
    assert_eq!(c, Some(1));
    assert_eq!(d.c_calls(), d.rust_calls());

    let mut big = full.clone();
    big.extend_from_slice(&full[..768 * 3]);
    for (k, contents) in [full[..100].to_vec(), full[..768 * 5].to_vec(), big]
        .iter()
        .enumerate()
    {
        let dir = tmp(&format!("case{k}"));
        std::fs::write(dir.join("palette.dat"), contents).expect("write");
        let (d, c, r) = load_pals_in(&dir);
        assert_eq!(c.map(|code| Halt { code }), r.err(), "case {k}");
        assert_eq!(diff::load::<State>(), d.game.palette, "case {k}");
        assert_eq!(d.c_calls(), d.rust_calls(), "case {k}");
    }

    // A directory named like the file opens but cannot be read.
    let dir = tmp("dir");
    std::fs::create_dir(dir.join("palette.dat")).expect("mkdir");
    let (d, c, r) = load_pals_in(&dir);
    assert_eq!(c.map(|code| Halt { code }), r.err());
    assert_eq!(d.c_calls(), d.rust_calls());
}

fn palette_of(seed: &[u8]) -> Palette {
    std::array::from_fn(|i| {
        let v = |k: usize| seed[(i * 4 + k) % seed.len()].wrapping_add((i * (k + 3)) as u8);
        Color {
            r: v(0),
            g: v(1),
            b: v(2),
            a: v(3),
        }
    })
}

/// Both engines with a texture format (as after `init_video`) and the given
/// current palette.
fn with_format(rgb565: bool, current: &Palette) -> Diff {
    let mut d = Diff::new();
    let format = if rgb565 {
        PIXELFORMAT_RGB565
    } else {
        PIXELFORMAT_RGB888
    };
    d.game.video.main_window_tex_format = alloc_format(format);
    d.game.palette.palette = *current;
    d.store_to_c();
    d
}

type Diffs = [[i32; 3]; 256];

proptest! {
    #[test]
    fn set_palette_and_set_colors_match(
        seed in prop::collection::vec(any::<u8>(), 1..64),
        first in 0u32..256,
        span in 0u32..256,
        color in any::<[u8; 4]>(),
        rgb565 in any::<bool>(),
    ) {
        let last = (first + span).min(255);
        let colors = palette_of(&seed);
        let mut d = with_format(rgb565, &palette_of(&[7, 1]));
        let mut c = palette_to_c(&colors);
        // SAFETY: a 256-entry palette and an in-range color range.
        unsafe { b::set_palette(c.as_mut_ptr(), first, last) };
        pl::set_palette(&mut d.game, &colors, first, last);
        prop_assert_eq!(diff::load::<State>(), d.game.palette.clone());

        let [r, g, b_, a] = color;
        // SAFETY: an in-range color range.
        unsafe { b::set_colors(b::SDL_Color { r, g, b: b_, a }, first, last) };
        pl::set_colors(&mut d.game, Color { r, g, b: b_, a }, first, last);
        prop_assert_eq!(diff::load::<State>(), d.game.palette.clone());
        prop_assert_eq!(d.c_calls(), d.rust_calls());
    }

    /// A whole fade: init (palette or solid), then every step down to 1.
    #[test]
    fn step_fade_palette_matches(
        seed in prop::collection::vec(any::<u8>(), 1..64),
        target in prop::collection::vec(any::<u8>(), 1..64),
        first in 0u32..256,
        span in 0u32..256,
        steps in 1i32..20,
        solid in any::<Option<[u8; 4]>>(),
        rgb565 in any::<bool>(),
    ) {
        let last = (first + span).min(255);
        let colors = palette_of(&target);
        let mut d = with_format(rgb565, &palette_of(&seed));
        let (mut cd, mut rd): (Box<Diffs>, Box<Diffs>) = (Box::new([[0; 3]; 256]), Box::new([[0; 3]; 256]));
        match solid {
            None => {
                let mut c = palette_to_c(&colors);
                // SAFETY: 256-entry arrays and an in-range color range.
                unsafe { b::init_step_fade_palette(cd.as_mut_ptr(), c.as_mut_ptr(), first, last) };
                pl::init_step_fade_palette(&d.game, &mut rd, &colors, first, last);
            }
            Some([r, g, b_, a]) => {
                // SAFETY: as above.
                unsafe { b::init_step_fade_solid(cd.as_mut_ptr(), b::SDL_Color { r, g, b: b_, a }, first, last) };
                pl::init_step_fade_solid(&d.game, &mut rd, Color { r, g, b: b_, a }, first, last);
            }
        }
        prop_assert_eq!(&cd, &rd);
        for step in (1..=steps).rev() {
            // SAFETY: as above.
            unsafe { b::step_fade_palette(cd.as_mut_ptr(), step, first, last) };
            pl::step_fade_palette(&mut d.game, &mut rd, step, first, last);
            prop_assert_eq!(&cd, &rd);
            prop_assert_eq!(diff::load::<State>(), d.game.palette.clone());
        }
    }

    /// Arbitrary diffs (not only those an init produces).
    #[test]
    fn step_fade_palette_matches_any_diff(
        diffs in prop::collection::vec(any::<[i16; 3]>(), 256),
        seed in prop::collection::vec(any::<u8>(), 1..64),
        steps in 1i32..300,
    ) {
        let mut d = with_format(false, &palette_of(&seed));
        let mut cd: Box<Diffs> = Box::new([[0; 3]; 256]);
        for (x, y) in cd.iter_mut().zip(&diffs) {
            *x = y.map(i32::from);
        }
        let mut rd = cd.clone();
        // SAFETY: a 256-entry array, full range.
        unsafe { b::step_fade_palette(cd.as_mut_ptr(), steps, 0, 255) };
        pl::step_fade_palette(&mut d.game, &mut rd, steps, 0, 255);
        prop_assert_eq!(&cd, &rd);
        prop_assert_eq!(diff::load::<State>(), d.game.palette.clone());
    }

    #[test]
    fn rgb_to_yuv_matches(r in any::<u8>(), g in any::<u8>(), b_ in any::<u8>()) {
        let (r, g, b_) = (i32::from(r), i32::from(g), i32::from(b_));
        // SAFETY: a pure static function.
        prop_assert_eq!(unsafe { b::ot_s_palette_rgb_to_yuv(r, g, b_) }, pl::rgb_to_yuv(r, g, b_));
    }
}

/// Every gray, every primary ramp, and the corners of the color cube.
#[test]
fn rgb_to_yuv_matches_ramps() {
    for v in 0..=255 {
        for (r, g, b_) in [
            (v, v, v),
            (v, 0, 0),
            (0, v, 0),
            (0, 0, v),
            (255 - v, v, 255),
            (v, 255 - v, 0),
        ] {
            // SAFETY: a pure static function.
            let c = unsafe { b::ot_s_palette_rgb_to_yuv(r, g, b_) };
            assert_eq!(c, pl::rgb_to_yuv(r, g, b_));
        }
    }
}

/// The colors mirror round trip used by the tests above.
#[test]
fn palette_conversion_round_trips() {
    let p = palette_of(&[1, 2, 3, 250]);
    assert_eq!(palette_from_c(&palette_to_c(&p)), p);
}
