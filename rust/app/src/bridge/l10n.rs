//! `L10n` singleton: gettext lookups for QML (replaces KI18n's context).
//!
//! Arguments are always passed as an array of strings, e.g.
//! `L10n.i18n("%1 of %2", [String(a), String(b)])`; `scripts/i18n_codemod.py`
//! produced the calls and xgettext extracts them with the keywords
//! `i18n:1`, `i18nc:1c,2` and `i18np:1,2`.

use qtbridge::qobject;

pub use object::L10n;

#[qobject(NoQmlElement)]
mod object {
    use kiosque_core::l10n::tr;

    #[derive(Default)]
    pub struct L10n {}

    impl L10n {
        #[qslot]
        fn i18n(&self, msgid: String, args: Vec<String>) -> String {
            tr().i18n(&msgid, &args)
        }

        #[qslot]
        fn i18nc(&self, context: String, msgid: String, args: Vec<String>) -> String {
            tr().i18nc(&context, &msgid, &args)
        }

        /// `n` becomes `%1`; `args` are `%2` onwards.
        #[qslot]
        fn i18np(&self, singular: String, plural: String, n: i64, args: Vec<String>) -> String {
            tr().i18np(&singular, &plural, n, &args)
        }
    }
}

qml_element!(L10n, "L10n", singleton);
