//! The 32-bit scalers on the `demo-cycle` fixture frames (`cargo xtask
//! fixtures`): nearest neighbour 2×/3×/4×, Scale2x/3x and hq2x/3x/4x. One
//! iteration scales one 320×200 frame into a streaming texture of the fake
//! platform.

mod common;

use criterion::{Criterion, criterion_group, criterion_main};
use rustyrian_core::Game;
use rustyrian_core::platform::{RendererId, TextureId};
use rustyrian_core::sdl::PIXELFORMAT_RGB888;
use rustyrian_core::video::{SDL_TEXTUREACCESS_STREAMING, ScreenId};
use rustyrian_core::video_scale as vs;
use rustyrian_core::video_scale_hqnx as hq;
use std::hint::black_box;

type Scaler = fn(&mut Game, ScreenId, TextureId);

const SCALERS: [(&str, i32, Scaler); 8] = [
    ("nn_32 2x", 2, vs::nn_32),
    ("nn_32 3x", 3, vs::nn_32),
    ("nn_32 4x", 4, vs::nn_32),
    ("scale2x_32", 2, vs::scale2x_32),
    ("scale3x_32", 3, vs::scale3x_32),
    ("hq2x_32", 2, hq::hq2x_32),
    ("hq3x_32", 3, hq::hq3x_32),
    ("hq4x_32", 4, hq::hq4x_32),
];

fn bench_scalers(c: &mut Criterion) {
    for frame in common::FIXTURE_FRAMES {
        let mut g = common::fixture_game(frame);
        let src = g.video.vga_screen.expect("VGAScreen");
        let mut group = c.benchmark_group(format!("scalers/demo-cycle@{frame}"));
        for (name, n, f) in SCALERS {
            let texture = g
                .platform
                .create_texture(
                    RendererId(1),
                    PIXELFORMAT_RGB888,
                    SDL_TEXTUREACCESS_STREAMING,
                    n * 320,
                    n * 200,
                )
                .expect("fake platform texture");
            group.bench_function(name, |b| b.iter(|| f(&mut g, black_box(src), texture)));
            g.platform.destroy_texture(texture);
        }
        group.finish();
    }
}

criterion_group!(benches, bench_scalers);
criterion_main!(benches);
