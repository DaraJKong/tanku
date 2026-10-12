//! How long the simulation takes: a tick must fit sixty times in a second with room to spare.

use bevy::prelude::*;
use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use tanku_core::{
    CorePlugin,
    modes::Difficulty,
    sim::{Enemy, Run, RunPhase, RunRequest},
};
use tanku_server::{ServerPlugin, field::Field};

/// A run in which the player stands in a late sector with every enemy in sight awake.
fn fight(sector: u32) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, CorePlugin, ServerPlugin));
    app.world_mut().write_message(RunRequest::Start {
        seed: "BENCH".into(),
        players: 1,
        drawn: true,
        difficulty: Difficulty::Normal,
        free: false,
        tanks: Vec::new(),
    });
    app.world_mut().run_schedule(FixedUpdate);

    // Come back at the barrier of the sector, as after a death at that checkpoint.
    app.world_mut()
        .resource_mut::<Field>()
        .cleared
        .extend(0..=sector);
    let mut run = app.world_mut().resource_mut::<Run>();
    run.checkpoint = sector;
    run.phase = RunPhase::Respawning(0.0);
    app.world_mut().run_schedule(FixedUpdate);
    app.world_mut().run_schedule(FixedUpdate);

    let world = app.world_mut();
    for mut enemy in world.query::<&mut Enemy>().iter_mut(world) {
        enemy.awake = true;
    }
    app
}

fn simulation(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("simulation");
    group.sample_size(20);
    group.bench_function("one second of a fight in sector 30", |bencher| {
        bencher.iter_batched(
            || fight(30),
            |mut app| {
                for _ in 0..60 {
                    app.world_mut().run_schedule(FixedUpdate);
                }
            },
            BatchSize::PerIteration,
        );
    });
    group.finish();
}

criterion_group!(benches, simulation);
criterion_main!(benches);
