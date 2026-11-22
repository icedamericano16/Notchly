//! `State`: everything the UI thread owns, apart from the GPU and windows.

use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use super::activity::{ActKind, ActivityManager, PopKind, Popup, PopupQueue, PushResult};
use super::island::{Island, View};
use super::{Action, InputId, PageId, Request};
use crate::features::Features;
use crate::motion::{MotionEnv, Spring, GENTLE, SNAPPY};
use crate::render::canvas::Zone;
use crate::render::image::CachedBitmap;
use crate::services::audio::AudioCmd;
use crate::services::lyrics::{LyricsRequest, LyricsResult};
use crate::services::media::{MediaCmd, MediaSnapshot};
use crate::services::network::NetInfo;
use crate::services::Bus;
use crate::settings::{Settings, Speed};
use crate::theme::tokens::Color;
use crate::theme::{self, Theme};
use crate::util::unix_ms;

#[derive(Default)]
pub struct MediaState {
    pub snap: MediaSnapshot,
    pub art: Option<CachedBitmap>,
    pub art_key: u64,
    pub palette: Vec<Color>,
    pub eq: [Spring; 4],
    pub app_volume: Option<f32>,
    pub seek_drag: Option<f32>,
    pub paused_since: Option<Instant>,
    pub track_changed: Option<Instant>,
    pub peak: f32,
    pub bands: Option<Vec<f32>>,
}

impl MediaState {
    pub fn has_session(&self) -> bool {
        self.snap.active.is_some() && !self.snap.title.is_empty()
    }
    /// Interpolated playback position (browsers update the timeline rarely).
    pub fn position_ms(&self) -> i64 {
        let mut p = self.snap.position_ms;
        if self.snap.playing && self.snap.updated_ms > 0 {
            p += (unix_ms() - self.snap.updated_ms).max(0);
        }
        if self.snap.duration_ms > 0 {
            p = p.min(self.snap.duration_ms);
        }
        p.max(0)
    }
    pub fn progress(&self) -> f32 {
        if let Some(d) = self.seek_drag {
            return d;
        }
        if self.snap.duration_ms <= 0 {
            return 0.0;
        }
        self.position_ms() as f32 / self.snap.duration_ms as f32
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LyricsStatus {
    Idle,
    Loading,
    Ready(LyricsResult),
}

pub struct LyricsState {
    pub key: u64,
    pub status: LyricsStatus,
    /// Smoothly scrolled line index.
    pub scroll: Spring,
    pub translation: Option<Vec<String>>,
    pub show_translation: bool,
    pub current: Option<usize>,
    pub line_change: Instant,
}

impl Default for LyricsState {
    fn default() -> Self {
        Self {
            key: 0,
            status: LyricsStatus::Idle,
            scroll: Spring::new(0.0, GENTLE),
            translation: None,
            show_translation: false,
            current: None,
            line_change: Instant::now(),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct Battery {
    pub present: bool,
    pub percent: u32,
    pub ac: bool,
    pub charging: bool,
    pub saver: bool,
    pub secs_left: Option<u32>,
}

#[derive(Default)]
pub struct SysState {
    pub volume: f32,
    pub muted: bool,
    pub mic_muted: bool,
    pub brightness: Option<u32>,
    pub battery: Battery,
    pub net: NetInfo,
    pub net_known: bool,
    pub audio_device: String,
    pub headphones: bool,
    pub caps: bool,
    pub num: bool,
    pub layout: u32,
    pub volume_known: bool,
    /// Names of currently connected Bluetooth devices.
    pub bt_connected: Vec<String>,
}

#[derive(Default)]
pub struct Suppression {
    pub fullscreen: bool,
    pub fullscreen_app: String,
    pub presentation: bool,
    pub sharing: bool,
    pub paused_until: Option<Instant>,
    pub meeting_quiet: bool,
}

#[derive(Default, Clone, Debug)]
pub struct TextField {
    pub text: String,
    /// Caret position in chars.
    pub caret: usize,
    pub anchor: Option<usize>,
    pub history: Vec<String>,
    pub hist_idx: Option<usize>,
    pub undo: Vec<String>,
}

impl TextField {
    pub fn set(&mut self, s: &str) {
        self.text = s.to_string();
        self.caret = self.text.chars().count();
        self.anchor = None;
    }
    pub fn byte(&self, ci: usize) -> usize {
        self.text
            .char_indices()
            .nth(ci)
            .map(|(b, _)| b)
            .unwrap_or(self.text.len())
    }
    pub fn selection(&self) -> Option<(usize, usize)> {
        let a = self.anchor?;
        if a == self.caret {
            None
        } else {
            Some((a.min(self.caret), a.max(self.caret)))
        }
    }
    pub fn delete_selection(&mut self) -> bool {
        if let Some((a, b)) = self.selection() {
            let (ba, bb) = (self.byte(a), self.byte(b));
            self.text.replace_range(ba..bb, "");
            self.caret = a;
            self.anchor = None;
            return true;
        }
        false
    }
    pub fn insert(&mut self, s: &str) {
        self.undo.push(self.text.clone());
        if self.undo.len() > 50 {
            self.undo.remove(0);
        }
        self.delete_selection();
        let b = self.byte(self.caret);
        let clean: String = s
            .chars()
            .filter(|c| !c.is_control() || *c == '\n')
            .collect();
        self.text.insert_str(b, &clean);
        self.caret += clean.chars().count();
    }
    pub fn backspace(&mut self) {
        if self.delete_selection() {
            return;
        }
        if self.caret > 0 {
            self.undo.push(self.text.clone());
            let (a, b) = (self.byte(self.caret - 1), self.byte(self.caret));
            self.text.replace_range(a..b, "");
            self.caret -= 1;
        }
    }
    pub fn delete(&mut self) {
        if self.delete_selection() {
            return;
        }
        if self.caret < self.text.chars().count() {
            self.undo.push(self.text.clone());
            let (a, b) = (self.byte(self.caret), self.byte(self.caret + 1));
            self.text.replace_range(a..b, "");
        }
    }
    pub fn take(&mut self) -> String {
        let t = std::mem::take(&mut self.text);
        if !t.trim().is_empty() {
            self.history.push(t.clone());
            if self.history.len() > 50 {
                self.history.remove(0);
            }
        }
        self.caret = 0;
        self.anchor = None;
        self.hist_idx = None;
        t
    }
    pub fn history_step(&mut self, up: bool) {
        if self.history.is_empty() {
            return;
        }
        let idx = match (self.hist_idx, up) {
            (None, true) => self.history.len() - 1,
            (None, false) => return,
            (Some(i), true) => i.saturating_sub(1),
            (Some(i), false) => {
                if i + 1 >= self.history.len() {
                    self.hist_idx = None;
                    self.set("");
                    return;
                }
                i + 1
            }
        };
        self.hist_idx = Some(idx);
        let t = self.history[idx].clone();
        self.set(&t);
    }
    pub fn undo(&mut self) {
        if let Some(t) = self.undo.pop() {
            self.text = t;
            self.caret = self.caret.min(self.text.chars().count());
            self.anchor = None;
        }
    }
}

pub struct InputState {
    /// Cursor in canvas DIPs (None when outside the window).
    pub cursor: Option<(f32, f32)>,
    pub inside: bool,
    pub open_due: Option<Instant>,
    pub close_due: Option<Instant>,
    pub last_move: Option<(f32, f32, Instant)>,
    pub speed: f32,
    pub pressed: Option<Action>,
    pub press_at: Option<(f32, f32, Instant)>,
    pub slider: Option<(super::SliderKind, crate::render::Rect)>,
    pub focus: Option<Action>,
    pub keyboard_mode: bool,
    pub text_focus: Option<InputId>,
    pub texts: HashMap<InputId, TextField>,
    pub drag_near: bool,
    pub scroll: HashMap<PageId, Spring>,
    pub hover_since: Option<Instant>,
    pub caret_blink: Instant,
    pub has_focus: bool,
    pub drag_float: Option<(f32, f32)>,
    /// Minimal mode: keep the pill (time + status) visible until this instant after leaving.
    pub pill_until: Option<Instant>,
    /// Top of the first to-do row in canvas DIPs (from the last frame), for drag-reorder.
    pub todo_list_top: f32,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            cursor: None,
            inside: false,
            open_due: None,
            close_due: None,
            last_move: None,
            speed: 0.0,
            pressed: None,
            press_at: None,
            slider: None,
            focus: None,
            keyboard_mode: false,
            text_focus: None,
            texts: HashMap::new(),
            drag_near: false,
            scroll: HashMap::new(),
            hover_since: None,
            caret_blink: Instant::now(),
            has_focus: false,
            drag_float: None,
            pill_until: None,
            todo_list_top: 0.0,
        }
    }
}

impl InputState {
    pub fn text(&mut self, id: InputId) -> &mut TextField {
        self.texts.entry(id).or_default()
    }
    pub fn text_ref(&self, id: InputId) -> Option<&TextField> {
        self.texts.get(&id)
    }
    pub fn scroll(&mut self, p: PageId) -> &mut Spring {
        self.scroll
            .entry(p)
            .or_insert_with(|| Spring::new(0.0, SNAPPY))
    }
    pub fn scroll_value(&self, p: PageId) -> f32 {
        self.scroll.get(&p).map(|s| s.value).unwrap_or(0.0)
    }
}

pub struct Mascot {
    pub alpha: Spring,
    pub blink_at: Instant,
    pub blinking: Option<Instant>,
    pub look: Spring,
    pub bounce: Spring,
    pub hearts_until: Option<Instant>,
    pub wink_until: Option<Instant>,
    pub wave_until: Option<Instant>,
}

impl Default for Mascot {
    fn default() -> Self {
        Self {
            alpha: Spring::unit(0.0, GENTLE),
            blink_at: Instant::now() + Duration::from_secs(4),
            blinking: None,
            look: Spring::new(0.0, GENTLE),
            bounce: Spring::new(0.0, crate::motion::BOUNCY),
            hearts_until: None,
            wink_until: None,
            wave_until: None,
        }
    }
}

pub struct Services {
    pub media: Option<Sender<MediaCmd>>,
    pub audio: Option<Sender<AudioCmd>>,
    pub lyrics: Option<Sender<(LyricsRequest, bool)>>,
    pub bus: Option<Bus>,
}

pub struct State {
    pub settings: Settings,
    pub settings_save_due: Option<Instant>,
    pub theme: Theme,
    pub packs: Vec<Theme>,
    pub adaptive: Option<([Spring; 9], [Color; 3])>,
    pub env: MotionEnv,
    pub os_animations: bool,
    pub high_contrast: bool,
    pub isl: Island,
    pub act: ActivityManager,
    pub pop: PopupQueue,
    pub expanded: bool,
    pub page: PageId,
    pub pinned: bool,
    pub input: InputState,
    pub zones: Vec<Zone>,
    /// Hover / press springs per interactive zone.
    pub anims: HashMap<Action, (Spring, Spring)>,
    pub media: MediaState,
    pub lyrics: LyricsState,
    pub sys: SysState,
    pub sup: Suppression,
    pub mascot: Mascot,
    pub f: Features,
    pub svc: Services,
    pub requests: Vec<Request>,
    pub started: Instant,
    pub last_input: Instant,
    pub last_frame: Instant,
    pub dirty: bool,
    /// Whether the previous frame was animating (first-frame dt clamp).
    pub was_moving: bool,
    pub lock_check: Option<(Instant, u32)>,
    pub lang_check: Vec<Instant>,
    pub demo: Option<(Instant, usize)>,
    /// Debug QA script for the drop / trash choreography: (next step at, step).
    pub qa_trash: Option<(Instant, u8)>,
    pub perf: bool,
    /// QA flag: keep the page dock open (screenshots).
    pub qa_dock: bool,
    pub onboarding_step: u8,
}

/// Events that arrive in the first moments after start are a quiet sync, not changes.
pub const STARTUP_QUIET: Duration = Duration::from_millis(2500);
/// How long the pill stays after the pointer leaves (minimal mode).
pub const PILL_LINGER: Duration = Duration::from_millis(2500);
/// Minimum dwell before a full expand when the notch rests as a hairline.
pub const MINIMAL_EXPAND_DWELL: u64 = 450;

impl State {
    pub fn new(settings: Settings) -> Self {
        let packs = theme::load_packs();
        let mut s = Self {
            theme: theme::resolve(&settings.appearance.theme, &packs),
            packs,
            adaptive: None,
            env: MotionEnv::default(),
            os_animations: true,
            high_contrast: false,
            isl: Island::default(),
            act: ActivityManager::default(),
            pop: PopupQueue::default(),
            expanded: false,
            page: PageId::Dashboard,
            pinned: false,
            input: InputState::default(),
            zones: Vec::new(),
            anims: HashMap::new(),
            media: MediaState::default(),
            lyrics: LyricsState::default(),
            sys: SysState::default(),
            sup: Suppression::default(),
            mascot: Mascot::default(),
            f: Features::load(&settings),
            svc: Services {
                media: None,
                audio: None,
                lyrics: None,
                bus: None,
            },
            requests: Vec::new(),
            started: Instant::now(),
            last_input: Instant::now(),
            last_frame: Instant::now(),
            dirty: true,
            was_moving: false,
            lock_check: None,
            lang_check: Vec::new(),
            demo: None,
            qa_trash: None,
            perf: false,
            qa_dock: false,
            onboarding_step: 0,
            settings_save_due: None,
            settings,
        };
        s.media.eq = std::array::from_fn(|_| Spring::unit(0.22, SNAPPY));
        s.apply_settings();
        s
    }

    pub fn quiet_start(&self) -> bool {
        self.started.elapsed() < STARTUP_QUIET
    }

    /// Re-derives theme, motion environment and dependent state from settings.
    pub fn apply_settings(&mut self) {
        crate::i18n::set_language(&self.settings.general.language);
        self.f.sysmon_alert.store(
            self.settings.productivity.sysmon_alert,
            std::sync::atomic::Ordering::Relaxed,
        );
        let a = &self.settings.appearance;
        let mut t = theme::resolve(&a.theme, &self.packs);
        t.high_contrast = self.high_contrast && a.high_contrast_follow;
        self.theme = t;
        let mul = match self.settings.motion.speed {
            Speed::Calm => 0.7,
            Speed::Default => 1.0,
            Speed::Fast => 1.8,
        };
        let mut mul = mul;
        if let Some(sp) = self.theme.spring_overrides.and_then(|o| o.island()) {
            mul *= sp.stiffness / crate::motion::ISLAND.stiffness;
        }
        self.env = MotionEnv {
            stiffness_mul: mul,
            reduced: self.settings.motion.reduce_motion || !self.os_animations,
        };
        self.dirty = true;
    }

    pub fn save_settings_soon(&mut self) {
        self.settings_save_due = Some(Instant::now() + Duration::from_millis(400));
        self.apply_settings();
        self.requests.push(Request::ApplySettings);
    }

    /// Effective accent stops (adaptive colour cross-fades through springs).
    pub fn accent(&self) -> [Color; 3] {
        if let Some((sp, _)) = &self.adaptive {
            if self.settings.appearance.adaptive {
                return std::array::from_fn(|i| Color {
                    r: sp[i * 3].value,
                    g: sp[i * 3 + 1].value,
                    b: sp[i * 3 + 2].value,
                    a: 1.0,
                });
            }
        }
        self.theme.accent
    }

    // ------------------------------------------------------------ suppression

    pub fn quiet_hours(&self) -> bool {
        let q = &self.settings.quiet;
        if q.focus || self.sup.meeting_quiet {
            return true;
        }
        // "Do Not Disturb during focus": Notchly's own pop-ups stay quiet while focusing
        let t = &self.f.state.timer;
        if self.settings.productivity.focus_dnd
            && t.running()
            && t.phase == crate::features::timer::Phase::Focus
        {
            return true;
        }
        if q.schedule {
            let m = crate::util::local_now().minutes_of_day();
            let (s, e) = (q.start_min, q.end_min);
            return if s <= e {
                m >= s && m < e
            } else {
                m >= s || m < e
            };
        }
        false
    }

    pub fn fullscreen_hidden(&self) -> bool {
        self.settings.quiet.auto_fullscreen && (self.sup.fullscreen || self.sup.presentation)
    }

    pub fn popups_paused(&self) -> bool {
        self.sup.paused_until.is_some_and(|u| Instant::now() < u)
    }

    /// Pushes a pop-up honouring quiet hours, fullscreen and the pause menu.
    pub fn popup(&mut self, mut p: Popup) {
        if self.quiet_start()
            && !p.critical
            && !matches!(
                p.kind,
                PopKind::Info | PopKind::Approval | PopKind::Reminder | PopKind::TimerDone
            )
        {
            return;
        }
        let quiet = self.quiet_hours() || self.popups_paused();
        let fs = self.fullscreen_hidden();
        let important = p.critical
            || (p.priority >= 90
                && (self.settings.quiet.allow_timers || p.kind != PopKind::TimerDone))
            || (self.settings.quiet.allow_calls && is_call(&p));
        if (quiet || fs) && !important {
            if p.kind == PopKind::Notification {
                self.f.notifs.suppressed(&p);
            }
            return;
        }
        if p.kind == PopKind::Notification
            && self.sup.sharing
            && self.settings.quiet.hide_when_sharing
        {
            p.subtitle = "Content hidden while sharing".into();
        }
        p.title = crate::i18n::tr(&p.title);
        p.subtitle = crate::i18n::tr(&p.subtitle);
        for a in p.actions.iter_mut() {
            a.0 = crate::i18n::tr(&a.0);
        }
        let announce = format!("{} {}", p.title, p.subtitle).trim().to_string();
        let notification = p.kind == PopKind::Notification;
        let now = Instant::now();
        match self.pop.push(p, now) {
            PushResult::Shown => {
                self.isl.peek_arrival(notification);
                self.requests.push(Request::Announce(announce));
            }
            PushResult::Merged => {
                self.isl.peek_update();
            }
            _ => {}
        }
        self.dirty = true;
    }

    // ------------------------------------------------------------ view

    pub fn desired_view(&self) -> View {
        let critical_popup = self
            .pop
            .current
            .as_ref()
            .is_some_and(|p| p.critical || p.priority >= 90);
        let call = self.act.main().is_some_and(|a| a.kind == ActKind::Call);
        if self.fullscreen_hidden() && !self.expanded && !critical_popup && !call {
            return View::Hidden;
        }
        if self.expanded {
            return View::Page(self.page);
        }
        if let Some(p) = &self.pop.current {
            return View::Peek(p.id);
        }
        if self.act.main().is_some() {
            return View::Live;
        }
        // minimal mode: a hairline at rest; the pill shows on hover and lingers briefly after
        let pill_held = self.input.pill_until.is_some_and(|t| Instant::now() < t);
        if self.settings.appearance.minimal_idle
            && !self.input.inside
            && !self.input.drag_near
            && !pill_held
        {
            return View::Minimal;
        }
        View::Idle
    }

    /// Page to open by default: the top live activity's page, else the dashboard.
    pub fn default_page(&self) -> PageId {
        if !self.settings.general.onboarded {
            return PageId::Onboarding;
        }
        match self.act.main().map(|a| (a.kind, a.key.clone())) {
            Some((ActKind::Music, _)) => PageId::Music,
            Some((ActKind::Focus | ActKind::TimerEnding, _)) => PageId::Timer,
            Some((ActKind::Call, _)) => PageId::Meeting,
            Some((ActKind::Transfer, k)) if k.starts_with("dl:") => PageId::Downloads,
            Some((ActKind::LiveApi, _)) => PageId::Live,
            Some((ActKind::Event, _)) => PageId::Calendar,
            Some((ActKind::Prayer, _)) => PageId::Prayer,
            Some((ActKind::Note, _)) => PageId::Notes,
            Some((ActKind::SysAlert, _)) => PageId::SysMon,
            _ => PageId::Dashboard,
        }
    }

    /// Pages reachable with dots/arrows, in order.
    pub fn carousel(&self) -> Vec<PageId> {
        let p = &self.settings.productivity;
        let pro = true;
        let mut v = vec![PageId::Dashboard];
        if self.settings.music.enabled {
            v.push(PageId::Music);
            if self.settings.music.lyrics {
                v.push(PageId::Lyrics);
            }
        }
        v.push(PageId::Timer);
        if p.water {
            v.push(PageId::Water);
        }
        v.push(PageId::Todo);
        v.push(PageId::Shelf);
        if p.clipboard {
            v.push(PageId::Clipboard);
        }
        if p.prayer {
            v.push(PageId::Prayer);
        }
        if pro {
            if self.settings.weather.enabled {
                v.push(PageId::Weather);
            }
            if self.settings.calendar.enabled {
                v.push(PageId::Calendar);
            }
            if p.notes {
                v.push(PageId::Notes);
            }
            if p.sysmon {
                v.push(PageId::SysMon);
            }
            v.push(PageId::Widgets);
        }
        v
    }

    pub fn open(&mut self, page: PageId) {
        if self.expanded && self.page != page {
            let car = self.carousel();
            let from = car.iter().position(|p| *p == self.page);
            let to = car.iter().position(|p| *p == page);
            let dir = match (from, to) {
                (Some(a), Some(b)) if b < a => -1.0,
                _ => 1.0,
            };
            self.isl.page_switch(self.page, dir);
        }
        if !self.expanded {
            self.isl.content_from_tile = None;
        }
        self.page = page;
        self.expanded = true;
        self.input.close_due = None;
        self.input.open_due = None;
        self.on_page_opened(page);
        self.dirty = true;
    }

    pub fn collapse(&mut self) {
        if !self.expanded {
            return;
        }
        self.input.pill_until = Some(Instant::now() + PILL_LINGER);
        self.expanded = false;
        self.pinned = false;
        self.input.text_focus = None;
        self.input.focus = None;
        self.input.keyboard_mode = false;
        self.input.close_due = None;
        if self.input.has_focus {
            self.requests.push(Request::RestoreFocus);
        }
        self.on_collapsed();
        self.dirty = true;
    }

    pub fn switch_page(&mut self, delta: i32) {
        let car = self.carousel();
        let i = car.iter().position(|p| *p == self.page).unwrap_or(0) as i32;
        let n = car.len() as i32;
        let j = (i + delta).clamp(0, n - 1);
        if j != i {
            self.open(car[j as usize]);
        }
    }

    pub fn media_cmd(&mut self, c: MediaCmd) {
        if let Some(tx) = &self.svc.media {
            let _ = tx.send(c);
        }
    }

    pub fn audio_cmd(&mut self, c: AudioCmd) {
        if let Some(tx) = &self.svc.audio {
            let _ = tx.send(c);
        }
    }

    pub fn priority_of(&self, kind: ActKind) -> i32 {
        self.settings
            .live
            .priority
            .get(kind.name())
            .copied()
            .unwrap_or(kind.default_priority())
    }

    pub fn upsert_activity(&mut self, key: &str, kind: ActKind, expires: Option<Instant>) {
        let p = self.priority_of(kind);
        if self.act.upsert(key, kind, p, expires) {
            self.dirty = true;
        }
    }

    pub fn remove_activity(&mut self, key: &str) {
        if self.act.remove(key) {
            self.dirty = true;
        }
    }

    /// Updates the adaptive palette from the current art.
    pub fn update_adaptive(&mut self) {
        let Some((stops, _surface)) =
            crate::theme::adaptive::accent_from_palette(&self.media.palette)
        else {
            return;
        };
        let cur = self.accent();
        let springs = self.adaptive.get_or_insert_with(|| {
            let mk = |v: f32| Spring::unit(v, GENTLE);
            (
                std::array::from_fn(|i| mk([cur[i / 3].r, cur[i / 3].g, cur[i / 3].b][i % 3])),
                stops,
            )
        });
        for (i, s) in springs.0.iter_mut().enumerate() {
            let c = stops[i / 3];
            s.set([c.r, c.g, c.b][i % 3]);
        }
        springs.1 = stops;
        self.dirty = true;
    }
}

/// Sets laptop panel brightness through the active power scheme (AC and DC),
/// which is what the Windows brightness slider does. No-op on desktops.
pub fn set_brightness(pct: u32) {
    use windows::Win32::System::Power::*;
    const VIDEO_SUBGROUP: windows::core::GUID =
        windows::core::GUID::from_u128(0x7516b95f_f776_4464_8c53_06167f40cc99);
    const BRIGHTNESS: windows::core::GUID =
        windows::core::GUID::from_u128(0xaded5e82_b909_4619_9949_f5d71dac0bcb);
    unsafe {
        let mut scheme: *mut windows::core::GUID = std::ptr::null_mut();
        if PowerGetActiveScheme(None, &mut scheme).is_err() || scheme.is_null() {
            return;
        }
        let v = pct.min(100);
        let _ = PowerWriteACValueIndex(None, scheme, Some(&VIDEO_SUBGROUP), Some(&BRIGHTNESS), v);
        let _ = PowerWriteDCValueIndex(None, scheme, Some(&VIDEO_SUBGROUP), Some(&BRIGHTNESS), v);
        let _ = PowerSetActiveScheme(None, Some(scheme));
        let _ = windows::Win32::Foundation::LocalFree(Some(windows::Win32::Foundation::HLOCAL(
            scheme as _,
        )));
    }
}

/// An incoming-call notification (Teams, Zoom, Phone Link, WhatsApp, Skype, Discord...).
pub fn is_call(p: &Popup) -> bool {
    if p.kind != PopKind::Notification {
        return false;
    }
    let text = format!("{} {} {}", p.source, p.title, p.subtitle).to_lowercase();
    [
        "incoming call",
        "is calling",
        "calling you",
        "incoming video call",
        "incoming voice call",
        "ringing",
    ]
    .iter()
    .any(|k| text.contains(k))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::icons::Icon;

    #[test]
    fn detects_call_notifications() {
        let mut p = Popup::new(PopKind::Notification, Icon::Phone, "Incoming call", "Asha");
        p.source = "Microsoft Teams".into();
        assert!(is_call(&p));
        let p = Popup::new(
            PopKind::Notification,
            Icon::Phone,
            "Ravi is calling you",
            "",
        );
        assert!(is_call(&p));
        let p = Popup::new(
            PopKind::Notification,
            Icon::Bell,
            "New message",
            "Call me later",
        );
        assert!(!is_call(&p));
        // only notifications count, never system pop-ups
        let p = Popup::new(PopKind::Info, Icon::Phone, "Incoming call", "");
        assert!(!is_call(&p));
    }
}
