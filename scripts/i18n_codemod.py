#!/usr/bin/env python3
"""One-shot migration of QML KI18n calls to the Rust `L10n` singleton.

    i18n(msg, a, b)       -> L10n.i18n(msg, [String(a), String(b)])
    i18nc(ctx, msg, a)    -> L10n.i18nc(ctx, msg, [String(a)])
    i18np(s, p, n, a)     -> L10n.i18np(s, p, n, [String(a)])

A small tokenizer skips strings, template literals and comments, so only
real calls are rewritten, including calls nested in arguments. `import
org.kde.ki18n` lines are removed and `import com.kiosque` is added where
needed. Run once, then review the diff:

    python3 scripts/i18n_codemod.py qml/**/*.qml
"""
import re
import sys
from pathlib import Path

FIXED = {'i18n': 1, 'i18nc': 2, 'i18np': 3}  # leading arguments kept as-is


def skip_string(text, i):
    quote = text[i]
    i += 1
    while i < len(text):
        c = text[i]
        if c == '\\':
            i += 2
            continue
        if quote == '`' and text.startswith('${', i):
            i = skip_balanced(text, i + 1, '{', '}')
            continue
        if c == quote:
            return i + 1
        i += 1
    raise ValueError('unterminated string')


def skip_comment(text, i):
    if text.startswith('//', i):
        end = text.find('\n', i)
        return len(text) if end < 0 else end
    end = text.find('*/', i + 2)
    if end < 0:
        raise ValueError('unterminated comment')
    return end + 2


def skip_balanced(text, i, open_, close):
    """`text[i]` is `open_`; return the index after the matching `close`."""
    depth = 0
    while i < len(text):
        c = text[i]
        if c in '"\'`':
            i = skip_string(text, i)
            continue
        if text.startswith(('//', '/*'), i):
            i = skip_comment(text, i)
            continue
        if c in '([{':
            depth += 1
        elif c in ')]}':
            depth -= 1
            if depth == 0:
                return i + 1
        i += 1
    raise ValueError('unbalanced ' + open_)


def split_args(inner):
    args, start, i = [], 0, 0
    while i < len(inner):
        c = inner[i]
        if c in '"\'`':
            i = skip_string(inner, i)
            continue
        if inner.startswith(('//', '/*'), i):
            i = skip_comment(inner, i)
            continue
        if c in '([{':
            i = skip_balanced(inner, i, c, {'(': ')', '[': ']', '{': '}'}[c])
            continue
        if c == ',':
            args.append(inner[start:i])
            start = i + 1
        i += 1
    if inner[start:].strip() or args:
        args.append(inner[start:])
    return [a.strip() for a in args]


IDENT = re.compile(r'[A-Za-z_$][A-Za-z0-9_$]*')


def rewrite(text):
    out, i, count = [], 0, 0
    while i < len(text):
        c = text[i]
        if c in '"\'`':
            j = skip_string(text, i)
            out.append(text[i:j]); i = j
            continue
        if text.startswith(('//', '/*'), i):
            j = skip_comment(text, i)
            out.append(text[i:j]); i = j
            continue
        m = IDENT.match(text, i)
        if not m:
            out.append(c); i += 1
            continue
        name, j = m.group(0), m.end()
        prev = text[:i].rstrip()[-1:] if text[:i].rstrip() else ''
        k = j
        while k < len(text) and text[k] in ' \t':
            k += 1
        if name in FIXED and prev != '.' and k < len(text) and text[k] == '(':
            end = skip_balanced(text, k, '(', ')')
            inner, nested = rewrite(text[k + 1:end - 1])
            count += nested + 1
            args = split_args(inner)
            fixed, extra = args[:FIXED[name]], args[FIXED[name]:]
            if len(fixed) < FIXED[name]:
                raise ValueError(f'{name} call with too few arguments: {text[i:end]}')
            wrapped = ', '.join(f'String({a})' for a in extra)
            out.append(f'L10n.{name}({", ".join(fixed)}, [{wrapped}])')
            i = end
            continue
        out.append(name); i = j
    return ''.join(out), count


def migrate(path):
    text = path.read_text()
    new, count = rewrite(text)
    new = re.sub(r'^import org\.kde\.ki18n[^\n]*\n', '', new, flags=re.M)
    if count and not re.search(r'^import com\.kiosque\b', new, flags=re.M):
        imports = list(re.finditer(r'^import [^\n]*\n', new, flags=re.M))
        at = imports[-1].end() if imports else 0
        new = new[:at] + 'import com.kiosque\n' + new[at:]
    if new != text:
        path.write_text(new)
    return count


def main(paths):
    total = 0
    for p in paths:
        n = migrate(Path(p))
        if n:
            print(f'{p}: {n} calls')
        total += n
    print(f'{total} calls rewritten')


if __name__ == '__main__':
    main(sys.argv[1:])
