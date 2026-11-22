//! Feature state, with persistence.

pub mod bsdate;
pub mod calc;
pub mod prayer;
pub mod store;
pub mod timer;
pub mod todo;
pub mod water;

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::motion::{MotionEnv, Spring, GENTLE};
use crate::render::icons::Icon;
use crate::render::image::{CachedBitmap, Pixels};
use crate::services::{calendar, downloads, ocr, privacy, sysmon, weather};
use crate::settings::Settings;
use crate::theme::tokens::Color;
use crate::util::{load_json, save_json};

#[derive(Serialize, Deserialize, Default, Clone, PartialEq)]
#[serde(default)]
pub struct StateFile {
    pub timer: timer::TimerData,
    pub water: water::WaterData,
    pub privacy_log: Vec<privacy::LogEntry>,
    pub palette_freq: BTreeMap<String, (u32, i64)>,
    pub lyrics_choice: String,
}

// ---------------------------------------------------------------- Ask

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AskMode {
    Ask,
    Translate,
    Palette,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChatMsg {
    pub user: bool,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PaletteItem {
    pub icon: Icon,
    pub title: String,
    pub subtitle: String,
    pub kind: PaletteKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PaletteKind {
    Launch(String),
    Url(String),
    Settings(String),
    Action(crate::app::Action),
    Command(String),
}

pub struct Ask {
    pub mode: AskMode,
    pub msgs: Vec<ChatMsg>,
    pub streaming: bool,
    pub cancel: Arc<AtomicBool>,
    pub provider: String,
    pub error: Option<String>,
    pub chips: [bool; 4],
    pub palette: Vec<PaletteItem>,
    pub palette_sel: usize,
    pub calc: Option<calc::Answer>,
    pub translation: String,
    pub request_id: u64,
    pub scroll: Spring,
    pub pinned_chat: bool,
}

impl Default for Ask {
    fn default() -> Self {
        Self {
            mode: AskMode::Ask,
            msgs: Vec::new(),
            streaming: false,
            cancel: Arc::new(AtomicBool::new(false)),
            provider: String::new(),
            error: None,
            chips: [false; 4],
            palette: Vec::new(),
            palette_sel: 0,
            calc: None,
            translation: String::new(),
            request_id: 0,
            scroll: Spring::new(0.0, GENTLE),
            pinned_chat: false,
        }
    }
}

impl Ask {
    pub fn busy(&self) -> bool {
        self.streaming
    }
    pub fn expanded_answer(&self) -> bool {
        self.mode == AskMode::Ask && (!self.msgs.is_empty() || self.error.is_some())
            || self.mode == AskMode::Palette && self.palette.len() > 2
            || self.mode == AskMode::Translate && !self.translation.is_empty()
    }
    pub fn stop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.streaming = false;
    }
    pub fn reset(&mut self) {
        self.stop();
        self.msgs.clear();
        self.error = None;
        self.translation.clear();
        self.calc = None;
        self.palette.clear();
    }
}

// ---------------------------------------------------------------- Live Activities

#[derive(Clone, Debug)]
pub struct LiveItem {
    pub id: String,
    pub client: u64,
    pub app: String,
    pub title: String,
    pub subtitle: String,
    pub icon: Option<Icon>,
    pub image: Option<CachedBitmap>,
    pub progress: Option<f32>,
    pub priority: i32,
    pub color: Option<Color>,
    pub actions: Vec<(String, String)>,
    pub final_text: Option<String>,
    pub updated: Instant,
    pub update_count: (Instant, u32),
}

#[derive(Default)]
pub struct LiveStore {
    pub items: Vec<LiveItem>,
    pub pending_approval: Vec<(u64, String)>,
    /// Messages held until the user approves the sending app.
    pub queued: Vec<crate::services::live_api::LiveMsg>,
}

#[derive(Clone, Debug)]
pub struct Meeting {
    pub app: String,
    pub started: Instant,
    pub from_calendar: bool,
}

impl LiveStore {
    pub fn get(&self, id: &str) -> Option<&LiveItem> {
        self.items.iter().find(|i| i.id == id)
    }
}

// ---------------------------------------------------------------- misc Pro feature state

#[derive(Default)]
pub struct WeatherState {
    pub forecast: Option<weather::Forecast>,
    pub fetched: Option<Instant>,
    pub fetching: bool,
    pub error: Option<String>,
    pub places: Vec<weather::Place>,
    pub rain_warned_for: i64,
    pub backoff: crate::services::http::Backoff,
}

#[derive(Default)]
pub struct CalendarState {
    pub events: Vec<calendar::Event>,
    pub fetched: Option<Instant>,
    pub fetching: bool,
    pub error: Option<String>,
    pub reminded: std::collections::HashSet<String>,
}

#[derive(Default)]
pub struct PrivacyState {
    pub uses: Vec<privacy::Use>,
    /// When the dots appeared (entrance animation).
    pub since: Option<Instant>,
}

pub struct SysMonState {
    pub cpu: VecDeque<f32>,
    pub ram: VecDeque<f32>,
    pub gpu: VecDeque<f32>,
    pub disk: VecDeque<f32>,
    pub net: VecDeque<f32>,
    pub last: Option<sysmon::Sample>,
    pub high_since: Option<Instant>,
    pub alerted: bool,
    /// Smoothed gauge values (CPU, memory, GPU) that spring to each new sample.
    pub gauges: [Spring; 3],
}

impl Default for SysMonState {
    fn default() -> Self {
        let q = || VecDeque::with_capacity(61);
        Self {
            cpu: q(),
            ram: q(),
            gpu: q(),
            disk: q(),
            net: q(),
            last: None,
            high_since: None,
            alerted: false,
            gauges: std::array::from_fn(|_| Spring::new(0.0, crate::motion::GENTLE)),
        }
    }
}

impl SysMonState {
    pub fn push(&mut self, s: sysmon::Sample) {
        self.gauges[0].set(s.cpu);
        self.gauges[1].set(s.ram);
        self.gauges[2].set(s.gpu);
        for (q, v) in [
            (&mut self.cpu, s.cpu),
            (&mut self.ram, s.ram),
            (&mut self.gpu, s.gpu),
            (&mut self.disk, s.disk_bps as f32),
            (&mut self.net, s.net_bps as f32),
        ] {
            q.push_back(v);
            while q.len() > 60 {
                q.pop_front();
            }
        }
        self.last = Some(s);
    }
}

#[derive(Default)]
pub struct DownloadsState {
    pub items: Vec<downloads::Download>,
}

#[derive(Default)]
pub struct OcrState {
    pub result: Option<ocr::OcrResult>,
    pub image: Option<Arc<Pixels>>,
    pub error: Option<String>,
    pub busy: bool,
}

#[derive(Default)]
pub struct AnnotateState {
    pub image: Option<Arc<Pixels>>,
    pub bitmap: Option<CachedBitmap>,
    pub tool: u8,
    pub shapes: Vec<AnnoShape>,
    pub drawing: Option<AnnoShape>,
    pub crop: Option<(f32, f32, f32, f32)>,
    /// Where the screenshot is drawn on the canvas (for mapping pointer input).
    pub img_rect: crate::render::Rect,
    /// Typing into the last text annotation.
    pub typing: bool,
}

/// Which animated list an item belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ListKind {
    Shelf,
    Clip,
    Todo,
}

/// Flight time of a deleted item into the trash, seconds.
pub const FLIGHT_SECS: f32 = 0.58;
/// Length of the drop ripple / particle burst, seconds.
pub const DROP_FX_SECS: f32 = 0.9;

/// A deleted item thrown into the trash can (a snapshot drawn along an arc).
#[derive(Clone)]
pub struct Flight {
    pub kind: ListKind,
    pub from: crate::render::Rect,
    pub title: String,
    pub icon: Icon,
    pub tint: Color,
    pub thumb: Option<CachedBitmap>,
    /// Stagger before take-off, seconds (Clear all).
    pub delay: f32,
    pub started: Instant,
    pub landed: bool,
}

impl Flight {
    /// Progress 0..1 of the flight (negative while waiting for its stagger).
    pub fn progress(&self, now: Instant) -> f32 {
        (now.saturating_duration_since(self.started).as_secs_f32() - self.delay) / FLIGHT_SECS
    }
}

/// Trash can that appears while items are thrown away.
pub struct Trash {
    pub kind: Option<ListKind>,
    /// Presence (bouncy pop-in).
    pub shown: Spring,
    /// Lid opening 0..1.
    pub lid: Spring,
    /// Gulp after an item lands (kicked, rests at 0).
    pub bump: Spring,
    /// Keep the can up briefly after the last landing.
    pub hold_until: Option<Instant>,
}

impl Default for Trash {
    fn default() -> Self {
        Self {
            kind: None,
            shown: Spring::unit(0.0, crate::motion::BOUNCY),
            lid: Spring::unit(0.0, crate::motion::SNAPPY),
            bump: Spring::unit(0.0, crate::motion::BOUNCY),
            hold_until: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnnoShape {
    pub tool: u8,
    pub a: (f32, f32),
    pub b: (f32, f32),
    pub text: String,
}

#[derive(Default)]
pub struct PhoneState {
    pub running: bool,
    pub url: String,
    pub pin: String,
    pub qr: Option<Vec<Vec<bool>>>,
    pub status: String,
    pub progress: Option<f32>,
}

pub struct Features {
    pub state: StateFile,
    pub state_dirty: bool,
    pub todo: todo::TodoList,
    pub todo_dirty: bool,
    pub shelf: store::Shelf,
    pub shelf_dirty: bool,
    pub clip: store::ClipHistory,
    pub clip_dirty: bool,
    pub notes: store::Notes,
    pub notes_dirty: bool,
    pub notifs: store::NotifHistory,
    pub ask: Ask,
    pub live: LiveStore,
    pub weather: WeatherState,
    pub calendar: CalendarState,
    pub privacy: PrivacyState,
    pub sysmon: SysMonState,
    pub downloads: DownloadsState,
    pub ocr: OcrState,
    pub annotate: AnnotateState,
    pub phone: PhoneState,
    pub rates: calc::Rates,
    pub water_fill: Spring,
    pub timer_ring: Spring,
    pub tick_anim: HashMap<u64, Spring>,
    /// Presence springs (0 = gone, 1 = shown) for list items entering / leaving.
    pub anim_shelf: HashMap<u64, Spring>,
    pub anim_clip: HashMap<u64, Spring>,
    pub anim_todo: HashMap<u64, Spring>,
    /// Items animating out; removed from their list once the spring reaches 0.
    pub leaving: Vec<(ListKind, u64)>,
    /// Where list items were last drawn (flight origins).
    pub item_rects: HashMap<(ListKind, u64), (crate::render::Rect, Icon)>,
    pub flights: Vec<Flight>,
    pub thrown: Vec<(ListKind, u64)>,
    pub trash: Trash,
    /// Drop celebration on the shelf: start time and the new item ids.
    pub drop_fx: Option<(Instant, Vec<u64>)>,
    /// Live copy of the high-CPU alert setting for the sampler thread.
    pub sysmon_alert: Arc<AtomicBool>,
    /// Row being dragged in the to-do list: (id, pointer y at grab).
    pub todo_drag: Option<(u64, f32)>,
    pub viz_active: Arc<AtomicBool>,
    pub sysmon_active: Arc<AtomicBool>,
    pub prayer_cache: Option<(i64, prayer::Times)>,
    pub prayer_notified: Option<(i64, usize)>,
    pub prayer_paused: Option<(i64, usize)>,
    pub save_due: Option<Instant>,
    pub meeting: Option<Meeting>,
    pub was_suppressed: bool,
    pub phone_server: Option<crate::services::phone_drop::Server>,
    pub update_url: Option<String>,
    pub holidays: Vec<bsdate::Holiday>,
}

impl Features {
    pub fn load(settings: &Settings) -> Self {
        let mut state: StateFile = load_json("state.json");
        state.water.roll();
        let mut shelf: store::Shelf = load_json("shelf.json");
        shelf.auto_clear(settings.productivity.shelf_autoclear_days);
        shelf.check_missing();
        let clip: store::ClipHistory = load_json("clipboard/index.json");
        Self {
            state,
            state_dirty: false,
            todo: load_json("todo.json"),
            todo_dirty: false,
            shelf,
            shelf_dirty: false,
            clip,
            clip_dirty: false,
            notes: load_json("notes.json"),
            notes_dirty: false,
            notifs: store::NotifHistory::default(),
            ask: Ask::default(),
            live: LiveStore::default(),
            weather: WeatherState::default(),
            calendar: CalendarState::default(),
            privacy: PrivacyState::default(),
            sysmon: SysMonState::default(),
            downloads: DownloadsState::default(),
            ocr: OcrState::default(),
            annotate: AnnotateState::default(),
            phone: PhoneState::default(),
            rates: crate::services::rates::load_cached(),
            water_fill: Spring::unit(0.0, GENTLE),
            timer_ring: Spring::unit(0.0, GENTLE),
            tick_anim: HashMap::new(),
            anim_shelf: HashMap::new(),
            anim_clip: HashMap::new(),
            anim_todo: HashMap::new(),
            leaving: Vec::new(),
            item_rects: HashMap::new(),
            flights: Vec::new(),
            thrown: Vec::new(),
            trash: Trash::default(),
            drop_fx: None,
            sysmon_alert: Arc::new(AtomicBool::new(settings.productivity.sysmon_alert)),
            todo_drag: None,
            viz_active: Arc::new(AtomicBool::new(false)),
            sysmon_active: Arc::new(AtomicBool::new(false)),
            prayer_cache: None,
            prayer_notified: None,
            prayer_paused: None,
            save_due: None,
            meeting: None,
            was_suppressed: false,
            phone_server: None,
            update_url: None,
            holidays: bsdate::holidays(),
        }
    }

    pub fn mark(&mut self) {
        self.save_due
            .get_or_insert(Instant::now() + std::time::Duration::from_millis(800));
    }

    /// Writes dirty files.
    pub fn save(&mut self, force: bool) {
        if !force && !self.save_due.is_some_and(|d| Instant::now() >= d) {
            return;
        }
        self.save_due = None;
        if self.state_dirty || force {
            save_json("state.json", &self.state);
            self.state_dirty = false;
        }
        if self.todo_dirty || force {
            save_json("todo.json", &self.todo);
            self.todo_dirty = false;
        }
        if self.shelf_dirty || force {
            save_json("shelf.json", &self.shelf);
            self.shelf_dirty = false;
        }
        if self.clip_dirty || force {
            let _ = crate::util::sub_dir("clipboard");
            save_json("clipboard/index.json", &self.clip);
            self.clip_dirty = false;
        }
        if self.notes_dirty || force {
            save_json("notes.json", &self.notes);
            self.notes_dirty = false;
        }
    }

    pub fn step(&mut self, dt: f32, env: &MotionEnv, now: Instant) -> bool {
        let mut m = false;
        m |= self.step_flights(dt, env, now);
        m |= self.water_fill.step(dt, env);
        m |= self.timer_ring.step(dt, env);
        m |= self.ask.scroll.step(dt, env);
        let mut done = Vec::new();
        for (k, s) in self.tick_anim.iter_mut() {
            let moving = s.step(dt, env);
            m |= moving;
            if !moving && s.target == 0.0 {
                done.push(*k);
            }
        }
        for k in done {
            self.tick_anim.remove(&k);
        }
        m |= self.step_lists(dt, env);
        for g in self.sysmon.gauges.iter_mut() {
            m |= g.step(dt, env);
        }
        m
    }

    fn map(&mut self, k: ListKind) -> &mut HashMap<u64, Spring> {
        match k {
            ListKind::Shelf => &mut self.anim_shelf,
            ListKind::Clip => &mut self.anim_clip,
            ListKind::Todo => &mut self.anim_todo,
        }
    }

    /// Presence 0..~1.1 of an item (1 when not animating).
    pub fn presence(&self, k: ListKind, id: u64) -> f32 {
        let m = match k {
            ListKind::Shelf => &self.anim_shelf,
            ListKind::Clip => &self.anim_clip,
            ListKind::Todo => &self.anim_todo,
        };
        m.get(&id).map(|s| s.value.max(0.0)).unwrap_or(1.0)
    }

    pub fn is_leaving(&self, k: ListKind, id: u64) -> bool {
        self.leaving.contains(&(k, id))
    }

    /// A new item springs in (bouncy on the shelf, smooth in lists).
    pub fn appear(&mut self, k: ListKind, id: u64) {
        let cfg = if k == ListKind::Shelf {
            crate::motion::BOUNCY
        } else {
            crate::motion::ISLAND
        };
        let mut s = Spring::unit(0.0, cfg);
        s.set(1.0);
        self.map(k).insert(id, s);
    }

    /// Starts an item's exit; it is removed from its list when the spring reaches 0.
    pub fn leave(&mut self, k: ListKind, id: u64) {
        if self.leaving.contains(&(k, id)) {
            return;
        }
        let cur = self.presence(k, id);
        let mut s = Spring::unit(cur, crate::motion::ISLAND);
        s.set(0.0);
        self.map(k).insert(id, s);
        self.leaving.push((k, id));
    }

    /// Throws an item into the trash (when it is on screen) and starts its exit.
    pub fn throw(&mut self, k: ListKind, id: u64, delay: f32) {
        if self.leaving.contains(&(k, id)) {
            return;
        }
        if let Some((from, icon)) = self.item_rects.get(&(k, id)).copied() {
            let snap = match k {
                ListKind::Shelf => self.shelf.items.iter().find(|i| i.id == id).map(|it| {
                    (
                        it.name.clone(),
                        icon,
                        crate::theme::tokens::TINT_SHELF,
                        self.shelf.thumbs.get(&id).cloned().flatten(),
                    )
                }),
                ListKind::Clip => self.clip.items.iter().find(|i| i.id == id).map(|it| {
                    (
                        it.text.lines().next().unwrap_or("").trim().to_string(),
                        icon,
                        crate::theme::tokens::TINT_CLIP,
                        self.clip.thumbs.get(&id).cloned().flatten(),
                    )
                }),
                ListKind::Todo => None,
            };
            if let Some((title, icon, tint, thumb)) = snap {
                self.flights.push(Flight {
                    kind: k,
                    from,
                    title,
                    icon,
                    tint,
                    thumb,
                    delay,
                    started: Instant::now(),
                    landed: false,
                });
                self.trash.kind = Some(k);
                self.trash.hold_until = None;
                self.thrown.push((k, id));
            }
        }
        self.leave(k, id);
    }

    /// Whether an item was thrown (drawn as a flying ghost, not in place).
    pub fn thrown(&self, k: ListKind, id: u64) -> bool {
        self.thrown.contains(&(k, id))
    }

    fn step_flights(&mut self, dt: f32, env: &MotionEnv, now: Instant) -> bool {
        let mut moving = !self.flights.is_empty();
        let mut lid_open = false;
        let mut landed = 0;
        for f in self.flights.iter_mut() {
            let p = f.progress(now);
            if p > 0.3 {
                lid_open = true;
            }
            if p >= 1.0 && !f.landed {
                f.landed = true;
                landed += 1;
            }
        }
        if landed > 0 {
            self.trash.bump.kick(7.0);
            self.trash.hold_until = Some(now + std::time::Duration::from_millis(520));
        }
        self.flights.retain(|f| !f.landed);
        let held = self.trash.hold_until.is_some_and(|t| now < t);
        let want = !self.flights.is_empty() || held;
        if self.trash.kind.is_some() {
            self.trash.shown.set(if want { 1.0 } else { 0.0 });
            if lid_open {
                self.trash.lid.set_cfg(1.0, crate::motion::SNAPPY);
            } else {
                self.trash.lid.set_cfg(0.0, crate::motion::BOUNCY);
            }
            moving |= held;
        }
        moving |= self.trash.shown.step(dt, env);
        moving |= self.trash.lid.step(dt, env);
        moving |= self.trash.bump.step(dt, env);
        if !want && self.trash.shown.value <= 0.01 && self.trash.shown.target == 0.0 {
            self.trash.kind = None;
        }
        if let Some((t, _)) = &self.drop_fx {
            if now.saturating_duration_since(*t).as_secs_f32() > DROP_FX_SECS {
                self.drop_fx = None;
            } else {
                moving = true;
            }
        }
        moving
    }

    fn step_lists(&mut self, dt: f32, env: &MotionEnv) -> bool {
        let mut moving = false;
        for k in [ListKind::Shelf, ListKind::Clip, ListKind::Todo] {
            let map = self.map(k);
            let mut settled = Vec::new();
            for (id, s) in map.iter_mut() {
                let m = s.step(dt, env);
                moving |= m;
                if !m && (s.value <= 0.001 || (s.value - 1.0).abs() < 0.001) {
                    settled.push((*id, s.value <= 0.001));
                }
            }
            for (id, gone) in settled {
                self.map(k).remove(&id);
                if gone && self.leaving.contains(&(k, id)) {
                    self.leaving.retain(|l| *l != (k, id));
                    self.thrown.retain(|l| *l != (k, id));
                    self.item_rects.remove(&(k, id));
                    match k {
                        ListKind::Shelf => {
                            self.shelf.remove(id);
                            self.shelf_dirty = true;
                        }
                        ListKind::Clip => {
                            self.clip.delete(id);
                            self.clip_dirty = true;
                        }
                        ListKind::Todo => {
                            self.todo.delete(id);
                            self.todo_dirty = true;
                        }
                    }
                    self.mark();
                }
            }
        }
        moving
    }

    /// Today's prayer times (cached per day).
    pub fn prayer_times(&mut self, s: &Settings) -> Option<prayer::Times> {
        let w = &s.weather;
        if w.lat == 0.0 && w.lon == 0.0 {
            return None;
        }
        let now = crate::util::local_now();
        let day = now.day_number();
        if let Some((d, t)) = self.prayer_cache {
            if d == day {
                return Some(t);
            }
        }
        let tz = crate::util::local_offset_secs() as f64 / 3600.0;
        let t = prayer::compute(
            now.year,
            now.month,
            now.day,
            w.lat,
            w.lon,
            tz,
            prayer::method(&s.productivity.prayer_method),
            s.productivity.asr_hanafi,
        );
        self.prayer_cache = Some((day, t));
        Some(t)
    }
}
