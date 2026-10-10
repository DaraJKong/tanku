//! The heads-up display: rounds, mines, shield, sector and lives.

use bevy::prelude::*;
use tanku_core::{
    progression,
    sim::{
        Enemy, Gun, Mine, Player, Playtest, Position, Run, Shield, ShieldState, SimEvent, Tank,
        Team,
    },
    stats::Stat,
};

use crate::{
    Screen,
    camera::MainCamera,
    scene,
    settings::{Action, Settings},
    theme,
};

/// Seconds a message stays on screen.
const TOAST_TIME: f32 = 2.5;
/// Markers point to the last enemies of a sector when this many remain, or fewer.
const MARKED_ENEMIES: usize = 2;

#[derive(Component)]
struct SectorLabel;

#[derive(Component)]
struct LifePip(u32);

/// What stands for the lives when a playtest doesn't count them.
#[derive(Component)]
struct LivesNote;

#[derive(Component)]
struct RoundPip(u8);

#[derive(Component)]
struct MinePip(u8);

#[derive(Component)]
struct ReloadBar;

#[derive(Component)]
struct ShieldBar;

#[derive(Component)]
struct Hint;

#[derive(Component)]
struct Toast(f32);

#[derive(Component)]
struct EnemyMarker(usize);

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Screen::Playing), build)
        .add_systems(
            Update,
            (status, weapons, toasts, mark_enemies).run_if(in_state(Screen::Playing)),
        );
}

fn pip(marker: impl Component) -> impl Bundle {
    (
        marker,
        Node {
            width: px(14),
            height: px(14),
            border_radius: BorderRadius::all(px(3)),
            ..default()
        },
        BackgroundColor(theme::SURFACE_RAISED),
    )
}

fn label(content: &str) -> impl Bundle {
    (
        Node {
            width: px(64),
            ..default()
        },
        children![theme::text(content, 13.0, theme::TEXT_DIM)],
    )
}

fn row() -> Node {
    Node {
        column_gap: px(5),
        align_items: AlignItems::Center,
        ..default()
    }
}

fn bar(marker: impl Component) -> impl Bundle {
    (
        Node {
            width: px(110),
            height: px(6),
            border_radius: BorderRadius::all(px(3)),
            ..default()
        },
        BackgroundColor(theme::SURFACE_RAISED),
        children![(
            marker,
            Node {
                width: percent(0),
                height: percent(100),
                border_radius: BorderRadius::all(px(3)),
                ..default()
            },
            BackgroundColor(theme::ACCENT),
        )],
    )
}

fn build(mut commands: Commands, playtest: Option<Res<Playtest>>) {
    // A playtest says so, because it sets no record.
    let mode = if playtest.is_some() { "PLAYTEST" } else { "" };
    commands.spawn((
        DespawnOnExit(Screen::Playing),
        Node {
            width: percent(100),
            height: percent(100),
            padding: UiRect::all(px(18)),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        },
        // The display never takes a click away from the game.
        Pickable::IGNORE,
        children![
            (
                Node {
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Start,
                    ..default()
                },
                children![
                    (
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: px(4),
                            ..default()
                        },
                        children![
                            (SectorLabel, theme::text("", 26.0, theme::TEXT)),
                            theme::text(mode, 13.0, theme::ACCENT),
                        ],
                    ),
                    (Toast(0.0), theme::text("", 20.0, theme::TEXT)),
                    (
                        row(),
                        children![
                            label("LIVES"),
                            pip(LifePip(1)),
                            pip(LifePip(2)),
                            pip(LifePip(3)),
                            (LivesNote, theme::text("", 13.0, theme::ACCENT)),
                        ],
                    ),
                ],
            ),
            (
                Node {
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::End,
                    ..default()
                },
                children![
                    (
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: px(8),
                            ..default()
                        },
                        children![
                            (
                                row(),
                                children![
                                    label("ROUNDS"),
                                    pip(RoundPip(1)),
                                    pip(RoundPip(2)),
                                    pip(RoundPip(3)),
                                    pip(RoundPip(4)),
                                    pip(RoundPip(5)),
                                    pip(RoundPip(6)),
                                ],
                            ),
                            (row(), children![label("RELOAD"), bar(ReloadBar)]),
                            (
                                row(),
                                children![
                                    label("MINES"),
                                    pip(MinePip(1)),
                                    pip(MinePip(2)),
                                    pip(MinePip(3)),
                                    pip(MinePip(4)),
                                ],
                            ),
                            (row(), children![label("SHIELD"), bar(ShieldBar)]),
                        ],
                    ),
                    (Hint, theme::text("", 15.0, theme::TEXT_DIM)),
                ],
            ),
        ],
    ));
    for index in 0..MARKED_ENEMIES {
        commands.spawn((
            DespawnOnExit(Screen::Playing),
            EnemyMarker(index),
            Node {
                position_type: PositionType::Absolute,
                width: px(16),
                height: px(16),
                border_radius: BorderRadius::all(px(8)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(theme::DANGER),
            Pickable::IGNORE,
        ));
    }
}

fn status(
    run: Option<Res<Run>>,
    playtest: Option<Res<Playtest>>,
    settings: Res<Settings>,
    mut sector: Single<&mut Text, (With<SectorLabel>, Without<Hint>, Without<LivesNote>)>,
    mut hint: Single<&mut Text, (With<Hint>, Without<SectorLabel>, Without<LivesNote>)>,
    mut note: Single<&mut Text, (With<LivesNote>, Without<SectorLabel>, Without<Hint>)>,
    mut lives: Query<(&LifePip, &mut BackgroundColor, &mut Node)>,
) {
    let Some(run) = run else {
        return;
    };
    sector.0 = if run.sector == 0 {
        "START BAY".to_string()
    } else {
        format!("SECTOR {}", run.sector)
    };
    let infinite_lives = playtest.is_some_and(|playtest| playtest.infinite_lives);
    note.0 = if infinite_lives { "INFINITE" } else { "" }.to_string();
    for (pip, mut color, mut node) in &mut lives {
        node.display = if infinite_lives {
            Display::None
        } else {
            Display::Flex
        };
        color.0 = if pip.0 <= run.lives {
            theme::ACCENT
        } else {
            theme::SURFACE_RAISED
        };
    }
    // The controls are shown where a run starts.
    hint.0 = if run.sector == 0 {
        let key = |action| {
            let bound = settings.bindings(action)[0];
            bound.map_or("?".to_string(), |binding| binding.name())
        };
        format!(
            "{}{}{}{} move, mouse aim, {} shoot, {} mine\n{} shield, {}/{} turn the view, Esc menu",
            key(Action::Up),
            key(Action::Left),
            key(Action::Down),
            key(Action::Right),
            key(Action::Shoot),
            key(Action::Mine),
            key(Action::Shield),
            key(Action::RotateLeft),
            key(Action::RotateRight),
        )
    } else {
        String::new()
    };
}

fn weapons(
    player: Query<(Entity, &Tank, &Gun, &Shield), With<Player>>,
    mines: Query<&Mine>,
    mut rounds: Query<
        (&RoundPip, &mut BackgroundColor, &mut Node),
        (Without<MinePip>, Without<ReloadBar>, Without<ShieldBar>),
    >,
    mut mine_pips: Query<
        (&MinePip, &mut BackgroundColor, &mut Node),
        (Without<RoundPip>, Without<ReloadBar>, Without<ShieldBar>),
    >,
    mut reload: Single<&mut Node, (With<ReloadBar>, Without<ShieldBar>)>,
    mut shield_bar: Single<
        (&mut Node, &mut BackgroundColor),
        (
            With<ShieldBar>,
            Without<ReloadBar>,
            Without<RoundPip>,
            Without<MinePip>,
        ),
    >,
) {
    let Ok((entity, tank, gun, shield)) = player.single() else {
        return;
    };
    let genome = &tank.genome;
    let shown = |visible: bool| {
        if visible {
            Display::Flex
        } else {
            Display::None
        }
    };
    let lit = |on: bool| {
        if on {
            theme::TEXT
        } else {
            theme::SURFACE_RAISED
        }
    };
    for (pip, mut color, mut node) in &mut rounds {
        node.display = shown(pip.0 <= genome.magazine());
        color.0 = lit(pip.0 <= gun.rounds);
    }
    let armed = mines.iter().filter(|mine| mine.owner == entity).count() as u8;
    for (pip, mut color, mut node) in &mut mine_pips {
        node.display = shown(pip.0 <= genome.mines());
        color.0 = lit(pip.0 + armed <= genome.mines());
    }
    reload.width = percent(100.0 * gun.reload / genome.reload_time());

    let (stun, cooldown) = genome.shield().unwrap_or((1.0, 1.0));
    let (fill, color) = match shield.state {
        ShieldState::Active(_) => (1.0, theme::TEXT),
        ShieldState::Overloaded(left) => (left / stun, theme::DANGER),
        ShieldState::Ready if shield.cooldown > 0.0 => {
            (1.0 - shield.cooldown / cooldown, theme::TEXT_DIM)
        }
        ShieldState::Ready => (1.0, theme::ACCENT),
    };
    let has_shield = genome.level(Stat::Shield) > 0;
    shield_bar.0.width = percent(if has_shield { 100.0 * fill } else { 0.0 });
    shield_bar.1.0 = color;
}

/// Short messages at the top of the screen.
fn toasts(
    time: Res<Time>,
    run: Option<Res<Run>>,
    playtest: Option<Res<Playtest>>,
    mut events: MessageReader<SimEvent>,
    mut toast: Single<(&mut Toast, &mut Text, &mut TextColor)>,
) {
    let (toast, text, color) = &mut *toast;
    let infinite_lives = playtest.is_some_and(|playtest| playtest.infinite_lives);
    // Several things can happen at once, such as capturing a flag and unlocking its barrier.
    // The message that matters most is shown.
    let mut news: Option<(u8, String)> = None;
    for event in events.read() {
        let (weight, message) = match *event {
            SimEvent::SectorCleared { sector } if sector > 0 => {
                if progression::is_checkpoint(sector) {
                    (1, format!("Sector {sector} cleared - capture the flag"))
                } else {
                    (1, format!("Sector {sector} cleared"))
                }
            }
            SimEvent::BarrierUnlocked { sector } if sector > 0 => {
                (1, "Barrier unlocked".to_string())
            }
            SimEvent::FlagCaptured { .. } => (2, "Flag captured".to_string()),
            SimEvent::TankDestroyed {
                team: Team::Players,
                ..
            } => {
                // A playtest that doesn't count its lives has none to announce.
                let lives = if infinite_lives {
                    0
                } else {
                    run.as_ref().map_or(0, |run| run.lives)
                };
                let message = match lives {
                    0 => "Tank destroyed".to_string(),
                    1 => "Tank destroyed - last life".to_string(),
                    lives => format!("Tank destroyed - {lives} lives left"),
                };
                (3, message)
            }
            _ => continue,
        };
        if news.as_ref().is_none_or(|(shown, _)| weight >= *shown) {
            news = Some((weight, message));
        }
    }
    if let Some((_, message)) = news {
        text.0 = message;
        toast.0 = TOAST_TIME;
    }
    toast.0 = (toast.0 - time.delta_secs()).max(0.0);
    color.0 = theme::TEXT.with_alpha((toast.0 / 0.5).min(1.0));
}

/// Points to the last enemies of the sector when they are out of view.
fn mark_enemies(
    run: Option<Res<Run>>,
    ui_scale: Res<UiScale>,
    camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    enemies: Query<(&Tank, &Position), With<Enemy>>,
    mut markers: Query<(&EnemyMarker, &mut Node)>,
) {
    let (camera, camera_transform) = *camera;
    let sector = run.map_or(0, |run| run.sector);
    let left: Vec<Vec2> = enemies
        .iter()
        .filter(|(tank, _)| tank.sector == sector)
        .map(|(_, position)| position.0)
        .collect();
    let Some(size) = camera.logical_viewport_size() else {
        return;
    };
    let margin = Vec2::splat(28.0);
    for (marker, mut node) in &mut markers {
        let on_screen = left
            .get(marker.0)
            .filter(|_| left.len() <= MARKED_ENEMIES)
            .and_then(|&at| {
                camera
                    .world_to_viewport(camera_transform, scene::to_world(at, 0.5))
                    .ok()
            });
        // An enemy behind the camera has no place on the screen, but that never happens when
        // looking down from above.
        let Some(at) = on_screen.filter(|at| at.cmplt(Vec2::ZERO).any() || at.cmpgt(size).any())
        else {
            node.display = Display::None;
            continue;
        };
        let edge = at.clamp(margin, size - margin) - Vec2::splat(8.0);
        node.display = Display::Flex;
        node.left = px(edge.x / ui_scale.0);
        node.top = px(edge.y / ui_scale.0);
    }
}
