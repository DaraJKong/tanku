//! Leaving the game without a crash.
//!
//! The engine compiles the pipelines of the graphics card in tasks of their own, off the
//! thread that draws, so that no frame waits for one. Nothing waited for those tasks when
//! the game left either: the process ended under a task that was still inside the driver of
//! the card, and the driver crashed. A player seldom leaves at such a moment. A scripted run
//! that quits a tenth of a second after it starts always did.
//!
//! The frame in which the game is told to leave is the last one that is drawn. Its drawing
//! now ends by waiting for the pipelines that are still being compiled, and the engine waits
//! for that drawing before it lets go of the card.

use std::time::{Duration, Instant};

use bevy::{
    prelude::*,
    render::{
        Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
        render_resource::{CachedPipelineState, PipelineCache},
    },
};

/// How long the game waits for its pipelines at most as it leaves: a driver that never
/// answers must not keep the game from closing.
const PATIENCE: Duration = Duration::from_secs(5);

/// Whether the game was told to leave, as the world that draws knows it.
#[derive(Resource, Default)]
struct Leaving(bool);

pub fn plugin(app: &mut App) {
    // A game without anything to draw with has no pipeline to wait for.
    let Some(render) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    render
        .init_resource::<Leaving>()
        .add_systems(ExtractSchedule, hear_of_it)
        .add_systems(
            Render,
            wait_for_the_pipelines.in_set(RenderSystems::PostCleanup),
        );
}

/// Tells the world that draws that the game leaves: the frame that takes this is its last.
fn hear_of_it(exits: Extract<Res<Messages<AppExit>>>, mut leaving: ResMut<Leaving>) {
    leaving.0 |= !exits.is_empty();
}

/// Holds the last frame until no pipeline is still being compiled.
fn wait_for_the_pipelines(leaving: Res<Leaving>, cache: Res<PipelineCache>) {
    if !leaving.0 {
        return;
    }
    let compiling = || {
        cache.pipelines().any(|pipeline| {
            matches!(&pipeline.state, CachedPipelineState::Creating(task) if !task.is_finished())
        })
    };
    let since = Instant::now();
    while compiling() && since.elapsed() < PATIENCE {
        std::thread::sleep(Duration::from_millis(1));
    }
}
