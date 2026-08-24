project('neo', 'c', version: '0.1.0', meson_version: '>=1.9')

gtk_dep = dependency('gtk4')
adw_dep = dependency('libadwaita-1')

configure_file(
  input: 'data/org.neo.Neo.metainfo.xml',
  output: 'org.neo.Neo.metainfo.xml',
  copy: true,
)

rust_target = ''
target_platform = ''
build_deps = []

if meson.is_cross_build()
  cpu_family = host_machine.cpu_family()
  if cpu_family == 'aarch64'
    rust_target = 'aarch64-linux-android'
    target_platform = '//platforms:android-aarch64'
  elif cpu_family == 'x86_64'
    rust_target = 'x86_64-linux-android'
    target_platform = '//platforms:android-x86_64'
  else
    error('unsupported Android ABI: @0@'.format(cpu_family))
  endif

  build_deps += subproject('gtk').get_variable('libgtk')
  build_deps += subproject('libadwaita').get_variable('libadwaita')
endif

profile = get_option('buildtype') == 'release' ? 'release' : 'dev'
arelm_wrapper = find_program(
  get_option('arelm_root') / 'build-scripts' / 'buck2-build-cdylib.sh',
)

custom_target(
  'neo',
  output: 'libneo.so',
  console: true,
  build_always_stale: true,
  depends: build_deps,
  command: [
    arelm_wrapper,
    '--out', '@OUTPUT@',
    '--project-root', meson.project_source_root(),
    '--target', '//:neo-lib[cdylib]',
    '--target-platform', target_platform,
    '--rust-target', rust_target,
    '--profile', profile,
  ],
  install: true,
  install_dir: get_option('libdir'),
  install_tag: 'runtime',
)
