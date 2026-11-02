cargo_build = find_program(meson.project_source_root() / 'build-aux/cargo-build.sh')

profile = get_option('profile') == 'development' ? 'debug' : 'release'

custom_target(
  'lumen',
  build_by_default: true,
  build_always_stale: true,
  output: 'lumen',
  console: true,
  install: true,
  install_dir: bindir,
  depends: resources,
  env: cargo_env,
  command: [
    cargo_build,
    meson.project_build_root(),
    meson.project_source_root(),
    profile,
    '@OUTPUT@',
  ],
)
