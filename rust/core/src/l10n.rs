//! Gettext catalogs for the `kiosque` domain, with KDE-style placeholders.
//!
//! Replaces KI18n. Messages are looked up in `.mo` catalogs parsed by the
//! a bounded UTF-8 reader and the `gettext_catalog` plural evaluator; `%1` … `%99` in the result are then replaced by
//! the supplied arguments, as `KLocalizedString::subs` does. Lookups fall
//! through the user's language list per message, so a partially translated
//! `pt_PT` catalog can be completed by `pt`.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use gettext_catalog::plural::PluralRule;
use std::borrow::Cow;
use std::collections::HashMap;

/// Bounded UTF-8 GNU MO catalog, as produced by this project's msgfmt step.
/// Keep context parsing here: the previous replacement library reversed the
/// GNU `context\x04message` convention. Plural expressions use the complete
/// gettext grammar supplied by `gettext_catalog`.
#[derive(Debug)]
pub struct Catalog {
    messages: HashMap<String, Vec<String>>,
    plural: PluralRule,
}
impl Catalog {
    pub fn from_path(path: &Path) -> Result<Self, String> {
        use std::io::Read;
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .map_err(|e| e.to_string())?
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        Self::from_vec_u8(bytes)
    }
    pub fn from_vec_u8(bytes: Vec<u8>) -> Result<Self, String> {
        if bytes.len() < 28 {
            return Err("truncated MO header".into());
        }
        if bytes.len() > 16 * 1024 * 1024 {
            return Err("catalog exceeds 16 MiB".into());
        }
        let little = match bytes.get(..4) {
            Some([0xde, 0x12, 0x04, 0x95]) => true,
            Some([0x95, 0x04, 0x12, 0xde]) => false,
            _ => return Err("invalid MO magic".into()),
        };
        let word = |offset: usize| -> Result<usize, String> {
            let raw: [u8; 4] = bytes
                .get(offset..offset + 4)
                .ok_or("truncated MO table")?
                .try_into()
                .map_err(|_| "invalid word")?;
            Ok(if little {
                u32::from_le_bytes(raw)
            } else {
                u32::from_be_bytes(raw)
            } as usize)
        };
        if word(4)? != 0 {
            return Err("unsupported MO revision".into());
        }
        let count = word(8)?;
        let ids = word(12)?;
        let values = word(16)?;
        if count > bytes.len() / 16 {
            return Err("invalid MO entry count".into());
        }
        let text = |table: usize, index: usize| -> Result<&str, String> {
            let entry = table.checked_add(index * 8).ok_or("MO table overflow")?;
            let length = word(entry)?;
            let offset = word(entry + 4)?;
            let end = offset.checked_add(length).ok_or("MO string overflow")?;
            std::str::from_utf8(bytes.get(offset..end).ok_or("truncated MO string")?)
                .map_err(|_| "catalog must be UTF-8".into())
        };
        let mut messages = HashMap::new();
        let mut plural = PluralRule::new(2, "n != 1").map_err(|e| e.to_string())?;
        for i in 0..count {
            let id = text(ids, i)?;
            let translation = text(values, i)?;
            if id.is_empty() {
                if let Some(forms) = translation
                    .lines()
                    .find_map(|line| line.strip_prefix("Plural-Forms:"))
                {
                    let mut nplurals = 2;
                    let mut expression = "n != 1";
                    for option in forms.split(';') {
                        if let Some((key, value)) = option.trim().split_once('=') {
                            match key.trim() {
                                "nplurals" => {
                                    nplurals =
                                        value.trim().parse().map_err(|_| "invalid plural count")?
                                }
                                "plural" => expression = value.trim(),
                                _ => {}
                            }
                        }
                    }
                    plural = PluralRule::new(nplurals, expression).map_err(|e| e.to_string())?;
                }
            } else if !translation.is_empty() {
                let singular = id.split('\0').next().unwrap_or(id);
                messages.insert(
                    singular.into(),
                    translation.split('\0').map(str::to_owned).collect(),
                );
            }
        }
        Ok(Self { messages, plural })
    }
    fn lookup(&self, id: &str, context: Option<&str>) -> Option<&Vec<String>> {
        match context {
            Some(context) => self.messages.get(&format!("{context}\u{4}{id}")),
            None => self.messages.get(id),
        }
    }
    pub fn translate<'a>(&'a self, id: &'a str, context: Option<&str>) -> Cow<'a, str> {
        Cow::Borrowed(
            self.lookup(id, context)
                .and_then(|forms| forms.first())
                .map(String::as_str)
                .filter(|s| !s.is_empty())
                .unwrap_or(id),
        )
    }
    pub fn ntranslate<'a>(
        &'a self,
        n: u64,
        singular: &'a str,
        plural: &'a str,
        context: Option<&str>,
    ) -> Cow<'a, str> {
        Cow::Borrowed(
            self.lookup(singular, context)
                .and_then(|forms| forms.get(self.plural.index(n)))
                .map(String::as_str)
                .filter(|s| !s.is_empty())
                .unwrap_or(if n == 1 { singular } else { plural }),
        )
    }
}

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

/// The catalog backend hands back its argument on a miss, so a returned
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
                if !path.is_file() {
                    return None;
                }
                match Catalog::from_path(&path) {
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

    pub fn gettext<'a>(&'a self, msgid: &'a str) -> Cow<'a, str> {
        for c in &self.catalogs {
            let t = c.translate(msgid, None);
            if !same(&t, msgid) {
                return t;
            }
        }
        Cow::Borrowed(msgid)
    }

    pub fn pgettext<'a>(&'a self, context: &'a str, msgid: &'a str) -> Cow<'a, str> {
        for c in &self.catalogs {
            let t = c.translate(msgid, Some(context));
            if !same(&t, msgid) {
                return t;
            }
        }
        Cow::Borrowed(msgid)
    }

    pub fn ngettext<'a>(&'a self, singular: &'a str, plural: &'a str, n: u64) -> Cow<'a, str> {
        for c in &self.catalogs {
            let t = c.ntranslate(n, singular, plural, None);
            if !same(&t, singular) && !same(&t, plural) {
                return t;
            }
        }
        Cow::Borrowed(if n == 1 { singular } else { plural })
    }

    /// KDE `i18n`: translate, then substitute.
    pub fn i18n(&self, msgid: &str, args: &[String]) -> String {
        substitute(&self.gettext(msgid), args)
    }

    /// KDE `i18nc`: translate with a disambiguating context.
    pub fn i18nc(&self, context: &str, msgid: &str, args: &[String]) -> String {
        substitute(&self.pgettext(context, msgid), args)
    }

    /// KDE `i18np`: `n` is `%1`; further arguments are `%2` onwards.
    pub fn i18np(&self, singular: &str, plural: &str, n: i64, args: &[String]) -> String {
        let count = n.unsigned_abs();
        let mut all = Vec::with_capacity(args.len() + 1);
        all.push(n.to_string());
        all.extend(args.iter().cloned());
        substitute(&self.ngettext(singular, plural, count), &all)
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
        Translator::load(&dirs, &languages, DOMAIN)
    })
}

/// The process-wide translator ([`init`] on first use).
pub fn tr() -> &'static Translator {
    init()
}

/// [`Translator::i18n`] on the process-wide translator. Free functions,
/// because xgettext's Rust parser only recognises plain calls as keywords.
pub fn i18n(msgid: &str, args: &[String]) -> String {
    tr().i18n(msgid, args)
}

/// [`Translator::i18nc`] on the process-wide translator.
pub fn i18nc(context: &str, msgid: &str, args: &[String]) -> String {
    tr().i18nc(context, msgid, args)
}

/// [`Translator::i18np`] on the process-wide translator.
pub fn i18np(singular: &str, plural: &str, n: i64, args: &[String]) -> String {
    tr().i18np(singular, plural, n, args)
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
        Catalog::from_vec_u8(mo(&all)).unwrap()
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
    fn catalogs_preserve_three_forms_and_contexts() {
        let bytes = mo(&[
            (
                "",
                "Content-Type: text/plain; charset=UTF-8\nPlural-Forms: nplurals=3; plural=(n == 1 ? 0 : n == 2 ? 1 : 2);\n",
            ),
            ("noun\u{4}%1 item\0%1 items", "uno\0dos\0muchos"),
        ]);
        let catalog = Catalog::from_vec_u8(bytes).unwrap();
        for (count, expected) in [(1, "uno"), (2, "dos"), (5, "muchos")] {
            assert_eq!(
                catalog.ntranslate(count, "%1 item", "%1 items", Some("noun")),
                expected
            );
        }
    }
    #[test]
    fn catalogs_accept_big_endian_and_reject_truncation() {
        let bytes = mo(&[("", HEADER), ("Settings", "Definições")]);
        let mut big = bytes.clone();
        for word in big[..28 + 2 * 16].as_chunks_mut::<4>().0 {
            let value = u32::from_le_bytes(*word);
            word.copy_from_slice(&value.to_be_bytes());
        }
        let t = Translator::from_catalogs(vec![Catalog::from_vec_u8(big).unwrap()]);
        assert_eq!(t.i18n("Settings", &[]), "Definições");
        assert!(Catalog::from_vec_u8(bytes[..20].to_vec()).is_err());
        let mut broken = bytes;
        broken[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Catalog::from_vec_u8(broken).is_err());
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
