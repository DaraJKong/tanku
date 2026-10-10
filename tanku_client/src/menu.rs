//! The menus: main menu, pause, settings and game over.

use bevy::prelude::*;
use tanku_core::{
    rules::{PLAYTEST_LAST_SECTOR, TICK_RATE},
    seed::{self, VERSION},
    sim::{Playtest, Run, RunRequest, SimEvent},
};

use crate::{
    Overlay, Screen,
    settings::{Action, Binding, Records, Settings},
    theme,
};

/// The only mode there is so far, as records name it.
const MODE: &str = "normal";
/// Seconds between the end of a run and the game over screen.
const GAME_OVER_DELAY: f32 = 1.4;

/// What a button does.
#[derive(Component, Clone, Copy, PartialEq)]
enum Press {
    Solo,
    OpenSettings,
    Quit,
    Resume,
    Abandon,
    Retry,
    NewRun,
    Back,
    Fullscreen,
    Vsync,
    Shake,
    Volume(f32),
    Scale(f32),
    Rebind(Action, usize),
    ResetBindings,
    /// In a playtest: turns the infinite lives on or off.
    InfiniteLives,
    /// In a playtest: moves the sector to skip to by this many.
    Destination(i32),
    /// In a playtest: skips to that sector.
    Skip,
}

#[derive(Component)]
struct PauseRoot;

#[derive(Component)]
struct SettingsRoot;

/// The sector a playtest offers to skip to.
#[derive(Resource)]
struct Destination(u32);

/// The binding the settings are waiting a key for.
#[derive(Resource, Default)]
struct Capture {
    waiting: Option<(Action, usize)>,
    /// The click that asked for a key isn't the key.
    armed: bool,
    /// A click that became a binding isn't a click on a button.
    swallowed: bool,
}

/// How the last run ended.
#[derive(Resource, Default)]
struct Outcome {
    countdown: Option<f32>,
    new_record: bool,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Capture>()
        .init_resource::<Outcome>()
        .insert_resource(Destination(1))
        .add_systems(OnEnter(Screen::Menu), main_menu)
        .add_systems(OnEnter(Screen::GameOver), game_over)
        .add_systems(
            OnTransition {
                exited: Overlay::None,
                entered: Overlay::Pause,
            },
            offer_the_next_sector.run_if(resource_exists::<Playtest>),
        )
        .add_systems(OnEnter(Overlay::Pause), pause_menu)
        .add_systems(OnEnter(Overlay::Settings), settings_menu)
        .add_systems(
            Update,
            (
                (capture_binding, press, escape).chain(),
                refresh_pause
                    .run_if(in_state(Overlay::Pause).and_then(resource_exists::<Playtest>)),
                refresh_settings.run_if(in_state(Overlay::Settings)),
                hover,
                pause_time,
                watch_run,
            ),
        );
}

/// A screen-wide container with a column of content in its middle.
fn panel(
    commands: &mut Commands,
    scope: impl Bundle,
    scrim: Color,
    content: impl FnOnce(&mut ChildSpawnerCommands),
) {
    commands
        .spawn((
            scope,
            Node {
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(scrim),
            GlobalZIndex(10),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Stretch,
                    row_gap: px(10),
                    padding: UiRect::all(px(28)),
                    min_width: px(340),
                    border_radius: BorderRadius::all(px(12)),
                    ..default()
                },
                BackgroundColor(theme::SURFACE),
            ))
            .with_children(content);
        });
}

fn heading(parent: &mut ChildSpawnerCommands, content: &str) {
    parent.spawn((
        theme::text(content, 40.0, theme::TEXT),
        Node {
            margin: UiRect::bottom(px(8)),
            ..default()
        },
    ));
}

fn line(parent: &mut ChildSpawnerCommands, content: impl Into<String>, color: Color) {
    parent.spawn(theme::text(content, 17.0, color));
}

fn button_node(width: Val) -> Node {
    Node {
        width,
        padding: UiRect::axes(px(16), px(9)),
        justify_content: JustifyContent::Center,
        border_radius: BorderRadius::all(px(7)),
        ..default()
    }
}

fn button(parent: &mut ChildSpawnerCommands, content: impl Into<String>, press: Press) {
    parent.spawn((
        Button,
        press,
        button_node(Val::Auto),
        BackgroundColor(theme::SURFACE_RAISED),
        children![theme::text(content, 20.0, theme::TEXT)],
    ));
}

/// A button for something that isn't there yet.
fn placeholder(parent: &mut ChildSpawnerCommands, content: &str) {
    parent.spawn((
        button_node(Val::Auto),
        BackgroundColor(theme::SURFACE_RAISED.with_alpha(0.4)),
        children![theme::text(
            format!("{content} (later)"),
            20.0,
            theme::TEXT_DIM
        )],
    ));
}

/// A label followed by its controls.
fn row(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    controls: impl FnOnce(&mut ChildSpawnerCommands),
) {
    parent
        .spawn(Node {
            column_gap: px(6),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|row| {
            // The label sits in a box of fixed width, so that the controls line up.
            row.spawn((
                Node {
                    width: px(200),
                    ..default()
                },
                children![theme::text(label, 16.0, theme::TEXT)],
            ));
            controls(row);
        });
}

/// A small button, for the controls of a row.
fn small(parent: &mut ChildSpawnerCommands, content: impl Into<String>, width: f32, press: Press) {
    parent.spawn((
        Button,
        press,
        Node {
            padding: UiRect::axes(px(6), px(5)),
            ..button_node(px(width))
        },
        BackgroundColor(theme::SURFACE_RAISED),
        children![theme::text(content, 15.0, theme::TEXT)],
    ));
}

fn on_off(on: bool) -> &'static str {
    if on { "On" } else { "Off" }
}

fn record_line(records: &Records) -> String {
    match records.best(MODE, 1) {
        Some(record) => format!(
            "Best run: sector {}, seed {}, v{}",
            record.sector, record.seed, record.version
        ),
        None => "No run yet".to_string(),
    }
}

fn main_menu(mut commands: Commands, records: Res<Records>, playtest: Option<Res<Playtest>>) {
    panel(
        &mut commands,
        DespawnOnExit(Screen::Menu),
        Color::NONE,
        |parent| {
            heading(parent, "TANKU");
            button(parent, "Solo", Press::Solo);
            placeholder(parent, "Host a game");
            placeholder(parent, "Join a game");
            placeholder(parent, "Achievements");
            button(parent, "Settings", Press::OpenSettings);
            // A browser tab is closed by the browser.
            if cfg!(not(target_arch = "wasm32")) {
                button(parent, "Quit", Press::Quit);
            }
            line(parent, record_line(&records), theme::TEXT_DIM);
            line(parent, format!("Version {VERSION}"), theme::TEXT_DIM);
            if playtest.is_some() {
                line(
                    parent,
                    "Playtest: cheats are in the pause menu",
                    theme::ACCENT,
                );
                line(parent, "A playtest sets no record", theme::TEXT_DIM);
            }
        },
    );
}

/// A playtest offers to skip the sector the player is in.
fn offer_the_next_sector(run: Option<Res<Run>>, mut destination: ResMut<Destination>) {
    if let Some(run) = run {
        destination.0 = (run.sector + 1).min(PLAYTEST_LAST_SECTOR);
    }
}

fn pause_menu(
    mut commands: Commands,
    run: Option<Res<Run>>,
    playtest: Option<Res<Playtest>>,
    destination: Res<Destination>,
) {
    panel(
        &mut commands,
        (PauseRoot, DespawnOnExit(Overlay::Pause)),
        theme::SCRIM,
        |parent| {
            heading(parent, "Paused");
            button(parent, "Resume", Press::Resume);
            button(parent, "Settings", Press::OpenSettings);
            button(parent, "Quit to the menu", Press::Abandon);
            if let Some(playtest) = playtest {
                line(parent, "PLAYTEST", theme::TEXT_DIM);
                row(parent, "Infinite lives", |row| {
                    let state = on_off(playtest.infinite_lives);
                    small(row, state, 70.0, Press::InfiniteLives);
                });
                row(parent, "Sector to skip to", |row| {
                    for change in [-10, -1, 1, 10] {
                        small(row, format!("{change:+}"), 48.0, Press::Destination(change));
                    }
                });
                let skip = format!("Skip to sector {}", destination.0);
                button(parent, skip, Press::Skip);
            }
            if let Some(run) = run {
                line(
                    parent,
                    format!("Seed {}, v{VERSION}", run.seed_text),
                    theme::TEXT_DIM,
                );
            }
        },
    );
}

/// Draws the pause menu again when a playtest changed one of its options.
fn refresh_pause(
    mut commands: Commands,
    run: Option<Res<Run>>,
    playtest: Res<Playtest>,
    destination: Res<Destination>,
    root: Query<Entity, With<PauseRoot>>,
) {
    if !playtest.is_changed() && !destination.is_changed() {
        return;
    }
    for entity in &root {
        commands.entity(entity).despawn();
    }
    pause_menu(commands, run, Some(playtest), destination);
}

/// Keeps the record of a run, unless it was a playtest. Returns whether the record is new.
fn keep_record(records: &mut Records, playtest: bool, run: &Run) -> bool {
    !playtest && records.submit(MODE, 1, run.cleared, &run.seed_text)
}

fn game_over(
    mut commands: Commands,
    run: Option<Res<Run>>,
    records: Res<Records>,
    outcome: Res<Outcome>,
    playtest: Option<Res<Playtest>>,
) {
    panel(
        &mut commands,
        DespawnOnExit(Screen::GameOver),
        theme::SCRIM,
        |parent| {
            heading(parent, "Game over");
            if let Some(run) = run {
                let seconds = run.ticks / TICK_RATE as u64;
                let sectors = if run.cleared == 1 {
                    "sector"
                } else {
                    "sectors"
                };
                line(
                    parent,
                    format!("{} {sectors} cleared", run.cleared),
                    theme::TEXT,
                );
                line(
                    parent,
                    format!("Time {}:{:02}", seconds / 60, seconds % 60),
                    theme::TEXT,
                );
                line(parent, format!("Kills {}", run.kills), theme::TEXT);
                line(
                    parent,
                    format!("Seed {}, v{VERSION}", run.seed_text),
                    theme::TEXT_DIM,
                );
            }
            if playtest.is_some() {
                line(parent, "Playtest: no record kept", theme::TEXT_DIM);
            } else if outcome.new_record {
                line(parent, "New record", theme::GOOD);
            } else {
                line(parent, record_line(&records), theme::TEXT_DIM);
            }
            button(parent, "Retry this seed", Press::Retry);
            button(parent, "New run", Press::NewRun);
            button(parent, "Menu", Press::Abandon);
        },
    );
}

fn settings_menu(mut commands: Commands, settings: Res<Settings>, capture: Res<Capture>) {
    panel(
        &mut commands,
        (SettingsRoot, DespawnOnExit(Overlay::Settings)),
        theme::SCRIM,
        |parent| {
            heading(parent, "Settings");
            parent
                .spawn(Node {
                    column_gap: px(36),
                    ..default()
                })
                .with_children(|columns| {
                    let column = Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: px(7),
                        ..default()
                    };
                    columns.spawn(column.clone()).with_children(|controls| {
                        line(controls, "CONTROLS", theme::TEXT_DIM);
                        for action in Action::ALL {
                            row(controls, action.label(), |row| {
                                for (slot, binding) in
                                    settings.bindings(action).into_iter().enumerate()
                                {
                                    let content = if capture.waiting == Some((action, slot)) {
                                        "press a key".to_string()
                                    } else {
                                        binding.map_or("-".to_string(), Binding::name)
                                    };
                                    small(row, content, 126.0, Press::Rebind(action, slot));
                                }
                            });
                        }
                        row(controls, "Menu", |row| {
                            row.spawn(theme::text("Esc", 16.0, theme::TEXT_DIM));
                        });
                        button(controls, "Reset the controls", Press::ResetBindings);
                    });

                    columns.spawn(column).with_children(|options| {
                        line(options, "VIDEO", theme::TEXT_DIM);
                        row(options, "Fullscreen", |row| {
                            small(row, on_off(settings.fullscreen), 70.0, Press::Fullscreen);
                        });
                        row(options, "Vertical sync", |row| {
                            small(row, on_off(settings.vsync), 70.0, Press::Vsync);
                        });
                        line(options, "AUDIO", theme::TEXT_DIM);
                        row(options, "Volume", |row| {
                            small(row, "-", 34.0, Press::Volume(-0.1));
                            row.spawn(theme::text(
                                format!("{:.0}%", settings.volume * 100.0),
                                16.0,
                                theme::TEXT,
                            ));
                            small(row, "+", 34.0, Press::Volume(0.1));
                        });
                        line(options, "ACCESSIBILITY", theme::TEXT_DIM);
                        row(options, "Camera shake", |row| {
                            small(row, on_off(settings.camera_shake), 70.0, Press::Shake);
                        });
                        row(options, "Interface size", |row| {
                            small(row, "-", 34.0, Press::Scale(-0.25));
                            row.spawn(theme::text(
                                format!("{:.0}%", settings.ui_scale * 100.0),
                                16.0,
                                theme::TEXT,
                            ));
                            small(row, "+", 34.0, Press::Scale(0.25));
                        });
                    });
                });
            button(parent, "Back", Press::Back);
        },
    );
}

/// Draws the settings again when one of them changed.
fn refresh_settings(
    mut commands: Commands,
    settings: Res<Settings>,
    capture: Res<Capture>,
    root: Query<Entity, With<SettingsRoot>>,
) {
    if !settings.is_changed() && !capture.is_changed() {
        return;
    }
    for entity in &root {
        commands.entity(entity).despawn();
    }
    settings_menu(commands, settings, capture);
}

/// Gives the next key or button pressed to the binding that waits for one.
fn capture_binding(
    mut capture: ResMut<Capture>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut settings: ResMut<Settings>,
) {
    let Some((action, slot)) = capture.waiting else {
        return;
    };
    if !capture.armed {
        capture.armed = true;
    } else if keys.just_pressed(KeyCode::Escape) {
        capture.waiting = None;
    } else if let Some(binding) = Binding::pressed_now(&keys, &mouse) {
        settings.bind(action, slot, binding);
        capture.waiting = None;
        capture.swallowed = matches!(binding, Binding::Mouse(_));
    }
}

fn press(
    buttons: Query<(&Interaction, &Press), Changed<Interaction>>,
    screen: Res<State<Screen>>,
    mut next_screen: ResMut<NextState<Screen>>,
    mut next_overlay: ResMut<NextState<Overlay>>,
    mut settings: ResMut<Settings>,
    mut capture: ResMut<Capture>,
    mut records: ResMut<Records>,
    mut playtest: Option<ResMut<Playtest>>,
    mut destination: ResMut<Destination>,
    run: Option<Res<Run>>,
    time: Res<Time<Real>>,
    mut requests: MessageWriter<RunRequest>,
    mut exit: MessageWriter<AppExit>,
) {
    if capture.swallowed {
        capture.swallowed = false;
        return;
    }
    for (interaction, press) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *press {
            Press::Solo | Press::NewRun | Press::Retry => {
                let seed = match (*press, &run) {
                    (Press::Retry, Some(run)) => run.seed_text.clone(),
                    // The moment of the click is all the entropy a seed needs.
                    _ => seed::random_seed_text(time.elapsed().as_nanos() as u64),
                };
                requests.write(RunRequest::Start { seed });
                next_screen.set(Screen::Playing);
                next_overlay.set(Overlay::None);
            }
            Press::OpenSettings => next_overlay.set(Overlay::Settings),
            Press::Resume => next_overlay.set(Overlay::None),
            Press::Back => {
                capture.waiting = None;
                next_overlay.set(if *screen.get() == Screen::Playing {
                    Overlay::Pause
                } else {
                    Overlay::None
                });
            }
            Press::Abandon => {
                if let Some(run) = &run {
                    keep_record(&mut records, playtest.is_some(), run);
                }
                requests.write(RunRequest::Abandon);
                next_screen.set(Screen::Menu);
                next_overlay.set(Overlay::None);
            }
            Press::Quit => {
                exit.write(AppExit::Success);
            }
            Press::Fullscreen => settings.fullscreen = !settings.fullscreen,
            Press::Vsync => settings.vsync = !settings.vsync,
            Press::Shake => settings.camera_shake = !settings.camera_shake,
            Press::Volume(change) => {
                settings.volume =
                    ((settings.volume + change) * 10.0).round().clamp(0.0, 10.0) / 10.0;
            }
            Press::Scale(change) => {
                settings.ui_scale = (settings.ui_scale + change).clamp(0.75, 2.0)
            }
            Press::Rebind(action, slot) => {
                capture.waiting = Some((action, slot));
                capture.armed = false;
            }
            Press::ResetBindings => settings.reset_bindings(),
            Press::InfiniteLives => {
                if let Some(playtest) = &mut playtest {
                    playtest.infinite_lives = !playtest.infinite_lives;
                }
            }
            Press::Destination(change) => {
                let sector = destination.0.saturating_add_signed(change);
                destination.0 = sector.clamp(1, PLAYTEST_LAST_SECTOR);
            }
            // The game goes on at once, in front of that sector.
            Press::Skip => {
                requests.write(RunRequest::SkipTo {
                    sector: destination.0,
                });
                next_overlay.set(Overlay::None);
            }
        }
    }
}

/// Esc opens and closes the menu during a run, and leaves the settings.
fn escape(
    keys: Res<ButtonInput<KeyCode>>,
    capture: Res<Capture>,
    screen: Res<State<Screen>>,
    overlay: Res<State<Overlay>>,
    mut next_overlay: ResMut<NextState<Overlay>>,
) {
    if !keys.just_pressed(KeyCode::Escape) || capture.waiting.is_some() {
        return;
    }
    let playing = *screen.get() == Screen::Playing;
    match *overlay.get() {
        Overlay::None if playing => next_overlay.set(Overlay::Pause),
        Overlay::Pause => next_overlay.set(Overlay::None),
        Overlay::Settings if playing => next_overlay.set(Overlay::Pause),
        Overlay::Settings => next_overlay.set(Overlay::None),
        Overlay::None => {}
    }
}

fn hover(
    mut buttons: Query<(&Interaction, &mut BackgroundColor), (Changed<Interaction>, With<Press>)>,
) {
    for (interaction, mut color) in &mut buttons {
        color.0 = match interaction {
            Interaction::Pressed => theme::ACCENT,
            Interaction::Hovered => theme::SURFACE_HOVERED,
            Interaction::None => theme::SURFACE_RAISED,
        };
    }
}

/// In solo, an open menu pauses the game.
fn pause_time(
    screen: Res<State<Screen>>,
    overlay: Res<State<Overlay>>,
    mut time: ResMut<Time<Virtual>>,
) {
    let paused = *screen.get() == Screen::Playing && *overlay.get() != Overlay::None;
    if paused && !time.is_paused() {
        time.pause();
    } else if !paused && time.is_paused() {
        time.unpause();
    }
}

/// Shows the game over screen a moment after the last life is lost, and keeps the record.
fn watch_run(
    time: Res<Time>,
    mut events: MessageReader<SimEvent>,
    mut outcome: ResMut<Outcome>,
    mut records: ResMut<Records>,
    playtest: Option<Res<Playtest>>,
    run: Option<Res<Run>>,
    screen: Res<State<Screen>>,
    mut next_screen: ResMut<NextState<Screen>>,
    mut next_overlay: ResMut<NextState<Overlay>>,
) {
    if events.read().any(|event| *event == SimEvent::RunOver) {
        outcome.countdown = Some(GAME_OVER_DELAY);
    }
    let Some(left) = outcome.countdown else {
        return;
    };
    if left > time.delta_secs() {
        outcome.countdown = Some(left - time.delta_secs());
        return;
    }
    outcome.countdown = None;
    if let (Some(run), Screen::Playing) = (run, *screen.get()) {
        outcome.new_record = keep_record(&mut records, playtest.is_some(), &run);
        next_screen.set(Screen::GameOver);
        next_overlay.set(Overlay::None);
    }
}

#[cfg(test)]
mod tests {
    use tanku_core::sim::RunPhase;

    use super::*;

    #[test]
    fn a_playtest_sets_no_record() {
        let run = Run {
            seed_text: "TEST".into(),
            seed: 1,
            sector: 41,
            cleared: 40,
            checkpoint: 40,
            lives: 3,
            kills: 0,
            ticks: 0,
            phase: RunPhase::Over,
        };
        let mut records = Records::default();
        assert!(!keep_record(&mut records, true, &run));
        assert_eq!(records, Records::default());
    }
}
