//! The Settings window: drawn with the same renderer and tokens as the notch.
//! Data-driven sections; every change applies live and persists.

use std::collections::HashMap;
use std::time::Instant;

use crate::app::state::{State, TextField};
use crate::app::{Action, InputId, Request, SliderKind};
use crate::motion::{Spring, ISLAND, ISLAND_H};
use crate::render::canvas::Zone;
use crate::render::icons::Icon;
use crate::render::text::{self, Align};
use crate::render::{shapes, Canvas, Gfx, Rect, Surface};
use crate::settings::*;
use crate::theme::tokens::*;

type Get<T> = fn(&State) -> T;
type Set<T> = fn(&mut State, T);

#[derive(Clone, Copy)]
pub enum Ctl {
    Toggle(Get<bool>, Set<bool>),
    Choice(fn(&State) -> Vec<String>, Get<usize>, Set<usize>),
    Slider {
        min: f32,
        max: f32,
        step: f32,
        get: Get<f32>,
        set: Set<f32>,
        fmt: fn(f32) -> String,
    },
    Text(Get<String>, Set<String>),
    /// Multi-line list, one entry per line.
    List(Get<Vec<String>>, Set<Vec<String>>),
    Secret(&'static str),
    Hotkey(Get<String>, Set<String>),
    Button(&'static str, fn(&mut State) -> String),
    Info(fn(&State) -> String),
    /// A connection: shows its live state and a Connect / Disconnect button.
    Account(fn(&State) -> AccountState, fn(&mut State) -> String),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AccountState {
    Disconnected,
    /// Sign-in started; waiting for the browser.
    Pending,
    Connected,
}

#[derive(Clone, Copy)]
pub struct Item {
    pub label: &'static str,
    pub hint: &'static str,
    pub ctl: Ctl,
}

pub struct Section {
    pub name: &'static str,
    pub icon: Icon,
    pub items: Vec<Item>,
}

fn it(label: &'static str, hint: &'static str, ctl: Ctl) -> Item {
    Item { label, hint, ctl }
}
fn opts(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}
fn ms(v: f32) -> String {
    format!("{v:.0} ms")
}
fn mins(v: f32) -> String {
    format!("{v:.0} min")
}
fn pct(v: f32) -> String {
    format!("{v:.0}%")
}
fn hm(v: f32) -> String {
    crate::util::fmt_hm(v as u32, true)
}
fn plain(v: f32) -> String {
    format!("{v:.0}")
}

macro_rules! toggle {
    ($($p:ident).+) => {
        Ctl::Toggle(|s: &State| s.settings.$($p).+, |s: &mut State, v: bool| s.settings.$($p).+ = v)
    };
}
macro_rules! slider {
    ($min:expr, $max:expr, $step:expr, $fmt:expr, $($p:ident).+) => {
        Ctl::Slider { min: $min, max: $max, step: $step, get: |s: &State| s.settings.$($p).+ as f32, set: |s: &mut State, v: f32| s.settings.$($p).+ = v as _, fmt: $fmt }
    };
}
macro_rules! textf {
    ($($p:ident).+) => {
        Ctl::Text(|s: &State| s.settings.$($p).+.clone(), |s: &mut State, v: String| s.settings.$($p).+ = v)
    };
}
macro_rules! listf {
    ($($p:ident).+) => {
        Ctl::List(|s: &State| s.settings.$($p).+.clone(), |s: &mut State, v: Vec<String>| s.settings.$($p).+ = v)
    };
}
macro_rules! hotkey {
    ($p:ident) => {
        Ctl::Hotkey(
            |s: &State| s.settings.shortcuts.$p.clone(),
            |s: &mut State, v: String| s.settings.shortcuts.$p = v,
        )
    };
}

pub fn sections() -> Vec<Section> {
    vec![
        Section {
            name: "General",
            icon: Icon::Settings,
            items: vec![
                it(
                    "Start with Windows",
                    "Launch Notchly when you sign in",
                    Ctl::Toggle(
                        |s| s.settings.general.start_with_windows,
                        |s, v| {
                            s.settings.general.start_with_windows = v;
                            crate::platform::set_autostart(v);
                        },
                    ),
                ),
                it(
                    "Display",
                    "Which monitor shows the notch",
                    Ctl::Choice(
                        |_| {
                            let mut v = vec!["Primary display".to_string()];
                            v.extend(crate::platform::monitors().iter().enumerate().map(
                                |(i, m)| {
                                    format!(
                                        "Display {} ({}×{})",
                                        i + 1,
                                        m.rect.right - m.rect.left,
                                        m.rect.bottom - m.rect.top
                                    )
                                },
                            ));
                            v
                        },
                        |s| s.settings.general.monitor as usize,
                        |s, v| s.settings.general.monitor = v as u32,
                    ),
                ),
                it(
                    "Open on",
                    "Hover opens after a short dwell; click needs a click",
                    Ctl::Choice(
                        |_| opts(&["Hover", "Click"]),
                        |s| (s.settings.general.open_mode == OpenMode::Click) as usize,
                        |s, v| {
                            s.settings.general.open_mode = if v == 1 {
                                OpenMode::Click
                            } else {
                                OpenMode::Hover
                            }
                        },
                    ),
                ),
                it(
                    "Hover delay",
                    "How long the cursor rests before opening",
                    slider!(50.0, 500.0, 10.0, ms, general.hover_delay_ms),
                ),
                it(
                    "Close delay",
                    "How long after leaving before it closes",
                    slider!(150.0, 1500.0, 50.0, ms, general.close_delay_ms),
                ),
                it(
                    "Language",
                    "Interface language",
                    Ctl::Choice(
                        |_| opts(&["English", "हिन्दी", "नेपाली"]),
                        |s| {
                            ["en", "hi", "ne"]
                                .iter()
                                .position(|l| *l == s.settings.general.language)
                                .unwrap_or(0)
                        },
                        |s, v| s.settings.general.language = ["en", "hi", "ne"][v].into(),
                    ),
                ),
                it(
                    "Clock",
                    "12- or 24-hour time",
                    Ctl::Choice(
                        |_| opts(&["Automatic", "12-hour", "24-hour"]),
                        |s| match s.settings.general.clock_24h {
                            None => 0,
                            Some(false) => 1,
                            Some(true) => 2,
                        },
                        |s, v| {
                            s.settings.general.clock_24h = match v {
                                1 => Some(false),
                                2 => Some(true),
                                _ => None,
                            }
                        },
                    ),
                ),
                it(
                    "Show tray icon",
                    "Settings stay reachable by right-clicking the notch",
                    toggle!(general.show_tray),
                ),
                it(
                    "Nepali date (Bikram Sambat)",
                    "Shown under the clock",
                    toggle!(general.nepali_date),
                ),
                it(
                    "Nepali numerals",
                    "०१२३ instead of 0123",
                    toggle!(general.nepali_numerals),
                ),
                it(
                    "World clocks",
                    "One per line: City=+5:45",
                    Ctl::List(
                        |s| {
                            s.settings
                                .general
                                .world_clocks
                                .iter()
                                .map(|w| {
                                    format!(
                                        "{}={}{}:{:02}",
                                        w.city,
                                        if w.offset_min >= 0 { "+" } else { "-" },
                                        w.offset_min.abs() / 60,
                                        w.offset_min.abs() % 60
                                    )
                                })
                                .collect()
                        },
                        |s, v| {
                            s.settings.general.world_clocks = v
                                .iter()
                                .filter_map(|l| {
                                    let (c, o) = l.split_once('=')?;
                                    let o = o.trim();
                                    let neg = o.starts_with('-');
                                    let (h, m) = o
                                        .trim_start_matches(['+', '-'])
                                        .split_once(':')
                                        .unwrap_or((o.trim_start_matches(['+', '-']), "0"));
                                    let mins =
                                        h.parse::<i32>().ok()? * 60 + m.parse::<i32>().unwrap_or(0);
                                    Some(WorldClock {
                                        city: c.trim().into(),
                                        offset_min: if neg { -mins } else { mins },
                                    })
                                })
                                .take(4)
                                .collect()
                        },
                    ),
                ),
            ],
        },
        Section {
            name: "Appearance",
            icon: Icon::Palette,
            items: vec![
                it(
                    "Style",
                    "Attached melts into the bezel; Floating detaches below it",
                    Ctl::Choice(
                        |_| opts(&["Attached", "Floating"]),
                        |s| (s.settings.appearance.style == Style::Floating) as usize,
                        |s, v| {
                            s.settings.appearance.style = if v == 1 {
                                Style::Floating
                            } else {
                                Style::Attached
                            }
                        },
                    ),
                ),
                it(
                    "Material",
                    "Glass blurs what's behind (Windows 11)",
                    Ctl::Choice(
                        |_| opts(&["Solid", "Glass"]),
                        |s| (s.settings.appearance.material == Material::Glass) as usize,
                        |s, v| {
                            s.settings.appearance.material = if v == 1 {
                                Material::Glass
                            } else {
                                Material::Solid
                            }
                        },
                    ),
                ),
                it(
                    "Glass intensity",
                    "Tint opacity",
                    Ctl::Slider {
                        min: 50.0,
                        max: 90.0,
                        step: 1.0,
                        get: |s| s.settings.appearance.glass_intensity * 100.0,
                        set: |s, v| s.settings.appearance.glass_intensity = v / 100.0,
                        fmt: pct,
                    },
                ),
                it(
                    "Theme",
                    "Accent colours and surfaces",
                    Ctl::Choice(
                        |s| {
                            crate::theme::PRESETS
                                .iter()
                                .map(|p| p.name.to_string())
                                .chain(s.packs.iter().map(|t| t.name.clone()))
                                .collect()
                        },
                        |s| {
                            crate::theme::PRESETS
                                .iter()
                                .position(|p| p.id == s.settings.appearance.theme)
                                .or_else(|| {
                                    s.packs
                                        .iter()
                                        .position(|t| t.id == s.settings.appearance.theme)
                                        .map(|i| i + crate::theme::PRESETS.len())
                                })
                                .unwrap_or(0)
                        },
                        |s, v| {
                            let id = crate::theme::PRESETS
                                .get(v)
                                .map(|p| p.id.to_string())
                                .or_else(|| {
                                    s.packs
                                        .get(v - crate::theme::PRESETS.len())
                                        .map(|t| t.id.clone())
                                })
                                .unwrap_or_else(|| "midnight".into());
                            s.settings.appearance.theme = id;
                        },
                    ),
                ),
                it(
                    "Adaptive colour",
                    "Recolour the glow from the album art",
                    toggle!(appearance.adaptive),
                ),
                it(
                    "Notch size",
                    "Scales everything",
                    Ctl::Choice(
                        |_| opts(&["Auto", "100%", "115%", "130%", "150%"]),
                        |s| {
                            [0, 100, 115, 130, 150]
                                .iter()
                                .position(|v| *v == s.settings.appearance.size)
                                .unwrap_or(0)
                        },
                        |s, v| s.settings.appearance.size = [0, 100, 115, 130, 150][v],
                    ),
                ),
                it(
                    "Minimal notch when idle",
                    "A thin line until something happens",
                    toggle!(appearance.minimal_idle),
                ),
                it(
                    "Privacy dots",
                    "Orange mic, green camera, blue location",
                    toggle!(appearance.privacy_dots),
                ),
                it(
                    "Mascot",
                    "Blink appears after a while idle",
                    toggle!(appearance.mascot),
                ),
                it(
                    "Mascot idle delay",
                    "",
                    slider!(1.0, 30.0, 1.0, mins, appearance.mascot_delay_min),
                ),
                it(
                    "Import theme pack…",
                    "A .json theme pack",
                    Ctl::Button("Import", |s| {
                        match crate::platform_dialogs::open_file("Theme pack", "*.json") {
                            Some(p) => {
                                let dest = crate::theme::themes_dir()
                                    .join(p.file_name().unwrap_or_default());
                                match std::fs::copy(&p, &dest) {
                                    Ok(_) => {
                                        s.packs = crate::theme::load_packs();
                                        "Theme pack imported".into()
                                    }
                                    Err(e) => e.to_string(),
                                }
                            }
                            None => String::new(),
                        }
                    }),
                ),
                it(
                    "Themes folder",
                    "",
                    Ctl::Button("Open", |_| {
                        crate::platform::shell_open(&crate::theme::themes_dir().to_string_lossy());
                        String::new()
                    }),
                ),
            ],
        },
        Section {
            name: "Dashboard & widgets",
            icon: Icon::Widgets,
            items: vec![
                it(
                    "Clock in the idle notch",
                    "Time plus headphones, Bluetooth and charging status",
                    toggle!(appearance.idle_clock),
                ),
                it(
                    "Calendar on Home",
                    "A week strip and your next event under tiles 1–2",
                    toggle!(widgets.home_calendar),
                ),
                it(
                    "Tile 1",
                    "Top-left dashboard tile",
                    Ctl::Choice(tile_names, |s| tile_get(s, 0), |s, v| tile_set(s, 0, v)),
                ),
                it(
                    "Tile 2",
                    "Top-right dashboard tile",
                    Ctl::Choice(tile_names, |s| tile_get(s, 1), |s, v| tile_set(s, 1, v)),
                ),
                it(
                    "Tile 3",
                    "Shown when the Home calendar is off",
                    Ctl::Choice(tile_names, |s| tile_get(s, 2), |s, v| tile_set(s, 2, v)),
                ),
                it(
                    "Tile 4",
                    "Shown when the Home calendar is off",
                    Ctl::Choice(tile_names, |s| tile_get(s, 3), |s, v| tile_set(s, 3, v)),
                ),
                it(
                    "Countdown title",
                    "For the Countdown tile and widget",
                    textf!(widgets.countdown_title),
                ),
                it(
                    "Countdown date",
                    "YYYY-MM-DD",
                    Ctl::Text(
                        |s| {
                            let d = s.settings.widgets.countdown_day;
                            if d == 0 {
                                String::new()
                            } else {
                                let (y, m, dd) = crate::util::civil_from_days(d);
                                format!("{y:04}-{m:02}-{dd:02}")
                            }
                        },
                        |s, v| {
                            let parts: Vec<i64> =
                                v.trim().split('-').filter_map(|x| x.parse().ok()).collect();
                            s.settings.widgets.countdown_day = match parts.as_slice() {
                                [y, m, d] if (1..=12).contains(m) && (1..=31).contains(d) => {
                                    crate::util::days_from_civil(*y as i32, *m as u32, *d as u32)
                                }
                                _ => 0,
                            };
                        },
                    ),
                ),
                it(
                    "Desktop: clock",
                    "Floating widget on the desktop (drag to move)",
                    Ctl::Toggle(|s| widget_on(s, 0), |s, v| widget_set(s, 0, v)),
                ),
                it(
                    "Desktop: world clocks",
                    "",
                    Ctl::Toggle(|s| widget_on(s, 1), |s, v| widget_set(s, 1, v)),
                ),
                it(
                    "Desktop: weather",
                    "",
                    Ctl::Toggle(|s| widget_on(s, 2), |s, v| widget_set(s, 2, v)),
                ),
                it(
                    "Desktop: calendar",
                    "",
                    Ctl::Toggle(|s| widget_on(s, 3), |s, v| widget_set(s, 3, v)),
                ),
                it(
                    "Desktop: system monitor",
                    "",
                    Ctl::Toggle(|s| widget_on(s, 4), |s, v| widget_set(s, 4, v)),
                ),
                it(
                    "Desktop: battery",
                    "",
                    Ctl::Toggle(|s| widget_on(s, 5), |s, v| widget_set(s, 5, v)),
                ),
                it(
                    "Desktop: now playing",
                    "",
                    Ctl::Toggle(|s| widget_on(s, 6), |s, v| widget_set(s, 6, v)),
                ),
                it(
                    "Desktop: countdown",
                    "",
                    Ctl::Toggle(|s| widget_on(s, 7), |s, v| widget_set(s, 7, v)),
                ),
                it(
                    "Desktop: sticky note",
                    "",
                    Ctl::Toggle(|s| widget_on(s, 8), |s, v| widget_set(s, 8, v)),
                ),
                it(
                    "Desktop: focus stats",
                    "",
                    Ctl::Toggle(|s| widget_on(s, 9), |s, v| widget_set(s, 9, v)),
                ),
            ],
        },
        Section {
            name: "Motion",
            icon: Icon::Sparkles,
            items: vec![
                it(
                    "Animation speed",
                    "",
                    Ctl::Choice(
                        |_| opts(&["Calm", "Default", "Fast"]),
                        |s| s.settings.motion.speed as usize,
                        |s, v| {
                            s.settings.motion.speed = [Speed::Calm, Speed::Default, Speed::Fast][v]
                        },
                    ),
                ),
                it(
                    "Reduce motion",
                    "No overshoot, no slides; also follows Windows",
                    toggle!(motion.reduce_motion),
                ),
                it(
                    "Equaliser source",
                    "Audio follows the real level",
                    Ctl::Choice(
                        |_| opts(&["Audio", "Simulated"]),
                        |s| s.settings.motion.eq_source as usize,
                        |s, v| {
                            s.settings.motion.eq_source = if v == 1 {
                                EqSource::Simulated
                            } else {
                                EqSource::Audio
                            }
                        },
                    ),
                ),
                it(
                    "Audio visualiser",
                    "Bars from the actual sound (only while visible)",
                    toggle!(motion.visualiser),
                ),
                it(
                    "Visualiser ring",
                    "A ring around the album art",
                    toggle!(motion.visualiser_ring),
                ),
            ],
        },
        Section {
            name: "Live activities",
            icon: Icon::Widgets,
            items: vec![
                it(
                    "Dual island",
                    "Show a second activity as a bubble",
                    toggle!(live.dual_island),
                ),
                it(
                    "Live Activities API",
                    "Lets apps and scripts post to the notch",
                    toggle!(live.api_enabled),
                ),
                it(
                    "Allowed apps",
                    "",
                    Ctl::Info(|s| {
                        if s.settings.live.api_apps.is_empty() {
                            "No apps yet — the first message from an app asks you".into()
                        } else {
                            s.settings
                                .live
                                .api_apps
                                .iter()
                                .map(|(k, v)| {
                                    format!("{k}: {}", if *v { "allowed" } else { "denied" })
                                })
                                .collect::<Vec<_>>()
                                .join(" · ")
                        }
                    }),
                ),
                it(
                    "Revoke all apps",
                    "",
                    Ctl::Button("Revoke", |s| {
                        s.settings.live.api_apps.clear();
                        "All apps will be asked again".into()
                    }),
                ),
                it(
                    "Music priority",
                    "Higher wins the main pill",
                    Ctl::Slider {
                        min: 10.0,
                        max: 95.0,
                        step: 5.0,
                        get: |s| s.priority_of(crate::app::activity::ActKind::Music) as f32,
                        set: |s, v| {
                            s.settings.live.priority.insert("music".into(), v as i32);
                        },
                        fmt: plain,
                    },
                ),
                it(
                    "Focus timer priority",
                    "",
                    Ctl::Slider {
                        min: 10.0,
                        max: 95.0,
                        step: 5.0,
                        get: |s| s.priority_of(crate::app::activity::ActKind::Focus) as f32,
                        set: |s, v| {
                            s.settings.live.priority.insert("focus".into(), v as i32);
                        },
                        fmt: plain,
                    },
                ),
                it(
                    "Downloads priority",
                    "",
                    Ctl::Slider {
                        min: 10.0,
                        max: 95.0,
                        step: 5.0,
                        get: |s| s.priority_of(crate::app::activity::ActKind::Transfer) as f32,
                        set: |s, v| {
                            s.settings.live.priority.insert("transfer".into(), v as i32);
                        },
                        fmt: plain,
                    },
                ),
            ],
        },
        Section {
            name: "Pop-ups",
            icon: Icon::Bell,
            items: vec![
                it("Volume", "", toggle!(popups.volume)),
                it("Brightness", "", toggle!(popups.brightness)),
                it("Caps Lock / Num Lock", "", toggle!(popups.caps)),
                it("Input language", "", toggle!(popups.language)),
                it("Battery", "", toggle!(popups.battery)),
                it("Wi-Fi / network", "", toggle!(popups.network)),
                it("Audio device", "", toggle!(popups.audio_device)),
                it("Bluetooth", "", toggle!(popups.bluetooth)),
                it("Microphone mute", "", toggle!(popups.mic)),
                it(
                    "Pop-up duration",
                    "",
                    Ctl::Slider {
                        min: 800.0,
                        max: 4000.0,
                        step: 100.0,
                        get: |s| s.settings.popups.duration_ms as f32,
                        set: |s, v| s.settings.popups.duration_ms = v as u32,
                        fmt: |v| format!("{:.1} s", v / 1000.0),
                    },
                ),
                it(
                    "Low battery at",
                    "",
                    slider!(5.0, 50.0, 1.0, pct, popups.low_battery),
                ),
                it(
                    "Notifications",
                    "Mirror Windows notifications",
                    toggle!(popups.notifications),
                ),
                it(
                    "Muted apps",
                    "One app name per line",
                    listf!(popups.muted_apps),
                ),
                it(
                    "Keep notification history",
                    "Otherwise cleared on exit",
                    toggle!(popups.keep_history),
                ),
                it(
                    "Hide the Windows volume flyout",
                    "Opens the Windows setting (Notchly never hides system UI itself)",
                    Ctl::Button("Open", |_| {
                        crate::platform::shell_open("ms-settings:easeofaccess-display");
                        String::new()
                    }),
                ),
            ],
        },
        Section {
            name: "Music",
            icon: Icon::Music,
            items: vec![
                it("Music", "Show what's playing", toggle!(music.enabled)),
                it("Synced lyrics", "From LRCLIB", toggle!(music.lyrics)),
                it(
                    "Lyrics in live view",
                    "The pill shows the current line",
                    toggle!(music.lyrics_live),
                ),
                it(
                    "Lyrics translation",
                    "Translate the whole song once",
                    toggle!(music.lyrics_translate),
                ),
                it(
                    "Romanisation",
                    "Readings for Devanagari, Japanese, Korean…",
                    toggle!(music.lyrics_romanise),
                ),
                it(
                    "Per-app volume",
                    "A volume slider for the player",
                    toggle!(music.app_volume),
                ),
                it(
                    "Preferred player",
                    "Favoured when several apps play (e.g. Spotify)",
                    textf!(music.preferred_player),
                ),
            ],
        },
        Section {
            name: "Productivity",
            icon: Icon::CheckCircle,
            items: vec![
                it(
                    "Focus length",
                    "",
                    slider!(5.0, 90.0, 5.0, mins, productivity.focus_min),
                ),
                it(
                    "Short break",
                    "",
                    slider!(1.0, 30.0, 1.0, mins, productivity.short_break_min),
                ),
                it(
                    "Long break",
                    "",
                    slider!(5.0, 60.0, 5.0, mins, productivity.long_break_min),
                ),
                it(
                    "Long break every",
                    "sessions",
                    slider!(2.0, 8.0, 1.0, plain, productivity.long_every),
                ),
                it("Water reminder", "", toggle!(productivity.water)),
                it(
                    "Daily water goal",
                    "",
                    Ctl::Slider {
                        min: 500.0,
                        max: 5000.0,
                        step: 250.0,
                        get: |s| s.settings.productivity.water_goal_ml as f32,
                        set: |s, v| s.settings.productivity.water_goal_ml = v as u32,
                        fmt: |v| format!("{v:.0} ml"),
                    },
                ),
                it(
                    "Glass size",
                    "",
                    Ctl::Slider {
                        min: 100.0,
                        max: 750.0,
                        step: 50.0,
                        get: |s| s.settings.productivity.water_glass_ml as f32,
                        set: |s, v| s.settings.productivity.water_glass_ml = v as u32,
                        fmt: |v| format!("{v:.0} ml"),
                    },
                ),
                it(
                    "Reminders from",
                    "",
                    slider!(0.0, 1410.0, 30.0, hm, productivity.water_start_min),
                ),
                it(
                    "Reminders until",
                    "",
                    slider!(30.0, 1440.0, 30.0, hm, productivity.water_end_min),
                ),
                it("To-do reminders", "", toggle!(productivity.todo_reminders)),
                it(
                    "Shelf auto-clear",
                    "0 = never",
                    Ctl::Slider {
                        min: 0.0,
                        max: 30.0,
                        step: 1.0,
                        get: |s| s.settings.productivity.shelf_autoclear_days as f32,
                        set: |s, v| s.settings.productivity.shelf_autoclear_days = v as u32,
                        fmt: |v| {
                            if v == 0.0 {
                                "Never".into()
                            } else {
                                format!("{v:.0} days")
                            }
                        },
                    },
                ),
                it("Clipboard history", "", toggle!(productivity.clipboard)),
                it(
                    "Items kept",
                    "",
                    slider!(10.0, 200.0, 10.0, plain, productivity.clipboard_max),
                ),
                it(
                    "Excluded apps",
                    "Never record copies from these (one per line)",
                    listf!(productivity.clipboard_excluded),
                ),
                it(
                    "Paste on select",
                    "Clicking an item pastes it into the previous app",
                    toggle!(productivity.paste_on_select),
                ),
                it("Prayer times", "", toggle!(productivity.prayer)),
                it(
                    "Calculation method",
                    "",
                    Ctl::Choice(
                        |_| {
                            crate::features::prayer::METHODS
                                .iter()
                                .map(|m| m.name.to_string())
                                .collect()
                        },
                        |s| {
                            crate::features::prayer::METHODS
                                .iter()
                                .position(|m| m.id == s.settings.productivity.prayer_method)
                                .unwrap_or(0)
                        },
                        |s, v| {
                            s.settings.productivity.prayer_method =
                                crate::features::prayer::METHODS[v].id.into();
                            s.f.prayer_cache = None;
                        },
                    ),
                ),
                it(
                    "Hanafi Asr",
                    "Later Asr (shadow ×2)",
                    Ctl::Toggle(
                        |s| s.settings.productivity.asr_hanafi,
                        |s, v| {
                            s.settings.productivity.asr_hanafi = v;
                            s.f.prayer_cache = None;
                        },
                    ),
                ),
                it(
                    "Remind before prayer",
                    "",
                    slider!(0.0, 30.0, 1.0, mins, productivity.prayer_reminder_min),
                ),
                it(
                    "Pause media at prayer time",
                    "",
                    toggle!(productivity.prayer_pause_media),
                ),
                it(
                    "OCR language",
                    "Installed Windows OCR languages",
                    Ctl::Choice(
                        |_| {
                            std::iter::once("Automatic".to_string())
                                .chain(
                                    crate::services::ocr::available_languages()
                                        .into_iter()
                                        .map(|l| l.1),
                                )
                                .collect()
                        },
                        |s| {
                            crate::services::ocr::available_languages()
                                .iter()
                                .position(|l| l.0 == s.settings.productivity.ocr_language)
                                .map(|i| i + 1)
                                .unwrap_or(0)
                        },
                        |s, v| {
                            s.settings.productivity.ocr_language = if v == 0 {
                                String::new()
                            } else {
                                crate::services::ocr::available_languages()
                                    .get(v - 1)
                                    .map(|l| l.0.clone())
                                    .unwrap_or_default()
                            }
                        },
                    ),
                ),
                it("Quick notes", "", toggle!(productivity.notes)),
                it(
                    "Download tracker",
                    "Follows downloads in your Downloads folder",
                    toggle!(productivity.downloads),
                ),
                it(
                    "Extra download folders",
                    "One folder per line",
                    listf!(productivity.download_folders),
                ),
                it("System monitor", "", toggle!(productivity.sysmon)),
                it(
                    "High CPU alert",
                    "When CPU stays above 90% for 30 s",
                    toggle!(productivity.sysmon_alert),
                ),
            ],
        },
        Section {
            name: "Ask & AI",
            icon: Icon::Sparkles,
            items: vec![
                it(
                    "Provider",
                    "Auto tries Ollama first, then your keys",
                    Ctl::Choice(
                        |_| {
                            opts(&["Auto", "Ollama (local)", "Gemini", "OpenAI", "Anthropic"])
                        },
                        |s| match s.settings.ai.provider {
                            Provider::Auto | Provider::WindowsAi => 0,
                            Provider::Ollama => 1,
                            Provider::Gemini => 2,
                            Provider::OpenAi => 3,
                            Provider::Anthropic => 4,
                        },
                        |s, v| {
                            s.settings.ai.provider = [
                                Provider::Auto,
                                Provider::Ollama,
                                Provider::Gemini,
                                Provider::OpenAi,
                                Provider::Anthropic,
                            ][v.min(4)]
                        },
                    ),
                ),
                it("Ollama address", "", textf!(ai.ollama_url)),
                it(
                    "Ollama model",
                    "Empty = first installed model",
                    textf!(ai.ollama_model),
                ),
                it("Gemini model", "", textf!(ai.gemini_model)),
                it("OpenAI model", "", textf!(ai.openai_model)),
                it("Anthropic model", "", textf!(ai.anthropic_model)),
                it(
                    "Gemini API key",
                    "Stored encrypted with Windows DPAPI",
                    Ctl::Secret("gemini"),
                ),
                it(
                    "OpenAI API key",
                    "Stored encrypted with Windows DPAPI",
                    Ctl::Secret("openai"),
                ),
                it(
                    "Anthropic API key",
                    "Stored encrypted with Windows DPAPI",
                    Ctl::Secret("anthropic"),
                ),
                it(
                    "Translate to",
                    "Language name, e.g. English, Nepali",
                    textf!(ai.translate_target),
                ),
                it(
                    "Custom commands",
                    "One per line: Name=command or URL",
                    Ctl::List(
                        |s| {
                            s.settings
                                .ai
                                .commands
                                .iter()
                                .map(|c| format!("{}={}", c.name, c.command))
                                .collect()
                        },
                        |s, v| {
                            s.settings.ai.commands = v
                                .iter()
                                .filter_map(|l| {
                                    l.split_once('=').map(|(n, c)| UserCommand {
                                        name: n.trim().into(),
                                        command: c.trim().into(),
                                    })
                                })
                                .collect()
                        },
                    ),
                ),
            ],
        },
        Section {
            name: "Calendar & weather",
            icon: Icon::CloudSun,
            items: vec![
                it("Weather", "", toggle!(weather.enabled)),
                it(
                    "Units",
                    "",
                    Ctl::Choice(
                        |_| opts(&["Metric", "Imperial"]),
                        |s| s.settings.weather.units as usize,
                        |s, v| {
                            s.settings.weather.units = if v == 1 {
                                Units::Imperial
                            } else {
                                Units::Metric
                            }
                        },
                    ),
                ),
                it(
                    "City",
                    "Used for weather and prayer times",
                    textf!(weather.city),
                ),
                it(
                    "Find city",
                    "Looks the city up (Open-Meteo)",
                    Ctl::Button("Search", |s| {
                        let city = s.settings.weather.city.clone();
                        if city.trim().is_empty() {
                            return "Type a city first".into();
                        }
                        if !s.settings.privacy.net_geocode {
                            return "City search is off in Privacy".into();
                        }
                        if let Some(bus) = s.svc.bus.clone() {
                            crate::services::weather::geocode(bus, city);
                        }
                        "Searching…".into()
                    }),
                ),
                it(
                    "Location",
                    "",
                    Ctl::Info(|s| {
                        let w = &s.settings.weather;
                        if w.lat == 0.0 && w.lon == 0.0 {
                            "Not set".into()
                        } else {
                            format!("{:.2}, {:.2}", w.lat, w.lon)
                        }
                    }),
                ),
                it(
                    "Use Windows location",
                    "Asks Windows for permission",
                    Ctl::Button("Locate", |s| {
                        if let Some(bus) = s.svc.bus.clone() {
                            crate::app_location::locate(bus);
                        }
                        "Locating…".into()
                    }),
                ),
                it(
                    "Keep location updated",
                    "Refresh from Windows each time Notchly starts",
                    toggle!(weather.use_windows_location),
                ),
                it("Calendar", "", toggle!(calendar.enabled)),
                it(
                    "ICS URLs",
                    "iCloud, school timetables… one per line",
                    listf!(calendar.ics_urls),
                ),
                it(
                    "Google client ID",
                    if crate::build_info::GOOGLE_CLIENT_ID.is_some() {
                        "Optional: Notchly's own sign-in is built in"
                    } else {
                        "OAuth desktop client ID (see the user guide)"
                    },
                    textf!(calendar.google_client_id),
                ),
                it(
                    "Google client secret",
                    "Only with your own client ID",
                    Ctl::Secret("google_oauth_secret"),
                ),
                it(
                    "Google Calendar",
                    "Read-only access to your primary calendar",
                    Ctl::Account(|_| account_state(true), |s| oauth_toggle(s, true)),
                ),
                it(
                    "Microsoft client ID",
                    if crate::build_info::MS_CLIENT_ID.is_some() {
                        "Optional: Notchly's own sign-in is built in"
                    } else {
                        "Azure app (public client) ID (see the user guide)"
                    },
                    textf!(calendar.microsoft_client_id),
                ),
                it(
                    "Outlook / Microsoft 365",
                    "Read-only access to your calendar",
                    Ctl::Account(|_| account_state(false), |s| oauth_toggle(s, false)),
                ),
                it(
                    "Remind before events",
                    "",
                    slider!(0.0, 60.0, 5.0, mins, calendar.reminder_min),
                ),
            ],
        },
        Section {
            name: "Quiet hours",
            icon: Icon::MoonStar,
            items: vec![
                it(
                    "Focus",
                    "Only calls, timers and critical battery show",
                    toggle!(quiet.focus),
                ),
                it(
                    "Allow incoming calls",
                    "Call notifications still show during quiet hours",
                    toggle!(quiet.allow_calls),
                ),
                it(
                    "Allow timers",
                    "Timer and focus alerts still show",
                    toggle!(quiet.allow_timers),
                ),
                it("Scheduled quiet hours", "", toggle!(quiet.schedule)),
                it("From", "", slider!(0.0, 1410.0, 30.0, hm, quiet.start_min)),
                it("Until", "", slider!(0.0, 1410.0, 30.0, hm, quiet.end_min)),
                it(
                    "Hide in fullscreen",
                    "Games, video and presentations",
                    toggle!(quiet.auto_fullscreen),
                ),
                it("Quiet during meetings", "", toggle!(quiet.auto_meetings)),
                it(
                    "Hide content while sharing",
                    "",
                    toggle!(quiet.hide_when_sharing),
                ),
                it(
                    "Never hide for",
                    "Apps (exe) that may stay fullscreen with the notch",
                    listf!(quiet.fullscreen_allow),
                ),
                it(
                    "Always hide for",
                    "Apps (exe) that always hide the notch",
                    listf!(quiet.fullscreen_deny),
                ),
            ],
        },
        Section {
            name: "Shortcuts",
            icon: Icon::Keyboard,
            items: vec![
                it("Open / close Notchly", "", hotkey!(toggle)),
                it("Ask / command palette", "", hotkey!(palette)),
                it("Smart OCR", "", hotkey!(ocr)),
                it("Clipboard history", "", hotkey!(clipboard)),
                it("Start / pause focus", "", hotkey!(focus)),
                it("Translate selection", "", hotkey!(translate)),
                it("Screenshot", "", hotkey!(screenshot)),
            ],
        },
        Section {
            name: "Privacy",
            icon: Icon::Shield,
            items: vec![
                it("Lyrics", "lrclib.net", toggle!(privacy.net_lyrics)),
                it(
                    "Currency rates",
                    "api.frankfurter.app (ECB)",
                    toggle!(privacy.net_currency),
                ),
                it("Weather", "api.met.no", toggle!(privacy.net_weather)),
                it(
                    "City search",
                    "geocoding-api.open-meteo.com",
                    toggle!(privacy.net_geocode),
                ),
                it(
                    "Calendar",
                    "Google, Microsoft Graph, your ICS URLs",
                    toggle!(privacy.net_calendar),
                ),
                it(
                    "Cloud AI",
                    "Only the provider you choose (Ollama stays on this PC)",
                    toggle!(privacy.net_ai),
                ),
                it(
                    "Update check",
                    "Daily, static feed",
                    toggle!(privacy.net_updates),
                ),
                it(
                    "Clear caches",
                    "Lyrics, art, weather, rates",
                    Ctl::Button("Clear", |_| {
                        let _ = std::fs::remove_dir_all(crate::util::data_dir().join("cache"));
                        "Caches cleared".into()
                    }),
                ),
                it(
                    "Clear history",
                    "Clipboard, notifications, privacy log",
                    Ctl::Button("Clear", |s| {
                        s.f.clip.items.clear();
                        s.f.clip_dirty = true;
                        s.f.notifs.items.clear();
                        s.f.state.privacy_log.clear();
                        s.f.state_dirty = true;
                        s.f.save(true);
                        "History cleared".into()
                    }),
                ),
                it(
                    "Export settings",
                    "",
                    Ctl::Button("Export", |s| {
                        match crate::platform_dialogs::save_file("notchly-settings.toml") {
                            Some(p) => match store::export_to(&p, &s.settings) {
                                Ok(()) => "Exported".into(),
                                Err(e) => e.to_string(),
                            },
                            None => String::new(),
                        }
                    }),
                ),
                it(
                    "Import settings",
                    "",
                    Ctl::Button("Import", |s| {
                        match crate::platform_dialogs::open_file("Settings", "*.toml") {
                            Some(p) => match store::import_from(&p) {
                                Ok(n) => {
                                    s.settings = n;
                                    "Imported".into()
                                }
                                Err(e) => e,
                            },
                            None => String::new(),
                        }
                    }),
                ),
                it(
                    "Crash logs",
                    "Stored locally only",
                    Ctl::Button("Open folder", |_| {
                        crate::platform::shell_open(
                            &crate::util::sub_dir("logs").to_string_lossy(),
                        );
                        String::new()
                    }),
                ),
                it(
                    "Settings sync folder",
                    "Mirror settings, themes, to-dos and notes (e.g. OneDrive)",
                    textf!(sync.folder),
                ),
                it("Sync", "", toggle!(sync.enabled)),
            ],
        },
        Section {
            name: "About",
            icon: Icon::Sparkles,
            items: vec![
                it(
                    "Version",
                    "",
                    Ctl::Info(|_| format!("Notchly {}", env!("CARGO_PKG_VERSION"))),
                ),
                it(
                    "Updates",
                    "",
                    Ctl::Button("Check now", |s| {
                        if let Some(bus) = s.svc.bus.clone() {
                            crate::updates::check(bus, true);
                        }
                        "Checking…".into()
                    }),
                ),
                it(
                    "Credits",
                    "",
                    Ctl::Info(|_| {
                        "Lyrics: LRCLIB · Weather data: MET Norway · Geocoding: Open-Meteo · Rates: ECB via Frankfurter · Prayer algorithm after the standard astronomical method".into()
                    }),
                ),
                it(
                    "Your data",
                    "Settings, shelf, notes and logs live here",
                    Ctl::Button("Open folder", |_| {
                        crate::platform::shell_open(&crate::util::data_dir().to_string_lossy());
                        String::new()
                    }),
                ),
            ]
            .into_iter()
            .chain(crate::build_info::WEBSITE.map(|_| {
                it(
                    "Website",
                    "",
                    Ctl::Button("Open", |_| {
                        if let Some(w) = crate::build_info::WEBSITE {
                            crate::platform::shell_open(w);
                        }
                        String::new()
                    }),
                )
            }))
            .chain(crate::build_info::SUPPORT_URL.map(|_| {
                it(
                    "Report a problem",
                    "",
                    Ctl::Button("Open", |_| {
                        if let Some(w) = crate::build_info::SUPPORT_URL {
                            crate::platform::shell_open(w);
                        }
                        String::new()
                    }),
                )
            }))
            .collect(),
        },
    ]
}

fn account_state(google: bool) -> AccountState {
    use crate::services::calendar::{is_connected, signing_in, GOOGLE, MICROSOFT};
    let p = if google { &GOOGLE } else { &MICROSOFT };
    if is_connected(p) {
        AccountState::Connected
    } else if signing_in(p) {
        AccountState::Pending
    } else {
        AccountState::Disconnected
    }
}

fn oauth_toggle(s: &mut State, google: bool) -> String {
    use crate::services::calendar::{
        has_builtin, is_connected, set_signing_in, sign_in, sign_out, signing_in, GOOGLE, MICROSOFT,
    };
    let p = if google { &GOOGLE } else { &MICROSOFT };
    if is_connected(p) {
        sign_out(p);
        // drop that account's events right away; the rest refresh
        let source = if google { "Google" } else { "Outlook" };
        s.f.calendar.events.retain(|e| e.calendar != source);
        s.f.calendar.fetched = None;
        return "Disconnected".into();
    }
    if signing_in(p) {
        return "Finish signing in in your browser".into();
    }
    let id = if google {
        s.settings.calendar.google_client_id.clone()
    } else {
        s.settings.calendar.microsoft_client_id.clone()
    };
    if id.trim().is_empty() && !has_builtin(p) {
        return "Add a client ID above first".into();
    }
    let bus = s.svc.bus.clone();
    let pid = p.id;
    set_signing_in(p, true);
    crate::services::spawn("oauth", move || {
        crate::services::com_mta();
        let p = if pid == "google" { &GOOGLE } else { &MICROSOFT };
        let r = sign_in(p, &id);
        set_signing_in(p, false);
        if let Some(b) = bus {
            let name = if pid == "google" {
                "Google Calendar"
            } else {
                "Outlook calendar"
            };
            b.send(crate::services::Event::OAuthDone(name.into(), r));
        }
    });
    "Finish signing in in your browser".into()
}

// ---------------------------------------------------------------- window state

pub struct SettingsWin {
    pub hwnd: windows::Win32::Foundation::HWND,
    pub surface: Surface,
    pub sections: Vec<Section>,
    pub section: usize,
    pub scroll: Spring,
    pub zones: Vec<Zone>,
    pub cursor: Option<(f32, f32)>,
    pub pressed: Option<Action>,
    pub slider: Option<(u16, Rect)>,
    pub focus: Option<u16>,
    pub fields: HashMap<u16, TextField>,
    pub recording: Option<u16>,
    pub message: Option<(String, Instant)>,
    pub dirty: bool,
    pub size: (f32, f32),
    pub dpi: f32,
    preview_w: Spring,
    preview_h: Spring,
    preview_phase: usize,
    preview_at: Instant,
    pub keyboard_focus: Option<Action>,
}

const SEARCH_ID: u16 = 0xFFFE;

impl SettingsWin {
    pub fn new(hwnd: windows::Win32::Foundation::HWND, w: u32, h: u32, dpi: f32) -> Self {
        Self {
            hwnd,
            surface: Surface::new(hwnd, w, h, dpi),
            sections: sections(),
            section: 0,
            scroll: Spring::new(0.0, crate::motion::SNAPPY),
            zones: Vec::new(),
            cursor: None,
            pressed: None,
            slider: None,
            focus: None,
            fields: HashMap::new(),
            recording: None,
            message: None,
            dirty: true,
            size: (w as f32 * 96.0 / dpi, h as f32 * 96.0 / dpi),
            dpi,
            preview_w: Spring::new(SHAPE_IDLE.w, ISLAND),
            preview_h: Spring::new(SHAPE_IDLE.h, ISLAND_H),
            preview_phase: 0,
            preview_at: Instant::now(),
            keyboard_focus: None,
        }
    }

    fn id(sec: usize, item: usize) -> u16 {
        (sec * 100 + item) as u16
    }

    fn item(&self, id: u16) -> Option<&Item> {
        self.sections
            .get(id as usize / 100)?
            .items
            .get(id as usize % 100)
    }

    fn query(&self) -> String {
        self.fields
            .get(&SEARCH_ID)
            .map(|f| f.text.to_lowercase())
            .unwrap_or_default()
    }

    /// Items visible in the content pane (search spans all sections).
    fn visible(&self) -> Vec<u16> {
        let q = self.query();
        if q.is_empty() {
            (0..self.sections[self.section].items.len())
                .map(|i| Self::id(self.section, i))
                .collect()
        } else {
            let mut v = Vec::new();
            for (si, s) in self.sections.iter().enumerate() {
                for (ii, it) in s.items.iter().enumerate() {
                    if it.label.to_lowercase().contains(&q)
                        || it.hint.to_lowercase().contains(&q)
                        || s.name.to_lowercase().contains(&q)
                    {
                        v.push(Self::id(si, ii));
                    }
                }
            }
            v
        }
    }

    pub fn step(&mut self, dt: f32, st: &State) -> bool {
        // preview cycles Idle → Live → Expanded with the user's motion settings
        if self.preview_at.elapsed().as_secs_f32() > 2.2 {
            self.preview_at = Instant::now();
            self.preview_phase = (self.preview_phase + 1) % 3;
            let s = [
                SHAPE_IDLE,
                SHAPE_LIVE,
                ShapeSpec {
                    w: 300.0,
                    h: 70.0,
                    ..SHAPE_EXPANDED
                },
            ][self.preview_phase];
            self.preview_w.set(s.w);
            self.preview_h.set(s.h);
        }
        let mut m = self.preview_w.step(dt, &st.env)
            | self.preview_h.step(dt, &st.env)
            | self.scroll.step(dt, &st.env);
        m |= self.focus.is_some();
        m
    }

    pub fn draw(&mut self, st: &mut State, gfx: &mut Gfx) {
        let Some(ctx) = gfx.begin(&mut self.surface) else {
            return;
        };
        let mut theme = st.theme.clone();
        theme.accent = st.accent();
        let Some(mut cv) = Canvas::new(gfx, ctx, theme) else {
            return;
        };
        cv.cursor = self.cursor.unwrap_or((-1e4, -1e4));
        cv.pressed = self.pressed.clone();
        cv.focused = self.keyboard_focus.clone();
        let (w, h) = self.size;
        let mica = crate::platform::is_windows_11();
        cv.rect(
            Rect::new(0.0, 0.0, w, h),
            if mica {
                cv.theme.surface.with_alpha(0.78)
            } else {
                cv.theme.surface
            },
        );
        // sidebar
        let side = Rect::new(0.0, 0.0, 210.0, h);
        cv.rect(side, NOTCH.with_alpha(0.25));
        let sr = Rect::new(14.0, 14.0, side.w - 28.0, 32.0);
        self.field(
            &mut cv,
            SEARCH_ID,
            sr,
            "Search settings",
            Some(Icon::Search),
        );
        let searching = !self.query().is_empty();
        for (i, s) in self.sections.iter().enumerate() {
            let r = Rect::new(10.0, 58.0 + i as f32 * 34.0, side.w - 20.0, 30.0);
            let active = i == self.section && !searching;
            if active || cv.hovered(r) {
                cv.sq(
                    r,
                    10.0,
                    if active {
                        cv.theme.surface_raised
                    } else {
                        LINE
                    },
                );
            }
            cv.icon(
                s.icon,
                r.x + 18.0,
                r.cy(),
                15.0,
                if active {
                    cv.theme.accent[0]
                } else {
                    cv.theme.mist
                },
            );
            cv.text(
                &crate::i18n::tr(s.name),
                &text::BODY,
                Rect::new(r.x + 36.0, r.y, r.w - 40.0, r.h),
                cv.theme.ink,
                Align::Left,
            );
            cv.zone_named(r, Action::Settings(0xFFFF, i as u16), true, s.name, 10.0);
        }
        // content
        let content = Rect::new(side.right() + 24.0, 0.0, w - side.right() - 48.0, h);
        let preview = Rect::new(content.x, 16.0, content.w, 96.0);
        self.preview(st, &mut cv, preview);
        let title = if searching {
            "Search results".to_string()
        } else {
            crate::i18n::tr(self.sections[self.section].name)
        };
        cv.text(
            &title,
            &text::TITLE.size(20.0),
            Rect::new(content.x, preview.bottom() + 10.0, content.w, 28.0),
            cv.theme.ink,
            Align::Left,
        );
        let list = Rect::new(
            content.x,
            preview.bottom() + 46.0,
            content.w,
            h - preview.bottom() - 56.0,
        );
        cv.push_clip(list);
        let mut y = list.y - self.scroll.value;
        let ids = self.visible();
        for id in ids.iter().copied() {
            let hgt = self.row_height(id);
            let r = Rect::new(list.x, y, list.w, hgt);
            y += hgt + 6.0;
            if !cv.visible(r) {
                continue;
            }
            self.draw_item(st, &mut cv, id, r);
        }
        let content_h = y + self.scroll.value - list.y;
        cv.pop_clip();
        let max = (content_h - list.h).max(0.0);
        if self.scroll.target > max {
            self.scroll.set(max);
        }
        if let Some((m, t)) = &self.message {
            if t.elapsed().as_secs() < 5 && !m.is_empty() {
                let mw = cv.measure(m, &text::BODY) + 32.0;
                let r = Rect::new(content.x + content.w - mw, h - 44.0, mw, 32.0);
                cv.sq(r, 16.0, SURFACE_RAISED);
                cv.text(m, &text::BODY, r, cv.theme.ink, Align::Center);
            }
        }
        self.zones = std::mem::take(&mut cv.zones);
        drop(cv);
        gfx.end(&mut self.surface);
        self.dirty = false;
    }

    fn row_height(&self, id: u16) -> f32 {
        match self.item(id).map(|i| &i.ctl) {
            Some(Ctl::List(..)) => 96.0,
            Some(Ctl::Info(_)) => 56.0,
            _ => 50.0,
        }
    }

    fn preview(&self, st: &State, cv: &mut Canvas, r: Rect) {
        cv.sq(r, 16.0, NOTCH.lerp(SURFACE_RAISED, 0.35));
        let scale = 0.8
            * match st.settings.appearance.size {
                115 => 1.15,
                130 => 1.3,
                150 => 1.5,
                _ => 1.0,
            };
        let floating = st.settings.appearance.style == Style::Floating;
        let (pw, ph) = (
            self.preview_w.value * scale * 0.7,
            self.preview_h.value * scale * 0.7,
        );
        let top = r.y + if floating { 8.0 } else { 0.0 };
        let factory = cv.gfx.factory.clone();
        cv.push_clip(r);
        if let Ok(p) = shapes::notch_path(
            &factory,
            r.cx(),
            top,
            pw,
            ph,
            (ph * 0.45).min(22.0),
            if floating { 0.0 } else { 8.0 },
            floating,
        ) {
            cv.fill_geo(&p, NOTCH);
            if floating {
                cv.stroke_geo(&p, RING, 1.0);
            }
        }
        if self.preview_phase > 0 {
            let pr = Rect::new(r.cx() - pw * 0.3, top + ph - 10.0, pw * 0.6, 3.0);
            cv.progress(pr, 0.62, true);
        }
        cv.pop_clip();
        cv.label(
            "Live preview",
            &text::MICRO,
            Rect::new(r.x + 14.0, r.bottom() - 22.0, 200.0, 14.0),
            FAINT,
            Align::Left,
        );
    }

    fn field(&self, cv: &mut Canvas, id: u16, r: Rect, placeholder: &str, icon: Option<Icon>) {
        let focused = self.focus == Some(id);
        cv.sq(r, R_INPUT, cv.theme.surface_raised);
        if focused {
            cv.stroke_rrect(
                r.inset(0.5),
                R_INPUT,
                cv.theme.accent[1].with_alpha(0.7),
                1.0,
            );
        }
        let mut x = r.x + 10.0;
        if let Some(ic) = icon {
            cv.icon(ic, x + 7.0, r.cy(), 14.0, cv.theme.mist);
            x += 22.0;
        }
        let f = self.fields.get(&id).cloned().unwrap_or_default();
        let tr = Rect::new(x, r.y, r.right() - x - 8.0, r.h);
        let multiline = r.h > 40.0;
        cv.push_clip(tr);
        if f.text.is_empty() {
            cv.text(
                placeholder,
                &text::BODY,
                if multiline {
                    Rect::new(tr.x, r.y + 6.0, tr.w, 18.0)
                } else {
                    tr
                },
                FAINT,
                Align::Left,
            );
        } else if multiline {
            cv.text_top(
                &f.text,
                &text::BODY.lines(5),
                tr.x,
                r.y + 6.0,
                tr.w,
                cv.theme.ink,
                Align::Left,
            );
        } else {
            let prefix: String = f.text.chars().take(f.caret).collect();
            let cx = cv.measure(&prefix, &text::BODY);
            let off = (cx - tr.w + 4.0).max(0.0);
            cv.text(
                &f.text,
                &text::BODY,
                Rect::new(tr.x - off, tr.y, tr.w + off + 400.0, tr.h),
                cv.theme.ink,
                Align::Left,
            );
            if focused {
                cv.rect(
                    Rect::new(tr.x + cx - off, r.cy() - 8.0, 1.5, 16.0),
                    cv.theme.accent[0],
                );
            }
        }
        cv.pop_clip();
        cv.zone_named(
            r,
            Action::Focus(InputId::Settings(id)),
            true,
            placeholder,
            R_INPUT,
        );
    }

    fn draw_item(&mut self, st: &State, cv: &mut Canvas, id: u16, r: Rect) {
        let Some(item) = self.item(id).copied() else {
            return;
        };
        cv.sq(r, 14.0, cv.theme.surface_raised.with_alpha(0.6));
        let label_w = r.w * 0.5;
        let ly = if item.hint.is_empty() { r.y } else { r.y - 8.0 };
        cv.text(
            &crate::i18n::tr(item.label),
            &text::BODY_MED,
            Rect::new(
                r.x + 16.0,
                ly,
                label_w,
                if matches!(item.ctl, Ctl::List(..)) {
                    50.0
                } else {
                    r.h
                },
            ),
            cv.theme.ink,
            Align::Left,
        );
        if !item.hint.is_empty() {
            cv.label(
                item.hint,
                &text::CAPTION,
                Rect::new(
                    r.x + 16.0,
                    ly + 18.0,
                    label_w,
                    if matches!(item.ctl, Ctl::List(..)) {
                        50.0
                    } else {
                        r.h
                    },
                ),
                cv.theme.mist,
                Align::Left,
            );
        }
        let cr = Rect::new(r.x + r.w * 0.52, r.y + 8.0, r.w * 0.48 - 16.0, r.h - 16.0);
        match &item.ctl {
            Ctl::Toggle(get, _) => {
                let on = get(st);
                let t = Rect::new(cr.right() - 44.0, cr.cy() - 12.0, 44.0, 24.0);
                if on {
                    if let Some(b) = cv.accent_brush(t.x, t.right(), 1.0) {
                        use windows::core::Interface;
                        cv.rrect_brush(t, 12.0, &b.cast().expect("brush"));
                    }
                } else {
                    cv.rrect(t, 12.0, TRACK);
                }
                cv.circle(
                    if on { t.right() - 12.0 } else { t.x + 12.0 },
                    t.cy(),
                    9.0,
                    WHITE,
                );
                cv.zone_named(
                    t.inset(-6.0),
                    Action::Settings(id, 0),
                    true,
                    item.label,
                    12.0,
                );
            }
            Ctl::Choice(options, get, _) => {
                let o = options(st);
                let cur = get(st).min(o.len().saturating_sub(1));
                if o.len() <= 3 && o.iter().map(|s| s.len()).sum::<usize>() < 30 {
                    // segmented control
                    let sw = cr.w.min(280.0);
                    let seg = Rect::new(cr.right() - sw, cr.cy() - 14.0, sw, 28.0);
                    cv.rrect(seg, 14.0, TRACK);
                    let n = o.len().max(1) as f32;
                    let cw = sw / n;
                    for (i, s) in o.iter().enumerate() {
                        let rr = Rect::new(seg.x + i as f32 * cw, seg.y, cw, seg.h);
                        if i == cur {
                            cv.rrect(rr.inset(2.0), 12.0, cv.theme.surface_raised.lerp(INK, 0.1));
                        }
                        cv.text(
                            s,
                            &text::CAPTION.weight(600),
                            rr,
                            if i == cur {
                                cv.theme.ink
                            } else {
                                cv.theme.mist
                            },
                            Align::Center,
                        );
                        cv.zone_named(rr, Action::Settings(id, i as u16 + 1), true, s, 12.0);
                    }
                } else {
                    // dropdown-style cycler: click to advance, shows current value
                    let dw = cr.w.min(260.0);
                    let d = Rect::new(cr.right() - dw, cr.cy() - 14.0, dw, 28.0);
                    cv.sq(d, 10.0, TRACK);
                    cv.text(
                        o.get(cur).map(|s| s.as_str()).unwrap_or(""),
                        &text::BODY,
                        Rect::new(d.x + 12.0, d.y, d.w - 36.0, d.h),
                        cv.theme.ink,
                        Align::Left,
                    );
                    cv.icon(
                        Icon::ChevronDown,
                        d.right() - 14.0,
                        d.cy(),
                        12.0,
                        cv.theme.mist,
                    );
                    let next = (cur + 1) % o.len().max(1);
                    cv.zone_named(
                        d,
                        Action::Settings(id, next as u16 + 1),
                        true,
                        item.label,
                        10.0,
                    );
                }
            }
            Ctl::Slider {
                min, max, get, fmt, ..
            } => {
                let v = get(st);
                let frac = ((v - min) / (max - min)).clamp(0.0, 1.0);
                let sr = Rect::new(cr.x, cr.y, cr.w - 70.0, cr.h);
                cv.slider(
                    sr,
                    frac,
                    Action::Slider(SliderKind::SettingsSlider(id)),
                    item.label,
                );
                cv.text(
                    &fmt(v),
                    &text::CAPTION_TNUM,
                    Rect::new(cr.right() - 64.0, cr.y, 64.0, cr.h),
                    cv.theme.mist,
                    Align::Right,
                );
            }
            Ctl::Text(get, _) => {
                if self.focus != Some(id) && !self.fields.contains_key(&id) {
                    let mut f = TextField::default();
                    f.set(&get(st));
                    self.fields.insert(id, f);
                }
                if self.focus != Some(id) {
                    if let Some(f) = self.fields.get_mut(&id) {
                        let cur = get(st);
                        if f.text != cur {
                            f.set(&cur);
                        }
                    }
                }
                self.field(
                    cv,
                    id,
                    Rect::new(cr.x, cr.cy() - 15.0, cr.w, 30.0),
                    "",
                    None,
                );
            }
            Ctl::List(get, _) => {
                if self.focus != Some(id) {
                    let mut f = TextField::default();
                    f.set(&get(st).join("\n"));
                    self.fields.insert(id, f);
                }
                self.field(cv, id, cr, "One per line", None);
            }
            Ctl::Secret(name) => {
                let has = crate::platform::credentials::has_secret(name);
                if self.focus == Some(id) {
                    self.field(
                        cv,
                        id,
                        Rect::new(cr.x, cr.cy() - 15.0, cr.w, 30.0),
                        "Paste key, then Enter",
                        None,
                    );
                } else {
                    let r2 = Rect::new(cr.x, cr.cy() - 15.0, cr.w, 30.0);
                    cv.sq(r2, R_INPUT, cv.theme.surface_raised);
                    cv.text(
                        if has {
                            "••••••••••••  (saved)"
                        } else {
                            "Not set — click to add"
                        },
                        &text::BODY,
                        r2.inset_xy(12.0, 0.0),
                        if has { cv.theme.ink } else { FAINT },
                        Align::Left,
                    );
                    cv.zone_named(
                        r2,
                        Action::Focus(InputId::Settings(id)),
                        true,
                        item.label,
                        R_INPUT,
                    );
                }
            }
            Ctl::Hotkey(get, _) => {
                let rec = self.recording == Some(id);
                let label = if rec {
                    "Press keys…".to_string()
                } else {
                    get(st)
                };
                let hw = cv.measure(&label, &text::BODY_MED) + 32.0;
                let b = Rect::new(cr.right() - hw, cr.cy() - 14.0, hw, 28.0);
                cv.sq(
                    b,
                    10.0,
                    if rec {
                        cv.theme.accent[1].with_alpha(0.4)
                    } else {
                        TRACK
                    },
                );
                cv.text(&label, &text::BODY_MED, b, cv.theme.ink, Align::Center);
                cv.zone_named(b, Action::Settings(id, 0), true, item.label, 10.0);
            }
            Ctl::Button(label, _) => {
                let bw = cv.measure(label, &text::BODY_MED) + 32.0;
                cv.pill_button(
                    label,
                    Rect::new(cr.right() - bw, cr.cy() - 14.0, bw, 28.0),
                    Action::Settings(id, 0),
                    false,
                );
            }
            Ctl::Account(state, _) => {
                let s = state(st);
                let label = match s {
                    AccountState::Connected => "Disconnect",
                    _ => "Connect",
                };
                let bw = cv.pill_width(label, 96.0);
                let br = Rect::new(cr.right() - bw, cr.cy() - 14.0, bw, 28.0);
                cv.pill_button(
                    label,
                    br,
                    Action::Settings(id, 0),
                    s == AccountState::Disconnected,
                );
                // live status to the left of the button
                let (dot, text_c, status) = match s {
                    AccountState::Connected => (GOOD, GOOD, "Connected"),
                    AccountState::Pending => (WARN, cv.theme.mist, "Waiting for browser…"),
                    AccountState::Disconnected => (FAINT, cv.theme.mist, "Not connected"),
                };
                let tr = crate::i18n::tr(status);
                let tw = cv.measure(&tr, &text::CAPTION.weight(600));
                let tx = br.x - 12.0 - tw;
                if s == AccountState::Connected {
                    cv.rrect(
                        Rect::new(tx - 22.0, cr.cy() - 11.0, tw + 30.0, 22.0),
                        11.0,
                        GOOD.with_alpha(0.14),
                    );
                }
                cv.circle(tx - 10.0, cr.cy(), 3.5, dot);
                cv.text(
                    &tr,
                    &text::CAPTION.weight(600),
                    Rect::new(tx, cr.y, tw + 2.0, cr.h),
                    text_c,
                    Align::Left,
                );
            }
            Ctl::Info(f) => {
                let t = f(st);
                cv.text_top(
                    &t,
                    &text::CAPTION.lines(3),
                    cr.x,
                    r.y + 10.0,
                    cr.w,
                    cv.theme.mist,
                    Align::Right,
                );
            }
        }
    }

    // ------------------------------------------------------------ input

    fn zone_at(&self, x: f32, y: f32) -> Option<(Action, Rect)> {
        self.zones
            .iter()
            .rev()
            .find(|z| z.rect.contains(x, y))
            .map(|z| (z.action.clone(), z.rect))
    }

    pub fn on_move(&mut self, st: &mut State, x: f32, y: f32) {
        self.cursor = Some((x, y));
        if let Some((id, r)) = self.slider {
            self.set_slider(st, id, (x - r.x) / r.w.max(1.0));
        }
        self.dirty = true;
    }

    pub fn on_down(&mut self, st: &mut State, x: f32, y: f32) {
        self.keyboard_focus = None;
        if let Some((a, r)) = self.zone_at(x, y) {
            if let Action::Slider(SliderKind::SettingsSlider(id)) = a {
                self.slider = Some((id, r));
                self.set_slider(st, id, (x - r.x) / r.w.max(1.0));
            }
            if let Action::Focus(InputId::Settings(id)) = a {
                self.commit_focus(st);
                self.focus = Some(id);
                if let Some(Ctl::Secret(_)) = self.item(id).map(|i| &i.ctl) {
                    self.fields.insert(id, TextField::default());
                }
            } else if self.focus.is_some() && !matches!(a, Action::Slider(_)) {
                self.commit_focus(st);
            }
            self.pressed = Some(a);
        } else {
            self.commit_focus(st);
        }
        self.dirty = true;
    }

    pub fn on_up(&mut self, st: &mut State, x: f32, y: f32) {
        if self.slider.take().is_some() {
            st.save_settings_soon();
            self.pressed = None;
            return;
        }
        let pressed = self.pressed.take();
        if let Some((a, _)) = self.zone_at(x, y) {
            if pressed.as_ref() == Some(&a) {
                self.activate(st, a);
            }
        }
        self.dirty = true;
    }

    pub fn on_wheel(&mut self, delta: f32) {
        let t = (self.scroll.target - delta / 120.0 * 60.0).max(0.0);
        self.scroll.set(t);
        self.dirty = true;
    }

    fn set_slider(&mut self, st: &mut State, id: u16, frac: f32) {
        if let Some(Ctl::Slider {
            min,
            max,
            step,
            set,
            ..
        }) = self.item(id).map(|i| &i.ctl)
        {
            let v = min + (max - min) * frac.clamp(0.0, 1.0);
            let v = (v / step).round() * step;
            set(st, v.clamp(*min, *max));
            st.apply_settings();
            st.dirty = true;
        }
    }

    fn activate(&mut self, st: &mut State, a: Action) {
        match a {
            Action::Settings(0xFFFF, s) => {
                self.section = s as usize;
                self.fields.remove(&SEARCH_ID);
                self.scroll.snap(0.0);
            }
            Action::Settings(id, sub) => {
                let Some(item) = self.item(id) else { return };
                let mut msg = None;
                match &item.ctl {
                    Ctl::Toggle(get, set) => {
                        let v = !get(st);
                        set(st, v);
                    }
                    Ctl::Choice(_, _, set) => set(st, sub.saturating_sub(1) as usize),
                    Ctl::Hotkey(..) => self.recording = Some(id),
                    Ctl::Button(_, f) | Ctl::Account(_, f) => msg = Some(f(st)),
                    _ => {}
                }
                if let Some(m) = msg {
                    self.message = Some((m, Instant::now()));
                }
                st.save_settings_soon();
            }
            Action::Slider(_) | Action::Focus(_) => {}
            _ => {}
        }
        self.dirty = true;
    }

    /// Writes a focused text field back into settings.
    pub fn commit_focus(&mut self, st: &mut State) {
        let Some(id) = self.focus.take() else { return };
        if id == SEARCH_ID {
            self.focus = None;
            return;
        }
        let text = self
            .fields
            .get(&id)
            .map(|f| f.text.clone())
            .unwrap_or_default();
        if let Some(item) = self.item(id) {
            match &item.ctl {
                Ctl::Text(_, set) => set(st, text),
                Ctl::List(_, set) => set(
                    st,
                    text.lines()
                        .map(|l| l.trim().to_string())
                        .filter(|l| !l.is_empty())
                        .collect(),
                ),
                Ctl::Secret(name) => {
                    if !text.trim().is_empty() {
                        crate::platform::credentials::set_secret(name, text.trim());
                        self.message = Some(("Key saved (encrypted)".into(), Instant::now()));
                    }
                    self.fields.remove(&id);
                }
                _ => {}
            }
            st.save_settings_soon();
        }
        self.dirty = true;
    }

    /// Returns true if the key was consumed.
    pub fn on_key(
        &mut self,
        st: &mut State,
        vk: u32,
        ctrl: bool,
        shift: bool,
        clipboard: &dyn Fn() -> Option<String>,
    ) -> bool {
        use crate::app::input::vk as k;
        self.dirty = true;
        if let Some(id) = self.recording {
            if vk == k::ESCAPE {
                self.recording = None;
                return true;
            }
            if let Some(combo) = crate::platform::format_hotkey(vk) {
                if let Some(Ctl::Hotkey(_, set)) = self.item(id).map(|i| &i.ctl) {
                    set(st, combo);
                    st.requests.push(Request::RegisterHotkeys);
                    st.save_settings_soon();
                }
                self.recording = None;
            }
            return true;
        }
        if let Some(id) = self.focus {
            let multiline = matches!(self.item(id).map(|i| &i.ctl), Some(Ctl::List(..)));
            let f = self.fields.entry(id).or_default();
            match vk {
                k::BACK => f.backspace(),
                k::DELETE => f.delete(),
                k::LEFT => f.caret = f.caret.saturating_sub(1),
                k::RIGHT => f.caret = (f.caret + 1).min(f.text.chars().count()),
                k::HOME => f.caret = 0,
                k::END => f.caret = f.text.chars().count(),
                0x41 if ctrl => {
                    f.anchor = Some(0);
                    f.caret = f.text.chars().count();
                }
                0x56 if ctrl => {
                    if let Some(c) = clipboard() {
                        f.insert(&if multiline {
                            c
                        } else {
                            c.replace(['\r', '\n'], " ")
                        });
                    }
                }
                k::RETURN if multiline && !ctrl => f.insert("\n"),
                k::RETURN => {
                    self.commit_focus(st);
                    return true;
                }
                k::ESCAPE => {
                    self.focus = None;
                    self.fields.remove(&id);
                    return true;
                }
                _ => return false,
            }
            if id == SEARCH_ID {
                self.scroll.snap(0.0);
            }
            return true;
        }
        match vk {
            k::TAB | k::DOWN | k::UP => {
                let f: Vec<Action> = self
                    .zones
                    .iter()
                    .filter(|z| z.focusable)
                    .map(|z| z.action.clone())
                    .collect();
                if !f.is_empty() {
                    let back = vk == k::UP || shift;
                    let i = self
                        .keyboard_focus
                        .as_ref()
                        .and_then(|a| f.iter().position(|x| x == a));
                    let n = f.len();
                    let j = match (i, back) {
                        (None, false) => 0,
                        (None, true) => n - 1,
                        (Some(i), false) => (i + 1) % n,
                        (Some(i), true) => (i + n - 1) % n,
                    };
                    self.keyboard_focus = Some(f[j].clone());
                }
                true
            }
            k::RETURN | k::SPACE => {
                if let Some(a) = self.keyboard_focus.clone() {
                    if let Action::Focus(InputId::Settings(id)) = a {
                        self.focus = Some(id);
                    } else {
                        self.activate(st, a);
                    }
                }
                true
            }
            k::LEFT | k::RIGHT => {
                if let Some(Action::Slider(SliderKind::SettingsSlider(id))) =
                    self.keyboard_focus.clone()
                {
                    if let Some(Ctl::Slider {
                        min,
                        max,
                        step,
                        get,
                        set,
                        ..
                    }) = self.item(id).map(|i| &i.ctl)
                    {
                        let v = (get(st) + if vk == k::RIGHT { *step } else { -*step })
                            .clamp(*min, *max);
                        set(st, v);
                        st.save_settings_soon();
                    }
                }
                true
            }
            _ => false,
        }
    }

    pub fn on_char(&mut self, c: char) {
        if self.recording.is_some() || c.is_control() {
            return;
        }
        if let Some(id) = self.focus {
            self.fields.entry(id).or_default().insert(&c.to_string());
            if id == SEARCH_ID {
                self.scroll.snap(0.0);
            }
            self.dirty = true;
        }
    }

    pub fn resize(&mut self, w: u32, h: u32, dpi: f32) {
        self.surface.resize(w, h, dpi);
        self.size = (w as f32 * 96.0 / dpi, h as f32 * 96.0 / dpi);
        self.dpi = dpi;
        self.dirty = true;
    }
}

fn tile_names(_: &State) -> Vec<String> {
    crate::ui::dashboard::TILES
        .iter()
        .map(|t| t.1.to_string())
        .collect()
}

fn tile_get(s: &State, slot: usize) -> usize {
    let id = s
        .settings
        .widgets
        .tiles
        .get(slot)
        .cloned()
        .unwrap_or_default();
    crate::ui::dashboard::TILES
        .iter()
        .position(|t| t.0 == id)
        .unwrap_or(0)
}

fn tile_set(s: &mut State, slot: usize, v: usize) {
    let tiles = &mut s.settings.widgets.tiles;
    while tiles.len() < 4 {
        tiles.push("battery".into());
    }
    if let Some(t) = crate::ui::dashboard::TILES.get(v) {
        tiles[slot] = t.0.to_string();
    }
}

fn widget_on(s: &State, i: usize) -> bool {
    let k = crate::ui::widgets::KINDS[i];
    s.settings.widgets.desktop.iter().any(|d| d.kind == k)
}

fn widget_set(s: &mut State, i: usize, on: bool) {
    let k = crate::ui::widgets::KINDS[i];
    let w = &mut s.settings.widgets.desktop;
    w.retain(|d| d.kind != k);
    if on {
        let n = w.len() as i32;
        w.push(DesktopWidget {
            kind: k.to_string(),
            x: 80 + 240 * (n % 4),
            y: 120 + 160 * (n / 4),
        });
    }
}
