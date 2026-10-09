//! How long generation takes: it runs while the player crosses a barrier.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use tanku_core::{enemy, map::Corridor};

fn generation(criterion: &mut Criterion) {
    criterion.bench_function("plan and fill 30 sectors", |bencher| {
        bencher.iter(|| {
            let mut corridor = Corridor::new(black_box(42));
            for index in 0..30 {
                black_box(corridor.layout(index));
            }
        });
    });
    criterion.bench_function("generate 100 enemies", |bencher| {
        bencher.iter(|| {
            for index in 0..100 {
                black_box(enemy::generate(black_box(42), 20 + index / 10, index));
            }
        });
    });
}

criterion_group!(benches, generation);
criterion_main!(benches);
