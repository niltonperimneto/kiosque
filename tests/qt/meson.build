test_qt = dependency('qt6', modules: ['Test', 'QuickTest'])
test_moc = qt6.preprocess(moc_headers: ['test_setup.h'], moc_sources: ['models.cpp'])
test_deps = [qt_deps, test_qt, kirigami_dep, rust_dep, ki18n_dep, ki18n_qml_dep]
model_test = executable('kiosque-model-test', 'models.cpp', test_moc,
  dependencies: test_deps, link_with: rust_lib[0], objects: [rust_lib[1]])
quick_test = executable('kiosque-quick-test', 'quick.cpp', test_moc, qrc_files,
  dependencies: test_deps, link_with: rust_lib[0], objects: [rust_lib[1]],
  cpp_args: ['-DQUICK_TEST_SOURCE_DIR="' + meson.project_source_root() / 'tests/qml' + '"'])
test('models', model_test, suite: 'qt', timeout: 60)
test('interactions', quick_test, suite: 'qml', timeout: 90)
