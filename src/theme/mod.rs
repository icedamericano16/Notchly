//! Theme engine: presets, JSON theme packs and album-art adaptive colour.

pub mod adaptive;
pub mod tokens;

use serde::{Deserialize, Serialize};
use tokens::*;

use crate::motion::SpringCfg;

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub id: String,
    pub name: String,
    pub accent: [Color; 3],
    pub surface: Color,
    pub surface_raised: Color,
    pub ink: Color,
    pub mist: Color,
    pub radius_scale: f32,
    pub font_override: Option<String>,
    pub spring_overrides: Option<SpringOverrides>,
    /// High-contrast mode: no glow, system colours.
    pub high_contrast: bool,
    /// How this theme's glass looks (derived for every preset and pack).
    pub glass: GlassTheme,
}

/// A theme's glass: the tint behind content, the rim light, the sheen, the shadow tint and the
/// glow behind active controls.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlassTheme {
    pub tint: Color,
    pub rim: Color,
    pub sheen: Color,
    pub shadow_tint: Color,
    pub accent_glow: Color,
}

impl GlassTheme {
    /// The glass a theme gets unless it says otherwise: its surface deepened toward the
    /// notch with a whisper of the accent, a rim lit slightly toward the accent.
    pub fn derive(surface: Color, accent: &[Color; 3]) -> Self {
        Self {
            tint: surface.lerp(NOTCH, 0.22).lerp(accent[1], 0.05),
            rim: GLASS_RIM.lerp(accent[0], 0.12),
            sheen: GLASS_SHEEN.lerp(accent[0], 0.18),
            shadow_tint: GLASS_SHADOW.lerp(accent[2], 0.18),
            accent_glow: accent[1],
        }
    }
}

/// Optional glass overrides in a theme pack (`"glass": {...}`), all `#RRGGBB`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GlassPack {
    #[serde(default)]
    pub tint: Option<String>,
    #[serde(default)]
    pub rim: Option<String>,
    #[serde(default)]
    pub sheen: Option<String>,
    #[serde(default)]
    pub shadow: Option<String>,
    #[serde(default)]
    pub glow: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpringOverrides {
    pub island: Option<(f32, f32)>,
    pub gentle: Option<(f32, f32)>,
}

impl SpringOverrides {
    pub fn island(&self) -> Option<SpringCfg> {
        self.island.map(|(k, c)| SpringCfg {
            stiffness: k,
            damping: c,
        })
    }
}

impl Default for Theme {
    fn default() -> Self {
        preset("midnight").unwrap_or_else(|| Theme {
            id: "midnight".into(),
            name: "Midnight".into(),
            accent: ACCENT,
            surface: SURFACE,
            surface_raised: SURFACE_RAISED,
            ink: INK,
            mist: MIST,
            radius_scale: 1.0,
            font_override: None,
            spring_overrides: None,
            high_contrast: false,
            glass: GlassTheme::derive(SURFACE, &ACCENT),
        })
    }
}

pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    pub stops: [u32; 3],
    pub surface: u32,
    /// Glass-first presets set their tint and rim (others derive them).
    pub glass: Option<(u32, u32)>,
}

pub const PRESETS: &[Preset] = &[
    Preset {
        id: "midnight",
        name: "Midnight",
        stops: [0x19B8FF, 0x2D5BFF, 0x7B4BFF],
        surface: 0x1B1C22,
        glass: None,
    },
    Preset {
        id: "aurora",
        name: "Aurora",
        stops: [0x2EE6C5, 0x19B8FF, 0x4D7CFF],
        surface: 0x181F1D,
        glass: None,
    },
    Preset {
        id: "sunset",
        name: "Sunset",
        stops: [0xFFB547, 0xFF5E7E, 0xB34BFF],
        surface: 0x221B1E,
        glass: None,
    },
    Preset {
        id: "rose",
        name: "Rose",
        stops: [0xFF8FB1, 0xFF4F8B, 0xC13CFF],
        surface: 0x221A20,
        glass: None,
    },
    Preset {
        id: "mono",
        name: "Mono",
        stops: [0xFFFFFF, 0xC9CED9, 0x8C94A8],
        surface: 0x1C1C1E,
        glass: None,
    },
    Preset {
        id: "blossom",
        name: "Blossom",
        stops: [0xFF6482, 0xFF375F, 0xBF5AF2],
        surface: 0x1C1C1E,
        glass: None,
    },
    Preset {
        id: "ocean",
        name: "Ocean",
        stops: [0x64D2FF, 0x0A84FF, 0x5E5CE6],
        surface: 0x1A1D23,
        glass: None,
    },
    Preset {
        id: "mint",
        name: "Mint",
        stops: [0x63E6BE, 0x30D158, 0x00C7BE],
        surface: 0x1A1F1D,
        glass: None,
    },
    // glass-first presets: balanced to read on light, dark and busy wallpapers
    Preset {
        id: "crystal",
        name: "Crystal",
        stops: [0xE8F6FF, 0x9FD8FF, 0x7AA7FF],
        surface: 0x161A20,
        glass: Some((0x10141B, 0xEAF6FF)),
    },
    Preset {
        id: "pearl",
        name: "Pearl",
        stops: [0xFFF3DC, 0xF2D3A2, 0xD9A86C],
        surface: 0x1D1A16,
        glass: Some((0x1A1612, 0xFFF1DA)),
    },
    Preset {
        id: "sapphire",
        name: "Sapphire",
        stops: [0x6FB6FF, 0x2F6BFF, 0x2A3FD6],
        surface: 0x0F1424,
        glass: Some((0x0A1022, 0xBFD8FF)),
    },
    Preset {
        id: "aurora_glass",
        name: "Aurora Glass",
        stops: [0x3EF0CF, 0x39A8FF, 0x8B5CFF],
        surface: 0x111A1E,
        glass: Some((0x0C1519, 0xC9FFF4)),
    },
    Preset {
        id: "ember_glass",
        name: "Ember Glass",
        stops: [0xFFC15E, 0xFF7A59, 0xF0456E],
        surface: 0x1E1513,
        glass: Some((0x1A100D, 0xFFE2CC)),
    },
    Preset {
        id: "obsidian",
        name: "Obsidian",
        stops: [0xF2F4F8, 0xB9C0CC, 0x7D8594],
        surface: 0x0B0B0D,
        glass: Some((0x050507, 0xE6EAF2)),
    },
];

pub fn preset(id: &str) -> Option<Theme> {
    let p = PRESETS.iter().find(|p| p.id == id)?;
    let surface = Color::rgb(p.surface);
    let accent = [
        Color::rgb(p.stops[0]),
        Color::rgb(p.stops[1]),
        Color::rgb(p.stops[2]),
    ];
    let mut glass = GlassTheme::derive(surface, &accent);
    if let Some((tint, rim)) = p.glass {
        glass.tint = Color::rgb(tint);
        glass.rim = Color::rgb(rim);
    }
    Some(Theme {
        id: p.id.into(),
        name: p.name.into(),
        accent,
        surface,
        surface_raised: surface.lerp(INK, 0.06),
        ink: INK,
        mist: MIST,
        radius_scale: 1.0,
        font_override: None,
        spring_overrides: None,
        high_contrast: false,
        glass,
    })
}

/// A theme pack file (themes/*.json).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThemePack {
    pub id: String,
    pub name: String,
    pub accent: [String; 3],
    pub surface: String,
    #[serde(default)]
    pub radius_scale: Option<f32>,
    #[serde(default)]
    pub font: Option<String>,
    #[serde(default)]
    pub springs: Option<SpringOverrides>,
    #[serde(default)]
    pub glass: Option<GlassPack>,
}

impl ThemePack {
    pub fn to_theme(&self) -> Result<Theme, String> {
        let parse = |s: &String| Color::parse(s).ok_or_else(|| format!("bad colour {s}"));
        let accent = [
            parse(&self.accent[0])?,
            parse(&self.accent[1])?,
            parse(&self.accent[2])?,
        ];
        let surface = parse(&self.surface)?;
        let radius_scale = self.radius_scale.unwrap_or(1.0);
        if !(0.5..=1.5).contains(&radius_scale) {
            return Err("radius_scale out of range".into());
        }
        if self.id.is_empty() || self.id.len() > 40 {
            return Err("bad id".into());
        }
        let mut glass = GlassTheme::derive(surface, &accent);
        if let Some(g) = &self.glass {
            let opt = |s: &Option<String>| s.as_ref().map(parse).transpose();
            if let Some(c) = opt(&g.tint)? {
                glass.tint = c;
            }
            if let Some(c) = opt(&g.rim)? {
                glass.rim = c;
            }
            if let Some(c) = opt(&g.sheen)? {
                glass.sheen = c;
            }
            if let Some(c) = opt(&g.shadow)? {
                glass.shadow_tint = c;
            }
            if let Some(c) = opt(&g.glow)? {
                glass.accent_glow = c;
            }
        }
        Ok(Theme {
            id: format!("pack:{}", self.id),
            name: self.name.chars().take(40).collect(),
            accent,
            surface,
            surface_raised: surface.lerp(INK, 0.06),
            ink: INK,
            mist: MIST,
            radius_scale,
            font_override: self.font.clone(),
            spring_overrides: self.springs,
            high_contrast: false,
            glass,
        })
    }
}

/// Five built-in packs (beyond the presets) shipped as JSON so they double as examples.
pub const BUILTIN_PACKS: &[&str] = &[
    r##"{"id":"glacier","name":"Glacier","accent":["#BFF3FF","#5FC8FF","#3A7BFF"],"surface":"#081018"}"##,
    r##"{"id":"ember","name":"Ember","accent":["#FFD166","#FF7A3D","#E8344E"],"surface":"#140C08"}"##,
    r##"{"id":"forest","name":"Forest","accent":["#B8F27C","#3DDC84","#13A88A"],"surface":"#08120C"}"##,
    r##"{"id":"lavender","name":"Lavender","accent":["#E2C6FF","#A98BFF","#6E5BFF"],"surface":"#0E0B16","radius_scale":1.1}"##,
    r##"{"id":"terminal","name":"Terminal","accent":["#7CFF9B","#39D98A","#1FA971"],"surface":"#050805","font":"Cascadia Mono","radius_scale":0.8}"##,
];

pub fn themes_dir() -> std::path::PathBuf {
    crate::util::sub_dir("themes")
}

/// All available theme packs: built-ins plus valid files in themes/.
pub fn load_packs() -> Vec<Theme> {
    let mut out: Vec<Theme> = BUILTIN_PACKS
        .iter()
        .filter_map(|s| serde_json::from_str::<ThemePack>(s).ok())
        .filter_map(|p| p.to_theme().ok())
        .collect();
    if let Ok(rd) = std::fs::read_dir(themes_dir()) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            let Ok(bytes) = std::fs::read(&p) else {
                continue;
            };
            if bytes.len() > 64 * 1024 {
                continue;
            }
            match serde_json::from_slice::<ThemePack>(&bytes)
                .map_err(|e| e.to_string())
                .and_then(|t| t.to_theme())
            {
                Ok(t) => out.push(t),
                Err(e) => crate::util::log(&format!("theme pack {} invalid: {e}", p.display())),
            }
        }
    }
    out
}

/// Resolves a theme id (preset or `pack:<id>`); falls back to Midnight.
pub fn resolve(id: &str, packs: &[Theme]) -> Theme {
    preset(id)
        .or_else(|| packs.iter().find(|t| t.id == id).cloned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn packs_validate() {
        for s in BUILTIN_PACKS {
            let p: ThemePack = serde_json::from_str(s).unwrap();
            p.to_theme().unwrap();
        }
        let bad = ThemePack {
            id: "x".into(),
            name: "x".into(),
            accent: ["#zz".into(), "#000".into(), "#000".into()],
            surface: "#000".into(),
            radius_scale: None,
            font: None,
            springs: None,
            glass: None,
        };
        assert!(bad.to_theme().is_err());
    }

    #[test]
    fn glass_overrides_validate() {
        let ok: ThemePack = serde_json::from_str(
            r##"{"id":"g","name":"G","accent":["#fff","#888","#000"],"surface":"#111",
                "glass":{"tint":"#102030","rim":"#FFEEDD"}}"##,
        )
        .unwrap();
        let t = ok.to_theme().unwrap();
        assert_eq!(t.glass.tint, Color::rgb(0x102030));
        assert_eq!(t.glass.rim, Color::rgb(0xFFEEDD));
        // untouched fields are derived
        assert_eq!(t.glass.accent_glow, Color::rgb(0x888888));
        let bad: ThemePack = serde_json::from_str(
            r##"{"id":"g","name":"G","accent":["#fff","#888","#000"],"surface":"#111",
                "glass":{"rim":"nope"}}"##,
        )
        .unwrap();
        assert!(bad.to_theme().is_err());
        // packs without a glass block still load
        for s in BUILTIN_PACKS {
            let p: ThemePack = serde_json::from_str(s).unwrap();
            assert!(p.to_theme().is_ok());
        }
    }

    #[test]
    fn every_theme_has_legible_glass() {
        use crate::render::glass::vibrancy::{contrast, over};
        let packs = load_packs();
        let all: Vec<Theme> = PRESETS
            .iter()
            .filter_map(|p| preset(p.id))
            .chain(packs)
            .collect();
        assert!(all.len() >= 19);
        for t in all {
            // a fully tinted glass over white keeps body text readable
            let bg = over(
                t.glass.tint.with_alpha(GLASS.tint_alpha[2]),
                Color::rgb(0xFFFFFF),
            );
            assert!(contrast(INK, bg) >= 4.5, "{}", t.id);
        }
    }
    #[test]
    fn fallback_is_midnight() {
        assert_eq!(resolve("nope", &[]).id, "midnight");
    }
    #[test]
    fn colour_parse() {
        assert_eq!(Color::parse("#FF0000").unwrap(), Color::rgb(0xFF0000));
        assert_eq!(Color::parse("fff").unwrap(), Color::rgb(0xFFFFFF));
        assert!(Color::parse("#12").is_none());
    }
}
