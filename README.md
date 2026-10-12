# Tanku

A cooperative top-down tank game for one to four players, built with Rust and Bevy. The players fight their way through an endless, procedurally generated corridor of enemy tanks, and one hit destroys a tank.

The solo game and co-op are playable, on Linux, on Windows and in a browser:

- **Solo:** the corridor, the generated enemies with their classes and their brains, arenas and their waves, lives and checkpoints, the menus, the sound cues and the music. Runs unlock achievements. Clearing sector 5 unlocks arena mode, a run in one room against one group after another. Clearing sector 10 lets the player type the seed of a run. Clearing sector 20 unlocks free mode, in which the player spends the 59 points of their tank as they like, in a garage that draws the tank as they build it. Clearing sector 30 unlocks hard mode, whose enemies are stronger from the first sector.
- **Co-op:** up to four players. One of them hosts a game, which the others find on the local network or join by its address, from a desktop build or from a browser. A game can have a name and a password. The host sets the run in the lobby: its difficulty, its seed, arena mode, and free mode, in which each player has the tank of their own garage. Nothing is locked for a host who has company. The run starts once everyone has said that they are ready. A destroyed tank is a wreck that a teammate repairs.

Co-op has been played by two players. With three or four, it is checked by tests and by scripted runs only. Arena mode is new and an experiment: its rules are still being set in play (see [Arena mode](#arena-mode)). The build for Windows is checked under wine, not yet on Windows itself. The specification is in [docs/PROJECT.md](docs/PROJECT.md).

## Workspace

| Crate | Role |
|---|---|
| `tanku_core` | The rules both sides share: stats, progression, map and enemy generation, simulation types, and what goes over the network |
| `tanku_server` | The authoritative simulation: a library that the client embeds for solo and hosted games, and a headless binary |
| `tanku_client` | The game: rendering, input, interface and sound |
| `tanku_audio` | The sound that the game computes: the music and the waveforms of the sound cues, which the client plays |
| `music_editor` | A window that changes the music: it opens the file of the piece, plays it as the game does and saves it. The game doesn't need it. |

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

All players need the same version of the game. The game listens on UDP port 28405, which the firewall of the host's machine must let in. Games find each other by multicast DNS, on UDP port 5353, which the firewalls that systems ship with let through. Where it is kept out, only the list goes: the address still joins. To play over the internet, that port of the host's router must lead to the host's machine, and the others type the host's public address.

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

Arena mode is an option of a run, as free mode is. A single player unlocks it by clearing sector 5 and turns it on on the solo screen, and the host of a game sets it in the lobby. A player who had earned an achievement before arena mode existed had cleared that sector already, and has it.

A run in arena mode has no corridor. It is fought in one room, which the seed draws, against one group after another:

- **Groups.** A group is the group of the sector of its number, for as many players. Half of it comes when the lull is over, rounded up: as many tanks as a wave of an arena of the corridor has. From then on, a tank comes for each one that is destroyed, so that the room stays as full for as long as a tank waits. The place of each tank is marked for a second before the tank appears, as in an arena. A tank whose place has a player within 4 m, or a tank of its group on it, takes another place that has room, so that no tank waits for a player to move.
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
| Nothing locked | The solo screen takes a typed seed and offers hard mode, arena mode and free mode, whatever the player has unlocked. |
| Bring a tank | `--tank TANK` after `--playtest` starts every run in free mode, in that tank, without a visit to the garage: see [The garage check](#the-garage-check). |

With a seed and a skip, a seed and a sector from a report are a few clicks away:

```bash
cargo run -p tanku_client -- --playtest --seed 85GVTHQA
```

A playtest sets no record and earns no achievement. The web build has none, because a page has no command line.

#### The garage check

Free mode lets a player spend the 59 points of their tank as they like. A playtest takes such a tank from the command line, to find out whether one of them makes the game too easy: the same seed is played in the standard tank and in a few extreme ones, each a command away.

A tank is written as its stats with their levels, with commas between them. A stat that isn't named is at level 0. The tank needs an engine, costs 59 points at most and may leave points over: the game says why it refuses a tank. `--help` lists the stats and what their levels cost, and [the specification](docs/PROJECT.md#hardware-stats) says what each level gives.

| Tank | What it spends its points on | Written as |
|---|---|---|
| Standard | A bit of everything | `engine=4,tracks=5,muzzle=2,magazine=4,loader=6,ricochet=1,mines=2,shield=2` |
| Gun | Bullets at 19 m/s, and neither a bounce, mines nor a shield | `engine=4,tracks=5,muzzle=7,magazine=4,loader=6` |
| Bouncer | Piercing bullets that bounce three times | `engine=4,tracks=4,muzzle=3,magazine=3,loader=5,ricochet=3,piercing=1` |
| Runner | 8 m/s, and a shield every 6 s | `engine=7,tracks=6,muzzle=2,magazine=2,loader=5,shield=5` |
| Seeker | Seeking bullets that bounce twice | `engine=4,tracks=5,muzzle=2,magazine=4,loader=6,ricochet=2,seeking=1` |
| Flanker | Splitting bullets whose halves bounce once, and a mine | `engine=4,tracks=5,muzzle=2,magazine=4,loader=6,ricochet=1,splitting=1,mines=1` |
| Grenadier | Exploding bullets, and a shield every 10 s | `engine=4,tracks=5,muzzle=2,magazine=4,loader=6,exploding=1,shield=1` |

A tank fires one type of bullet: `piercing`, `seeking`, `splitting` and `exploding` are four stats of one level, and the game refuses a tank that has two of them.

```bash
cargo run -p tanku_client -- --playtest --seed GARAGE --tank engine=4,tracks=5,muzzle=7,magazine=4,loader=6
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

This prints what the groups of each mode are made of, by stretch of sectors: which brains they have, and how many points they can't spend:

```bash
cargo test -p tanku_core what_the_groups_are_made_of -- --ignored --nocapture
```

Benchmarks measure generation, the simulation and the music, and take under a minute:

```bash
cargo bench -p tanku_core -p tanku_server -p tanku_audio
```

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

The command `tape` keeps the music that the run would have played from there on. When the script quits, the music goes to a sound file, and the list of what the game told the music, with the time of each change, to a text file of the same name:

```bash
TANKU_DEV_SCRIPT="tape run.wav; wait 4; start TANKU; wait 4; play 0 1 0 0 4; wait 8; clear; wait 8; quit" cargo run -p tanku_client
```

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
