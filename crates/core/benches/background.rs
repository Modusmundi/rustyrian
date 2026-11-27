//! Background layers and the smoothie filters, on the `demo-cycle` fixture
//! states (demo 1 mid-level; `cargo xtask fixtures`). One iteration draws one
//! layer or filters one 320×200 frame. Drawing scrolls the layer, so each
//! draw starts from the same background state (restored outside the timing).

mod common;

use criterion::{Criterion, criterion_group, criterion_main};
use rustyrian_core::Game;
use rustyrian_core::backgrnd as bg;
use rustyrian_core::video::ScreenId;
use std::hint::black_box;
use std::time::{Duration, Instant};

type Draw = fn(&mut Game, ScreenId);
type Filter = fn(&mut Game, ScreenId, ScreenId);

const LAYERS: [(&str, Draw); 4] = [
    ("draw_background_1", bg::draw_background_1),
    ("draw_background_2", bg::draw_background_2),
    ("draw_background_2_blend", bg::draw_background_2_blend),
    ("draw_background_3", bg::draw_background_3),
];

const SMOOTHIES: [(&str, Filter); 4] = [
    ("lava_filter", bg::lava_filter),
    ("water_filter", bg::water_filter),
    ("iced_blur_filter", bg::iced_blur_filter),
    ("blur_filter", bg::blur_filter),
];

fn bench_background(c: &mut Criterion) {
    for frame in common::FIXTURE_FRAMES {
        let mut g = common::fixture_game(frame);
        let vga = g.video.vga_screen.expect("VGAScreen");
        let game_screen = g.video.game_screen.expect("game_screen");
        let mut group = c.benchmark_group(format!("background/demo-cycle@{frame}"));
        let start = g.backgrnd.clone();
        for (name, f) in LAYERS {
            group.bench_function(name, |b| {
                b.iter_custom(|iters| {
                    let mut total = Duration::ZERO;
                    for _ in 0..iters {
                        g.backgrnd.clone_from(&start);
                        let t = Instant::now();
                        f(&mut g, black_box(vga));
                        total += t.elapsed();
                    }
                    total
                })
            });
        }
        // As the game calls them: filter VGAScreen into game_screen.
        for (name, f) in SMOOTHIES {
            group.bench_function(name, |b| {
                b.iter(|| f(&mut g, black_box(game_screen), black_box(vga)))
            });
        }
        group.finish();
        black_box(&g.video.screens);
    }
}

criterion_group!(benches, bench_background);
criterion_main!(benches);
