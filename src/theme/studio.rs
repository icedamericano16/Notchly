//! Theme Studio: user themes (ThemePack v2 files in themes/), editing with undo, share
//! codes, the contrast guard and "generate from one colour".

use std::time::{Duration, Instant};

use super::tokens::*;
use super::{GlassPack, Theme, ThemePack};

/// Undo keeps this many edits.
const UNDO_STEPS: usize = 50;
/// Edits closer together than this (a slider drag, a picker drag) are one undo step.
const COALESCE: Duration = Duration::from_millis(600);
/// Share codes start with this.
pub const SHARE_PREFIX: &str = "NTH2:";
/// Theme files and share codes are capped like before.
pub const MAX_BYTES: usize = 64 * 1024;
/// Body text needs 4.5:1, secondary text 3:1 (WCAG AA).
pub const AA_TEXT: f32 = 4.5;
pub const AA_SECONDARY: f32 = 3.0;

#[derive(Default)]
pub struct Studio {
    /// The user's own themes (files in themes/).
    pub user: Vec<ThemePack>,
    undo: Vec<ThemePack>,
    redo: Vec<ThemePack>,
    last_push: Option<Instant>,
    /// Theme files to write (debounced).
    pub save_due: Option<Instant>,
    pending: Vec<String>,
    /// A message for the Settings window ("Made a copy you can edit").
    pub notice: Option<String>,
    /// Recently picked colours, newest first.
    pub recent: Vec<String>,
}

impl Studio {
    pub fn load() -> Self {
        Self {
            user: load_user_packs(),
            ..Default::default()
        }
    }

    pub fn is_user(&self, theme_id: &str) -> bool {
        self.find(theme_id).is_some()
    }

    pub fn find(&self, theme_id: &str) -> Option<&ThemePack> {
        let id = theme_id.strip_prefix("pack:")?;
        self.user.iter().find(|p| p.id == id)
    }

    /// Records the pack before an edit (one step per burst of edits).
    pub fn checkpoint(&mut self, before: &ThemePack) {
        let burst = self.last_push.is_some_and(|t| t.elapsed() < COALESCE)
            && self.undo.last().is_some_and(|p| p.id == before.id);
        self.last_push = Some(Instant::now());
        if burst {
            return;
        }
        self.undo.push(before.clone());
        if self.undo.len() > UNDO_STEPS {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Steps back: returns the pack to restore (its id says which theme).
    pub fn undo(&mut self) -> Option<ThemePack> {
        let p = self.undo.pop()?;
        if let Some(cur) = self.user.iter().find(|u| u.id == p.id) {
            self.redo.push(cur.clone());
        }
        self.last_push = None;
        Some(p)
    }

    pub fn redo(&mut self) -> Option<ThemePack> {
        let p = self.redo.pop()?;
        if let Some(cur) = self.user.iter().find(|u| u.id == p.id) {
            self.undo.push(cur.clone());
        }
        self.last_push = None;
        Some(p)
    }

    /// Puts a pack into the user list and schedules its file to be written.
    pub fn store(&mut self, p: ThemePack) {
        self.pending.retain(|i| *i != p.id);
        self.pending.push(p.id.clone());
        match self.user.iter_mut().find(|u| u.id == p.id) {
            Some(u) => *u = p,
            None => self.user.push(p),
        }
        self.save_due = Some(Instant::now() + Duration::from_millis(400));
    }

    /// Writes pending theme files once the debounce has passed (or now, with `force`).
    pub fn flush(&mut self, force: bool) {
        if !force && self.save_due.is_none_or(|d| Instant::now() < d) {
            return;
        }
        self.save_due = None;
        for id in std::mem::take(&mut self.pending) {
            if let Some(p) = self.user.iter().find(|u| u.id == id) {
                if let Err(e) = save_pack(p) {
                    crate::util::log(&format!("theme {id} not saved: {e}"));
                }
            }
        }
    }

    pub fn delete(&mut self, id: &str) {
        self.user.retain(|p| p.id != id);
        self.pending.retain(|i| i != id);
        self.undo.retain(|p| p.id != id);
        self.redo.retain(|p| p.id != id);
        let _ = std::fs::remove_file(pack_path(id));
    }

    pub fn remember_colour(&mut self, hex: &str) {
        self.recent.retain(|h| !h.eq_ignore_ascii_case(hex));
        self.recent.insert(0, hex.to_uppercase());
        self.recent.truncate(8);
    }
}

pub fn pack_path(id: &str) -> std::path::PathBuf {
    super::themes_dir().join(format!("{id}.json"))
}

pub fn save_pack(p: &ThemePack) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(p).map_err(std::io::Error::other)?;
    crate::util::atomic_write(&pack_path(&p.id), text.as_bytes())
}

/// The user's theme files (valid ones; the built-ins are not files).
pub fn load_user_packs() -> Vec<ThemePack> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(super::themes_dir()) else {
        return out;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        let Ok(bytes) = std::fs::read(&p) else {
            continue;
        };
        if bytes.len() > MAX_BYTES {
            continue;
        }
        if let Ok(pack) = serde_json::from_slice::<ThemePack>(&bytes) {
            if pack.to_theme().is_ok() {
                out.push(pack);
            }
        }
    }
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

pub fn hex(c: Color) -> String {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02X}{:02X}{:02X}", b(c.r), b(c.g), b(c.b))
}

/// A resolved theme as an editable v2 pack (ids lose their `pack:` prefix).
pub fn pack_of(t: &Theme) -> ThemePack {
    ThemePack {
        version: Some(2),
        id: t.id.trim_start_matches("pack:").to_string(),
        name: t.name.clone(),
        author: None,
        accent: [hex(t.accent[0]), hex(t.accent[1]), hex(t.accent[2])],
        surface: hex(t.surface),
        surface_raised: Some(hex(t.surface_raised)),
        ink: Some(hex(t.ink)),
        mist: Some(hex(t.mist.with_alpha(1.0))),
        radius_scale: Some(t.radius_scale),
        font: t.font_override.clone(),
        springs: t.spring_overrides,
        glass: Some(GlassPack {
            tint: Some(hex(t.glass.tint)),
            rim: Some(hex(t.glass.rim)),
            sheen: Some(hex(t.glass.sheen)),
            shadow: Some(hex(t.glass.shadow_tint)),
            glow: Some(hex(t.glass.accent_glow)),
            strength: t.glass_strength,
            blur: t.glass_blur,
        }),
    }
}

/// A file-safe id from a name, not clashing with `taken`.
pub fn unique_id(name: &str, taken: &[String]) -> String {
    let slug: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let mut base = slug
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    base.truncate(32);
    if base.is_empty() {
        base = "theme".into();
    }
    let mut id = base.clone();
    let mut n = 2;
    while taken.iter().any(|t| t.trim_start_matches("pack:") == id) {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

/// "Name copy", "Name copy 2", …
pub fn copy_name(name: &str, names: &[String]) -> String {
    let base = format!("{name} copy");
    let mut n = base.clone();
    let mut k = 2;
    while names.contains(&n) {
        n = format!("{base} {k}");
        k += 1;
    }
    n
}

// ---------------------------------------------------------------- share codes

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn b64url(data: &[u8]) -> String {
    let mut s = String::with_capacity(data.len() * 4 / 3 + 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..=c.len() {
            s.push(B64[((n >> (18 - 6 * i)) & 63) as usize] as char);
        }
    }
    s
}

fn unb64url(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for ch in s.bytes() {
        let v = B64.iter().position(|b| *b == ch)? as u32;
        acc = (acc << 6 | v) & 0xFFFFFF;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xFF) as u8);
        }
    }
    Some(out)
}

/// A theme as a compact text code ("NTH2:…") to paste anywhere.
pub fn share_code(p: &ThemePack) -> String {
    let json = serde_json::to_string(p).unwrap_or_default();
    format!("{SHARE_PREFIX}{}", b64url(json.as_bytes()))
}

/// Reads a share code, validated exactly like a theme file (data only, never executed).
pub fn from_share_code(code: &str) -> Result<ThemePack, String> {
    let body = code
        .trim()
        .strip_prefix(SHARE_PREFIX)
        .ok_or_else(|| crate::i18n::tr("Not a Notchly theme code"))?;
    let body: String = body.chars().filter(|c| !c.is_whitespace()).collect();
    if body.len() > MAX_BYTES * 4 / 3 + 4 {
        return Err(crate::i18n::tr("Theme code is too long"));
    }
    let bytes = unb64url(&body).ok_or_else(|| crate::i18n::tr("Theme code is damaged"))?;
    let p: ThemePack = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    p.to_theme()?;
    Ok(p)
}

/// Reads a theme file the user picked.
pub fn read_pack_file(path: &std::path::Path) -> Result<ThemePack, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_BYTES {
        return Err(crate::i18n::tr("Theme file is too large"));
    }
    let p: ThemePack = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    p.to_theme()?;
    Ok(p)
}

// ---------------------------------------------------------------- contrast guard

/// The weakest text contrast in a theme: (body text, secondary text). Body text is checked
/// on the surface, the raised surface and the glass at full tint over a white and a dark
/// backdrop; secondary text on the surfaces.
pub fn contrast(t: &Theme) -> (f32, f32) {
    use crate::render::glass::vibrancy::{contrast, over};
    let a = GLASS.tint_alpha[2];
    let glass_light = over(t.glass.tint.with_alpha(a), Color::rgb(0xFFFFFF));
    let glass_dark = over(t.glass.tint.with_alpha(a), Color::rgb(0x101010));
    let ink = [t.surface, t.surface_raised, glass_light, glass_dark]
        .iter()
        .map(|b| contrast(t.ink, *b))
        .fold(f32::MAX, f32::min);
    let mist = [t.surface, t.surface_raised]
        .iter()
        .map(|b| contrast(t.mist.with_alpha(1.0), *b))
        .fold(f32::MAX, f32::min);
    (ink, mist)
}

pub fn contrast_ok(t: &Theme) -> bool {
    let (i, m) = contrast(t);
    i >= AA_TEXT && m >= AA_SECONDARY
}

/// Nudges text lightness (and, when that isn't enough, the glass tint and surfaces) until
/// the theme passes. Returns false if it couldn't.
pub fn fix_contrast(p: &mut ThemePack) -> bool {
    for _ in 0..60 {
        let Ok(t) = p.to_theme() else { return false };
        let (ink, mist) = contrast(&t);
        if ink >= AA_TEXT && mist >= AA_SECONDARY {
            return true;
        }
        let dark_bg = t.surface.luminance() < 0.5;
        let (toward, away) = if dark_bg {
            (Color::rgb(0xFFFFFF), Color::rgb(0x000000))
        } else {
            (Color::rgb(0x000000), Color::rgb(0xFFFFFF))
        };
        if ink < AA_TEXT {
            p.ink = Some(hex(t.ink.lerp(toward, 0.15)));
            // even the extreme text colour fails: deepen the glass tint and surfaces
            if contrast_with_ink(&t, toward) < AA_TEXT {
                let g = p.glass.get_or_insert_with(Default::default);
                g.tint = Some(hex(t.glass.tint.lerp(away, 0.12)));
                p.surface = hex(t.surface.lerp(away, 0.08));
                p.surface_raised = Some(hex(t.surface_raised.lerp(away, 0.08)));
            }
        }
        if mist < AA_SECONDARY {
            p.mist = Some(hex(t.mist.with_alpha(1.0).lerp(toward, 0.15)));
        }
    }
    p.to_theme().is_ok_and(|t| contrast_ok(&t))
}

fn contrast_with_ink(t: &Theme, ink: Color) -> f32 {
    let mut t = t.clone();
    t.ink = ink;
    contrast(&t).0
}

/// A whole palette from one colour: accent stops around its hue, a deep surface tinted
/// with it, text from the defaults. The user tweaks from there.
pub fn from_one_colour(p: &mut ThemePack, c: Color) {
    use super::adaptive::{from_hsl, to_hsl};
    let (h, s, _) = to_hsl(c);
    let s = s.max(0.55);
    let stops = [
        from_hsl(h - 20.0, s, 0.64),
        from_hsl(h, s, 0.56),
        from_hsl(h + 22.0, s, 0.58),
    ];
    p.accent = [hex(stops[0]), hex(stops[1]), hex(stops[2])];
    let surface = from_hsl(h, s * 0.35, 0.09);
    p.surface = hex(surface);
    p.surface_raised = Some(hex(surface.lerp(INK, 0.06)));
    p.ink = Some(hex(INK));
    p.mist = Some(hex(MIST.with_alpha(1.0)));
    let g = p.glass.get_or_insert_with(Default::default);
    g.tint = Some(hex(surface.lerp(NOTCH, 0.22).lerp(stops[1], 0.05)));
    g.rim = Some(hex(GLASS_RIM.lerp(stops[0], 0.12)));
    g.sheen = Some(hex(GLASS_SHEEN.lerp(stops[0], 0.18)));
    g.shadow = Some(hex(GLASS_SHADOW.lerp(stops[2], 0.18)));
    g.glow = Some(hex(stops[1]));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ThemePack {
        pack_of(&super::super::preset("aurora").unwrap())
    }

    #[test]
    fn v2_round_trip_and_v1_compat() {
        let p = sample();
        let json = serde_json::to_string(&p).unwrap();
        let back: ThemePack = serde_json::from_str(&json).unwrap();
        assert_eq!(back.to_theme().unwrap(), p.to_theme().unwrap());
        // a v1 file (no version, no v2 fields) loads unchanged
        let v1: ThemePack = serde_json::from_str(
            r##"{"id":"x","name":"X","accent":["#FFF","#888","#000"],"surface":"#111"}"##,
        )
        .unwrap();
        let t = v1.to_theme().unwrap();
        assert_eq!(t.ink, INK);
        assert_eq!(t.glass_strength, None);
        // unknown fields are ignored
        let extra: Result<ThemePack, _> = serde_json::from_str(
            r##"{"id":"x","name":"X","accent":["#FFF","#888","#000"],"surface":"#111","future":1}"##,
        );
        assert!(extra.is_ok());
        // out-of-range glass defaults are refused
        let mut bad = sample();
        bad.glass.as_mut().unwrap().blur = Some(3.0);
        assert!(bad.to_theme().is_err());
    }

    #[test]
    fn share_code_round_trip() {
        let p = sample();
        let code = share_code(&p);
        assert!(code.starts_with(SHARE_PREFIX));
        assert!(!code.contains('=') && !code.contains('+') && !code.contains('/'));
        let back = from_share_code(&format!("  {code}\n")).unwrap();
        assert_eq!(back.to_theme().unwrap(), p.to_theme().unwrap());
        assert!(from_share_code("hello").is_err());
        assert!(from_share_code("NTH2:!!!").is_err());
        // valid base64 but not a theme
        assert!(from_share_code(&format!("{SHARE_PREFIX}{}", b64url(b"{\"id\":1}"))).is_err());
        for n in 0..8u8 {
            let data: Vec<u8> = (0..n).map(|i| i.wrapping_mul(97)).collect();
            assert_eq!(unb64url(&b64url(&data)).unwrap(), data);
        }
    }

    #[test]
    fn contrast_guard_fixes_low_contrast() {
        let mut p = sample();
        p.ink = Some("#3A3A3A".into());
        p.mist = Some("#2A2A2A".into());
        assert!(!contrast_ok(&p.to_theme().unwrap()));
        assert!(fix_contrast(&mut p));
        let (i, m) = contrast(&p.to_theme().unwrap());
        assert!(i >= AA_TEXT && m >= AA_SECONDARY, "{i} {m}");
        // every built-in passes as shipped
        for pr in super::super::PRESETS {
            let t = super::super::preset(pr.id).unwrap();
            assert!(contrast(&t).0 >= AA_TEXT, "{}", pr.id);
        }
    }

    #[test]
    fn ids_and_names() {
        let taken = vec!["pack:my-theme".to_string(), "midnight".to_string()];
        assert_eq!(unique_id("My Theme!", &taken), "my-theme-2");
        assert_eq!(unique_id("Midnight", &taken), "midnight-2");
        assert_eq!(unique_id("***", &taken), "theme");
        let names = vec!["Aurora copy".to_string()];
        assert_eq!(copy_name("Aurora", &names), "Aurora copy 2");
    }

    #[test]
    fn one_colour_makes_a_valid_theme() {
        let mut p = sample();
        from_one_colour(&mut p, Color::rgb(0xE0457B));
        let t = p.to_theme().unwrap();
        assert!(t.accent[1].r > t.accent[1].b);
        assert!(t.surface.luminance() < 0.15);
        assert!(contrast_ok(&t));
    }

    #[test]
    fn undo_coalesces_bursts() {
        let mut s = Studio::default();
        let mut p = sample();
        p.id = "a".into();
        s.user.push(p.clone());
        s.checkpoint(&p);
        s.checkpoint(&p); // same burst
        assert_eq!(s.undo.len(), 1);
        assert!(s.undo().is_some());
        assert!(s.can_redo());
        assert!(!s.can_undo());
    }
}
