# Contributing to Lumen

Thanks for helping. Lumen is a small GNOME app. Keep changes focused and covered by a test when they touch the Yeelight protocol.

## Build

```sh
sudo dnf install rust cargo clippy rustfmt gtk4-devel libadwaita-devel \
  meson blueprint-compiler desktop-file-utils appstream gettext gcc pkgconf
meson setup _build -Dprofile=development
meson compile -C _build
_build/src/lumen
```

## Tests

```sh
cargo test -p yeelight -p yeelight-sim
cargo test -p lumen
cargo clippy -p yeelight -p yeelight-sim --all-targets -- -D warnings
cargo fmt --all --check
```

`cargo test -p lumen` needs the GTK packages from the build section. Without a display, run `xvfb-run cargo test -p lumen`.

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
