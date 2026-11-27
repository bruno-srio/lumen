# Contributing to Lumen

Thanks for helping. Lumen is a small GNOME app, and I maintain it on my own. Keep a change focused. If you touch the Yeelight protocol, add a test.

## Get the code

```sh
git clone https://github.com/bruno-srio/lumen.git
cd lumen
```

You need Rust 1.87 or newer, GTK 4.20 or newer, libadwaita 1.8 or newer, and `blueprint-compiler` if you compile the interface. `pkg-config` has to see `gtk4` and `libadwaita-1`.

## Run it

```sh
cargo test --workspace
cargo run -p lumen
```

`./scripts/lumen` does the same Cargo commands through a small helper. I use it because my Fedora host does not have the GTK development libraries. That Toolbox setup is mine. You do not need it when the libraries are already installed. The details are in [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md).

Without bulbs:

```sh
cargo run -p yeelight-sim
```

## Checks

```sh
cargo fmt --all --check
cargo clippy -p yeelight -p yeelight-sim --all-targets -- -D warnings
cargo test --workspace
```

CI runs the protocol tests, the app tests, and a development Flatpak build on pull requests. Without a display, the app tests can be run with `xvfb-run cargo test -p lumen` where GTK is installed.

## Bugs, ideas, and patches

Open a GitHub issue for a bug or an idea.

For a code change, use a branch and open a pull request. CI should be green before it is merged. After the merge, GitHub shows **Delete branch**. Use it, so merged branches do not stay on the remote.

You do not need to use the same editor I use.
