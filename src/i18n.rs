//! Localization. Strings are keyed by their English text; translations live in
//! assets/i18n/<lang>.toml (embedded at build time). Missing keys fall back to English.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

static LANG: RwLock<String> = RwLock::new(String::new());

fn tables() -> &'static HashMap<&'static str, HashMap<String, String>> {
    static T: OnceLock<HashMap<&'static str, HashMap<String, String>>> = OnceLock::new();
    T.get_or_init(|| {
        let mut m = HashMap::new();
        for (lang, src) in [
            ("hi", include_str!("../assets/i18n/hi.toml")),
            ("ne", include_str!("../assets/i18n/ne.toml")),
        ] {
            let table: HashMap<String, String> = toml::from_str(src).unwrap_or_default();
            m.insert(lang, table);
        }
        m
    })
}

pub fn set_language(lang: &str) {
    if let Ok(mut l) = LANG.write() {
        *l = lang.to_string();
    }
}

/// Translates an English UI string for the current language.
pub fn tr(s: &str) -> String {
    let lang = LANG.read().map(|l| l.clone()).unwrap_or_default();
    if lang.is_empty() || lang == "en" {
        return s.to_string();
    }
    tables()
        .get(lang.as_str())
        .and_then(|t| t.get(s))
        .cloned()
        .unwrap_or_else(|| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tables_parse_and_cover_titles() {
        for lang in ["hi", "ne"] {
            let t = tables().get(lang).expect("table");
            assert!(t.len() > 50, "{lang} has {} entries", t.len());
            assert!(t.contains_key("Settings"));
        }
        set_language("ne");
        assert_ne!(tr("Volume"), "Volume");
        assert_eq!(tr("Untranslated key xyz"), "Untranslated key xyz");
        set_language("en");
        assert_eq!(tr("Volume"), "Volume");
    }
}
