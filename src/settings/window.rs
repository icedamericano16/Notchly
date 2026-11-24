//! The Settings window: drawn with the same renderer and tokens as the notch.
//! Data-driven sections; every change applies live and persists.

use std::collections::HashMap;
use std::time::Instant;

use crate::app::state::{State, TextField};
use crate::app::{Action, InputId, Request, SliderKind};
use crate::motion::{Spring, BOUNCY, GENTLE, ISLAND, ISLAND_H, SNAPPY};
use crate::render::canvas::Zone;
use crate::render::glass::compositor::PreviewBackdrop;
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
    /// A row of glass chips spanning the row (presets).
    Chips(fn(&State) -> Vec<String>, Get<usize>, Set<usize>),
    /// Starts a new card titled by the item's label (not a row).
    Card,
    /// "-  value  +" for whole numbers; holding a button repeats.
    Stepper {
        min: f32,
        max: f32,
        step: f32,
        get: Get<f32>,
        set: Set<f32>,
        fmt: fn(f32) -> String,
    },
    /// A destructive button that morphs into "Label · Cancel" before it acts.
    Confirm(&'static str, fn(&mut State) -> String),
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
    /// Shown only when this returns true (None = always).
    pub show: Option<fn(&State) -> bool>,
}

pub struct Section {
    pub name: &'static str,
    pub icon: Icon,
    /// The icon's feature tint in the sidebar.
    pub tint: Color,
    pub group: Group,
    /// One line under the section title.
    pub desc: &'static str,
    /// Puts this section's settings back to their defaults.
    pub reset: Option<fn(&mut State)>,
    /// Shows the sticky live preview.
    pub preview: bool,
    pub items: Vec<Item>,
}

/// The sidebar's groups.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Group {
    Personalise,
    Features,
    System,
}

impl Group {
    pub const ALL: [Group; 3] = [Group::Personalise, Group::Features, Group::System];
    pub fn name(self) -> &'static str {
        match self {
            Group::Personalise => "Personalise",
            Group::Features => "Features",
            Group::System => "System",
        }
    }
}

fn card(title: &'static str) -> Item {
    Item {
        label: title,
        hint: "",
        ctl: Ctl::Card,
        show: None,
    }
}

fn it(label: &'static str, hint: &'static str, ctl: Ctl) -> Item {
    Item {
        label,
        hint,
        ctl,
        show: None,
    }
}
fn when(i: Item, f: fn(&State) -> bool) -> Item {
    Item { show: Some(f), ..i }
}
fn glass_on(s: &State) -> bool {
    s.settings.appearance.material != Material::Solid
}
fn liquid_on(s: &State) -> bool {
    s.settings.appearance.material == Material::LiquidGlass
}

/// Water settings changed: the schedule and the page's glass follow straight away.
fn water_changed(s: &mut State) {
    let w = s.settings.water.clone();
    let d = &mut s.f.state.water;
    if !w.reminders {
        d.reminders_off();
    } else if !d.snoozed() && !d.paused_today() {
        d.schedule(&w);
    }
    s.f.water_fill
        .set((d.ml as f32 / w.goal_ml.max(1) as f32).min(1.0));
    s.f.state_dirty = true;
}

/// After settings change wholesale (import, reset, undo): apply the side effects that
/// single rows apply themselves.
pub fn after_bulk_change(s: &mut State) {
    s.settings.sanitize();
    crate::platform::set_autostart(s.settings.general.start_with_windows);
    s.requests.push(Request::RegisterHotkeys);
    water_changed(s);
    s.apply_settings();
}

fn reset_appearance(s: &mut State) {
    let a = &mut s.settings.appearance;
    let pos = std::mem::take(&mut a.floating_pos);
    *a = Appearance::default();
    a.floating_pos = pos;
}
fn reset_glass(s: &mut State) {
    s.settings.appearance.glass = GlassSettings::default();
}
fn reset_motion(s: &mut State) {
    s.settings.motion = Motion::default();
}
fn reset_widgets(s: &mut State) {
    s.settings.widgets = Widgets::default();
}
fn reset_live(s: &mut State) {
    s.settings.live = Live::default();
}
fn reset_popups(s: &mut State) {
    s.settings.popups = Popups::default();
    s.settings.sound = Sound::default();
}
fn reset_music(s: &mut State) {
    let choice = std::mem::take(&mut s.settings.music.session_choice);
    s.settings.music = Music::default();
    s.settings.music.session_choice = choice;
}
fn reset_productivity(s: &mut State) {
    s.settings.productivity = Productivity::default();
}
fn reset_water(s: &mut State) {
    s.settings.water = Water::default();
}
fn reset_ai(s: &mut State) {
    s.settings.ai = Ai::default();
}
fn reset_calendar(s: &mut State) {
    // client IDs are connection setup, not preferences: they stay
    let c = &s.settings.calendar;
    let (g, m) = (c.google_client_id.clone(), c.microsoft_client_id.clone());
    s.settings.calendar = Calendar {
        google_client_id: g,
        microsoft_client_id: m,
        ..Calendar::default()
    };
    s.settings.weather = Weather::default();
}
fn reset_general(s: &mut State) {
    let onboarded = s.settings.general.onboarded;
    s.settings.general = General::default();
    s.settings.general.onboarded = onboarded;
}
fn reset_quiet(s: &mut State) {
    s.settings.quiet = Quiet::default();
}
fn reset_shortcuts(s: &mut State) {
    s.settings.shortcuts = Shortcuts::default();
}
fn reset_privacy(s: &mut State) {
    s.settings.privacy = Privacy::default();
    s.settings.sync = Sync::default();
}
/// The strength slider's label (snap detents at Clearest / Default / Tinted).
pub const STRENGTH_LABEL: &str = "Glass strength";
const DETENTS: [f32; 3] = [0.0, 0.5, 1.0];
fn strength_fmt(v: f32) -> String {
    let t = v / 100.0;
    match DETENTS.iter().position(|d| (t - d).abs() < 0.005) {
        Some(0) => crate::i18n::tr("Clearest"),
        Some(1) => crate::i18n::tr("Default"),
        Some(_) => crate::i18n::tr("Tinted"),
        None => format!("{v:.0}%"),
    }
}
/// Snaps to a detent when within reach (like a haptic notch).
fn detent(t: f32) -> f32 {
    DETENTS
        .iter()
        .copied()
        .find(|d| (t - d).abs() < 0.035)
        .unwrap_or(t)
}
fn glass_status(s: &State) -> String {
    let tier = crate::i18n::tr(s.glass.tier.name());
    let what = crate::i18n::tr(match s.settings.appearance.material {
        Material::Solid => "The true-black notch.",
        Material::Glass => {
            "Frosted glass: a strong blur, tint and a lit rim. Calm and light on power."
        }
        Material::LiquidGlass => {
            "Glass that bends light at its edges, lifts colour and moves like liquid."
        }
    });
    let status = match s.glass.reason {
        Some(r) => format!(
            "{} {} — {}",
            crate::i18n::tr("Showing"),
            tier.to_lowercase(),
            crate::i18n::tr(r.message())
        ),
        None => format!("{}: {tier}", crate::i18n::tr("Active")),
    };
    format!(
        "{what}
{status}"
    )
}
fn custom(s: &mut State) {
    s.settings.appearance.glass.preset = GlassPreset::Custom;
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
fn ml(v: f32) -> String {
    format!("{v:.0} ml")
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
            name: "Appearance",
            icon: Icon::Palette,
            tint: TINT_SHELF,
            group: Group::Personalise,
            desc: "Shape, material and colour of the notch",
            reset: Some(reset_appearance),
            preview: true,
            items: vec![
                card("Shape"),
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
                card("Material"),
                it(
                    "Material",
                    "Liquid Glass bends and blurs what's behind the notch",
                    Ctl::Choice(
                        |_| opts(&["Solid", "Glass", "Liquid Glass"]),
                        |s| match s.settings.appearance.material {
                            Material::Solid => 0,
                            Material::Glass => 1,
                            Material::LiquidGlass => 2,
                        },
                        |s, v| {
                            s.settings.appearance.material = match v {
                                1 => Material::Glass,
                                2 => Material::LiquidGlass,
                                _ => Material::Solid,
                            }
                        },
                    ),
                ),
                when(
                    it(
                        STRENGTH_LABEL,
                        "Clearest, Default or Tinted, and anything between",
                        Ctl::Slider {
                            min: 0.0,
                            max: 100.0,
                            step: 1.0,
                            get: |s| s.settings.appearance.glass.strength * 100.0,
                            set: |s, v| {
                                s.settings.appearance.glass.strength = detent(v / 100.0);
                                custom(s);
                            },
                            fmt: strength_fmt,
                        },
                    ),
                    glass_on,
                ),
                when(
                    it(
                        "Glass style",
                        "",
                        Ctl::Chips(
                            |_| GlassPreset::ALL.iter().map(|p| p.name().to_string()).collect(),
                            |s| {
                                GlassPreset::ALL
                                    .iter()
                                    .position(|p| *p == s.settings.appearance.glass.preset)
                                    .unwrap_or(usize::MAX)
                            },
                            |s, v| {
                                if let Some(p) = GlassPreset::ALL.get(v) {
                                    s.settings.appearance.glass.apply_preset(*p);
                                }
                            },
                        ),
                    ),
                    glass_on,
                ),
                when(it("Glass status", "", Ctl::Info(glass_status)), glass_on),
                card("Colour"),
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
                card("Details"),
                it(
                    "Privacy dots",
                    "Orange mic, green camera, blue location",
                    toggle!(appearance.privacy_dots),
                ),
                it(
                    "Clock in the idle notch",
                    "Time plus headphones, Bluetooth and charging status",
                    toggle!(appearance.idle_clock),
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
            ],
        },
        Section {
            name: "Glass & Fluid",
            icon: Icon::Waves,
            tint: TINT_SYSTEM,
            group: Group::Personalise,
            desc: "How the glass bends, blurs and moves",
            reset: Some(reset_glass),
            preview: true,
            items: vec![
                card("Advanced glass"),
                when(
                    it(
                        "Glass is off",
                        "",
                        Ctl::Info(|_| {
                            crate::i18n::tr("Choose Glass or Liquid Glass in Appearance to use these")
                        }),
                    ),
                    |s| !glass_on(s),
                ),
                when(
                    it(
                        "Refraction",
                        "How much the edges bend what's behind",
                        Ctl::Slider {
                            min: 0.0,
                            max: 100.0,
                            step: 1.0,
                            get: |s| s.settings.appearance.glass.refraction * 100.0,
                            set: |s, v| {
                                s.settings.appearance.glass.refraction = v / 100.0;
                                custom(s);
                            },
                            fmt: pct,
                        },
                    ),
                    liquid_on,
                ),
                when(
                    it(
                        "Blur",
                        "",
                        Ctl::Slider {
                            min: 0.0,
                            max: 100.0,
                            step: 1.0,
                            get: |s| s.settings.appearance.glass.blur * 100.0,
                            set: |s, v| {
                                s.settings.appearance.glass.blur = v / 100.0;
                                custom(s);
                            },
                            fmt: pct,
                        },
                    ),
                    liquid_on,
                ),
                when(
                    it(
                        "Rim light",
                        "The bright edge facing the light",
                        Ctl::Slider {
                            min: 0.0,
                            max: 100.0,
                            step: 1.0,
                            get: |s| s.settings.appearance.glass.rim_light * 100.0,
                            set: |s, v| {
                                s.settings.appearance.glass.rim_light = v / 100.0;
                                custom(s);
                            },
                            fmt: pct,
                        },
                    ),
                    glass_on,
                ),
                when(
                    it(
                        "Motion response",
                        "How much the glass stretches and ripples as it moves",
                        Ctl::Slider {
                            min: 0.0,
                            max: 100.0,
                            step: 1.0,
                            get: |s| s.settings.appearance.glass.motion_response * 100.0,
                            set: |s, v| s.settings.appearance.glass.motion_response = v / 100.0,
                            fmt: pct,
                        },
                    ),
                    glass_on,
                ),
                when(
                    it(
                        "Glass tint",
                        "Where the glass colour comes from",
                        Ctl::Choice(
                            |_| opts(&["Theme", "Adaptive", "Custom"]),
                            |s| match s.settings.appearance.glass.tint_source {
                                TintSource::Theme => 0,
                                TintSource::Adaptive => 1,
                                TintSource::Custom => 2,
                            },
                            |s, v| {
                                s.settings.appearance.glass.tint_source = match v {
                                    1 => TintSource::Adaptive,
                                    2 => TintSource::Custom,
                                    _ => TintSource::Theme,
                                }
                            },
                        ),
                    ),
                    glass_on,
                ),
                when(
                    it(
                        "Custom tint",
                        "A colour like #1C1C24",
                        textf!(appearance.glass.custom_tint),
                    ),
                    |s| {
                        glass_on(s)
                            && s.settings.appearance.glass.tint_source == TintSource::Custom
                    },
                ),
                when(
                    it(
                        "Light follows the pointer",
                        "The sheen drifts toward the cursor",
                        toggle!(appearance.glass.specular_follows_pointer),
                    ),
                    glass_on,
                ),
                when(
                    it(
                        "Idle shimmer",
                        "A slow sheen every few seconds",
                        toggle!(appearance.glass.idle_shimmer),
                    ),
                    glass_on,
                ),
                when(
                    it(
                        "Show Notchly in screen recordings",
                        "The glass keeps the notch out of screen captures; this shows tinted glass instead",
                        toggle!(appearance.glass.show_in_recordings),
                    ),
                    glass_on,
                ),
                when(
                    it(
                        "Follow Windows transparency",
                        "Solid when transparency effects are off",
                        toggle!(appearance.glass.follow_system_transparency),
                    ),
                    glass_on,
                ),
                when(
                    it(
                        "Solid in High Contrast",
                        "System colours and strong edges",
                        toggle!(appearance.glass.follow_system_contrast),
                    ),
                    glass_on,
                ),
            ],
        },
        Section {
            name: "Motion",
            icon: Icon::Sparkles,
            tint: TINT_CONTROL,
            group: Group::Personalise,
            desc: "The speed and feel of every animation",
            reset: Some(reset_motion),
            preview: false,
            items: vec![
                card("Animation"),
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
                card("Visualiser"),
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
            name: "Dashboard & widgets",
            icon: Icon::Widgets,
            tint: TINT_WIDGETS,
            group: Group::Personalise,
            desc: "Home tiles and desktop widgets",
            reset: Some(reset_widgets),
            preview: false,
            items: vec![
                card("Countdown"),
                it(
                    "Countdown title",
                    "For the Countdown desktop widget",
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
                card("Desktop widgets"),
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
            name: "Live activities",
            icon: Icon::Bolt,
            tint: TINT_TIMER,
            group: Group::Features,
            desc: "What the pill shows while something is happening",
            reset: Some(reset_live),
            preview: false,
            items: vec![
                card("Activities"),
                it(
                    "Dual island",
                    "Show a second activity as a bubble",
                    toggle!(live.dual_island),
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
                card("Live Activities API"),
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
            ],
        },
        Section {
            name: "Pop-ups",
            icon: Icon::Bell,
            tint: TINT_CLIP,
            group: Group::Features,
            desc: "System pop-ups, notifications and sounds",
            reset: Some(reset_popups),
            preview: false,
            items: vec![
                card("System pop-ups"),
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
                    "Hide the Windows volume flyout",
                    "Opens the Windows setting (Notchly never hides system UI itself)",
                    Ctl::Button("Open", |_| {
                        crate::platform::shell_open("ms-settings:easeofaccess-display");
                        String::new()
                    }),
                ),
                card("Notifications"),
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
                card("Sounds"),
                it(
                    "Timer chime",
                    "When a timer or focus session ends",
                    toggle!(sound.timer_chime),
                ),
                it(
                    "Reminder chime",
                    "Water, to-do and prayer reminders",
                    toggle!(sound.reminder_chime),
                ),
                it(
                    "Interface sounds",
                    "Soft clicks for buttons and switches",
                    toggle!(sound.ui_sounds),
                ),
            ],
        },
        Section {
            name: "Music",
            icon: Icon::Music,
            tint: TINT_MUSIC,
            group: Group::Features,
            desc: "Now playing, lyrics and the player",
            reset: Some(reset_music),
            preview: false,
            items: vec![
                card("Player"),
                it("Music", "Show what's playing", toggle!(music.enabled)),
                it(
                    "Preferred player",
                    "Favoured when several apps play (e.g. Spotify)",
                    textf!(music.preferred_player),
                ),
                it(
                    "Per-app volume",
                    "A volume slider for the player",
                    toggle!(music.app_volume),
                ),
                card("Lyrics"),
                it("Synced lyrics", "From LRCLIB, with lyrics.ovh as a fallback", toggle!(music.lyrics)),
                it(
                    "Lyrics on the music page",
                    "The CC button on the player does the same",
                    toggle!(music.captions),
                ),
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
            ],
        },
        Section {
            name: "Productivity",
            icon: Icon::CheckCircle,
            tint: TINT_TODO,
            group: Group::Features,
            desc: "Focus timer, clipboard, prayer times and tools",
            reset: Some(reset_productivity),
            preview: false,
            items: vec![
                card("Focus timer"),
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
                card("To-do, shelf and notes"),
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
                it("Quick notes", "", toggle!(productivity.notes)),
                card("Clipboard"),
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
                card("Prayer times"),
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
                card("Tools"),
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
            name: "Water",
            icon: Icon::Droplet,
            tint: TINT_WATER,
            group: Group::Features,
            desc: "Your daily goal and gentle reminders",
            reset: Some(reset_water),
            preview: false,
            items: vec![
                card("Reminders"),
                it(
                    "Water reminders",
                    "The same switch as the bell on the Water page",
                    Ctl::Toggle(
                        |s| s.settings.water.reminders,
                        |s, v| {
                            s.settings.water.reminders = v;
                            water_changed(s);
                        },
                    ),
                ),
                it(
                    "Reminder style",
                    "How a reminder shows itself",
                    Ctl::Choice(
                        |_| opts(&["Pop-up", "Pop-up + sound", "Silent glow"]),
                        |s| match s.settings.water.style {
                            ReminderStyle::Popup => 0,
                            ReminderStyle::PopupSound => 1,
                            ReminderStyle::Glow => 2,
                        },
                        |s, v| {
                            s.settings.water.style = match v {
                                0 => ReminderStyle::Popup,
                                2 => ReminderStyle::Glow,
                                _ => ReminderStyle::PopupSound,
                            }
                        },
                    ),
                ),
                it(
                    "Snooze length",
                    "For the Snooze button on a reminder",
                    Ctl::Choice(
                        |_| SNOOZE_CHOICES.iter().map(|m| format!("{m} min")).collect(),
                        |s| {
                            SNOOZE_CHOICES
                                .iter()
                                .position(|m| *m == s.settings.water.snooze_min)
                                .unwrap_or(1)
                        },
                        |s, v| s.settings.water.snooze_min = SNOOZE_CHOICES[v.min(2)],
                    ),
                ),
                it(
                    "Stop reminding once the goal is reached",
                    "Reminders start again tomorrow",
                    Ctl::Toggle(
                        |s| s.settings.water.stop_at_goal,
                        |s, v| {
                            s.settings.water.stop_at_goal = v;
                            water_changed(s);
                        },
                    ),
                ),
                card("Goal"),
                it(
                    "Daily goal",
                    "500 to 6000 ml",
                    Ctl::Stepper {
                        min: 500.0,
                        max: 6000.0,
                        step: 50.0,
                        get: |s| s.settings.water.goal_ml as f32,
                        set: |s, v| {
                            s.settings.water.goal_ml = v as u32;
                            water_changed(s);
                        },
                        fmt: ml,
                    },
                ),
                it(
                    "Glass size",
                    "What one tap on the Water page adds",
                    Ctl::Stepper {
                        min: 50.0,
                        max: 1000.0,
                        step: 25.0,
                        get: |s| s.settings.water.glass_ml as f32,
                        set: |s, v| {
                            s.settings.water.glass_ml = v as u32;
                            water_changed(s);
                        },
                        fmt: ml,
                    },
                ),
                card("Active hours"),
                it(
                    "Reminders from",
                    "",
                    Ctl::Stepper {
                        min: 0.0,
                        max: 1425.0,
                        step: 15.0,
                        get: |s| s.settings.water.start_min as f32,
                        set: |s, v| {
                            let w = &mut s.settings.water;
                            w.start_min = (v as u32).min(w.end_min.saturating_sub(15));
                            water_changed(s);
                        },
                        fmt: hm,
                    },
                ),
                it(
                    "Reminders until",
                    "",
                    Ctl::Stepper {
                        min: 15.0,
                        max: 1440.0,
                        step: 15.0,
                        get: |s| s.settings.water.end_min as f32,
                        set: |s, v| {
                            let w = &mut s.settings.water;
                            w.end_min = (v as u32).max(w.start_min + 15);
                            water_changed(s);
                        },
                        fmt: hm,
                    },
                ),
            ],
        },
        Section {
            name: "Ask & AI",
            icon: Icon::Sparkles,
            tint: TINT_PRAYER,
            group: Group::Features,
            desc: "The command palette and AI providers",
            reset: Some(reset_ai),
            preview: false,
            items: vec![
                card("Provider"),
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
                card("API keys"),
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
                card("Commands"),
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
            tint: TINT_WEATHER,
            group: Group::Features,
            desc: "Forecast, location and your calendars",
            reset: Some(reset_calendar),
            preview: false,
            items: vec![
                card("Weather"),
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
                card("Location"),
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
                card("Calendar"),
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
            name: "General",
            icon: Icon::Settings,
            tint: TINT_SYSTEM,
            group: Group::System,
            desc: "Start-up, opening, language and clock",
            reset: Some(reset_general),
            preview: false,
            items: vec![
                card("Start-up"),
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
                    "Show tray icon",
                    "Settings stay reachable by right-clicking the notch",
                    toggle!(general.show_tray),
                ),
                card("Opening"),
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
                    "Long-press time",
                    "How long to hold for a long press (the water bell's menu)",
                    slider!(300.0, 1500.0, 50.0, ms, general.long_press_ms),
                ),
                card("Language and clock"),
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
                card("Troubleshooting"),
                it(
                    "Debug logging",
                    "Writes input and camera / microphone events to the log",
                    toggle!(general.debug_log),
                ),
                it(
                    "Diagnostics",
                    "Live input events and camera / microphone detection",
                    Ctl::Button("Open", |s| {
                        s.open(crate::app::PageId::Diagnostics);
                        s.pinned = true;
                        String::new()
                    }),
                ),
            ],
        },
        Section {
            name: "Quiet hours",
            icon: Icon::MoonStar,
            tint: TINT_SHELF,
            group: Group::System,
            desc: "When the notch stays calm",
            reset: Some(reset_quiet),
            preview: false,
            items: vec![
                card("Focus"),
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
                card("Schedule"),
                it("Scheduled quiet hours", "", toggle!(quiet.schedule)),
                it("From", "", slider!(0.0, 1410.0, 30.0, hm, quiet.start_min)),
                it("Until", "", slider!(0.0, 1410.0, 30.0, hm, quiet.end_min)),
                card("Automatic"),
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
            tint: TINT_CONTROL,
            group: Group::System,
            desc: "Global keyboard shortcuts",
            reset: Some(reset_shortcuts),
            preview: false,
            items: vec![
                card("Shortcuts"),
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
            tint: TINT_TODO,
            group: Group::System,
            desc: "What leaves this PC, and your data",
            reset: Some(reset_privacy),
            preview: false,
            items: vec![
                card("Screen"),
                it(
                    "Screen behind the notch",
                    "Glass and Liquid Glass read it on the GPU only: never saved, logged or sent",
                    Ctl::Info(|s| {
                        crate::i18n::tr(if s.glass.tier.captures() {
                            "In use (glass)"
                        } else {
                            "Not in use"
                        })
                    }),
                ),
                card("Network"),
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
                card("Your data"),
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
                    "Crash logs",
                    "Stored locally only",
                    Ctl::Button("Open folder", |_| {
                        crate::platform::shell_open(
                            &crate::util::log_dir().to_string_lossy(),
                        );
                        String::new()
                    }),
                ),
                card("Settings file"),
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
                                    after_bulk_change(s);
                                    "Imported".into()
                                }
                                Err(e) => e,
                            },
                            None => String::new(),
                        }
                    }),
                ),
                it(
                    "Reset all settings",
                    "Everything back to how Notchly started (your data stays)",
                    Ctl::Confirm("Reset all", |s| {
                        s.settings.reset_all();
                        after_bulk_change(s);
                        crate::i18n::tr("All settings reset")
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
            icon: Icon::Info,
            tint: TINT_HOME,
            group: Group::System,
            desc: "Version, updates and credits",
            reset: None,
            preview: false,
            items: vec![
                card("Notchly"),
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
        let source = if google { "google" } else { "outlook" };
        s.f.calendar.events.retain(|e| e.source != source);
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

/// Window-level actions (`Action::Settings(id, sub)` with these ids).
const NAV: u16 = 0xFFFF;
const SEARCH_ID: u16 = 0xFFFE;
const CMD: u16 = 0xFFFD;
const RESULT: u16 = 0xFFFC;
const DROP: u16 = 0xFFFB;
/// CMD subs.
const CMD_RESET: u16 = 0;
const CMD_RESET_OK: u16 = 1;
const CMD_RESET_CANCEL: u16 = 2;
const CMD_UNDO: u16 = 3;
const CMD_BACKDROP: u16 = 10;
const CMD_STATE: u16 = 20;
const CMD_PLAY: u16 = 30;
/// The section reset's inline confirm uses this pseudo item id.
const RESET_SECTION: u16 = 0xFFF0;
/// How many changes Undo remembers.
const UNDO_STEPS: usize = 50;

/// The preview's island states.
const PREVIEW_STATES: [&str; 4] = ["Idle", "Live", "Peek", "Expanded"];

/// One card of the content pane.
struct CardLayout {
    title: Option<&'static str>,
    ids: Vec<u16>,
}

pub struct SettingsWin {
    pub hwnd: windows::Win32::Foundation::HWND,
    pub surface: Surface,
    pub sections: Vec<Section>,
    pub section: usize,
    pub scroll: Spring,
    nav_scroll: Spring,
    pub zones: Vec<Zone>,
    pub cursor: Option<(f32, f32)>,
    pub pressed: Option<Action>,
    pub slider: Option<(u16, Rect)>,
    pub focus: Option<u16>,
    pub fields: HashMap<u16, TextField>,
    pub recording: Option<u16>,
    /// Bottom-centre toast (message, since).
    toast: Option<(String, Instant)>,
    pub dirty: bool,
    pub size: (f32, f32),
    pub dpi: f32,
    preview_w: Spring,
    preview_h: Spring,
    preview_phase: usize,
    preview_at: Instant,
    preview_play: bool,
    backdrop: PreviewBackdrop,
    /// The decoded desktop wallpaper (loaded the first time "Your desktop" is picked).
    desktop: Option<Option<crate::render::image::Pixels>>,
    started: Instant,
    pub keyboard_focus: Option<Action>,
    // ---- Settings 2.0 motion
    pill_y: Spring,
    pill_h: Spring,
    pill_placed: bool,
    switched: Instant,
    knobs: HashMap<u16, Spring>,
    thumbs: HashMap<u16, Spring>,
    /// A slider knob just snapped onto a detent (id, when).
    snap: Option<(u16, Instant)>,
    last_click: Option<(Action, Instant)>,
    /// A held stepper button (action, pressed at, last repeat).
    repeat: Option<(Action, Instant, Instant)>,
    /// An inline confirm that's armed (item id or RESET_SECTION, since).
    confirm: Option<(u16, Instant)>,
    undo: Vec<Settings>,
    /// Settings before a slider drag (one undo step per drag).
    before: Option<Settings>,
    /// A search result was opened: its row flashes (id, since).
    flash: Option<(u16, Instant)>,
    /// Scroll this row into view on the next frame.
    scroll_to: Option<u16>,
    saved_at: Option<Instant>,
    was_due: bool,
    saved_pop: Spring,
    /// The open dropdown (item id, its button).
    dropdown: Option<(u16, Rect)>,
    /// A stepper repeat waiting for `&mut State`.
    pending_repeat: Option<Action>,
}

impl SettingsWin {
    pub fn new(hwnd: windows::Win32::Foundation::HWND, w: u32, h: u32, dpi: f32) -> Self {
        Self {
            hwnd,
            surface: Surface::new(hwnd, w, h, dpi),
            sections: sections(),
            section: 0,
            scroll: Spring::new(0.0, crate::motion::SNAPPY),
            nav_scroll: Spring::new(0.0, crate::motion::SNAPPY),
            zones: Vec::new(),
            cursor: None,
            pressed: None,
            slider: None,
            focus: None,
            fields: HashMap::new(),
            recording: None,
            toast: None,
            dirty: true,
            size: (w as f32 * 96.0 / dpi, h as f32 * 96.0 / dpi),
            dpi,
            preview_w: Spring::new(SHAPE_IDLE.w, ISLAND),
            preview_h: Spring::new(SHAPE_IDLE.h, ISLAND_H),
            preview_phase: 0,
            preview_at: Instant::now(),
            preview_play: true,
            backdrop: PreviewBackdrop::Busy,
            desktop: None,
            started: Instant::now(),
            keyboard_focus: None,
            pill_y: Spring::new(0.0, SNAPPY),
            pill_h: Spring::new(SET_NAV_ROW_H, GENTLE),
            pill_placed: false,
            switched: Instant::now(),
            knobs: HashMap::new(),
            thumbs: HashMap::new(),
            snap: None,
            last_click: None,
            repeat: None,
            confirm: None,
            undo: Vec::new(),
            before: None,
            flash: None,
            scroll_to: None,
            saved_at: None,
            was_due: false,
            saved_pop: Spring::unit(0.0, BOUNCY),
            dropdown: None,
            pending_repeat: None,
        }
    }

    /// QA (debug builds): types into the search box.
    pub fn qa_search(&mut self, q: &str) {
        self.fields.entry(SEARCH_ID).or_default().set(q);
        self.dirty = true;
    }

    /// Shows a result at the bottom of the window.
    pub fn show_toast(&mut self, m: String) {
        self.toast = Some((m, Instant::now()));
        self.dirty = true;
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
            .map(|f| f.text.trim().to_lowercase())
            .unwrap_or_default()
    }

    fn shown(&self, st: &State, id: u16) -> bool {
        self.item(id)
            .is_some_and(|i| !matches!(i.ctl, Ctl::Card) && i.show.is_none_or(|f| f(st)))
    }

    /// The card an item sits in.
    fn card_of(&self, id: u16) -> Option<&'static str> {
        let s = self.sections.get(id as usize / 100)?;
        s.items[..=(id as usize % 100).min(s.items.len().saturating_sub(1))]
            .iter()
            .rev()
            .find(|i| matches!(i.ctl, Ctl::Card))
            .map(|i| i.label)
    }

    /// The current section's cards with their visible rows.
    fn cards(&self, st: &State) -> Vec<CardLayout> {
        let mut out: Vec<CardLayout> = Vec::new();
        for (i, it) in self.sections[self.section].items.iter().enumerate() {
            if matches!(it.ctl, Ctl::Card) {
                out.push(CardLayout {
                    title: Some(it.label),
                    ids: Vec::new(),
                });
                continue;
            }
            let id = Self::id(self.section, i);
            if !self.shown(st, id) {
                continue;
            }
            if out.is_empty() {
                out.push(CardLayout {
                    title: None,
                    ids: Vec::new(),
                });
            }
            if let Some(c) = out.last_mut() {
                c.ids.push(id);
            }
        }
        out.retain(|c| !c.ids.is_empty());
        out
    }

    /// Search results across every section, best first: labels match fuzzily (English
    /// and the UI language), descriptions and section names by substring.
    fn results(&self, st: &State) -> Vec<u16> {
        let q = self.query();
        let mut v: Vec<(i32, u16)> = Vec::new();
        for (si, s) in self.sections.iter().enumerate() {
            for (ii, it) in s.items.iter().enumerate() {
                let id = Self::id(si, ii);
                if !self.shown(st, id) {
                    continue;
                }
                let label = crate::ui::ask::fuzzy(&q, it.label)
                    .max(crate::ui::ask::fuzzy(&q, &crate::i18n::tr(it.label)));
                let hint = (!it.hint.is_empty()
                    && (it.hint.to_lowercase().contains(&q)
                        || crate::i18n::tr(it.hint).to_lowercase().contains(&q)))
                .then_some(5);
                let sec = s.name.to_lowercase().contains(&q).then_some(1);
                if let Some(score) = label.map(|l| l + 100).or(hint).or(sec) {
                    v.push((score, id));
                }
            }
        }
        v.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        v.into_iter().map(|(_, id)| id).take(40).collect()
    }

    pub fn step(&mut self, dt: f32, st: &State) -> bool {
        let now = Instant::now();
        let sec = &self.sections[self.section];
        let previewing = sec.preview && self.query().is_empty();
        // the preview cycles Idle → Live → Peek → Expanded with the user's motion settings
        if previewing && self.preview_play && self.preview_at.elapsed().as_secs_f32() > 2.2 {
            self.preview_at = now;
            self.set_preview_state((self.preview_phase + 1) % PREVIEW_STATES.len());
        }
        let env = st.env;
        let mut m = self.preview_w.step(dt, &env) | self.preview_h.step(dt, &env);
        m |= self.scroll.step(dt, &env) | self.nav_scroll.step(dt, &env);
        m |= self.pill_y.step(dt, &env) | self.pill_h.step(dt, &env);
        m |= self.saved_pop.step(dt, &env);
        for s in self.knobs.values_mut().chain(self.thumbs.values_mut()) {
            m |= s.step(dt, &env);
        }
        m |= self.focus.is_some();
        // the "Saved" check pops in after each write, then fades
        let due = st.settings_save_due.is_some();
        if self.was_due && !due {
            self.saved_at = Some(now);
            self.saved_pop.snap(0.6);
            self.saved_pop.set(1.0);
        }
        self.was_due = due;
        let ms = |t: Option<Instant>| t.map(|t| t.elapsed().as_secs_f32() * 1000.0);
        if ms(self.saved_at).is_some_and(|e| e < SAVED_HOLD_MS + SET_FADE_MS) {
            m = true;
        }
        if ms(self.toast.as_ref().map(|t| t.1)).is_some_and(|e| e < TOAST_MS + SET_FADE_MS) {
            m = true;
        }
        if ms(self.flash.map(|f| f.1)).is_some_and(|e| e < SEARCH_FLASH_MS) {
            m = true;
        }
        if ms(self.snap.map(|s| s.1)).is_some_and(|e| e < SNAP_MS) {
            m = true;
        }
        // an armed confirm disarms itself
        if let Some((_, t)) = self.confirm {
            if t.elapsed().as_secs_f32() * 1000.0 > CONFIRM_MS {
                self.confirm = None;
            }
            m = true;
        }
        // section switch: cards rise in one after another
        let cards = self.sections[self.section].items.len() as f32;
        if self.switched.elapsed().as_secs_f32() * 1000.0
            < SET_FADE_OUT_MS + cards * crate::motion::choreo::STAGGER_MS + SET_RISE_MS
        {
            m = true;
        }
        if previewing && self.backdrop == PreviewBackdrop::Moving && !env.reduced {
            m = true;
        }
        // a held stepper button repeats
        if let Some((a, at, last)) = self.repeat.clone() {
            if at.elapsed().as_secs_f32() * 1000.0 > STEPPER_DELAY_MS
                && last.elapsed().as_secs_f32() > 1.0 / STEPPER_RATE
            {
                self.repeat = Some((a.clone(), at, now));
                self.step_stepper_soon(a);
            }
            m = true;
        }
        m
    }

    /// Stepper repeats are applied on the next input pass (they need `&mut State`).
    fn step_stepper_soon(&mut self, a: Action) {
        self.pending_repeat = Some(a);
    }

    /// Applies a pending stepper repeat (called by the window loop with `&mut State`).
    pub fn apply_pending(&mut self, st: &mut State) {
        if let Some(Action::Settings(id, sub)) = self.pending_repeat.take() {
            self.stepper(st, id, sub);
            self.dirty = true;
        }
    }

    fn set_preview_state(&mut self, i: usize) {
        self.preview_phase = i;
        let s = match i {
            0 => SHAPE_IDLE,
            1 => SHAPE_LIVE,
            2 => SHAPE_PEEK,
            _ => ShapeSpec {
                w: 300.0,
                h: 96.0,
                ..SHAPE_EXPANDED
            },
        };
        self.preview_w.set(s.w);
        self.preview_h.set(s.h);
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
                cv.theme.surface.with_alpha(SET_MICA_ALPHA)
            } else {
                cv.theme.surface
            },
        );
        self.draw_sidebar(st, &mut cv, h);

        // ---- content
        let searching = !self.query().is_empty();
        let x0 = SET_SIDEBAR_W + SET_GUTTER;
        let content = Rect::new(x0, 0.0, (w - x0 - SET_GUTTER).max(200.0), h);
        let header = Rect::new(content.x, SET_GUTTER * 0.5, content.w, SET_HEADER_H);
        self.draw_header(st, &mut cv, header, searching);
        let mut top = header.bottom() + 8.0;
        if self.sections[self.section].preview && !searching {
            let pr = Rect::new(content.x, top, content.w, SET_PREVIEW_H);
            self.preview(st, &mut cv, pr);
            top = pr.bottom() + SET_CARD_GAP;
        }
        let list = Rect::new(content.x - 6.0, top, content.w + 12.0, h - top);
        cv.push_clip(list);
        let inner = Rect::new(content.x, top, content.w, h - top);
        let content_h = if searching {
            self.draw_results(st, &mut cv, inner)
        } else {
            self.draw_cards(st, &mut cv, inner)
        };
        cv.pop_clip();
        // soft fade where the list scrolls under the header
        if self.scroll.value > 1.0 {
            if let Some(g) = cv.gradient(
                crate::render::canvas::v2(0.0, top),
                crate::render::canvas::v2(0.0, top + 14.0),
                &[
                    (
                        0.0,
                        cv.theme.surface.with_alpha(if mica { 0.6 } else { 1.0 }),
                    ),
                    (1.0, cv.theme.surface.with_alpha(0.0)),
                ],
                1.0,
            ) {
                use windows::core::Interface;
                cv.rrect_brush(
                    Rect::new(content.x - 6.0, top, content.w + 12.0, 14.0),
                    0.0,
                    &g.cast().expect("brush"),
                );
            }
        }
        let max = (content_h - list.h + SET_GUTTER).max(0.0);
        if self.scroll.target > max {
            self.scroll.set(max);
        }
        self.draw_dropdown(st, &mut cv);
        self.draw_toast(&mut cv, w, h);
        self.zones = std::mem::take(&mut cv.zones);
        drop(cv);
        gfx.end(&mut self.surface);
        self.dirty = false;
    }

    fn draw_sidebar(&mut self, st: &State, cv: &mut Canvas, h: f32) {
        let side = Rect::new(0.0, 0.0, SET_SIDEBAR_W, h);
        cv.rect(side, NOTCH.with_alpha(SET_SIDEBAR_ALPHA));
        cv.rect(Rect::new(side.right() - 1.0, 0.0, 1.0, h), LINE);
        // app mark: a little notch on an accent squircle
        let mark = Rect::new(18.0, 18.0, 26.0, 26.0);
        if let Some(b) = cv.accent_brush(mark.x, mark.right(), 1.0) {
            use windows::core::Interface;
            cv.rrect_brush(mark, 8.0, &b.cast().expect("brush"));
        }
        cv.rrect(
            Rect::centered(mark.cx(), mark.y + 7.0, 14.0, 5.0),
            2.5,
            NOTCH,
        );
        cv.text(
            "Notchly",
            &text::SET_CARD.size(SET_APP_TITLE),
            Rect::new(mark.right() + 10.0, mark.y, 140.0, mark.h),
            cv.theme.ink,
            Align::Left,
        );
        let sr = Rect::new(14.0, 58.0, side.w - 28.0, 32.0);
        self.field(cv, SEARCH_ID, sr, "Search settings", Some(Icon::Search));
        let searching = !self.query().is_empty();

        // ---- grouped sections (scrolls when the window is short)
        let nav = Rect::new(
            0.0,
            sr.bottom() + 10.0,
            side.w,
            h - sr.bottom() - 10.0 - 44.0,
        );
        cv.push_clip(nav);
        let mut y = nav.y + 4.0 - self.nav_scroll.value;
        let mut rows: Vec<(usize, Rect)> = Vec::new();
        for g in Group::ALL {
            cv.text(
                &crate::i18n::tr(g.name()),
                &text::SET_GROUP,
                Rect::new(22.0, y + 8.0, side.w - 40.0, 14.0),
                FAINT,
                Align::Left,
            );
            y += SET_GROUP_H;
            for (i, _) in self
                .sections
                .iter()
                .enumerate()
                .filter(|(_, s)| s.group == g)
            {
                rows.push((i, Rect::new(10.0, y, side.w - 20.0, SET_NAV_ROW_H)));
                y += SET_NAV_ROW_H + 2.0;
            }
            y += 6.0;
        }
        let nav_h = y + self.nav_scroll.value - nav.y;
        let max = (nav_h - nav.h).max(0.0);
        if self.nav_scroll.target > max {
            self.nav_scroll.set(max);
        }
        // the selected pill glides to its row (position snappy, height gentle, a little
        // stretch while it travels)
        if let Some((_, r)) = rows.iter().find(|(i, _)| *i == self.section) {
            if !self.pill_placed {
                self.pill_y.snap(r.y + self.nav_scroll.value);
                self.pill_placed = true;
            }
            self.pill_y.set(r.y + self.nav_scroll.value);
            let stretch = (self.pill_y.velocity.abs() * SET_PILL_STRETCH).min(SET_PILL_STRETCH_MAX);
            self.pill_h.set(r.h + stretch);
            if !searching {
                let ph = self.pill_h.value;
                let pr = Rect::new(
                    r.x,
                    self.pill_y.value - self.nav_scroll.value + (r.h - ph) / 2.0,
                    r.w,
                    ph,
                );
                if let Some(b) = cv.accent_brush(pr.x, pr.right(), SET_PILL_ALPHA) {
                    use windows::core::Interface;
                    cv.rrect_brush(pr, 10.0, &b.cast().expect("brush"));
                }
            }
        }
        for (i, r) in rows {
            let s = &self.sections[i];
            let active = i == self.section && !searching;
            if !active && cv.hovered(r) && cv.hovered(nav) {
                cv.rrect(r, 10.0, cv.theme.ink.with_alpha(SET_HOVER_LIFT));
            }
            // icon in its feature tint inside a small glass squircle
            let ib = Rect::centered(r.x + 21.0, r.cy(), SET_NAV_ICON_BOX, SET_NAV_ICON_BOX);
            cv.rrect(ib, 8.0, s.tint.with_alpha(if active { 0.22 } else { 0.14 }));
            cv.stroke_rrect(ib.inset(0.5), 7.5, WHITE.with_alpha(0.08), 1.0);
            cv.icon(s.icon, ib.cx(), ib.cy(), SET_NAV_ICON, s.tint);
            cv.text(
                &crate::i18n::tr(s.name),
                if active { &text::BODY_MED } else { &text::BODY },
                Rect::new(r.x + 42.0, r.y, r.w - 46.0, r.h),
                cv.theme.ink,
                Align::Left,
            );
            cv.zone_named(r, Action::Settings(NAV, i as u16), true, s.name, 10.0);
        }
        cv.pop_clip();

        // ---- "Saved" check
        if let Some(t) = self.saved_at {
            let e = t.elapsed().as_secs_f32() * 1000.0;
            let a = 1.0 - ((e - SAVED_HOLD_MS) / SET_FADE_MS).clamp(0.0, 1.0);
            if a > 0.0 {
                let s = self.saved_pop.value.max(0.0);
                let c = Rect::new(18.0, h - 34.0, 120.0, 20.0);
                cv.push_alpha(a);
                cv.push_tf(0.0, 0.0, s, c.x + 9.0, c.cy());
                cv.circle(c.x + 9.0, c.cy(), 9.0, GOOD.with_alpha(0.18));
                cv.icon(Icon::Check, c.x + 9.0, c.cy(), 11.0, GOOD);
                cv.pop_tf();
                cv.text(
                    &crate::i18n::tr("Saved"),
                    &text::CAPTION.weight(600),
                    Rect::new(c.x + 24.0, c.y, 90.0, c.h),
                    cv.theme.mist,
                    Align::Left,
                );
                cv.pop_alpha();
            }
        }
        let _ = st;
    }

    fn draw_header(&mut self, st: &State, cv: &mut Canvas, r: Rect, searching: bool) {
        let sec = &self.sections[self.section];
        let (title, desc) = if searching {
            (crate::i18n::tr("Search results"), String::new())
        } else {
            (crate::i18n::tr(sec.name), crate::i18n::tr(sec.desc))
        };
        // buttons on the right first, so the title knows its room
        let by = r.y + 18.0;
        let mut right = r.right();
        if !searching {
            if sec.reset.is_some() {
                let armed = self.confirm.is_some_and(|c| c.0 == RESET_SECTION);
                if armed {
                    // the button morphs into "Reset · Cancel", sliding in from the right
                    let e = self
                        .confirm
                        .map(|c| c.1.elapsed().as_secs_f32())
                        .unwrap_or(1.0);
                    let k = if st.env.reduced {
                        1.0
                    } else {
                        ease_out(e / 0.18)
                    };
                    let dx = (1.0 - k) * 16.0;
                    cv.push_alpha(k);
                    let cw = cv.pill_width("Cancel", 76.0);
                    let cr = Rect::new(right - cw + dx, by, cw, 28.0);
                    cv.pill_button("Cancel", cr, Action::Settings(CMD, CMD_RESET_CANCEL), false);
                    let rw = cv.pill_width("Reset", 72.0);
                    let rr = Rect::new(cr.x - 8.0 - rw, by, rw, 28.0);
                    danger_button(cv, "Reset", rr, Action::Settings(CMD, CMD_RESET_OK));
                    cv.pop_alpha();
                    right = rr.x - 10.0;
                } else {
                    let rw = cv.pill_width("Reset section", 110.0);
                    let rr = Rect::new(right - rw, by, rw, 28.0);
                    cv.pill_button("Reset section", rr, Action::Settings(CMD, CMD_RESET), false);
                    right = rr.x - 10.0;
                }
            }
            // Undo (Ctrl+Z), enabled after a change
            let ur = Rect::new(right - 32.0, by - 2.0, 32.0, 32.0);
            let can = !self.undo.is_empty();
            cv.push_alpha(if can { 1.0 } else { DISABLED_ALPHA });
            let hv = can && cv.hovered(ur);
            cv.circle(
                ur.cx(),
                ur.cy(),
                15.0,
                LINE.with_alpha(if hv { 1.8 } else { 1.0 }),
            );
            cv.icon(Icon::Undo, ur.cx(), ur.cy(), 15.0, cv.theme.ink);
            cv.pop_alpha();
            if can {
                cv.zone_named(ur, Action::Settings(CMD, CMD_UNDO), true, "Undo", 16.0);
            }
            right = ur.x - 12.0;
        }
        cv.text(
            &title,
            &text::SET_TITLE,
            Rect::new(r.x, r.y + 6.0, right - r.x, 32.0),
            cv.theme.ink,
            Align::Left,
        );
        if !desc.is_empty() {
            cv.text(
                &desc,
                &text::SET_DESC,
                Rect::new(r.x, r.y + 38.0, right - r.x, 18.0),
                cv.theme.mist,
                Align::Left,
            );
        }
    }

    /// A card's appearance during a section switch (alpha, rise).
    fn card_in(&self, st: &State, i: usize) -> (f32, f32) {
        let e = self.switched.elapsed().as_secs_f32() * 1000.0
            - SET_FADE_OUT_MS
            - i as f32 * crate::motion::choreo::STAGGER_MS;
        let k = ease_out(e / SET_RISE_MS);
        if st.env.reduced {
            (k, 0.0)
        } else {
            (k, (1.0 - k) * SET_RISE)
        }
    }

    fn draw_cards(&mut self, st: &State, cv: &mut Canvas, area: Rect) -> f32 {
        let cards = self.cards(st);
        let mut y = area.y - self.scroll.value;
        for (ci, card) in cards.iter().enumerate() {
            let title_h = if card.title.is_some() {
                SET_CARD_TITLE_H
            } else {
                6.0
            };
            let rows_h: f32 = card.ids.iter().map(|id| self.row_height(*id)).sum();
            let r = Rect::new(area.x, y, area.w, title_h + rows_h + 6.0);
            y = r.bottom() + SET_CARD_GAP;
            if let Some(target) = self.scroll_to {
                if let Some(pos) = card.ids.iter().position(|i| *i == target) {
                    let row_y = r.y
                        + title_h
                        + card.ids[..pos]
                            .iter()
                            .map(|id| self.row_height(*id))
                            .sum::<f32>();
                    let want = self.scroll.target + (row_y - area.y) - 40.0;
                    self.scroll.set(want.max(0.0));
                    self.scroll_to = None;
                }
            }
            if !cv.visible(r) {
                continue;
            }
            let (a, dy) = self.card_in(st, ci);
            if a <= 0.0 {
                continue;
            }
            cv.push_alpha(a);
            cv.push_tf(0.0, dy, 1.0, r.cx(), r.cy());
            cv.sq(
                r,
                SET_CARD_R,
                cv.theme.surface_raised.with_alpha(SET_CARD_ALPHA),
            );
            cv.stroke_rrect(r.inset(0.5), SET_CARD_R - 0.5, LINE, 1.0);
            if let Some(t) = card.title {
                cv.text(
                    &crate::i18n::tr(t),
                    &text::SET_CARD,
                    Rect::new(
                        r.x + SET_CARD_PAD,
                        r.y + 12.0,
                        r.w - 2.0 * SET_CARD_PAD,
                        22.0,
                    ),
                    cv.theme.ink,
                    Align::Left,
                );
            }
            let mut ry = r.y + title_h;
            for (k, id) in card.ids.iter().enumerate() {
                let rh = self.row_height(*id);
                let row = Rect::new(r.x, ry, r.w, rh);
                if k > 0 {
                    cv.rect(
                        Rect::new(r.x + SET_CARD_PAD, ry, r.w - 2.0 * SET_CARD_PAD, 1.0),
                        LINE,
                    );
                }
                if cv.visible(row) {
                    self.draw_item(st, cv, *id, row);
                    self.draw_flash(cv, *id, row);
                }
                ry += rh;
            }
            cv.pop_tf();
            cv.pop_alpha();
        }
        y + self.scroll.value - area.y
    }

    fn draw_flash(&self, cv: &mut Canvas, id: u16, row: Rect) {
        if let Some((fid, t)) = self.flash {
            let e = t.elapsed().as_secs_f32() * 1000.0;
            if fid == id && e < SEARCH_FLASH_MS {
                let a = 1.0 - e / SEARCH_FLASH_MS;
                cv.stroke_rrect(
                    row.inset_xy(6.0, 3.0),
                    12.0,
                    cv.theme.accent[0].with_alpha(a),
                    2.0,
                );
                cv.rrect(
                    row.inset_xy(6.0, 3.0),
                    12.0,
                    cv.theme.accent[0].with_alpha(0.08 * a),
                );
            }
        }
    }

    fn draw_results(&mut self, st: &State, cv: &mut Canvas, area: Rect) -> f32 {
        let ids = self.results(st);
        let y0 = area.y - self.scroll.value;
        if ids.is_empty() {
            cv.text(
                &crate::i18n::tr("No settings match"),
                &text::BODY,
                Rect::new(area.x, y0 + 8.0, area.w, 20.0),
                cv.theme.mist,
                Align::Left,
            );
            return 40.0;
        }
        let count = format!("{} {}", ids.len(), crate::i18n::tr("results"));
        let card = Rect::new(
            area.x,
            y0,
            area.w,
            SET_CARD_TITLE_H + ids.len() as f32 * SET_ROW_H + 6.0,
        );
        cv.sq(
            card,
            SET_CARD_R,
            cv.theme.surface_raised.with_alpha(SET_CARD_ALPHA),
        );
        cv.stroke_rrect(card.inset(0.5), SET_CARD_R - 0.5, LINE, 1.0);
        cv.text(
            &count,
            &text::SET_CARD,
            Rect::new(card.x + SET_CARD_PAD, card.y + 12.0, card.w, 22.0),
            cv.theme.ink,
            Align::Left,
        );
        for (k, id) in ids.iter().enumerate() {
            let r = Rect::new(
                card.x,
                card.y + SET_CARD_TITLE_H + k as f32 * SET_ROW_H,
                card.w,
                SET_ROW_H,
            );
            if !cv.visible(r) {
                continue;
            }
            if k > 0 {
                cv.rect(
                    Rect::new(r.x + SET_CARD_PAD, r.y, r.w - 2.0 * SET_CARD_PAD, 1.0),
                    LINE,
                );
            }
            let Some(item) = self.item(*id).copied() else {
                continue;
            };
            let sec = &self.sections[*id as usize / 100];
            let crumb = match self.card_of(*id) {
                Some(c) => format!("{} › {}", crate::i18n::tr(sec.name), crate::i18n::tr(c)),
                None => crate::i18n::tr(sec.name),
            };
            let a = Action::Settings(RESULT, *id);
            if cv.hovered(r) || self.keyboard_focus.as_ref() == Some(&a) {
                cv.rrect(
                    r.inset_xy(6.0, 3.0),
                    12.0,
                    cv.theme.ink.with_alpha(SET_HOVER_LIFT),
                );
            }
            let ib = Rect::centered(r.x + SET_CARD_PAD + 13.0, r.cy(), 26.0, 26.0);
            cv.rrect(ib, 8.0, sec.tint.with_alpha(0.14));
            cv.icon(sec.icon, ib.cx(), ib.cy(), 15.0, sec.tint);
            let tx = ib.right() + 12.0;
            cv.text(
                &crate::i18n::tr(item.label),
                &text::SET_LABEL,
                Rect::new(tx, r.y + 8.0, r.w - tx + r.x - 40.0, 18.0),
                cv.theme.ink,
                Align::Left,
            );
            cv.text(
                &crumb,
                &text::SET_DESC,
                Rect::new(tx, r.y + 27.0, r.w - tx + r.x - 40.0, 16.0),
                cv.theme.mist,
                Align::Left,
            );
            cv.icon(Icon::ChevronRight, r.right() - 24.0, r.cy(), 13.0, FAINT);
            cv.zone_named(r.inset_xy(6.0, 3.0), a, true, item.label, 12.0);
        }
        card.bottom() + self.scroll.value - area.y
    }

    fn draw_dropdown(&mut self, st: &State, cv: &mut Canvas) {
        let Some((id, b)) = self.dropdown else { return };
        let Some(Ctl::Choice(options, get, _)) = self.item(id).map(|i| i.ctl) else {
            self.dropdown = None;
            return;
        };
        let o = options(st);
        let cur = get(st);
        let rh = 30.0;
        let h = o.len() as f32 * rh + 8.0;
        let below = b.bottom() + 6.0 + h < self.size.1 - 8.0;
        let r = Rect::new(
            b.x,
            if below {
                b.bottom() + 6.0
            } else {
                b.y - 6.0 - h
            },
            b.w.max(180.0),
            h,
        );
        cv.sq(r.inset(-1.0), 13.0, NOTCH.with_alpha(0.35));
        cv.sq(r, 12.0, cv.theme.surface_raised);
        cv.stroke_rrect(r.inset(0.5), 11.5, LINE.with_alpha(1.6), 1.0);
        for (i, s) in o.iter().enumerate() {
            let rr = Rect::new(r.x + 4.0, r.y + 4.0 + i as f32 * rh, r.w - 8.0, rh);
            let a = Action::Settings(DROP, i as u16);
            if cv.hovered(rr) || self.keyboard_focus.as_ref() == Some(&a) {
                cv.rrect(rr, 8.0, cv.theme.ink.with_alpha(0.08));
            }
            if i == cur {
                cv.icon(Icon::Check, rr.x + 14.0, rr.cy(), 12.0, cv.theme.accent[0]);
            }
            cv.text(
                s,
                &text::BODY,
                Rect::new(rr.x + 28.0, rr.y, rr.w - 34.0, rr.h),
                cv.theme.ink,
                Align::Left,
            );
            cv.zone_named(rr, a, true, s, 8.0);
        }
    }

    fn draw_toast(&self, cv: &mut Canvas, w: f32, h: f32) {
        let Some((m, t)) = &self.toast else { return };
        let e = t.elapsed().as_secs_f32() * 1000.0;
        if m.is_empty() || e > TOAST_MS + SET_FADE_MS {
            return;
        }
        let k_in = ease_out(e / SET_FADE_MS);
        let k_out = 1.0 - ((e - TOAST_MS) / SET_FADE_MS).clamp(0.0, 1.0);
        let a = k_in.min(k_out);
        let tw = cv.measure(m, &text::BODY_MED) + 40.0;
        let cx = SET_SIDEBAR_W + (w - SET_SIDEBAR_W) / 2.0;
        let r = Rect::new(
            cx - tw / 2.0,
            h - 52.0 + (1.0 - k_in) * TOAST_RISE,
            tw,
            34.0,
        );
        cv.push_alpha(a);
        cv.sq(r.inset(-1.0), 18.0, NOTCH.with_alpha(0.3));
        cv.sq(r, 17.0, cv.theme.surface_raised);
        cv.stroke_rrect(r.inset(0.5), 16.5, WHITE.with_alpha(0.1), 1.0);
        cv.text(m, &text::BODY_MED, r, cv.theme.ink, Align::Center);
        cv.pop_alpha();
    }

    fn row_height(&self, id: u16) -> f32 {
        match self.item(id).map(|i| &i.ctl) {
            Some(Ctl::List(..)) => 104.0,
            Some(Ctl::Chips(..)) => 100.0,
            Some(Ctl::Info(_)) => 60.0,
            _ => SET_ROW_H,
        }
    }

    fn preview(&mut self, st: &State, cv: &mut Canvas, r: Rect) {
        use crate::render::glass::{compositor, lens::Body, Backdrop, Tier};
        let material = st.settings.appearance.material;
        let glassy = material != Material::Solid;
        let scale = self.dpi / 96.0;
        if self.backdrop == PreviewBackdrop::Desktop && self.desktop.is_none() {
            // decoded once, the first time it's picked (never saved anywhere)
            self.desktop = Some(
                crate::platform::wallpaper_path()
                    .and_then(|p| crate::render::image::decode_file(&p, 1600).ok()),
            );
        }
        let t = if st.env.reduced {
            0.0
        } else {
            self.started.elapsed().as_secs_f32() % PREVIEW_MOVING_SECS
        };
        let (pw, ph) = (
            (r.right() * scale).ceil() as u32,
            (r.bottom() * scale).ceil() as u32,
        );
        let wall = compositor::preview_backdrop(
            cv,
            pw,
            ph,
            self.backdrop,
            t,
            self.desktop.as_ref().and_then(|d| d.as_ref()),
        );
        if let Some(b) = &wall {
            if let Ok(g) = shapes::squircle(&cv.gfx.factory.clone(), r, SET_CARD_R) {
                use windows::core::Interface;
                if let Ok(g) = g.cast() {
                    cv.push_geo_clip_any(&g);
                    unsafe {
                        cv.ctx.DrawBitmap(
                            b,
                            Some(&windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F {
                                left: 0.0,
                                top: 0.0,
                                right: pw as f32 / scale,
                                bottom: ph as f32 / scale,
                            }),
                            1.0,
                            windows::Win32::Graphics::Direct2D::D2D1_INTERPOLATION_MODE_LINEAR,
                            None,
                            None,
                        );
                    }
                    cv.pop_layer();
                }
            }
        } else {
            cv.sq(r, SET_CARD_R, NOTCH.lerp(SURFACE_RAISED, 0.35));
        }
        cv.stroke_rrect(r.inset(0.5), SET_CARD_R - 0.5, LINE, 1.0);
        let size = match st.settings.appearance.size {
            115 => 1.15,
            130 => 1.3,
            150 => 1.5,
            _ => 1.0,
        };
        let floating = st.settings.appearance.style == Style::Floating;
        let (iw, ih) = (self.preview_w.value * size, self.preview_h.value * size);
        let top = r.y + if floating { 10.0 } else { 0.0 };
        let radius = (ih * 0.45).min(26.0);
        let factory = cv.gfx.factory.clone();
        cv.push_clip(r);
        if let Ok(p) = shapes::notch_path(
            &factory,
            r.cx(),
            top,
            iw,
            ih,
            radius,
            if floating { 0.0 } else { 9.0 },
            floating,
        ) {
            match (&wall, glassy) {
                (Some(b), true) => {
                    use windows::core::Interface;
                    let tier = if material == Material::LiquidGlass {
                        Tier::Liquid
                    } else {
                        Tier::Frosted
                    };
                    let f = crate::app::glass::preview_frame(
                        st,
                        tier,
                        1.0,
                        Some(Backdrop {
                            bitmap: b.clone(),
                            slot: 1,
                            transform: None,
                        }),
                        (r.cx() - iw * 0.25, top + ih * 0.25),
                        floating,
                    );
                    let body = Body {
                        x: r.cx() - iw / 2.0,
                        y: top,
                        w: iw,
                        h: ih,
                        radius,
                        open_top: !floating,
                    };
                    if let Ok(g) = p.cast() {
                        compositor::surface(cv, &g, &[body], &f, floating);
                    }
                }
                _ => {
                    cv.fill_geo(&p, NOTCH);
                    if floating {
                        cv.stroke_geo(&p, RING, 1.0);
                    }
                }
            }
        }
        // a hint of content per state
        let ink = cv.theme.ink;
        let cy = top + ih / 2.0;
        match self.preview_phase {
            1 => {
                cv.circle(r.cx() - iw / 2.0 + 18.0, cy, 6.0, TINT_MUSIC);
                cv.progress(
                    Rect::new(r.cx() + iw / 2.0 - 60.0, cy - 1.5, 40.0, 3.0),
                    0.62,
                    true,
                );
            }
            2 => {
                cv.rrect(
                    Rect::new(r.cx() - iw / 2.0 + 16.0, cy - 14.0, 28.0, 28.0),
                    8.0,
                    TINT_WATER.with_alpha(0.85),
                );
                cv.rrect(
                    Rect::new(r.cx() - iw / 2.0 + 54.0, cy - 8.0, iw * 0.4, 6.0),
                    3.0,
                    ink.with_alpha(0.85),
                );
                cv.rrect(
                    Rect::new(r.cx() - iw / 2.0 + 54.0, cy + 3.0, iw * 0.28, 5.0),
                    2.5,
                    ink.with_alpha(0.45),
                );
            }
            3 => {
                for i in 0..3 {
                    let x = r.cx() - iw / 2.0 + 18.0 + i as f32 * ((iw - 36.0) / 3.0);
                    cv.rrect(
                        Rect::new(x, cy - 22.0, (iw - 36.0) / 3.0 - 8.0, 44.0),
                        10.0,
                        ink.with_alpha(0.10),
                    );
                }
            }
            _ => {}
        }
        cv.pop_clip();
        // ---- controls along the bottom: backdrop swatches · state segmented · play
        let by = r.bottom() - 38.0;
        let mut x = r.x + 14.0;
        for (i, k) in PreviewBackdrop::ALL.iter().enumerate() {
            let c = Rect::new(x, by + 3.0, 22.0, 22.0);
            let a = Action::Settings(CMD, CMD_BACKDROP + i as u16);
            let hv = cv.hovered(c);
            let s = if hv { SWATCH_HOVER } else { 1.0 };
            cv.push_tf(0.0, 0.0, s, c.cx(), c.cy());
            let sw = k.swatch();
            if let Some(g) = cv.gradient(
                crate::render::canvas::v2(c.x, c.y),
                crate::render::canvas::v2(c.right(), c.bottom()),
                &[(0.0, sw[0]), (1.0, sw[1])],
                1.0,
            ) {
                use windows::core::Interface;
                cv.rrect_brush(c, 11.0, &g.cast().expect("brush"));
            }
            cv.stroke_rrect(c.inset(0.5), 10.5, WHITE.with_alpha(0.35), 1.0);
            if *k == self.backdrop {
                cv.stroke_rrect(c.inset(-3.0), 14.0, WHITE, SWATCH_RING);
            }
            cv.pop_tf();
            cv.zone_named(c.inset(-3.0), a, true, k.name(), 14.0);
            x += 30.0;
        }
        let pw = 36.0;
        let play = Rect::new(r.right() - 14.0 - pw + 4.0, by, 28.0, 28.0);
        let sw = (r.w - (x - r.x) - pw - 40.0).clamp(120.0, 300.0);
        let seg = Rect::new(play.x - 10.0 - sw, by, sw, 28.0);
        cv.sq(seg, 14.0, NOTCH.with_alpha(0.45));
        let n = PREVIEW_STATES.len() as f32;
        let cw = seg.w / n;
        cv.rrect(
            Rect::new(
                seg.x + self.preview_phase as f32 * cw + 2.0,
                seg.y + 2.0,
                cw - 4.0,
                seg.h - 4.0,
            ),
            12.0,
            WHITE.with_alpha(0.18),
        );
        for (i, s) in PREVIEW_STATES.iter().enumerate() {
            let rr = Rect::new(seg.x + i as f32 * cw, seg.y, cw, seg.h);
            cv.text(
                &crate::i18n::tr(s),
                &text::CAPTION.weight(600),
                rr,
                if i == self.preview_phase {
                    WHITE
                } else {
                    WHITE.with_alpha(0.65)
                },
                Align::Center,
            );
            cv.zone_named(
                rr,
                Action::Settings(CMD, CMD_STATE + i as u16),
                true,
                s,
                12.0,
            );
        }
        cv.circle(play.cx(), play.cy(), 14.0, NOTCH.with_alpha(0.45));
        cv.icon(
            if self.preview_play {
                Icon::Pause
            } else {
                Icon::Play
            },
            play.cx(),
            play.cy(),
            12.0,
            WHITE,
        );
        cv.zone_named(
            play,
            Action::Settings(CMD, CMD_PLAY),
            true,
            if self.preview_play { "Pause" } else { "Play" },
            14.0,
        );
        cv.label(
            "Live preview",
            &text::MICRO,
            Rect::new(r.x + 14.0, r.y + 10.0, 200.0, 14.0),
            WHITE.with_alpha(0.7),
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
        let wide = matches!(item.ctl, Ctl::List(..) | Ctl::Chips(..));
        let label_w = if matches!(item.ctl, Ctl::Info(_)) {
            r.w * 0.34
        } else {
            r.w * 0.5 - SET_CARD_PAD
        };
        let lx = r.x + SET_CARD_PAD;
        let (ly, lh) = if wide {
            (r.y + 10.0, 18.0)
        } else if item.hint.is_empty() {
            (r.y, r.h)
        } else {
            (r.y + 8.0, 18.0)
        };
        cv.text(
            &crate::i18n::tr(item.label),
            &text::SET_LABEL,
            Rect::new(lx, ly, label_w, lh),
            cv.theme.ink,
            Align::Left,
        );
        if !item.hint.is_empty() {
            cv.text(
                &crate::i18n::tr(item.hint),
                &text::SET_DESC,
                Rect::new(lx, ly + 19.0, label_w, 16.0),
                cv.theme.mist,
                Align::Left,
            );
        }
        let cr = Rect::new(
            r.x + r.w * 0.5,
            r.y + 8.0,
            r.w * 0.5 - SET_CARD_PAD,
            r.h - 16.0,
        );
        match &item.ctl {
            Ctl::Card => {}
            Ctl::Toggle(get, _) => {
                let on = get(st);
                let t = Rect::new(
                    cr.right() - TOGGLE_W,
                    cr.cy() - TOGGLE_H / 2.0,
                    TOGGLE_W,
                    TOGGLE_H,
                );
                let k = self
                    .knobs
                    .entry(id)
                    .or_insert_with(|| Spring::unit(on as u8 as f32, SNAPPY));
                k.set(on as u8 as f32);
                let v = k.value.clamp(-0.1, 1.1);
                cv.rrect(t, TOGGLE_H / 2.0, TRACK);
                if v > 0.01 {
                    if let Some(b) = cv.accent_brush(t.x, t.right(), v.clamp(0.0, 1.0)) {
                        use windows::core::Interface;
                        cv.rrect_brush(t, TOGGLE_H / 2.0, &b.cast().expect("brush"));
                    }
                }
                // the knob stretches mid-travel, then settles (liquid feel)
                let travel = 1.0 - (2.0 * v.clamp(0.0, 1.0) - 1.0).abs();
                let kw = TOGGLE_KNOB + TOGGLE_STRETCH * travel;
                let pad = (TOGGLE_H - TOGGLE_KNOB) / 2.0;
                let kx = t.x + pad + (t.w - 2.0 * pad - kw) * v.clamp(0.0, 1.0);
                let kr = Rect::new(kx, t.y + pad, kw, TOGGLE_KNOB);
                cv.rrect(
                    kr.inset(-0.5).translate(0.0, 0.8),
                    TOGGLE_KNOB / 2.0,
                    NOTCH.with_alpha(0.25),
                );
                cv.rrect(kr, TOGGLE_KNOB / 2.0, WHITE);
                cv.zone_named(
                    t.inset(-6.0),
                    Action::Settings(id, 0),
                    true,
                    item.label,
                    TOGGLE_H / 2.0 + 6.0,
                );
            }
            Ctl::Choice(options, get, _) => {
                let o = options(st);
                let cur = get(st).min(o.len().saturating_sub(1));
                let total: usize = o.iter().map(|s| crate::i18n::tr(s).chars().count()).sum();
                if o.len() <= 3 && total < 34 {
                    self.segmented(cv, id, item.label, &o, cur, cr);
                } else {
                    // a dropdown: shows the current value, opens a list
                    let dw = cr.w.min(260.0);
                    let d = Rect::new(cr.right() - dw, cr.cy() - 15.0, dw, 30.0);
                    let open = self.dropdown.is_some_and(|x| x.0 == id);
                    let hv = cv.hovered(d);
                    cv.sq(
                        d,
                        10.0,
                        TRACK.with_alpha(if hv || open { 1.4 } else { 1.0 }),
                    );
                    cv.text(
                        &o.get(cur).map(|s| crate::i18n::tr(s)).unwrap_or_default(),
                        &text::BODY,
                        Rect::new(d.x + 12.0, d.y, d.w - 36.0, d.h),
                        cv.theme.ink,
                        Align::Left,
                    );
                    cv.icon_rot(
                        Icon::ChevronDown,
                        d.right() - 14.0,
                        d.cy(),
                        12.0,
                        cv.theme.mist,
                        if open { std::f32::consts::PI } else { 0.0 },
                    );
                    cv.zone_named(d, Action::Settings(id, 0), true, item.label, 10.0);
                }
            }
            Ctl::Slider {
                min, max, get, fmt, ..
            } => {
                let v = get(st);
                let frac = ((v - min) / (max - min)).clamp(0.0, 1.0);
                let sr = Rect::new(cr.x, cr.y, cr.w - 76.0, cr.h);
                self.slider_ctl(
                    cv,
                    id,
                    item.label,
                    sr,
                    frac,
                    &fmt(v),
                    item.label == STRENGTH_LABEL,
                );
                cv.text(
                    &fmt(v),
                    &text::SET_VALUE,
                    Rect::new(cr.right() - 70.0, cr.y, 70.0, cr.h),
                    cv.theme.mist,
                    Align::Right,
                );
            }
            Ctl::Stepper {
                min, max, get, fmt, ..
            } => {
                let v = get(st);
                let bw = 30.0;
                let vw = 92.0;
                let plus = Rect::new(cr.right() - bw, cr.cy() - bw / 2.0, bw, bw);
                let val = Rect::new(plus.x - vw, cr.y, vw, cr.h);
                let minus = Rect::new(val.x - bw, plus.y, bw, bw);
                for (b, icon, sub, enabled) in [
                    (minus, Icon::Minus, 1u16, v > *min + 1e-3),
                    (plus, Icon::Plus, 2u16, v < *max - 1e-3),
                ] {
                    let a = Action::Settings(id, sub);
                    let hv = enabled && cv.hovered(b);
                    let pressed = enabled && self.pressed.as_ref() == Some(&a);
                    cv.push_alpha(if enabled { 1.0 } else { DISABLED_ALPHA });
                    cv.push_tf(0.0, 0.0, if pressed { 0.92 } else { 1.0 }, b.cx(), b.cy());
                    cv.circle(
                        b.cx(),
                        b.cy(),
                        bw / 2.0,
                        TRACK.with_alpha(if hv { 1.5 } else { 1.0 }),
                    );
                    cv.icon(icon, b.cx(), b.cy(), 13.0, cv.theme.ink);
                    cv.pop_tf();
                    cv.pop_alpha();
                    if enabled {
                        cv.zone_named(b, a, true, if sub == 1 { "Less" } else { "More" }, bw / 2.0);
                    }
                }
                cv.text(&fmt(v), &text::SET_VALUE, val, cv.theme.ink, Align::Center);
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
                self.field(
                    cv,
                    id,
                    Rect::new(cr.x, r.y + 10.0, cr.w, r.h - 20.0),
                    "One per line",
                    None,
                );
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
                    crate::i18n::tr("Press keys…")
                } else {
                    get(st)
                };
                let hw = cv.measure(&label, &text::MONO) + 32.0;
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
                cv.text(&label, &text::MONO, b, cv.theme.ink, Align::Center);
                cv.zone_named(b, Action::Settings(id, 0), true, item.label, 10.0);
            }
            Ctl::Button(label, _) => {
                let bw = cv.pill_width(label, 80.0);
                cv.pill_button(
                    label,
                    Rect::new(cr.right() - bw, cr.cy() - 14.0, bw, 28.0),
                    Action::Settings(id, 0),
                    false,
                );
            }
            Ctl::Confirm(label, _) => {
                let armed = self.confirm.is_some_and(|c| c.0 == id);
                if armed {
                    let cw = cv.pill_width("Cancel", 76.0);
                    let c = Rect::new(cr.right() - cw, cr.cy() - 14.0, cw, 28.0);
                    cv.pill_button("Cancel", c, Action::Settings(id, 2), false);
                    let dw = cv.pill_width(label, 80.0);
                    let d = Rect::new(c.x - 8.0 - dw, c.y, dw, 28.0);
                    danger_button(cv, label, d, Action::Settings(id, 1));
                } else {
                    let bw = cv.pill_width(label, 80.0);
                    cv.pill_button(
                        label,
                        Rect::new(cr.right() - bw, cr.cy() - 14.0, bw, 28.0),
                        Action::Settings(id, 0),
                        false,
                    );
                }
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
                let x = r.x + r.w * 0.36;
                cv.text_top(
                    &t,
                    &text::SET_DESC.lines(3),
                    x,
                    r.y + 10.0,
                    r.right() - SET_CARD_PAD - x,
                    cv.theme.mist,
                    Align::Right,
                );
            }
            Ctl::Chips(options, get, _) => {
                let o = options(st);
                let cur = get(st);
                let row = Rect::new(
                    r.x + SET_CARD_PAD,
                    r.y + 38.0,
                    r.w - 2.0 * SET_CARD_PAD,
                    50.0,
                );
                let n = o.len().max(1) as f32;
                let gap = 8.0;
                let w = (row.w - gap * (n - 1.0)) / n;
                for (i, name) in o.iter().enumerate() {
                    let c = Rect::new(row.x + i as f32 * (w + gap), row.y, w, row.h);
                    let a = Action::Settings(id, i as u16 + 1);
                    let hover = cv.hovered(c);
                    let (hv, pv) = cv.anim(&a, hover, false);
                    cv.push_tf(0.0, 0.0, 1.0 + 0.02 * hv - 0.03 * pv, c.cx(), c.cy());
                    let (s, _, _, rim) = GlassPreset::ALL
                        .get(i)
                        .and_then(|p| p.values())
                        .unwrap_or((0.5, 0.5, 0.5, 0.5));
                    if let Some(g) = cv.gradient(
                        crate::render::canvas::v2(c.x, c.y),
                        crate::render::canvas::v2(c.right(), c.bottom()),
                        &[
                            (0.0, PREVIEW_WALL[0].with_alpha(0.55)),
                            (1.0, PREVIEW_WALL[1].with_alpha(0.55)),
                        ],
                        1.0,
                    ) {
                        use windows::core::Interface;
                        cv.rrect_brush(c, 12.0, &g.cast().expect("brush"));
                    }
                    cv.rrect(c, 12.0, GLASS_TINT.with_alpha(0.15 + 0.8 * s));
                    cv.stroke_rrect(
                        c.inset(0.75),
                        11.25,
                        WHITE.with_alpha(0.12 + 0.45 * rim),
                        1.2,
                    );
                    if i == cur {
                        cv.rrect(c, 12.0, cv.theme.accent[0].with_alpha(0.08));
                        cv.stroke_rrect(c.inset(-2.0), 14.0, cv.theme.accent[0], CHIP_RIM);
                        cv.circle(c.right() - 10.0, c.y + 10.0, 7.0, cv.theme.accent[0]);
                        cv.icon(Icon::Check, c.right() - 10.0, c.y + 10.0, 9.0, WHITE);
                    }
                    cv.text(
                        &crate::i18n::tr(name),
                        &text::CAPTION.weight(600),
                        c,
                        WHITE,
                        Align::Center,
                    );
                    cv.pop_tf();
                    cv.zone_named(c, a, true, name, 12.0);
                }
            }
        }
    }

    /// The segmented control: a raised thumb slides between segments and stretches toward
    /// where it's going.
    fn segmented(
        &mut self,
        cv: &mut Canvas,
        id: u16,
        label: &str,
        o: &[String],
        cur: usize,
        cr: Rect,
    ) {
        let names: Vec<String> = o.iter().map(|s| crate::i18n::tr(s)).collect();
        let natural: f32 = names
            .iter()
            .map(|s| cv.measure(s, &text::CAPTION.weight(600)) + 26.0)
            .fold(0.0, f32::max)
            * names.len() as f32;
        let sw = natural.clamp(120.0, cr.w.min(300.0));
        let seg = Rect::new(cr.right() - sw, cr.cy() - 15.0, sw, 30.0);
        cv.rrect(seg, 15.0, TRACK);
        let n = names.len().max(1) as f32;
        let cw = sw / n;
        let th = self
            .thumbs
            .entry(id)
            .or_insert_with(|| Spring::unit(cur as f32, SNAPPY));
        th.set(cur as f32);
        let (v, vel) = (th.value, th.velocity);
        let stretch = (vel.abs() * 0.02).min(SEG_STRETCH) * cw;
        let x = seg.x + v * cw + 2.0 - if vel < 0.0 { stretch } else { 0.0 };
        let thumb = Rect::new(x, seg.y + 2.0, cw - 4.0 + stretch, seg.h - 4.0);
        cv.rrect(thumb.translate(0.0, 0.8), 13.0, NOTCH.with_alpha(0.22));
        cv.rrect(thumb, 13.0, cv.theme.surface_raised.lerp(INK, 0.16));
        cv.stroke_rrect(thumb.inset(0.5), 12.5, WHITE.with_alpha(0.10), 1.0);
        for (i, s) in names.iter().enumerate() {
            let rr = Rect::new(seg.x + i as f32 * cw, seg.y, cw, seg.h);
            let near = 1.0 - (v - i as f32).abs().min(1.0);
            cv.text(
                s,
                &text::CAPTION.weight(600),
                rr,
                cv.theme.mist.lerp(cv.theme.ink, near),
                Align::Center,
            );
            cv.zone_named(
                rr,
                Action::Settings(id, i as u16 + 1),
                true,
                &format!("{label}: {s}"),
                13.0,
            );
        }
    }

    /// The slider: track (thicker on hover), accent fill, a knob with a rim light, a value
    /// bubble while dragging and a little pop when it snaps onto a detent.
    #[allow(clippy::too_many_arguments)]
    fn slider_ctl(
        &self,
        cv: &mut Canvas,
        id: u16,
        label: &str,
        r: Rect,
        frac: f32,
        value: &str,
        detents: bool,
    ) {
        let a = Action::Slider(SliderKind::SettingsSlider(id));
        let dragging = self.slider.is_some_and(|s| s.0 == id);
        let hover = dragging || cv.hovered(r.inset_xy(-4.0, -6.0));
        let th = if hover {
            SLIDER_TRACK_HOVER
        } else {
            SLIDER_TRACK
        };
        let track = Rect::new(r.x, r.cy() - th / 2.0, r.w, th);
        cv.rrect(track, th / 2.0, TRACK);
        let x = r.x + r.w * frac;
        if frac > 0.0 {
            if let Some(b) = cv.accent_brush(r.x, r.right(), 1.0) {
                use windows::core::Interface;
                cv.rrect_brush(
                    Rect::new(r.x, track.y, (x - r.x).max(th), th),
                    th / 2.0,
                    &b.cast().expect("brush"),
                );
            }
        }
        if detents {
            for d in DETENTS {
                cv.circle(
                    r.x + r.w * d,
                    r.cy() + 10.0,
                    1.6,
                    cv.theme.mist.with_alpha(0.7),
                );
            }
        }
        let pop = self
            .snap
            .filter(|s| s.0 == id)
            .map(|s| {
                let k = (s.1.elapsed().as_secs_f32() * 1000.0 / SNAP_MS).min(1.0);
                1.0 + 0.15 * (1.0 - k) * (1.0 - k)
            })
            .unwrap_or(1.0);
        let kr = SLIDER_KNOB / 2.0 * pop * if dragging { 1.06 } else { 1.0 };
        cv.circle(x, r.cy() + 1.0, kr + 0.5, NOTCH.with_alpha(0.3));
        cv.circle(x, r.cy(), kr, WHITE);
        cv.stroke_circle(
            x,
            r.cy(),
            kr - 0.6,
            cv.theme.accent[0].with_alpha(0.35),
            1.2,
        );
        if dragging {
            let tw = cv.measure(value, &text::CAPTION_TNUM) + 16.0;
            let b = Rect::new(x - tw / 2.0, r.cy() - kr - 30.0, tw, 22.0);
            cv.sq(b, 8.0, cv.theme.surface_raised.lerp(INK, 0.1));
            cv.text(value, &text::CAPTION_TNUM, b, cv.theme.ink, Align::Center);
        }
        cv.zone_named(r.inset_xy(-SLIDER_KNOB / 2.0, -8.0), a, true, label, 8.0);
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
        let hit = self.zone_at(x, y);
        // a click outside an open dropdown closes it
        if self.dropdown.is_some()
            && !matches!(&hit, Some((Action::Settings(DROP, _), _)))
            && !matches!(&hit, Some((Action::Settings(id, 0), _)) if Some(*id) == self.dropdown.map(|d| d.0))
        {
            self.dropdown = None;
        }
        if let Some((a, r)) = hit {
            let double = self.last_click.as_ref().is_some_and(|(la, t)| {
                *la == a && t.elapsed().as_secs_f32() * 1000.0 < DOUBLE_CLICK_MS
            });
            self.last_click = Some((a.clone(), Instant::now()));
            if let Action::Slider(SliderKind::SettingsSlider(id)) = a {
                if double {
                    // double-click the knob: back to the default
                    self.slider_default(st, id);
                } else {
                    self.before = Some(st.settings.clone());
                    let r = r.inset_xy(SLIDER_KNOB / 2.0, 0.0);
                    self.slider = Some((id, r));
                    self.set_slider(st, id, (x - r.x) / r.w.max(1.0));
                }
            }
            if let Action::Settings(id, sub @ (1 | 2)) = a {
                if matches!(self.item(id).map(|i| i.ctl), Some(Ctl::Stepper { .. })) {
                    // steppers act on press and repeat while held
                    self.stepper(st, id, sub);
                    self.repeat = Some((a.clone(), Instant::now(), Instant::now()));
                }
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
        self.repeat = None;
        if self.slider.take().is_some() {
            if let Some(b) = self.before.take() {
                self.remember(b, st);
            }
            st.save_settings_soon();
            self.pressed = None;
            return;
        }
        let pressed = self.pressed.take();
        if let Some((a, _)) = self.zone_at(x, y) {
            let stepper = matches!(&a, Action::Settings(id, 1 | 2)
                if matches!(self.item(*id).map(|i| i.ctl), Some(Ctl::Stepper { .. })));
            if pressed.as_ref() == Some(&a) && !stepper {
                self.activate(st, a);
            }
        }
        self.dirty = true;
    }

    pub fn on_wheel(&mut self, delta: f32) {
        let over_nav = self.cursor.is_some_and(|(x, _)| x < SET_SIDEBAR_W);
        let s = if over_nav {
            &mut self.nav_scroll
        } else {
            &mut self.scroll
        };
        let t = (s.target - delta / 120.0 * 60.0).max(0.0);
        s.set(t);
        self.dropdown = None;
        self.dirty = true;
    }

    /// One undo step: what the settings were before a change (kept only if it changed).
    fn remember(&mut self, before: Settings, st: &State) {
        if before != st.settings && self.undo.last() != Some(&before) {
            self.undo.push(before);
            if self.undo.len() > UNDO_STEPS {
                self.undo.remove(0);
            }
        }
    }

    fn do_undo(&mut self, st: &mut State) {
        if let Some(prev) = self.undo.pop() {
            st.settings = prev;
            after_bulk_change(st);
            st.save_settings_soon();
            self.fields.retain(|k, _| *k == SEARCH_ID);
            self.toast = Some((crate::i18n::tr("Undone"), Instant::now()));
        }
    }

    fn set_slider(&mut self, st: &mut State, id: u16, frac: f32) {
        if let Some(Ctl::Slider {
            min,
            max,
            step,
            get,
            set,
            ..
        }) = self.item(id).map(|i| i.ctl)
        {
            let old = get(st);
            let v = min + (max - min) * frac.clamp(0.0, 1.0);
            let v = (v / step).round() * step;
            set(st, v.clamp(min, max));
            let new = get(st);
            // landed on a detent: the knob pops
            if (new - old).abs() > 1e-3
                && self.item(id).is_some_and(|i| i.label == STRENGTH_LABEL)
                && DETENTS.iter().any(|d| (new / 100.0 - d).abs() < 1e-3)
            {
                self.snap = Some((id, Instant::now()));
            }
            st.apply_settings();
            st.dirty = true;
        }
    }

    /// A slider back to its default value.
    fn slider_default(&mut self, st: &mut State, id: u16) {
        if let Some(Ctl::Slider { get, set, .. }) = self.item(id).map(|i| i.ctl) {
            let before = st.settings.clone();
            let cur = std::mem::take(&mut st.settings);
            let d = get(st);
            st.settings = cur;
            set(st, d);
            self.remember(before, st);
            self.snap = Some((id, Instant::now()));
            st.save_settings_soon();
        }
    }

    fn stepper(&mut self, st: &mut State, id: u16, sub: u16) {
        if let Some(Ctl::Stepper {
            min,
            max,
            step,
            get,
            set,
            ..
        }) = self.item(id).map(|i| i.ctl)
        {
            let before = st.settings.clone();
            let v = get(st) + if sub == 1 { -step } else { step };
            let v = ((v / step).round() * step).clamp(min, max);
            set(st, v);
            self.remember(before, st);
            st.save_settings_soon();
        }
    }

    fn open_result(&mut self, id: u16) {
        self.section = id as usize / 100;
        self.fields.remove(&SEARCH_ID);
        self.focus = None;
        self.scroll.snap(0.0);
        self.switched = Instant::now();
        self.flash = Some((id, Instant::now()));
        self.scroll_to = Some(id);
    }

    fn activate(&mut self, st: &mut State, a: Action) {
        match a {
            Action::Settings(NAV, s) => {
                if self.section != s as usize || !self.query().is_empty() {
                    self.section = s as usize;
                    self.switched = Instant::now();
                }
                self.fields.remove(&SEARCH_ID);
                self.confirm = None;
                self.dropdown = None;
                self.scroll.snap(0.0);
            }
            Action::Settings(RESULT, id) => self.open_result(id),
            Action::Settings(DROP, i) => {
                if let Some((id, _)) = self.dropdown.take() {
                    if let Some(Ctl::Choice(_, _, set)) = self.item(id).map(|i| i.ctl) {
                        let before = st.settings.clone();
                        set(st, i as usize);
                        self.remember(before, st);
                        st.save_settings_soon();
                    }
                }
            }
            Action::Settings(CMD, sub) => match sub {
                CMD_RESET => self.confirm = Some((RESET_SECTION, Instant::now())),
                CMD_RESET_CANCEL => self.confirm = None,
                CMD_RESET_OK => {
                    self.confirm = None;
                    if let Some(f) = self.sections[self.section].reset {
                        let before = st.settings.clone();
                        f(st);
                        after_bulk_change(st);
                        self.remember(before, st);
                        st.save_settings_soon();
                        self.fields.retain(|k, _| *k == SEARCH_ID);
                        self.toast = Some((
                            format!(
                                "{} — {}",
                                crate::i18n::tr(self.sections[self.section].name),
                                crate::i18n::tr("reset to defaults")
                            ),
                            Instant::now(),
                        ));
                    }
                }
                CMD_UNDO => self.do_undo(st),
                CMD_PLAY => {
                    self.preview_play = !self.preview_play;
                    self.preview_at = Instant::now();
                }
                s if (CMD_BACKDROP..CMD_BACKDROP + 5).contains(&s) => {
                    self.backdrop = PreviewBackdrop::ALL[(s - CMD_BACKDROP) as usize];
                }
                s if (CMD_STATE..CMD_STATE + 4).contains(&s) => {
                    self.preview_play = false;
                    self.set_preview_state((s - CMD_STATE) as usize);
                }
                _ => {}
            },
            Action::Settings(id, sub) => {
                let Some(item) = self.item(id).copied() else {
                    return;
                };
                let before = st.settings.clone();
                let mut msg = None;
                match item.ctl {
                    Ctl::Toggle(get, set) => {
                        let v = !get(st);
                        set(st, v);
                    }
                    Ctl::Choice(..) if sub == 0 => {
                        // the dropdown button
                        self.dropdown = match self.dropdown {
                            Some((d, _)) if d == id => None,
                            _ => self
                                .zones
                                .iter()
                                .find(|z| z.action == Action::Settings(id, 0))
                                .map(|z| (id, z.rect)),
                        };
                    }
                    Ctl::Choice(_, _, set) | Ctl::Chips(_, _, set) => {
                        set(st, sub.saturating_sub(1) as usize)
                    }
                    Ctl::Hotkey(..) => self.recording = Some(id),
                    Ctl::Button(_, f) | Ctl::Account(_, f) => msg = Some(f(st)),
                    Ctl::Confirm(_, f) => match sub {
                        0 => self.confirm = Some((id, Instant::now())),
                        1 => {
                            self.confirm = None;
                            msg = Some(f(st));
                        }
                        _ => self.confirm = None,
                    },
                    Ctl::Stepper { .. } => {}
                    _ => {}
                }
                if let Some(m) = msg {
                    if !m.is_empty() {
                        self.toast = Some((crate::i18n::tr(&m), Instant::now()));
                    }
                }
                self.remember(before, st);
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
        if let Some(item) = self.item(id).copied() {
            let before = st.settings.clone();
            match item.ctl {
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
                        self.toast =
                            Some((crate::i18n::tr("Key saved (encrypted)"), Instant::now()));
                    }
                    self.fields.remove(&id);
                }
                _ => {}
            }
            self.remember(before, st);
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
                if let Some(Ctl::Hotkey(_, set)) = self.item(id).map(|i| i.ctl) {
                    let before = st.settings.clone();
                    set(st, combo);
                    self.remember(before, st);
                    st.requests.push(Request::RegisterHotkeys);
                    st.save_settings_soon();
                }
                self.recording = None;
            }
            return true;
        }
        if let Some(id) = self.focus {
            let multiline = matches!(self.item(id).map(|i| &i.ctl), Some(Ctl::List(..)));
            if id == SEARCH_ID && vk == k::RETURN {
                // Enter opens the best match and flashes its row
                if let Some(first) = self.results(st).first().copied() {
                    self.open_result(first);
                }
                return true;
            }
            if id == SEARCH_ID && (vk == k::DOWN || vk == k::TAB) {
                self.focus = None;
                self.keyboard_focus = self
                    .results(st)
                    .first()
                    .map(|i| Action::Settings(RESULT, *i));
                return true;
            }
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
                0x5A if ctrl => f.undo(),
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
        // Ctrl+F searches, Ctrl+Z undoes the last change
        if ctrl && vk == 0x46 {
            self.focus = Some(SEARCH_ID);
            return true;
        }
        if ctrl && vk == 0x5A {
            self.do_undo(st);
            return true;
        }
        if vk == k::ESCAPE {
            if self.dropdown.take().is_some() || self.confirm.take().is_some() {
                return true;
            }
            return false;
        }
        match vk {
            k::TAB | k::DOWN | k::UP => {
                let f: Vec<Action> = self
                    .zones
                    .iter()
                    .filter(|z| z.focusable)
                    .filter(|z| {
                        // an open dropdown keeps focus inside its list
                        self.dropdown.is_none() || matches!(z.action, Action::Settings(DROP, _))
                    })
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
                    } else if let Action::Settings(id, sub @ (1 | 2)) = a {
                        if matches!(self.item(id).map(|i| i.ctl), Some(Ctl::Stepper { .. })) {
                            self.stepper(st, id, sub);
                        } else {
                            self.activate(st, a);
                        }
                    } else {
                        self.activate(st, a);
                    }
                }
                true
            }
            k::LEFT | k::RIGHT | k::PRIOR | k::NEXT | k::HOME | k::END => {
                let up = matches!(vk, k::RIGHT | k::PRIOR | k::END);
                match self.keyboard_focus.clone() {
                    Some(Action::Slider(SliderKind::SettingsSlider(id))) => {
                        if let Some(Ctl::Slider {
                            min,
                            max,
                            step,
                            get,
                            set,
                            ..
                        }) = self.item(id).map(|i| i.ctl)
                        {
                            let before = st.settings.clone();
                            let v = match vk {
                                k::HOME => min,
                                k::END => max,
                                k::PRIOR | k::NEXT => {
                                    get(st) + if up { 10.0 } else { -10.0 } * step
                                }
                                _ => get(st) + if up { step } else { -step },
                            };
                            set(st, v.clamp(min, max));
                            self.remember(before, st);
                            st.save_settings_soon();
                        }
                    }
                    // arrows move a segmented control's selection
                    Some(Action::Settings(id, sub))
                        if sub >= 1 && matches!(vk, k::LEFT | k::RIGHT) =>
                    {
                        if let Some(Ctl::Choice(options, _, _)) = self.item(id).map(|i| i.ctl) {
                            let n = options(st).len() as u16;
                            let next = if up {
                                (sub % n) + 1
                            } else {
                                (sub + n - 2) % n + 1
                            };
                            let a = Action::Settings(id, next);
                            self.activate(st, a.clone());
                            self.keyboard_focus = Some(a);
                        }
                    }
                    Some(Action::Settings(id, 0)) if matches!(vk, k::LEFT | k::RIGHT) => {
                        // a closed dropdown: arrows step through its options
                        if let Some(Ctl::Choice(options, get, set)) = self.item(id).map(|i| i.ctl) {
                            let n = options(st).len().max(1);
                            let before = st.settings.clone();
                            let cur = get(st);
                            set(st, if up { (cur + 1) % n } else { (cur + n - 1) % n });
                            self.remember(before, st);
                            st.save_settings_soon();
                        }
                    }
                    _ => {}
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
        self.dropdown = None;
        self.dirty = true;
    }
}

/// Ease-out cubic on 0..1 (clamped).
fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// A destructive confirm button (red text on a red-tinted pill).
fn danger_button(cv: &mut Canvas, label: &str, r: Rect, a: Action) {
    let hv = cv.hovered(r);
    cv.rrect(
        r,
        r.h / 2.0,
        CRITICAL.with_alpha(if hv { 0.30 } else { 0.20 }),
    );
    cv.text(
        &crate::i18n::tr(label),
        &text::BODY_MED,
        r,
        CRITICAL.lerp(WHITE, 0.25),
        Align::Center,
    );
    cv.zone_named(r, a, true, label, r.h / 2.0);
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
