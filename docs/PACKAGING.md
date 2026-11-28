# Packaging

I ship Lumen as a Flatpak. The local manifests are in `build-aux/`. They were drafted with Cursor, so they are not the manifest Flathub should build. I will write that one myself. See [AI_USAGE.md](AI_USAGE.md).

## What I run

Install the SDK once. The GNOME 51 runtime is based on Freedesktop SDK 26.08, so the Rust extension uses that branch:

```sh
flatpak install -y flathub org.gnome.Sdk//51 org.gnome.Platform//51 \
  org.freedesktop.Sdk.Extension.rust-stable//26.08 org.flatpak.Builder
```

When `Cargo.lock` changes, regenerate the vendor file:

```sh
flatpak-cargo-generator Cargo.lock -o build-aux/cargo-sources.json
```

`flatpak-cargo-generator` is the script from [flatpak-builder-tools](https://github.com/flatpak/flatpak-builder-tools/tree/master/cargo). I install it with `pipx install flatpak-cargo-generator`.

A development bundle:

```sh
./scripts/lumen flatpak
```

That builds `io.github.bruno_srio.Lumen.Devel` and writes `$LUMEN_WORK/flatpak/lumen-devel.flatpak`. The bundle is branch `stable` and names Flathub as the place to get `org.gnome.Platform//51`. A finished build directory is not the package. `flatpak-builder` has to finish, then `flatpak build-bundle` writes the file.

A release bundle:

```sh
./scripts/lumen release
```

This refuses to start unless the Git tree is clean, Cargo, Meson, and the newest AppStream release agree, and `v<version>` points at `HEAD`. It runs the tests, then writes `$LUMEN_WORK/packages/v<version>/lumen-<version>.flatpak`. It does not tag, push, or publish. `--allow-dirty` is off unless you pass it. It warns that the package may contain unreleased changes and writes `$LUMEN_WORK/packages/local/` instead.

Neither command hands the working tree to `flatpak-builder`. The committed manifests use a directory source, which would copy ignored files such as `target/`. A release is staged with `git archive` of the tag. A development build, and `--allow-dirty`, stage `git archive HEAD` plus tracked edits and untracked files that Git does not ignore.

Development and release each have their own `state/`, `build/`, and `repo/` directories under `$LUMEN_WORK/flatpak`. `flatpak-builder --state-dir` stores downloads and build caches, so I do not share one state directory between them. The command stops if a state directory and its build directory are on different filesystems. Where those directories sit on my machine is in [WORKSPACE.md](WORKSPACE.md).

Then I lint:

```sh
flatpak run --command=flatpak-builder-lint org.flatpak.Builder \
  manifest build-aux/io.github.bruno_srio.Lumen.json
flatpak run --command=flatpak-builder-lint org.flatpak.Builder \
  appstream data/io.github.bruno_srio.Lumen.metainfo.xml.in
```

## Screenshots in AppStream

AppStream uses the images in [data/screenshots/flathub/](../data/screenshots/flathub/). The README uses the single compact `data/screenshots/lights.png`, which is not listed in the metainfo.

Each `<image>` in [data/io.github.bruno_srio.Lumen.metainfo.xml.in](../data/io.github.bruno_srio.Lumen.metainfo.xml.in) points at the release tag:

```text
https://raw.githubusercontent.com/bruno-srio/lumen/v<version>/data/screenshots/flathub/<file>.png
```

Those URLs return 404 until the `v<version>` tag is pushed. That is expected before a release. Do not point them back at an older tag, and do not use a local path. After pushing the tag, open each URL once to confirm it loads.

If `appstreamcli` is installed, run `appstreamcli validate` on the metainfo file. This is not a Flathub submission.

## Permissions

The finish arguments are:

- `--share=network` — UDP multicast discovery and TCP to bulbs. There is no Flatpak permission for "LAN only". Lumen enforces that itself. See [SECURITY.md](SECURITY.md).
- `--share=ipc` and `--socket=wayland` — normal GTK display access.
- `--socket=fallback-x11` — X11 sessions other than GNOME.
- `--device=dri` — GPU rendering.

Nothing else is requested. When I write the Flathub pull request, I will explain `--share=network` in those terms myself.

## Flathub

Flathub wants a public repository, a stable application id, tagged releases, and a metainfo file whose id matches the manifest. The id `io.github.bruno_srio.Lumen` is tied to the GitHub account `bruno-srio`. The name and icon must not use the Yeelight trademark. The summary may say that the app controls Yeelight lights.

The local release manifest is `build-aux/io.github.bruno_srio.Lumen.json`. The Flathub checkout I have still builds the `v0.2.0` archive. Updating that checkout is a later submission, and I will write the manifest and the pull request myself. Flathub's current generative AI rules are linked from [AI_USAGE.md](AI_USAGE.md).

A distribution package can also build with Meson. The runtime needs GTK 4.20 or newer and libadwaita 1.8 or newer. Configuration belongs in the XDG config directory.
