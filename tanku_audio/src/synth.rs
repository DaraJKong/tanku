//! The six instruments, the echo and the mix.
//!
//! Everything here adds, subtracts, multiplies and divides 32-bit floats, which gives the same
//! result on every platform. Nothing calls a library function such as `sin` or `exp`, whose
//! last bit differs from one platform to the next: sines and decays come from the few lines
//! below.
//!
//! A sound is also a function of what was played and of nothing else. A note of a given pitch
//! is the same every time and so is every hit of a drum, which makes a loop of the piece
//! repeat exactly.

use std::f32::consts::TAU;

use crate::{
    SAMPLE_RATE,
    director::{Hush, Step, Tone},
};

const RATE: f32 = SAMPLE_RATE as f32;

/// A level below this is silence.
const SILENT: f32 = 1e-4;

/// Frames a note takes to fall to nothing before the next one can start: 5 ms.
const GAP: u32 = 220;
/// Frames a note takes to fall when another one waits for its place: 3 ms.
const CUT: u32 = 132;

/// Frames the bass takes to rise: 3 ms.
const BASS_RISE: u32 = 132;
/// The filter of the bass lets through what is below a frequency that starts at the sum of
/// these two and comes down to the first, in Hz.
const BASS_FLOOR: f32 = 300.0;
const BASS_SWEEP: f32 = 1100.0;
/// The filter is closed 0.15 s after the attack.
const BASS_CLOSE: f32 = fall(0.05);

/// Plucked strings that can ring at once.
const PLUCKS: usize = 4;
/// Longest string, in frames. The lowest note of the pluck, A3, needs 199.
const STRING: usize = 256;
/// Seconds a string takes to fall by 60 dB.
const RING: f32 = 1.0;
/// What a string that is stopped keeps at each turn.
const STOPPED: f32 = 0.9;
const PLUCK_SEED: u32 = 0x5EED_0001;
/// Times the burst of noise that plucks a string is smoothed.
const SMOOTHING: u32 = 3;
/// The echo of the pluck's arpeggio comes three steps later and a third as loud.
const ECHO_FRAMES: usize = 16_537;
const ECHO_LEVEL: f32 = 1.0 / 3.0;

/// Notes the lead can play at once: its line, and the same an octave higher.
const LEADS: usize = 2;
/// The part of its period that the pulse of the lead spends up.
const LEAD_WIDTH: f32 = 0.3;
/// Frames the lead takes to rise: 4 ms.
const LEAD_RISE: u32 = 176;
/// The lead lets through what is below this frequency, in Hz.
const LEAD_POLE: f32 = pole(3000.0);
/// The vibrato comes in after this many frames, and is whole after as many again.
const VIBRATO_AFTER: f32 = 6000.0;
/// How far the vibrato bends the pitch, and how many times per second.
const VIBRATO_DEPTH: f32 = 0.004;
const VIBRATO_RATE: f32 = 5.5;

/// The kick falls from the sum of these two to the first, in Hz, in 0.12 s.
const KICK_FLOOR: f32 = 55.0;
const KICK_DROP: f32 = 55.0;
const KICK_SWEEP: f32 = fall(0.03);
const KICK_FALL: f32 = fall(0.045);

/// The hi-hat is noise above 7 kHz that lasts 0.03 s.
const HAT_POLE: f32 = pole(7000.0);
const HAT_FALL: f32 = fall(0.008);
const HAT_SEED: u32 = 0x5EED_0002;

/// The snare is noise around 2 kHz with a tone at 220 Hz, and lasts 0.12 s.
const SNARE_TUNE: f32 = 2.0 * sine(2000.0 / RATE / 2.0);
const SNARE_DAMP: f32 = 0.8;
const SNARE_FALL: f32 = fall(0.035);
const SNARE_TONE: f32 = 220.0;
const SNARE_TONE_FALL: f32 = fall(0.03);
const SNARE_TONE_LEVEL: f32 = 0.7;
const SNARE_SEED: u32 = 0x5EED_0003;

/// How loud each instrument is on the left and on the right.
const BASS_MIX: [f32; 2] = [0.10, 0.10];
const PLUCK_MIX: [f32; 2] = [0.20, 0.10];
const LEAD_MIX: [f32; 2] = [0.05, 0.08];
const KICK_MIX: [f32; 2] = [0.32, 0.32];
const HAT_MIX: [f32; 2] = [0.75, 0.45];
const SNARE_MIX: [f32; 2] = [0.28, 0.28];

/// A muffled music loses what is above this frequency, in Hz, and half its loudness.
const MUFFLE_POLE: f32 = pole(600.0);
const MUFFLE_LEVEL: f32 = 0.5;
/// It takes 0.2 s to get there and as long to come back.
const MUFFLE_RATE: f32 = 1.0 / (0.2 * RATE);

/// The mix is left alone below this level and bent above it, so that it never clips.
const KNEE: f32 = 0.6;

/// The sine of an angle given in turns, between 0 and 1.
const fn sine(turns: f32) -> f32 {
    // Folded onto a quarter turn either side of zero, where a short series is exact to a few
    // millionths.
    let folded = if turns < 0.25 {
        turns
    } else if turns < 0.75 {
        0.5 - turns
    } else {
        turns - 1.0
    };
    let x = folded * TAU;
    let x2 = x * x;
    x * (1.0 - x2 / 6.0 * (1.0 - x2 / 20.0 * (1.0 - x2 / 42.0 * (1.0 - x2 / 72.0))))
}

/// The frequency of a pitch in Hz. Pitches count semitones, and 69 is the A of 440 Hz.
fn frequency(pitch: u8) -> f32 {
    // The 12 semitones from that A up. The other octaves are these doubled or halved, which
    // is exact.
    const OCTAVE: [f32; 12] = [
        440.0, 466.163_76, 493.883_3, 523.251_16, 554.365_23, 587.329_5, 622.253_97, 659.255_1,
        698.456_5, 739.988_83, 783.990_84, 830.609_4,
    ];
    let above = pitch as i32 - 69;
    let octave = above.div_euclid(12);
    let doubled = (1u32 << octave.unsigned_abs()) as f32;
    let hz = OCTAVE[above.rem_euclid(12) as usize];
    if octave < 0 {
        hz / doubled
    } else {
        hz * doubled
    }
}

/// What a level keeps from one frame to the next when it loses about two thirds of itself in
/// this many seconds.
const fn fall(seconds: f32) -> f32 {
    1.0 - 1.0 / (seconds * RATE)
}

/// The weight of a filter of one pole that lets through what is below a frequency in Hz.
const fn pole(hz: f32) -> f32 {
    let turn = TAU * hz / RATE;
    turn / (1.0 + turn)
}

/// A level that the director gives, from 0 to 255, as a loudness from 0 to 1.
fn loudness(level: u8) -> f32 {
    level as f32 / 255.0
}

/// A phase that went past the end of its period, brought back into it.
fn wrap(phase: f32) -> f32 {
    if phase >= 1.0 { phase - 1.0 } else { phase }
}

/// Rounds off the jump of a square or pulse wave, which would otherwise ring with tones that
/// are not in the note. `phase` is how far past the jump the wave is, in periods, and `step`
/// is how far the wave goes per frame.
fn blep(phase: f32, step: f32) -> f32 {
    if phase < step {
        let t = phase / step;
        t + t - t * t - 1.0
    } else if phase > 1.0 - step {
        let t = (phase - 1.0) / step;
        t * t + t + t + 1.0
    } else {
        0.0
    }
}

/// Keeps a level in range without ever clipping it: what is above the knee is bent toward 1,
/// which it never reaches.
fn limit(level: f32) -> f32 {
    let size = level.abs();
    if size <= KNEE {
        return level;
    }
    let over = (size - KNEE) / (1.0 - KNEE);
    (KNEE + (1.0 - KNEE) * over / (1.0 + over)).copysign(level)
}

/// White noise from a generator of its own.
struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / (1 << 23) as f32 - 1.0
    }
}

/// The loudness of a note over its life. It rises when the note starts, stays while the note
/// lasts and falls to nothing just before the next one can start, so that two notes in a row
/// never overlap and none starts or ends with a click.
#[derive(Default)]
struct Envelope {
    /// How far the note has risen, from 0 to 1, and how much it rises per frame.
    risen: f32,
    rise: f32,
    /// Frames before the note lets go.
    left: u32,
    /// Frames the fall takes, and how many of them are left.
    fall: u32,
    falling: u32,
    sounding: bool,
}

impl Envelope {
    /// Starts a note that lasts a number of frames, the fall included.
    fn start(&mut self, frames: u32, rise: u32, fall: u32) {
        *self = Self {
            risen: 0.0,
            rise: 1.0 / rise as f32,
            left: frames.saturating_sub(fall).max(1),
            fall,
            falling: fall,
            sounding: true,
        };
    }

    /// Lets go now.
    fn release(&mut self) {
        self.left = 0;
    }

    /// Lets go now and falls fast, to make room for a note that waits.
    fn cut(&mut self) {
        self.left = 0;
        if self.falling > CUT {
            self.risen *= self.falling as f32 / self.fall as f32;
            self.fall = CUT;
            self.falling = CUT;
        }
    }

    /// The level of the next frame. A note that is held doesn't let go.
    fn next(&mut self, held: bool) -> f32 {
        if !self.sounding {
            return 0.0;
        }
        if self.left > 0 {
            if !held {
                self.left -= 1;
            }
            self.risen = (self.risen + self.rise).min(1.0);
            return self.risen;
        }
        self.falling -= 1;
        self.sounding = self.falling > 0;
        self.risen * (self.falling as f32 / self.fall as f32)
    }
}

/// A note of the bass or of the lead.
#[derive(Clone, Copy, Default)]
struct Note {
    /// How far its wave goes per frame, in periods.
    step: f32,
    /// Frames it lasts, its fall included.
    frames: u32,
    loud: f32,
}

impl Note {
    fn new(tone: Tone, frames: u32) -> Self {
        Self {
            step: frequency(tone.pitch) / RATE,
            frames,
            loud: loudness(tone.level),
        }
    }
}

/// A square wave through a filter that opens at each attack and closes again.
#[derive(Default)]
struct Bass {
    envelope: Envelope,
    note: Note,
    /// Where the wave is in its period.
    phase: f32,
    /// How far the filter is open, from 1 at the attack down to 0.
    open: f32,
    low: [f32; 2],
    /// A note that waits for the one before it to fall.
    waiting: Option<Note>,
}

impl Bass {
    fn start(&mut self, note: Note) {
        if self.envelope.sounding {
            self.waiting = Some(note);
            self.envelope.cut();
        } else {
            self.begin(note);
        }
    }

    fn begin(&mut self, note: Note) {
        self.note = note;
        self.phase = 0.0;
        self.open = 1.0;
        self.envelope.start(note.frames, BASS_RISE, GAP);
    }

    fn frame(&mut self, held: bool) -> f32 {
        if !self.envelope.sounding {
            match self.waiting.take() {
                Some(note) => self.begin(note),
                None => return 0.0,
            }
        }
        let level = self.envelope.next(held) * self.note.loud;
        let step = self.note.step;
        let mut wave = if self.phase < 0.5 { 1.0 } else { -1.0 };
        wave += blep(self.phase, step) - blep(wrap(self.phase + 0.5), step);
        self.phase = wrap(self.phase + step);

        let weight = pole(BASS_FLOOR + BASS_SWEEP * self.open);
        self.open *= BASS_CLOSE;
        self.low[0] += weight * (wave - self.low[0]);
        self.low[1] += weight * (self.low[0] - self.low[1]);
        self.low[1] * level
    }
}

/// A plucked string: a burst of noise that goes round a delay as long as one period of the
/// note, and wears off a little at each turn.
struct Pluck {
    string: [f32; STRING],
    length: usize,
    at: usize,
    /// The last frame that left the string, for the average that wears it off.
    last: f32,
    /// A filter that delays by a part of a frame, for notes whose period is not a whole
    /// number of frames: its weight, and what went in and came out last.
    tune: f32,
    tuned: [f32; 2],
    /// What the string keeps of itself at each turn.
    keep: f32,
    /// Frames before the string is silent.
    left: u32,
    /// Whether the note has an echo.
    echo: bool,
}

impl Pluck {
    fn new() -> Self {
        Self {
            string: [0.0; STRING],
            length: 1,
            at: 0,
            last: 0.0,
            tune: 0.0,
            tuned: [0.0; 2],
            keep: 0.0,
            left: 0,
            echo: false,
        }
    }

    fn strike(&mut self, tone: Tone, echo: bool) {
        self.echo = echo;
        let hz = frequency(tone.pitch);
        // One turn takes the length of the string, half a frame for the average and the
        // delay of the tuning filter, which is kept between 0.2 and 1.2 frames.
        let period = RATE / hz;
        self.length = (period - 0.7) as usize;
        let rest = period - 0.5 - self.length as f32;
        self.tune = (1.0 - rest) / (1.0 + rest);
        self.tuned = [0.0; 2];
        self.last = 0.0;
        self.at = 0;
        // What is lost at each turn makes the note fall by 60 dB in the time it rings.
        self.keep = 1.0 - 6.908 / (hz * RING);
        self.left = (1.5 * RING * RATE) as u32;

        // The burst is the same noise for every note. It is smoothed, which takes the edge
        // off the attack, then centered on zero and brought to the loudness of the note.
        let burst = &mut self.string[..self.length];
        let mut noise = Noise(PLUCK_SEED);
        for frame in burst.iter_mut() {
            *frame = noise.next();
        }
        for _ in 0..SMOOTHING {
            let mut before = burst[self.length - 1];
            for frame in burst.iter_mut() {
                (*frame, before) = (0.5 * (*frame + before), *frame);
            }
        }
        let mean = burst.iter().sum::<f32>() / self.length as f32;
        let mut peak = 0.0f32;
        for frame in burst.iter_mut() {
            *frame -= mean;
            peak = peak.max(frame.abs());
        }
        let scale = loudness(tone.level) / peak;
        for frame in burst.iter_mut() {
            *frame *= scale;
        }
    }

    /// Stops the string: it dies away in a moment.
    fn stop(&mut self) {
        self.keep = self.keep.min(STOPPED);
    }

    fn frame(&mut self) -> f32 {
        if self.left == 0 {
            return 0.0;
        }
        self.left -= 1;
        let out = self.string[self.at];
        let mean = 0.5 * (out + self.last);
        self.last = out;
        let tuned = self.tune * mean + self.tuned[0] - self.tune * self.tuned[1];
        self.tuned = [mean, tuned];
        self.string[self.at] = tuned * self.keep;
        self.at += 1;
        if self.at == self.length {
            self.at = 0;
        }
        out
    }
}

/// A pulse wave with a slow vibrato.
#[derive(Default)]
struct Lead {
    envelope: Envelope,
    note: Note,
    phase: f32,
    /// Frames since the note began, and where the vibrato is in its period.
    age: f32,
    wobble: f32,
    low: f32,
    /// A note that waits for the one before it to fall.
    waiting: Option<Note>,
}

impl Lead {
    fn start(&mut self, note: Note) {
        if self.envelope.sounding {
            self.waiting = Some(note);
            self.envelope.cut();
        } else {
            self.begin(note);
        }
    }

    fn begin(&mut self, note: Note) {
        self.note = note;
        self.phase = 0.0;
        self.age = 0.0;
        self.wobble = 0.0;
        self.envelope.start(note.frames, LEAD_RISE, GAP);
    }

    /// Lets go of the note, and of the one that waits behind it.
    fn release(&mut self) {
        self.waiting = None;
        self.envelope.release();
    }

    fn frame(&mut self, held: bool) -> f32 {
        if !self.envelope.sounding {
            match self.waiting.take() {
                Some(note) => self.begin(note),
                None => return 0.0,
            }
        }
        let level = self.envelope.next(held) * self.note.loud;
        let depth = VIBRATO_DEPTH * (self.age / VIBRATO_AFTER - 1.0).clamp(0.0, 1.0);
        let step = self.note.step * (1.0 + depth * sine(self.wobble));
        self.wobble = wrap(self.wobble + VIBRATO_RATE / RATE);
        self.age += 1.0;

        // The wave is up for a part of its period and down for the rest, less its mean.
        let mut wave = if self.phase < LEAD_WIDTH { 1.0 } else { -1.0 };
        wave += blep(self.phase, step) - blep(wrap(self.phase + 1.0 - LEAD_WIDTH), step);
        wave -= 2.0 * LEAD_WIDTH - 1.0;
        self.phase = wrap(self.phase + step);

        self.low += LEAD_POLE * (wave - self.low);
        self.low * level
    }
}

/// A sine that falls fast, without any noise: every weapon cue has some.
#[derive(Default)]
struct Kick {
    phase: f32,
    /// How far above its floor the pitch still is, in Hz.
    drop: f32,
    level: f32,
}

impl Kick {
    fn hit(&mut self, strength: f32) {
        *self = Self {
            phase: 0.0,
            drop: KICK_DROP,
            level: strength,
        };
    }

    fn frame(&mut self) -> f32 {
        if self.level == 0.0 {
            return 0.0;
        }
        let out = sine(self.phase) * self.level;
        self.phase = wrap(self.phase + (KICK_FLOOR + self.drop) / RATE);
        self.drop *= KICK_SWEEP;
        self.level *= KICK_FALL;
        if self.level < SILENT {
            self.level = 0.0;
        }
        out
    }
}

/// A short burst of the top of white noise.
struct Hat {
    noise: Noise,
    level: f32,
    low: [f32; 2],
}

impl Hat {
    fn hit(&mut self, strength: f32) {
        *self = Self {
            noise: Noise(HAT_SEED),
            level: strength,
            low: [0.0; 2],
        };
    }

    fn frame(&mut self) -> f32 {
        if self.level == 0.0 {
            return 0.0;
        }
        // Twice over, the noise loses what a low-pass filter keeps of it.
        let mut high = self.noise.next();
        for low in &mut self.low {
            *low += HAT_POLE * (high - *low);
            high -= *low;
        }
        let out = high * self.level;
        self.level *= HAT_FALL;
        if self.level < SILENT {
            self.level = 0.0;
        }
        out
    }
}

/// A burst of the middle of white noise over a short tone.
struct Snare {
    noise: Noise,
    level: f32,
    /// The filter that keeps the middle of the noise: its band and what is below it.
    band: f32,
    low: f32,
    phase: f32,
    tone: f32,
}

impl Snare {
    fn hit(&mut self, strength: f32) {
        *self = Self {
            noise: Noise(SNARE_SEED),
            level: strength,
            band: 0.0,
            low: 0.0,
            phase: 0.0,
            tone: strength * SNARE_TONE_LEVEL,
        };
    }

    fn frame(&mut self) -> f32 {
        if self.level == 0.0 {
            return 0.0;
        }
        let high = self.noise.next() - self.low - SNARE_DAMP * self.band;
        self.band += SNARE_TUNE * high;
        self.low += SNARE_TUNE * self.band;
        let out = self.band * self.level + sine(self.phase) * self.tone;
        self.phase = wrap(self.phase + SNARE_TONE / RATE);
        self.tone *= SNARE_TONE_FALL;
        self.level *= SNARE_FALL;
        if self.level < SILENT {
            self.level = 0.0;
        }
        out
    }
}

/// The six instruments, and what they go through before they are heard.
pub struct Synth {
    bass: Bass,
    plucks: [Pluck; PLUCKS],
    /// The string that the next note of the pluck takes.
    next_pluck: usize,
    leads: [Lead; LEADS],
    kick: Kick,
    hat: Hat,
    snare: Snare,
    /// What the pluck played three steps ago.
    echo: Box<[f32]>,
    echo_at: usize,
    hush: Hush,
    muffled: bool,
    /// How muffled the music is, from 0 to 1, and its muffled version on each side.
    muffle: f32,
    dull: [[f32; 2]; 2],
}

impl Default for Synth {
    fn default() -> Self {
        Self {
            bass: Bass::default(),
            plucks: std::array::from_fn(|_| Pluck::new()),
            next_pluck: 0,
            leads: std::array::from_fn(|_| Lead::default()),
            kick: Kick::default(),
            hat: Hat {
                noise: Noise(HAT_SEED),
                level: 0.0,
                low: [0.0; 2],
            },
            snare: Snare {
                noise: Noise(SNARE_SEED),
                level: 0.0,
                band: 0.0,
                low: 0.0,
                phase: 0.0,
                tone: 0.0,
            },
            echo: vec![0.0; ECHO_FRAMES].into(),
            echo_at: 0,
            hush: Hush::None,
            muffled: false,
            muffle: 0.0,
            dull: [[0.0; 2]; 2],
        }
    }
}

impl Synth {
    /// Starts what a step asks for. `at` is where the step starts, in half frames since the
    /// piece began, which tells how many whole frames its notes last.
    pub fn play(&mut self, step: &Step, at: u64) {
        let frames = |steps: u8| {
            let end = at + steps as u64 * step.length as u64;
            (end / 2 - at / 2) as u32
        };
        self.muffled = step.muffled;

        if step.fresh {
            self.bass.envelope.release();
            for lead in &mut self.leads {
                lead.release();
            }
        }
        if step.hush == Hush::BassOnly && self.hush != Hush::BassOnly {
            for lead in &mut self.leads {
                lead.release();
            }
            for pluck in &mut self.plucks {
                pluck.stop();
            }
        }
        self.hush = step.hush;

        if let Some(tone) = step.bass {
            self.bass.start(Note::new(tone, frames(tone.steps)));
        }
        if let Some(tone) = step.pluck {
            self.plucks[self.next_pluck].strike(tone, step.echo);
            self.next_pluck = (self.next_pluck + 1) % PLUCKS;
        }
        // The lead plays one line: its new notes take the place of the old ones.
        if step.lead[0].is_some() {
            for (lead, tone) in self.leads.iter_mut().zip(step.lead) {
                match tone {
                    Some(tone) => lead.start(Note::new(tone, frames(tone.steps))),
                    None => lead.release(),
                }
            }
        }
        if step.kick > 0 {
            self.kick.hit(loudness(step.kick));
        }
        if step.hat > 0 {
            self.hat.hit(loudness(step.hat));
        }
        if step.snare > 0 {
            self.snare.hit(loudness(step.snare));
        }
    }

    /// The next frame of the music: its left and its right.
    pub fn frame(&mut self) -> [f32; 2] {
        // While the music is hushed the bass holds its note, and so does the lead if the
        // hush is the end of a fight.
        let bass = self.bass.frame(self.hush != Hush::None);
        let (mut pluck, mut echoed) = (0.0, 0.0);
        for string in &mut self.plucks {
            let frame = string.frame();
            pluck += frame;
            if string.echo {
                echoed += frame;
            }
        }
        let mut lead = 0.0;
        for voice in &mut self.leads {
            lead += voice.frame(self.hush == Hush::Held);
        }
        let (kick, hat, snare) = (self.kick.frame(), self.hat.frame(), self.snare.frame());

        // The echo of the arpeggio comes back on the other side.
        let echo = self.echo[self.echo_at] * ECHO_LEVEL;
        self.echo[self.echo_at] = echoed;
        self.echo_at = (self.echo_at + 1) % ECHO_FRAMES;

        let target = if self.muffled { 1.0 } else { 0.0 };
        self.muffle += (target - self.muffle).clamp(-MUFFLE_RATE, MUFFLE_RATE);

        let mut out = [0.0; 2];
        for (side, out) in out.iter_mut().enumerate() {
            let mix = bass * BASS_MIX[side]
                + pluck * PLUCK_MIX[side]
                + echo * PLUCK_MIX[1 - side]
                + lead * LEAD_MIX[side]
                + kick * KICK_MIX[side]
                + hat * HAT_MIX[side]
                + snare * SNARE_MIX[side];
            // The muffled version is always computed, so that going to it is smooth.
            let [first, second] = &mut self.dull[side];
            *first += MUFFLE_POLE * (mix - *first);
            *second += MUFFLE_POLE * (*first - *second);
            let heard = mix + self.muffle * (*second - mix);
            *out = limit(heard * (1.0 - MUFFLE_LEVEL * self.muffle));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::score::STEP;

    /// A step that starts one note on one instrument.
    fn note(instrument: &str, pitch: u8, steps: u8) -> Step {
        let tone = Some(Tone {
            pitch,
            steps,
            level: 255,
        });
        let step = Step {
            length: STEP,
            ..Step::default()
        };
        match instrument {
            "bass" => Step { bass: tone, ..step },
            "pluck" => Step {
                pluck: tone,
                ..step
            },
            _ => Step {
                lead: [tone, None],
                ..step
            },
        }
    }

    /// The left side of some frames of a synthesizer.
    fn listen(synth: &mut Synth, frames: usize) -> Vec<f32> {
        (0..frames).map(|_| synth.frame()[0]).collect()
    }

    /// The period of a sound in frames, near the one it is expected to have: the lag at which
    /// the sound is most like itself.
    fn period(sound: &[f32], expected: f32) -> f32 {
        let likeness = |lag: usize| -> f64 {
            let pairs = sound.iter().zip(&sound[lag..]);
            pairs.map(|(&a, &b)| a as f64 * b as f64).sum()
        };
        // Within a semitone either way.
        let (low, high) = ((expected / 1.06) as usize, (expected * 1.06) as usize + 1);
        let best = (low..=high)
            .max_by(|&a, &b| likeness(a).total_cmp(&likeness(b)))
            .unwrap();
        // The top of the curve through the best lag and its two neighbors.
        let (before, at, after) = (likeness(best - 1), likeness(best), likeness(best + 1));
        best as f32 + (0.5 * (before - after) / (before - 2.0 * at + after)) as f32
    }

    #[test]
    fn the_sine_is_exact_to_a_few_millionths() {
        for index in 0..=10_000 {
            let turns = index as f32 / 10_000.0;
            let exact = (turns as f64 * std::f64::consts::TAU).sin();
            assert!((sine(turns) as f64 - exact).abs() < 5e-6, "{turns}");
        }
    }

    #[test]
    fn pitches_are_those_of_equal_temperament() {
        assert_eq!(frequency(69), 440.0);
        assert_eq!(frequency(45), 110.0);
        assert_eq!(frequency(93), 1760.0);
        for pitch in 36..=100u8 {
            let exact = 440.0 * 2f64.powf((pitch as f64 - 69.0) / 12.0);
            assert!(
                (frequency(pitch) as f64 / exact - 1.0).abs() < 1e-6,
                "{pitch}"
            );
        }
    }

    #[test]
    fn every_instrument_is_in_tune() {
        // The lowest and the highest note of each instrument, and some between them. The
        // lead is heard before its vibrato comes in, and the bass once its filter has closed:
        // a filter that moves bends the pitch by a few cents while it does.
        let notes = [
            ("bass", vec![40, 45, 50, 62], 14_000..26_000),
            ("pluck", vec![57, 61, 64, 69, 73, 78, 81], 800..5800),
            ("lead", vec![69, 73, 76, 83, 88, 95], 800..5800),
        ];
        for (instrument, pitches, heard) in notes {
            for pitch in pitches {
                let mut synth = Synth::default();
                synth.play(&note(instrument, pitch, 16), 0);
                let sound = &listen(&mut synth, heard.end)[heard.clone()];
                let expected = RATE / frequency(pitch);
                let cents = 1200.0 * (expected / period(sound, expected)).log2();
                assert!(cents.abs() < 3.0, "{instrument} {pitch}: {cents} cents off");
            }
        }
    }

    #[test]
    fn a_note_rises_from_nothing_and_falls_to_nothing() {
        let mut envelope = Envelope::default();
        envelope.start(1000, 100, GAP);
        let levels: Vec<f32> = (0..1000).map(|_| envelope.next(false)).collect();
        assert!(levels[0] > 0.0 && levels[0] <= 0.01);
        assert!(levels[98] < 1.0);
        assert_eq!(levels[100], 1.0);
        assert_eq!(levels[1000 - GAP as usize - 1], 1.0);
        assert!(
            levels
                .windows(2)
                .all(|pair| (pair[1] - pair[0]).abs() <= 0.0101)
        );
        assert_eq!(levels[999], 0.0);
        // The frame after its last one, the note is over and the next can start.
        assert!(!envelope.sounding);
    }

    #[test]
    fn a_held_note_lets_go_only_when_it_is_told_to() {
        let mut envelope = Envelope::default();
        envelope.start(1000, 100, GAP);
        for _ in 0..5000 {
            envelope.next(true);
        }
        assert_eq!(envelope.next(true), 1.0);
        // A note that waits behind it makes it fall in 3 ms, without a jump.
        envelope.cut();
        let levels: Vec<f32> = (0..CUT).map(|_| envelope.next(true)).collect();
        assert!(levels[0] < 1.0 && levels[0] > 0.98);
        assert_eq!(levels[CUT as usize - 1], 0.0);
        assert!(!envelope.sounding);
    }

    #[test]
    fn a_note_that_waits_starts_once_the_last_one_has_fallen() {
        let mut synth = Synth::default();
        synth.play(&note("bass", 45, 16), 0);
        listen(&mut synth, 20_000);
        synth.play(&note("bass", 52, 16), 4 * STEP as u64);
        let sound = listen(&mut synth, 400);
        // Nothing jumps while one note gives way to the other.
        let jump = sound.windows(2).map(|pair| (pair[1] - pair[0]).abs());
        assert!(jump.fold(0.0, f32::max) < 0.02);
        assert_eq!(synth.bass.note.step, frequency(52) / RATE);
    }

    #[test]
    fn drums_die_out() {
        let mut synth = Synth::default();
        let hit = Step {
            kick: 255,
            hat: 255,
            snare: 255,
            ..Step::default()
        };
        synth.play(&hit, 0);
        let sound = listen(&mut synth, RATE as usize);
        assert!(sound[..2000].iter().any(|level| level.abs() > 0.2));
        assert!(sound[RATE as usize / 2..].iter().all(|&level| level == 0.0));
        assert_eq!(synth.hat.level, 0.0);
    }

    #[test]
    fn the_limit_bends_what_is_loud_and_never_clips() {
        assert_eq!(limit(0.3), 0.3);
        assert_eq!(limit(-KNEE), -KNEE);
        let mut last = 0.0;
        for index in 1..=4000 {
            let level = index as f32 / 100.0;
            let bent = limit(level);
            assert!(bent > last && bent < 1.0, "{level}");
            assert_eq!(limit(-level), -bent);
            last = bent;
        }
    }

    #[test]
    fn a_muffled_music_is_duller_and_quieter() {
        let power = |sound: &[f32]| sound.iter().map(|level| level * level).sum::<f32>();
        let mut synth = Synth::default();
        let lead = note("lead", 81, 64);
        synth.play(&lead, 0);
        let clear = power(&listen(&mut synth, 20_000)[10_000..]);
        synth.play(
            &Step {
                muffled: true,
                ..Step::default()
            },
            1,
        );
        // It gets there within 0.2 s.
        let muffled = power(&listen(&mut synth, 20_000)[10_000..]);
        assert_eq!(synth.muffle, 1.0);
        assert!(muffled < clear / 8.0, "{muffled} against {clear}");
    }
}
