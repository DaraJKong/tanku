//! Where the time of a frame goes, in a scripted game. It is a development tool, which comes
//! with the `script` feature, and `TANKU_PROFILE=SECONDS` turns it on: it leaves out that many
//! seconds from the start, where the game loads, and measures the rest of the run. When the
//! script quits, the game prints in lines that start with `PROFILE`:
//!
//! - how long the main schedule took in each frame, from `First` to `Last`, and how long it was
//!   from one frame to the next: at the median, at the 90th and the 99th percentile and at
//!   worst;
//! - each frame of the main schedule that took longer than `TANKU_PROFILE_SLOW` milliseconds, 8
//!   unless told, with the systems that took the most of it;
//! - how long each pass of the graphics card took on average;
//! - how many components of the kinds that the drawing reads changed in a frame, what the
//!   interface painted 300 frames into the measure, and what the world held at the end;
//! - how long each span took in a frame, by name, the dearest first. A build with the feature
//!   `profile` has a span for every system and every schedule, which costs a little time of its
//!   own. `TANKU_PROFILE_CSV=FILE` writes them all to a file.
//!
//! `TANKU_PROFILE_UNCAPPED` lets the frames come as fast as they can. A graphics card that has
//! little to do lowers its clocks, and every pass then takes longer: at 60 frames a second,
//! the card of the machine that this was written on stayed at its lowest clocks.
//! `tanku_client/dev/profile.sh` runs the scenes that the README names, and reads the clocks of
//! the card meanwhile where it can.

use std::{
    cell::RefCell,
    cmp::Reverse,
    collections::{BTreeMap, HashMap},
    fmt::Write,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
    time::{Duration, Instant},
};

use bevy::{
    diagnostic::DiagnosticsStore,
    log::{
        BoxedLayer,
        tracing::{
            Subscriber,
            callsite::Identifier,
            field::{Field, Visit},
            span::{Attributes, Id},
        },
        tracing_subscriber::{Layer, layer::Context, registry::LookupSpan},
    },
    prelude::*,
    render::diagnostic::RenderDiagnosticsPlugin,
    text::TextLayoutInfo,
};

/// A frame of the main schedule that takes longer than this is told of, in milliseconds.
const SLOW: f64 = 8.0;
/// How many systems each slow frame lists, and how many spans the lines show.
const SLOWEST: usize = 12;
const DEAREST: usize = 40;
/// The frame of the measure on which the interface is counted: 5 s in at 60 frames a second.
const CENSUS_FRAME: usize = 300;
/// How many of the largest boxes that the interface paints are listed.
const LARGEST: usize = 5;

/// How many seconds from the start the profile leaves out, if it is on.
fn asked() -> Option<f32> {
    let seconds = std::env::var("TANKU_PROFILE").ok()?;
    Some(seconds.trim().parse().unwrap_or(0.0))
}

/// Whether the frames of a profiled run come as fast as they can.
pub fn uncapped() -> bool {
    asked().is_some() && std::env::var_os("TANKU_PROFILE_UNCAPPED").is_some()
}

/// The layer of the engine's log that times the spans, if the profile is on.
pub fn layer(_: &mut App) -> Option<BoxedLayer> {
    asked()?;
    Some(Box::new(Timing))
}

pub fn plugin(app: &mut App) {
    let Some(after) = asked() else {
        return;
    };
    let slow = std::env::var("TANKU_PROFILE_SLOW")
        .ok()
        .and_then(|slow| slow.trim().parse().ok())
        .unwrap_or(SLOW);
    app.insert_resource(Profile {
        after,
        slow,
        ..default()
    })
    .add_plugins(RenderDiagnosticsPlugin)
    .add_systems(First, begin)
    .add_systems(Last, (end, report).chain());
}

/// The spans seen so far, and how long they lasted in all while the profile measured.
#[derive(Default)]
struct Spans {
    names: Vec<String>,
    /// Where each name is in `names`, by the place of the span in the code and the value of
    /// its field `name`, which tells the systems and the schedules apart.
    known: HashMap<Identifier, HashMap<String, usize>>,
    totals: Vec<(Duration, u64)>,
}

impl Spans {
    fn index(&mut self, callsite: Identifier, kind: &str, name: &str) -> usize {
        let known = self.known.entry(callsite).or_default();
        if let Some(index) = known.get(name) {
            return *index;
        }
        let index = self.names.len();
        self.names.push(if name.is_empty() {
            kind.to_string()
        } else {
            format!("{kind}:{name}")
        });
        self.totals.push((Duration::ZERO, 0));
        known.insert(name.to_string(), index);
        index
    }
}

static SPANS: LazyLock<Mutex<Spans>> = LazyLock::new(Default::default);

/// The spans, even after a thread panicked while it held them: they are only numbers.
fn spans() -> MutexGuard<'static, Spans> {
    SPANS.lock().unwrap_or_else(PoisonError::into_inner)
}

thread_local! {
    /// The spans that this thread is in, with the moment it entered each one.
    static ENTERED: RefCell<Vec<(u64, Instant)>> = const { RefCell::new(Vec::new()) };
    /// The field `name` of the span that is being made.
    static NAME: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Which of the names of `Spans` a span has.
struct Label(usize);

/// Reads the field `name` of a span, which the engine gives to its systems and schedules.
struct NameField<'a>(&'a mut String);

impl Visit for NameField<'_> {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "name" {
            self.0.push_str(value);
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "name" {
            let _ = write!(self.0, "{value:?}");
        }
    }
}

/// Times every span, by its name.
struct Timing;

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for Timing {
    fn on_new_span(&self, attributes: &Attributes<'_>, id: &Id, context: Context<'_, S>) {
        let label = NAME.with_borrow_mut(|name| {
            name.clear();
            attributes.record(&mut NameField(name));
            let metadata = attributes.metadata();
            spans().index(metadata.callsite(), metadata.name(), name)
        });
        if let Some(span) = context.span(id) {
            span.extensions_mut().insert(Label(label));
        }
    }

    fn on_enter(&self, id: &Id, _: Context<'_, S>) {
        ENTERED.with_borrow_mut(|entered| entered.push((id.into_u64(), Instant::now())));
    }

    fn on_exit(&self, id: &Id, context: Context<'_, S>) {
        let now = Instant::now();
        let entered = ENTERED.with_borrow_mut(|entered| {
            let at = entered
                .iter()
                .rposition(|(span, _)| *span == id.into_u64())?;
            Some(entered.remove(at).1)
        });
        let Some(entered) = entered else {
            return;
        };
        let Some(label) = context
            .span(id)
            .and_then(|span| span.extensions().get::<Label>().map(|label| label.0))
        else {
            return;
        };
        if let Some(total) = spans().totals.get_mut(label) {
            total.0 += now - entered;
            total.1 += 1;
        }
    }
}

/// What the profile has measured.
#[derive(Resource, Default)]
struct Profile {
    after: f32,
    slow: f64,
    measuring: bool,
    /// When the frame that runs started.
    started: Option<Instant>,
    /// How long the main schedule took in each frame, and the time from the start of a frame
    /// to the start of the next, in milliseconds.
    work: Vec<f64>,
    periods: Vec<f64>,
    /// How long each span had lasted in all when the frame started.
    before: Vec<Duration>,
    /// The frames that were slow, told as they came.
    slow_frames: Vec<String>,
    /// How many components of each kind of `CHANGED` changed, over every frame.
    changed: [usize; CHANGED.len()],
    /// What the interface painted on the frame of the census.
    painted: Vec<String>,
    /// How long each pass of the graphics card took, summed, and how many frames told it.
    passes: BTreeMap<String, (f64, u64)>,
}

/// The kinds of components whose changes are counted.
const CHANGED: [&str; 8] = [
    "transform",
    "global transform",
    "node",
    "visibility",
    "background",
    "border",
    "text color",
    "text",
];

/// Starts the frame, and the measure once the seconds that are left out are over.
fn begin(time: Res<Time<Real>>, mut profile: ResMut<Profile>) {
    let now = Instant::now();
    if !profile.measuring && time.elapsed_secs() >= profile.after {
        profile.measuring = true;
        spans().totals.fill((Duration::ZERO, 0));
    } else if let Some(started) = profile.started
        && profile.measuring
    {
        profile.periods.push((now - started).as_secs_f64() * 1e3);
    }
    profile.started = Some(now);
    if profile.measuring {
        profile.before = spans().totals.iter().map(|(time, _)| *time).collect();
    }
}

/// Ends the frame: how long it took, what it spent if that was long, what changed in it, and
/// what the graphics card told of the frames that it drew.
fn end(
    time: Res<Time<Real>>,
    mut profile: ResMut<Profile>,
    store: Res<DiagnosticsStore>,
    changed: (
        Query<Ref<Transform>>,
        Query<Ref<GlobalTransform>>,
        Query<Ref<Node>>,
        Query<Ref<Visibility>>,
        Query<Ref<BackgroundColor>>,
        Query<Ref<BorderColor>>,
        Query<Ref<TextColor>>,
        Query<Ref<Text>>,
    ),
    interface: Query<(
        &ComputedNode,
        &InheritedVisibility,
        &BackgroundColor,
        &BorderColor,
        Option<&Outline>,
    )>,
    cameras: Query<&Camera>,
) {
    let Some(started) = profile.started.filter(|_| profile.measuring) else {
        return;
    };
    let work = started.elapsed().as_secs_f64() * 1e3;
    profile.work.push(work);
    if work > profile.slow {
        let told = slow_frame(work, time.elapsed_secs(), &profile.before);
        profile.slow_frames.push(told);
    }
    let (transforms, globals, nodes, visibilities, backgrounds, borders, colors, texts) = changed;
    let counts = [
        transforms.iter().filter(|it| it.is_changed()).count(),
        globals.iter().filter(|it| it.is_changed()).count(),
        nodes.iter().filter(|it| it.is_changed()).count(),
        visibilities.iter().filter(|it| it.is_changed()).count(),
        backgrounds.iter().filter(|it| it.is_changed()).count(),
        borders.iter().filter(|it| it.is_changed()).count(),
        colors.iter().filter(|it| it.is_changed()).count(),
        texts.iter().filter(|it| it.is_changed()).count(),
    ];
    for (sum, count) in profile.changed.iter_mut().zip(counts) {
        *sum += count;
    }
    if profile.work.len() == CENSUS_FRAME {
        let screen = cameras
            .iter()
            .filter_map(Camera::physical_target_size)
            .map(|size| size.as_vec2())
            .next()
            .unwrap_or(Vec2::ONE);
        profile.painted = census(&interface, screen);
    }
    for diagnostic in store.iter() {
        let path = diagnostic.path().as_str();
        if !path.ends_with("elapsed_gpu") {
            continue;
        }
        if let Some(measurement) = diagnostic.measurement() {
            let pass = profile.passes.entry(path.to_string()).or_default();
            pass.0 += measurement.value;
            pass.1 += 1;
        }
    }
}

/// What a slow frame spent on its systems, the dearest first.
fn slow_frame(work: f64, at: f32, before: &[Duration]) -> String {
    let spans = spans();
    let mut spent: Vec<(Duration, &str)> = spans
        .totals
        .iter()
        .zip(&spans.names)
        .enumerate()
        .filter(|(_, (_, name))| name.starts_with("system:"))
        .map(|(index, ((time, _), name))| {
            let earlier = before.get(index).copied().unwrap_or_default();
            (time.saturating_sub(earlier), name.as_str())
        })
        .filter(|(time, _)| !time.is_zero())
        .collect();
    spent.sort_by_key(|(time, _)| Reverse(*time));
    let mut told = format!("PROFILE slow frame of {work:.2} ms at {at:.2} s");
    for (time, name) in spent.iter().take(SLOWEST) {
        let _ = write!(told, "\nPROFILE   {:>8.3} ms  {name}", ms(*time));
    }
    told
}

/// How many boxes the interface paints and how large, as screens, and the largest of them.
fn census(
    interface: &Query<(
        &ComputedNode,
        &InheritedVisibility,
        &BackgroundColor,
        &BorderColor,
        Option<&Outline>,
    )>,
    screen: Vec2,
) -> Vec<String> {
    let mut painted: Vec<(f32, String)> = Vec::new();
    for (computed, visible, background, border, outline) in interface {
        if !visible.get() {
            continue;
        }
        let edges = [border.top, border.right, border.bottom, border.left];
        let edged = computed.border() != Default::default()
            && edges.iter().any(|color| !color.is_fully_transparent());
        let filled = !background.0.is_fully_transparent();
        let outlined = outline.is_some_and(|outline| {
            !outline.color.is_fully_transparent() && outline.width != Val::ZERO
        });
        if !(edged || filled || outlined) {
            continue;
        }
        let size = computed.size();
        let what = [(filled, "filled"), (edged, "edged"), (outlined, "outlined")]
            .into_iter()
            .filter_map(|(is, word)| is.then_some(word))
            .collect::<Vec<_>>()
            .join(" and ");
        painted.push((size.x * size.y, format!("{} by {}, {what}", size.x, size.y)));
    }
    painted.sort_by(|a, b| b.0.total_cmp(&a.0));
    let area: f32 = painted.iter().map(|(area, _)| area).sum();
    let mut lines = vec![format!(
        "PROFILE interface: {} boxes painted, {:.2} screens of them",
        painted.len(),
        area / (screen.x * screen.y)
    )];
    for (_, what) in painted.iter().take(LARGEST) {
        lines.push(format!("PROFILE   {what}"));
    }
    lines
}

/// Prints what the profile measured, once the script quits.
fn report(
    profile: Res<Profile>,
    mut exits: MessageReader<AppExit>,
    world: (
        Query<Entity>,
        Query<&ViewVisibility, With<Mesh3d>>,
        Query<(), With<Node>>,
        Query<&TextLayoutInfo>,
        Res<Assets<Mesh>>,
        Res<Assets<StandardMaterial>>,
    ),
) {
    if exits.read().next().is_none() {
        return;
    }
    let frames = profile.work.len().max(1) as f64;
    let (entities, meshes, nodes, texts, mesh_assets, materials) = world;
    eprintln!(
        "PROFILE {} frames, main schedule ms: {}; from frame to frame ms: {}",
        profile.work.len(),
        spread(&profile.work),
        spread(&profile.periods),
    );
    for told in &profile.slow_frames {
        eprintln!("{told}");
    }
    let mut passes: Vec<(&String, f64)> = profile
        .passes
        .iter()
        .map(|(path, (sum, count))| (path, sum / (*count).max(1) as f64))
        .collect();
    passes.sort_by(|a, b| b.1.total_cmp(&a.1));
    let total: f64 = passes.iter().map(|(_, mean)| mean).sum();
    eprintln!("PROFILE graphics card: {total:.3} ms a frame");
    for (path, mean) in passes {
        eprintln!("PROFILE   {mean:>8.3} ms  {path}");
    }
    let changed = CHANGED
        .iter()
        .zip(profile.changed)
        .map(|(kind, count)| format!("{kind} {:.1}", count as f64 / frames))
        .collect::<Vec<_>>()
        .join(", ");
    eprintln!("PROFILE changed in a frame: {changed}");
    for line in &profile.painted {
        eprintln!("{line}");
    }
    let glyphs: usize = texts.iter().map(|text| text.glyphs.len()).sum();
    eprintln!(
        "PROFILE world: {} entities, {} meshes of which {} in view, {} boxes of the interface, \
         {} texts of {glyphs} glyphs, {} meshes and {} materials in store",
        entities.iter().count(),
        meshes.iter().count(),
        meshes.iter().filter(|visible| visible.get()).count(),
        nodes.iter().count(),
        texts.iter().count(),
        mesh_assets.len(),
        materials.len(),
    );
    let spans = spans();
    let mut rows: Vec<(&str, Duration, u64)> = spans
        .names
        .iter()
        .zip(&spans.totals)
        .filter(|(_, (_, count))| *count > 0)
        .map(|(name, (time, count))| (name.as_str(), *time, *count))
        .collect();
    rows.sort_by_key(|(_, time, _)| Reverse(*time));
    if let Ok(path) = std::env::var("TANKU_PROFILE_CSV") {
        let mut csv = String::from("ms_per_frame,calls_per_frame,us_per_call,name\n");
        for (name, time, count) in &rows {
            let _ = writeln!(
                csv,
                "{:.4},{:.3},{:.3},\"{}\"",
                ms(*time) / frames,
                *count as f64 / frames,
                ms(*time) * 1e3 / *count as f64,
                name.replace('"', "\"\""),
            );
        }
        if let Err(error) = std::fs::write(&path, csv) {
            eprintln!("PROFILE could not write {path}: {error}");
        }
    }
    if rows.is_empty() {
        eprintln!("PROFILE no spans: a build with the feature `profile` has one for every system");
    }
    for (name, time, count) in rows.iter().take(DEAREST) {
        eprintln!(
            "PROFILE {:>9.3} ms a frame {:>9.2} calls a frame {:>9.2} us a call  {name}",
            ms(*time) / frames,
            *count as f64 / frames,
            ms(*time) * 1e3 / *count as f64
        );
    }
}

/// Values at the median, at the 90th and the 99th percentile, and at worst.
fn spread(values: &[f64]) -> String {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let at = |share: f64| {
        let last = sorted.len().saturating_sub(1);
        sorted
            .get((last as f64 * share).round() as usize)
            .copied()
            .unwrap_or_default()
    };
    format!(
        "median {:.2}, 90% {:.2}, 99% {:.2}, worst {:.2}",
        at(0.5),
        at(0.9),
        at(0.99),
        at(1.0)
    )
}

fn ms(time: Duration) -> f64 {
    time.as_secs_f64() * 1e3
}
