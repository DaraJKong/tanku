# Tanku: music specification

Status: the six [steps](#steps) are built. The sketch of step 1 was listened to four times, and its notes are accepted. With step 2 the game plays the music on the desktop, where it follows the place and the fight and has a volume of its own. A playtest answered its questions: the music that follows the fight too was kept over music that follows the place only, and a shot sounds different from the beat, so one is told from the other. Steps 3 to 5 have not been judged by ear yet. With step 3 the lead, the bass and the pluck follow the brains of the enemies that are awake. With step 4 the waveforms of the cues are in the crate, two cues are tuned to the music, and the warning of a mine ducks it. A playtest never heard that warning, which only a tank next to a mine could set off: the last 0.6 s of a fuse now sound it too. With step 5 the web build has its sound, which a browser had never let it start, and the music with it, handed over ahead of time so that a long frame leaves no hole in it. With step 6 the piece is written in a file, [`music.score`](#the-file), and plays as it did, to the sample. Whether its notation is enough to change the piece by hand has not been tried yet.

The music of Tanku is one piece that never stops. The game makes it while it plays, from three short motifs and six instruments that it computes itself, so no sound file ships. The piece is the same for every seed and on every platform. What changes is its arrangement, which follows the run: where the players are, whether they are fighting, and how smart the enemies in front of them are.

All numbers and all notes in this document are starting values to tune by ear.

## Overview

| Topic | Summary |
|---|---|
| Piece | One, in A, at 120 beats per minute and at 105 in its bridge, without an end |
| Written by hand | Three motifs, the chords and the melody of each section, twelve patterns with their levels, the arrangement, the tempos, the key and its two modes, the roll, the connection and the mix. They are in one file, [`music.score`](#the-file). |
| Derived by rule | Everything else: the notes of each mode, the skeleton, the ornaments and the answers of a melody, every connection, every sound |
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
| Mark of a tank that is about to appear in an arena | 1 s | Two beats |
| Respawn | 1.5 s | Three beats |
| Wait before the first wave of an arena | 2 s | One bar |
| Repair | 2 s | One bar |
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

The tonic is A because of the cues. Those that play notes already play A and E: sector cleared (E5, A5), life taken (E5, A5, E6), barrier unlocked (A5, E6) and run over (A4, E4, A3). A and E are degrees 1 and 5 of both modes, so these cues are in tune with the music whatever it plays. The key never changes, for the same reason.

## Material

Five things are written by hand: the motifs, the sections with their chords, the patterns, the arrangement, and a few numbers, which are the tempos, the key, the roll, the connection and the mix. They are in one file.

### The file

The piece is written in `tanku_audio/music.score`. The crate reads the file while it is compiled, so the game carries the piece in itself and opens no file when it starts. A file that can't be read stops the build, and the error says on which line and why.

```
tanku-music 1

tempo 120
key A dorian aeolian

motif A 1..2 ..3. 5..3 ..2. | 3..5 ..6. .... 5...
motif B 8... ..9. 8... 6... | 5..6 ..5. 3..2 ..1.
motif C ---- 5.8. ---- ----
turn  A 3..5 ..2. .... 5...

pat kick  heart    x.o. .... .... .... 180 135
pat kick  fight    x... x... x... x... 255
pat hat   corridor ..x. ..x. ..x. ..x. 190
pat hat   arena    ooxo ooxo ooxo ooxo 200 100
pat snare arena    .... x... .... x... 230
pat bass  long     R... .... .... .... 255
pat bass  plain    R..R ..R. R..R ..R. 255
pat bass  leaping  R..O ..R. R..O ..V. 255
pat bass  pair     ---- ---- 5-5. .... 255
pat pluck arpeggio x.x. x.x. x.x. x.x. 150 echo
pat pluck fast     xxxx xxxx xxxx xxxx 150 echo
pat pluck tick     5.9. 5.9. 5.9. 5.9. 110

sec intro  11C4 11C5  A A A a
sec verse  11C4 11C5  A A A a
sec chorus 451C 4511  B B B B
sec bridge 5555 5555  C C:7 C C:9 C C:7 C C:4  105

play menu     intro         bass=long@2 pluck=melody@4
play calm     bridge        kick=heart bass=pair@1 pluck=melody@3/tick@2 lead=answer@4:150
play corridor verse,chorus  kick=fight hat=corridor bass=* pluck=* lead=*
play arena    verse,chorus  kick=fight hat=arena snare=arena bass=* pluck=* lead=*

threat 0  bass=plain@2    pluck=arpeggio@3
threat 1  bass=plain@2    pluck=arpeggio@3  lead=skeleton@4
threat 2  bass=leaping@2  pluck=arpeggio@3  lead=melody@4
threat 3  bass=leaping@2  pluck=arpeggio@3  lead=ornate@4
threat 4  bass=leaping@2  pluck=fast@3      lead=ornate@4+ornate@5:130

roll 2 hat snare 40 255
link 2 5 4

mix kick  32% 32%
mix hat   75% 45%
mix snare 28% 28%
mix bass  10% 10%
mix pluck 20% 10%
mix lead   5%  8%

sound kick  pitch 110Hz 55Hz 30ms  decay 45ms
sound hat   above 7000Hz  decay 8ms
sound snare band 2000Hz 1600Hz  decay 35ms  tone 220Hz 30ms 70%
sound bass  rise 3ms  filter 1400Hz 300Hz 50ms
sound pluck ring 1000ms  smooth 3  echo 3 33%
sound lead  width 30%  rise 4ms  below 3000Hz  vibrato 136ms 0.4% 5.5Hz
```

The file is text. It has one record per line, with its words apart by spaces. An empty line is ignored, and so is a line that starts with `#`.

| Line | What it says |
|---|---|
| `tanku-music 1` | The format of the file and its version. It comes before every other record. |
| `tempo` | The tempo of the piece in beats per minute, from 30 to 300 |
| `key` | The tonic, then the mode of the piece and the mode of the fights of an arena, each by its name: one of the seven modes below |
| `motif` | A capital letter, which names the motif, then its bars, one to four of them, with a `\|` between two bars |
| `turn` | The letter of a motif, then the bar that this motif ends a section with, in place of its last one |
| `pat` | A pattern: its instrument, its name, one bar, and how loud a hit is, up to 255. A pattern with soft hits also says how loud those are. The instrument is `kick`, `hat`, `snare`, `bass` or `pluck`: the lead has no pattern. A pattern of the pluck that ends with `echo` has the echo of the pluck on its notes. |
| `sec` | A section: its name, its chords, its melody, and its tempo if it has one of its own |
| `play` | A line of the [arrangement](#arrangement): a scene, the sections that it plays in turn, and what each instrument plays in them |
| `threat` | A threat, from 0 to 4, and what the instruments play at it |
| `roll` | The roll that announces a fight: its beats, the drum that rolls, the drum that rolls in an arena, and how loud it is at its start and at its end |
| `link` | The [connection](#connection) out of a fight: its beats, the degree that it goes to, and how many notes it has at most |
| `mix` | An instrument, then how loud it is on the left and on the right, in percent of the loudest that a sound can be. Every instrument has its line. |
| `sound` | An instrument, then the settings of its sound, each by its name, in ms, in Hz and in percent. [Instruments](#instruments) lists them. Every instrument has its line. |

The chords of a section are two words of four, one chord per bar. A chord is the degree that it is built on, or `C` for the chord that gives the mode its color. The melody of a section is eight bars of motifs, one word for each: `A` is motif A as it is written, `a` is motif A with its turn, and `C:7` is motif C with its last note on degree 7.

The key names two of the seven modes of the major scale. Each has its color chord, which is what `C` stands for in the chords of a section:

| Mode | Its scale in A | Its color chord |
|---|---|---|
| `ionian` | A, B, C♯, D, E, F♯, G♯ | F♯ minor, on degree 6 |
| `dorian` | A, B, C, D, E, F♯, G | G, on degree 7 |
| `phrygian` | A, B♭, C, D, E, F, G | B♭, on degree 2 |
| `lydian` | A, B, C♯, D♯, E, F♯, G♯ | B, on degree 2 |
| `mixolydian` | A, B, C♯, D, E, F♯, G | G, on degree 7 |
| `aeolian` | A, B, C, D, E, F, G | F, on degree 6 |
| `locrian` | A, B♭, C, D, E♭, F, G | B♭, on degree 2 |

The piece is in Dorian, and in Aeolian in the fights of an arena: [Pitch](#pitch) says why. In three of the other modes, one of the chords on 1, 4 and 5 is neither major nor minor: the chord on 5 in Phrygian, the chord on 4 in Lydian and the chord on 1 in Locrian. The checks name a section that has such a chord.

A bar has 16 steps in four beats. It is written beat by beat, as four words of four steps:

| Symbol | In a melody | In a pattern |
|---|---|---|
| A digit | Starts a note on that degree | The same, on the bass and the pluck |
| `.` | Holds the note | Starts nothing, and holds a note that has a pitch |
| `-` | Silence | Ends a note that has a pitch |
| `x` | | A hit. On the bass and the pluck, it is the next note of the chord of the bar: its root, its third and its fifth in turn, rising. |
| `o` | | The same, softer |
| `R`, `O`, `V` | | The root of the chord of the bar, that root an octave higher, and its fifth |

The arrangement has a `play` line for each of the four scenes that it tells apart, and a `threat` line for each threat:

| Scene | When |
|---|---|
| `menu` | No run is on, or the run is in its start bay |
| `calm` | Nobody fights, in a corridor or in an arena |
| `corridor` | A fight in a corridor |
| `arena` | A fight in an arena, which is in the second mode of the key |

After its scene, a `play` line has the sections that play in turn, with a comma between two of them, then what the instruments play. An instrument is the `kick`, the `hat`, the `snare`, the `bass`, the `pluck` or the `lead`, and one that the line doesn't name is silent:

| Written | What the instrument plays |
|---|---|
| `kick=heart` | A drum plays one of its patterns, by its name |
| `bass=long@2` | The bass and the pluck play one of their patterns in an octave, from 1 to 5: the octave of the tonic above which the degrees of the pattern are counted. The pluck builds the chord of the bar above that tonic, and the bass takes the root that is nearest to it. In A, octave 1 starts at the A of 55 Hz, and octave 4 at the A of 440 Hz. |
| `pluck=melody@4` | The bass, the pluck and the lead play the melody of the section, in an octave: `melody` as it is written, `skeleton` for its notes that start on a beat, `ornate` with its ornaments, or `answer` for the answer to each of its bars |
| `lead=answer@4:150` | A melody has a level after its octave, up to 255, when it isn't as loud as can be |
| `pluck=melody@3/tick@2` | A line, and another line that sounds where the first one starts nothing |
| `lead=ornate@4+ornate@5:130` | Two lines together, which only the lead can play. The second one starts a note where the first one does. |
| `bass=*` | The instrument plays what the line of the threat says. A scene without a fight takes the line of threat 0. |

A `threat` line is written in the same way, without `*`. It only names instruments that a scene leaves to the threat.

The file has the settings of each [instrument](#instruments), and not how the instrument is made: that is in the code. So are the rules: how a line is derived from a melody, and how the music follows the game from one scene to the next.

After a change of the file, two commands tell what it did:

```bash
cargo run -p tanku_audio --example render
cargo test -p tanku_audio --test checks
```

The first writes the piece to sound files, which [Engine](#engine) lists. The second runs the checks that hold for any piece: the rules of this document that a change of a note, of a pattern or of a level could break. The other tests say what the piece is today, to the note and to the sample, so a change that is heard fails them until they are brought up to date.

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

The bridge has no chords. Its bass stays on degree 5, the one from which the tonic of a fight is best heard coming. The file writes that degree where the other sections have their chords.

### Patterns

```
Kick, heart          x.o. .... .... ....    the two beats of a heart, in the bridge
Kick, fight          x... x... x... x...
Hi-hat, corridor     ..x. ..x. ..x. ..x.
Hi-hat, arena        ooxo ooxo ooxo ooxo    every step, and louder on the steps of the corridor pattern
Snare, arena         .... x... .... x...
Bass, long           R... .... .... ....
Bass, plain          R..R ..R. R..R ..R.
Bass, leaping        R..O ..R. R..O ..V.
Bass, pair           ---- ---- 5-5. ....    two notes in a row, in the bridge
Pluck, arpeggio      x.x. x.x. x.x. x.x.    root, third and fifth of the chord in turn, rising
Pluck, fast          xxxx xxxx xxxx xxxx    the same, twice as fast
Pluck, tick          5.9. 5.9. 5.9. 5.9.    quiet, an octave below the call, which takes its place on the second beat
```

Two patterns of the bridge are written in degrees, like a melody: the pair of the bass and the tick of the pluck.

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
| Ducked | On or off | The warning of a mine sounds within hearing range |

In multiplayer the context is the team's, so every player hears the same arrangement, and a fight starts for everyone when the first player crosses. A wrecked player hears the music muffled until repaired.

## Arrangement

The drums and the mode tell where the players are and whether they fight. The lead tells whom they fight. The two tables below are the `play` lines and the `threat` lines of [the file](#the-file).

| Context | Section | Mode | Kick | Hi-hat | Snare | Bass | Pluck | Lead |
|---|---|---|---|---|---|---|---|---|
| Menu or bay | Intro | Dorian | Silent | Silent | Silent | Long | Motif A | Silent |
| No fight, in a corridor or an arena | Bridge | None | Heart | Silent | Silent | Pair | Motif C over its tick | Answer |
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
| 4 | Ace | The motif with ornaments, in octaves | Leaping | Fast: the arpeggio, twice as fast |

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
| The warning of a mine sounds within hearing range | Within 0.02 s the music is half as loud, without waiting for a step | When the warning is over, 0.6 s later: the music comes back in 0.2 s |

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

How an instrument is made is in the code, and it is the same for every piece. Its settings are in the `sound` lines of [the file](#the-file). A number is written right before its unit, as in `45ms`, and can have a point, as in `5.5Hz`. The settings of an instrument come in any order, and each of them is there once.

| Line | Setting | What it says | Bounds of its own |
|---|---|---|---|
| `sound kick` | `pitch 110Hz 55Hz 30ms` | The frequency that the sine starts at, the one that it goes to, and the time of the way | |
| | `decay 45ms` | The time that it dies away in | |
| `sound hat` | `above 7000Hz` | The frequency above which the noise is kept | |
| | `decay 8ms` | The time that it dies away in | |
| `sound snare` | `band 2000Hz 1600Hz` | The frequency that the noise is around, and how wide that band is | Around 5000 Hz at most. As wide as a tenth of that frequency at least, and as twice that frequency at most. |
| | `decay 35ms` | The time that the noise dies away in | |
| | `tone 220Hz 30ms 70%` | The tone under the noise: its frequency, the time that it dies away in, and how loud it is against the noise | |
| `sound bass` | `rise 3ms` | The time that a note takes to rise | |
| | `filter 1400Hz 300Hz 50ms` | The filter lets through what is below a frequency: where that frequency starts at each attack, where it goes, and the time of the way | |
| `sound pluck` | `ring 1000ms` | The time that a string takes to fall by 60 dB | 100 ms to 10 s |
| | `smooth 3` | How many times the burst of noise that plucks a string is smoothed. More of it makes a softer attack. | 0 to 8 |
| | `echo 3 33%` | How many steps after a note its echo comes, at the tempo of the piece, and how loud the echo is against the note | 1 to 8 steps |
| `sound lead` | `width 30%` | The part of its period that the wave spends up. At 50% it is a square wave. | 1% to 99% |
| | `rise 4ms` | The time that a note takes to rise | |
| | `below 3000Hz` | The frequency below which the lead is heard | |
| | `vibrato 136ms 0.4% 5.5Hz` | The time after which the vibrato comes in, how far it bends the pitch, and how many times per second. It is whole after as long again. | Up to 10%, and from 0.1 Hz to 20 Hz |

Where the table gives no bounds, a time is from 0.1 ms to 10 s, a frequency from 20 Hz to 20 kHz, and a level from 0% to 100%. A pitch and a filter can go either way: up or down.

The times are of two kinds. What dies away, and a pitch or a filter on its way, loses about two thirds of what is left in the time that is written, and as much again in the next: the kick, which dies away in 45 ms, is at a hundredth of its level after 0.2 s. A string rings for the time in which it falls by 60 dB, which is to a thousandth of its level.

The bounds are there so that a setting can only sound bad. Within them the music never goes out of range and is never NaN, whatever the six lines say.

Three things stay in the code, because they keep the music the same everywhere and free of clicks: how many notes an instrument plays at once, the noise that its sound starts from, and the few milliseconds in which a note makes room for the next one.

## Mix

- **Two volumes.** The settings have a second level, Music, next to Volume. Volume sets everything and Music sets the music against the cues. It starts at 50%. At 0 the music keeps counting its bars and stays silent.
- **Under the cues.** At the default settings, the loudest arrangement peaks 12 dB below a shot fired next to the player. With Music all the way up it peaks 6 dB below that shot.
- **Melody on top.** The melody is the loudest thing with a pitch. The arpeggio of the pluck and the octave that the lead adds at threat 4 are quieter than the line they go with.
- **Ducking.** A mine warns for 0.6 s before it blows, unless a bullet or a blast sets it off. While that warning sounds within hearing range, the music is half as loud. It gets there in 0.02 s, in time for the first beep, and takes 0.2 s to come back once the warning is over. Hearing range is 50 m from the middle of the screen. No other cue ducks the music.
- **Stereo.** The music is in stereo and the cues stay in the middle. The pluck and the lead, which share the pitches of the cues, sit to the sides. How loud each instrument is on each side is in the `mix` lines of [the file](#the-file).
- **Dry.** There is no reverb. The arpeggio of the pluck has one echo, three steps later, a third as loud and with its sides the other way round: the pluck is louder on the left, and its echo on the right. Its melodies have none: with an echo, the intro sounded too relaxed.
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
- **The score is integers.** What each instrument plays on a step is a function of the step number and of the context. Steps are counted in half frames, at 44,100 frames per second: one lasts 11,025 half frames at 120 beats per minute and 12,600 in the bridge. Nothing is rounded along the way, so nothing drifts. At a tempo whose step is not a whole number of half frames, the step is rounded to the nearest one, once.
- **The crate does its own math.** The synthesizer and the cues add, subtract, multiply and divide 32-bit floats, and take sines and decays from a few lines of their own. They never call a library function such as `sin` or `exp`, whose last bit differs between platforms. Their output is therefore the same bit for bit on every platform, which lets a test compare it with a stored value. The web build gives the same stored values, and the first piece of music that its page hands to the browser is the native one to the bit.

## Engine

```
tanku_audio
├── music.score the piece as it is written by hand
├── notation    what a line of that file means, and the reader of the file
├── score       the piece that the game plays, from that file, and the rules of derivation
├── director    from the context to what each instrument plays on each step
├── synth       the six instruments, the echo and the mix
├── cues        the waveforms of the sound cues
├── checks      the checks that hold for any piece, for the tests and for the editor
├── math        the sines, the decays and the noise that every sound is made with
└── wav         writes a sound file, for the game, the listening tool and the tests
```

[The file](#the-file) is read while the crate is compiled, by a reader that is a plain function: a program can call it too, to read a piece while it runs.

A program that reads a piece of its own hands it to the engine. `music_of` plays it, and `Music::rescore` gives a music another piece to play from its next step on, where it is and without starting over. The [editor](#editor) does both.

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
| Real time | `render` never allocates, locks or waits. The context crosses between the threads as one atomic number, read at the start of every step, and about 700 times a second for the ducking, which can't wait for a step. |
| Cost | Under 10 ms of one core for one second of the fullest arrangement. It takes 1.3 ms on the development machine. |
| Development builds | The crate is compiled with full optimization, like the dependencies, because it runs in real time |
| Start | Nothing is computed ahead and no file is opened, so the music adds nothing to the time the game takes to start or to restart |

On a desktop, the music is an asset that implements Bevy's `Decodable`, which Bevy mixes with the cues. It is played once and has no end. A system writes the context every frame from `Screen`, `Overlay`, `Run`, the `Sector` the run has reached and the genomes of the enemies that are awake. Another keeps the music as loud as the settings say, because Bevy counts the volume of the game into a sound only when the sound starts. Nothing changes in the server or in the simulation, and the headless server has no music.

A scripted run plays no music either, but it can keep what it would have played: from its command `tape` on, it computes the music frame by frame and writes it to a sound file when it quits, next to the list of what the music was told and when. It is how the music is checked against a real run without anyone listening.

The web build plays the music in a way of its own, which [In a browser](#in-a-browser) describes.

A tool renders the music without the game:

```bash
cargo run -p tanku_audio --example render
```

It writes one sound file per arrangement, a tour through a made-up run with the list of what happens in it and when, and a file of short fights that each end with the lead on another note, to hear every connection. It also writes a file of the cues: each one alone, then over the fullest music, next to the player and as far away as a cue gets, all as loud as the game plays them with the settings it starts with.

## In a browser

The web build plays the same music, sample for sample. A browser sets three conditions of its own.

**No sound before the first click or key.** The sound library of the engine asks the browser for its sound when the game starts, which is too early, and never asks again: until step 5 the web build was silent, cues included. The page of the game now asks again each time the player clicks or presses a key. The music therefore starts with the first click, from its first bar.

**Sound is handed over between two frames.** A page gives the browser its sound in pieces of 46 ms, and can only do so while it isn't busy. The sound library keeps one piece in hand: a frame that takes longer than about 40 ms leaves a hole in the sound, and a much longer one garbles it for about as long as the frame lasted. The cues stay with that library, because they are short and must not be late.

The music has a source of sound of its own, which the client feeds every frame, a few pieces ahead:

| When | What the music does |
|---|---|
| At first | It is handed over 0.1 s ahead |
| After each hole | It reaches 0.1 s further, up to 0.4 s |
| A frame takes longer than its reach | A hole, as long as what was missing, and the music goes on from where it was |
| The page was hidden | Nothing: sound that ran out then says nothing about the machine |

The price is that the music hears of the game that much later: a roll starts a tenth of a second later than on a desktop, and up to four tenths on a machine whose frames are long. The ducking can't wait that long, so a browser turns down the loudness of the music's source, where the desktop turns down the music itself.

Measured in Firefox on the development machine, with the page held up on purpose, four times for each length:

| Page held up for | Hole in the music | Its reach afterward | Hole in the cues |
|---|---|---|---|
| 30 ms | None | 0.1 s | None |
| 60 ms | None | 0.1 s | 4 to 27 ms |
| 90 ms | None | 0.1 s | 24 to 53 ms |
| 150 ms | 78 ms the first time, then none | 0.2 s | 77 to 126 ms |
| 250 ms | 52 ms the first time, then none | 0.3 s | 191 to 254 ms |
| 350 ms | 87 ms the first time, then none | 0.4 s | 303 to 378 ms |
| 600 ms | About 220 ms each time | 0.4 s | About 620 ms |

On that machine the game itself never holds the page up for so long. Over runs that went up to sector 53, no frame took longer than 33 ms, except one of 50 ms the first time a run starts. That one frame was enough to leave a hole of 10 ms in the music while it was still played through the sound library.

**A page that isn't shown draws no frame.** The game stands still there, so the page stops its sound too, and starts it again when it is shown.

With `?probe` after its address, the page shows how long its frames are, how far ahead the music is handed over, and the holes in the music and in the cues. It is how to answer the question of step 5 on another machine.

## Editor

`music_editor` is a window that changes the piece. It is a crate of its own, which the game doesn't need: it opens [the file](#the-file), shows it in seven tabs, plays it and saves it.

| Tab | Shows | Changes |
|---|---|---|
| Motifs | A motif on a grid of nine degrees, with its skeleton, its ornaments and its turn | The notes of a motif and of its turn, and its number of bars |
| Patterns | Every pattern on one grid of sixteen steps, and the notes that one of them gives over a chord | The steps of a pattern, its levels, its echo and its name |
| Sections | The eight bars of a section, the arrangement and the lines of the threats | The chords, the motifs that the melody states, the tempo of a section, and what each instrument plays in each scene and at each threat |
| Transitions | The roll and the connection, with every way that a connection can go | The numbers of the `roll` and `link` lines |
| Instruments | The first bar that an instrument plays alone, as the synthesizer computes it, and the mix | The numbers of a `sound` line and the levels of a `mix` line |
| Piece | The tempo against the rules of the game, and the two modes on the twelve notes | The tempo, the key and its two modes |
| File | The lines as they will be saved, what is not saved, and every check | Nothing: it saves, reverts and copies |

Three rules shape it:

- **The document is the lines of the file.** A change rewrites words of a line and hands the text to the reader of `tanku_audio`. A change that the reader refuses is not made, and the window shows the reader's own sentence. The editor therefore holds no piece that it could write wrong. The lines that no change touched stay as they were written, comments and columns included, and what it saves is what the game builds with.
- **A control only for what a line holds.** What the rules derive is drawn in dashes and can't be changed: the skeleton, the ornaments, the ways of a connection, the hits of a roll. What is in the code is told and tagged as such: what an instrument is made of, how many notes it sounds at once, when a connection starts.
- **Heard as the game plays it.** The play button plays the thing of the tab alone, or the piece as in the game, in the place, the fight and the threat that the transport is set to. What is played alone is a few lines of the file written again for the occasion, and read by the same reader. A change is heard from the next step on: the music takes the new piece where it is.

The checks that [hold for any piece](#tests-and-benchmarks) run after every change: at once for those that only read the piece, and on a thread of their own for those that listen to it. The editor adds six. Four are about the game: the timings of its rules against the tempo, steps that are whole numbers of half frames, the notes of the cues against the key, and the music 12 dB under a shot. One measures the cost of a second of music. The last says whether the piece still sounds as the saved file does, which tells that a change will move the stored values of the tests.

The key of the file moves every instrument, and the cues stay in A and E: the editor says so next to the key, and one of its checks fails when a mode of the piece has no A or no E in that key.

Like the game, the editor runs from a script without a window and without a sound, which is how it is looked at and listened to in checks: see the [README](../README.md#changing-the-music).

## Tests and benchmarks

Some tests say what the piece is today:

- **Stable.** Thirty-two seconds of each arrangement match a stored value, as the map of a seed does, and so does each cue.
- **As written.** The file has the motifs, the chords, the patterns, the arrangement, the mix, the settings of the instruments and the numbers of this document.
- **On time.** A fight that is announced on each of the 16 steps of a bar gives a roll at once and the fight two beats later. A fight that ends on each of them plays on to its next beat, and gives a connection there and the bridge two beats later.
- **Connected.** For every note the lead can be on, the connection goes to degree 5 one step of the scale at a time and never turns back.

Others hold for any piece that the file writes. They are the checks to run after a change of the file, with `cargo test -p tanku_audio --test checks`. They are part of the crate, in `checks`, so that the [editor](#editor) runs them too:

- **Sound in both modes.** The melodies use the five degrees, the chords are major or minor, and every melody, with its skeleton and its ornaments, passes the check on semitones above the root. The notes of the bridge are the same in both modes, and its third degree is never heard.
- **In range.** Whatever happens, every instrument stays in its range: in every arrangement, on every way out of a fight, and while a tank is destroyed.
- **Never silent.** Every bar of every arrangement makes sound.
- **Never too loud.** No frame of any arrangement or of any change goes past full scale, and none is NaN.
- **Seamless.** The third pass of a loop is identical to the second.

The rest is about the reader and about the sounds:

- **Read.** The file of the piece reads. A file that doesn't says on which line and why: a bar that is not four beats of four steps, a symbol that an instrument doesn't have, a motif that a section plays and that isn't written above it, a melody that doesn't fill its eight bars, a scene without its line, an instrument that is left to the threat when a threat has no line, a setting that an instrument doesn't have or that is past its bounds.
- **Arranged.** A second piece, which arranges what the piece of the game doesn't, plays as its file says: a lead in the menu, two sections in turn at two tempos, a drum that follows the threat, a line under another, two fights of different lengths.
- **Set.** The file gives the instruments the numbers that they had while these were in the code, but for two that it writes round: see [Steps](#steps). The echo of the pluck follows the tempo of its piece.
- **Bounded.** With every instrument at one end of what its `sound` line can say, then at the other, and the mix all the way up, the music stays in range and is never NaN. A tone of the snare that outlasts its noise dies away without a click.
- **In tune.** Every instrument plays the lowest and the highest note of its range within 3 hundredths of a semitone. The cues that play notes play A and E and nothing else, and the warning of a mine is between two notes.
- **Whole.** Each cue lasts as long as it says, never goes past full scale and ends without a click.
- **Ducked.** A ducked music is half as loud within 0.02 s, wherever the step is, and comes back to what it was, to the bit.
- **Cost.** A benchmark renders ten seconds of a fight in an arena at threat 4.

The client has its own:

- **Told right.** The context follows the screen, the run, the kind of its sector and the enemies that are awake. The smartest of them sets the threat, which falls when it is destroyed, and those that sleep don't count. A run that is over counts as the menu, and only the pause menu muffles. The warning of a mine within hearing range ducks the music from the frame that plays it to the moment it ends, and a warning further away doesn't.
- **Unchanged on the way.** The sound library reads the very samples that the crate renders, and reads every cue back from its sound file to a step of 16 bits.
- **Under the cues.** At the default settings, the loudest arrangement peaks 12 dB below the shot of a player, within half a decibel.
- **Heard.** On request, a test plays the piece on the sound device of the machine at no volume, and checks that the device asks for more: `cargo test -p tanku_client reaches_the_sound_device -- --ignored`.
- **Ahead.** In a browser, steady frames leave no hole in the music and keep it a tenth of a second ahead. A long frame leaves one hole, after which the music reaches further, up to four tenths of a second. A page that was hidden teaches nothing.

## Steps

Each step answers a question, and the first one changes nothing in the game.

| Step | Content | Question it answers |
|---|---|---|
| 1, Sketch | The crate with the score, rough instruments and the listening tool. Sound files of the intro, of the bridge, of a fight in a corridor and in an arena at each threat, and of a tour of three minutes. | Are the two motifs worth keeping? Does the minor mode suit the arenas? |
| 2, In the game | The music plays in the client and follows the place and the fight. The Music volume. A switch in the playtest menu between music that follows the place only and music that follows the fight too. | Does the music feel tied to what happens? Can a shot off screen still be told from the beat? |
| 3, Threat | Lead, bass and pluck follow the brain tier | Can a player tell the smarter of two groups by ear? |
| 4, Cues | The waveforms of the cues move to the crate, two cues are retuned, and the warning of a mine ducks the music | Is any cue lost under the music? |
| 5, Web | The web build with music | Does the music play without gaps in a browser? |
| 6, File | The piece moves from the code to a file, which the crate reads while it is compiled, and plays as before | Is the notation of the file enough to change the piece by hand? What does an editor of the piece need beyond it? |
| 7, Editor | A window that shows the file as motifs, patterns, sections, transitions and instruments, plays it while it changes and saves it | Is a change made faster in the window than in the file? Which tabs are used, and what is still changed by hand? |

Step 6 left the music as it was, to the sample, but for two settings of the instruments that the file writes round. The echo of the pluck is 33% as loud as its note, where it was a third. The vibrato of the lead comes in after 136 ms, where it came in after 6000 frames, which is 136.05 ms. In a sound file of 16 bits, no sample of any arrangement is further than 19 steps from what it was, where the loudest samples are at about 20,000.

Step 7 changes nothing in the game. To let a piece change while it plays, the engine now holds its piece behind a shared pointer, where it held the piece of the game by name: every arrangement still matches its stored value.

The switch of step 2 was an experiment, to settle by ear whether the music needs to know of the fights, and it is gone. On its other side the place alone set the arrangement, as in the first proposal for this music: the verse and the chorus played wherever the players were outside the bay, with the kick alone in a corridor and all three drums in an arena, and nothing called for the bridge. After a playtest, the music that follows the fight too was kept. [IDEAS.md](IDEAS.md) keeps the other one.

## Open questions

- Every mine now warns at the end of its fuse, and most mines end that way: brains from the grunt up keep 4 m away from every mine, so few are set off by a tank. A fight with many mines therefore has many warnings, and the music dips for each one within hearing range. Is that too often, for the beeps or for the dips?
- Far from the middle of the screen, two cues stand hardly above the fullest music on some beats, by an estimate of their level in their own bands: a destroyed bullet against the hi-hat, and a placed mine when the lead plays the E that the cue now plays too. Every other cue is at least 12 dB above the music there, and all are next to the player. Are these two heard far away?
- In a browser the music hears of the game a tenth of a second later than on a desktop, and later still on a machine with long frames. Is a roll still felt as the answer to a crossing there?
- The first time a run starts, the web build takes one frame of 50 ms on the development machine, and longer on a slower one, where it leaves a hole in the music once before the music reaches further. What takes that long?
- Two machines in one room play the same piece out of step, because each has its own clock. Does co-op in one room need them in step?
- In hard mode every tier is available from sector 1, so the lead is at its fullest almost at once. Is that the right sound for hard mode, or does it waste the build-up?
- The file can give the piece another key, and the cues still play A and E. Should the cues follow the key of the file?
