//! DirectWrite text formats and a layout cache.

use std::collections::HashMap;

use windows::core::{Interface, BOOL, HSTRING, PCWSTR};
use windows::Win32::Graphics::DirectWrite::*;

use crate::theme::tokens::*;

/// Weight + optical-size axis values for Segoe UI Variable (opsz follows the point size).
pub fn axes(weight: u16, size: f32) -> [DWRITE_FONT_AXIS_VALUE; 2] {
    [
        DWRITE_FONT_AXIS_VALUE {
            axisTag: DWRITE_FONT_AXIS_TAG_WEIGHT,
            value: weight.clamp(100, 700) as f32,
        },
        DWRITE_FONT_AXIS_VALUE {
            axisTag: DWRITE_FONT_AXIS_TAG_OPTICAL_SIZE,
            value: size.clamp(8.0, 36.0),
        },
    ]
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Family {
    Display,
    Text,
    Mono,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TextStyle {
    pub family: Family,
    pub size: f32,
    pub weight: u16,
    pub tnum: bool,
    pub upper: bool,
    pub tracking: f32,
    /// Max lines when wrapping (1 = single line with ellipsis).
    pub lines: u8,
}

impl TextStyle {
    pub const fn new(family: Family, size: f32, weight: u16) -> Self {
        Self {
            family,
            size,
            weight,
            tnum: false,
            upper: false,
            tracking: 0.0,
            lines: 1,
        }
    }
    pub const fn tnum(mut self) -> Self {
        self.tnum = true;
        self
    }
    pub const fn lines(mut self, n: u8) -> Self {
        self.lines = n;
        self
    }
    pub const fn size(mut self, s: f32) -> Self {
        self.size = s;
        self
    }
    pub const fn weight(mut self, w: u16) -> Self {
        self.weight = w;
        self
    }
}

pub const DISPLAY: TextStyle = TextStyle {
    tracking: DISPLAY_TRACKING,
    ..TextStyle::new(Family::Display, DISPLAY_SIZE, 600).tnum()
};
pub const TITLE: TextStyle = TextStyle::new(Family::Text, TITLE_SIZE, WEIGHT_TITLE);
pub const BODY: TextStyle = TextStyle::new(Family::Text, BODY_SIZE, 400);
pub const BODY_MED: TextStyle = TextStyle::new(Family::Text, BODY_SIZE, WEIGHT_TITLE);
pub const CAPTION: TextStyle = TextStyle::new(Family::Text, CAPTION_SIZE, WEIGHT_CAPTION);
pub const CAPTION_TNUM: TextStyle = CAPTION.tnum();
pub const MICRO: TextStyle = TextStyle {
    family: Family::Text,
    size: MICRO_SIZE,
    weight: 600,
    tnum: false,
    upper: true,
    tracking: MICRO_TRACKING,
    lines: 1,
};
pub const MONO: TextStyle = TextStyle::new(Family::Mono, 12.0, 400);

// Settings 2.0
pub const SET_TITLE: TextStyle = TextStyle {
    tracking: SET_TITLE_TRACKING,
    ..TextStyle::new(Family::Display, SET_TITLE_SIZE, SET_TITLE_WEIGHT)
};
pub const SET_CARD: TextStyle = TextStyle::new(Family::Text, SET_CARD_SIZE, SET_CARD_WEIGHT);
pub const SET_LABEL: TextStyle = TextStyle::new(Family::Text, SET_LABEL_SIZE, 500);
pub const SET_DESC: TextStyle = TextStyle::new(Family::Text, SET_DESC_SIZE, 400);
pub const SET_GROUP: TextStyle = TextStyle {
    upper: true,
    tracking: SET_GROUP_TRACKING,
    ..TextStyle::new(Family::Text, MICRO_SIZE, 600)
};
pub const SET_VALUE: TextStyle = TextStyle::new(Family::Text, SET_LABEL_SIZE, 500).tnum();

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Clone)]
pub struct Laid {
    pub layout: IDWriteTextLayout,
    pub width: f32,
    pub height: f32,
}

pub struct TextCache {
    dwrite: IDWriteFactory,
    /// Segoe UI Variable through its real axes (weight, optical size), when DirectWrite and
    /// the font support it (Windows 11); otherwise named instances as before.
    variable: Option<(IDWriteFactory6, IDWriteFontCollection2)>,
    families: [String; 3],
    pub family_override: Option<String>,
    formats: HashMap<(u8, u32, u16, bool), IDWriteTextFormat>,
    layouts: HashMap<u64, Laid>,
}

impl TextCache {
    pub fn new(dwrite: &IDWriteFactory) -> Self {
        let display = resolve_family(dwrite, FONT_DISPLAY);
        let text = resolve_family(dwrite, FONT_TEXT);
        let mono = resolve_family(dwrite, &["Cascadia Mono", "Consolas"]);
        let variable = dwrite.cast::<IDWriteFactory6>().ok().and_then(|f6| unsafe {
            let coll = f6
                .GetSystemFontCollection(false, DWRITE_FONT_FAMILY_MODEL_TYPOGRAPHIC)
                .ok()?;
            let (mut idx, mut exists) = (0u32, BOOL(0));
            coll.FindFamilyName(&HSTRING::from(FONT_VARIABLE), &mut idx, &mut exists)
                .ok()?;
            exists.as_bool().then_some((f6, coll))
        });
        Self {
            dwrite: dwrite.clone(),
            variable,
            families: [display, text, mono],
            family_override: None,
            formats: HashMap::new(),
            layouts: HashMap::new(),
        }
    }

    pub fn set_override(&mut self, f: Option<String>) {
        if self.family_override != f {
            self.family_override = f;
            self.formats.clear();
            self.layouts.clear();
        }
    }

    fn format(&mut self, st: &TextStyle) -> Option<IDWriteTextFormat> {
        let fam = st.family as u8;
        let key = (fam, st.size.to_bits(), st.weight, st.lines == 1);
        if let Some(f) = self.formats.get(&key) {
            return Some(f.clone());
        }
        let name = match (&self.family_override, st.family) {
            (Some(o), Family::Display | Family::Text) => o.clone(),
            _ => self.families[fam as usize].clone(),
        };
        // the variable font: exact weight (590, 510 ...) and optical size matched to the size
        let var = match (&self.variable, &self.family_override, st.family) {
            (Some((f6, coll)), None, Family::Display | Family::Text) => unsafe {
                let coll: IDWriteFontCollection = coll.cast().ok()?;
                f6.CreateTextFormat(
                    &HSTRING::from(FONT_VARIABLE),
                    &coll,
                    &axes(st.weight, st.size),
                    st.size,
                    &HSTRING::from(""),
                )
                .ok()
                .and_then(|f| f.cast::<IDWriteTextFormat>().ok())
            },
            _ => None,
        };
        let f = match var {
            Some(f) => f,
            None => unsafe {
                self.dwrite
                    .CreateTextFormat(
                        &HSTRING::from(name),
                        None,
                        DWRITE_FONT_WEIGHT(st.weight as i32),
                        DWRITE_FONT_STYLE_NORMAL,
                        DWRITE_FONT_STRETCH_NORMAL,
                        st.size,
                        &HSTRING::from(""),
                    )
                    .ok()?
            },
        };
        unsafe {
            if st.lines == 1 {
                let _ = f.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP);
            } else {
                let _ = f.SetWordWrapping(DWRITE_WORD_WRAPPING_WRAP);
            }
            let trimming = DWRITE_TRIMMING {
                granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
                delimiter: 0,
                delimiterCount: 0,
            };
            if let Ok(sign) = self.dwrite.CreateEllipsisTrimmingSign(&f) {
                let _ = f.SetTrimming(&trimming, &sign);
            }
        }
        self.formats.insert(key, f.clone());
        Some(f)
    }

    /// Lays out `s` in a box `max_w` wide. Results are cached.
    pub fn layout(&mut self, s: &str, st: &TextStyle, max_w: f32, align: Align) -> Option<Laid> {
        let max_w = max_w.max(1.0);
        let mut h = crate::util::fnv1a(s);
        for v in [
            st.family as u32,
            st.size.to_bits(),
            st.weight as u32,
            st.tnum as u32,
            st.upper as u32,
            st.lines as u32,
            max_w.round() as u32,
            align as u32,
            st.tracking.to_bits(),
        ] {
            h = (h ^ v as u64).wrapping_mul(0x100000001b3);
        }
        if let Some(l) = self.layouts.get(&h) {
            return Some(l.clone());
        }
        if self.layouts.len() > 600 {
            self.layouts.clear();
        }
        let fmt = self.format(st)?;
        let owned;
        let text: &str = if st.upper {
            owned = s.to_uppercase();
            &owned
        } else {
            s
        };
        let w: Vec<u16> = text.encode_utf16().collect();
        let max_h = if st.lines <= 1 {
            st.size * 2.0
        } else {
            st.size * 1.36 * st.lines as f32
        };
        let layout = unsafe { self.dwrite.CreateTextLayout(&w, &fmt, max_w, max_h).ok()? };
        let all = DWRITE_TEXT_RANGE {
            startPosition: 0,
            length: w.len() as u32,
        };
        unsafe {
            let _ = layout.SetTextAlignment(match align {
                Align::Left => DWRITE_TEXT_ALIGNMENT_LEADING,
                Align::Center => DWRITE_TEXT_ALIGNMENT_CENTER,
                Align::Right => DWRITE_TEXT_ALIGNMENT_TRAILING,
            });
            if st.tnum {
                if let Ok(t) = self.dwrite.CreateTypography() {
                    let _ = t.AddFontFeature(DWRITE_FONT_FEATURE {
                        nameTag: DWRITE_FONT_FEATURE_TAG_TABULAR_FIGURES,
                        parameter: 1,
                    });
                    let _ = layout.SetTypography(&t, all);
                }
            }
            if st.tracking != 0.0 {
                if let Ok(l1) = layout.cast::<IDWriteTextLayout1>() {
                    let _ = l1.SetCharacterSpacing(0.0, st.tracking * st.size, 0.0, all);
                }
            }
        }
        let mut m = DWRITE_TEXT_METRICS::default();
        unsafe {
            let _ = layout.GetMetrics(&mut m);
        }
        let laid = Laid {
            layout,
            width: m.widthIncludingTrailingWhitespace.min(max_w),
            height: m.height,
        };
        self.layouts.insert(h, laid.clone());
        Some(laid)
    }

    /// Whether text uses the variable font's axes (for range styling).
    pub fn variable(&self) -> bool {
        self.variable.is_some() && self.family_override.is_none()
    }

    /// A fresh, uncached, wrapping layout the caller may style by range (font size, weight,
    /// strikethrough, drawing effects) without touching the shared cache.
    pub fn fresh_layout(
        &mut self,
        s: &str,
        st: &TextStyle,
        max_w: f32,
        max_h: f32,
    ) -> Option<IDWriteTextLayout> {
        let fmt = self.format(&st.lines(st.lines.max(2)))?;
        let w: Vec<u16> = s.encode_utf16().collect();
        unsafe {
            self.dwrite
                .CreateTextLayout(&w, &fmt, max_w.max(1.0), max_h.max(1.0))
                .ok()
        }
    }

    pub fn measure(&mut self, s: &str, st: &TextStyle) -> f32 {
        self.layout(s, st, 4000.0, Align::Left)
            .map(|l| l.width)
            .unwrap_or(0.0)
    }

    pub fn clear(&mut self) {
        self.layouts.clear();
    }
}

fn resolve_family(dwrite: &IDWriteFactory, candidates: &[&str]) -> String {
    let mut coll: Option<IDWriteFontCollection> = None;
    unsafe {
        let _ = dwrite.GetSystemFontCollection(&mut coll, false);
    }
    if let Some(c) = coll {
        for name in candidates {
            let w = crate::util::wide(name);
            let mut idx = 0u32;
            let mut exists = BOOL(0);
            unsafe {
                if c.FindFamilyName(PCWSTR(w.as_ptr()), &mut idx, &mut exists)
                    .is_ok()
                    && exists.as_bool()
                {
                    return name.to_string();
                }
            }
        }
    }
    candidates.last().copied().unwrap_or("Segoe UI").to_string()
}
