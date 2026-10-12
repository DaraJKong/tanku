# Tanku

A cooperative top-down tank game for one to four players, built with Rust and Bevy. The players fight their way through an endless, procedurally generated corridor of enemy tanks, and one hit destroys a tank.

The solo game and co-op are playable, on Linux, on Windows and in a browser:

- **Solo:** the corridor, the generated enemies with their classes and their brains, arenas and their waves, lives and checkpoints, the menus, the sound cues and the music. Runs unlock achievements: the first one lets the player type the seed of a run. Clearing sector 20 unlocks free mode, in which the player spends the 59 points of their tank as they like, in a garage that draws the tank as they build it. Clearing sector 30 unlocks hard mode, whose enemies are stronger from the first sector.
- **Co-op:** up to four players. One of them hosts a game, which the others find on the local network or join by its address, from a desktop build or from a browser. A destroyed tank is a wreck that a teammate repairs.
- **Not in yet:** what a host sets for a game (its name, its password, its difficulty, free mode), and the ready check of the lobby. Until then, a game of several players is in normal mode and in the standard tank.

Co-op has been played by two players. With three or four, it is checked by tests and by scripted runs only. The build for Windows is checked under wine, not yet on Windows itself. The specification is in [docs/PROJECT.md](docs/PROJECT.md).

## Workspace

| Crate | Role |
|---|---|
| `tanku_core` | The rules both sides share: stats, progression, map and enemy generation, simulation types, and what goes over the network |
| `tanku_server` | The authoritative simulation: a library that the client embeds for solo and hosted games, and a headless binary |
| `tanku_client` | The game: rendering, input, interface and sound |
| `tanku_audio` | The sound that the game computes: the music and the waveforms of the sound cues, which the client plays |

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

1. The host picks "Host a game" in the main menu. The lobby shows the address of the machine on its network.
2. Each of the others picks "Join a game". A game that is hosted on the same network shows there, and a click joins it. One that doesn't show is joined by typing the address of its host.
3. The host starts the run once everyone is in the lobby. Nobody joins a run in progress.

A tank that is destroyed stays as a wreck, in a circle in dashes. It is in nobody's way: tanks drive over it and bullets fly over it. A teammate repairs it by driving into the circle and holding the repair key for two seconds, and can drive, shoot, lay mines and shield meanwhile: the circle is inked in as the repair goes on, and starts over if the key is let go or the tank leaves the circle. A tank repairs one wreck at a time: the nearest one whose circle it is in. The run is over when every tank is a wreck. A marker at the edge of the screen points to each teammate who is out of view, by the number of its player: full for a tank that runs, hollow for a wreck.

All players need the same version of the game. The game listens on UDP port 28405, which the firewall of the host's machine must let in. Games find each other by multicast DNS, on UDP port 5353, which the firewalls that systems ship with let through. Where it is kept out, only the list goes: the address still joins. To play over the internet, that port of the host's router must lead to the host's machine, and the others type the host's public address.

Two games on one machine can play together, which is the quickest way to try it: the second one finds the first on its join screen.

#### From a browser

The web build joins games too, and hosts none. A browser can't look around a network, and it only speaks to a game that shows it a certificate. A hosted game makes its own certificate, which a browser takes by its print alone. So a browser joins by a link that the host hands out:

1. The host picks "For a browser" in the lobby. The lobby then shows what the link ends with: a `#`, where the game takes browsers, and the print of its certificate. "Copy" puts it on the clipboard, and the button says whether it got there. The game also writes it to its log.
2. The player opens the page of the game with that at the end of its address, as in `http://localhost:8000/#192.168.1.20:28406/3f0c…`. The page joins the game at once. What the host copied can also be pasted into a page that is already open, with or without the address of the page in front of it.

What a host hands out stays the same from one game to the next, until the host quits the program. A player who left, or whose connection was lost, joins again from "Join a game".

A hosted game listens for browsers on UDP port 28406, which the firewall of the host's machine must let in. Over the internet, that port of the host's router must lead to the host's machine as well, and the host's public address goes into the link in place of the one that the lobby shows.

A browser only joins from a page that it loaded over HTTPS or from its own machine, such as `http://localhost:8000`. A page that another machine serves over plain HTTP joins nothing, and says so. The page stands still while its tab is hidden, and a game takes a player whose machine stays silent for 5 s for gone. Only Firefox was tried.

### Playtest

A playtest lets the player cheat, to test the game faster:

```bash
cargo run -p tanku_client -- --playtest
```

| Cheat | How |
|---|---|
| Infinite lives | On from the start. The pause menu turns them off and on. |
| Skip to a sector | The pause menu picks any sector up to 999. The tank comes back at the end of the sector before it, in front of its barrier, as it does after clearing that sector. |
| Choose the seed | `--seed TEXT` after `--playtest` starts every run on that seed instead of a random one. |
| Nothing locked | The solo screen takes a typed seed and offers hard mode and free mode, whatever the player has unlocked. |
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

```bash
cargo run -p tanku_client -- --playtest --seed GARAGE --tank engine=4,tracks=5,muzzle=7,magazine=4,loader=6
```

The main menu draws the tank and says how many of its points it spends. In a game of several players, the tank is the host's alone, and the others keep the standard one. A tank from the command line doesn't change the one of the garage.

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

The tests check the map generator over a dozen seeds. A wider check over a thousand seeds takes two to three minutes and is worth running after a change to the generator:

```bash
cargo test -p tanku_core many_seeds -- --ignored
```

The tests also lay every menu out, without a screen, in windows from 1280 by 720 pixels up. They fail if a button or a text crosses the fine line around the sheet, if something lies on it, or if a box lost one of its lines. After a change to a menu, this prints how far down each menu reaches in each of those windows, and where that line is:

```bash
cargo test -p tanku_client room_the_menus_take -- --ignored --nocapture
```

After a change to a brain, two tests compare a soldier, a veteran and an ace in the same five tanks. Each fights a crude player that nothing destroys, in about twenty sectors of six seeds: one tank alone, against bursts and against single shots, and two tanks together, against bursts. They print how long the enemies lasted and how many of their shots landed, and take six minutes:

```bash
cargo test -p tanku_server trials::brains -- --ignored --nocapture
```

After a change to what a mode makes of its enemies, two more tests send the same crude player against the groups of the first thirty sectors as they are, in normal mode and in hard mode. They print the same figures for each mode by stretch of sectors, and take nine minutes:

```bash
cargo test -p tanku_server trials::modes -- --ignored --nocapture
```

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

Two scripted games can play together: one hosts on a port, and the other joins it. A game that hosts on a port of its own takes browsers on the port after it.

```bash
TANKU_DEV_SCRIPT="host 28999; wait 6; press Start; wait 5; shot host.png; quit" cargo run -p tanku_client
```

```bash
TANKU_DEV_SCRIPT="wait 2; join 127.0.0.1:28999; wait 9; shot guest.png; quit" cargo run -p tanku_client
```

Two games on one machine talk over the loopback, where every address is the machine's own and nothing is late or lost. That hides what goes wrong between two machines. A script runs the two games as two machines on a private network, a host at `10.77.0.1` and a guest at `10.77.0.2`, and can worsen the line between them. A change to the network is checked with it:

```bash
tanku_client/dev/two_machines.sh "host; wait 6; press Start; wait 5; shot host.png; quit" "wait 2; join 10.77.0.1; wait 9; shot guest.png; quit" "delay 50ms 10ms loss 2%"
```

It takes Linux with user namespaces and needs no privileges. The third argument is optional: here it makes a round trip of 100 ms that loses one packet in fifty each way.

The guest can also find the game as a player does, instead of being told its address: `press Join a game; wait 3; press 10.77.0.1` opens the join screen and presses the row of the game that it lists. With `WALL=mdns` in front of the command, each machine has a firewall that lets in what the ones that systems ship with do: the game must still be found. With `WALL=shut`, the firewalls keep multicast DNS out as well: only the address joins.

A browser doesn't join as a desktop game does, so a second script runs a scripted host and a Firefox as two machines. The browser opens the web build with the link of the host at its end, and then does what its steps say: it waits, clicks, holds keys and saves what the page shows. The steps are listed at the top of `tanku_client/dev/firefox.py`.

```bash
tanku_client/dev/browser_guest.sh "host; wait 30; press Start; wait 14; shot host.png; quit" "wait:24 shot:lobby wait:14 click:683:600 key:w:2 shot:run" "delay 60ms 10ms loss 2%"
```

It takes Firefox, Python 3 and the web build in `target/web-dist`. No window opens. What the browser showed goes to `target/browser_guest`.

On a bad line, a host may log errors of `lightyear_debug::input` about an input for a tick that it has already simulated. They tell that the orders of a guest came late: its clock was behind, after the guest stalled for a moment or the line suddenly got slower. The host goes on with the last orders it had, the network library sets the clock of the guest anew within a second or two, and the errors stop. Nothing is lost: what the guest shows of its own tank is put right by what the host tells.

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

In the game, the lead tells how smart the enemies are. To know which sectors of a seed to compare by ear, list the brains of its first thirty groups and what the lead plays for each:

```bash
TANKU_SEED=TANKU cargo test -p tanku_client threats_of_a_seed -- --ignored --nocapture
```

## License

Dual-licensed under MIT or Apache-2.0, at your option.

The fonts in `tanku_client/assets/fonts` are not ours and keep their own license, the SIL Open Font License 1.1, whose text is next to each of them: Barlow Semi Condensed, by The Barlow Project Authors, and Share Tech Mono, by Carrois Type Design. The game carries them in its binary.
