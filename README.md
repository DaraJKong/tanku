# Tanku

A cooperative top-down tank game for one to four players, built with Rust and Bevy. The players fight their way through an endless, procedurally generated corridor of enemy tanks, and one hit destroys a tank.

The solo game is playable: the corridor, the generated enemies and their brains, lives and checkpoints, the menus, the sound cues and the music are in. Co-op has begun: a desktop build hosts a game that others join by its address. Achievements and the other modes are not in. The specification is in [docs/PROJECT.md](docs/PROJECT.md).

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

Settings and records are kept in the user's configuration directory, under `tanku`.

### With friends

Up to four players share a run. One of them hosts the game, and the others join it:

1. The host picks "Host a game" in the main menu. The lobby shows the address of the machine on its network.
2. Each of the others picks "Join a game" and types that address.
3. The host starts the run once everyone is in the lobby. Nobody joins a run in progress.

All players need the same version of the game. The game listens on UDP port 28405, which the firewall of the host's machine must let in. To play over the internet, that port of the host's router must lead to the host's machine, and the others type the host's public address.

Two games on one machine can play together, which is the quickest way to try it: the second one joins `127.0.0.1`.

The web build doesn't join games yet.

### Playtest

A playtest lets the player cheat, to test the game faster:

```bash
cargo run -p tanku_client -- --playtest
```

| Cheat | How |
|---|---|
| Infinite lives | On from the start. The pause menu turns them off and on. |
| Skip to a sector | The pause menu picks any sector up to 999. The tank comes back at the end of the sector before it, in front of its open barrier, as it does after capturing a flag there. |
| Choose the seed | `--seed TEXT` after `--playtest` starts every run on that seed instead of a random one. |

With both, a seed and a sector from a report are a few clicks away:

```bash
cargo run -p tanku_client -- --playtest --seed 85GVTHQA
```

A playtest sets no record. The web build has none, because a page has no command line.

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

Benchmarks measure generation, the simulation and the music, and take under a minute:

```bash
cargo bench -p tanku_core -p tanku_server -p tanku_audio
```

## Looking at the game without playing

Development builds read a script from `TANKU_DEV_SCRIPT`: commands separated by semicolons that start a run, jump to a sector, drive the tank, press the buttons of the menus and save screenshots. The commands are listed at the top of `tanku_client/src/dev.rs`.

A scripted run opens no window and needs no display: it draws into an image of 1920 by 1080 pixels, which the `shot` command saves. It makes no sound, starts from the default settings, and neither reads nor writes the saved settings and records.

```bash
TANKU_DEV_SCRIPT="start TANKU; wait 1; warp 12; wait 1; peek 0; wait 1; shot sector12.png; quit" cargo run -p tanku_client
```

Two scripted games can play together: one hosts on a port, and the other joins it.

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
