//! Design tokens (section 4). No colour or size literal belongs anywhere else.

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn rgb(hex: u32) -> Self {
        Self {
            r: ((hex >> 16) & 0xff) as f32 / 255.0,
            g: ((hex >> 8) & 0xff) as f32 / 255.0,
            b: (hex & 0xff) as f32 / 255.0,
            a: 1.0,
        }
    }
    pub const fn rgba(hex: u32, a: f32) -> Self {
        let c = Self::rgb(hex);
        Self { a, ..c }
    }
    pub fn with_alpha(self, a: f32) -> Self {
        Self {
            a: self.a * a,
            ..self
        }
    }
    pub fn lerp(self, o: Color, t: f32) -> Self {
        Self {
            r: self.r + (o.r - self.r) * t,
            g: self.g + (o.g - self.g) * t,
            b: self.b + (o.b - self.b) * t,
            a: self.a + (o.a - self.a) * t,
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        let h = s.trim().trim_start_matches('#');
        match h.len() {
            6 => u32::from_str_radix(h, 16).ok().map(Self::rgb),
            8 => {
                let v = u32::from_str_radix(h, 16).ok()?;
                Some(Self::rgba(v >> 8, (v & 0xff) as f32 / 255.0))
            }
            3 => {
                let v = u32::from_str_radix(h, 16).ok()?;
                let (r, g, b) = ((v >> 8) & 0xf, (v >> 4) & 0xf, v & 0xf);
                Some(Self::rgb((r * 17) << 16 | (g * 17) << 8 | (b * 17)))
            }
            _ => None,
        }
    }
    pub fn luminance(self) -> f32 {
        0.2126 * self.r + 0.7152 * self.g + 0.0722 * self.b
    }
}

// ----- colour tokens (Midnight)
pub const NOTCH: Color = Color::rgb(0x000000);
pub const SURFACE: Color = Color::rgb(0x1C1C1E);
pub const SURFACE_RAISED: Color = Color::rgb(0x2C2C2E);
pub const INK: Color = Color::rgb(0xF5F5F7);
pub const MIST: Color = Color::rgb(0x98989F);
pub const FAINT: Color = Color::rgba(0xEBEBF5, 0.30);
pub const LINE: Color = Color::rgba(0xEBEBF5, 0.09);
pub const TRACK: Color = Color::rgba(0x787880, 0.36);
pub const ACCENT: [Color; 3] = [
    Color::rgb(0x19B8FF),
    Color::rgb(0x2D5BFF),
    Color::rgb(0x7B4BFF),
];
pub const ACCENT_STOPS: [f32; 3] = [0.0, 0.55, 1.0];
pub const GOOD: Color = Color::rgb(0x30D158);
pub const WARN: Color = Color::rgb(0xFF9F0A);
pub const CRITICAL: Color = Color::rgb(0xFF453A);
pub const PRIVACY_MIC: Color = Color::rgb(0xFF9F0A);
pub const PRIVACY_CAM: Color = Color::rgb(0x30D158);
pub const PRIVACY_LOC: Color = Color::rgb(0x0A84FF);
pub const SHADOW: Color = Color::rgba(0x000000, 0.6);
pub const RING: Color = Color::rgba(0xECEFF7, 0.08);
pub const HIGHLIGHT: Color = Color::rgba(0xFFFFFF, 0.08);
pub const WHITE: Color = Color::rgb(0xFFFFFF);

// ----- per-feature tints (Apple system colours, dark appearance)
pub const TINT_HOME: Color = Color::rgb(0xF5F5F7);
pub const TINT_MUSIC: Color = Color::rgb(0xFF375F);
pub const TINT_TIMER: Color = Color::rgb(0xFF9F0A);
pub const TINT_WATER: Color = Color::rgb(0x64D2FF);
pub const TINT_TODO: Color = Color::rgb(0x30D158);
pub const TINT_SHELF: Color = Color::rgb(0x5E5CE6);
pub const TINT_CLIP: Color = Color::rgb(0xFFD60A);
pub const TINT_PRAYER: Color = Color::rgb(0xBF5AF2);
pub const TINT_WEATHER: Color = Color::rgb(0x0A84FF);
pub const TINT_CALENDAR: Color = Color::rgb(0xFF453A);
pub const TINT_SYSTEM: Color = Color::rgb(0x32ADE6);
pub const TINT_WIDGETS: Color = Color::rgb(0xAC8E68);
pub const TINT_CONTROL: Color = Color::rgb(0x7D7AFF);

// ----- typography
pub const FONT_DISPLAY: &[&str] = &["Segoe UI Variable Display", "Segoe UI"];
pub const FONT_TEXT: &[&str] = &["Segoe UI Variable Text", "Segoe UI"];
pub const DISPLAY_SIZE: f32 = 40.0;
pub const TITLE_SIZE: f32 = 15.0;
pub const BODY_SIZE: f32 = 13.0;
pub const CAPTION_SIZE: f32 = 11.5;
pub const MICRO_SIZE: f32 = 10.0;
pub const MICRO_TRACKING: f32 = 0.04;

// ----- geometry (DIPs at 100 %)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapeSpec {
    pub w: f32,
    pub h: f32,
    pub radius: f32,
    pub shoulder: f32,
}

pub const SHAPE_HIDDEN: ShapeSpec = ShapeSpec {
    w: 120.0,
    h: 0.0,
    radius: 0.0,
    shoulder: 0.0,
};
pub const SHAPE_MINIMAL: ShapeSpec = ShapeSpec {
    w: 120.0,
    h: 5.0,
    radius: 2.5,
    shoulder: 3.0,
};
pub const SHAPE_IDLE: ShapeSpec = ShapeSpec {
    w: 150.0,
    h: 30.0,
    radius: 14.0,
    shoulder: 10.0,
};
pub const SHAPE_LIVE: ShapeSpec = ShapeSpec {
    w: 236.0,
    h: 32.0,
    radius: 15.0,
    shoulder: 10.0,
};
pub const SHAPE_DUAL_MAIN: ShapeSpec = ShapeSpec {
    w: 200.0,
    h: 32.0,
    radius: 15.0,
    shoulder: 10.0,
};
pub const DUAL_GAP: f32 = 8.0;
pub const BUBBLE: f32 = 32.0;
pub const SHAPE_PEEK: ShapeSpec = ShapeSpec {
    w: 310.0,
    h: 60.0,
    radius: 19.0,
    shoulder: 13.0,
};
pub const SHAPE_EXPANDED: ShapeSpec = ShapeSpec {
    w: 440.0,
    h: 150.0,
    radius: 32.0,
    shoulder: 22.0,
};
pub const SHAPE_WIDE: ShapeSpec = ShapeSpec {
    w: 540.0,
    h: 172.0,
    radius: 32.0,
    shoulder: 22.0,
};
pub const SHAPE_TALL: ShapeSpec = ShapeSpec {
    w: 440.0,
    h: 300.0,
    radius: 32.0,
    shoulder: 22.0,
};
pub const SHAPE_WIDE_TALL: ShapeSpec = ShapeSpec {
    w: 540.0,
    h: 300.0,
    radius: 32.0,
    shoulder: 22.0,
};

/// Canvas (swap chain) size in DIPs: big enough for every shape + tooltip + shadow.
pub const CANVAS_W: f32 = 620.0;
pub const CANVAS_H: f32 = 380.0;
pub const FLOAT_OFFSET: f32 = 10.0;
pub const SHADOW_MARGIN: f32 = 24.0;
/// The window keeps a small margin even when attached so glows are not clipped.
pub const ATTACHED_MARGIN: f32 = 6.0;

pub const R_TILE: f32 = 18.0;
pub const R_ART: f32 = 14.0;
pub const R_ART_LIVE: f32 = 6.0;
pub const R_INPUT: f32 = 12.0;
pub const ICON_STROKE: f32 = 1.75;
pub const FOCUS_RING: f32 = 2.0;
pub const PAD: f32 = 16.0;
/// Content inside the notch starts below this (the top part sits behind the bezel).
pub const TOP_INSET: f32 = 10.0;

// ----- mascot
pub const EYE_W: f32 = 8.0;
pub const EYE_H: f32 = 10.0;
