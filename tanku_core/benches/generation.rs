//! How long generation takes: it runs while the player crosses a barrier.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use tanku_core::{enemy, map::Corridor, modes::Difficulty};

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
            for sector in 20..30 {
                black_box(enemy::group(black_box(42), Difficulty::Normal, sector, 10));
            }
        });
    });
}

criterion_group!(benches, generation);
criterion_main!(benches);
