# Packaging

Lumen is meant to be installed as a Flatpak. The manifests live in `build-aux/`.

## Local Flatpak build

Install the SDK once. The GNOME 51 runtime is based on Freedesktop SDK 26.08, so the Rust extension uses that branch:

```sh
flatpak install -y flathub org.gnome.Sdk//51 org.gnome.Platform//51 \
  org.freedesktop.Sdk.Extension.rust-stable//26.08 org.flatpak.Builder
```

Regenerate the Cargo vendor file whenever `Cargo.lock` changes:

```sh
flatpak-cargo-generator Cargo.lock -o build-aux/cargo-sources.json
```

`flatpak-cargo-generator` is the script from [flatpak-builder-tools](https://github.com/flatpak/flatpak-builder-tools/tree/master/cargo). It can also be installed with `pipx install flatpak-cargo-generator`.

Build the development manifest, which uses a separate application id:

```sh
flatpak run org.flatpak.Builder --force-clean --user --install \
  _build/flatpak build-aux/io.github.bruno_srio.Lumen.Devel.json
flatpak run io.github.bruno_srio.Lumen.Devel
```

The release manifest is `build-aux/io.github.bruno_srio.Lumen.json`. Both manifests point at this source tree so they can be built locally. The Flathub submission replaces the `dir` source with the `v0.2.0` GitHub archive and its `sha256`, and commits a fresh `cargo-sources.json` beside that manifest.

Check the result:

```sh
flatpak run --command=flatpak-builder-lint org.flatpak.Builder \
  manifest build-aux/io.github.bruno_srio.Lumen.json
flatpak run --command=flatpak-builder-lint org.flatpak.Builder \
  appstream data/io.github.bruno_srio.Lumen.metainfo.xml.in
```

## Permissions

The finish arguments are:

- `--share=network` — UDP multicast discovery and TCP to bulbs. There is no Flatpak permission for "LAN only"; Lumen enforces that itself. See [SECURITY.md](SECURITY.md).
- `--share=ipc` and `--socket=wayland` — normal GTK display access.
- `--socket=fallback-x11` — X11 sessions other than GNOME.
- `--device=dri` — GPU rendering.

Nothing else is requested. In the Flathub pull request, explain `--share=network` in those terms.

## Flathub

Flathub expects a public repository, a stable application id, tagged releases, and a metainfo file whose id matches the manifest. The id `io.github.bruno_srio.Lumen` is tied to the GitHub account `bruno-srio`. The name and icon must not use the Yeelight trademark; the summary may say that the app controls Yeelight lights.

Submission is a pull request to the `flathub/flathub` repository's `new-pr` branch. Reviewers build the manifest, run the linter, and approve the network permission separately from code review. After it is accepted, updates are pull requests to `flathub/io.github.bruno_srio.Lumen`.

Flathub also asks for a visible development history, so the first submission should wait until the application has had a few real releases.

## Host packages

A distribution package can build with Meson. The runtime needs GTK 4.20 or newer and libadwaita 1.8 or newer. Configuration belongs in the XDG config directory; do not add a custom storage path.
