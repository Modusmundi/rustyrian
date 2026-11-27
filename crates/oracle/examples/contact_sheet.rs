//! Writes C|Rust image pairs for the milestone reports' contact sheets:
//! every `tyrian.pic` picture as loaded by `JE_loadPic`, and the demo-cycle
//! fixture frames through `scale_and_render_screen` with every scaler.
//!
//! `cargo run -p rustyrian-oracle --example contact_sheet -- OUT_DIR`
//! (run `cargo xtask fixtures` first). Each pair is `NAME_c.png` and
//! `NAME_rust.png`; `ffmpeg` stacks and tiles them.

#[path = "../tests/platform_support/mod.rs"]
mod platform_support;

use platform_support::Both;
use rustyrian_core::palette::{self, Palette};
use rustyrian_core::picload;
use rustyrian_core::platform::Platform;
use rustyrian_core::video::{self, ScalingMode};
use rustyrian_core::video_scale::{SCALERS, SCALERS_COUNT};
use rustyrian_oracle::bindings as b;
use rustyrian_oracle::diff;
use rustyrian_oracle::mirror::video::screen_to_c;
use rustyrian_oracle::session;
use std::path::{Path, PathBuf};

fn write_png(path: &Path, width: u32, height: u32, rgb: &[u8]) {
    let file = std::fs::File::create(path).expect("create png");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .and_then(|mut w| w.write_image_data(rgb))
        .expect("write png");
}

/// An 8-bit 320×200 screen through `palette`.
fn indexed_rgb(pixels: &[u8], palette: &Palette) -> Vec<u8> {
    pixels
        .iter()
        .take(320 * 200)
        .flat_map(|&i| {
            let c = palette[usize::from(i)];
            [c.r, c.g, c.b]
        })
        .collect()
}

/// A locked XRGB8888 texture (little-endian B, G, R, X bytes).
fn xrgb_rgb(pixels: &[u8], pitch: usize, width: usize, height: usize) -> Vec<u8> {
    (0..height)
        .flat_map(|y| {
            pixels[y * pitch..y * pitch + width * 4]
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|p| [p[2], p[1], p[0]])
                .collect::<Vec<_>>()
        })
        .collect()
}

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data")
        .canonicalize()
        .expect("data/ (run `cargo xtask fetch-data`)")
}

fn pictures(out: &Path) {
    use std::os::unix::ffi::OsStrExt;
    let mut d = Both::new(&[]);
    d.game.file.custom_data_dir_path = Some(data_dir().as_os_str().as_bytes().to_vec());
    d.store();
    assert_eq!(d.init_video(), None);
    // SAFETY: the real palette.dat loads without exiting.
    unsafe { b::loadPals() };
    palette::load_pals(&mut d.game).expect("palette.dat");
    let screen = d.game.video.vga_screen.expect("VGAScreen");
    for id in 1..=13u8 {
        // SAFETY: a live surface; the real tyrian.pic loads without exiting.
        unsafe { b::JE_loadPic(screen_to_c(Some(screen)), id, true) };
        picload::je_load_pic(&mut d.game, screen, id, true).expect("tyrian.pic");
        let c_video = diff::load::<video::State>();
        let c_palette = diff::load::<palette::State>();
        let c = indexed_rgb(&c_video.screen(screen).pixels, &c_palette.colors);
        let r = indexed_rgb(&d.game.video.screen(screen).pixels, &d.game.palette.colors);
        write_png(&out.join(format!("pic{id:02}_c.png")), 320, 200, &c);
        write_png(&out.join(format!("pic{id:02}_rust.png")), 320, 200, &r);
        d.clear_calls();
    }
}

fn scalers(out: &Path) {
    for frame in [60, 500, 2000] {
        let fx = diff::fixture("demo-cycle", frame);
        for scaler in 0..SCALERS_COUNT {
            let mut d = Both::new(&[]);
            d.game.video_scale.scaler = scaler;
            d.store();
            assert_eq!(d.init_video(), None);
            d.game.palette.rgb_palette = fx.palette.rgb_palette;
            let src = fx.video.vga_screen.expect("VGAScreen");
            if let Some(s) = &fx.video.screens[src as usize] {
                d.game
                    .video
                    .screen_mut(src)
                    .pixels
                    .copy_from_slice(&s.pixels);
            }
            d.game.video.vga_screen = Some(src);
            d.game.video.scaling_mode = ScalingMode::from_c(0).expect("mode 0");
            d.store();
            let tex = d.game.video.main_window_texture.expect("texture");
            // SAFETY: a live surface.
            unsafe { b::scale_and_render_screen(screen_to_c(Some(src))) };
            video::scale_and_render_screen(&mut d.game, src);
            let sc = SCALERS[scaler as usize];
            let (w, h) = (sc.width as usize, sc.height as usize);
            let c = session::with(|s| {
                let (px, pitch) = s.lock_texture(tex, None).expect("lock");
                xrgb_rgb(px, pitch as usize, w, h)
            });
            let (px, pitch) = d.game.platform.lock_texture(tex, None).expect("lock");
            let r = xrgb_rgb(px, pitch as usize, w, h);
            let name = format!("f{frame:04}_{}", sc.name.replace(':', ""));
            write_png(&out.join(format!("{name}_c.png")), w as u32, h as u32, &c);
            write_png(
                &out.join(format!("{name}_rust.png")),
                w as u32,
                h as u32,
                &r,
            );
        }
    }
}

fn main() {
    let out = PathBuf::from(
        std::env::args()
            .nth(1)
            .expect("usage: contact_sheet OUT_DIR"),
    );
    std::fs::create_dir_all(&out).expect("out dir");
    pictures(&out);
    scalers(&out);
    eprintln!("contact_sheet: wrote pairs to {}", out.display());
}
