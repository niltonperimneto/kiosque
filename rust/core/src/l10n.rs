//! Gettext catalogs for the `kiosque` domain, with KDE-style placeholders.
//!
//! Replaces KI18n. Messages are looked up in `.mo` catalogs parsed by the
//! pure-Rust `gettext` crate; `%1` … `%99` in the result are then replaced by
//! the supplied arguments, as `KLocalizedString::subs` does. Lookups fall
//! through the user's language list per message, so a partially translated
//! `pt_PT` catalog can be completed by `pt`.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use gettext::Catalog;

pub const DOMAIN: &str = "kiosque";

/// Ordered language candidates from the usual gettext environment variables.
///
/// The locale is the first non-empty value of `LC_ALL`, `LC_MESSAGES` and
/// `LANG`. A `C`/`POSIX` locale disables translation, as in GNU gettext;
/// otherwise the colon-separated `LANGUAGE` list takes precedence. Each entry
/// loses its codeset and expands to its language-only fallback, so
/// `pt_PT.UTF-8` yields `["pt_PT", "pt"]`.
pub fn languages_from_env(env: impl Fn(&str) -> Option<String>) -> Vec<String> {
    let get = |name: &str| env(name).filter(|v| !v.is_empty());
    let locale = get("LC_ALL")
        .or_else(|| get("LC_MESSAGES"))
        .or_else(|| get("LANG"));
    let Some(locale) = locale else {
        return get("LANGUAGE").map(|l| expand(&l)).unwrap_or_default();
    };
    if is_c_locale(&locale) {
        return vec![];
    }
    match get("LANGUAGE") {
        Some(list) => expand(&list),
        None => expand(&locale),
    }
}

fn is_c_locale(locale: &str) -> bool {
    let base = locale.split(['.', '@']).next().unwrap_or("");
    base == "C" || base == "POSIX"
}

fn expand(list: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for entry in list.split(':').filter(|e| !e.is_empty()) {
        if is_c_locale(entry) {
            continue;
        }
        // Drop the codeset; keep a modifier (`sr@latin`) on the full form.
        let (body, modifier) = match entry.split_once('@') {
            Some((b, m)) => (b, Some(m)),
            None => (entry, None),
        };
        let body = body.split('.').next().unwrap_or(body);
        let language = body.split('_').next().unwrap_or(body);
        let mut candidates = vec![];
        if let Some(m) = modifier {
            candidates.push(format!("{body}@{m}"));
            if language != body {
                candidates.push(format!("{language}@{m}"));
            }
        }
        candidates.push(body.to_string());
        candidates.push(language.to_string());
        for c in candidates {
            if !c.is_empty() && !out.contains(&c) {
                out.push(c);
            }
        }
    }
    out
}

/// Replace `%1` … `%99` with `args[n - 1]`. Up to two digits are read, so
/// `%10` is the tenth argument rather than `%1` followed by `0`. A
/// placeholder without a matching argument is kept verbatim.
pub fn substitute(text: &str, args: &[String]) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
            let mut end = i + 2;
            if end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            let n: usize = text[i + 1..end].parse().unwrap_or(0);
            if n >= 1 && n <= args.len() {
                out.push_str(&text[last..i]);
                out.push_str(&args[n - 1]);
                last = end;
            }
            i = end;
        } else {
            i += 1;
        }
    }
    out.push_str(&text[last..]);
    out
}

/// A list of catalogs in preference order.
#[derive(Default)]
pub struct Translator {
    catalogs: Vec<Catalog>,
}

/// The `gettext` crate hands back its argument on a miss, so a returned
/// slice that *is* one of the inputs means "not translated here".
fn same(a: &str, b: &str) -> bool {
    std::ptr::eq(a.as_ptr(), b.as_ptr()) && a.len() == b.len()
}

impl Translator {
    pub fn from_catalogs(catalogs: Vec<Catalog>) -> Self {
        Self { catalogs }
    }

    /// Load `<dir>/<lang>/LC_MESSAGES/<domain>.mo` for each language, using
    /// the first directory that has a catalog for that language.
    pub fn load(dirs: &[PathBuf], languages: &[String], domain: &str) -> Self {
        let mut catalogs = vec![];
        for lang in languages {
            let found = dirs.iter().find_map(|dir| {
                let path = dir
                    .join(lang)
                    .join("LC_MESSAGES")
                    .join(format!("{domain}.mo"));
                let file = std::fs::File::open(&path).ok()?;
                match Catalog::parse(std::io::BufReader::new(file)) {
                    Ok(c) => Some(c),
                    Err(e) => {
                        kerr!("l10n: cannot parse {}: {}", path.display(), e);
                        None
                    }
                }
            });
            catalogs.extend(found);
        }
        Self { catalogs }
    }

    pub fn is_empty(&self) -> bool {
        self.catalogs.is_empty()
    }

    pub fn gettext<'a>(&'a self, msgid: &'a str) -> &'a str {
        for c in &self.catalogs {
            let t = c.gettext(msgid);
            if !same(t, msgid) {
                return t;
            }
        }
        msgid
    }

    pub fn pgettext<'a>(&'a self, context: &'a str, msgid: &'a str) -> &'a str {
        for c in &self.catalogs {
            let t = c.pgettext(context, msgid);
            if !same(t, msgid) {
                return t;
            }
        }
        msgid
    }

    pub fn ngettext<'a>(&'a self, singular: &'a str, plural: &'a str, n: u64) -> &'a str {
        for c in &self.catalogs {
            let t = c.ngettext(singular, plural, n);
            if !same(t, singular) && !same(t, plural) {
                return t;
            }
        }
        if n == 1 { singular } else { plural }
    }

    /// KDE `i18n`: translate, then substitute.
    pub fn i18n(&self, msgid: &str, args: &[String]) -> String {
        substitute(self.gettext(msgid), args)
    }

    /// KDE `i18nc`: translate with a disambiguating context.
    pub fn i18nc(&self, context: &str, msgid: &str, args: &[String]) -> String {
        substitute(self.pgettext(context, msgid), args)
    }

    /// KDE `i18np`: `n` is `%1`; further arguments are `%2` onwards.
    pub fn i18np(&self, singular: &str, plural: &str, n: i64, args: &[String]) -> String {
        let count = n.unsigned_abs();
        let mut all = Vec::with_capacity(args.len() + 1);
        all.push(n.to_string());
        all.extend(args.iter().cloned());
        substitute(self.ngettext(singular, plural, count), &all)
    }
}

/// Catalog search path: `KIOSQUE_LOCALEDIR`, then `<exe>/../share/locale`,
/// then the install-time `KIOSQUE_LOCALEDIR` baked in at compile time.
pub fn locale_dirs(env: impl Fn(&str) -> Option<String>, exe: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = vec![];
    if let Some(dir) = env("KIOSQUE_LOCALEDIR").filter(|d| !d.is_empty()) {
        dirs.push(PathBuf::from(dir));
    }
    if let Some(prefix) = exe.and_then(Path::parent).and_then(Path::parent) {
        dirs.push(prefix.join("share/locale"));
    }
    if let Some(dir) = option_env!("KIOSQUE_LOCALEDIR") {
        dirs.push(PathBuf::from(dir));
    }
    dirs.dedup();
    dirs
}

static GLOBAL: OnceLock<Translator> = OnceLock::new();

/// Load the process-wide translator from the real environment. Idempotent.
pub fn init() -> &'static Translator {
    GLOBAL.get_or_init(|| {
        let env = |k: &str| std::env::var(k).ok();
        let exe = std::env::current_exe().ok();
        let languages = languages_from_env(env);
        let dirs = locale_dirs(env, exe.as_deref());
        let t = Translator::load(&dirs, &languages, DOMAIN);
        if !languages.is_empty() && t.is_empty() {
            klog!("l10n: no catalog for {:?} in {:?}", languages, dirs);
        }
        t
    })
}

/// The process-wide translator ([`init`] on first use).
pub fn tr() -> &'static Translator {
    init()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Assemble a little-endian `.mo` file from `(msgid, msgstr)` pairs. Plural
    /// ids/strings use NUL separators and contexts an EOT, as msgfmt writes them.
    pub(crate) fn mo(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut entries: Vec<(&str, &str)> = entries.to_vec();
        entries.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
        let n = entries.len() as u32;
        let header = 28u32;
        let ids_table = header;
        let strs_table = ids_table + 8 * n;
        let mut data_offset = strs_table + 8 * n;
        let mut tables = vec![];
        let mut blob = vec![];
        for (k, _) in &entries {
            tables.push((k.len() as u32, data_offset));
            blob.extend_from_slice(k.as_bytes());
            blob.push(0);
            data_offset += k.len() as u32 + 1;
        }
        for (_, v) in &entries {
            tables.push((v.len() as u32, data_offset));
            blob.extend_from_slice(v.as_bytes());
            blob.push(0);
            data_offset += v.len() as u32 + 1;
        }
        let mut out = vec![];
        for word in [0x950412de_u32, 0, n, ids_table, strs_table, 0, 0] {
            out.extend_from_slice(&word.to_le_bytes());
        }
        for (len, off) in tables {
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(&off.to_le_bytes());
        }
        out.extend(blob);
        out
    }

    const HEADER: &str =
        "Content-Type: text/plain; charset=UTF-8\nPlural-Forms: nplurals=2; plural=(n != 1);\n";

    fn catalog(entries: &[(&str, &str)]) -> Catalog {
        let mut all = vec![("", HEADER)];
        all.extend_from_slice(entries);
        Catalog::parse(&mo(&all)[..]).unwrap()
    }

    fn env(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = vars
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k| map.get(k).cloned()
    }

    #[test]
    fn language_precedence_and_fallback() {
        assert_eq!(
            languages_from_env(env(&[("LANG", "pt_PT.UTF-8")])),
            ["pt_PT", "pt"]
        );
        assert_eq!(
            languages_from_env(env(&[("LANG", "en_US.UTF-8"), ("LC_MESSAGES", "fr_FR")])),
            ["fr_FR", "fr"]
        );
        assert_eq!(
            languages_from_env(env(&[("LC_ALL", "it_IT"), ("LC_MESSAGES", "fr_FR")])),
            ["it_IT", "it"]
        );
        assert_eq!(
            languages_from_env(env(&[("LANG", "de_DE.UTF-8"), ("LANGUAGE", "pt_BR:fr")])),
            ["pt_BR", "pt", "fr"]
        );
        assert_eq!(
            languages_from_env(env(&[("LANG", "C.UTF-8"), ("LANGUAGE", "fr")])),
            Vec::<String>::new()
        );
        assert_eq!(
            languages_from_env(env(&[("LANG", "sr_RS@latin")])),
            ["sr_RS@latin", "sr@latin", "sr_RS", "sr"]
        );
        assert!(languages_from_env(env(&[])).is_empty());
    }

    #[test]
    fn substitution_reads_two_digits() {
        let args: Vec<String> = (1..=10).map(|i| format!("<{i}>")).collect();
        assert_eq!(substitute("%1 and %10", &args), "<1> and <10>");
        assert_eq!(substitute("%2%1", &args[..2]), "<2><1>");
        assert_eq!(substitute("%10", &args[..1]), "%10");
        assert_eq!(substitute("100% sure, %3", &args[..2]), "100% sure, %3");
        assert_eq!(substitute("ação %1", &["é".into()]), "ação é");
    }

    #[test]
    fn falls_through_catalogs_per_message() {
        let pt_pt = catalog(&[("Install", "Instalar")]);
        let pt = catalog(&[("Install", "Instale"), ("Remove", "Remover")]);
        let t = Translator::from_catalogs(vec![pt_pt, pt]);
        assert_eq!(t.i18n("Install", &[]), "Instalar");
        assert_eq!(t.i18n("Remove", &[]), "Remover");
        assert_eq!(t.i18n("Missing %1", &["x".into()]), "Missing x");
    }

    #[test]
    fn plurals_and_contexts() {
        let t = Translator::from_catalogs(vec![catalog(&[
            ("%1 update\0%1 updates", "%1 atualização\0%1 atualizações"),
            ("button\u{4}Open", "Abrir"),
            (
                "%1 of %2 app\0%1 of %2 apps",
                "%1 de %2 aplicação\0%1 de %2 aplicações",
            ),
        ])]);
        assert_eq!(t.i18np("%1 update", "%1 updates", 1, &[]), "1 atualização");
        assert_eq!(t.i18np("%1 update", "%1 updates", 3, &[]), "3 atualizações");
        assert_eq!(
            t.i18np("%1 of %2 app", "%1 of %2 apps", 2, &["5".into()]),
            "2 de 5 aplicações"
        );
        assert_eq!(t.i18nc("button", "Open", &[]), "Abrir");
        assert_eq!(t.i18nc("menu", "Open", &[]), "Open");
        let empty = Translator::default();
        assert_eq!(empty.i18np("%1 file", "%1 files", 1, &[]), "1 file");
        assert_eq!(empty.i18np("%1 file", "%1 files", 0, &[]), "0 files");
    }

    #[test]
    fn loads_catalogs_from_directories() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pt/LC_MESSAGES");
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(
            path.join("kiosque.mo"),
            mo(&[("", HEADER), ("Settings", "Definições")]),
        )
        .unwrap();
        let t = Translator::load(
            &[dir.path().join("missing"), dir.path().to_path_buf()],
            &["pt_PT".into(), "pt".into()],
            DOMAIN,
        );
        assert_eq!(t.i18n("Settings", &[]), "Definições");
    }

    #[test]
    fn locale_dir_order() {
        let dirs = locale_dirs(
            env(&[("KIOSQUE_LOCALEDIR", "/override")]),
            Some(Path::new("/opt/kiosque/bin/kiosque")),
        );
        assert_eq!(dirs[0], PathBuf::from("/override"));
        assert_eq!(dirs[1], PathBuf::from("/opt/kiosque/share/locale"));
    }
}
