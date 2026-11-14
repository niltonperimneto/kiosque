project('kiosque', 'cpp',
  meson_version: '>=1.1.0',
  version: run_command('python3', '-c', '''
import subprocess
try:
    s = subprocess.run(["git", "describe", "--tags"], capture_output=True, text=True)
    version = s.stdout.strip().lstrip("v") if (s.returncode == 0 and s.stdout.strip()) else "0.8"
except Exception:
    version = "0.8"
print(version)
''', check: true).stdout().strip(),
  default_options: ['cpp_std=c++17']
)

qt6 = import('qt6')

# Dependencies
qt_deps = dependency('qt6', modules: ['Core', 'Gui', 'Qml', 'Quick', 'QuickControls2', 'DBus', 'Widgets'])
kirigami_dep = dependency('KF6Kirigami')
dbus_dep = dependency('dbus-1', required: false)
ki18n_dep = dependency('KF6I18n', method: 'cmake', modules: ['KF6::I18n'])
ki18n_qml_dep = dependency('KF6I18n', method: 'cmake', modules: ['KF6::I18nQml'])


python = find_program('python3')
is_release = get_option('buildtype') != 'debug'
cargo_profile = is_release ? 'release' : 'debug'
localedir_path = get_option('prefix') / get_option('localedir')
cargo_target_dir = get_option('cargo_target_dir') == '' ? meson.current_build_dir() / 'cargo_target' : get_option('cargo_target_dir')
rust_lib = custom_target(
  'cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: ['libkiosque.a', 'kiosque_initializers.o', 'kiosque-update'],
  console: true,
  install: true,
  install_dir: [false, false, get_option('bindir')],
  command: [python, files('scripts/build_rust.py'),
    '--manifest', meson.current_source_dir() / 'rust/Cargo.toml',
    '--target-dir', cargo_target_dir,
    '--profile', cargo_profile,
    '--backend', get_option('flatpak_backend'),
    '--test-support', get_option('test_support') ? 'true' : 'false',
    '--output', '@OUTPUT0@', '--initializers', '@OUTPUT1@', '--updater', '@OUTPUT2@',
    '--localedir', localedir_path]
)
backend_deps = []
if get_option('flatpak_backend') == 'native'
  backend_deps += dependency('flatpak')
endif
cpp = meson.get_compiler('cpp')
rust_dep = declare_dependency(dependencies: [backend_deps, cpp.find_library('dl'), cpp.find_library('pthread'), cpp.find_library('m')])

qrc_files = qt6.compile_resources(sources: 'resources.qrc')

subdir('po')


# Application executable
executable('kiosque',
  sources: ['src/main.cpp', qrc_files],
  link_with: rust_lib[0],
  objects: [rust_lib[1]],
  dependencies: [qt_deps, kirigami_dep, rust_dep, dbus_dep, ki18n_dep, ki18n_qml_dep],
  cpp_args: [
    '-DTRANSLATION_DOMAIN="kiosque"',
    '-DPROJECT_VERSION="' + meson.project_version() + '"',
    '-DLOCALE_INSTALL_DIR="' + localedir_path + '"'
  ],
  install: true
)

install_data('org.kiosque.Kiosque.desktop',
  install_dir: get_option('datadir') / 'applications'
)

install_data('qml/images/logo.svg',
  rename: 'org.kiosque.Kiosque.svg',
  install_dir: get_option('datadir') / 'icons/hicolor/scalable/apps'
)

gnome = import('gnome')
gnome.post_install(
  gtk_update_icon_cache: true,
  update_desktop_database: true
)


if get_option('tests')
  if not get_option('test_support')
    error('tests requires -Dtest_support=true')
  endif
  subdir('tests/qt')
endif
