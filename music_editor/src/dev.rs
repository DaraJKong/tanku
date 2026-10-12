//! A run of the editor without anyone at it, for looking at it and listening to it from a
//! script.
//!
//! `MUSIC_EDITOR_SCRIPT` holds commands separated by semicolons, run one after the other:
//! `tab NAME`, which shows a tab, `press LABEL`, which presses the button that bears this
//! label, in capitals or not, `click X Y`, which presses the button at that point of the
//! sheet, `type TEXT`, `key NAME` for `enter`, `escape`, `back` or `space`,
//! `wheel LINES`, which turns the wheel of the mouse that many lines, down if it is negative,
//! `wait SECONDS`, `checks`, which waits for the checks that listen, `canvas WIDTH HEIGHT`,
//! which draws from there on as a window of that many pixels does, `shot PATH`, which saves
//! what is drawn, `tape PATH`, which saves what was played since the run began, and `quit`.
//!
//! A scripted run opens no window: it draws into an image. It makes no sound: what the
//! sound device would have asked for is computed all the same, and kept for `tape`. It only
//! saves the document if it was given the file to open on its command line.

use std::{collections::VecDeque, sync::Mutex, time::Duration};

use bevy::{
    app::ScheduleRunnerPlugin,
    camera::{ImageRenderTarget, RenderTarget},
    input::{
        mouse::{MouseScrollUnit, MouseWheel},
        touch::TouchPhase,
    },
    prelude::*,
    render::{
        render_resource::TextureFormat,
        view::screenshot::{Screenshot, save_to_disk},
    },
    ui::UiSystems,
    window::ExitCondition,
    winit::WinitPlugin,
};
use tanku_audio::{CHANNELS, SAMPLE_RATE};

use crate::{
    actions::{self, Key},
    checks::{Checks, Item},
    doc::Doc,
    sound::Stream,
    state::{Press, Tab, View},
    ui::{MainCamera, SHEET},
};

#[derive(Resource)]
struct Script {
    commands: VecDeque<String>,
    wait: f32,
    /// The label of the button that the script presses, or the point of the sheet where it
    /// clicks.
    press: Option<String>,
    click: Option<Vec2>,
    /// The script waits for the checks that listen.
    checks: bool,
}

/// The sound that nobody plays: what the device would have asked for so far.
#[derive(Resource)]
struct Tape {
    /// Only the system that rolls the tape reads it: the lock is there for the engine, which
    /// shares what it holds between its threads.
    stream: Mutex<Stream>,
    samples: Vec<f32>,
}

/// Whether a script runs the editor.
pub fn scripted() -> bool {
    std::env::var_os("MUSIC_EDITOR_SCRIPT").is_some()
}

/// Adds the engine for a scripted run: no window, and frames at a steady pace.
pub fn add_engine(app: &mut App) {
    app.add_plugins((
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .disable::<WinitPlugin>(),
        ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(1.0 / 60.0)),
    ));
}

pub fn plugin(app: &mut App, stream: Stream) {
    let script = std::env::var("MUSIC_EDITOR_SCRIPT").unwrap_or_default();
    app.insert_resource(Script {
        commands: script
            .split(';')
            .map(|command| command.trim().to_string())
            .collect(),
        wait: 0.0,
        press: None,
        click: None,
        checks: false,
    })
    .insert_resource(Tape {
        stream: Mutex::new(stream),
        samples: Vec::new(),
    })
    .add_systems(PostStartup, draw_into_an_image)
    .add_systems(Update, run_script)
    .add_systems(Last, roll_the_tape)
    .add_systems(PreUpdate, press.after(UiSystems::Focus));
}

/// An image of this size for the editor to draw into.
fn blank(size: UVec2) -> Image {
    Image::new_target_texture(size.x, size.y, TextureFormat::Rgba8UnormSrgb, None)
}

/// Points the camera at an image, which stands in for the window.
fn draw_into_an_image(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    camera: Single<Entity, With<MainCamera>>,
) {
    let canvas = images.add(blank(SHEET.as_uvec2()));
    commands
        .entity(*camera)
        .insert((RenderTarget::from(canvas), IsDefaultUiCamera));
}

/// Computes the sound that the frame would have played, since nothing plays it.
fn roll_the_tape(time: Res<Time<Real>>, mut tape: ResMut<Tape>) {
    let tape = &mut *tape;
    let due = (time.elapsed_secs_f64() * SAMPLE_RATE as f64) as usize * CHANNELS as usize;
    let more = due.saturating_sub(tape.samples.len());
    if let Ok(mut stream) = tape.stream.lock() {
        tape.samples.extend(stream.by_ref().take(more));
    }
}

/// Presses the button that the script asked for, as a click does. It comes after the
/// interface has looked for the cursor, which would let go of the button.
fn press(
    mut script: ResMut<Script>,
    mut buttons: Query<
        (
            &mut Interaction,
            Option<&Children>,
            &ComputedNode,
            &UiGlobalTransform,
        ),
        With<Button>,
    >,
    labels: Query<&Text>,
    scale: Res<UiScale>,
) {
    if let Some(label) = script.press.take() {
        let bears = |children: &Option<&Children>| {
            let children = children.iter().flat_map(|children| children.iter());
            let mut texts = children.filter_map(|child| labels.get(child).ok());
            texts.any(|text| text.0.eq_ignore_ascii_case(&label))
        };
        match buttons.iter_mut().find(|(_, children, ..)| bears(children)) {
            Some((mut interaction, ..)) => *interaction = Interaction::Pressed,
            None => warn!("no button bears the label: {label}"),
        }
    }
    if let Some(point) = script.click.take() {
        // The point is in the pixels of the sheet, which the image shows at some scale.
        let point = point * scale.0;
        let under = buttons.iter_mut().filter(|(_, _, node, place)| {
            let (half, center) = (node.size() / 2.0, place.translation);
            (point - center).abs().cmple(half).all()
        });
        // The smallest of the buttons there is the one on top.
        let smallest = under.min_by(|(_, _, one, _), (_, _, other, _)| {
            let area = |node: &ComputedNode| node.size().x * node.size().y;
            area(one).total_cmp(&area(other))
        });
        match smallest {
            Some((mut interaction, ..)) => *interaction = Interaction::Pressed,
            None => warn!("no button is at {point}"),
        }
    }
}

fn run_script(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut script: ResMut<Script>,
    mut doc: ResMut<Doc>,
    mut view: ResMut<View>,
    checks: Res<Checks>,
    tape: Res<Tape>,
    mut images: ResMut<Assets<Image>>,
    mut camera: Single<(Entity, &RenderTarget, &mut Projection), With<MainCamera>>,
    mut wheel: MessageWriter<MouseWheel>,
    mut exit: MessageWriter<AppExit>,
) {
    script.wait -= time.delta_secs();
    if script.wait > 0.0 || script.press.is_some() || script.click.is_some() {
        return;
    }
    if script.checks {
        // Until every check of the window has been told.
        if checks.count(Item::of(Tab::File)).1 {
            return;
        }
        script.checks = false;
    }
    let Some(command) = script.commands.pop_front() else {
        return;
    };
    let words: Vec<&str> = command.split_whitespace().collect();
    let Some(&name) = words.first() else {
        return;
    };
    let word = |index: usize| words.get(index).copied().unwrap_or_default();
    let number = |index: usize| word(index).parse::<f32>().unwrap_or(0.0);
    match name {
        "wait" => script.wait = number(1),
        "checks" => script.checks = true,
        "tab" => match Tab::ALL
            .into_iter()
            .find(|tab| tab.name().eq_ignore_ascii_case(word(1)))
        {
            Some(tab) => actions::apply(&Press::Tab(tab), false, &mut doc, &mut view),
            None => warn!("no tab is called {}", word(1)),
        },
        "press" => {
            script.press = Some(words[1..].join(" "));
            // The button is only there once the sheet has been drawn again.
            script.wait = 0.05;
        }
        "click" => {
            script.click = Some(Vec2::new(number(1), number(2)));
            script.wait = 0.05;
        }
        "type" => {
            for letter in words[1..].join(" ").chars() {
                actions::typed(&mut doc, &mut view, Key::Letter(letter));
            }
        }
        "key" => match word(1) {
            "enter" => actions::typed(&mut doc, &mut view, Key::Enter),
            "escape" => actions::typed(&mut doc, &mut view, Key::Escape),
            "back" => actions::typed(&mut doc, &mut view, Key::Back),
            "space" => {
                let press = if view.playing {
                    Press::Stop
                } else {
                    Press::Play
                };
                actions::apply(&press, false, &mut doc, &mut view);
            }
            other => warn!("no key is called {other}"),
        },
        "wheel" => {
            wheel.write(MouseWheel {
                unit: MouseScrollUnit::Line,
                x: 0.0,
                y: number(1),
                window: Entity::PLACEHOLDER,
                phase: TouchPhase::Moved,
            });
        }
        "canvas" => {
            let size = UVec2::new(number(1) as u32, number(2) as u32).max(UVec2::ONE);
            let (camera, target, projection) = &mut *camera;
            let Some(canvas) = target.as_image() else {
                return;
            };
            if let Some(mut image) = images.get_mut(canvas) {
                *image = blank(size);
            }
            commands
                .entity(*camera)
                .insert(RenderTarget::Image(ImageRenderTarget {
                    handle: canvas.clone(),
                    scale_factor: 1.0,
                }));
            // The engine only looks at the size of an image a frame after it changed. A
            // projection that changed makes it look at once.
            projection.set_changed();
            script.wait = 0.2;
        }
        "shot" => {
            commands
                .spawn(Screenshot(camera.1.clone()))
                .observe(save_to_disk(std::path::PathBuf::from(word(1))));
            script.wait = 0.5;
        }
        "tape" => {
            if let Err(error) = tanku_audio::wav::write(word(1), &tape.samples) {
                warn!("the tape was not saved: {error}");
            }
        }
        "quit" => {
            exit.write(AppExit::Success);
        }
        other => warn!("unknown command: {other}"),
    }
}
