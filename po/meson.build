i18n = import('i18n')
# Compile and install po/*.mo. Extraction (QML as JavaScript plus Rust) is
# done by scripts/update_pot.py; the keywords match L10n/tr().
i18n.gettext('kiosque',
  args: ['--from-code=UTF-8', '--keyword=i18n:1', '--keyword=i18nc:1c,2', '--keyword=i18np:1,2'],
)
