//! The OPL2 emulator: `adlib_getsample` for 1 s at 44.1 kHz, with the chip
//! programmed by a playing song (`music.mus` song 0 after 200 LDS ticks).

mod common;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use rustyrian_core::crand;
use rustyrian_core::lds_play as l;
use rustyrian_core::opl::{self, Bits};
use std::hint::black_box;
use std::sync::Mutex;

fn bench_opl(c: &mut Criterion) {
    let (mut lds, mut opl) = common::loaded_song(0);
    for _ in 0..200 {
        l::lds_update(&mut lds, &mut opl);
    }
    let rand = Mutex::new(crand::State::default());
    let samples = common::RATE as usize;
    let mut out = vec![0i16; samples];
    c.bench_function("adlib_getsample 1 s at 44.1 kHz", |b| {
        b.iter_batched_ref(
            || opl.clone(),
            |s| {
                opl::adlib_getsample(s, &rand, &mut out, samples as Bits);
                black_box(&out);
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, bench_opl);
criterion_main!(benches);
