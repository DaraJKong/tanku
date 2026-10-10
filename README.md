# Tanku

A cooperative top-down tank game for one to four players, built with Rust and Bevy. The players fight their way through an endless, procedurally generated corridor of enemy tanks, and one hit destroys a tank.

The solo game is playable: the corridor, the generated enemies and their brains, lives and checkpoints, the menus and the sound cues are in. Co-op, achievements and the other modes are not. The specification is in [docs/PROJECT.md](docs/PROJECT.md).

## Workspace

| Crate | Role |
|---|---|
| `tanku_core` | The rules both sides share: stats, progression, map and enemy generation, simulation types |
| `tanku_server` | The authoritative simulation: a library that the client embeds for solo and hosted games, and a headless binary |
| `tanku_client` | The game: rendering, input, interface and sound |
| `tanku_audio` | The sound that the game computes. For now it holds the music, which doesn't play in the game yet. |

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
| Turn the view | Q and E |
| Menu | Esc |

Settings and records are kept in the user's configuration directory, under `tanku`.

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

The web build keeps settings and records in the local storage of the browser.

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

## Listening to the music

The music is specified in [docs/MUSIC.md](docs/MUSIC.md). It doesn't play in the game yet: a tool writes it to sound files, to judge it by ear first.

```bash
cargo run -p tanku_audio --example render
```

The files go to `target/music`, or to the directory given after `--`. The mode of the piece is still to be chosen between two, so the piece is written once for each, with the name of the mode in front, as in `dorian_tour.wav` and `mixolydian_tour.wav`:

| File | Content |
|---|---|
| `intro.wav` | Two turns of the loop |
| `corridor_threat_0.wav` to `corridor_threat_4.wav` | The verse and the chorus of a fight in a corridor, at each threat |
| `tour.wav` | Three minutes of a made-up run |

The files that don't depend on that choice are written once:

| File | Content |
|---|---|
| `bridge.wav` | Two turns of the loop |
| `arena_threat_0.wav` to `arena_threat_4.wav` | The verse and the chorus of a fight in an arena, at each threat |
| `tour.txt` | What happens in the tour and when |

## License

Dual-licensed under MIT or Apache-2.0, at your option.

The fonts in `tanku_client/assets/fonts` are not ours and keep their own license, the SIL Open Font License 1.1, whose text is next to each of them: Barlow Semi Condensed, by The Barlow Project Authors, and Share Tech Mono, by Carrois Type Design. The game carries them in its binary.
