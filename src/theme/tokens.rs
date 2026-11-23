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
pub const MICRO_TRACKING: f32 = 0.05;
/// Display numerals sit a touch tighter.
pub const DISPLAY_TRACKING: f32 = -0.01;
/// The variable family (weight and optical-size axes) used where available.
pub const FONT_VARIABLE: &str = "Segoe UI Variable";
/// Fractional weights on the variable font (the static fallback snaps to 600 / 500).
pub const WEIGHT_TITLE: u16 = 590;
pub const WEIGHT_CAPTION: u16 = 510;

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
pub const CANVAS_W: f32 = 700.0;
pub const CANVAS_H: f32 = 420.0;
pub const FLOAT_OFFSET: f32 = 10.0;
pub const SHADOW_MARGIN: f32 = 60.0;
/// The window keeps a small margin even when attached so glows are not clipped.
pub const ATTACHED_MARGIN: f32 = 32.0;

pub const R_TILE: f32 = 18.0;
pub const R_ART: f32 = 14.0;
pub const R_ART_LIVE: f32 = 6.0;
pub const R_INPUT: f32 = 12.0;
pub const ICON_STROKE: f32 = 1.75;
/// Icon stroke (DIP) beside regular (400) and bold (700) text: icons and labels read as one
/// family.
pub const ICON_WEIGHT_STROKE: (f32, f32) = (1.5, 2.1);
/// Layered glass glyphs: plate corner (fraction of height), icon size (fraction of height),
/// top light, and the active glow.
pub const GLYPH_RADIUS: f32 = 0.32;
pub const GLYPH_ICON: f32 = 0.6;
pub const GLYPH_TOP_LIGHT: f32 = 0.3;
pub const GLYPH_GLOW: f32 = 0.38;
/// Bell shake on a new notification (s) and the play/pause morph's turn (rad).
pub const BELL_SHAKE_SECS: f32 = 0.9;
pub const MORPH_TURN: f32 = 0.6;
pub const FOCUS_RING: f32 = 2.0;
pub const PAD: f32 = 16.0;
/// Content inside the notch starts below this (the top part sits behind the bezel).
pub const TOP_INSET: f32 = 10.0;

// ----- Liquid Glass (every glass number lives here)

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlassTokens {
    /// Tint alpha at the strength stops Clearest (0), Default (0.5), Tinted (1).
    pub tint_alpha: [f32; 3],
    /// Same for tinted glass (nothing is captured, so it carries more of the look).
    pub tint_alpha_tinted: [f32; 3],
    /// Gaussian blur sigma (DIP) for blur 0 and 1, before the strength factor.
    pub sigma: (f32, f32),
    /// Strength scales sigma between these factors (clear glass is a little sharper).
    pub sigma_strength: (f32, f32),
    /// Edge lens band (DIP) for small and large shapes.
    pub lens_band: (f32, f32),
    /// Largest displacement (DIP) at refraction 1.
    pub lens_max: f32,
    /// Falloff exponent of the lens across the band.
    pub lens_exponent: f32,
    /// Lens map texel size (DIP).
    pub lens_texel: f32,
    /// Saturation lift for strength 1 and 0 (clear glass lifts more).
    pub saturation: (f32, f32),
    /// Luminance compression toward mid grey (0 = none).
    pub luma_compress: f32,
    /// Rim stroke width (DIP) at rim light 0 and 1.
    pub rim_width: (f32, f32),
    /// Rim alpha where the edge faces the light / faces away.
    pub rim_alpha_lit: f32,
    pub rim_alpha_unlit: f32,
    pub inner_shadow_blur: f32,
    pub inner_shadow_alpha: f32,
    pub inner_shadow_lift: f32,
    pub inner_glow_alpha: f32,
    /// Tinted glass has no real lens: a soft inner band of light stands in.
    pub fake_lens_alpha: f32,
    /// Glass (frosted) refreshes the backdrop at most this often while the notch is still.
    pub frosted_hz: f32,
    /// Glass, frosted acrylic: blur σ range (DIP), tint alpha at the strength stops, a milky
    /// white haze (alpha at strength 0 and 1), fine grain, muted colour, a soft even rim.
    pub frost_sigma: (f32, f32),
    pub frost_tint_alpha: [f32; 3],
    pub frost_haze: (f32, f32),
    pub frost_grain: f32,
    pub frost_saturation: f32,
    pub frost_rim_lit: f32,
    pub frost_rim_unlit: f32,
    pub frost_rim_width: f32,
    /// Grain texture size (texels).
    pub grain_size: u32,
    /// Sheen radius as a fraction of the shape width, resting and press-flash alpha.
    pub sheen_radius: f32,
    pub sheen_alpha: f32,
    pub sheen_flash_alpha: f32,
    /// Outer shadow (floating style).
    pub shadow_blur: f32,
    pub shadow_alpha: f32,
    pub shadow_dy: f32,
    /// Glass-on-glass tiles: tint over the glass, rim alpha, top light.
    pub tile_tint: f32,
    pub tile_tint_raised: f32,
    pub tile_rim: f32,
    pub tile_top_light: f32,
    /// Light direction (unit-ish vector, top-left by default).
    pub light: (f32, f32),
    /// How far the light / sheen may drift toward the pointer (0-1 of the way).
    pub pointer_pull: f32,
    /// Motion: max stretch along the motion axis, and the velocity (DIP/s) giving it.
    pub stretch_max: f32,
    pub stretch_velocity: f32,
    /// Arrival ripple length (ms) and lens boost.
    pub ripple_ms: f32,
    pub ripple_boost: f32,
    /// Idle shimmer period (s) and sweep length (s).
    pub shimmer_period: f32,
    pub shimmer_sweep: f32,
    /// Backdrop capture margin around the canvas (DIP) and refresh caps (Hz).
    pub capture_margin: f32,
    pub backdrop_hz_static: f32,
    /// Backdrop brightness sampling interval (ms) and grid size.
    pub luma_sample_ms: u64,
    pub luma_grid: u32,
    /// Tint floor before the first backdrop measurement.
    pub unmeasured_floor: f32,
    /// Contrast targets: primary body text, secondary text, large/decorative text.
    pub contrast_primary: f32,
    pub contrast_secondary: f32,
    pub contrast_large: f32,
    /// How far secondary text may brighten toward primary to stay legible.
    pub secondary_lift_max: f32,
    /// High-contrast rim width (DIP).
    pub hc_rim: f32,
}

pub const GLASS: GlassTokens = GlassTokens {
    tint_alpha: [0.03, 0.30, 0.80],
    tint_alpha_tinted: [0.84, 0.88, 0.94],
    sigma: (3.0, 26.0),
    sigma_strength: (0.75, 1.15),
    lens_band: (12.0, 20.0),
    lens_max: 13.0,
    lens_exponent: 2.2,
    lens_texel: 2.0,
    saturation: (1.35, 1.8),
    luma_compress: 0.16,
    rim_width: (0.9, 1.3),
    rim_alpha_lit: 0.62,
    rim_alpha_unlit: 0.30,
    inner_shadow_blur: 5.0,
    inner_shadow_alpha: 0.42,
    inner_shadow_lift: 3.0,
    inner_glow_alpha: 0.14,
    fake_lens_alpha: 0.10,
    frosted_hz: 20.0,
    frost_sigma: (30.0, 48.0),
    frost_tint_alpha: [0.12, 0.38, 0.82],
    frost_haze: (0.11, 0.06),
    frost_grain: 0.07,
    frost_saturation: 1.04,
    frost_rim_lit: 0.26,
    frost_rim_unlit: 0.18,
    frost_rim_width: 1.6,
    grain_size: 96,
    sheen_radius: 0.55,
    sheen_alpha: 0.07,
    sheen_flash_alpha: 0.24,
    shadow_blur: 18.0,
    shadow_alpha: 0.42,
    shadow_dy: 10.0,
    tile_tint: 0.055,
    tile_tint_raised: 0.09,
    tile_rim: 0.16,
    tile_top_light: 0.08,
    light: (-0.55, -0.83),
    pointer_pull: 0.35,
    stretch_max: 0.02,
    stretch_velocity: 2400.0,
    ripple_ms: 350.0,
    ripple_boost: 0.9,
    shimmer_period: 12.0,
    shimmer_sweep: 1.6,
    capture_margin: 0.0,
    backdrop_hz_static: 30.0,
    luma_sample_ms: 250,
    luma_grid: 8,
    unmeasured_floor: 0.34,
    contrast_primary: 4.5,
    contrast_secondary: 4.5,
    contrast_large: 3.0,
    secondary_lift_max: 0.5,
    hc_rim: 2.0,
};

/// Tinted glass can't see what's behind it: it keeps text legible over this (near white).
pub const GLASS_TINTED_WORST: Color = Color::rgb(0xF2F2F2);
/// How far outside the notch outline the pointer still counts as "on" it (DIP), and how often
/// the cursor is polled while the margins are click-through (ms).
pub const HIT_TOLERANCE: f32 = 2.0;
pub const HIT_POLL_MS: u32 = 30;

/// Neutral glass tint when a theme gives none (cool near-black).
pub const GLASS_TINT: Color = Color::rgb(0x14151B);
pub const GLASS_RIM: Color = Color::rgb(0xFFFFFF);
pub const GLASS_SHEEN: Color = Color::rgb(0xFFFFFF);
pub const GLASS_SHADOW: Color = Color::rgb(0x05060A);
/// Inner shadow colour (deep, slightly cool).
pub const GLASS_INNER_SHADOW: Color = Color::rgb(0x000000);
/// The procedural sample wallpaper behind the Settings preview.
pub const PREVIEW_WALL: [Color; 4] = [
    Color::rgb(0xFF9A5C),
    Color::rgb(0x6A5CFF),
    Color::rgb(0x18D1C5),
    Color::rgb(0xF2F4FF),
];

// ----- shelf: dropping onto a soft surface

pub struct SoftDrop {
    /// Fall height (DIP) and time (s); fade-in time while it appears.
    pub fall: f32,
    pub fall_secs: f32,
    pub fade_secs: f32,
    /// Vertical stretch at full fall speed, and how fast it melts on contact (s).
    pub stretch: f32,
    pub stretch_release: f32,
    /// The cushion: natural frequency (Hz) and damping ratio.
    pub freq: f32,
    pub damping: f32,
    /// Compression (DIP) becomes: sinking (DIP per DIP), rebound lift, and squash per DIP.
    pub sink: f32,
    pub lift: f32,
    pub squash_per_dip: f32,
    /// After touchdown it has settled by this time (s).
    pub settle_secs: f32,
    /// Contact shadow: alpha, width as a fraction of the card, fade after settling (s).
    pub shadow_alpha: f32,
    pub shadow_width: f32,
    pub shadow_fade: f32,
    /// Stagger between several dropped cards (s).
    pub stagger: f32,
    /// Landing effects: length (s), glow, surface ripple, puffs.
    pub burst_secs: f32,
    pub glow_secs: f32,
    pub glow_alpha: f32,
    pub ripple_secs: f32,
    pub ripple_gap: f32,
    pub ripple_alpha: f32,
    pub puffs: u32,
    pub puff_secs: f32,
    pub puff_speed: f32,
    pub puff_drag: f32,
    pub puff_rise: f32,
    pub puff_alpha: f32,
}

pub const SOFT_DROP: SoftDrop = SoftDrop {
    fall: 30.0,
    fall_secs: 0.40,
    fade_secs: 0.12,
    stretch: 0.045,
    stretch_release: 0.05,
    freq: 2.1,
    damping: 0.36,
    sink: 0.15,
    lift: 1.5,
    squash_per_dip: 0.0135,
    settle_secs: 0.85,
    shadow_alpha: 0.32,
    shadow_width: 0.42,
    shadow_fade: 0.3,
    stagger: 0.09,
    burst_secs: 1.1,
    glow_secs: 0.7,
    glow_alpha: 0.22,
    ripple_secs: 0.8,
    ripple_gap: 0.14,
    ripple_alpha: 0.34,
    puffs: 8,
    puff_secs: 0.9,
    puff_speed: 70.0,
    puff_drag: 0.22,
    puff_rise: 5.0,
    puff_alpha: 0.32,
};

// ----- idle pill: a centred clock between two groups of status glyphs
/// Room for the clock ("12:45"), glyph widths, the gap between clock and glyphs, the inner
/// padding, and the minute roll (s, DIP).
pub const IDLE_CLOCK_W: f32 = 46.0;
pub const IDLE_GLYPH_W: f32 = 22.0;
pub const IDLE_CHARGING_W: f32 = 44.0;
pub const IDLE_GAP: f32 = 8.0;
pub const IDLE_PAD: f32 = 6.0;
pub const IDLE_ROLL_SECS: f32 = 0.38;
pub const IDLE_ROLL_DIP: f32 = 7.0;
/// Privacy dots: spacing and where the first one sits from the right edge (DIP).
pub const PRIVACY_DOT_STEP: f32 = 9.0;
pub const PRIVACY_DOT_INSET: f32 = 12.5;

// ----- Control Center
/// Mouse wheel: one notch moves a slider this far (and snaps to multiples of it).
pub const CTL_WHEEL_STEP: f32 = 0.05;
/// Trackpad / free wheel: a full notch's worth of travel moves the slider this far.
pub const CTL_PAD_GAIN: f32 = 0.022;
/// A scroll gesture continues from its own target for this long (ms).
pub const CTL_WHEEL_HOLD_MS: u64 = 450;
/// Brightness is written to the display at most this often while scrolling (ms).
pub const CTL_BRIGHT_MS: u64 = 90;
/// The slider's elastic edge: stretch per unit of overscroll and its cap.
pub const CTL_BAND_GAIN: f32 = 0.9;
pub const CTL_BAND_MAX: f32 = 0.07;
/// After asking an app to close, offer Force quit after this long (s).
pub const CLOSE_GRACE_SECS: f32 = 3.0;
/// The in-use sheet: width cap, row height.
pub const SHEET_W: f32 = 380.0;
pub const SHEET_ROW: f32 = 46.0;
/// The grid: gap, row height cap, switch width, gap between the two halves, slider width and
/// the card corner radius.
pub const CTL_GAP: f32 = 8.0;
pub const CTL_ROW_MAX: f32 = 54.0;
pub const CTL_TILE_W: f32 = 128.0;
pub const CTL_BLOCK_GAP: f32 = 4.0;
pub const CTL_SLIDER_W: f32 = 50.0;
pub const CTL_CARD_R: f32 = 16.0;
/// Toggle tints.
pub const CTL_WIFI: Color = Color::rgb(0x0A84FF);
pub const CTL_AIRPLANE: Color = Color::rgb(0xFF9F0A);
pub const CTL_FOCUS: Color = Color::rgb(0x5E5CE6);
pub const CTL_DARK: Color = Color::rgb(0x636366);
pub const CTL_MIC: Color = Color::rgb(0x30D158);
pub const CTL_CAMERA: Color = Color::rgb(0x30D158);

// ----- mascot
pub const EYE_W: f32 = 8.0;
pub const EYE_H: f32 = 10.0;
