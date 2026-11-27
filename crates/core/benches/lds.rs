//! The LDS music player: `lds_update` for 1,000 ticks from the start of a
//! song of `music.mus`, at 44.1 kHz.

mod common;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use rustyrian_core::lds_play as l;
use std::hint::black_box;

/// Songs benched: the first and the eleventh of `music.mus`.
const SONGS: [usize; 2] = [0, 10];
const TICKS: usize = 1_000;

fn bench_lds(c: &mut Criterion) {
    let mut group = c.benchmark_group("lds_update 1000 ticks");
    for n in SONGS {
        let start = common::loaded_song(n);
        group.bench_function(format!("song {n}"), |b| {
            b.iter_batched_ref(
                || start.clone(),
                |(lds, opl)| {
                    for _ in 0..TICKS {
                        black_box(l::lds_update(lds, opl));
                    }
                },
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

criterion_group!(benches, bench_lds);
criterion_main!(benches);
