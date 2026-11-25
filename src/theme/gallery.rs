//! The theme gallery: extra themes shipped as NTH2 share codes in
//! `assets/themes/gallery.txt` (one code per line), offered in Theme Studio's gallery until
//! installed. Decoded exactly like a pasted code, so the gallery is also a test of the
//! format: anyone can add a theme by pasting its code into the file.

use std::sync::OnceLock;

use super::studio::from_share_code;
use super::ThemePack;

const GALLERY: &str = include_str!("../../assets/themes/gallery.txt");

fn decode(text: &str) -> Vec<ThemePack> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| from_share_code(l).ok())
        .collect()
}

/// Every gallery theme, decoded once.
pub fn packs() -> &'static [ThemePack] {
    static PACKS: OnceLock<Vec<ThemePack>> = OnceLock::new();
    PACKS.get_or_init(|| decode(GALLERY))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_line_is_a_valid_readable_theme() {
        let lines = GALLERY
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
            .count();
        let packs = decode(GALLERY);
        assert!(packs.len() >= 10);
        assert_eq!(packs.len(), lines, "a gallery line doesn't decode");
        for p in &packs {
            let t = p.to_theme().expect("valid colours");
            assert!(
                super::super::studio::contrast_ok(&t),
                "{} fails the text contrast check",
                p.name
            );
        }
    }

    #[test]
    fn ids_are_unique_and_clear_of_built_ins() {
        let packs = packs();
        let mut ids: Vec<&str> = packs.iter().map(|p| p.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), packs.len(), "duplicate gallery id");
        for p in packs {
            assert!(
                super::super::preset(&p.id).is_none(),
                "{} is a preset id",
                p.id
            );
            assert!(
                !super::super::PRESETS
                    .iter()
                    .any(|pr| pr.name.eq_ignore_ascii_case(&p.name)),
                "{} is a preset name",
                p.name
            );
            assert!(
                !super::super::BUILTIN_PACKS
                    .iter()
                    .any(|j| j.contains(&format!("\"id\":\"{}\"", p.id))),
                "{} is a built-in pack id",
                p.id
            );
        }
    }

    #[test]
    fn codes_survive_a_round_trip() {
        for p in packs() {
            let back = from_share_code(&super::super::studio::share_code(p)).unwrap();
            assert_eq!(&back, p);
        }
    }
}
