# Thin driver: Cargo builds everything; Meson installs it, builds
# translations and runs the Qt Quick tests. No compiled language is used.
project('kiosque',
  meson_version: '>=1.1.0',
  version: run_command('python3', '-c', '''
import subprocess
try:
    s = subprocess.run(["git", "describe", "--tags"], capture_output=True, text=True)
    version = s.stdout.strip().lstrip("v") if (s.returncode == 0 and s.stdout.strip()) else "1.0.0"
except Exception:
    version = "1.0.0"
print(version)
''', check: true).stdout().strip(),
)

python = find_program('python3')
is_release = get_option('buildtype') != 'debug'
bindir = get_option('bindir')
localedir_path = get_option('prefix') / get_option('localedir')
cargo_target_dir = get_option('cargo_target_dir') == '' ? meson.current_build_dir() / 'cargo_target' : get_option('cargo_target_dir')

if get_option('tests') and not get_option('test_support')
  error('tests requires -Dtest_support=true')
endif

binaries = ['kiosque', 'kiosque-update']
install_dirs = [bindir, bindir]
if get_option('tests')
  # The Qt Quick Test runner is built for tests only and never installed.
  binaries += 'kiosque-quick-test'
  install_dirs += false
endif

rust = custom_target(
  'cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: binaries,
  console: true,
  install: true,
  install_dir: install_dirs,
  command: [python, files('scripts/build_rust.py'),
    '--manifest', meson.current_source_dir() / 'rust/Cargo.toml',
    '--target-dir', cargo_target_dir,
    '--profile', is_release ? 'release' : 'debug',
    '--backend', get_option('flatpak_backend'),
    '--test-support', get_option('test_support') ? 'true' : 'false',
    '--version', meson.project_version(),
    '--localedir', localedir_path,
    '--out-dir', '@OUTDIR@',
    '--bins', binaries],
)

subdir('po')

install_data('org.kiosque.Kiosque.desktop',
  install_dir: get_option('datadir') / 'applications'
)
install_data('org.kiosque.Kiosque.metainfo.xml',
  install_dir: get_option('datadir') / 'metainfo'
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
  quick_test = rust[2]
  test_env = {'KIOSQUE_QML_TEST_DIR': meson.current_source_dir() / 'tests/qml'}
  test('models', quick_test, suite: 'qt', timeout: 90, env: test_env,
    args: ['-input', meson.current_source_dir() / 'tests/qml/tst_models.qml'])
  foreach t : ['navigation', 'services']
    test(t, quick_test, suite: 'qml', timeout: 120, env: test_env,
      args: ['-input', meson.current_source_dir() / 'tests/qml/tst_' + t + '.qml'])
  endforeach
endif
