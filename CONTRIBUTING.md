# Contributing to Lumen

Thanks for helping. Lumen is a small GNOME app. Keep changes focused and covered by a test when they touch the Yeelight protocol.

## Pull requests

Do new work on a branch and open a pull request. CI runs the protocol tests, the app tests, and the development Flatpak build on that request. Merge only after those checks pass.

GitHub shows **Delete branch** as soon as the pull request is merged. Use it. Merged branches, including a release branch after that version is on `main`, do not stay on the remote.

## Building

Contributors with GTK 4.20 or newer and libadwaita 1.8 or newer:

```sh
cargo test --workspace
cargo run -p lumen
```

On Fedora, those libraries can stay inside the `yeelight-dev` Toolbox. `./scripts/dev.sh` uses that container when the host does not have them. After a Fedora upgrade, run `./scripts/dev.sh bootstrap` to recreate it. Details are in [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md).

## Build

Day-to-day builds and tests go through [scripts/dev.sh](scripts/dev.sh). It runs Cargo on the host when GTK is visible to pkg-config, and otherwise in the `yeelight-dev` Toolbox. See [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md).

A Meson build, used by Flatpak, still needs the toolchain on the machine that runs Meson:

```sh
sudo dnf install rust cargo clippy rustfmt gtk4-devel libadwaita-devel \
  meson blueprint-compiler desktop-file-utils appstream gettext gcc pkgconf
meson setup _build -Dprofile=development
meson compile -C _build
_build/src/lumen
```

## Tests

```sh
./scripts/dev.sh test
./scripts/dev.sh fmt --check
cargo clippy -p yeelight -p yeelight-sim --all-targets -- -D warnings
```

`./scripts/dev.sh test` runs the protocol crate and the app. On a machine where pkg-config can see GTK it runs on the host; otherwise it runs inside the Toolbox. Without a display, the app tests can also be run with `xvfb-run cargo test -p lumen` where those libraries are installed.

`yeelight-sim` speaks the real LAN protocol, so most work does not need a bulb:

```sh
cargo run -p yeelight-sim
```

## Translations

Strings live in Rust (`gettext`), Blueprint files (`_()`), the desktop entry, and the metainfo file. They are listed in `po/POTFILES.in`.

```sh
meson setup _build -Dprofile=default
meson compile -C _build lumen-pot
# then edit po/<lang>.po and add the language to po/LINGUAS
```

Brazilian Portuguese (`pt_BR`) is the first translation and a good template.

## Protocol changes

Add unit tests next to the code in `crates/yeelight` and, when a bulb conversation changes, an integration test in `crates/yeelight/tests/session.rs`. Do not assume every bulb supports every command. Check the `support` list.
