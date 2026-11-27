# Development

Lumen is a GTK 4 application. [`scripts/dev.sh`](../scripts/dev.sh) runs Cargo on the host when `pkg-config` can see `gtk4` and `libadwaita-1`. On this Fedora machine those libraries are not installed on the host, so the script uses the `yeelight-dev` Toolbox instead. Running `cargo` by hand on this host still fails at the link step:

```text
rust-lld: error: unable to find library -lgtk-4
rust-lld: error: unable to find library -lvulkan
rust-lld: error: unable to find library -lgraphene-1.0
rust-lld: error: unable to find library -ladwaita-1
```

`pkg-config` on the host also does not know `gtk4` or `libadwaita-1`. That is expected. Do not install GTK, libadwaita, or Vulkan on the host to work around it.

## Toolbox

The container used on this machine is `yeelight-dev`. Its base image is the `FROM` line in [build-aux/yeelight-dev.Containerfile](../build-aux/yeelight-dev.Containerfile), currently `fedora-toolbox:44`. Another container name can be selected with `LUMEN_TOOLBOX`. Confirm it before building:

```sh
toolbox list
podman container exists yeelight-dev && echo yeelight-dev
```

After a Fedora upgrade, recreate the container from that file. This replaces the container and does not delete Cargo output:

```sh
./scripts/dev.sh bootstrap
```

The first `./scripts/lumen dev` command after that compiles again into the Toolbox Cargo directory. Bump the `FROM` line when the Toolbox should match a newer Fedora.

These development packages are installed in the current container and in the image:

| Package | Version verified |
|---|---|
| rust, cargo, rustfmt, clippy | 1.99.1 |
| gcc | 16.2.1 |
| pkgconf | 2.5.1 |
| gtk4-devel | 4.22.5 |
| libadwaita-devel | 1.9.4 |
| graphene-devel | 1.10.8 |
| vulkan-loader-devel | 1.4.341.0 |
| blueprint-compiler | 0.20.4 |

`pkg-config` inside the container resolves `gtk4`, `libadwaita-1`, and `graphene-1.0`. `blueprint-compiler` is required because the interface is compiled while Cargo builds the app. The Meson and Flatpak instructions in [CONTRIBUTING.md](../CONTRIBUTING.md) also use `meson`, `desktop-file-utils`, `appstream`, and `gettext`; those are for that packaging path, not for `scripts/dev.sh`.

## Daily commands

[`scripts/lumen`](../scripts/lumen) is the command to remember. It calls [`scripts/dev.sh`](../scripts/dev.sh) for Cargo. If `pkg-config` sees `gtk4` and `libadwaita-1`, Cargo runs on the host and writes to `$LUMEN_WORK/cargo/host`. Otherwise Cargo runs in the Toolbox container named by `LUMEN_TOOLBOX` (default `yeelight-dev`) and writes to `$LUMEN_WORK/cargo/toolbox`. Those directories are never shared. It works from any directory.

`LUMEN_WORK` defaults to `$HOME/Code/workspaces/Lumen`. A non-empty value replaces that default:

```sh
LUMEN_WORK="$HOME/SomeOtherLocation/Lumen" ./scripts/lumen check
```

The default is set only in [`scripts/workspace.sh`](../scripts/workspace.sh). See [WORKSPACE.md](WORKSPACE.md).

```sh
./scripts/lumen dev           # build and launch Lumen, and start the simulator if it is not running
./scripts/lumen sim           # run only the simulator
./scripts/lumen status        # show whether the simulator and dev Lumen are running
./scripts/lumen stop          # stop a simulator left running
./scripts/lumen test          # protocol crate and the app, with a pass or fail summary
./scripts/lumen test parse_probe_address
./scripts/lumen check
./scripts/lumen fmt
./scripts/lumen fmt --check
./scripts/lumen bootstrap     # recreate the Toolbox after a Fedora upgrade
```

`./scripts/dev.sh` accepts the same Cargo commands. `fmt` does not create `LUMEN_WORK`.

A simulator left running is found by the installed app on refresh. `./scripts/dev.sh status` shows it, and `./scripts/dev.sh stop` ends it. The script says when it starts a simulator and when that simulator stops, including if the terminal is closed.

In Cursor, the same commands are tasks: Run Lumen Development Build, Run Lumen Tests, Check Lumen Compilation, and Check Rust Formatting. Build Lumen Development Flatpak and Preview Lumen Cleanup call `scripts/lumen`. The tasks do not embed a workspace path.

The script exits with Cargo's status code. If GTK is missing and Toolbox is missing, it exits 127. If Toolbox is installed but the named container does not exist, it exits 1. `bootstrap` is the command that builds the image and replaces the container. The other commands do not install packages or create a container.

## Build cache

`toolbox run` does not forward environment variables, so the Toolbox path sets `CARGO_TARGET_DIR` inside the container to `$LUMEN_WORK/cargo/toolbox`. A host that already has GTK uses `$LUMEN_WORK/cargo/host`. Sharing one directory is what makes a host link fail after a Toolbox build: the recorded libraries (`-lgtk-4` and the others) exist only inside the container.

The first Toolbox run compiles that directory from scratch. Later Toolbox runs reuse it. An older cache may still sit in `target/yeelight-dev` inside the repository. Flatpak and Meson builds keep the separate target directory in [build-aux/cargo-build.sh](../build-aux/cargo-build.sh). CI does not use `LUMEN_WORK`.

## When a command fails

- `unable to find library -lgtk-4` (or vulkan, graphene, adwaita): Cargo ran on this host directly. Use `./scripts/dev.sh`, which will enter the Toolbox.
- `gtk4 and libadwaita-1 are not visible to pkg-config, and Toolbox is not available`: install those libraries on that system, or install Toolbox and create a container.
- `Toolbox container 'yeelight-dev' does not exist`: set `LUMEN_TOOLBOX` to an existing container, or run `./scripts/dev.sh bootstrap`. That builds the image and replaces the container. It does not install packages on the host and does not delete Cargo artifacts.
- `blueprint-compiler is required`: the container is missing `blueprint-compiler`. The package is already present in the current `yeelight-dev`.
