blueprint_compiler = find_program('blueprint-compiler', required: true)
compile_blueprints = find_program(meson.project_source_root() / 'build-aux/compile-blueprints.sh')

blueprints = custom_target(
  'blueprints',
  input: files(
    meson.project_source_root() / 'src/widgets/window.blp',
    meson.project_source_root() / 'src/widgets/device_card.blp',
    meson.project_source_root() / 'src/widgets/color_popover.blp',
    meson.project_source_root() / 'src/widgets/rename_dialog.blp',
    meson.project_source_root() / 'src/widgets/device_details.blp',
    meson.project_source_root() / 'src/widgets/group_card.blp',
    meson.project_source_root() / 'src/widgets/group_dialog.blp',
  ),
  output: ['window.ui', 'device_card.ui', 'color_popover.ui', 'rename_dialog.ui', 'device_details.ui', 'group_card.ui', 'group_dialog.ui'],
  command: [
    compile_blueprints,
    blueprint_compiler,
    meson.current_build_dir(),
    meson.project_source_root() / 'src/widgets',
  ],
)

resources = gnome.compile_resources(
  'lumen-resources',
  'resources.gresource.xml',
  source_dir: meson.project_source_root() / 'data/icons',
  dependencies: blueprints,
  gresource_bundle: true,
  install: true,
  install_dir: pkgdatadir,
)
