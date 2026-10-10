# Tanku: music specification

Status: steps 1 to 3 of the [steps](#steps) are built. The sketch of step 1 was listened to four times, and its notes are accepted. With step 2 the game plays the music on the desktop, where it follows the place and the fight and has a volume of its own. A playtest answered its questions: the music that follows the fight too was kept over music that follows the place only, and a shot sounds different from the beat, so one is told from the other. With step 3 the lead, the bass and the pluck follow the brains of the enemies that are awake: nobody has played with it yet. The web build has no music until step 5.

The music of Tanku is one piece that never stops. The game makes it while it plays, from two short motifs and six instruments that it computes itself, so no sound file ships. The piece is the same for every seed and on every platform. What changes is its arrangement, which follows the run: where the players are, whether they are fighting, and how smart the enemies in front of them are.

All numbers and all notes in this document are starting values to tune by ear.

## Overview

| Topic | Summary |
|---|---|
| Piece | One, in A, at 120 beats per minute and at 105 in its bridge, without an end |
| Written by hand | Three motifs, the chords of the verse and of the chorus, nine rhythm patterns |
| Derived by rule | Everything else: both modes, every section, every arrangement, every connection, every sound |
| Instruments | Bass, pluck, lead, kick, hi-hat and snare, all synthesized by the game |
| Follows | The place (bay, corridor, arena), the fight (on or off) and the threat (the highest brain tier awake) |
| Ignores | The seed, the platform and the clock of the machine |
| Code | A crate of its own, `tanku_audio`, which depends on nothing |

### Principles

1. **Cues first.** Sound cues carry information, and the music never covers one.
2. **The music follows the game.** The simulation never waits for a beat and never reads the music. A run plays the same with the music off.
3. **It repeats what the screen shows.** The music tells the place, the fight and the threat, which the screen tells too. It is never the only carrier of anything.
4. **One piece.** The same few motifs come back all the time, so the piece is recognized in every arrangement.
5. **Same everywhere.** The same events at the same times give the same samples on every platform.

## Glossary

| Term | Meaning |
|---|---|
| Step | A quarter of a beat (a sixteenth note), the smallest unit of time |
| Bar | Four beats, or 16 steps |
| Phrase | Four bars |
| Degree | The place of a note in the scale, counted from the tonic, which is 1. Degree 8 is the tonic an octave higher. |
| Mode | The scale in use: Dorian, the mode of the piece, or Aeolian in the fights of an arena |
| Motif | A melody of two bars, written in degrees |
| Section | Eight bars with their chords and their melody: intro, verse, chorus or bridge |
| Layer | What one instrument plays |
| Arrangement | The layers that play together |
| Context | What the game tells the music |
| Threat | The highest brain tier among the enemies that are awake |
| Marker | A short gesture that answers an event: the roll that announces a fight, the connection that ends it |

## Time

| Quantity | Value |
|---|---|
| Tempo | 120 beats per minute. The bridge alone is slower, at 105. |
| Bar | 2 s, or 120 simulation ticks. 2.29 s in the bridge. |
| Step | 0.125 s. 0.143 s in the bridge. |
| Phrase | 8 s |
| Swing | None. It is the first knob to try if the rhythm feels stiff. |

The tempo of a fight is the game's own. At 120 beats per minute, the timings of the rules are whole numbers of beats:

| Timing in the rules | Length | In the music |
|---|---|---|
| Time between two shots of a gun | 0.25 s | Half a beat |
| Shield, time between two mines, time without shooting after a tank appears | 0.5 s | One beat |
| Reload of one round of the player tank, arming of a mine | 1 s | Two beats |
| Respawn | 1.5 s | Three beats |
| Repair | 4 s | Two bars |
| Fuse of a mine | 10 s | Five bars |

A player who holds the trigger therefore fires at the pace of the music, two shots per beat. The shots don't land on its steps, because the game never moves a shot to fit the music. This only holds at 120, which is why a fight never has another tempo. The bridge can be slower because nobody shoots back while it plays.

## Pitch

The piece is in A. Its mode is Dorian, and the fights of an arena are in Aeolian:

| Degree | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|---|---|
| Dorian (minor, with a major sixth) | A | B | C | D | E | F♯ | G |
| Aeolian (minor) | A | B | C | D | E | F | G |

Both modes are minor and differ by one note, the sixth degree, so an arena doesn't change the piece: it darkens it. Two major modes were tried for the piece and set aside. In Ionian, the plain major scale, it was too bright and uplifting for a game about fighting. Mixolydian, which has a minor seventh, was built next to Dorian for comparison, and Dorian was the one to keep.

Every line is written in degrees and not in notes, so it exists in both modes without being written twice. Two rules keep a line sound in both:

- **Five degrees for the motifs.** The motifs use degrees 1, 2, 3, 5 and 6 only. In Aeolian these are A, B, C, E and F, a scale of five notes with two semitones that is known as [hirajōshi](https://piano.samesound.ru/en/reference/scales/piano/a/hirajoshi). In Dorian the F becomes F♯. The same tune is tense in an arena without a second version.
- **Four chords per mode.** Chords are major or minor triads. Each mode has those on degrees 1, 4 and 5, and one more that gives it its color: the major chord a tone below the tonic, on degree 7, in Dorian, and the major chord on degree 6 in Aeolian.

| Chord on degree | 1 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|
| Dorian | Am | D | Em | Not used | G |
| Aeolian | Am | Dm | Em | F | Not used |

The bridge is apart. It uses degrees 1, 2, 4, 5 and 7: A, B, D, E and G. It has neither the third degree, which tells major from minor, nor the sixth, which tells Dorian from Aeolian, so it has no mode of its own and sounds the same wherever it plays.

The tonic is A because of the cues. Those that play notes already play A and E: sector cleared (E5, A5), flag captured (E5, A5, E6), barrier unlocked (A5, E6) and run over (A4, E4, A3). A and E are degrees 1 and 5 of both modes, so these cues are in tune with the music whatever it plays. The key never changes, for the same reason.

## Material

Three things are written by hand: the motifs, the chords and the patterns.

A grid shows bars of 16 steps, in four beats. In a melody, a digit starts a note on that degree, a dot holds it and a dash is silence. In a pattern, `x` is a hit, `o` is a softer one, `R` is the root of the chord of the bar, `O` is that root an octave higher, `5` is its fifth, and a dot starts nothing.

### Motifs

```
Motif A, the question    1..2 ..3. 5..3 ..2. | 3..5 ..6. .... 5...
Motif B, the answer      8... ..9. 8... 6... | 5..6 ..5. 3..2 ..1.
Motif C, the call        ---- 5.8. ---- ----
```

Motif A climbs and stops on degree 5, so it asks for more. Motif B starts on the high tonic and comes down to the tonic, so it closes. The first bar of A and the second bar of B share one rhythm, twice 3 + 3 + 2 steps. It is the rhythm of the fights, and the bass plays it too.

Motif C belongs to the bridge. It is two notes in a row, the second a fourth above the first and left to ring. Over the eight bars of the bridge its second note is degree 8, 7, 8, 9, 8, 7, 8 and 4 in turn, so the last call falls where the others rise.

### Connection

The connection is the way out of a fight. It leads the tune from the note the lead is on to degree 5, the one on which the bridge stays, so the tune of a fight is never cut off: it arrives where the bridge begins. It goes by steps of the scale and never turns back on the way, in four notes at most. The bass and the pluck go with the lead, each in its own octave, and a lead that is silent leaves them the fall.

| The lead is on | Connection | What it is |
|---|---|---|
| 8 or 9 (the upper A, or the B above it) | `8.7. 6.5.` | The fall: A, G, F♯, E |
| 6 (F♯, or F in an arena) | `6... 5...` | One step down, a beat each |
| 5 (E) | `5... ....` | The lead is there already, and stays |
| 3 (C) | `3.4. 5...` | A rise: C, D, E |
| 1 or 2 (the lower A, or B) | `2.3. 4.5.` | A rise: B, C, D, E. From A it starts one step on, where four notes are left. |
| Nothing: the lead is silent | `8.7. 6.5.` | The fall, on the pluck and the bass |

It is in Dorian even after an arena, where its F♯ is the first sign that the fight is over. It replaces a fall that always started from the upper A: when the tune had just come down, the fall went back up to start, which sounded wrong.

### Chords

One chord per bar. C stands for the chord that gives the mode its color: the one on degree 7 in Dorian, and the one on degree 6 in Aeolian.

| Section | First phrase | Second phrase |
|---|---|---|
| Intro and verse | 1, 1, C, 4 | 1, 1, C, 5 |
| Chorus | 4, 5, 1, C | 4, 5, 1, 1 |

The bridge has no chords. Its bass stays on degree 5, the one from which the tonic of a fight is best heard coming.

### Patterns

```
Kick, bridge         x.o. .... .... ....    the two beats of a heart
Kick, fight          x... x... x... x...
Hi-hat, corridor     ..x. ..x. ..x. ..x.
Hi-hat, arena        xxxx xxxx xxxx xxxx    louder on the steps of the corridor pattern
Snare, arena         .... x... .... x...
Bass, long           R... .... .... ....
Bass, plain          R..R ..R. R..R ..R.
Bass, leaping        R..O ..R. R..O ..5.
Pluck, arpeggio      x.x. x.x. x.x. x.x.    root, third and fifth of the chord in turn, rising
```

Two lines of the bridge are written in degrees, like a melody:

```
Bass, bridge         ---- ---- 5-5. ....
Pluck, tick          5.9. 5.9. 5.9. 5.9.    quiet, an octave below the call, which takes its place on the second beat
```

Syncopation lives in the bass, the pluck and the lead. The drums stay on a plain grid on purpose: a regular pulse is easy to stop hearing, and a shot, which is irregular, stands out against it.

### Rules of derivation

| Rule | What it does | Used by |
|---|---|---|
| Mode | Reads each degree in the mode of the bar | Everything with a pitch |
| Skeleton | Keeps the notes of a motif that start on a beat and holds each until the next | Lead at threat 1 |
| Ornaments | A note of three steps or more gives its last two steps to the degree above it, among the five, and to itself again | Lead at threats 3 and 4 |
| Octave | Plays the same line an octave higher as well | Lead at threat 4 |
| Turnaround | Swaps the long degree 6 of motif A for degree 2, a note of chord 5: its second bar becomes `3..5 ..2. .... 5...` | Last bar of the intro and of the verse, which is on chord 5 |
| Answer | Plays the two notes of a call in the other order, two beats later and short | Lead in the bridge |
| Connection | Goes by steps of the scale from the note the lead is on to degree 5, without turning back | Lead, pluck and bass at the end of a fight |

One check guards every change to the notes. In both modes, a note that lies a semitone above the root of its chord must pass quickly: it lasts one step if it starts on a beat, and three steps at most otherwise. The turnaround exists because of it: without it, the last bar of the verse would hold F over E minor in an arena.

## Sections

| Section | Bars | Melody | Feel |
|---|---|---|---|
| Intro | 8 | Motif A four times on the pluck, the last time with the turnaround | Still, without drums |
| Verse | 8 | Motif A four times on the lead, the last time with the turnaround | Driving |
| Chorus | 8 | Motif B four times on the lead | Driving, and higher |
| Bridge | 8 | Motif C in every bar on the pluck, answered by the lead | Slower: a break |

The piece has no fixed order of sections. The game chooses them through the context, and each section loops for as long as its context lasts. The bridge is therefore not on a timer: it is the music of the calm between two fights.

A bar of the bridge is one idea, played four times: two hits in a row. The kick has them on the first beat, as the two beats of a heart. The pluck has them on the second, as its call. The bass has them on the third, on its one degree. The lead has them on the fourth, as its answer to the call. Under them the pluck ticks on every half beat, rocking between that degree and the fifth above it, so the bar is never empty. It replaces two earlier bridges: one that had the second motif at half speed over long chords on the lead and sounded like any other, and one at 90 beats per minute without the tick and the answer, which was too slow and too empty.

## Context

The context is all that the music knows about the game. The client writes it every frame from what it already reads from the simulation.

| Part | Values | Meaning |
|---|---|---|
| Place | Menu, bay, corridor, arena | The sector the run has reached. The main menu and the game over screen count as the menu, and so does a run from the moment it is over. A switchback is a corridor. |
| Fight | On or off | On while at least one enemy is awake |
| Threat | 0 to 4 | The highest brain tier among the enemies that are awake, and 0 when none is |
| Down | On or off | In solo, the player's tank is destroyed and on its way back |
| Muffled | On or off | The pause menu is open over the game. The settings leave the music clear, so that its volume is set by ear. |

In multiplayer the context is the team's, so every player hears the same arrangement, and a fight starts for everyone when the first player crosses. A wrecked player hears the music muffled until repaired.

## Arrangement

The drums and the mode tell where the players are and whether they fight. The lead tells whom they fight.

| Context | Section | Mode | Kick | Hi-hat | Snare | Bass | Pluck | Lead |
|---|---|---|---|---|---|---|---|---|
| Menu or bay | Intro | Dorian | Silent | Silent | Silent | Long | Motif A | Silent |
| No fight, in a corridor or an arena | Bridge | None | Heart | Silent | Silent | Two notes | Motif C over its tick | Answer |
| Fight in a corridor | Verse and chorus in turn | Dorian | Fight | Corridor | Silent | By threat | By threat | By threat |
| Fight in an arena | Verse and chorus in turn | Aeolian | Fight | Arena | Arena | By threat | By threat | By threat |

The mode is Aeolian only while the enemies of an arena are alive. The moment the last one is destroyed, the music turns back to Dorian, which makes clearing an arena heard as a relief.

During a fight, the threat sets the three instruments that have a pitch:

| Threat | Brain | Lead | Bass | Pluck |
|---|---|---|---|---|
| 0 | Sentry | Silent | Plain | Arpeggio |
| 1 | Grunt | Skeleton of the motif | Plain | Arpeggio |
| 2 | Soldier | The motif | Leaping | Arpeggio |
| 3 | Veteran | The motif with ornaments | Leaping | Arpeggio |
| 4 | Ace | The motif with ornaments, in octaves | Leaping | Arpeggio, twice as fast |

The threat falls during a fight when the smartest enemies are destroyed first, and the lead then thins out. At the start of a fight it is the highest tier the sector allows about three times out of four, and half the time in the first two sectors, which have one enemy each. Over a run it therefore mostly rises with the sector number: the lead comes in during the first sectors, plays the whole motif from sector 6 and reaches its fullest form from sector 16.

To know what to listen for on a seed, a test lists the brains of the first thirty groups and the threat that each fight starts with:

```bash
TANKU_SEED=TANKU cargo test -p tanku_client threats_of_a_seed -- --ignored --nocapture
```

## Changes

Three rules make the music answer fast and stay in time:

1. A change within a fight waits for the bar line.
2. A fight has a way in and a way out, which last two beats each. The roll starts at once. The connection starts on the next beat of the fight, so that the kick never stumbles.
3. What follows a roll or a connection starts on its own first beat, wherever the bar was. The bridge has a tempo of its own, so nothing has to line up.

| Event | At once | Then |
|---|---|---|
| A fight starts | A roll of two beats that grows louder, on the hi-hat in a corridor and on the snare in an arena. Nothing else starts while it lasts. | When the roll is over, one second later: the fight sections start, in Aeolian in an arena |
| The fight ends | The fight plays on to its next beat, half a second away at most. Then the [connection](#connection): in two beats that slow down to the tempo of the bridge, the bass, the pluck and the lead, if it was playing, go together to degree 5. The kick keeps the beat under them and the other drums stop. | When the connection is over: the bridge starts on its first beat |
| The threat changes | Nothing | On the next bar line: lead, bass and pluck change |
| The player's tank is destroyed | Everything stops but the bass, which holds the root of the chord | On the bar line after the tank is back: the context decides again |
| The run ends | The same | On the next bar line: the intro starts. The game over screen comes 1.4 s after the end, so the intro is there before it or soon after. |
| A run starts | Nothing: the intro goes on | The count of phrases starts over |
| The pause menu opens over the game | Within 0.2 s the music loses what is above 600 Hz and half its loudness. It gets them back when the menu closes, or when the settings open from it. | Nothing |

The fight sections are counted in phrases: the two of the verse, then the two of the chorus, then the verse again. A fight opens on the phrase after the one where the last fight ended. The first fight of a run opens the verse, so every run begins the same way, and the short fights of the first sectors walk through the whole piece instead of repeating its first bars.

A roll is heard within one step of its event, and the fight is in full swing one second later. A connection starts within half a second of the end of a fight, and the bridge one second after that: the sound cue of a cleared sector is what answers at once. Every other change is complete within one bar.

## Instruments

Each instrument is a small synthesizer made of plain shapes, like the drawing: square, pulse and sine waves, noise, and one plucked string.

| Instrument | Sound | Notes at once | Range | Place in stereo |
|---|---|---|---|---|
| Bass | A square wave through a filter that opens at each attack and closes in 0.15 s | 1 | E2 to D4 | Middle |
| Pluck | A plucked string: a burst of noise that goes round a tuned delay and wears off | 4 | E3 to A5 | Left |
| Lead | A pulse wave with a slow vibrato | 2 | A4 to B6 | Right |
| Kick | A sine that falls from 110 Hz to 55 Hz in 0.12 s, without any noise | 1 | None | Middle |
| Hi-hat | Noise above 7 kHz for 0.03 s | 1 | None | Left |
| Snare | Noise around 2 kHz with a tone at 220 Hz, for 0.12 s | 1 | None | Middle |

The kick has no noise in it because every weapon cue has some. The bass stays above 80 Hz, which leaves the bottom to the kick and to the blasts and keeps it audible on small loudspeakers.

## Mix

- **Two volumes.** The settings have a second level, Music, next to Volume. Volume sets everything and Music sets the music against the cues. It starts at 50%. At 0 the music keeps counting its bars and stays silent.
- **Under the cues.** At the default settings, the loudest arrangement peaks 12 dB below a shot fired next to the player. With Music all the way up it peaks 6 dB below that shot.
- **Melody on top.** The melody is the loudest thing with a pitch. The arpeggio of the pluck and the octave that the lead adds at threat 4 are quieter than the line they go with.
- **Ducking.** While the warning of a mine sounds within hearing range, the music is half as loud.
- **Stereo.** The music is in stereo and the cues stay in the middle. The pluck and the lead, which share the pitches of the cues, sit to the sides.
- **Dry.** There is no reverb. The arpeggio of the pluck has one echo, three steps later, a third as loud and on the other side. Its melodies have none: with an echo, the intro sounded too relaxed.
- **No clipping.** The sum of the instruments goes through a soft limit.

## Cues

The waveforms of the cues are in `tanku_audio`, next to the instruments, so that everything the game computes as sound is in one place and is made with the same math. Playing them stays in the client, which knows where a sound comes from and how loud it is. The move left them as they were: no sample of a cue is further from what it was than eight steps of a sound file of 16 bits, and two steps for all but the warning of a mine.

A cue is in mono, at 22,050 samples per second. The client computes each one when it starts and keeps it as a sound file in memory.

A cue that plays a note plays A or E, taken from the table of pitches of the music, so it is in tune with whatever the music plays. Two cues were out of tune with music in A and have moved, and one stays between two notes on purpose:

| Cue | Was | Is |
|---|---|---|
| Mine placed | 620 Hz, six semitones from A | E5 |
| Shield blocked | 900 Hz and 1350 Hz, four tenths of a semitone above A5 and E6 | A5 and E6 |
| Mine triggered | 1500 Hz, a quarter of a semitone above F♯6 | Unchanged: a warning should stand apart from the music |

The four cues that played A and E already had their E a pure fifth above A, at 660 Hz. They now take the E of the music, which is 2 hundredths of a semitone lower: no ear tells the two apart.

## Same everywhere

| What | Depends on | Guarantee |
|---|---|---|
| The piece: notes, patterns, sounds | Version | Identical on every platform |
| The arrangement at a given moment | Version and the context since the game started | Identical on every platform |
| The cues | Version | Identical on every platform |
| What reaches the ears | The above and the sound device | Not guaranteed: devices differ in rate and delay |

- **Nothing is random.** The noise of the hi-hat, the snare and the pluck comes from a generator of its own that always starts from the same state, and so does the noise of the cues.
- **The score is integers.** What each instrument plays on a step is a function of the step number and of the context. Steps are counted in half frames, at 44,100 frames per second: one lasts 11,025 half frames at 120 beats per minute and 12,600 in the bridge. Nothing is rounded along the way, so nothing drifts.
- **The crate does its own math.** The synthesizer and the cues add, subtract, multiply and divide 32-bit floats, and take sines and decays from a few lines of their own. They never call a library function such as `sin` or `exp`, whose last bit differs between platforms. Their output is therefore the same bit for bit on every platform, which lets a test compare it with a stored value.

## Engine

```
tanku_audio
├── score       the motifs, the chords, the patterns and the rules of derivation
├── director    from the context to what each instrument plays on each step
├── synth       the six instruments, the echo and the mix
├── cues        the waveforms of the sound cues
├── math        the sines, the decays and the noise that every sound is made with
└── wav         writes a sound file, for the game, the listening tool and the tests
```

The crate has two ends. The game holds a `Conductor` and writes the context into it. The sound thread holds the `Music` and asks it for frames:

```rust
let conductor = Conductor::default();
// In the client, every frame:
conductor.set(Context { place: Place::Corridor, fight: true, threat: 2, ..Context::default() });
// On the sound thread, once the sound library is ready to play:
let mut music = conductor.music();
// And whenever the device wants more:
music.render(&mut frames);
```

| Topic | Requirement |
|---|---|
| Output | Stereo, 44,100 frames per second, 32-bit floats. It is the rate the sound library opens by default on Linux and on the web, so nothing resamples the music there. |
| Dependencies | None, not even Bevy |
| Real time | `render` never allocates, locks or waits. The context crosses between the threads as one atomic number, read at the start of every step. |
| Cost | Under 10 ms of one core for one second of the fullest arrangement. It takes 1.3 ms on the development machine. |
| Development builds | The crate is compiled with full optimization, like the dependencies, because it runs in real time |
| Start | Nothing is computed ahead, so the music adds nothing to the time the game takes to start or to restart |

In the client, the music is an asset that implements Bevy's `Decodable`, which Bevy mixes with the cues. It is played once and has no end. A system writes the context every frame from `Screen`, `Overlay`, `Run`, the `Sector` the run has reached and the genomes of the enemies that are awake. Another keeps the music as loud as the settings say, because Bevy counts the volume of the game into a sound only when the sound starts. Nothing changes in the server or in the simulation, and the headless server has no music.

A scripted run plays no music either, but it can keep what it would have played: from its command `tape` on, it computes the music frame by frame and writes it to a sound file when it quits, next to the list of what the music was told and when. It is how the music is checked against a real run without anyone listening.

The web build has no music until step 5. There, the browser lets no sound play before the first click or key, so the music will start with it. The web build also mixes its sound on the main thread, 46 ms at a time: a frame that takes longer than about 50 ms leaves a gap in the music. The cues have the same limit today, but a gap in a held note is heard far more than one in a short cue.

A tool renders the music without the game:

```bash
cargo run -p tanku_audio --example render
```

It writes one sound file per arrangement, a tour through a made-up run with the list of what happens in it and when, and a file of short fights that each end with the lead on another note, to hear every connection. From step 4 it also lays cues over the music.

## Tests and benchmarks

- **Stable.** Sixteen bars of each arrangement match a stored value, as the map of a seed does, and so does each cue.
- **In tune.** Every instrument plays the lowest and the highest note of its range within 3 hundredths of a semitone. The cues that play notes play A and E and nothing else, and the warning of a mine is between two notes.
- **Never silent.** Every bar of every arrangement makes sound.
- **Never too loud.** No frame of any arrangement or of any change goes past full scale, and none is NaN.
- **Seamless.** The third pass of a loop is identical to the second.
- **On time.** A fight that is announced on each of the 16 steps of a bar gives a roll at once and the fight two beats later. A fight that ends on each of them plays on to its next beat, and gives a connection there and the bridge two beats later.
- **Connected.** For every note the lead can be on, the connection goes to degree 5 one step of the scale at a time and never turns back.
- **Sound in both modes.** The motifs use the five degrees, the chords are major or minor, and every melody, with its skeleton and its ornaments, passes the check on semitones above the root.
- **Whole.** Each cue lasts as long as it says, never goes past full scale and ends without a click.
- **Cost.** A benchmark renders ten seconds of a fight in an arena at threat 4.

The client has its own:

- **Told right.** The context follows the screen, the run, the kind of its sector and the enemies that are awake. The smartest of them sets the threat, which falls when it is destroyed, and those that sleep don't count. A run that is over counts as the menu, and only the pause menu muffles.
- **Unchanged on the way.** The sound library reads the very samples that the crate renders, and reads every cue back from its sound file to a step of 16 bits.
- **Under the cues.** At the default settings, the loudest arrangement peaks 12 dB below the shot of a player, within half a decibel.
- **Heard.** On request, a test plays the piece on the sound device of the machine at no volume, and checks that the device asks for more: `cargo test -p tanku_client reaches_the_sound_device -- --ignored`.

## Steps

Each step answers a question, and the first one changes nothing in the game.

| Step | Content | Question it answers |
|---|---|---|
| 1, Sketch | The crate with the score, rough instruments and the listening tool. Sound files of the intro, of the bridge, of a fight in a corridor and in an arena at each threat, and of a tour of three minutes. | Are the two motifs worth keeping? Does the minor mode suit the arenas? |
| 2, In the game | The music plays in the client and follows the place and the fight. The Music volume. A switch in the playtest menu between music that follows the place only and music that follows the fight too. | Does the music feel tied to what happens? Can a shot off screen still be told from the beat? |
| 3, Threat | Lead, bass and pluck follow the brain tier | Can a player tell the smarter of two groups by ear? |
| 4, Cues | The waveforms of the cues move to the crate, two cues are retuned, and the warning of a mine ducks the music | Is any cue lost under the music? |
| 5, Web | The web build with music | Does the music play without gaps in a browser? |

The switch of step 2 was an experiment, to settle by ear whether the music needs to know of the fights, and it is gone. On its other side the place alone set the arrangement, as in the first proposal for this music: the verse and the chorus played wherever the players were outside the bay, with the kick alone in a corridor and all three drums in an arena, and nothing called for the bridge. After a playtest, the music that follows the fight too was kept. [IDEAS.md](IDEAS.md) keeps the other one.

## Open questions

- How long are the frames of the web build when a sector loads, and is the gap they leave in the music acceptable?
- Two machines in one room play the same piece out of step, because each has its own clock. Does co-op in one room need them in step?
- In hard mode every tier is available from sector 1, so the lead is at its fullest almost at once. Is that the right sound for hard mode, or does it waste the build-up?
