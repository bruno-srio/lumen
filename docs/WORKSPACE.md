# Workspace

Source stays in the Git repository. Generated files go under `LUMEN_WORK`.

```text
~/Code/repos/yeelight-control/     this repository
~/Code/workspaces/Lumen/           default LUMEN_WORK
  cargo/toolbox/                   Toolbox Cargo output
  cargo/host/                      host Cargo output, when GTK is on the host
  flatpak/state/devel/             Flatpak state for the development build
  flatpak/state/release/           Flatpak state for the release build
  flatpak/build/                   disposable build trees
  flatpak/repo/                    OSTree repositories
  flatpak/lumen-devel.flatpak      development bundle
  packages/vX.Y.Z/                 kept release bundles
  packages/local/                  dirty test bundles only
  logs/                            simulator and Flatpak logs
```

The default is set in [`scripts/workspace.sh`](../scripts/workspace.sh):

```sh
LUMEN_WORK="${LUMEN_WORK:-$HOME/Code/workspaces/Lumen}"
```

A non-empty `LUMEN_WORK` always wins. The path is rejected when it is a symlink, or when it is equal to, inside, or an ancestor of `~/.config`, `~/.var`, `~/.local`, `~/Code/repos`, or this repository. `$HOME` and `~/Code` are rejected for that reason.

```sh
./scripts/lumen dev
./scripts/lumen test
./scripts/lumen check
./scripts/lumen flatpak
./scripts/lumen release
./scripts/lumen clean
```

`clean` prints a dry-run. `--yes` deletes only the listed generated directories: `target/`, `_build/`, and `.flatpak-builder` inside the repository, plus `$LUMEN_WORK/flatpak/build` and `$LUMEN_WORK/flatpak/repo`. A `lumen-<version>.flatpak` in the repository root is deleted only when it is a regular file and byte-identical to `$LUMEN_WORK/packages/v<version>/`. `$LUMEN_WORK/cargo` stays unless `--cargo` is passed. Logs stay unless `--logs` is passed. Flatpak state stays unless `--caches` is passed. `packages/` is never deleted. A symlink is skipped rather than followed.

`./scripts/lumen bootstrap` recreates the `yeelight-dev` Toolbox from [build-aux/yeelight-dev.Containerfile](../build-aux/yeelight-dev.Containerfile). It does not install GTK on the host. Daily Cargo commands and the Toolbox choice are described in [DEVELOPMENT.md](DEVELOPMENT.md). Flatpak export is described in [PACKAGING.md](PACKAGING.md).
