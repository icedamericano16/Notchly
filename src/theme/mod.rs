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
        })
    }
}

pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    pub stops: [u32; 3],
    pub surface: u32,
}

pub const PRESETS: &[Preset] = &[
    Preset {
        id: "midnight",
        name: "Midnight",
        stops: [0x19B8FF, 0x2D5BFF, 0x7B4BFF],
        surface: 0x1B1C22,
    },
    Preset {
        id: "aurora",
        name: "Aurora",
        stops: [0x2EE6C5, 0x19B8FF, 0x4D7CFF],
        surface: 0x181F1D,
    },
    Preset {
        id: "sunset",
        name: "Sunset",
        stops: [0xFFB547, 0xFF5E7E, 0xB34BFF],
        surface: 0x221B1E,
    },
    Preset {
        id: "rose",
        name: "Rose",
        stops: [0xFF8FB1, 0xFF4F8B, 0xC13CFF],
        surface: 0x221A20,
    },
    Preset {
        id: "mono",
        name: "Mono",
        stops: [0xFFFFFF, 0xC9CED9, 0x8C94A8],
        surface: 0x1C1C1E,
    },
    Preset {
        id: "blossom",
        name: "Blossom",
        stops: [0xFF6482, 0xFF375F, 0xBF5AF2],
        surface: 0x1C1C1E,
    },
    Preset {
        id: "ocean",
        name: "Ocean",
        stops: [0x64D2FF, 0x0A84FF, 0x5E5CE6],
        surface: 0x1A1D23,
    },
    Preset {
        id: "mint",
        name: "Mint",
        stops: [0x63E6BE, 0x30D158, 0x00C7BE],
        surface: 0x1A1F1D,
    },
];

pub fn preset(id: &str) -> Option<Theme> {
    let p = PRESETS.iter().find(|p| p.id == id)?;
    let surface = Color::rgb(p.surface);
    Some(Theme {
        id: p.id.into(),
        name: p.name.into(),
        accent: [
            Color::rgb(p.stops[0]),
            Color::rgb(p.stops[1]),
            Color::rgb(p.stops[2]),
        ],
        surface,
        surface_raised: surface.lerp(INK, 0.06),
        ink: INK,
        mist: MIST,
        radius_scale: 1.0,
        font_override: None,
        spring_overrides: None,
        high_contrast: false,
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
        };
        assert!(bad.to_theme().is_err());
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
