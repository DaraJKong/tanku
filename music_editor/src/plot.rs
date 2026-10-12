//! What an instrument sounds like, as a drawing: the first bar that it plays alone, computed
//! by the synth of the game, and the first milliseconds of its first note.
//!
//! Nothing here is a sketch of a wave: the drawing is the samples that the game would play,
//! so it moves with every number of the instrument.

use tanku_audio::{
    Context, Place, SAMPLE_RATE, checks, music_of,
    notation::{Instrument, Piece, Scene},
};

use crate::audition::{self, Audition};

/// The size of the drawing, in pixels.
pub const WIDTH: usize = 1022;
pub const HEIGHT: usize = 200;
/// Where its two halves are: the wave at the left, and the bar at the right.
pub const WAVE: usize = 40;
pub const BAR: usize = 560;
pub const SPAN: usize = 440;
/// The line in the middle of each, and how far a sample goes above and below it.
const MIDDLE: usize = 108;
const REACH: f32 = 62.0;
/// Milliseconds of the first note that the wave shows.
pub const WAVE_MS: usize = 20;

/// A sample under this, against the loudest one, is silence still.
const ONSET: f32 = 0.02;
const SILENT: f32 = 1e-4;

/// The colors of the drawing: its lines, and the samples.
const AXIS: [u8; 4] = [0x2E, 0x3B, 0x4C, 0xFF];
const RULE: [u8; 4] = [0x5E, 0x73, 0x91, 0xFF];
const INK: [u8; 4] = [0xC9, 0xD8, 0xEC, 0xFF];

/// What an instrument was heard as: its first bar alone, and the scene that played it.
#[derive(Clone, Debug, PartialEq)]
pub struct Heard {
    /// One sample per frame: the middle of the left and of the right.
    pub samples: Vec<f32>,
    pub context: Context,
    pub scene: Scene,
}

/// The first bar that an instrument plays alone in a context, if it plays there.
fn listen(
    lines: &[String],
    piece: &Piece,
    instrument: Instrument,
    context: Context,
) -> Option<Heard> {
    let score = audition::score(lines, piece, &Audition::Solo(instrument)).ok()?;
    let (mut music, _) = music_of(score.clone(), context);
    let spot = music.spot();
    let frames = checks::bar(score.piece.section(spot.section));
    let played = checks::play(&mut music, frames);
    let samples: Vec<f32> = played
        .chunks(2)
        .map(|frame| (frame[0] + frame[1]) / 2.0)
        .collect();
    let peak = samples
        .iter()
        .fold(0.0f32, |peak, sample| peak.max(sample.abs()));
    (peak > SILENT).then_some(Heard {
        samples,
        context,
        scene: spot.scene,
    })
}

/// The first bar that an instrument plays alone: where the transport is if it plays there, or
/// in the first of the fullest fight, the calm and the menu where it does. An instrument that
/// plays nowhere has none.
pub fn heard(
    lines: &[String],
    piece: &Piece,
    instrument: Instrument,
    wanted: Context,
) -> Option<Heard> {
    let contexts = [
        wanted,
        checks::fight(Place::Arena, 4),
        checks::fight(Place::Corridor, 4),
        checks::at(Place::Corridor),
        checks::at(Place::Bay),
    ];
    contexts
        .into_iter()
        .find_map(|context| listen(lines, piece, instrument, context))
}

/// The scene of what was heard, in a few words.
pub fn told(heard: &Heard) -> String {
    match heard.scene {
        Scene::Menu => "the menu".to_string(),
        Scene::Calm => "the calm".to_string(),
        Scene::Corridor => format!("a fight in a corridor at threat {}", heard.context.threat),
        Scene::Arena => format!("a fight in an arena at threat {}", heard.context.threat),
    }
}

/// The pixels of a drawing: four bytes each, in rows from the top.
struct Sheet(Vec<u8>);

impl Sheet {
    fn dot(&mut self, x: usize, y: usize, color: [u8; 4]) {
        if x < WIDTH && y < HEIGHT {
            let at = 4 * (y * WIDTH + x);
            self.0[at..at + 4].copy_from_slice(&color);
        }
    }

    fn across(&mut self, from: usize, to: usize, y: usize, color: [u8; 4]) {
        for x in from..to {
            self.dot(x, y, color);
        }
    }

    fn down(&mut self, x: usize, from: usize, to: usize, color: [u8; 4]) {
        for y in from..=to {
            self.dot(x, y, color);
        }
    }

    /// Samples over a span of the sheet: each column goes from the lowest to the highest of
    /// the samples that fall on it, and reaches the next one, so that the line never breaks.
    fn wave(&mut self, samples: &[f32], left: usize, peak: f32) {
        if samples.is_empty() {
            return;
        }
        let height = |sample: f32| {
            let above = (sample / peak).clamp(-1.0, 1.0) * REACH;
            (MIDDLE as f32 - above).round() as usize
        };
        for column in 0..SPAN {
            let from = column * samples.len() / SPAN;
            let to = ((column + 1) * samples.len() / SPAN).clamp(from + 1, samples.len());
            // The first sample of the next column too.
            let end = (to + 1).min(samples.len());
            let column_of = &samples[from.min(samples.len() - 1)..end];
            let (low, high) = column_of
                .iter()
                .fold((f32::MAX, f32::MIN), |(low, high), &sample| {
                    (low.min(sample), high.max(sample))
                });
            self.down(left + column, height(high), height(low), INK);
        }
    }
}

/// Where the first note of some samples starts.
fn onset(samples: &[f32], peak: f32) -> usize {
    let starts = |sample: &f32| sample.abs() > ONSET * peak;
    samples.iter().position(starts).unwrap_or(0)
}

/// The drawing of what was heard, as the bytes of an image of `WIDTH` by `HEIGHT`: the
/// first milliseconds of the first note at the left, and the whole bar at the right.
pub fn picture(heard: Option<&Heard>) -> Vec<u8> {
    let mut sheet = Sheet(vec![0; 4 * WIDTH * HEIGHT]);
    sheet.down(WIDTH / 2, 0, HEIGHT - 1, RULE);
    for left in [WAVE, BAR] {
        sheet.across(left, left + SPAN, MIDDLE, AXIS);
    }
    // The four beats of the bar.
    for beat in 0..=4 {
        let x = BAR + beat * SPAN / 4;
        sheet.down(
            x.min(BAR + SPAN - 1),
            MIDDLE + REACH as usize + 6,
            MIDDLE + REACH as usize + 12,
            RULE,
        );
    }
    let Some(heard) = heard else {
        return sheet.0;
    };
    let samples = &heard.samples;
    let peak = samples
        .iter()
        .fold(SILENT, |peak, sample| peak.max(sample.abs()));
    let from = onset(samples, peak);
    let frames = (WAVE_MS * SAMPLE_RATE as usize / 1000).min(samples.len() - from);
    let first = &samples[from..from + frames];
    // The wave is as high as its own loudest sample: a note that starts softly is still read.
    let first_peak = first
        .iter()
        .fold(SILENT, |peak, sample| peak.max(sample.abs()));
    sheet.wave(first, WAVE, first_peak);
    sheet.wave(samples, BAR, peak);
    sheet.0
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::fixture::PIECE;

    const FILE: &str = crate::fixture::FILE;

    fn lines() -> Vec<String> {
        FILE.lines().map(str::to_string).collect()
    }

    #[test]
    fn an_instrument_is_heard_where_it_plays() {
        let lines = lines();
        // The lead plays in a fight at threat 2: one bar of the verse at 120 beats per minute.
        let fight = checks::fight(Place::Corridor, 2);
        let lead = heard(&lines, &PIECE, Instrument::Lead, fight).unwrap();
        assert_eq!((lead.context, lead.scene), (fight, Scene::Corridor));
        assert_eq!(lead.samples.len(), 2 * SAMPLE_RATE as usize);
        assert_eq!(told(&lead), "a fight in a corridor at threat 2");
        // The snare plays in an arena only, and the kick has its heart in the calm.
        let snare = heard(&lines, &PIECE, Instrument::Snare, fight).unwrap();
        assert_eq!(snare.scene, Scene::Arena);
        let calm = checks::at(Place::Corridor);
        let kick = heard(&lines, &PIECE, Instrument::Kick, calm).unwrap();
        assert_eq!(
            (kick.scene, told(&kick).as_str()),
            (Scene::Calm, "the calm")
        );
        // The bar of the calm is that of the bridge, which is slower.
        assert!(kick.samples.len() > lead.samples.len());
        // An instrument that the mix silences is heard nowhere.
        let silent: Vec<String> = lines
            .iter()
            .map(|line| line.replace("mix hat   75% 45%", "mix hat    0%  0%"))
            .collect();
        assert_ne!(silent, lines);
        assert_eq!(heard(&silent, &PIECE, Instrument::Hat, fight), None);
    }

    #[test]
    fn the_drawing_shows_the_first_note_and_the_bar() {
        let lines = lines();
        let fight = checks::fight(Place::Corridor, 2);
        let kick = heard(&lines, &PIECE, Instrument::Kick, fight).unwrap();
        let drawn = picture(Some(&kick));
        assert_eq!(drawn.len(), 4 * WIDTH * HEIGHT);
        let lit = |x: usize| {
            (0..HEIGHT)
                .filter(|y| drawn[4 * (y * WIDTH + x)..][..4] == INK)
                .count()
        };
        // Every column of the wave has its line, and the kick is on every beat of the bar:
        // high at the start of each, and down to the middle line before the next.
        assert!((WAVE..WAVE + SPAN).all(|x| lit(x) >= 1));
        for beat in 0..4 {
            let start = BAR + beat * SPAN / 4;
            assert!(lit(start + 2) > 40, "beat {beat}: {}", lit(start + 2));
            assert!(lit(start + SPAN / 4 - 4) <= 2, "beat {beat}");
        }
        // Nothing is drawn of an instrument that isn't heard, but the lines of the drawing.
        let empty = picture(None);
        assert!(empty.chunks(4).all(|pixel| pixel != INK));
        assert!(empty.chunks(4).any(|pixel| pixel == AXIS));
    }
}
