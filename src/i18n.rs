//! Translations. Each language is a plain UTF-8 text file in `lang/`, built into the exe, so
//! translators need nothing but a text editor:
//!
//! ```text
//! # A comment.
//! tray.exit = Exit
//! tray.hotkeys_active = {count} hotkeys active.
//! ```
//!
//! Keys are grouped by where the text appears. `{name}` placeholders are filled in by the app,
//! and `\n` is a line break. English (`en.txt`) is the reference: a key missing from another
//! language falls back to it, and the tests check every language against it.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

/// A language the app can show: its code (as saved in the settings), its name in that language
/// (as listed in the settings), and its text.
pub struct Language {
    pub code: &'static str,
    pub name: &'static str,
    source: &'static str,
}

/// Every language, English first. To add one, add its file to `lang/` and a line here.
pub const LANGUAGES: &[Language] = &[
    Language { code: "en", name: "English", source: include_str!("../lang/en.txt") },
    Language { code: "af", name: "Afrikaans", source: include_str!("../lang/af.txt") },
];

pub const DEFAULT: &str = "en";

type Table = HashMap<&'static str, &'static str>;

/// Each language's text, parsed once.
fn tables() -> &'static [Table] {
    static TABLES: OnceLock<Vec<Table>> = OnceLock::new();
    TABLES.get_or_init(|| LANGUAGES.iter().map(|l| parse(l.source)).collect())
}

/// The language being shown, as an index into [`LANGUAGES`].
static CURRENT: RwLock<usize> = RwLock::new(0);

/// Shows the language with this code (English if there's no such language).
pub fn set_language(code: &str) {
    let index = LANGUAGES.iter().position(|l| l.code.eq_ignore_ascii_case(code)).unwrap_or(0);
    *CURRENT.write().unwrap_or_else(|e| e.into_inner()) = index;
    crate::ui::dialog::set_button_labels(t("common.ok"), t("common.cancel"));
}

/// The code of the language being shown.
pub fn language() -> &'static str {
    LANGUAGES[current()].code
}

fn current() -> usize {
    *CURRENT.read().unwrap_or_else(|e| e.into_inner())
}

/// The text for `key` in the current language, else in English, else the key itself (so a
/// missing key shows up rather than leaving a blank).
pub fn t(key: &'static str) -> &'static str {
    let tables = tables();
    tables[current()].get(key).or_else(|| tables[0].get(key)).copied().unwrap_or(key)
}

/// The text for `key` with its `{name}` placeholders filled in.
pub fn tf(key: &'static str, args: &[(&str, &dyn std::fmt::Display)]) -> String {
    let mut text = t(key).to_string();
    for (name, value) in args {
        text = text.replace(&format!("{{{name}}}"), &value.to_string());
    }
    text
}

/// Reads a language file: `key = text` lines; blank lines and lines starting with `#` are
/// skipped. `\n` in the text is a line break.
fn parse(source: &'static str) -> Table {
    source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split_once('='))
        .map(|(key, text)| (key.trim(), unescape(text.trim())))
        .collect()
}

/// Text with `\n` turned into line breaks. Leaked: tables live for the app's lifetime.
fn unescape(text: &'static str) -> &'static str {
    if text.contains("\\n") { Box::leak(text.replace("\\n", "\n").into_boxed_str()) } else { text }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn placeholders(text: &str) -> BTreeSet<&str> {
        text.split('{').skip(1).filter_map(|s| s.split_once('}')).map(|(name, _)| name).collect()
    }

    /// Lines that aren't comments, blank or `key = text`: probably a typo.
    fn malformed(source: &str) -> Vec<&str> {
        source.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#') && !l.contains('=')).collect()
    }

    #[test]
    fn every_language_matches_english() {
        let english = &tables()[0];
        for (language, table) in LANGUAGES.iter().zip(tables()) {
            assert!(malformed(language.source).is_empty(), "{}: lines without '=': {:?}", language.code, malformed(language.source));
            for (key, text) in table {
                let Some(reference) = english.get(key) else { panic!("{}: unknown key {key}", language.code) };
                assert_eq!(placeholders(text), placeholders(reference), "{}: {key} has different placeholders", language.code);
            }
        }
    }

    #[test]
    fn translations_are_complete() {
        let english = &tables()[0];
        for (language, table) in LANGUAGES.iter().zip(tables()).skip(1) {
            let missing: Vec<_> = english.keys().filter(|k| !table.contains_key(*k)).collect();
            assert!(missing.is_empty(), "{}: missing {missing:?}", language.code);
        }
    }

    /// Every key the code uses is in English: any string literal that starts with one of the
    /// groups in en.txt ("tray.", "editor." and so on) is taken to be a key.
    #[test]
    fn code_uses_only_known_keys() {
        let english = &tables()[0];
        let groups: BTreeSet<&str> = english.keys().filter_map(|k| k.split_once('.')).map(|(group, _)| group).collect();
        let is_key = |s: &str| {
            s.split_once('.').is_some_and(|(group, rest)| groups.contains(group) && !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'))
        };
        let mut unknown = Vec::new();
        let mut dirs = vec![std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
        while let Some(dir) = dirs.pop() {
            for path in std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()) {
                if path.is_dir() {
                    dirs.push(path);
                } else if path.file_name() != Some("i18n.rs".as_ref()) {
                    let code = std::fs::read_to_string(&path).unwrap();
                    // Every other piece between quotes is a string literal (near enough).
                    for literal in code.split('"').skip(1).step_by(2) {
                        if is_key(literal) && !english.contains_key(literal) {
                            unknown.push(format!("{}: {literal}", path.display()));
                        }
                    }
                }
            }
        }
        assert!(unknown.is_empty(), "keys missing from en.txt: {unknown:#?}");
    }

    #[test]
    fn fills_placeholders_and_falls_back() {
        let table = parse("a.b = Hello {name}\\nbye\n# c.d = nope\n  e.f=x = y ");
        assert_eq!(table["a.b"], "Hello {name}\nbye");
        assert_eq!(table["e.f"], "x = y");
        assert!(!table.contains_key("c.d"));
        assert_eq!(t("no.such.key"), "no.such.key");
    }
}
