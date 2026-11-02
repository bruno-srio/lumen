desktop_conf = configuration_data()
desktop_conf.set('APP_ID', application_id)
desktop_conf.set('BIN', bindir / 'lumen')

desktop_file = configure_file(
  input: 'io.github.bruno_srio.Lumen.desktop.in',
  output: '@0@.desktop'.format(application_id),
  configuration: desktop_conf,
)
install_data(desktop_file, install_dir: datadir / 'applications')

desktop_validate = find_program('desktop-file-validate', required: false)
if desktop_validate.found()
  test('validate-desktop', desktop_validate, args: [desktop_file])
endif

metainfo_conf = configuration_data()
metainfo_conf.set('APP_ID', application_id)

metainfo_file = configure_file(
  input: 'io.github.bruno_srio.Lumen.metainfo.xml.in',
  output: '@0@.metainfo.xml'.format(application_id),
  configuration: metainfo_conf,
)
install_data(metainfo_file, install_dir: datadir / 'metainfo')

appstreamcli = find_program('appstreamcli', required: false)
if appstreamcli.found()
  test(
    'validate-metainfo',
    appstreamcli,
    args: ['validate', '--no-net', '--explain', metainfo_file],
  )
endif

schema_conf = configuration_data()
schema_conf.set('APP_ID', application_id)
schema_conf.set('APP_PATH', '/' + application_id.replace('.', '/') + '/')

schema_file = configure_file(
  input: 'io.github.bruno_srio.Lumen.gschema.xml.in',
  output: '@0@.gschema.xml'.format(application_id),
  configuration: schema_conf,
)
install_data(schema_file, install_dir: datadir / 'glib-2.0/schemas')

glib_compile_schemas = find_program('glib-compile-schemas', required: true)
custom_target(
  'gschemas-compiled',
  input: schema_file,
  output: 'gschemas.compiled',
  command: [glib_compile_schemas, meson.current_build_dir()],
  build_by_default: true,
)

install_data(
  'icons/hicolor/scalable/apps/io.github.bruno_srio.Lumen.svg',
  install_dir: datadir / 'icons/hicolor/scalable/apps',
  rename: '@0@.svg'.format(application_id),
)
install_data(
  'icons/hicolor/symbolic/apps/io.github.bruno_srio.Lumen-symbolic.svg',
  install_dir: datadir / 'icons/hicolor/symbolic/apps',
  rename: '@0@-symbolic.svg'.format(application_id),
)

subdir('resources')
