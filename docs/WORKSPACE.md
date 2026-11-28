# Workspace

This is how I lay out files on my machine. It is not a requirement for building Lumen. If you only run Cargo, you can ignore the extra directories.

`LUMEN_WORK` defaults to `$HOME/Code/workspaces/Lumen`. The assignment is in [`scripts/workspace.sh`](../scripts/workspace.sh). A non-empty value replaces that default. The path is rejected when it is a symlink, or when it is equal to, inside, or a parent of `~/.config`, `~/.var`, `~/.local`, `~/Code/repos`, or this repository. `$HOME` and `~/Code` are rejected because they contain `~/Code/repos`.

```text
~/Code/repos/yeelight-control/     this repository
~/Code/workspaces/Lumen/           my LUMEN_WORK
  cargo/toolbox/                   Toolbox Cargo output
  cargo/host/                      host Cargo output, when GTK is on the host
  flatpak/state/devel/             Flatpak state for the development build
  flatpak/state/release/           Flatpak state for the release build
  flatpak/build/                   build trees I can delete
  flatpak/repo/                    OSTree repositories
  flatpak/lumen-devel.flatpak      development bundle
  packages/vX.Y.Z/                 release bundles I keep
  packages/local/                  dirty test bundles only
  logs/                            simulator and Flatpak logs
```

```sh
./scripts/lumen dev
./scripts/lumen test
./scripts/lumen check
./scripts/lumen flatpak
./scripts/lumen release
./scripts/lumen clean
```

`clean` prints a dry-run. `--yes` deletes only listed generated paths: `target/`, `_build/`, and `.flatpak-builder` in the repository, plus `$LUMEN_WORK/flatpak/build` and `$LUMEN_WORK/flatpak/repo`. A `lumen-<version>.flatpak` in the repository root is deleted only when it is a regular file and byte-identical to the copy in `$LUMEN_WORK/packages/v<version>/`. `--cargo` adds `$LUMEN_WORK/cargo/toolbox` and `$LUMEN_WORK/cargo/host`, which stay otherwise. `--caches` adds the Flatpak state directories. `--logs` adds regular files in `logs/`. `packages/` is never deleted. A symlink is skipped rather than followed.

`./scripts/lumen bootstrap` recreates my `yeelight-dev` Toolbox from [build-aux/yeelight-dev.Containerfile](../build-aux/yeelight-dev.Containerfile). It does not install GTK on the host. How I run Cargo day to day is in [DEVELOPMENT.md](DEVELOPMENT.md). Flatpak export is in [PACKAGING.md](PACKAGING.md).
