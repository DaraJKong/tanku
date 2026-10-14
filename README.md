# Tanku

A cooperative top-down tank game for one to four players, built with Rust and Bevy. The players fight their way through an endless, procedurally generated corridor of enemy tanks, and one hit destroys a tank.

The solo game and co-op are playable, on Linux, on Windows and in a browser:

- **Solo:** the corridor, the generated enemies with their classes and their brains, arenas and their waves, lives and checkpoints, the menus, the sound cues and the music. Runs unlock achievements. Clearing sector 10 unlocks arena mode, a run in one room against one group after another. Clearing sector 15 lets the player type the seed of a run. Clearing sector 20 unlocks free mode, in which the player spends the 59 points of their tank as they like, in a garage that draws the tank as they build it. Clearing sector 30 unlocks hard mode, whose enemies are stronger from the first sector.
- **Co-op:** up to four players. One of them hosts a game, which the others find on the local network or join by its address, from a desktop build or from a browser. A game can have a name and a password. The host sets the run in the lobby: its difficulty, its seed, arena mode, and free mode, in which each player has the tank of their own garage. Nothing is locked for a host who has company. The run starts once everyone has said that they are ready. A destroyed tank is a wreck that a teammate repairs.

Co-op has been played by two players. With three or four, it is checked by tests and by scripted runs only. Arena mode is new and an experiment: its rules are still being set in play (see [Arena mode](#arena-mode)). The bullets that seek, split and explode are new too, with the three classes that fire them and their place in the garage: they are checked by tests and by scripted runs, and nobody has played against them yet. The build for Windows is checked under wine, not yet on Windows itself. The specification is in [docs/PROJECT.md](docs/PROJECT.md).

## Workspace

| Crate | Role |
|---|---|
| `tanku_core` | The rules both sides share: stats, progression, map and enemy generation, simulation types, and what goes over the network. It is compiled with `tanku_core/game.balance`, the file that holds the numbers of the balance. |
| `tanku_server` | The authoritative simulation: a library that the client embeds for solo and hosted games, and a headless binary |
| `tanku_client` | The game: rendering, input, interface and sound |
| `tanku_audio` | The sound that the game computes: the music and the waveforms of the sound cues, which the client plays |
| `music_editor` | A window that changes the music: it opens the file of the piece, plays it as the game does and saves it. The game doesn't need it. |
| `tanku_trials` | The trials of the balance: a tool that counts and measures what the rules make, and tells it in reports. The game doesn't need it. |

## Playing

```bash
cargo run -p tanku_client
```

| Action | Default |
|---|---|
| Move | WASD or the arrow keys |
| Aim | Mouse |
| Shoot | Left click |
| Place a mine | Right click |
| Shield | Space |
| Repair a teammate | Hold Left Shift next to their wreck |
| Turn the view | Q and E |
| Menu | Esc |

Settings, records, what the player has unlocked and the tank of their garage are kept in the user's configuration directory, under `tanku`.

The game keeps a journal there too, `journal.txt`: a line for each run that entered a sector, with its seed, its mode and its options, the sectors that it entered, each tank that was lost with its sector, the weapon and the class of the enemy behind it, the time, the kills and how the run ended. The records keep the best run of each kind, and the journal keeps all of them. Nothing in the game reads it: [the trials](#journal) set it next to their reference players, to tell where a person loses their tanks and which of those players plays as that person does. The machine that runs a run keeps its line, so a single player and the host of a game do, and a guest doesn't. The web build keeps none. A game that is started with `--journal FILE` keeps its journal in that file, and leaves the player's own alone.

The interface is drawn for a window of 1600 by 900 pixels or more. A smaller window shows all of it smaller, so that every menu is whole from 1280 by 720 pixels on. The size that the settings give the interface comes on top of that.

### With friends

Up to four players share a run. One of them hosts the game, and the others join it:

1. The host picks "Host a game" in the main menu, gives the game a name and a password if they like, and opens it. The lobby shows the address of the machine on its network.
2. Each of the others picks "Join a game". A game that is hosted on the same network shows there, under its name if it has one, and a click joins it. One that doesn't show is joined by typing the address of its host. A game that has a password asks for it.
3. The host sets the run in the lobby, and the others see it: normal or hard mode, a seed that the game draws or one that the host types, arena mode, and free mode. In free mode, each player opens their own garage from the lobby and builds their tank there.
4. Each player says that they are ready, the host too, with the button of the lobby or with Enter. Once everyone is, the lobby counts down three seconds and the run starts by itself. Nobody joins a run in progress.

A player takes their word back with the same button, which stops the count. Two changes take it back for them: a player who changes their tank in the garage is no longer ready, and when the host changes what the run is set to, nobody is. A player who joins isn't ready either. After a run, the host picks the same seed again or a new run, which start at once for everyone as the lobby is set, or brings everyone back to the lobby, where nobody is ready until they say so again.

A host needs none of the achievements: every difficulty and every option is theirs from the start. Only a host who starts alone plays a solo run, with its locks. A run in arena mode, in free mode or on a seed that the host typed earns no achievement, and the lobby says so. The game keeps what a host chose last, the difficulty, arena mode and free mode, for their next game.

A tank that is destroyed stays as a wreck, in a circle in dashes. It is in nobody's way: tanks drive over it and bullets fly over it. A teammate repairs it by driving into the circle and holding the repair key for two seconds, and can drive, shoot, lay mines and shield meanwhile: the circle is inked in as the repair goes on, and starts over if the key is let go or the tank leaves the circle. A tank repairs one wreck at a time: the nearest one whose circle it is in. The run is over when every tank is a wreck. A marker at the edge of the screen points to each teammate who is out of view, by the number of its player: full for a tank that runs, hollow for a wreck.

All players need the same version of the game, compiled with the same balance: a game with another `tanku_core/game.balance` is listed as one of another version, and can't be joined. The game listens on UDP port 28405, which the firewall of the host's machine must let in. Games find each other by multicast DNS, on UDP port 5353, which the firewalls that systems ship with let through. Where it is kept out, only the list goes: the address still joins. To play over the internet, that port of the host's router must lead to the host's machine, and the others type the host's public address.

Two games on one machine can play together, which is the quickest way to try it: the second one finds the first on its join screen.

A game that has a password lets in only those who type it, whichever way they come. The join screen lists it as locked. The password itself is never sent: the host asks whoever knocks a question that only the password answers, takes one answer, and sends nothing of the game to someone who has not answered right. That keeps out those who were not given the password. It hides nothing from someone who listens to the network, and a short word that others could guess keeps out the honest alone. The game keeps the name of a game for the next one, and never its password.

#### From a browser

The web build joins games too, and hosts none. A browser can't look around a network, and it only speaks to a game that shows it a certificate. A hosted game makes its own certificate, which a browser takes by its print alone. So a browser joins by a link that the host hands out:

1. The host picks "For a browser" in the lobby. The lobby then shows what the link ends with: a `#`, where the game takes browsers, and the print of its certificate. "Copy" puts it on the clipboard, and the button says whether it got there. The game also writes it to its log.
2. The player opens the page of the game with that at the end of its address, as in `http://localhost:8000/#192.168.1.20:28406/3f0c…`. The page joins the game at once. What the host copied can also be pasted into a page that is already open, with or without the address of the page in front of it.

What a host hands out stays the same from one game to the next, until the host quits the program. It carries no password: a page that follows the link of a game that has one asks for it. A player who left, or whose connection was lost, joins again from "Join a game".

A hosted game listens for browsers on UDP port 28406, which the firewall of the host's machine must let in. Over the internet, that port of the host's router must lead to the host's machine as well, and the host's public address goes into the link in place of the one that the lobby shows.

A browser only joins from a page that it loaded over HTTPS or from its own machine, such as `http://localhost:8000`. A page that another machine serves over plain HTTP joins nothing, and says so. The page stands still while its tab is hidden, and a game takes a player whose machine stays silent for 5 s for gone. Only Firefox was tried.

### Arena mode

Arena mode is an option of a run, as free mode is. A single player unlocks it by clearing sector 10 and turns it on on the solo screen, and the host of a game sets it in the lobby. A player who had earned an achievement before arena mode existed had cleared that sector already, and has it.

A run in arena mode has no corridor. It is fought in one room, which the seed draws, against one group after another:

- **Groups.** A group is the group of the sector of its number, for as many players. Half of it comes when the lull is over, rounded up: as many tanks as a wave of an arena of the corridor has. A group of two comes whole, since one tank and then the other felt like two groups of one. From then on, a tank comes for each one that is destroyed, so that the room stays as full for as long as a tank waits. The place of each tank is marked for a second before the tank appears, as in an arena. A tank whose place has a player within 4 m, or a tank of its group on it, takes another place that has room, so that no tank waits for a player to move.
- **The lull and the reserve.** Between two groups, and before the first one, there is a lull of 3 s. The tanks that have not come yet wait in reserve south of the room, as outlines in dashes with every part that tells a stat, so that the next group is read as a group is through a barrier. A tank leaves the reserve when its place in the room is marked: during a fight, the reserve holds the tanks that each wait for one to be destroyed, and they leave it from left to right.
- **The room.** It is laid out anew for each group, with the cover and the crates of the sector of that number. The place of each block and of each crate is inked in on the floor for 2 s first, and one that has a tank or a mine in its place waits. The marks that a fight left on the floor go with the old layout.
- **Lives.** A single player gets a life back for every fifth group that is cleared, up to three. A tank that is destroyed comes back in the middle of the room, and what was left of the group goes back into reserve and comes again. Several players repair each other, as in the corridor.
- **The view** stays on the room: on a wide screen, all of it and the reserve show at once.
- **The score** is the number of groups cleared. Arena mode has records of its own, and its runs earn no achievement.

Three other rules were tried behind switches of a playtest and left the game: a group in two waves, the second when the last tank of the first was destroyed, a group all at once, and blocks and crates that stood again 10 s after they were destroyed.

```bash
cargo run -p tanku_client -- --playtest --seed TANKU
```

Pick "Solo", turn arena mode on and start. Esc opens the menu, where "Skip to group" goes straight to a later group: group 15 is the first with six tanks.

### Playtest

A playtest lets the player cheat, to test the game faster:

```bash
cargo run -p tanku_client -- --playtest
```

| Cheat | How |
|---|---|
| Infinite lives | On from the start. The pause menu turns them off and on. |
| Skip to a sector | The pause menu picks any sector up to 999. The tank comes back at the end of the sector before it, in front of its barrier, as it does after clearing that sector. In arena mode, it picks a group: the run goes on with the lull before that group. |
| Choose the seed | `--seed TEXT` after `--playtest` starts every run on that seed instead of a random one. |
| Nothing locked | The solo screen takes a typed seed and offers hard mode, arena mode and free mode, whatever the player has unlocked, unless the playtest fakes achievements. |
| Fake achievements | Off at the start. The pause menu turns them on: the machine sets its achievements and its records aside, and plays as on its first time, every option locked. What the playtest earns then, by the rules of a run that counts, is kept in memory until the game is closed: no file is written. Turned off, the machine has its own back. |
| Bring a tank | `--tank TANK` after `--playtest` starts every run in free mode, in that tank, without a visit to the garage: see [The garage check](#the-garage-check). |
| Teammates | The solo screen offers up to three teammates that the game drives, and how well they play. The run is then one of several players: see [Teammates](#teammates). |

With a seed and a skip, a seed and a sector from a report are a few clicks away:

```bash
cargo run -p tanku_client -- --playtest --seed 85GVTHQA
```

A playtest sets no record and earns no achievement, unless it fakes them. The web build has none, because a page has no command line.

#### Teammates

A game of several players can be playtested by one person. The solo screen of a playtest has a row for teammates, from none to three, and one for their skill. The game drives each of them in the hands of a [reference player](#reference-players) of that skill, novice, regular or expert, and the run is one of several players: wrecks and repairs instead of lives, the groups of as many players, and barriers that wait for every tank that runs. The choice stays for the next runs until the game is closed. The teammates play by four rules:

- **They fight** the nearest enemy that is awake in their sector, and dodge and shield as their skill lets them. They hold a shot that would pass a teammate or go off next to one. They keep out of the reach of the player's mines, which they set off, and dodge the player's bullets, which spare them no more than an enemy's.
- **They repair.** The teammate nearest to a wreck drives to it and repairs it, fighting on from there, while the others go on.
- **They wait at barriers.** Once a sector is cleared, they wait in front of its barrier, within the reach that unlocks it, and cross after the player. They never wake a sector first.
- **They follow** the player when nothing is left to fight, and keep off the places where the tanks of an arena wait for room.

They lay no mines, take no cover and never lead, as the reference players don't. They try the rules of a game of several players; they are no measure of what a person does with them.

A script starts such a run with `teammates` after the number of players, and the name of a skill if it isn't the regular one. The run must be a playtest:

```bash
TANKU_DEV_SCRIPT="start TANKU 3 normal teammates expert; wait 12; shot team.png; quit" cargo run -p tanku_client -- --playtest
```

#### The garage check

Free mode lets a player spend the 59 points of their tank as they like. A playtest takes such a tank from the command line, to find out whether one of them makes the game too easy: the same seed is played in the standard tank and in a few extreme ones, each a command away.

A tank is written as its stats with their levels, with commas between them. A stat that isn't named is at level 0. The tank needs an engine, costs 59 points at most and may leave points over: the game says why it refuses a tank. `--help` lists the stats and what their levels cost, and [the specification](docs/PROJECT.md#hardware-stats) says what each level gives.

| Tank | What it spends its points on | Written as |
|---|---|---|
| Standard | A bit of everything | `engine=4,tracks=5,muzzle=2,magazine=4,loader=6,ricochet=1,mines=2,shield=2` |
| Gun | Bullets at 19 m/s, three rounds, and neither a bounce, mines nor a shield | `engine=4,tracks=4,muzzle=7,magazine=2,loader=5` |
| Bouncer | Piercing bullets that bounce three times | `engine=4,tracks=3,muzzle=3,magazine=3,loader=5,ricochet=3,piercing=1` |
| Runner | 8 m/s, and a shield every 6 s | `engine=7,tracks=6,muzzle=2,magazine=2,loader=5,shield=5` |
| Seeker | Seeking bullets that bounce twice | `engine=4,tracks=4,muzzle=2,magazine=4,loader=6,ricochet=2,seeking=1` |
| Flanker | Splitting bullets whose halves bounce once, and a mine | `engine=4,tracks=5,muzzle=2,magazine=4,loader=6,ricochet=1,splitting=1,mines=1` |
| Grenadier | Exploding bullets, and a shield every 10 s | `engine=4,tracks=5,muzzle=2,magazine=4,loader=6,exploding=1,shield=1` |

A tank fires one type of bullet: `piercing`, `seeking`, `splitting` and `exploding` are four stats of one level, and the game refuses a tank that has two of them.

```bash
cargo run -p tanku_client -- --playtest --seed GARAGE --tank engine=4,tracks=4,muzzle=7,magazine=2,loader=5
```

The main menu draws the tank and says how many of its points it spends. In a game of several players, the tank is the host's alone: the others keep the standard one, or the one of their garage if the host turns free mode on. A tank from the command line doesn't change the one of the garage.

## Building

The toolchain is pinned to a nightly in `rust-toolchain.toml`, because development builds use the Cranelift backend and `-Zshare-generics` to compile faster. `rustup` installs it on first use. On Linux, linking needs `clang` and `mold`, and sound needs the ALSA development files.

Run the headless server:

```bash
cargo run -p tanku_server
```

Development builds link Bevy dynamically. A release build leaves the development features out:

```bash
cargo build --release --no-default-features -p tanku_client
```

Cargo can't leave a feature out for a profile by itself, so the build of the client refuses a release that has them: `cargo build --release` and `cargo run --release` alone stop early and name the command above. Such a program would start on the machine that built it and nowhere else. The refusal comes within seconds once a release was built before, and about half a minute into a build from nothing.

A release build takes no script (see [Looking at the game without playing](#looking-at-the-game-without-playing)). With `--features script` on top, it does, and still stands alone.

### Windows

The game is built for Windows from Linux, with the compiler of mingw-w64 (`x86_64-w64-mingw32-gcc`). The toolchain that is pinned has the target:

```bash
cargo build --release --no-default-features -p tanku_client --target x86_64-pc-windows-gnu
```

`target/x86_64-pc-windows-gnu/release/tanku_client.exe` stands alone: it asks for nothing that Windows doesn't have. It opens a console next to its window, which shows what the game logs. The development features are always left out there: a build that links Bevy dynamically crashes on Windows.

A script checks that build under [wine](https://www.winehq.org/), on a Linux machine, without a window and without a sound:

```bash
tanku_client/dev/windows.sh "start TANKU 1; wait 2; shot target/windows/solo.png; quit"
TESTS=1 tanku_client/dev/windows.sh
```

The first plays a scripted game in the build for Windows and saves what it draws. The second runs the tests of the rules, of the sound and of the simulation as Windows programs: a seed gives the same level there, and the music the same sound. Wine is not Windows: a window, a sound device, the keys and a firewall are only checked on a real one.

### Web

The web build needs the `wasm-bindgen` tool, installed once into `target/tools`:

```bash
cargo install wasm-bindgen-cli --version 0.2.129 --root target/tools --locked
```

Build the game into `target/web-dist`:

```bash
tanku_client/web/build.sh
```

Serve it locally:

```bash
python3 -m http.server --directory target/web-dist
```

The web build keeps settings and records in the local storage of the browser. A browser lets no sound play before the first click or key, so the sound of the game starts with it.

The web build joins the games that desktop builds host: see [From a browser](#from-a-browser).

With `?probe` after the address, the page shows how long its frames are and whether its sound has holes, for the music and for the cues. A click on those numbers starts the count again.

```
http://localhost:8000/?probe
```

## Checks

Run these before finishing a change:

```bash
cargo fmt --all
```

```bash
cargo clippy --workspace --all-targets
```

```bash
cargo check -p tanku_server
```

```bash
cargo test --workspace
```

The server-only check exists because a workspace build gives the server the client's Bevy features.

Clippy refuses the calls that stop the program when they fail: `unwrap`, `expect`, `panic!`, `unreachable!`, `todo!` and `unimplemented!`. Write what happens in the case that they leave out. Where stopping is meant, mark the place with its reason, and Clippy tells when the mark is no longer needed:

```rust
#[expect(clippy::unwrap_used, reason = "each wall of a shape starts with a point at its entry")]
```

The tests keep these calls, and so do the trials, which must stop when they can't go on. Clippy takes a module for tests when its attribute is `#[cfg(test)]` and nothing else: another condition goes into a second attribute.

GitHub runs the checks again on every push, with the warnings of Clippy as errors (`.github/workflows/checks.yml`):

- **Linux:** all four.
- **Windows:** the tests of the rules, of the sound and of the simulation, as `tanku_client/dev/windows.sh` runs them under wine but on a real Windows, and Clippy on the client as a release has it. They are built for `x86_64-pc-windows-msvc`, where the release is built from Linux for `x86_64-pc-windows-gnu`.
- **Web:** Clippy on the client compiled for the page, which can't run the tests.

A run is listed on the page of its commit on GitHub, and here by `gh run list`. The first run of a branch builds everything from nothing and takes the longest; the next ones start from a cache.

The tests of the network open real games on this machine and play them in real time. On a machine that is busy with something else, such as another build, a dozen of them can fail together, most of them in `mirror`: run them again once the machine is quiet. Keeping their threads on two cores shows it every time:

```bash
taskset -c 0-1 cargo test -p tanku_client -- --test-threads=24
```

The tests check the map generator over a dozen seeds. A wider check over a thousand seeds takes two to three minutes and is worth running after a change to the generator:

```bash
cargo test -p tanku_core many_seeds -- --ignored
```

The tests also lay every menu out, without a screen, in windows from 1280 by 720 pixels up. They fail if a button or a text crosses the fine line around the sheet, if something lies on it, or if a box lost one of its lines. After a change to a menu, this prints how far down each menu reaches in each of those windows, and where that line is:

```bash
cargo test -p tanku_client room_the_menus_take -- --ignored --nocapture
```

The tests of the music editor lay each of its tabs out in the same way, in three sizes of window, and fail if a button crosses that line or lies on another.

After a change to a brain, two tests compare a soldier, a veteran and an ace in the same five tanks. Each fights a crude player that nothing destroys, in about twenty sectors of six seeds: one tank alone, against bursts and against single shots, and two tanks together, against bursts. They print how long the enemies lasted and how many of their shots landed, and take six minutes:

```bash
cargo test -p tanku_server trials::brains -- --ignored --nocapture
```

After a change to what a mode makes of its enemies, two more tests send the same crude player against the groups of the first thirty sectors as they are, in normal mode and in hard mode. They print the same figures for each mode by stretch of sectors, and take nine minutes:

```bash
cargo test -p tanku_server trials::modes -- --ignored --nocapture
```

After a change to arena mode, one more test fights the groups of sectors 6 to 9 and 15 to 20 twice on the same eight seeds: each in its sector of the corridor, and in the room. It prints the same figures, and takes a minute and a half:

```bash
cargo test -p tanku_server trials::arena -- --ignored --nocapture
```

On 2026-10-12 it gave this, with the two ways that were tried first, on the same seeds. A room that is kept full is faster and more dangerous than the corridor: about 30% more shots land in a minute, and in the late groups a fifth more for each tank that falls. It lies between the two waves, which were about as dangerous as the corridor, and the group that came all at once:

| Groups | Fought | Median time | Shots that landed in a minute | For each tank that fell |
|---|---|---|---|---|
| 6 to 9 | In the corridor | 13.1 s | 7.0 | 0.62 |
| 6 to 9 | In the room | 9.6 s | 9.1 | 0.52 |
| 6 to 9 | In the room, in two waves (left the game) | 10.3 s | 8.2 | 0.48 |
| 6 to 9 | In the room, all at once (left the game) | 10.9 s | 14.3 | 0.85 |
| 15 to 20 | In the corridor | 33.3 s | 13.3 | 1.30 |
| 15 to 20 | In the room | 26.9 s | 17.1 | 1.57 |
| 15 to 20 | In the room, in two waves (left the game) | 27.5 s | 14.7 | 1.29 |
| 15 to 20 | In the room, all at once (left the game) | 25.6 s | 20.2 | 1.66 |

The crude player says little about one of flesh and blood: nothing destroys it, and it neither takes cover nor reads the reserve.

Before a change to what a player's tank may be, one more test puts the same crude player in the four tanks of [the garage check](#the-garage-check), against the groups of sectors 6 to 15. That player raises its shield just before a bullet reaches its tank and lays no mine. The test prints the same figures for each tank, and how many shots landed for each enemy that fell. It takes four minutes:

```bash
cargo test -p tanku_server trials::garage -- --ignored --nocapture
```

After a change to the [teammates](#teammates) of a playtest, one more test sends teams of two, three and four of them alone through the corridor of twelve seeds, the first seat leading through the barriers, and nothing protects them. It prints the sectors that each team cleared, and every run that stopped clearing sectors for three minutes, with what held it: where each tank stood, and what was left to fight. It takes six to seven minutes:

```bash
cargo test -p tanku_server teams_of_teammates -- --ignored --nocapture
```

On 2026-10-14, regular teammates cleared 16.0 sectors on average in teams of two, 20.6 in teams of three and 21.5 in teams of four, and no run stopped. The regular player alone, with three lives, clears about 16. Teams of reference players tell little of what people do together, and twelve seeds are few.

This prints what the groups of each mode are made of, by stretch of sectors: which brains they have, and how many points they can't spend:

```bash
cargo test -p tanku_core what_the_groups_are_made_of -- --ignored --nocapture
```

A game that is told to leave while it is still starting must leave without a crash. The engine compiles the pipelines of the graphics card in tasks of their own, and the game waits for them before it goes (`tanku_client/src/quit.rs`): without that, the process ended under a task that was still inside the driver of the card. After a change to how the game starts or leaves, this quits a tenth of a second after the start, where it used to crash every time. The number that it prints last is how the game left, and 0 is well:

```bash
TANKU_DEV_SCRIPT="wait 0.1; quit" cargo run -q -p tanku_client; echo $?
```

Benchmarks measure generation, the simulation and the music, and take under a minute:

```bash
cargo bench -p tanku_core -p tanku_server -p tanku_audio --features tanku_core/game-balance
```

The feature makes them measure the game as it is compiled. Without it, they are built like the tests, with the balance of the tests (see [Changing the balance](#changing-the-balance)).

## Looking at the game without playing

Development builds, and any build with the `script` feature, read a script from `TANKU_DEV_SCRIPT`: commands separated by semicolons that start a run, jump to a sector, drive the tank, press the buttons of the menus and save screenshots. The commands are listed at the top of `tanku_client/src/dev.rs`.

A scripted run opens no window and needs no display: it draws into an image of 1920 by 1080 pixels, which the `shot` command saves. It makes no sound, starts from the default settings, and neither reads nor writes the saved settings and records.

The command `canvas` gives the image another size, to see what a smaller window shows: `canvas 1280 720` draws as a window of 1280 by 720 pixels does. A third number is the scale of the screen: `canvas 1920 1080 1.5` draws as a screen of 1920 by 1080 pixels does when its system enlarges everything by half, which leaves the game 1280 by 720 pixels of its own.

```bash
TANKU_DEV_SCRIPT="start TANKU; wait 1; warp 12; wait 1; peek 0; wait 1; shot sector12.png; quit" cargo run -p tanku_client
```

A run starts in arena mode with `arena` after the difficulty. There, `warp 14` goes to the lull before group 15, and `clear` destroys the tanks that are in the room:

```bash
TANKU_DEV_SCRIPT="start TANKU 1 normal arena; warp 14; wait 2; shot reserve.png; wait 3; shot group.png; quit" cargo run -p tanku_client
```

Two scripted games can play together: one hosts on a port, and the other joins it. A game that hosts on a port of its own takes browsers on the port after it, and the words after the port are the name of the game, as in `host 28999 Friday night`. `password sesame` before it gives the game a password, and `join 127.0.0.1:28999 sesame` joins with one. A script can also go through the menus as a player does: `press` presses a button by its label, `type` writes a text and `key` presses `enter`, `tab` or `backspace`. Where two rows of a menu hold buttons with the same label, `press` names the row first, as in `press Arena mode: On`.

```bash
TANKU_DEV_SCRIPT="host 28999; wait 6; press Ready; wait 9; shot host.png; quit" cargo run -p tanku_client
```

```bash
TANKU_DEV_SCRIPT="wait 2; join 127.0.0.1:28999; wait 3; press Ready; wait 9; shot guest.png; quit" cargo run -p tanku_client
```

The run starts three seconds after both have said that they are ready.

Two games on one machine talk over the loopback, where every address is the machine's own and nothing is late or lost. That hides what goes wrong between two machines. A script runs the two games as two machines on a private network, a host at `10.77.0.1` and a guest at `10.77.0.2`, and can worsen the line between them. A change to the network is checked with it:

```bash
tanku_client/dev/two_machines.sh "host; wait 6; press Ready; wait 9; shot host.png; quit" "wait 2; join 10.77.0.1; wait 3; press Ready; wait 9; shot guest.png; quit" "delay 50ms 10ms loss 2%"
```

It takes Linux with user namespaces and needs no privileges. The third argument is optional: here it makes a round trip of 100 ms that loses one packet in fifty each way.

The guest can also find the game as a player does, instead of being told its address: `press Join a game; wait 3; press 10.77.0.1` opens the join screen and presses the row of the game that it lists. With `WALL=mdns` in front of the command, each machine has a firewall that lets in what the ones that systems ship with do: the game must still be found. With `WALL=shut`, the firewalls keep multicast DNS out as well: only the address joins.

A browser doesn't join as a desktop game does, so a second script runs a scripted host and a Firefox as two machines. The browser opens the web build with the link of the host at its end, and then does what its steps say: it waits, clicks, holds keys and saves what the page shows. The steps are listed at the top of `tanku_client/dev/firefox.py`.

```bash
tanku_client/dev/browser_guest.sh "host; wait 34; press Ready; wait 7; shot host.png; wait 14; quit" "wait:24 shot:lobby click:262:448 wait:12 click:683:600 key:w:2 shot:run wait:5" "delay 60ms 10ms loss 2%"
```

The click at 262, 448 is on the button "Ready" of the lobby, as the page of the script shows it, 1366 by 682 pixels, while the run is in normal mode and in the corridor. Enter says the same wherever the button is: `click:683:600` gives the page the keys, and `key:K:0.1` presses Enter when K is the key U+E007, `$'\uE007'` in bash.

It takes Firefox, Python 3 and the web build in `target/web-dist`. No window opens. What the browser showed goes to `target/browser_guest`.

On a bad line, a host may log errors of `lightyear_debug::input` about an input for a tick that it has already simulated. They tell that the orders of a guest came late: its clock was behind, after the guest stalled for a moment or the line suddenly got slower. The host goes on with the last orders it had, the network library sets the clock of the guest anew within a second or two, and the errors stop. Nothing is lost: what the guest shows of its own tank is put right by what the host tells.

A host also logs an error of `lightyear_udp::server` about a packet for an unknown entity when a guest leaves over a slow line. A guest says goodbye several times over, in case one is lost, and the later ones find it gone already. Nothing is wrong.

A sector takes a third of a second to wake once the first player is through its barrier, which a screenshot seldom catches. The command `lift` holds everything that wakes at one moment of its rise, in seconds since it woke, and `lift` alone lets it go on. The simulation doesn't wait meanwhile: the enemies of the sector move and fire.

```bash
TANKU_DEV_SCRIPT="start TANKU; wait 1; warp 4; wait 3; lift 0.1; cross; wait 0.5; shot rising.png; lift; wait 0.5; shot awake.png; quit" cargo run -p tanku_client
```

In the room of arena mode, the blocks and the crates of the next group rise off their plans in the same way, two seconds after a group is cleared. The command `rout` sets off a mine under each enemy that is in the room, which clears the first group of a single player:

```bash
TANKU_DEV_SCRIPT="start TANKU 1 normal arena; wait 4.5; rout; wait 1; lift 0.1; wait 2; shot rising.png; lift; wait 0.5; shot standing.png; quit" cargo run -p tanku_client
```

A scripted run is silent, and its log can list the cues that it would have played, each with its time and how loud: the module that plays them tells of each one to a game that logs what is for debugging.

```bash
RUST_LOG=info,tanku_client::audio=debug TANKU_DEV_SCRIPT="start TANKU; wait 1; warp 4; wait 3; cross; wait 2; clear; wait 4; quit" cargo run -p tanku_client
```

The command `tape` keeps the music that the run would have played from there on. When the script quits, the music goes to a sound file, and the list of what the game told the music, with the time of each change, to a text file of the same name:

```bash
TANKU_DEV_SCRIPT="tape run.wav; wait 4; start TANKU; wait 4; play 0 1 0 0 4; wait 8; clear; wait 8; quit" cargo run -p tanku_client
```

## Trials of the balance

The trials measure what the rules of the game make, without anyone at the keyboard, and tell it in reports that can be compared from one change to the next. They are a tool of their own, `tanku_trials`, with a command for each kind of trial. The census counts what the seeds generate, the comparison of the reference players says what the yardstick of every fight is worth, the skirmishes say what each stretch of sectors costs and which tanks the cost comes from, the duels say what each level of a stat adds to the danger of a tank next to what it adds to its price, the trial of the classes draws the curve of the danger of each class over its points, the runs say how far a player gets with three lives, the journal sets the runs that a person played next to the reference players, and the breeding looks for the tank that does the most with its points. A comparison tells what a change to the game moved between two reports. The numbers that the trials are there to judge are in one file: see [Changing the balance](#changing-the-balance).

### Census

The census fights nothing. It draws the corridor and the groups of many seeds as the game does, and counts what each stretch of sectors holds. It takes seconds:

```bash
cargo run --release -p tanku_trials -- census
```

| Table | What it counts |
|---|---|
| Levers | What the progression gives each sector: the budget, the tanks of a group, the highest tiers of software, the fastest gun, the reaction and the aiming error of the brains, the terrain |
| Groups | The tanks and the points of a group, how the points are shared, what its leader has |
| Classes | The share of each class, and the points that its tanks spend |
| Classes that show | The share of the tanks of a class that have every stat that their color announces |
| Bullets | The types of bullet that fly, the groups that hold a special one, the bounces and the speeds |
| Software | The tiers of brain and of ballistics |
| Hardware | The tanks that can't move, the speeds, the magazines, the mines and the shields |
| Terrain | The floor of a corridor, its blocks and its crates as the seeds draw them |

| Option | Effect |
|---|---|
| `--sectors 1-50` | The sectors to count |
| `--seeds 200` | How many seeds to count them on: `CENSUS0`, `CENSUS1` and so on, which the game takes too |
| `--players 1` | The number of players that the groups are made for, from 1 to 4 |
| `--difficulty normal` | `normal` or `hard` |
| `--each` | One row for each sector, in place of one for each stretch of sectors |
| `--out DIRECTORY` | Writes the report there as `census.md` and `census.csv`. It is printed either way. |

The CSV file holds one cell on each line, with its table, its row and its column, for whatever compares two reports. The same command gives the same report.

### Reference players

A fight of a trial is the real simulation, without a window, with a reference player in the players' tank. The player is always the same, so that two reports differ by what was changed in the game and by nothing else. There are three, which fight the same way and differ in what they do about what flies at them:

| | Novice | Regular | Expert |
|---|---|---|---|
| Sees a bullet coming | Never | After 0.30 s | After 0.20 s |
| Follows a bullet off a wall | No | No | Yes |
| Shield | Never | When the bullet can't be dodged | The same |

With the time that the standard tank takes to get out of the way, the regular player reacts in about the 0.55 s that the specification counts for a player who keeps moving. All three keep 8 to 14 m from the nearest enemy and drive around it, lead their shots, find a shot off a wall at an enemy that they don't see, and fire as fast as the gun does. They find their way around what hides an enemy, and they hold a shot that would come back to their own tank off a wall or go off within reach of it. Those who see bullets coming keep out of the reach of mines and of the blasts that are marked, and know where their own shots go. None lays a mine, takes cover or picks the moment of a fight.

Three fights would never end from 8 to 14 m, and the players have a way through each. When two enemies or fewer are awake and none has fallen for 20 s, they close in to 3 to 5 m: a last ace that backs away and a tank that shoots down every bullet don't fall to shots from afar. When something stands between them and the place that they drive to, they take the way around it, where they used to turn back in front of it. And in an arena, they drive off the places where the tanks of the next wave wait for room. Closing in costs them tanks, as it costs a person: against a fast ace that is alone, the expert ends the duel in a third of the time and loses nearly three times as many tanks in it.

Nothing destroys the tank of a reference player. What would have is counted as a loss, so that every fight lasts until the group has fallen or the time is up. The losses are the one measure of what a group costs. What hits the tank within 1.5 s of a loss is no other one, since a tank that is destroyed is gone for that long.

This command puts each of them against the group of every sector that is a corridor, on the same seeds. It takes about half a minute on 24 cores:

```bash
cargo run --release -p tanku_trials -- players
```

| Option | Effect |
|---|---|
| `--sectors 1-30` | The sectors whose groups are fought |
| `--seeds 40` | How many seeds to fight them on: `TRIAL0`, `TRIAL1` and so on |
| `--difficulty normal` | `normal` or `hard` |
| `--player NAME` | A player to compare: `novice`, `regular` or `expert`. It can be given several times, and all three are compared without it. |
| `--limit 60` | How long a fight lasts at most, in seconds |
| `--threads N` | How many fights run side by side: as many as the machine has cores, unless told |
| `--out DIRECTORY` | Writes the report there as `players.md` and `players.csv` |

The players use nothing of the brains of the enemies but how to aim and how to steer around a wall, so a change to how a brain dodges doesn't move the yardstick. A change to a reference player does: its numbers are then compared with nothing that came before.

### Skirmishes

A skirmish is one reference player against the group of a sector that is a corridor, as the game makes it, from the moment the barrier is crossed. Arenas, whose groups come in waves, are not fought yet. It takes seconds:

```bash
cargo run --release -p tanku_trials -- skirmish
```

A loss is told to its cause, and to the tank of the group that fired the bullet or laid the mine. The blast of a crate is nobody's.

| Table | What it tells |
|---|---|
| Sectors | For each stretch of sectors: the fights, the groups that fell in time, how long they lasted, the losses in a fight, in a minute and for each enemy that fell, and the losses to bullets, to mines, to exploding bullets and to crates |
| Points | The tanks by the points that they spent |
| Classes | The tanks by their class |
| Bullets | The tanks by the type of their bullets |
| Brains, Ballistics | The tanks by the tiers of their software |
| Guns, Engines | The tanks by the speed of their bullets, and by their own |

Each table of the tanks tells how many there were, the points that they spent, how many fell, how long they lasted, and the losses that were their doing: for each tank, for each minute that it lasted and for each hundred points that it spent. Prices are right when that last number is about the same in every row. These tables hold every tank of the sectors asked for together, and a late sector has more tanks, with more points and better brains: a narrow `--sectors 21-30` compares like with like.

| Option | Effect |
|---|---|
| `--sectors 1-30` | The sectors whose groups are fought |
| `--seeds 40` | How many seeds to fight them on: `TRIAL0`, `TRIAL1` and so on |
| `--difficulty normal` | `normal` or `hard` |
| `--player regular` | The player who fights: `novice`, `regular` or `expert` |
| `--limit 60` | How long a fight lasts at most, in seconds |
| `--each` | One row for each sector, in place of one for each stretch of sectors |
| `--threads N` | How many fights run side by side |
| `--out DIRECTORY` | Writes the report there as `skirmish.md` and `skirmish.csv` |

### Duels

The duels answer whether a stat is priced right, without a group around the tank. They start from one tank, the base, and try one stat at a time at each of its levels, with everything else as it is. Each of these tanks is fought in the corridors of some sectors, with the brains of those sectors, in the places of tanks of their groups. It takes about a minute on 24 cores:

```bash
cargo run --release -p tanku_trials -- duels
```

The player fights three tanks of the same make at once: one alone falls within seconds, and tells little. A stat that needs another raises it, and one that is needed by another lowers that one when it is tried lower: ballistics 3 comes with two bounces, a tank without an engine has no tracks, and a tank whose bullets explode has no ricochet. Each row tells what its tank is made of.

There is a table for the base, one for each stat, and one for the types of bullet. Each row tells the points of the tank, the duels, those that the tank lost in time, how long it lasted, the tanks that the player lost in all, the losses in a duel and in a minute, the losses in a duel for each hundred points of the tank, and the slope from the base to the tank: the losses in a duel that each hundred points of difference made. Prices are right when the slope is about the same for every stat. A row is as sure as it has tanks lost: 25 are known to a fifth, and 100 to a tenth.

| Option | Effect |
|---|---|
| `--tank TANK` | The base, as its stats with their levels: `engine=3,tracks=3,turret=3,muzzle=2,magazine=2,loader=3,ricochet=1,brain=2,ballistics=1` unless told. A stat that isn't named is at level 0. |
| `--stat NAME` | A stat to try. It can be given several times, and every stat is tried without it. |
| `--sectors 11-14` | The sectors that the duels are fought in: their corridors, and the experience of their brains |
| `--seeds 60` | How many seeds to fight on |
| `--count 3` | How many tanks of the make the player fights at once, from 1 to 6 |
| `--difficulty normal` | `normal` or `hard`, which sets the experience of the brains |
| `--player regular` | The player who fights |
| `--limit 40` | How long a duel lasts at most, in seconds |
| `--threads N` | How many duels run side by side |
| `--out DIRECTORY` | Writes the report there as `duels.md` and `duels.csv` |

### Classes

The trial of the classes fights each class alone. Its tanks are made as the game makes a tank of that class, with a budget that is given and under the caps of the sectors that they are fought in. What a tank buys besides the stats of its class is drawn anew for each tank, so what is left over hundreds of them is the class itself. The budget grows from one column to the next, which gives the curve of the danger of a class over its points. It takes about a minute on 24 cores:

```bash
cargo run --release -p tanku_trials -- classes
```

Every table has a row for each class and a column for each budget: the losses of the player in a fight, the losses for each hundred points that the tanks spent, how long a tank lasted, the fights that were cleared in time, the points that a tank spent, and the tanks that the player lost in all, which say how sure the other numbers are. Prices are right when the classes are close to each other in a column of the first table.

| Option | Effect |
|---|---|
| `--budgets 20,40,60,80,100,120` | The budgets that the tanks are made with |
| `--sectors 21-30` | The sectors that the tanks are made for and fought in: their caps, their corridors and the experience of their brains |
| `--seeds 20` | How many seeds to fight on |
| `--count 3` | How many tanks of the class the player fights at once, each made apart |
| `--difficulty normal` | `normal` or `hard` |
| `--player regular` | The player who fights |
| `--limit 40` | How long a fight lasts at most, in seconds |
| `--threads N` | How many fights run side by side |
| `--out DIRECTORY` | Writes the report there as `classes.md` and `classes.csv` |

### Runs

The runs are whole runs of a single player: from the start bay, with three lives, the checkpoints, the arenas with their waves and the lives that they leave, until the last life is lost. Here the tank of the reference player is destroyed as any other. It takes about a minute on 24 cores:

```bash
cargo run --release -p tanku_trials -- runs
```

The player fights and doesn't drive the corridor: once a sector is cleared, the tank is put through its barrier, over the life of an arena if one is missing. The length of a run is therefore the time of its fights, and the drive between them is an estimate, the way from one barrier to the next in a straight line at the speed of the standard tank. A run whose fight lasts longer than the patience of the trial is stopped and counted apart, and its score is then a lower bound. The report says what held each of them: the enemies that were left, how far they were and whether the player saw them, and the tanks of an arena that waited for room.

| Table | What it tells |
|---|---|
| Runs | For each player: the sectors cleared, as a mean, at five points of their spread and at best, how the runs ended, and the median of their length |
| Sectors cleared | The runs that cleared sectors 5, 10, 15, 20, 30, 40 and 50, among them the four that unlock something |
| Where the tanks are lost | For each stretch of sectors: the sectors entered, the tanks lost, and the runs that ended there |
| Corridors and arenas | The tanks lost for each sector entered, by the kind of the sector |
| What destroyed the player | The tanks lost, by the weapon and the type of bullet |
| Whose doing it was | The tanks lost, by the class of the enemy behind it |
| Runs that stalled | Each run that was stopped in a fight that neither side could end: its sector, and what was left in it |

| Option | Effect |
|---|---|
| `--seeds 100` | How many seeds to play: `TRIAL0`, `TRIAL1` and so on |
| `--difficulty normal` | `normal` or `hard` |
| `--player NAME` | A player to play them. It can be given several times, and all three play without it. |
| `--last 50` | The sector at which a run is stopped if it gets there |
| `--patience 180` | How long a fight may last, in seconds, before its run is stopped |
| `--threads N` | How many runs go side by side |
| `--out DIRECTORY` | Writes the report there as `runs.md` and `runs.csv` |

### Journal

The reference players are a yardstick, and nothing says which of them a person is. The game keeps a journal of the runs that are played (see [Playing](#playing)), and this command sets it next to the three players. It takes about half a minute on 24 cores:

```bash
cargo run --release -p tanku_trials -- journal
```

The runs that compare with those of the trials are counted: a single player's, in the standard tank, in the corridor, at the difficulty asked for, played with this version of the game and with the balance that the tool is compiled with. The report says how many runs it left out, and why. A playtest is counted, with its infinite lives and its skips, because the score of a run is not what is compared: most of what is played is no whole run. What every run tells is the tanks that it lost for each sector that it entered.

Each reference player therefore fights for every sector that the journal got to, as in the comparison of the players: on many seeds, with a tank that nothing destroys, and each tank that it would have lost is counted. The player that the journal is nearest to is the one whose losses make those of the journal the most likely. A few tanks seldom tell two players apart, and the report then names both.

| Table | What it tells |
|---|---|
| What was read | The runs of the journal, those that are left out, each for the first reason that holds, and those that are counted |
| Tanks lost in each sector | For each stretch of sectors: the corridors that the journal entered, the tanks that it lost there, what each reference player loses in a fight for such a corridor, and the nearest player. The arenas of the journal are a row of their own. |
| Over the corridors of the journal | The tanks that the journal lost, those that each player would lose over the same corridors, and the player that the journal plays as |
| What destroyed the player | The same tanks, by the weapon and the type of bullet |
| Whose doing it was | The same tanks, by the class of the enemy behind it |
| Whole runs | Only for a journal that has runs played as a record counts one, from the start bay with three lives until the last is lost: their scores next to those of the runs of each reference player |

| Option | Effect |
|---|---|
| `FILE` | The journal to read. Without it, the player's own, in the user's configuration directory. |
| `--seeds 40` | How many seeds each reference player fights on: `TRIAL0`, `TRIAL1` and so on |
| `--difficulty normal` | `normal` or `hard`: the runs of the other one are left out |
| `--since 2026-10-13` | Counts the runs that started at that moment or later, as the journal writes one: `2026-10-13`, or `2026-10-13T17:46Z` |
| `--every-build` | Counts the runs of every version and of every balance |
| `--last 60` | The furthest sector that the reference players fight in |
| `--limit 60` | How long a fight lasts at most, in seconds |
| `--patience 180` | How long a fight of a whole run may last, in seconds, before the run is stopped |
| `--threads N` | How many fights run side by side |
| `--out DIRECTORY` | Writes the report there as `journal.md` and `journal.csv` |

The report is worth what the journal holds. A stretch of sectors says little before the journal has entered a few dozen of its corridors, and a journal that was played with several balances compares with none of them: `--every-build` counts all its runs against the balance of the tool. Three things are not compared at all. The reference players fight in no arena, so the arenas of the journal are counted and set next to nothing. A whole run stops at its third tank, in the middle of a fight that goes on for a reference player, so the last sector of such a run counts a little less than a fight of the trials does. And the journal tells which player loses as many tanks as a person does, not that it loses them the same way: the two tables of what destroyed the player are there to show the difference.

### Breeding

Prices are right when points buy as much danger wherever a tank spends them. The other trials can't say whether they do: the duels try one stat at a time, and the classes are tried as the generator makes them, which spends by the weights of each class. The breeding looks for the tank that costs a player the most for its points, and shows where that tank puts them. It takes about three minutes on 24 cores:

```bash
cargo run --release -p tanku_trials -- breed
```

A generation is 32 makes that have the same points. Each make fights a reference player, three tanks of it at once, in fights that are the same for every make of the generation. The first generation is made as the game makes its tanks, class after class. Each of the others is the best quarter of the one before it and children of theirs: a child has each of its levels from one parent or from the other, one thing about it is changed, and it is brought back under the caps of its class and of its sector and within its points. A make that is kept fights again in each generation, so the number that it is kept for gets surer. The makes that lead at the end then fight again, in fights that chose none of them, next to tanks of each class as the game makes them with the same points.

The step from two tanks to a child is `tanku_core::breed`, which [impossible mode](docs/PROJECT.md#impossible-mode) will breed its enemies with. No run of the game breeds a tank yet.

| Table | What it tells |
|---|---|
| The best makes | The makes that the breeding ended with: their stats, what the breeding knew of them, and what the fights of the end tell, as tanks lost in a fight and as that number for each tank that the game's own cost |
| The tanks of the game | The same fights against tanks of each class as the game makes them with the same points, and against a tank of the game whatever its class |
| Where the points go | For each stat: its level and the points spent on it, on the best makes and on the tanks of the game |
| Generations | What a fight cost against a make of each generation, a new one, one that is kept and the one that leads |

| Option | Effect |
|---|---|
| `--budget 60` | The points of every tank of the trial |
| `--sectors 21-30` | The sectors that the tanks fight in, with their corridors and the experience of their brains. The tanks are bred under the caps of the last one. |
| `--count 3` | How many tanks of one make the player fights at once |
| `--difficulty normal` | `normal` or `hard` |
| `--player regular` | The player who fights: `novice`, `regular` or `expert` |
| `--class NAME` | Breeds the makes of one class alone: `all-rounder`, `sniper` and so on. Without it, they are bred of every class of the sector, and a child may take another class than its parents. |
| `--limit 40` | How long a fight lasts at most, in seconds |
| `--population 32` | How many makes a generation holds |
| `--generations 20` | How many generations fight, the first one included |
| `--fights 24` | How many fights each make fights in each generation that it is in |
| `--finals 200` | How many fights each of the best makes fights at the end, and each class of the game with it |
| `--best 10` | How many of the best makes are fought again at the end, and told |
| `--seed BREED` | The text that decides what is drawn: the first tanks, the parents of each child and what changes in it. Another text breeds other tanks. |
| `--threads N` | How many fights run side by side |
| `--out DIRECTORY` | Writes the report there as `breed.md` and `breed.csv` |

A make that does several times what the tanks of the game do with the same points has found points that are worth more than their price. The table of the points names the stats: those that the best makes spend far more on than the game does, and those that they leave out. It doesn't say which level of them carries the make. The duels do, started from the make itself: `duels --tank` followed by its stats, as the report writes them, and `--stat` for each stat to try.

The trial has four limits to read it with:

- **It finds what beats the player that it fights.** The regular player follows no bullet off a wall, so it is the easier prey of a make that seeks or banks its shots. `--player expert` shows what is left against a player who follows them.
- **Three tanks of one make are no group of the game.** What is worth more in numbers, such as the crossfire of the aces, is worth more here than in a group that has one of them.
- **It tells of one budget.** Every tank of it has the same points, so it says nothing of whether sixty points are worth twice thirty.
- **Its best makes are close relatives.** They are what one breeding ended with. Another `--seed` says whether another one ends in the same place, and `--class` what the best make of one class is.

A make is told by the first class that may have its levels, which is the all-rounder for a make that any class could have: a child keeps the class of a parent whatever it kept of that parent, and a class is no promise of what a bred tank favors.

### Comparing two reports

A change to the game is judged by the same trial before and after it, each written to a directory of its own:

```bash
cargo run --release -p tanku_trials -- skirmish --out target/trials/before
```

```bash
cargo run --release -p tanku_trials -- skirmish --out target/trials/after
```

```bash
cargo run --release -p tanku_trials -- compare target/trials/before/skirmish.csv target/trials/after/skirmish.csv
```

The comparison lists every cell that changed by 10% or more, table by table, with its value before and after, and the rows that came or went. It says so when the two reports were not asked for the same thing. It knows nothing of noise: a cell that rests on few losses moves by itself from one set of seeds to another, so the same seeds are compared, and a small change of a small number is no finding.

Every report says which balance it was made with, on a line under what it was asked for: a number that tells the words of `tanku_core/game.balance` from those of another. A comparison says whether the balance changed between its two reports.

| Option | Effect |
|---|---|
| `--least 10` | The least change that is told, in percent of the value before |
| `--table NAME` | A table to compare. It can be given several times, and every table is compared without it. |
| `--out DIRECTORY` | Writes the comparison there as `compare.md` and `compare.csv` |

### Changing the balance

The numbers that decide how hard the game is are written by hand in `tanku_core/game.balance`: what the levels of the stats give and cost, the standard tank, the weapons, how the sectors get harder, how a group shares its points, the classes, the terrain, the arenas, arena mode, how the brains fight, and what unlocks the modes. [docs/BALANCE.md](docs/BALANCE.md) describes each line. The game is compiled with the file, so a change shows the next time anything is built, and a file that can't be read stops the build and says on which line:

```
error[E0080]: evaluation panicked: game.balance, line 11: a stat has a number for each of its levels, level 0 included
```

A change of the balance is tried in four steps:

1. Write the report of a trial as the game is: `cargo run --release -p tanku_trials -- skirmish --out target/trials/before`.
2. Change a line of the file. A group that gets its sixth tank at sector 24 and not at 20 is `step tanks  1  4 8 12 16 24`.
3. Write the report again, to `target/trials/after`. The tool is compiled again first, which takes a quarter of a minute.
4. Compare the two: `cargo run --release -p tanku_trials -- compare target/trials/before/skirmish.csv target/trials/after/skirmish.csv`.

The file holds numbers and no rules. What a number can't say stays in the code: which stats and classes there are, what each tier of a brain does, how the walls of the corridor are shaped.

A change to the file fails one test, which watches it: it says that `docs/PROJECT.md` holds the same numbers, and gives the fingerprint to set once the specification is up to date. No other test fails, because the tests have a balance of their own, `tanku_core/test.balance`, which doesn't move with the one of the game. To run them on the balance of the game all the same:

```bash
cargo test --workspace --features tanku_core/game-balance
```

A test that holds a number then fails where the two files differ, and the others say whether the rules still hold with the new numbers.

## Listening to the music

The music is specified in [docs/MUSIC.md](docs/MUSIC.md). The game plays it, on the desktop and on the web, and the settings have a volume for it. A tool also writes it to sound files, to hear an arrangement without playing up to it:

```bash
cargo run -p tanku_audio --example render
```

The files go to `target/music`, or to the directory given after `--`:

| File | Content |
|---|---|
| `intro.wav`, `bridge.wav` | Two turns of each loop |
| `corridor_threat_0.wav` to `corridor_threat_4.wav` | The verse and the chorus of a fight in a corridor, at each threat |
| `arena_threat_0.wav` to `arena_threat_4.wav` | The same in an arena |
| `tour.wav`, `tour.txt` | Three minutes of a made-up run, and what happens in it and when |
| `connections.wav`, `connections.txt` | Eight short fights that each end with the lead on another note, to hear every way from a fight into the bridge, and which is which |
| `cues.wav`, `cues.txt` | Every sound cue alone, then over the fullest music as loud as the game plays both with the settings it starts with: next to the player, then as far away as a cue gets. The list tells which is which. |

The piece is written in `tanku_audio/music.score`: its tempo, its key, its motifs, its patterns, its sections, what each instrument plays in each of them, how loud it is on each side and the settings of its sound, in a notation that [docs/MUSIC.md](docs/MUSIC.md#the-file) describes. The game is compiled with it, so the tool above renders a change of the file the next time it runs. A file that can't be read stops the build and says on which line. These are the checks that hold for any piece, to run after a change:

```bash
cargo test -p tanku_audio --test checks
```

In the game, the lead tells how smart the enemies are. To know which sectors of a seed to compare by ear, list the brains of its first thirty groups and what the lead plays for each:

```bash
TANKU_SEED=TANKU cargo test -p tanku_client threats_of_a_seed -- --ignored --nocapture
```

### Changing the music

The file can be changed by hand, or in an editor that shows it as motifs, patterns, sections, transitions and instruments, and plays it while it changes:

```bash
cargo run -p music_editor
```

It opens `tanku_audio/music.score`, or the file named after `--`. Every change is a change of a line of the file, which the reader of the game reads at once: a change that it refuses is not made, and the window says why in the reader's words. So the editor can't save a file that the game doesn't build with. It runs the checks above after every change, with a few of its own about the tempo, the cues and the cost of the sound. The game itself only changes when it is compiled again.

| Input | Effect |
|---|---|
| Space | Plays, or stops |
| Ctrl+Z | Undoes a change |
| Ctrl+Shift+Z or Ctrl+Y | Does it again |
| Ctrl+S | Saves the file |
| A button that moves a number | Goes on while it is held, and takes a larger step with Shift |
| The wheel of the mouse | Moves what the cursor is on, when it is longer than its place: the checks of a tab, the listing of the file |

[docs/MUSIC.md](docs/MUSIC.md#editor) says what each tab shows and what the editor leaves to the code.

Like the game, the editor runs from a script, without a window and without a sound. `MUSIC_EDITOR_SCRIPT` holds commands that show a tab, press a button, type, save what is drawn and keep what was played, which `music_editor/src/dev.rs` lists. Such a run only saves the piece if its file was named on the command line:

```bash
MUSIC_EDITOR_SCRIPT="tab patterns; wait 1; shot target/patterns.png; quit" cargo run -p music_editor
```

## License

Dual-licensed under MIT or Apache-2.0, at your option.

The fonts in `tanku_client/assets/fonts` are not ours and keep their own license, the SIL Open Font License 1.1, whose text is next to each of them: Barlow Semi Condensed, by The Barlow Project Authors, and Share Tech Mono, by Carrois Type Design. The game carries them in its binary.
