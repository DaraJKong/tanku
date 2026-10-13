//! The music. The crate `tanku_audio` computes it while it plays. This module tells it, every
//! frame, what happens in the game, and hands it to what plays it: the sound library of the
//! engine on a desktop, and the browser itself on the web.

#[cfg(any(test, target_arch = "wasm32"))]
mod ahead;
#[cfg(not(target_arch = "wasm32"))]
mod device;
#[cfg(target_arch = "wasm32")]
mod web;

use std::time::Duration;

use bevy::prelude::*;
use tanku_audio::{Conductor, Context, MUSIC_LEVEL, Place};
use tanku_core::{
    map::{SectorKind, SectorPlan},
    sim::{Enemy, Run, RunPhase, Sector, Tank},
};

use crate::{Overlay, Screen, settings::Settings};

/// Whether the music turns itself down when it is ducked. On the web it is computed ahead of
/// time, so it would be late: its loudness is turned down where it is played instead.
const DUCKS_ITSELF: bool = cfg!(not(target_arch = "wasm32"));

/// The end of the music that the game holds, to tell it what happens.
#[derive(Resource, Default)]
pub struct Baton(pub Conductor);

/// The time, on the clock of the machine, until which the music stays half as loud: the
/// warning of a mine sounds.
#[derive(Resource, Default)]
pub struct DuckedUntil(pub Duration);

pub fn plugin(app: &mut App) {
    app.init_resource::<Baton>()
        .init_resource::<DuckedUntil>()
        .add_systems(Update, tell);
    #[cfg(not(target_arch = "wasm32"))]
    app.add_plugins(device::plugin);
    #[cfg(target_arch = "wasm32")]
    app.add_plugins(web::plugin);
}

/// How loud the music is against the cues, under the volume of the game.
fn loudness(settings: &Settings) -> f32 {
    settings.music * MUSIC_LEVEL
}

/// What the music is told about the game: where the run has got to, given the kind of its
/// sector if that sector is loaded, and how smart the enemies of its fight are, if anyone
/// fights: [`fought`] says who does.
fn context(
    screen: Screen,
    overlay: Overlay,
    run: Option<&Run>,
    kind: Option<SectorKind>,
    smartest: Option<u8>,
    ducked: bool,
) -> Context {
    // The main menu and the game over screen. A run that is over is no place either: its
    // last moments are still on the screen, and so is the start of the run that follows it.
    let on = |run: &&Run| screen == Screen::Playing && run.phase != RunPhase::Over;
    let Some(run) = run.filter(on) else {
        return Context::default();
    };
    Context {
        place: match kind {
            Some(SectorKind::Bay) => Place::Bay,
            Some(SectorKind::Corridor) => Place::Corridor,
            // The room of arena mode is an arena that a run never leaves.
            Some(SectorKind::Arena | SectorKind::Room) => Place::Arena,
            // The sector isn't loaded yet.
            None if run.sector == 0 => Place::Bay,
            None => Place::Corridor,
        },
        // A sentry has no brain to speak of, and is a fight all the same.
        fight: smartest.is_some(),
        threat: smartest.unwrap_or(0),
        down: run.phase != RunPhase::Playing,
        // The settings leave the music clear, so that its volume is set by ear.
        muffled: overlay == Overlay::Pause,
        ducked,
    }
}

/// The fight that the music is told of: how smart its enemies are, if anyone fights.
///
/// In a corridor, a fight is on while an enemy is awake. An arena sends its group in waves,
/// and the room of arena mode a tank for each one that is destroyed: between two of them,
/// nobody is awake, and the fight is far from over. There, a fight that has started goes on
/// until its sector is cleared, or its group, and the music is told meanwhile of the threat
/// that it was last told of. `held` keeps that fight: where it is fought, and its threat.
fn fought(
    held: &mut Option<(u32, u8)>,
    run: Option<&Run>,
    kind: Option<SectorKind>,
    smartest: Option<u8>,
) -> Option<u8> {
    let in_waves = matches!(kind, Some(SectorKind::Arena | SectorKind::Room));
    // The run says when a sector is cleared, and a group of arena mode: its last tank is
    // destroyed, and none is left to come.
    let Some(run) = run.filter(|run| in_waves && run.cleared < run.sector) else {
        *held = None;
        return smartest;
    };
    *held = match smartest {
        Some(threat) => Some((run.sector, threat)),
        // Nothing has started in a sector that was only just entered.
        None => held.filter(|(sector, _)| *sector == run.sector),
    };
    held.map(|(_, threat)| threat)
}

/// Tells the music what happens in the game.
pub fn tell(
    baton: Res<Baton>,
    screen: Res<State<Screen>>,
    overlay: Res<State<Overlay>>,
    run: Option<Res<Run>>,
    sectors: Query<&Sector>,
    enemies: Query<(&Enemy, &Tank)>,
    time: Res<Time<Real>>,
    ducked: Res<DuckedUntil>,
    mut held: Local<Option<(u32, u8)>>,
) {
    let run = run.as_deref();
    let reached = run.map_or(0, |run| run.sector);
    let mut plans = sectors.iter().map(|sector| &sector.layout.plan);
    // The room of arena mode is where the run is, whichever group it has reached.
    let here = |plan: &&SectorPlan| plan.index == reached || plan.kind == SectorKind::Room;
    let kind = plans.find(here).map(|plan| plan.kind);
    let awake = enemies.iter().filter(|(enemy, _)| enemy.awake);
    let smartest = awake.map(|(_, tank)| tank.genome.brain()).max();
    let smartest = fought(&mut held, run, kind, smartest);
    // A warning sounds whether or not the game is paused, so its time is that of the clock.
    let ducked = DUCKS_ITSELF && time.elapsed() < ducked.0;
    baton.0.set(context(
        *screen.get(),
        *overlay.get(),
        run,
        kind,
        smartest,
        ducked,
    ));
}

#[cfg(test)]
mod tests {
    use tanku_audio::{CHANNELS, SAMPLE_RATE, cues::Cue};
    use tanku_core::{
        enemy,
        map::Corridor,
        progression, seed,
        sim::Team,
        stats::{Genome, Stat},
    };

    use super::*;

    fn run(sector: u32, phase: RunPhase) -> Run {
        Run {
            seed_text: "TEST".into(),
            seed: 1,
            difficulty: default(),
            players: 1,
            team: 1,
            solo: true,
            sector,
            cleared: 0,
            checkpoint: 0,
            lives: 3,
            kills: 0,
            ticks: 0,
            phase,
            lost_to: None,
            playtest: false,
            earns: true,
            free: false,
            arena: false,
            lull: None,
            paused: false,
        }
    }

    #[test]
    fn the_music_is_twelve_decibels_under_a_shot() {
        // The loudest arrangement, for the verse and the chorus of each mode.
        let mut peak = 0.0f32;
        for (place, threat) in [(Place::Arena, 1), (Place::Arena, 4), (Place::Corridor, 4)] {
            let (mut music, _) = tanku_audio::music_from(Context {
                place,
                fight: true,
                threat,
                ..default()
            });
            let mut frames = vec![0.0; 32 * SAMPLE_RATE as usize * CHANNELS as usize];
            music.render(&mut frames);
            peak = frames
                .iter()
                .fold(peak, |peak, sample| peak.max(sample.abs()));
        }
        let shot = Cue::Shot.samples().into_iter().map(f32::abs);
        let shot = shot.fold(0.0, f32::max);
        let settings = Settings::default();
        // The volume of the game sets both, so it doesn't count.
        let under = 20.0 * (shot / (peak * loudness(&settings))).log10();
        assert!((11.5..12.5).contains(&under), "{under} dB");
    }

    #[test]
    fn the_music_is_told_where_the_run_is() {
        let told =
            |screen, run: &Run, kind| context(screen, Overlay::None, Some(run), kind, None, false);
        let playing = run(3, RunPhase::Playing);
        // No run, or a screen that isn't the game: the menu.
        let menu = context(Screen::Menu, Overlay::None, None, None, None, false);
        assert_eq!(menu, Context::default());
        assert_eq!(menu.place, Place::Menu);
        assert_eq!(told(Screen::Menu, &playing, Some(SectorKind::Arena)), menu);
        assert_eq!(told(Screen::GameOver, &playing, None), menu);

        let place = |kind| told(Screen::Playing, &playing, kind).place;
        assert_eq!(place(Some(SectorKind::Bay)), Place::Bay);
        assert_eq!(place(Some(SectorKind::Corridor)), Place::Corridor);
        assert_eq!(place(Some(SectorKind::Arena)), Place::Arena);
        assert_eq!(place(Some(SectorKind::Room)), Place::Arena);
        // Until the sector is loaded, the number of the sector tells the bay from the rest.
        assert_eq!(place(None), Place::Corridor);
        let bay = run(0, RunPhase::Playing);
        assert_eq!(told(Screen::Playing, &bay, None).place, Place::Bay);
    }

    #[test]
    fn the_music_is_told_of_fights_and_of_losses() {
        let corridor = Some(SectorKind::Corridor);
        let told = |overlay, run: &Run, smartest| {
            context(
                Screen::Playing,
                overlay,
                Some(run),
                corridor,
                smartest,
                false,
            )
        };
        let playing = run(3, RunPhase::Playing);
        let calm = told(Overlay::None, &playing, None);
        assert!(!calm.fight && !calm.down && !calm.muffled);
        assert_eq!(calm.threat, 0);
        // The smartest enemy that is awake sets the threat.
        let fight = told(Overlay::None, &playing, Some(3));
        assert!(fight.fight && !fight.down);
        assert_eq!(fight.threat, 3);
        // Sentries are a fight too, at the lowest threat.
        let sentries = told(Overlay::None, &playing, Some(0));
        assert!(sentries.fight);
        assert_eq!(sentries.threat, 0);

        // A tank that is on its way back is down, and the enemies are still awake.
        let back_soon = told(Overlay::None, &run(3, RunPhase::Respawning(1.0)), Some(2));
        assert!(back_soon.down && back_soon.fight);
        assert_eq!(back_soon.place, Place::Corridor);
        // A run that is over has left its place, though the game is still on the screen.
        let over = told(Overlay::None, &run(3, RunPhase::Over), Some(2));
        assert_eq!(over, Context::default());

        // The pause menu muffles the music, and the settings don't.
        assert!(told(Overlay::Pause, &playing, Some(2)).muffled);
        assert!(!told(Overlay::Settings, &playing, Some(2)).muffled);
        let settings_of_the_menu =
            context(Screen::Menu, Overlay::Settings, None, None, None, false);
        assert!(!settings_of_the_menu.muffled);

        // The warning of a mine ducks the music of a run, under a menu too.
        let warned = |overlay| {
            let playing = Some(&playing);
            context(Screen::Playing, overlay, playing, corridor, Some(2), true)
        };
        assert!(!fight.ducked && warned(Overlay::None).ducked);
        assert!(warned(Overlay::Pause).ducked && warned(Overlay::Pause).muffled);
    }

    /// An enemy with a brain of some tier.
    fn foe(brain: u8, sector: u32, awake: bool) -> impl Bundle {
        let mut genome = Genome::empty(60);
        genome.set(Stat::Brain, brain);
        let tank = Tank {
            genome,
            team: Team::Enemies,
            sector,
        };
        (tank, Enemy { awake })
    }

    #[test]
    fn the_game_tells_the_music_every_frame() {
        let mut app = App::new();
        app.init_resource::<Baton>()
            .init_resource::<DuckedUntil>()
            .init_resource::<Time<Real>>()
            .insert_resource(State::new(Screen::Playing))
            .insert_resource(State::new(Overlay::None))
            .add_systems(Update, tell);
        let baton = app.world().resource::<Baton>().0.clone();
        app.update();
        // The screen changes a moment before the run is there.
        assert_eq!(baton.context().place, Place::Menu);

        // The bay, the first sector and the first arena of a seed are loaded.
        let mut corridor = Corridor::new(1);
        let arena = (1..).find(|&index| corridor.plan(index).kind == SectorKind::Arena);
        let arena = arena.unwrap();
        for index in [0, 1, arena] {
            let layout = corridor.layout(index);
            app.world_mut().spawn(Sector { layout });
        }
        // A grunt and a veteran wait in the first sector, and an ace in the arena.
        let grunt = app.world_mut().spawn(foe(1, 1, false)).id();
        let veteran = app.world_mut().spawn(foe(3, 1, false)).id();
        app.world_mut().spawn(foe(4, arena, false));
        app.insert_resource(run(0, RunPhase::Playing));
        app.update();
        assert_eq!(baton.context().place, Place::Bay);
        assert!(!baton.context().fight);

        // The player crosses into the first sector, whose enemies wake up. The smartest of
        // them sets the threat, and the ace doesn't count: it sleeps.
        let told = |app: &App| {
            let context = app.world().resource::<Baton>().0.context();
            (context.place, context.fight, context.threat)
        };
        app.world_mut().resource_mut::<Run>().sector = 1;
        for entity in [grunt, veteran] {
            let awake = Enemy { awake: true };
            app.world_mut().entity_mut(entity).insert(awake);
        }
        app.update();
        assert_eq!(told(&app), (Place::Corridor, true, 3));
        // The veteran is destroyed first: the threat falls to the grunt's.
        app.world_mut().entity_mut(veteran).despawn();
        app.update();
        assert_eq!(told(&app), (Place::Corridor, true, 1));
        // Then the grunt: the fight is over.
        app.world_mut().entity_mut(grunt).despawn();
        app.update();
        assert_eq!(told(&app), (Place::Corridor, false, 0));

        // The players are in the arena before its first wave: nobody fights yet.
        let mut state = app.world_mut().resource_mut::<Run>();
        (state.sector, state.cleared) = (arena, arena - 1);
        app.update();
        assert_eq!(told(&app), (Place::Arena, false, 0));
        // The first wave is there, and then destroyed. Until the next one appears, nobody is
        // awake, and the fight goes on at the threat that it had.
        let ace = app.world_mut().spawn(foe(4, arena, true)).id();
        app.update();
        assert_eq!(told(&app), (Place::Arena, true, 4));
        app.world_mut().entity_mut(ace).despawn();
        app.update();
        assert_eq!(told(&app), (Place::Arena, true, 4));
        // The next wave sets the threat, and leaves it when it is destroyed in its turn.
        let grunt = app.world_mut().spawn(foe(1, arena, true)).id();
        app.update();
        assert_eq!(told(&app), (Place::Arena, true, 1));
        app.world_mut().entity_mut(grunt).despawn();
        app.update();
        assert_eq!(told(&app), (Place::Arena, true, 1));
        // The last tank of the last wave was that one: the arena is cleared.
        app.world_mut().resource_mut::<Run>().cleared = arena;
        app.update();
        assert_eq!(told(&app), (Place::Arena, false, 0));
    }

    #[test]
    fn the_fight_of_an_arena_lasts_until_the_arena_is_cleared() {
        let arena = Some(SectorKind::Arena);
        let in_sector = |sector: u32, cleared: u32| Run {
            cleared,
            ..run(sector, RunPhase::Playing)
        };
        let fighting = in_sector(9, 8);
        let mut held = None;
        // Nobody is awake in an arena that was just entered: no fight has started.
        assert_eq!(fought(&mut held, Some(&fighting), arena, None), None);
        // A wave is there, then none is: the fight goes on, at the threat that it had.
        assert_eq!(fought(&mut held, Some(&fighting), arena, Some(3)), Some(3));
        assert_eq!(fought(&mut held, Some(&fighting), arena, None), Some(3));
        assert_eq!(fought(&mut held, Some(&fighting), arena, Some(0)), Some(0));
        assert_eq!(fought(&mut held, Some(&fighting), arena, None), Some(0));
        // The arena is cleared: the fight is over, and nothing is left of it.
        assert_eq!(fought(&mut held, Some(&in_sector(9, 9)), arena, None), None);
        assert_eq!(held, None);

        // A corridor has no waves: a fight there is on while an enemy is awake, and no longer.
        let corridor = Some(SectorKind::Corridor);
        assert_eq!(
            fought(&mut held, Some(&fighting), corridor, Some(2)),
            Some(2)
        );
        assert_eq!(fought(&mut held, Some(&fighting), corridor, None), None);

        // A single player who falls in an arena comes back in front of its barrier, in the
        // sector before it: the fight that was on has stopped, and starts again with the wave
        // that is sent again.
        assert_eq!(fought(&mut held, Some(&fighting), arena, Some(4)), Some(4));
        assert_eq!(
            fought(&mut held, Some(&in_sector(8, 8)), corridor, None),
            None
        );
        assert_eq!(fought(&mut held, Some(&fighting), arena, None), None);
        // A fight of one arena says nothing of the next one.
        assert_eq!(fought(&mut held, Some(&fighting), arena, Some(4)), Some(4));
        assert_eq!(
            fought(&mut held, Some(&in_sector(16, 15)), arena, None),
            None
        );

        // The room of arena mode is fought the same way, group by group: its sector is the
        // number of the group that is on, and a lull comes once that group is cleared.
        let room = Some(SectorKind::Room);
        let group = in_sector(4, 3);
        assert_eq!(fought(&mut held, Some(&group), room, None), None);
        assert_eq!(fought(&mut held, Some(&group), room, Some(1)), Some(1));
        assert_eq!(fought(&mut held, Some(&group), room, None), Some(1));
        assert_eq!(fought(&mut held, Some(&in_sector(4, 4)), room, None), None);
        // Without a run, nobody fights.
        assert_eq!(fought(&mut held, None, room, None), None);
    }

    #[test]
    fn a_run_in_arena_mode_sounds_like_an_arena_whichever_group_it_is_at() {
        let mut app = App::new();
        app.init_resource::<Baton>()
            .init_resource::<DuckedUntil>()
            .init_resource::<Time<Real>>()
            .insert_resource(State::new(Screen::Playing))
            .insert_resource(State::new(Overlay::None))
            .add_systems(Update, tell);
        let layout = tanku_core::map::Room::new(1).shell();
        app.world_mut().spawn(Sector { layout });
        let told = |app: &App| {
            let context = app.world().resource::<Baton>().0.context();
            (context.place, context.fight, context.threat)
        };
        // The lull before the first group, then a group that is on, then the lull after it.
        app.insert_resource(Run {
            arena: true,
            lull: Some(3),
            paused: false,
            ..run(0, RunPhase::Playing)
        });
        app.update();
        assert_eq!(told(&app), (Place::Arena, false, 0));
        let mut state = app.world_mut().resource_mut::<Run>();
        (state.sector, state.lull) = (7, None);
        let soldier = app.world_mut().spawn(foe(2, 7, true)).id();
        app.update();
        assert_eq!(told(&app), (Place::Arena, true, 2));
        // The soldier is destroyed, and the tank that comes for it is still on its way: the
        // room is empty for a moment, and the fight goes on.
        app.world_mut().entity_mut(soldier).despawn();
        app.update();
        assert_eq!(told(&app), (Place::Arena, true, 2));
        let grunt = app.world_mut().spawn(foe(1, 7, true)).id();
        app.update();
        assert_eq!(told(&app), (Place::Arena, true, 1));
        // The last tank of the group is destroyed: the group is cleared, and the lull is calm.
        app.world_mut().entity_mut(grunt).despawn();
        let mut state = app.world_mut().resource_mut::<Run>();
        (state.cleared, state.lull) = (7, Some(3));
        app.update();
        assert_eq!(told(&app), (Place::Arena, false, 0));
    }

    /// The threat that each of the first sectors of a seed starts its fight with, to know
    /// what to listen for there:
    /// `TANKU_SEED=TANKU cargo test -p tanku_client threats_of_a_seed -- --ignored --nocapture`
    #[test]
    #[ignore = "prints a list"]
    fn the_threats_of_a_seed() {
        const LEADS: [&str; 5] = [
            "silent",
            "the skeleton of the motif",
            "the motif",
            "the motif with ornaments",
            "the motif with ornaments, in octaves",
        ];
        let text = std::env::var("TANKU_SEED").unwrap_or("TANKU".into());
        let seed = seed::seed_value(&text);
        let mut corridor = Corridor::new(seed);
        println!("Seed {}", text.trim().to_uppercase());
        println!("Sector  Place     Brains              Threat  Lead");
        for sector in 1..=30 {
            let arena = corridor.plan(sector).kind == SectorKind::Arena;
            let count = progression::enemy_count(sector, 1, arena);
            let group = enemy::group(seed, default(), sector, count);
            let brains: Vec<u8> = group.iter().map(|tank| tank.brain()).collect();
            let threat = brains.iter().copied().max().unwrap_or(0);
            let brains: Vec<String> = brains.iter().map(u8::to_string).collect();
            println!(
                "{sector:<6}  {:<8}  {:<18}  {threat:<6}  {}",
                if arena { "arena" } else { "corridor" },
                brains.join(" "),
                LEADS[threat as usize],
            );
        }
    }
}
