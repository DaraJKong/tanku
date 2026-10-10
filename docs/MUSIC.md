# Tanku: music specification

Status: a proposal. Nothing in it is built, nobody has heard the notes, and [PROJECT.md](PROJECT.md) still says that music comes later.

The music of Tanku is one piece that never stops. The game makes it while it plays, from two short motifs and six instruments that it computes itself, so no sound file ships. The piece is the same for every seed and on every platform. What changes is its arrangement, which follows the run: where the players are, whether they are fighting, and how smart the enemies in front of them are.

All numbers and all notes in this document are starting values to tune by ear.

## Overview

| Topic | Summary |
|---|---|
| Piece | One, in A, at 120 beats per minute, without an end |
| Written by hand | Two motifs of two bars, three chord progressions, nine rhythm patterns |
| Derived by rule | Everything else: both modes, every section, every arrangement, every sound |
| Instruments | Bass, pluck, lead, kick, hi-hat and snare, all synthesized by the game |
| Follows | The place (bay, corridor, arena), the fight (on or off) and the threat (the highest brain tier awake) |
| Ignores | The seed, the platform and the clock of the machine |
| Code | A crate of its own, `tanku_audio`, which depends on nothing |

### Principles

1. **Cues first.** Sound cues carry information, and the music never covers one.
2. **The music follows the game.** The simulation never waits for a beat and never reads the music. A run plays the same with the music off.
3. **It repeats what the screen shows.** The music tells the place, the fight and the threat, which the screen tells too. It is never the only carrier of anything.
4. **One piece.** Two motifs come back in every section, so the piece is recognized in every arrangement.
5. **Same everywhere.** The same events at the same times give the same samples on every platform.

## Glossary

| Term | Meaning |
|---|---|
| Step | A quarter of a beat (a sixteenth note), the smallest unit of time |
| Bar | Four beats, or 16 steps |
| Phrase | Four bars |
| Degree | The place of a note in the scale, counted from the tonic, which is 1. Degree 8 is the tonic an octave higher. |
| Mode | The scale in use: Ionian (major) or Aeolian (minor) |
| Motif | A melody of two bars, written in degrees |
| Section | Eight bars with their chords and their melody: intro, verse, chorus or bridge |
| Layer | What one instrument plays |
| Arrangement | The layers that play together |
| Context | What the game tells the music |
| Threat | The highest brain tier among the enemies that are awake |
| Marker | A short gesture that answers an event at once, before the next bar line |

## Time

| Quantity | Value |
|---|---|
| Tempo | 120 beats per minute, which never changes |
| Bar | 2 s, or 120 simulation ticks |
| Step | 0.125 s |
| Phrase | 8 s |
| Swing | None. It is the first knob to try if the rhythm feels stiff. |

The tempo is the game's own. At 120 beats per minute, the timings of the rules are whole numbers of beats:

| Timing in the rules | Length | In the music |
|---|---|---|
| Time between two shots of a gun | 0.25 s | Half a beat |
| Shield, time between two mines, time without shooting after a tank appears | 0.5 s | One beat |
| Reload of one round of the player tank, arming of a mine | 1 s | Two beats |
| Respawn | 1.5 s | Three beats |
| Repair | 4 s | Two bars |
| Fuse of a mine | 10 s | Five bars |

A player who holds the trigger therefore fires at the pace of the music, two shots per beat. The shots don't land on its steps, because the game never moves a shot to fit the music. This only holds while the tempo stays at 120, which is one reason why it never changes.

## Pitch

The piece is in A and has two modes, which share that tonic:

| Degree | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|---|---|
| Ionian (major) | A | B | C♯ | D | E | F♯ | G♯ |
| Aeolian (minor) | A | B | C | D | E | F | G |

Every line is written in degrees and not in notes, so it exists in both modes without being written twice. Two rules keep a line sound in both:

- **Five degrees for the motifs.** The motifs use degrees 1, 2, 3, 5 and 6 only. In Ionian these are A, B, C♯, E and F♯, the major pentatonic scale. In Aeolian they are A, B, C, E and F, a scale of five notes with two semitones that is known as [hirajōshi](https://piano.samesound.ru/en/reference/scales/piano/a/hirajoshi). The same tune is bright in the first and tense in the second.
- **Four degrees for the chords.** Chords are triads on degrees 1, 4, 5 and 6, the only four whose triad is major or minor in both modes.

| Chord on degree | 1 | 4 | 5 | 6 |
|---|---|---|---|---|
| Ionian | A | D | E | F♯m |
| Aeolian | Am | Dm | Em | F |

The tonic is A because of the cues. Those that play notes already play A and E: sector cleared (E5, A5), flag captured (E5, A5, E6), barrier unlocked (A5, E6) and run over (A4, E4, A3). A and E are degrees 1 and 5 of both modes, so these cues are in tune with the music whatever it plays. The key never changes, for the same reason.

## Material

Three things are written by hand: the motifs, the chords and the patterns.

A grid shows bars of 16 steps, in four beats. In a melody, a digit starts a note on that degree and a dot holds it. In a pattern, `x` is a hit, `R` is the root of the chord of the bar, `O` is that root an octave higher, `5` is its fifth, and a dot starts nothing.

### Motifs

```
Motif A, the question    1..2 ..3. 5..3 ..2. | 3..5 ..6. .... 5...
Motif B, the answer      8... ..9. 8... 6... | 5..6 ..5. 3..2 ..1.
```

Motif A climbs and stops on degree 5, so it asks for more. Motif B starts on the high tonic and comes down to the tonic, so it closes. The first bar of A and the second bar of B share one rhythm, twice 3 + 3 + 2 steps. It is the rhythm of the piece, and the bass plays it too.

### Chords

One chord per bar:

| Section | First phrase | Second phrase |
|---|---|---|
| Intro and verse | 1, 1, 6, 4 | 1, 1, 6, 5 |
| Chorus | 4, 5, 1, 6 | 4, 5, 1, 1 |
| Bridge | 4, 4, 1, 1 | 4, 4, 5, 5 |

### Patterns

```
Kick, calm           x... .... x... ....
Kick, fight          x... x... x... x...
Hi-hat, corridor     ..x. ..x. ..x. ..x.
Hi-hat, arena        xxxx xxxx xxxx xxxx    louder on the steps of the corridor pattern
Snare, arena         .... x... .... x...
Bass, long           R... .... .... ....
Bass, plain          R..R ..R. R..R ..R.
Bass, leaping        R..O ..R. R..O ..5.
Pluck, arpeggio      x.x. x.x. x.x. x.x.    root, third and fifth of the chord in turn, rising
```

Syncopation lives in the bass, the pluck and the lead. The drums stay on a plain grid on purpose: a regular pulse is easy to stop hearing, and a shot, which is irregular, stands out against it.

### Rules of derivation

| Rule | What it does | Used by |
|---|---|---|
| Mode | Reads each degree in Ionian or in Aeolian | Everything with a pitch |
| Skeleton | Keeps the notes of a motif that start on a beat and holds each until the next | Lead at threat 1 |
| Ornaments | A note of three steps or more gives its last two steps to the degree above it, among the five, and to itself again | Lead at threats 3 and 4 |
| Octave | Plays the same line an octave higher as well | Lead at threat 4 |
| Half speed | Makes every note twice as long | Pluck in the bridge |
| Turnaround | Swaps the long degree 6 of motif A for degree 2, a note of chord 5: its second bar becomes `3..5 ..2. .... 5...` | Last bar of the intro and of the verse, which is on chord 5 |
| Triads | Holds the chord of the bar in the inversion nearest to the last one | Lead in the bridge |

One check guards every change to the notes. In both modes, a note that lies a semitone above the root of its chord must pass quickly: it lasts one step if it starts on a beat, and three steps at most otherwise. The turnaround exists because of it: without it, the last bar of the verse would hold F over E minor in an arena.

## Sections

| Section | Bars | Melody | Feel |
|---|---|---|---|
| Intro | 8 | Motif A four times on the pluck, the last time with the turnaround | Still, without drums |
| Verse | 8 | Motif A four times on the lead, the last time with the turnaround | Driving |
| Chorus | 8 | Motif B four times on the lead | Driving, and higher |
| Bridge | 8 | Motif B twice at half speed on the pluck, over triads on the lead | Half as fast: a break |

The piece has no fixed order of sections. The game chooses them through the context, and each section loops for as long as its context lasts. The bridge is therefore not on a timer: it is the music of the calm between two fights.

## Context

The context is all that the music knows about the game. The client writes it every frame from what it already reads from the simulation.

| Part | Values | Meaning |
|---|---|---|
| Place | Menu, bay, corridor, arena | The sector the run has reached. The main menu and the game over screen count as the menu, and a switchback is a corridor. |
| Fight | On or off | On while at least one enemy is awake |
| Threat | 0 to 4 | The highest brain tier among the enemies that are awake |
| Down | On or off | In solo, the player's tank is destroyed and not back yet |
| Muffled | On or off | A menu is open over the game |

In multiplayer the context is the team's, so every player hears the same arrangement, and a fight starts for everyone when the first player crosses. A wrecked player hears the music muffled until repaired.

## Arrangement

The drums and the mode tell where the players are and whether they fight. The lead tells whom they fight.

| Context | Section | Mode | Kick | Hi-hat | Snare | Bass | Pluck | Lead |
|---|---|---|---|---|---|---|---|---|
| Menu or bay | Intro | Ionian | Silent | Silent | Silent | Long | Motif A | Silent |
| No fight, in a corridor or an arena | Bridge | Ionian | Calm | Silent | Silent | Long | Motif B at half speed | Triads |
| Fight in a corridor | Verse and chorus in turn | Ionian | Fight | Corridor | Silent | By threat | By threat | By threat |
| Fight in an arena | Verse and chorus in turn | Aeolian | Fight | Arena | Arena | By threat | By threat | By threat |

The mode is Aeolian only while the enemies of an arena are alive. The moment the last one is destroyed, the music turns back to major, which makes clearing an arena heard as a relief.

During a fight, the threat sets the three instruments that have a pitch:

| Threat | Brain | Lead | Bass | Pluck |
|---|---|---|---|---|
| 0 | Sentry | Silent | Plain | Arpeggio |
| 1 | Grunt | Skeleton of the motif | Plain | Arpeggio |
| 2 | Soldier | The motif | Leaping | Arpeggio |
| 3 | Veteran | The motif with ornaments | Leaping | Arpeggio |
| 4 | Ace | The motif with ornaments, in octaves | Leaping | Arpeggio, twice as fast |

The threat falls during a fight when the smartest enemies are destroyed first, and the lead then thins out. At the start of a fight it is the highest tier the sector allows about three times out of four, so over a run it mostly rises with the sector number: the lead comes in during the first sectors, plays the whole motif from sector 6 and reaches its fullest form from sector 16.

## Changes

Two rules make the music answer fast and stay in time:

1. Every change of section, of mode or of pattern happens on a bar line.
2. What can't wait for the bar line is said at once by a marker.

| Event | At once | On the next bar line |
|---|---|---|
| A fight starts | A roll that grows louder up to the bar line, on the hi-hat in a corridor and on the snare in an arena. It lasts one beat at least: when the bar line is closer than that, it runs to the one after. | The fight sections start, in Aeolian in an arena |
| The fight ends | The drums stop and the notes that are sounding are held | The bridge starts from its first bar, in Ionian |
| The threat changes | Nothing | Lead, bass and pluck change |
| The player's tank is destroyed | Everything stops but the bass, which holds its note | Once the tank is back, the context decides again |
| The run ends | The same | The intro starts |
| A run starts | Nothing: the intro goes on | The count of phrases starts over |
| A menu opens over the game | Within 0.2 s the music loses what is above 600 Hz and half its loudness. It gets them back when the menu closes. | Nothing |

The fight sections are counted in phrases: the two of the verse, then the two of the chorus, then the verse again. A fight opens on the phrase after the one where the last fight ended. The first fight of a run opens the verse, so every run begins the same way, and the short fights of the first sectors walk through the whole piece instead of repeating its first bars.

A marker is heard within one step of its event. A change is complete within one bar, or within a bar and a beat when a fight starts.

## Instruments

Each instrument is a small synthesizer made of plain shapes, like the drawing: square, pulse and sine waves, noise, and one plucked string.

| Instrument | Sound | Notes at once | Range | Place in stereo |
|---|---|---|---|---|
| Bass | A square wave through a filter that opens at each attack and closes in 0.15 s | 1 | E2 to D4 | Middle |
| Pluck | A plucked string: a burst of noise that goes round a tuned delay and wears off | 4 | A3 to A5 | Left |
| Lead | A pulse wave with a slow vibrato. In the bridge, a softer attack and three notes at once. | 3 | A4 to B6 | Right |
| Kick | A sine that falls from 110 Hz to 55 Hz in 0.12 s, without any noise | 1 | None | Middle |
| Hi-hat | Noise above 7 kHz for 0.03 s | 1 | None | Left |
| Snare | Noise around 2 kHz with a tone at 220 Hz, for 0.12 s | 1 | None | Middle |

The kick has no noise in it because every weapon cue has some. The bass stays above 80 Hz, which leaves the bottom to the kick and to the blasts and keeps it audible on small loudspeakers.

## Mix

- **Two volumes.** The settings get a second level, Music, next to Volume. Volume sets everything and Music sets the music against the cues. It starts at 50%. At 0 the music keeps counting its bars and stays silent.
- **Under the cues.** At the default settings, the loudest arrangement peaks 12 dB below a shot fired next to the player.
- **Ducking.** While the warning of a mine sounds within hearing range, the music is half as loud.
- **Stereo.** The music is in stereo and the cues stay in the middle. The pluck and the lead, which share the pitches of the cues, sit to the sides.
- **Dry.** There is no reverb. The pluck has one echo, three steps later, a third as loud and on the other side.
- **No clipping.** The sum of the instruments goes through a soft limit.

## Cues

The waveforms of the cues move to `tanku_audio`, next to the instruments, so that everything the game computes as sound is in one place and shares the same notes. Playing them stays in the client, which knows where a sound comes from and how loud it is.

Two cues have a pitch that will sound out of tune over music in A:

| Cue | Today | Proposed |
|---|---|---|
| Mine placed | 620 Hz, six semitones from A | 660 Hz (E5) |
| Shield blocked | 900 Hz and 1350 Hz, four tenths of a semitone above A5 and E6 | 880 Hz and 1320 Hz |
| Mine triggered | 1500 Hz, a quarter of a semitone above F♯6 | Unchanged: a warning should stand apart from the music |

## Same everywhere

| What | Depends on | Guarantee |
|---|---|---|
| The piece: notes, patterns, sounds | Version | Identical on every platform |
| The arrangement at a given moment | Version and the context since the game started | Identical on every platform |
| What reaches the ears | The above and the sound device | Not guaranteed: devices differ in rate and delay |

- **Nothing is random.** The noise of the hi-hat, the snare and the pluck comes from a generator of its own that always starts from the same state, as the noise of the cues does today.
- **The score is integers.** What each instrument plays on a step is a function of the step number and of the context. At 44,100 frames per second, step $k$ of the piece starts at frame $\lfloor 11025\,k / 2 \rfloor$.
- **The synthesizer does its own math.** It adds, subtracts, multiplies and divides 32-bit floats, and it reads sines and decays from its own tables. It never calls a library function such as `sin` or `exp`, whose last bit differs between platforms. Its output is therefore the same bit for bit on every platform, which lets a test compare it with a stored value.

## Engine

```
tanku_audio
├── score       the motifs, the chords, the patterns and the rules of derivation
├── director    from the context to what each instrument plays on each step
├── synth       the six instruments, the echo and the mix
├── cues        the waveforms of the sound cues
└── wav         writes a sound file, for the listening tool and the tests
```

The crate has two ends. The game holds a `Conductor` and writes the context into it. The sound thread holds the `Music` and asks it for frames:

```rust
let (music, conductor) = tanku_audio::music();
// In the client, every frame:
conductor.set(Context { place: Place::Corridor, fight: true, threat: 2, ..default() });
// On the sound thread, whenever the device wants more:
music.render(&mut frames);
```

| Topic | Requirement |
|---|---|
| Output | Stereo, 44,100 frames per second, 32-bit floats. It is the rate the sound library opens by default on Linux and on the web, so nothing resamples the music there. |
| Dependencies | None, not even Bevy |
| Real time | `render` never allocates, locks or waits. The context crosses between the threads as one atomic number, read at the start of every step. |
| Cost | Under 10 ms of one core for one second of the fullest arrangement |
| Development builds | The crate is compiled with full optimization, like the dependencies, because it runs in real time |
| Start | Nothing is computed ahead, so the music adds nothing to the time the game takes to start or to restart |

In the client, the music is an asset that implements Bevy's `Decodable`, which Bevy mixes with the cues. A system writes the context from `Screen`, `Overlay`, `Run`, the `Sector` the run has reached and the genomes of the enemies that are awake. Nothing changes in the server or in the simulation. A scripted run and the headless server have no music.

On the web, the browser lets no sound play before the first click or key, so the music starts there. The web build also mixes its sound on the main thread, 46 ms at a time: a frame that takes longer than about 50 ms leaves a gap in the music. The cues have the same limit today, but a gap in a held note is heard far more than one in a short cue.

A tool renders the music without the game:

```bash
cargo run -p tanku_audio --example render -- tour.wav
```

It writes a tour through a made-up run, or one arrangement at a time, and it can lay cues over the music.

## Tests and benchmarks

- **Stable.** Sixteen bars of each arrangement match a stored value, as the map of a seed does.
- **Never silent.** Every bar of every arrangement makes sound.
- **Never too loud.** No frame of any arrangement or of any change goes past full scale, and none is NaN.
- **Seamless.** The third pass of a loop is identical to the second.
- **On the bar.** A context that changes on each of the 16 steps of a bar gives a marker within one step and a section that starts on a bar line.
- **Sound in both modes.** The motifs use the five degrees, the chords use the four, and every melody, with its skeleton and its ornaments, passes the check on semitones above the root.
- **Cost.** A benchmark renders ten seconds of a fight in an arena at threat 4.

## Steps

Each step answers a question, and the first one changes nothing in the game.

| Step | Content | Question it answers |
|---|---|---|
| 1, Sketch | The crate with the score, rough instruments and the listening tool. Sound files of each section in both modes, of each threat, and of a tour of three minutes. | Are the two motifs worth keeping? Does the minor mode suit the arenas? |
| 2, In the game | The music plays in the client and follows the place and the fight. The Music volume. A switch in the playtest menu between drums that follow the place only and drums that follow the fight too. | Does the music feel tied to what happens? Can a shot off screen still be told from the beat? |
| 3, Threat | Lead, bass and pluck follow the brain tier | Can a player tell the smarter of two groups by ear? |
| 4, Cues | The waveforms of the cues move to the crate, two cues are retuned, and the warning of a mine ducks the music | Is any cue lost under the music? |
| 5, Web | The web build with music | Does the music play without gaps in a browser? |

## Open questions

- How long are the frames of the web build when a sector loads, and is the gap they leave in the music acceptable?
- Two machines in one room play the same piece out of step, because each has its own clock. Does co-op in one room need them in step?
- In hard mode every tier is available from sector 1, so the lead is at its fullest almost at once. Is that the right sound for hard mode, or does it waste the build-up?
