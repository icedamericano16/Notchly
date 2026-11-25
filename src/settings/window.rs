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

mod draw;

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
    /// A colour: swatch + hex; opens the colour picker.
    Colour(Get<Color>, Set<Color>),
    /// The theme gallery (spans the row).
    Themes,
    /// The Fluid Style gallery (spans the row).
    Fluid,
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

pub(crate) fn card(title: &'static str) -> Item {
    Item {
        label: title,
        hint: "",
        ctl: Ctl::Card,
        show: None,
    }
}

pub(crate) fn it(label: &'static str, hint: &'static str, ctl: Ctl) -> Item {
    Item {
        label,
        hint,
        ctl,
        show: None,
    }
}
pub(crate) fn when(i: Item, f: fn(&State) -> bool) -> Item {
    Item { show: Some(f), ..i }
}
fn glass_on(s: &State) -> bool {
    s.settings.appearance.material != Material::Solid
}
fn fluid_status(s: &State) -> String {
    let st = crate::render::glass::fluid::style(&s.settings.appearance.fluid.style);
    let theme = crate::theme::preset(st.theme)
        .map(|t| t.name)
        .unwrap_or_default();
    let how = match s.settings.appearance.material {
        Material::LiquidGlass if s.env.reduced => {
            crate::i18n::tr("Held still: reduce motion is on")
        }
        Material::LiquidGlass => crate::i18n::tr("Moves with Liquid Glass"),
        Material::Glass => {
            crate::i18n::tr("Glass shows its still frame; Liquid Glass makes it move")
        }
        Material::Solid => crate::i18n::tr("Choose Liquid Glass in Appearance to see it"),
    };
    format!("{how}\n{}: {theme}", crate::i18n::tr("Suggested theme"))
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

/// Every setting, section by section.
mod catalog;
pub use catalog::sections;

// ---------------------------------------------------------------- window state

/// Window-level actions (`Action::Settings(id, sub)` with these ids).
const NAV: u16 = 0xFFFF;
const SEARCH_ID: u16 = 0xFFFE;
const CMD: u16 = 0xFFFD;
const RESULT: u16 = 0xFFFC;
const DROP: u16 = 0xFFFB;
/// The colour picker's parts, and its hex field's focus id.
const PICK: u16 = 0xFFFA;
const HEX_ID: u16 = 0xFFF9;
const PICK_SV: u16 = 0;
const PICK_HUE: u16 = 1;
const PICK_RECENT: u16 = 10;
const CMD_REDO: u16 = 4;
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
    /// The open colour picker (item id, the control it opened from), its HSV and a drag.
    picker: Option<(u16, Rect)>,
    hsv: [f32; 3],
    picker_drag: Option<(u16, Rect)>,
    /// The hex field rejected its text (shakes).
    shake: Option<Instant>,
    /// How many themes the gallery shows (its height).
    theme_count: usize,
    #[cfg(debug_assertions)]
    qa_picked: u8,
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
            picker: None,
            hsv: [0.0, 0.0, 1.0],
            picker_drag: None,
            shake: None,
            theme_count: 0,
            #[cfg(debug_assertions)]
            qa_picked: 0,
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
        if ms(self.shake).is_some_and(|e| e < SHAKE_MS) {
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
        // Glass & Fluid: the style cards and the preview's fluid play
        if self.sections[self.section].name == "Glass & Fluid"
            && !env.reduced
            && self.query().is_empty()
        {
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
        // QA (debug builds): NOTCHLY_QA_SCROLL=<dip> scrolls, NOTCHLY_QA_PICK opens the first
        // colour picker on screen
        #[cfg(debug_assertions)]
        if crate::util::qa_env("NOTCHLY_QA_SCROLL").is_some()
            || crate::util::qa_env("NOTCHLY_QA_PICK").is_some()
        {
            if let Some(v) =
                crate::util::qa_env("NOTCHLY_QA_SCROLL").and_then(|v| v.parse::<f32>().ok())
            {
                if self.scroll.target == 0.0 && v > 0.0 {
                    self.scroll.snap(v);
                    self.dirty = true;
                }
            }
            if crate::util::qa_env("NOTCHLY_QA_PICK").is_some()
                && self.picker.is_none()
                && self.qa_picked < 1
            {
                let z = self.zones.iter().find(|z| matches!(z.action, Action::Settings(id, 0) if matches!(self.item(id).map(|i| i.ctl), Some(Ctl::Colour(..))))).map(|z| z.action.clone());
                if let Some(a) = z {
                    self.qa_picked += 1;
                    self.activate(st, a);
                }
            }
        }
        if let Some(n) = st.studio.notice.take() {
            self.toast = Some((n, Instant::now()));
            self.dirty = true;
        }
        if let Some(name) = st.settings_nav.take() {
            if let Some(i) = self.sections.iter().position(|s| s.name == name) {
                self.activate(st, Action::Settings(NAV, i as u16));
            }
        }
    }

    fn in_studio(&self) -> bool {
        self.sections[self.section].name == "Theme Studio" && self.query().is_empty()
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
        self.theme_count =
            super::studio::all_themes(st).len() + super::studio::gallery_extras(st).len();
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
                cv.rrect_brush(
                    Rect::new(content.x - 6.0, top, content.w + 12.0, 14.0),
                    0.0,
                    crate::render::canvas::as_brush(&g),
                );
            }
        }
        let max = (content_h - list.h + SET_GUTTER).max(0.0);
        if self.scroll.target > max {
            self.scroll.set(max);
        }
        self.draw_dropdown(st, &mut cv);
        self.draw_picker(st, &mut cv);
        self.draw_toast(&mut cv, w, h);
        self.zones = std::mem::take(&mut cv.zones);
        drop(cv);
        gfx.end(&mut self.surface);
        self.dirty = false;
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

    fn fluid_cols(&self) -> usize {
        let w = self.size.0 - SET_SIDEBAR_W - 2.0 * SET_GUTTER - 2.0 * SET_CARD_PAD;
        if w > 620.0 {
            4
        } else {
            2
        }
    }

    /// The gallery's columns and rows at the current window width.
    fn gallery_grid(&self) -> (usize, usize) {
        let w = self.size.0 - SET_SIDEBAR_W - 2.0 * SET_GUTTER - 2.0 * SET_CARD_PAD;
        let cols = (((w + THEME_GAP) / (THEME_CARD_W + THEME_GAP)).floor() as usize).max(1);
        (cols, self.theme_count.div_ceil(cols).max(1))
    }

    fn row_height(&self, id: u16) -> f32 {
        match self.item(id).map(|i| &i.ctl) {
            Some(Ctl::List(..)) => 104.0,
            Some(Ctl::Chips(..)) => 100.0,
            Some(Ctl::Info(_)) => 60.0,
            Some(Ctl::Fluid) => {
                let cols = self.fluid_cols();
                let rows = crate::render::glass::fluid::STYLES.len().div_ceil(cols);
                rows as f32 * (FLUID_CARD_H + THEME_GAP) - THEME_GAP + 24.0
            }
            Some(Ctl::Themes) => {
                let (cols, rows) = self.gallery_grid();
                let _ = cols;
                rows as f32 * (THEME_CARD_H + THEME_GAP) - THEME_GAP + 24.0
            }
            _ => SET_ROW_H,
        }
    }

    fn picker_rect(&self) -> Option<Rect> {
        let (_, b) = self.picker?;
        let (w, h) = (PICKER_W, PICKER_H);
        let x = (b.right() - w).clamp(SET_SIDEBAR_W + 8.0, self.size.0 - w - 8.0);
        let below = b.bottom() + 8.0 + h < self.size.1 - 8.0;
        let y = if below {
            b.bottom() + 8.0
        } else {
            (b.y - 8.0 - h).max(8.0)
        };
        Some(Rect::new(x, y, w, h))
    }

    fn picker_set(&mut self, st: &mut State) {
        let Some((id, _)) = self.picker else { return };
        if let Some(Ctl::Colour(_, set)) = self.item(id).map(|i| i.ctl) {
            let [h, s, v] = self.hsv;
            set(st, hsv_to_rgb(h, s, v));
        }
    }

    fn picker_drag_to(&mut self, st: &mut State, x: f32, y: f32) {
        let Some((part, r)) = self.picker_drag else {
            return;
        };
        let fx = ((x - r.x) / r.w.max(1.0)).clamp(0.0, 1.0);
        let fy = ((y - r.y) / r.h.max(1.0)).clamp(0.0, 1.0);
        if part == PICK_SV {
            self.hsv[1] = fx;
            self.hsv[2] = 1.0 - fy;
        } else {
            self.hsv[0] = fx * 359.9;
        }
        self.picker_set(st);
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
        if self.picker_drag.is_some() {
            self.picker_drag_to(st, x, y);
        }
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
        // the colour picker closes on a click outside it
        if let Some(pr) = self.picker_rect() {
            let on_own_button = matches!(&hit, Some((Action::Settings(id, 0), _)) if Some(*id) == self.picker.map(|p| p.0));
            if !pr.contains(x, y) && !on_own_button {
                self.close_picker(st);
            }
        }
        if let Some((Action::Settings(PICK, part @ (PICK_SV | PICK_HUE)), r)) = &hit {
            self.picker_drag = Some((*part, *r));
            self.picker_drag_to(st, x, y);
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
        if self.picker_drag.take().is_some() {
            let [h, s, v] = self.hsv;
            st.studio
                .remember_colour(&crate::theme::studio::hex(hsv_to_rgb(h, s, v)));
            self.pressed = None;
            self.dirty = true;
            return;
        }
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
            Action::Settings(PICK, sub) if sub >= PICK_RECENT => {
                let hx = st.studio.recent.get((sub - PICK_RECENT) as usize).cloned();
                if let Some(c) = hx.and_then(|h| Color::parse(&h)) {
                    self.hsv = rgb_to_hsv(c);
                    self.picker_set(st);
                }
            }
            Action::Settings(PICK, _) => {}
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
                CMD_UNDO if self.in_studio() => {
                    if super::studio::undo(st) {
                        self.toast = Some((crate::i18n::tr("Undone"), Instant::now()));
                    }
                }
                CMD_REDO => {
                    if super::studio::redo(st) {
                        self.toast = Some((crate::i18n::tr("Redone"), Instant::now()));
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
                    Ctl::Colour(get, _) => {
                        // opens (or closes) the colour picker
                        if self.picker.is_some_and(|p| p.0 == id) {
                            self.close_picker(st);
                        } else {
                            self.hsv = rgb_to_hsv(get(st));
                            self.picker = self
                                .zones
                                .iter()
                                .find(|z| z.action == Action::Settings(id, 0))
                                .map(|z| (id, z.rect));
                        }
                    }
                    Ctl::Fluid => {
                        if let Some(f) =
                            crate::render::glass::fluid::STYLES.get(sub.saturating_sub(1) as usize)
                        {
                            st.settings.appearance.fluid.style = f.id.to_string();
                        }
                    }
                    Ctl::Themes => {
                        let themes = super::studio::all_themes(st);
                        let i = sub.saturating_sub(1) as usize;
                        match themes.get(i) {
                            Some(t) => super::studio::apply_theme(st, &t.id.clone()),
                            // past the installed ones: a gallery theme, installed on click
                            None => {
                                msg = Some(super::studio::install_gallery(st, i - themes.len()))
                            }
                        }
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
        if id == HEX_ID {
            let t = self
                .fields
                .get(&HEX_ID)
                .map(|f| f.text.clone())
                .unwrap_or_default();
            match Color::parse(&t).filter(|_| t.trim().trim_start_matches('#').len() >= 6) {
                Some(c) => {
                    self.hsv = rgb_to_hsv(c);
                    self.picker_set(st);
                    st.studio.remember_colour(&crate::theme::studio::hex(c));
                }
                // never saved: the field shakes and keeps the last good colour
                None => self.shake = Some(Instant::now()),
            }
            self.fields.remove(&HEX_ID);
            self.dirty = true;
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
            if id == HEX_ID && vk == k::ESCAPE {
                self.focus = None;
                self.fields.remove(&HEX_ID);
                return true;
            }
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
        let redo = ctrl && (vk == 0x59 || (vk == 0x5A && shift));
        if redo {
            if self.in_studio() && super::studio::redo(st) {
                self.toast = Some((crate::i18n::tr("Redone"), Instant::now()));
            }
            return true;
        }
        if ctrl && vk == 0x5A {
            if self.in_studio() {
                if super::studio::undo(st) {
                    self.toast = Some((crate::i18n::tr("Undone"), Instant::now()));
                }
            } else {
                self.do_undo(st);
            }
            return true;
        }
        if vk == k::ESCAPE {
            if self.picker.is_some() {
                self.close_picker(st);
                return true;
            }
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

impl SettingsWin {
    fn close_picker(&mut self, st: &mut State) {
        if self.focus == Some(HEX_ID) {
            self.commit_focus(st);
        }
        self.picker = None;
        self.picker_drag = None;
        self.dirty = true;
    }
}

/// HSV (hue in degrees, s and v 0-1) to a colour.
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Color {
    let h = h.rem_euclid(360.0) / 60.0;
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    Color {
        r: r + m,
        g: g + m,
        b: b + m,
        a: 1.0,
    }
}

fn rgb_to_hsv(c: Color) -> [f32; 3] {
    let max = c.r.max(c.g).max(c.b);
    let min = c.r.min(c.g).min(c.b);
    let d = max - min;
    let h = if d < 1e-6 {
        0.0
    } else if max == c.r {
        60.0 * ((c.g - c.b) / d).rem_euclid(6.0)
    } else if max == c.g {
        60.0 * ((c.b - c.r) / d + 2.0)
    } else {
        60.0 * ((c.r - c.g) / d + 4.0)
    };
    [h, if max > 0.0 { d / max } else { 0.0 }, max]
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

#[cfg(test)]
mod picker_tests {
    use super::*;

    #[test]
    fn hsv_round_trip() {
        for hex in [
            0xFF0000, 0x00FF00, 0x2D5BFF, 0x7B4BFF, 0xFFD60A, 0x808080, 0x000000,
        ] {
            let c = Color::rgb(hex);
            let [h, s, v] = rgb_to_hsv(c);
            let d = hsv_to_rgb(h, s, v);
            assert!(
                (c.r - d.r).abs() < 1e-3 && (c.g - d.g).abs() < 1e-3 && (c.b - d.b).abs() < 1e-3,
                "{hex:06X}"
            );
        }
    }
}
