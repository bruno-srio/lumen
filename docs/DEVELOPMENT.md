# Development

You need Rust 1.87 or newer, GTK 4.20 or newer, libadwaita 1.8 or newer, and `blueprint-compiler`. `pkg-config` has to see `gtk4` and `libadwaita-1`.

```sh
cargo test --workspace
cargo run -p lumen
```

`./scripts/lumen` runs those Cargo commands for me. `dev` builds and opens the app, and starts the simulator if it is not already running. `test`, `check`, `fmt`, `sim`, `status`, and `stop` do what they say. `fmt` does not create a workspace directory.

```sh
./scripts/lumen dev
./scripts/lumen test
./scripts/lumen check
./scripts/lumen fmt --check
./scripts/lumen sim
```

If the libraries are on your machine, `./scripts/lumen` runs Cargo there and writes build output to `$LUMEN_WORK/cargo/host`. If they are not, it looks for a Toolbox container. The default name is `yeelight-dev`. Set `LUMEN_TOOLBOX` to use another one. Toolbox builds go to `$LUMEN_WORK/cargo/toolbox`. Those two directories are separate on purpose: a Toolbox build records library paths that the host linker cannot see.

`LUMEN_WORK` defaults to `$HOME/Code/workspaces/Lumen`. That default is only in `scripts/workspace.sh`. Set the variable to put the files somewhere else:

```sh
LUMEN_WORK="$HOME/SomeOtherLocation/Lumen" ./scripts/lumen check
```

The script exits with Cargo's status. If GTK is missing and Toolbox is missing, it exits 127. If Toolbox is installed but the container does not exist, it exits 1.

Without a display, run `xvfb-run cargo test -p lumen` where the libraries are installed.

To try the app without bulbs, run `cargo run -p yeelight-sim` in another terminal, then start Lumen and search again. A simulator left running is found on refresh. `./scripts/lumen stop` ends one that the script started. See [TESTING.md](TESTING.md) for the simulator flags.

## On my Fedora machine

I do not install GTK, libadwaita, or Vulkan on the host. `pkg-config` there does not know `gtk4` or `libadwaita-1`, and a host `cargo run -p lumen` fails with `unable to find library -lgtk-4` (and vulkan, graphene, adwaita). `./scripts/lumen` uses the `yeelight-dev` Toolbox instead.

The container image is [build-aux/yeelight-dev.Containerfile](../build-aux/yeelight-dev.Containerfile). The `FROM` line is `fedora-toolbox:44`. After a Fedora upgrade I recreate the container. That replaces the container and does not delete Cargo output:

```sh
./scripts/lumen bootstrap
```

The next `./scripts/lumen dev` compiles again into `$LUMEN_WORK/cargo/toolbox`.

Packages in that container, as I last checked them: rust, cargo, rustfmt, and clippy 1.99.1; gcc 16.2.1; pkgconf 2.5.1; gtk4-devel 4.22.5; libadwaita-devel 1.9.4; graphene-devel 1.10.8; vulkan-loader-devel 1.4.341.0; blueprint-compiler 0.20.4. `blueprint-compiler` is required because the interface is compiled while Cargo builds the app. Those versions are a snapshot of this container, not a support matrix.

`toolbox run` does not forward environment variables, so the script sets `CARGO_TARGET_DIR` inside the container. An older cache may still sit in `target/yeelight-dev` in the repository. Flatpak and Meson keep their own target directory in [build-aux/cargo-build.sh](../build-aux/cargo-build.sh). CI does not use `LUMEN_WORK`.

Cursor tasks call the same scripts. They do not hardcode a workspace path.
