//! The versioned settings model. Every field has a default so older files load.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const SETTINGS_VERSION: u32 = 6;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    pub general: General,
    pub appearance: Appearance,
    pub motion: Motion,
    pub live: Live,
    pub popups: Popups,
    pub music: Music,
    pub productivity: Productivity,
    pub water: Water,
    pub ai: Ai,
    pub weather: Weather,
    pub calendar: Calendar,
    pub quiet: Quiet,
    pub shortcuts: Shortcuts,
    pub privacy: Privacy,
    pub sound: Sound,
    pub widgets: Widgets,
    pub sync: Sync,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            general: General::default(),
            appearance: Appearance::default(),
            motion: Motion::default(),
            live: Live::default(),
            popups: Popups::default(),
            music: Music::default(),
            productivity: Productivity::default(),
            water: Water::default(),
            ai: Ai::default(),
            weather: Weather::default(),
            calendar: Calendar::default(),
            quiet: Quiet::default(),
            shortcuts: Shortcuts::default(),
            privacy: Privacy::default(),
            sound: Sound::default(),
            widgets: Widgets::default(),
            sync: Sync::default(),
        }
    }
}

impl Settings {
    /// Brings every value into its valid range; values that can't be repaired (not a
    /// number, unknown option, an empty name) go back to their defaults. Run after every
    /// load and import, so a hand-edited file can never put the app in a bad state.
    pub fn sanitize(&mut self) {
        let d = Settings::default();
        let g = &mut self.general;
        g.hover_delay_ms = g.hover_delay_ms.clamp(50, 500);
        g.close_delay_ms = g.close_delay_ms.clamp(150, 1500);
        g.long_press_ms = g.long_press_ms.clamp(300, 1500);
        if g.language.trim().is_empty() {
            g.language = d.general.language.clone();
        }
        let a = &mut self.appearance;
        a.glass = a.glass.sanitized();
        if !matches!(a.size, 0 | 100 | 115 | 130 | 150) {
            a.size = 0;
        }
        a.mascot_delay_min = a.mascot_delay_min.clamp(1, 30);
        let f = &mut a.fluid;
        f.intensity = if f.intensity.is_finite() {
            f.intensity.clamp(0.0, 1.0)
        } else {
            0.7
        };
        f.speed = if f.speed.is_finite() {
            f.speed.clamp(0.25, 2.0)
        } else {
            1.0
        };
        if !crate::render::glass::fluid::STYLES
            .iter()
            .any(|s| s.id == f.style)
        {
            f.style = d.appearance.fluid.style.clone();
        }
        self.motion.speed_pct = self.motion.speed_pct.clamp(50, 200);
        if a.theme.trim().is_empty() {
            a.theme = d.appearance.theme.clone();
        }
        if a.theme_light.trim().is_empty() {
            a.theme_light = d.appearance.theme_light.clone();
        }
        a.floating_pos.retain(|_, x| x.is_finite());
        for x in a.floating_pos.values_mut() {
            *x = x.clamp(0.0, 1.0);
        }
        let p = &mut self.popups;
        p.duration_ms = p.duration_ms.clamp(800, 4000);
        p.low_battery = p.low_battery.clamp(5, 50);
        p.notification_ms = p.notification_ms.clamp(1000, 30_000);
        let pr = &mut self.productivity;
        pr.focus_min = pr.focus_min.clamp(1, 180);
        pr.short_break_min = pr.short_break_min.clamp(1, 60);
        pr.long_break_min = pr.long_break_min.clamp(1, 120);
        pr.long_every = pr.long_every.clamp(1, 12);
        pr.clipboard_max = pr.clipboard_max.clamp(10, 500);
        pr.prayer_reminder_min = pr.prayer_reminder_min.min(60);
        pr.shelf_autoclear_days = pr.shelf_autoclear_days.min(365);
        let w = &mut self.water;
        w.goal_ml = w.goal_ml.clamp(500, 6000);
        w.glass_ml = w.glass_ml.clamp(50, 1000);
        if w.start_min >= 1440 || w.end_min > 1440 || w.end_min <= w.start_min {
            w.start_min = d.water.start_min;
            w.end_min = d.water.end_min;
        }
        if !SNOOZE_CHOICES.contains(&w.snooze_min) {
            w.snooze_min = d.water.snooze_min;
        }
        let we = &mut self.weather;
        if !(we.lat.is_finite() && we.lon.is_finite())
            || we.lat.abs() > 90.0
            || we.lon.abs() > 180.0
        {
            we.lat = 0.0;
            we.lon = 0.0;
        }
        self.calendar.reminder_min = self.calendar.reminder_min.min(120);
        let q = &mut self.quiet;
        if q.start_min >= 1440 {
            q.start_min = d.quiet.start_min;
        }
        if q.end_min >= 1440 {
            q.end_min = d.quiet.end_min;
        }
        let sc = &mut self.shortcuts;
        for (v, dv) in [
            (&mut sc.toggle, &d.shortcuts.toggle),
            (&mut sc.palette, &d.shortcuts.palette),
            (&mut sc.ocr, &d.shortcuts.ocr),
            (&mut sc.clipboard, &d.shortcuts.clipboard),
            (&mut sc.focus, &d.shortcuts.focus),
            (&mut sc.translate, &d.shortcuts.translate),
            (&mut sc.screenshot, &d.shortcuts.screenshot),
        ] {
            if v.trim().is_empty() {
                *v = dv.clone();
            }
        }
        self.widgets.tiles.retain(|t| !t.trim().is_empty());
        self.widgets.tiles.dedup();
        if self.widgets.tiles.is_empty() {
            self.widgets.tiles = d.widgets.tiles.clone();
        }
        self.version = SETTINGS_VERSION;
    }

    /// Everything back to defaults, keeping what isn't a preference (onboarding done,
    /// where the floating notch was dragged to, the chosen music session).
    pub fn reset_all(&mut self) {
        let mut d = Settings::default();
        d.general.onboarded = self.general.onboarded;
        d.appearance.floating_pos = std::mem::take(&mut self.appearance.floating_pos);
        d.music.session_choice = std::mem::take(&mut self.music.session_choice);
        *self = d;
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OpenMode {
    Hover,
    Click,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct General {
    pub start_with_windows: bool,
    /// 0 = primary monitor, otherwise 1-based index in enumeration order.
    pub monitor: u32,
    pub open_mode: OpenMode,
    pub hover_delay_ms: u32,
    pub close_delay_ms: u32,
    pub language: String,
    pub onboarded: bool,
    pub show_tray: bool,
    pub clock_24h: Option<bool>,
    pub nepali_date: bool,
    pub nepali_numerals: bool,
    pub world_clocks: Vec<WorldClock>,
    /// How long a press must be held to count as a long press (ms, 300-1500).
    pub long_press_ms: u32,
    /// Write detailed input / camera / microphone events to the log.
    pub debug_log: bool,
}

impl General {
    pub fn long_press(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.long_press_ms.clamp(300, 1500) as u64)
    }
}

impl Default for General {
    fn default() -> Self {
        Self {
            start_with_windows: true,
            monitor: 0,
            open_mode: OpenMode::Hover,
            hover_delay_ms: 110,
            close_delay_ms: 450,
            language: "en".into(),
            onboarded: false,
            show_tray: true,
            clock_24h: None,
            nepali_date: false,
            nepali_numerals: false,
            world_clocks: Vec::new(),
            long_press_ms: 500,
            debug_log: false,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WorldClock {
    pub city: String,
    /// Offset from UTC in minutes.
    pub offset_min: i32,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Style {
    Attached,
    Floating,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Material {
    Solid,
    /// Windows' own blur behind the notch (DWM system backdrop).
    Glass,
    /// Notchly's own optical glass: blur, edge lensing, colour lift, rim light.
    LiquidGlass,
}

/// Where the glass tint comes from.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TintSource {
    Theme,
    /// The album art while music plays, otherwise the colour behind the notch.
    Adaptive,
    Custom,
}

/// Named starting points for the glass sliders; touching a slider makes it `Custom`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GlassPreset {
    Crystal,
    Default,
    Frosted,
    Tinted,
    Obsidian,
    Custom,
}

impl GlassPreset {
    pub const ALL: [GlassPreset; 5] = [
        GlassPreset::Crystal,
        GlassPreset::Default,
        GlassPreset::Frosted,
        GlassPreset::Tinted,
        GlassPreset::Obsidian,
    ];
    pub fn name(self) -> &'static str {
        match self {
            GlassPreset::Crystal => "Crystal",
            GlassPreset::Default => "Default",
            GlassPreset::Frosted => "Frosted",
            GlassPreset::Tinted => "Tinted",
            GlassPreset::Obsidian => "Obsidian",
            GlassPreset::Custom => "Custom",
        }
    }
    /// (strength, refraction, blur, rim light)
    pub fn values(self) -> Option<(f32, f32, f32, f32)> {
        Some(match self {
            GlassPreset::Crystal => (0.12, 0.9, 0.35, 0.8),
            GlassPreset::Default => (0.5, 0.55, 0.6, 0.6),
            GlassPreset::Frosted => (0.45, 0.15, 1.0, 0.45),
            GlassPreset::Tinted => (0.9, 0.4, 0.75, 0.5),
            GlassPreset::Obsidian => (0.97, 0.3, 0.8, 1.0),
            GlassPreset::Custom => return None,
        })
    }
}

/// The glass control panel (Appearance > Material).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct GlassSettings {
    /// 0 = clearest ... 0.5 = default ... 1 = tinted (one slider).
    pub strength: f32,
    /// Edge lensing amount, 0-1.
    pub refraction: f32,
    /// Blur amount, 0-1 (mapped to a sigma range in the tokens).
    pub blur: f32,
    pub rim_light: f32,
    pub tint_source: TintSource,
    /// `#RRGGBB` used when `tint_source` is Custom.
    pub custom_tint: String,
    pub specular_follows_pointer: bool,
    /// How much the glass stretches and ripples with motion, 0-1.
    pub motion_response: f32,
    /// Honour Windows "Transparency effects".
    pub follow_system_transparency: bool,
    /// High Contrast: solid with strong rims.
    pub follow_system_contrast: bool,
    /// Liquid Glass hides Notchly from screen capture (it has to, to see what is behind it);
    /// this keeps it visible in recordings and shares by using the system blur instead.
    pub show_in_recordings: bool,
    pub preset: GlassPreset,
    /// A very slow sheen sweep every few seconds.
    pub idle_shimmer: bool,
    /// Settings: the advanced glass options are expanded.
    pub advanced: bool,
}

impl Default for GlassSettings {
    fn default() -> Self {
        Self {
            strength: 0.5,
            refraction: 0.55,
            blur: 0.6,
            rim_light: 0.6,
            tint_source: TintSource::Theme,
            custom_tint: "#1C1C24".into(),
            specular_follows_pointer: true,
            motion_response: 0.6,
            follow_system_transparency: true,
            follow_system_contrast: true,
            show_in_recordings: false,
            preset: GlassPreset::Default,
            idle_shimmer: false,
            advanced: false,
        }
    }
}

impl GlassSettings {
    pub fn apply_preset(&mut self, p: GlassPreset) {
        if let Some((s, r, b, rim)) = p.values() {
            self.strength = s;
            self.refraction = r;
            self.blur = b;
            self.rim_light = rim;
        }
        self.preset = p;
    }
    /// Values clamped into range (hand-edited files).
    pub fn sanitized(&self) -> Self {
        let c = |v: f32| {
            if v.is_finite() {
                v.clamp(0.0, 1.0)
            } else {
                0.5
            }
        };
        Self {
            strength: c(self.strength),
            refraction: c(self.refraction),
            blur: c(self.blur),
            rim_light: c(self.rim_light),
            motion_response: c(self.motion_response),
            ..self.clone()
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Appearance {
    pub style: Style,
    pub material: Material,
    pub glass: GlassSettings,
    /// The theme for dark mode (and the only one when the mode is Dark).
    pub theme: String,
    /// The theme for light mode.
    pub theme_light: String,
    pub theme_mode: ThemeMode,
    pub adaptive: bool,
    /// 0 = auto, else percent (100/115/130/150).
    pub size: u32,
    pub minimal_idle: bool,
    pub privacy_dots: bool,
    /// Show the time and status glyphs (headphones, Bluetooth, charging) in the idle notch.
    pub idle_clock: bool,
    pub mascot: bool,
    pub mascot_delay_min: u32,
    /// Floating style horizontal anchor per monitor key, as a fraction of monitor width.
    pub floating_pos: BTreeMap<String, f32>,
    pub high_contrast_follow: bool,
    pub fluid: FluidSettings,
}

/// Settings › Glass & Fluid › Fluid.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct FluidSettings {
    /// A Fluid Style id (`still`, `flow`, `aurora` …).
    pub style: String,
    /// 0-1.
    pub intensity: f32,
    /// 0.25-2 (multiplies the global animation speed).
    pub speed: f32,
    pub react_music: bool,
    pub react_notifications: bool,
    /// Still frame while the notch is idle (no frames at all).
    pub pause_idle: bool,
}

impl Default for FluidSettings {
    fn default() -> Self {
        Self {
            style: "still".into(),
            intensity: 0.7,
            speed: 1.0,
            react_music: true,
            react_notifications: true,
            pause_idle: true,
        }
    }
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            style: Style::Attached,
            material: Material::Solid,
            glass: GlassSettings::default(),
            theme: "midnight".into(),
            theme_light: "daylight".into(),
            theme_mode: ThemeMode::Dark,
            adaptive: false,
            size: 0,
            minimal_idle: false,
            privacy_dots: true,
            idle_clock: true,
            mascot: true,
            mascot_delay_min: 3,
            floating_pos: BTreeMap::new(),
            high_contrast_follow: true,
            fluid: FluidSettings::default(),
        }
    }
}

/// Which theme is in use: the dark one, the light one, or whichever matches Windows.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    Dark,
    Light,
    System,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EqSource {
    Audio,
    Simulated,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Motion {
    /// Global animation speed, percent (50-200; 100 = as designed).
    pub speed_pct: u32,
    pub reduce_motion: bool,
    pub eq_source: EqSource,
    pub visualiser: bool,
    pub visualiser_ring: bool,
}

impl Default for Motion {
    fn default() -> Self {
        Self {
            speed_pct: 100,
            reduce_motion: false,
            eq_source: EqSource::Audio,
            visualiser: false,
            visualiser_ring: false,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Live {
    pub dual_island: bool,
    /// Activity kind name -> priority override.
    pub priority: BTreeMap<String, i32>,
    pub api_enabled: bool,
    /// Executable name -> allowed?
    pub api_apps: BTreeMap<String, bool>,
}

impl Default for Live {
    fn default() -> Self {
        Self {
            dual_island: true,
            priority: BTreeMap::new(),
            api_enabled: true,
            api_apps: BTreeMap::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Popups {
    pub volume: bool,
    pub brightness: bool,
    pub caps: bool,
    pub language: bool,
    pub battery: bool,
    pub network: bool,
    pub audio_device: bool,
    pub bluetooth: bool,
    pub mic: bool,
    pub duration_ms: u32,
    pub low_battery: u32,
    pub notifications: bool,
    pub notification_ms: u32,
    pub muted_apps: Vec<String>,
    pub keep_history: bool,
}

impl Default for Popups {
    fn default() -> Self {
        Self {
            volume: true,
            brightness: true,
            caps: true,
            language: true,
            battery: true,
            network: true,
            audio_device: true,
            bluetooth: true,
            mic: true,
            duration_ms: 1600,
            low_battery: 20,
            notifications: true,
            notification_ms: 5000,
            muted_apps: Vec::new(),
            keep_history: false,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Music {
    pub enabled: bool,
    pub preferred_player: String,
    pub lyrics: bool,
    pub lyrics_live: bool,
    pub lyrics_translate: bool,
    pub lyrics_romanise: bool,
    pub app_volume: bool,
    /// Last session chosen by the user (source app id).
    pub session_choice: String,
    /// The music page shows the lyrics panel (the caption button).
    pub captions: bool,
}

impl Default for Music {
    fn default() -> Self {
        Self {
            enabled: true,
            preferred_player: String::new(),
            lyrics: true,
            lyrics_live: false,
            lyrics_translate: false,
            lyrics_romanise: false,
            app_volume: true,
            session_choice: String::new(),
            captions: true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Productivity {
    pub focus_min: u32,
    pub short_break_min: u32,
    pub long_break_min: u32,
    pub long_every: u32,
    pub focus_dnd: bool,
    pub todo_reminders: bool,
    pub shelf_autoclear_days: u32,
    pub clipboard: bool,
    pub clipboard_max: u32,
    pub clipboard_excluded: Vec<String>,
    pub paste_on_select: bool,
    pub prayer: bool,
    pub prayer_method: String,
    pub asr_hanafi: bool,
    pub prayer_reminder_min: u32,
    pub prayer_pause_media: bool,
    pub ocr_language: String,
    pub notes: bool,
    pub downloads: bool,
    pub download_folders: Vec<String>,
    pub sysmon: bool,
    pub sysmon_alert: bool,
}

impl Default for Productivity {
    fn default() -> Self {
        Self {
            focus_min: 25,
            short_break_min: 5,
            long_break_min: 15,
            long_every: 4,
            focus_dnd: false,
            todo_reminders: true,
            shelf_autoclear_days: 0,
            clipboard: true,
            clipboard_max: 50,
            clipboard_excluded: vec![
                "KeePass.exe".into(),
                "KeePassXC.exe".into(),
                "1Password.exe".into(),
                "Bitwarden.exe".into(),
            ],
            paste_on_select: false,
            prayer: false,
            prayer_method: "MWL".into(),
            asr_hanafi: false,
            prayer_reminder_min: 10,
            prayer_pause_media: false,
            ocr_language: String::new(),
            notes: true,
            downloads: true,
            download_folders: Vec::new(),
            sysmon: true,
            sysmon_alert: false,
        }
    }
}

/// How a water reminder shows itself.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReminderStyle {
    /// A pop-up, silently.
    Popup,
    /// A pop-up with the reminder chime (when sounds are on).
    PopupSound,
    /// No pop-up: the idle notch glows softly in the water colour.
    Glow,
}

/// Settings › Water (moved out of Productivity in settings v5).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Water {
    /// Reminders on (the same switch as the Water page's bell).
    pub reminders: bool,
    pub goal_ml: u32,
    pub glass_ml: u32,
    /// Active hours, minutes after midnight.
    pub start_min: u32,
    pub end_min: u32,
    pub style: ReminderStyle,
    /// Snooze length for the pop-up's Snooze button (10, 15 or 30 minutes).
    pub snooze_min: u32,
    /// No more reminders once the day's goal is reached.
    pub stop_at_goal: bool,
}

impl Default for Water {
    fn default() -> Self {
        Self {
            reminders: true,
            goal_ml: 2000,
            glass_ml: 250,
            start_min: 9 * 60,
            end_min: 21 * 60,
            style: ReminderStyle::PopupSound,
            snooze_min: 15,
            stop_at_goal: true,
        }
    }
}

/// The snooze lengths offered in Settings › Water.
pub const SNOOZE_CHOICES: [u32; 3] = [10, 15, 30];

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Auto,
    Ollama,
    /// Retired (needed the Windows App SDK); treated as Auto.
    WindowsAi,
    Gemini,
    OpenAi,
    Anthropic,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Ai {
    pub provider: Provider,
    pub ollama_url: String,
    pub ollama_model: String,
    pub gemini_model: String,
    pub openai_model: String,
    pub anthropic_model: String,
    pub translate_target: String,
    pub commands: Vec<UserCommand>,
}

impl Default for Ai {
    fn default() -> Self {
        Self {
            provider: Provider::Auto,
            ollama_url: "http://127.0.0.1:11434".into(),
            ollama_model: String::new(),
            gemini_model: "gemini-2.5-flash".into(),
            openai_model: "gpt-4o-mini".into(),
            anthropic_model: "claude-haiku-5-5".into(),
            translate_target: "English".into(),
            commands: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct UserCommand {
    pub name: String,
    pub command: String,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Units {
    Metric,
    Imperial,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Weather {
    pub enabled: bool,
    pub units: Units,
    pub city: String,
    pub lat: f64,
    pub lon: f64,
    /// IANA time zone of `city` (e.g. "Asia/Tokyo"); empty means this PC's zone. Weather,
    /// sun and prayer times are shown in it.
    pub timezone: String,
    /// Refresh the location from Windows at start-up.
    pub use_windows_location: bool,
}

impl Default for Weather {
    fn default() -> Self {
        Self {
            enabled: true,
            units: Units::Metric,
            city: String::new(),
            lat: 0.0,
            lon: 0.0,
            timezone: String::new(),
            use_windows_location: false,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Calendar {
    pub enabled: bool,
    pub ics_urls: Vec<String>,
    pub google_client_id: String,
    pub microsoft_client_id: String,
    pub reminder_min: u32,
}

impl Default for Calendar {
    fn default() -> Self {
        Self {
            enabled: true,
            ics_urls: Vec::new(),
            google_client_id: String::new(),
            microsoft_client_id: String::new(),
            reminder_min: 10,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Quiet {
    pub schedule: bool,
    pub start_min: u32,
    pub end_min: u32,
    pub focus: bool,
    pub allow_calls: bool,
    pub allow_timers: bool,
    pub auto_meetings: bool,
    pub auto_fullscreen: bool,
    pub hide_when_sharing: bool,
    /// Executable names that never hide the notch even when fullscreen.
    pub fullscreen_allow: Vec<String>,
    /// Executable names that always hide the notch while foreground.
    pub fullscreen_deny: Vec<String>,
}

impl Default for Quiet {
    fn default() -> Self {
        Self {
            schedule: false,
            start_min: 22 * 60,
            end_min: 7 * 60,
            focus: false,
            allow_calls: true,
            allow_timers: true,
            auto_meetings: false,
            auto_fullscreen: true,
            hide_when_sharing: true,
            fullscreen_allow: Vec::new(),
            fullscreen_deny: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Shortcuts {
    pub toggle: String,
    pub palette: String,
    pub ocr: String,
    pub clipboard: String,
    pub focus: String,
    pub translate: String,
    pub screenshot: String,
}

impl Default for Shortcuts {
    fn default() -> Self {
        Self {
            toggle: "Win+Alt+N".into(),
            palette: "Ctrl+Alt+Space".into(),
            ocr: "Ctrl+Alt+T".into(),
            clipboard: "Ctrl+Alt+V".into(),
            focus: "Ctrl+Alt+F".into(),
            translate: "Ctrl+Alt+R".into(),
            screenshot: "Ctrl+Alt+S".into(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Privacy {
    pub net_lyrics: bool,
    pub net_currency: bool,
    pub net_weather: bool,
    pub net_geocode: bool,
    pub net_calendar: bool,
    pub net_ai: bool,
    pub net_updates: bool,
}

impl Default for Privacy {
    fn default() -> Self {
        Self {
            net_lyrics: true,
            net_currency: true,
            net_weather: true,
            net_geocode: true,
            net_calendar: true,
            net_ai: true,
            net_updates: true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Sound {
    pub timer_chime: bool,
    pub reminder_chime: bool,
    pub ui_sounds: bool,
}

impl Default for Sound {
    fn default() -> Self {
        Self {
            timer_chime: true,
            reminder_chime: true,
            ui_sounds: false,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Widgets {
    /// Dashboard tile order (tile ids).
    pub tiles: Vec<String>,
    /// Desktop widgets: (kind, monitor key, x, y) in DIPs.
    pub desktop: Vec<DesktopWidget>,
    pub countdown_title: String,
    /// Unix day number of the countdown date.
    pub countdown_day: i64,
    /// Home shows a calendar card under tiles 1–2 (tiles 3–4 show when this is off).
    pub home_calendar: bool,
}

impl Default for Widgets {
    fn default() -> Self {
        Self {
            tiles: vec![
                "music".into(),
                "battery".into(),
                "todo".into(),
                "weather".into(),
            ],
            desktop: Vec::new(),
            countdown_title: String::new(),
            countdown_day: 0,
            home_calendar: true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DesktopWidget {
    pub kind: String,
    pub x: i32,
    pub y: i32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct Sync {
    pub enabled: bool,
    pub folder: String,
}
