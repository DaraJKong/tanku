# Tanku

A cooperative top-down tank game for one to four players, built with Rust and Bevy. The players fight their way through an endless, procedurally generated corridor of enemy tanks, and one hit destroys a tank.

The project is at its very beginning: the workspace builds and there is no game yet. The specification is in [docs/PROJECT.md](docs/PROJECT.md).

## Workspace

| Crate | Role |
|---|---|
| `tanku_core` | Library shared by the client and the server |
| `tanku_server` | The authoritative server: a library that the client embeds for solo and hosted games, and a headless binary |
| `tanku_client` | The game |

## Building

The toolchain is pinned to a nightly in `rust-toolchain.toml`, because development builds use the Cranelift backend and `-Zshare-generics` to compile faster. `rustup` installs it on first use. On Linux, linking needs `clang` and `mold`.

Run the game:

```bash
cargo run -p tanku_client
```

Run the headless server:

```bash
cargo run -p tanku_server
```

Development builds link Bevy dynamically. A release build leaves the development features out:

```bash
cargo build --release --no-default-features -p tanku_client
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

## License

Dual-licensed under MIT or Apache-2.0, at your option.
