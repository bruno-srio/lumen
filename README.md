# Lumen

Lumen is a native GNOME application for controlling Yeelight lights on the local network. It discovers bulbs with the official LAN protocol, shows each one as a card, and can turn them on or off, change brightness, and pick a color.

It does not use a Yeelight account, a cloud API, or an internet connection. LAN Control must be enabled on each light in the Yeelight mobile app.

Yeelight is a trademark of its owner. Lumen is an independent project and is not affiliated with Yeelight.

## What it does

- Finds lights with the Yeelight UDP discovery protocol, and also reconnects to lights it has seen before.
- Remembers names you choose. Names are stored by device id, so they survive IP address changes.
- Shows power, brightness, and color controls that match what each light actually supports.
- Keeps working when a light goes away, and reconnects when it comes back.
- Slows down slider and color changes so a drag does not hit the bulb's command limit.

Color temperature, groups, presets, and scenes are intentionally not in this version. The protocol code already knows about color temperature so it can be added later.

## Requirements

Fedora (or another distribution with GTK 4.20, libadwaita 1.8, and a Rust 1.85 toolchain):

```sh
sudo dnf install rust cargo clippy rustfmt gtk4-devel libadwaita-devel \
  meson blueprint-compiler desktop-file-utils appstream gettext gcc pkgconf
```

## Build and run

```sh
meson setup _build -Dprofile=development
meson compile -C _build
_build/src/lumen
```

The development profile uses the application id `io.github.bruno_srio.Lumen.Devel`, which keeps its settings separate from a release build.

Release build:

```sh
meson setup _build-release --buildtype=release -Dprofile=default
meson compile -C _build-release
```

## Try it without bulbs

`yeelight-sim` speaks the real protocol:

```sh
cargo run -p yeelight-sim
```

It prints a discovery port and one TCP address per virtual light. In another terminal:

```sh
cargo run -p yeelight --example discover
cargo run -p yeelight --example control -- 127.0.0.1:PORT bright 30
```

The simulator starts a color bulb, a white-only bulb, and a color-temperature bulb. See [docs/TESTING.md](docs/TESTING.md).

## Tests

```sh
cargo test -p yeelight -p yeelight-sim
```

## Data

Custom names and the last address of each light are stored in `$XDG_CONFIG_HOME/io.github.bruno_srio.Lumen/devices.json` (or the `.Devel` id for development builds). Window size is stored with GSettings.

## Security

The Yeelight protocol has no encryption and no authentication. Read [docs/SECURITY.md](docs/SECURITY.md) before using Lumen on a shared network.

## Packaging

A Flatpak manifest is in `build-aux/`. Building it needs the GNOME 51 SDK, the Freedesktop Rust SDK extension, and `build-aux/cargo-sources.json` generated from `Cargo.lock`. The process is described in [docs/PACKAGING.md](docs/PACKAGING.md).
