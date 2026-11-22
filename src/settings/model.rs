//! The versioned settings model. Every field has a default so older files load.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const SETTINGS_VERSION: u32 = 2;

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
    Glass,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Appearance {
    pub style: Style,
    pub material: Material,
    /// Glass tint opacity, 0.5–0.9.
    pub glass_intensity: f32,
    pub theme: String,
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
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            style: Style::Attached,
            material: Material::Solid,
            glass_intensity: 0.72,
            theme: "midnight".into(),
            adaptive: false,
            size: 0,
            minimal_idle: false,
            privacy_dots: true,
            idle_clock: true,
            mascot: true,
            mascot_delay_min: 3,
            floating_pos: BTreeMap::new(),
            high_contrast_follow: true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Speed {
    Calm,
    Default,
    Fast,
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
    pub speed: Speed,
    pub reduce_motion: bool,
    pub eq_source: EqSource,
    pub visualiser: bool,
    pub visualiser_ring: bool,
}

impl Default for Motion {
    fn default() -> Self {
        Self {
            speed: Speed::Default,
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
    pub water: bool,
    pub water_goal_ml: u32,
    pub water_glass_ml: u32,
    pub water_start_min: u32,
    pub water_end_min: u32,
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
            water: true,
            water_goal_ml: 2000,
            water_glass_ml: 250,
            water_start_min: 9 * 60,
            water_end_min: 21 * 60,
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
}

impl Default for Widgets {
    fn default() -> Self {
        Self {
            tiles: vec![
                "music".into(),
                "timer".into(),
                "water".into(),
                "battery".into(),
            ],
            desktop: Vec::new(),
            countdown_title: String::new(),
            countdown_day: 0,
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
