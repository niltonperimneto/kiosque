#!/usr/bin/env python3
"""Regenerate po/kiosque.pot from QML and Rust sources.

QML calls `L10n.i18n/i18nc/i18np(...)` and Rust calls
`tr().i18n/i18nc/i18np(...)`; both are extracted with the keywords
`i18n:1`, `i18nc:1c,2` and `i18np:1,2`. `--merge` also updates every
po/<lang>.po with msgmerge.
"""
import argparse
import re
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
KEYWORDS = ['--keyword=i18n:1', '--keyword=i18nc:1c,2', '--keyword=i18np:1,2']
COMMON = ['--from-code=UTF-8', '--add-comments=TRANSLATORS', '--package-name=kiosque', '--sort-by-file']


def sources(directory, suffix):
    root = ROOT / directory
    return sorted(str(p.relative_to(ROOT)) for p in root.rglob('*' + suffix)
                  if 'target' not in p.relative_to(root).parts)


def rust_sources():
    # Only files that translate anything; keeps the POT's references short.
    return [p for p in sources('rust', '.rs') if re.search(r'\bi18n[cp]?\("', (ROOT / p).read_text())]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--merge', action='store_true', help='also msgmerge po/*.po')
    parser.add_argument('--check', action='store_true', help='fail if po/POTFILES is out of date')
    args = parser.parse_args()
    qml, rust = sources('qml', '.qml'), rust_sources()
    potfiles = ROOT / 'po/POTFILES'
    listed = '\n'.join(qml + rust) + '\n'
    if args.check:
        if potfiles.read_text() != listed:
            sys.exit('po/POTFILES is out of date; run scripts/update_pot.py')
        return
    potfiles.write_text(listed)
    with tempfile.TemporaryDirectory() as tmp:
        parts = []
        for language, files in (('JavaScript', qml), ('Rust', rust)):
            if not files:
                continue
            part = Path(tmp) / f'{language}.pot'
            subprocess.run(['xgettext', f'--language={language}', *COMMON, *KEYWORDS, '-o', part, *files],
                           cwd=ROOT, check=True)
            parts.append(part)
        subprocess.run(['msgcat', '--use-first', '-o', ROOT / 'po/kiosque.pot', *parts], check=True)
    if args.merge:
        for po in sorted((ROOT / 'po').glob('*.po')):
            subprocess.run(['msgmerge', '--quiet', '--update', '--backup=none', po, ROOT / 'po/kiosque.pot'], check=True)
    print('wrote po/kiosque.pot')


if __name__ == '__main__':
    main()
