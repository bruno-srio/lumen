project(
  'lumen',
  'rust',
  version: '0.3.1',
  meson_version: '>= 1.1',
  license: 'GPL-3.0-or-later',
  default_options: ['warning_level=2'],
)

i18n = import('i18n')
gnome = import('gnome')

base_id = 'io.github.bruno_srio.Lumen'
if get_option('profile') == 'development'
  application_id = '@0@.Devel'.format(base_id)
else
  application_id = base_id
endif

prefix = get_option('prefix')
bindir = prefix / get_option('bindir')
datadir = prefix / get_option('datadir')
localedir = prefix / get_option('localedir')
pkgdatadir = datadir / meson.project_name()

cargo_env = environment()
cargo_env.set('APP_ID', application_id)
cargo_env.set('VERSION', meson.project_version())
cargo_env.set('PKGDATADIR', pkgdatadir)
cargo_env.set('LOCALEDIR', localedir)
cargo_env.set('LUMEN_SCHEMA_DIR', meson.project_build_root() / 'data')

subdir('data')
subdir('src')
subdir('po')

gnome.post_install(
  glib_compile_schemas: true,
  gtk_update_icon_cache: true,
  update_desktop_database: true,
)
