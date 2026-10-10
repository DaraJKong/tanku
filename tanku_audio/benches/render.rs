//! How long the music takes to compute. It has to stay far below the time it lasts.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use tanku_audio::{CHANNELS, Context, Mode, Place, SAMPLE_RATE, music_from};

fn render(criterion: &mut Criterion) {
    // The fullest arrangement: every instrument plays, and the lead plays in octaves.
    let fullest = Context {
        place: Place::Arena,
        fight: true,
        threat: 4,
        ..Context::default()
    };
    let mut frames = vec![0.0; 10 * SAMPLE_RATE as usize * CHANNELS as usize];
    criterion.bench_function(
        "ten seconds of a fight in an arena at threat 4",
        |bencher| {
            let mut music = music_from(Mode::Dorian, fullest).0;
            bencher.iter(|| music.render(black_box(&mut frames)));
        },
    );
}

criterion_group!(benches, render);
criterion_main!(benches);
