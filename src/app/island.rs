//! Island choreography: view state machine, shape springs, content transitions
//! and the frame-rate scheduler (section 5).

use std::time::{Duration, Instant};

use super::activity::ActKind;
use super::state::State;
use super::PageId;
use crate::motion::{choreo, MotionEnv, Spring, BOUNCY, GENTLE, ISLAND, ISLAND_H, SNAPPY};
use crate::render::Rect;
use crate::settings::Style;
use crate::theme::tokens::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum View {
    Hidden,
    Minimal,
    Idle,
    Live,
    Peek(u32),
    Page(PageId),
}

pub struct Island {
    pub w: Spring,
    pub h: Spring,
    pub radius: Spring,
    pub shoulder: Spring,
    pub bubble: Spring,
    pub view: View,
    pub prev: Option<(View, Instant, f32)>,
    pub reveal_at: Option<Instant>,
    pub from: (f32, f32),
    pub target: ShapeSpec,
    h_pending: Option<(Instant, f32)>,
    pub page_t: Spring,
    pub prev_page: Option<PageId>,
    pub page_dir: f32,
    pub roll_t: Spring,
    pub live_key: Option<String>,
    pub prev_live: Option<String>,
    pub peek_icon: Spring,
    pub peek_label: Spring,
    pub peek_value: Spring,
    pub pulse: Option<(Instant, u8, Color)>,
    pub breathe: Option<(Instant, Color)>,
    pub swell: Spring,
    pub sweep: Option<Instant>,
    pub content_from_tile: Option<Rect>,
    pub window_hidden: bool,
    pub float_x: Spring,
    pub last_view_change: Instant,
    /// Shape change deferred until outgoing content has faded (collapse / pop-up exit).
    shape_pending: Option<(Instant, ShapeSpec)>,
    /// The current pop-up is a notification: height drops in with a bouncy overshoot.
    pub notif_bounce: bool,
    /// When the visible pop-up arrived (drives the 120 ms type-on).
    pub peek_shown: Instant,
    /// Shared-element album art morph: Some(true) = live → page, Some(false) = page → live.
    pub art_morph: Option<bool>,
    /// Page dots morph into an icon dock while hovered (0 = dots, 1 = dock).
    pub dock: Spring,
    /// Hover region of the dots / dock in canvas DIPs (from the last frame).
    pub dock_rect: Rect,
    /// When each idle status glyph appeared (pop-in animation).
    pub glyph_since: std::collections::HashMap<Glyph, Instant>,
}

/// Status glyphs shown in the idle notch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Glyph {
    Headphones,
    Bluetooth,
    Charging,
    BatteryLow,
    Offline,
}

impl Default for Island {
    fn default() -> Self {
        let s = SHAPE_IDLE;
        Self {
            w: Spring::new(s.w, ISLAND),
            h: Spring::new(s.h, ISLAND_H),
            radius: Spring::new(s.radius, ISLAND),
            shoulder: Spring::new(s.shoulder, ISLAND),
            bubble: Spring::unit(0.0, ISLAND),
            view: View::Idle,
            prev: None,
            reveal_at: Some(Instant::now()),
            from: (s.w, s.h),
            target: s,
            h_pending: None,
            page_t: Spring::unit(1.0, GENTLE),
            prev_page: None,
            page_dir: 1.0,
            roll_t: Spring::unit(1.0, GENTLE),
            live_key: None,
            prev_live: None,
            peek_icon: Spring::unit(1.0, BOUNCY),
            peek_label: Spring::unit(1.0, GENTLE),
            peek_value: Spring::unit(0.0, SNAPPY),
            pulse: None,
            breathe: None,
            swell: Spring::unit(0.0, SNAPPY),
            sweep: None,
            content_from_tile: None,
            window_hidden: false,
            float_x: Spring::new(0.5, SNAPPY),
            last_view_change: Instant::now(),
            shape_pending: None,
            notif_bounce: false,
            peek_shown: Instant::now(),
            art_morph: None,
            dock: Spring::unit(0.0, ISLAND),
            dock_rect: Rect::default(),
            glyph_since: std::collections::HashMap::new(),
        }
    }
}

impl Island {
    /// Moves to a new view. Width animates now; height follows 20 ms later when growing.
    pub fn set_view(&mut self, v: View, shape: ShapeSpec, now: Instant, reduced: bool) {
        if v != self.view {
            let fade = match (self.view, v) {
                (View::Page(_), View::Page(_)) => 0.0, // page switch handles its own slide
                (View::Page(_), _) => choreo::COLLAPSE_OUT_MS,
                (View::Peek(_), _) => choreo::POPUP_OUT_MS,
                _ => choreo::CONTENT_OUT_MS,
            };
            let fade = if reduced {
                fade.min(choreo::REDUCED_FADE_MS)
            } else {
                fade
            };
            if !matches!((self.view, v), (View::Page(_), View::Page(_))) {
                self.prev = Some((self.view, now, fade));
                self.reveal_at = None;
            }
            self.art_morph = match (self.view, v) {
                (View::Live, View::Page(PageId::Music)) => Some(true),
                (View::Page(PageId::Music), View::Live) => Some(false),
                _ => None,
            };
            // content leaves first, then the shape springs back (collapse, pop-up exit)
            let shrinking = matches!(self.view, View::Page(_) | View::Peek(_))
                && !matches!(v, View::Page(_) | View::Peek(_));
            self.shape_pending = None;
            // a notification drops in with a bouncy height; everything else uses the island spring
            self.h.cfg = if matches!(v, View::Peek(_)) && self.notif_bounce {
                BOUNCY
            } else {
                ISLAND_H
            };
            self.from = (self.w.value, self.h.value);
            self.view = v;
            self.last_view_change = now;
            if shrinking && fade > 0.0 && self.art_morph.is_none() {
                self.shape_pending = Some((now + Duration::from_millis(fade as u64), shape));
                return;
            }
        }
        if let Some((at, pending)) = self.shape_pending.as_mut() {
            // keep the deferred target current; it is applied once the fade has finished
            *pending = shape;
            if now < *at {
                return;
            }
            self.shape_pending = None;
            self.from = (self.w.value, self.h.value);
        }
        self.set_shape(shape, now);
    }

    pub fn set_shape(&mut self, s: ShapeSpec, now: Instant) {
        if s == self.target {
            return;
        }
        let growing = s.h > self.h.target + 0.5;
        self.target = s;
        self.w.set(s.w);
        self.radius.set(s.radius);
        self.shoulder.set(s.shoulder);
        if growing {
            self.h_pending = Some((
                now + Duration::from_millis(choreo::HEIGHT_DELAY_MS as u64),
                s.h,
            ));
        } else {
            self.h_pending = None;
            self.h.set(s.h);
        }
    }

    /// 0..1 progress of the shape towards its target (dominant dimension).
    pub fn progress(&self) -> f32 {
        let dw = (self.target.w - self.from.0).abs();
        let dh = (self.target.h - self.from.1).abs();
        let (cur, from, to) = if dw >= dh {
            (self.w.value, self.from.0, self.target.w)
        } else {
            (self.h.value, self.from.1, self.target.h)
        };
        if (to - from).abs() < 0.5 {
            return 1.0;
        }
        ((cur - from) / (to - from)).clamp(0.0, 1.0)
    }

    /// Pop-up arrival: icon pops 0.6→1 (0.8→1 for notifications) on the bouncy spring,
    /// the label slides in from 8 DIP on the gentle one.
    pub fn peek_arrival(&mut self, notification: bool) {
        self.peek_icon.snap(if notification { 0.8 } else { 0.6 });
        self.peek_icon.set_cfg(1.0, BOUNCY);
        self.peek_label.snap(0.0);
        self.peek_label.set_cfg(1.0, GENTLE);
        self.notif_bounce = notification;
        self.peek_shown = Instant::now();
    }

    pub fn peek_update(&mut self) {}

    /// True while a shape change waits for outgoing content to fade.
    pub fn shape_deferred(&self) -> bool {
        self.shape_pending.is_some()
    }

    pub fn page_switch(&mut self, from: PageId, dir: f32) {
        self.prev_page = Some(from);
        self.page_dir = dir;
        self.page_t.snap(0.0);
        self.page_t.set(1.0);
    }

    pub fn roll_to(&mut self, key: Option<String>) {
        if key != self.live_key {
            self.prev_live = self.live_key.take();
            self.live_key = key;
            if self.prev_live.is_some() && self.live_key.is_some() {
                self.roll_t.snap(0.0);
                self.roll_t.set(1.0);
            }
        }
    }

    pub fn pulse(&mut self, c: Color, count: u8) {
        self.pulse = Some((Instant::now(), count, c));
    }

    pub fn breathe(&mut self, c: Color) {
        self.breathe = Some((Instant::now(), c));
    }

    /// Steps all springs. Returns true while anything moves.
    pub fn step(&mut self, dt: f32, env: &MotionEnv, now: Instant) -> bool {
        if let Some((at, h)) = self.h_pending {
            if now >= at {
                self.h.set(h);
                self.h_pending = None;
            }
        }
        let mut moving = self.h_pending.is_some() || self.shape_pending.is_some();
        if self.art_morph.is_some()
            && self.progress() >= 0.999
            && self.h.is_settled()
            && self.w.is_settled()
        {
            self.art_morph = None;
        }
        for s in [
            &mut self.w,
            &mut self.h,
            &mut self.radius,
            &mut self.shoulder,
            &mut self.bubble,
            &mut self.page_t,
            &mut self.roll_t,
            &mut self.peek_icon,
            &mut self.peek_label,
            &mut self.peek_value,
            &mut self.swell,
            &mut self.float_x,
        ] {
            moving |= s.step(dt, env);
        }
        if let Some((_, at, fade)) = self.prev {
            if (now - at).as_secs_f32() * 1000.0 > fade {
                self.prev = None;
            } else {
                moving = true;
            }
        }
        if self.page_t.is_settled() {
            self.prev_page = None;
        }
        if self.roll_t.is_settled() {
            self.prev_live = None;
        }
        if let Some((at, n, _)) = self.pulse {
            if now - at > Duration::from_millis(600 * n as u64) {
                self.pulse = None;
            } else {
                moving = true;
            }
        }
        if let Some((at, _)) = self.breathe {
            if now - at > Duration::from_millis(1200) {
                self.breathe = None;
            } else {
                moving = true;
            }
        }
        if let Some(at) = self.sweep {
            if now - at > Duration::from_millis(700) {
                self.sweep = None;
            } else {
                moving = true;
            }
        }
        moving
    }

    /// Glow intensity 0..1 from pulses / breathing.
    pub fn glow(&self, now: Instant) -> Option<(f32, Color)> {
        if let Some((at, _, c)) = self.pulse {
            let t = (now - at).as_secs_f32() / 0.6;
            let ph = t.fract();
            return Some(((ph * std::f32::consts::PI).sin() * 0.9, c));
        }
        if let Some((at, c)) = self.breathe {
            let t = (now - at).as_secs_f32() / 1.2;
            return Some(((t.min(1.0) * std::f32::consts::PI).sin() * 0.8, c));
        }
        None
    }
}

/// Island geometry for the current frame, in canvas DIPs.
#[derive(Clone, Copy, Debug)]
pub struct Geo {
    pub main: Rect,
    pub bubble: Option<Rect>,
    pub bubble_t: f32,
    pub floating: bool,
    pub radius: f32,
    pub shoulder: f32,
    /// Union of shapes + margin (window bounds).
    pub bounds: Rect,
}

/// How much taller the notch gets when the page dots open into the icon dock (DIP).
pub const DOCK_EXTRA: f32 = 24.0;

impl State {
    pub fn floating(&self) -> bool {
        self.settings.appearance.style == Style::Floating
    }

    /// Extra height the notch grows at the bottom while the page dock is open.
    pub fn dock_extra(&self) -> f32 {
        // not gated on the view, so the extra height eases away together with a collapse
        self.isl.dock.value.clamp(0.0, 1.2) * DOCK_EXTRA
    }

    pub fn geo(&self) -> Geo {
        let i = &self.isl;
        let floating = self.floating();
        let top = if floating { FLOAT_OFFSET } else { 0.0 };
        let swell = i.swell.value;
        let w = (i.w.value + swell * 24.0).max(1.0);
        let h = (i.h.value + swell * 6.0 + self.dock_extra()).max(0.0);
        let cx = CANVAS_W / 2.0;
        let bt = i.bubble.value.clamp(0.0, 1.2);
        let (main, bubble) = if bt > 0.01 {
            let b = BUBBLE * (0.55 + 0.45 * bt.min(1.0));
            let gap = -b + (DUAL_GAP + b) * bt;
            let total = w + gap + b;
            let mx = cx - total / 2.0;
            let main = Rect::new(mx, top, w, h);
            let bh = b.min(h.max(1.0));
            (main, Some(Rect::new(main.right() + gap, top, b, bh)))
        } else {
            (Rect::new(cx - w / 2.0, top, w, h), None)
        };
        let mut shapes = main;
        if let Some(b) = bubble {
            shapes = shapes.union(&b);
        }
        let shoulder = if floating { 0.0 } else { i.shoulder.value };
        let margin = if floating {
            SHADOW_MARGIN
        } else {
            ATTACHED_MARGIN
        };
        let mut bounds = Rect::new(
            shapes.x - shoulder - margin,
            0.0,
            shapes.w + 2.0 * (shoulder + margin),
            shapes.bottom() + margin,
        );
        // minimal mode: the window keeps a hover band over the hairline
        if self.settings.appearance.minimal_idle
            && matches!(self.isl.view, View::Minimal | View::Idle)
        {
            bounds = bounds.union(&Rect::new(cx - 110.0, 0.0, 220.0, 26.0));
        }
        // the music marquee tooltip hangs below the pill
        if self.marquee_visible() {
            bounds = bounds.union(&Rect::new(cx - 160.0, shapes.bottom(), 320.0, 34.0));
        }
        Geo {
            main,
            bubble,
            bubble_t: bt,
            floating,
            radius: i.radius.value,
            shoulder,
            bounds,
        }
    }

    pub fn marquee_visible(&self) -> bool {
        self.isl.view == super::island::View::Live
            && self.act.main().is_some_and(|a| a.kind == ActKind::Music)
            && self
                .input
                .hover_since
                .is_some_and(|t| t.elapsed() > Duration::from_millis(600))
            && !self.expanded
    }

    /// Shape for a view, given current data.
    pub fn shape_for(&self, v: View) -> (ShapeSpec, bool) {
        match v {
            View::Hidden => (
                ShapeSpec {
                    w: self.isl.w.target.min(SHAPE_IDLE.w),
                    ..SHAPE_HIDDEN
                },
                false,
            ),
            View::Minimal => (SHAPE_MINIMAL, false),
            View::Idle => {
                // the idle pill widens to fit the clock and status glyphs; the mascot keeps it compact
                let mascot = self.mascot.alpha.target > 0.5;
                if !self.settings.appearance.idle_clock || mascot {
                    return (SHAPE_IDLE, false);
                }
                let glyphs = self.idle_glyphs();
                let mut w: f32 = 58.0 + 30.0; // clock + padding
                for g in &glyphs {
                    w += if *g == Glyph::Charging { 44.0 } else { 22.0 };
                }
                (
                    ShapeSpec {
                        w: w.max(SHAPE_IDLE.w) + 20.0,
                        ..SHAPE_IDLE
                    },
                    false,
                )
            }
            View::Peek(_) => {
                let notif = self.pop.current.as_ref().is_some_and(|p| {
                    p.kind == super::activity::PopKind::Notification || !p.actions.is_empty()
                });
                if notif {
                    (
                        ShapeSpec {
                            w: 360.0,
                            h: 84.0,
                            ..SHAPE_PEEK
                        },
                        false,
                    )
                } else {
                    (SHAPE_PEEK, false)
                }
            }
            View::Page(p) => {
                let mut s = p.shape();
                if p == PageId::Ask && self.f.ask.expanded_answer() {
                    s = SHAPE_WIDE_TALL;
                }
                if p == PageId::Lyrics && self.lyrics.show_translation {
                    s.h += 24.0;
                }
                (s, false)
            }
            View::Live => {
                let dual = self.settings.live.dual_island && self.act.secondary().is_some();
                let main = self.act.main();
                let mut s = if dual { SHAPE_DUAL_MAIN } else { SHAPE_LIVE };
                if let Some(a) = main {
                    match a.kind {
                        ActKind::Music
                            if self.settings.music.lyrics_live
                                && self.lyrics_line_now().is_some() =>
                        {
                            s.w = if dual { 320.0 } else { 380.0 }
                        }
                        ActKind::TimerEnding => s.w = s.w.max(250.0),
                        ActKind::Call | ActKind::LiveApi | ActKind::SysAlert => {
                            s.w = s.w.max(if dual { 240.0 } else { 280.0 })
                        }
                        _ => {}
                    }
                }
                (s, dual)
            }
        }
    }

    pub fn lyrics_line_now(&self) -> Option<String> {
        if let super::state::LyricsStatus::Ready(crate::services::lyrics::LyricsResult::Synced(
            lines,
        )) = &self.lyrics.status
        {
            let i = crate::services::lyrics::current_line(lines, self.media.position_ms(), 250)?;
            let t = lines[i].1.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
        None
    }

    /// Per-frame update. Returns true if another smooth frame is needed.
    pub fn frame(&mut self, now: Instant) -> bool {
        // after idle the springs are at rest: start with one frame of time, not the whole gap
        let raw = (now - self.last_frame).as_secs_f32();
        let dt = if self.was_moving {
            raw.min(0.05)
        } else {
            raw.min(1.0 / 60.0)
        };
        self.last_frame = now;
        let view = self.desired_view();
        let (shape, dual) = self.shape_for(view);
        let reduced = self.env.reduced;
        self.isl.set_view(view, shape, now, reduced);
        self.isl
            .bubble
            .set(if dual && view == View::Live { 1.0 } else { 0.0 });
        let live_key = self.act.main().map(|a| a.key.clone());
        self.isl.roll_to(live_key);
        if let Some(p) = &self.pop.current {
            if let Some(v) = p.value {
                self.isl.peek_value.set(v);
                if self.isl.peek_value.value == 0.0
                    && self.isl.peek_value.target > 0.0
                    && self.isl.prev.is_some()
                {
                    self.isl.peek_value.snap(v);
                }
            }
        }
        let mut moving = self.isl.step(dt, &self.env, now);
        if self.isl.reveal_at.is_none() {
            let at = if view == View::Live {
                choreo::LIVE_REVEAL_AT
            } else {
                choreo::REVEAL_AT
            };
            if (self.isl.progress() >= at || reduced) && !self.isl.shape_deferred() {
                self.isl.reveal_at = Some(now);
            }
            moving = true;
        } else if let Some(r) = self.isl.reveal_at {
            // staggered rows keep animating for ~600 ms
            if (now - r) < Duration::from_millis(650) && !matches!(view, View::Hidden) {
                moving = true;
            }
        }
        // adaptive colour cross-fade
        if let Some((sp, _)) = &mut self.adaptive {
            for s in sp.iter_mut() {
                moving |= s.step(dt, &self.env);
            }
        }
        moving |= self.step_media_visuals(dt, now);
        moving |= self.step_lyrics(dt);
        moving |= self.step_mascot(dt, now);
        moving |= self.step_scroll(dt);
        moving |= self.step_zone_anims(dt);
        moving |= self
            .f
            .privacy
            .since
            .is_some_and(|t| t.elapsed() < Duration::from_millis(170));
        moving |= self.f.step(dt, &self.env, now);
        // window visibility
        let g = self.geo();
        self.was_moving = moving;
        let hide = view == View::Hidden && self.isl.h.value < 0.6 && !moving;
        self.isl.window_hidden = hide || g.main.h < 0.3;
        moving
    }

    /// Status glyphs for the idle notch, in display order (left to right).
    pub fn idle_glyphs(&self) -> Vec<Glyph> {
        let s = &self.sys;
        let mut v = Vec::new();
        if s.net_known && !s.net.connected {
            v.push(Glyph::Offline);
        }
        if s.headphones {
            v.push(Glyph::Headphones);
        } else if !s.bt_connected.is_empty() {
            v.push(Glyph::Bluetooth);
        }
        let b = &s.battery;
        if b.present && b.ac {
            v.push(Glyph::Charging);
        } else if b.present && b.percent <= self.settings.popups.low_battery {
            v.push(Glyph::BatteryLow);
        }
        v
    }

    /// Hover and press springs (snappy) for every focusable zone under the cursor.
    fn step_zone_anims(&mut self, dt: f32) -> bool {
        // glyph pop-ins: remember when each glyph appeared
        let now = Instant::now();
        let glyphs = self.idle_glyphs();
        self.isl.glyph_since.retain(|g, _| glyphs.contains(g));
        for g in glyphs {
            self.isl.glyph_since.entry(g).or_insert(now);
        }
        let popping = self.isl.view == View::Idle
            && self
                .isl
                .glyph_since
                .values()
                .any(|t| now.duration_since(*t) < Duration::from_millis(700));
        // page dots → icon dock while the pointer is over them
        let on_page = self.expanded && matches!(self.isl.view, View::Page(_));
        let over = on_page
            && self
                .input
                .cursor
                .is_some_and(|(x, y)| self.isl.dock_rect.inset(-6.0).contains(x, y));
        self.isl.dock.set(if over || (self.qa_dock && on_page) {
            1.0
        } else {
            0.0
        });
        let env = self.env;
        let dock_moving = self.isl.dock.step(dt, &env);
        let cursor = self.input.cursor;
        let pressed = self.input.pressed.clone();
        for z in self.zones.iter().filter(|z| z.focusable) {
            let hov = cursor.is_some_and(|(x, y)| z.rect.contains(x, y));
            let down = hov && pressed.as_ref() == Some(&z.action);
            if !hov && !down && !self.anims.contains_key(&z.action) {
                continue;
            }
            let e = self
                .anims
                .entry(z.action.clone())
                .or_insert_with(|| (Spring::unit(0.0, SNAPPY), Spring::unit(0.0, SNAPPY)));
            e.0.set(hov as u8 as f32);
            e.1.set(down as u8 as f32);
        }
        let mut moving = popping || dock_moving;
        let zones = &self.zones;
        self.anims.retain(|k, (h, p)| {
            if !zones.iter().any(|z| &z.action == k) {
                h.set(0.0);
                p.set(0.0);
            }
            let m = h.step(dt, &env) | p.step(dt, &env);
            moving |= m;
            m || h.target > 0.0 || p.target > 0.0
        });
        moving
    }

    fn step_scroll(&mut self, dt: f32) -> bool {
        let env = self.env;
        let mut m = false;
        for s in self.input.scroll.values_mut() {
            m |= s.step(dt, &env);
        }
        m
    }

    fn step_lyrics(&mut self, dt: f32) -> bool {
        let env = self.env;
        let mut target = None;
        if let super::state::LyricsStatus::Ready(crate::services::lyrics::LyricsResult::Synced(
            lines,
        )) = &self.lyrics.status
        {
            target = crate::services::lyrics::current_line(lines, self.media.position_ms(), 250);
        }
        if target != self.lyrics.current {
            self.lyrics.current = target;
            self.lyrics.line_change = Instant::now();
            if let Some(t) = target {
                self.lyrics.scroll.set(t as f32);
            }
            self.dirty = true;
        }
        self.lyrics.scroll.step(dt, &env)
    }

    /// Equaliser bars: real peak level when available, else smoothed sines at 6–10 Hz.
    fn step_media_visuals(&mut self, dt: f32, now: Instant) -> bool {
        let playing = self.media.snap.playing;
        let t = self.started.elapsed().as_secs_f32();
        let env = self.env;
        let mut m = false;
        let peak = self.media.peak;
        let sim = self.settings.motion.eq_source == crate::settings::EqSource::Simulated
            || peak <= 0.0001;
        for (i, s) in self.media.eq.iter_mut().enumerate() {
            let target = if !playing || env.reduced {
                0.22
            } else if let Some(b) = &self.media.bands {
                {
                    let g = b.len() / 4;
                    let avg = if g > 0 {
                        b[i * g..(i + 1) * g].iter().sum::<f32>() / g as f32
                    } else {
                        0.0
                    };
                    0.22 + 0.78 * avg.clamp(0.0, 1.0)
                }
            } else {
                let f = [6.3, 7.7, 9.1, 8.2][i];
                let ph = [0.0, 1.3, 2.1, 3.7][i];
                let n =
                    0.5 + 0.5 * ((t * f + ph).sin() * 0.6 + (t * f * 0.53 + ph * 2.0).sin() * 0.4);
                let lvl = if sim { 0.75 } else { (peak * 1.6).min(1.0) };
                0.22 + 0.78 * (n * lvl).clamp(0.0, 1.0)
            };
            s.set(target);
            m |= s.step(dt, &env);
        }
        let _ = now;
        m
    }

    fn step_mascot(&mut self, dt: f32, now: Instant) -> bool {
        let a = &self.settings.appearance;
        let idle_for = self.last_input.elapsed();
        let show = a.mascot
            && !self.env.reduced
            && self.isl.view == View::Idle
            && !self.fullscreen_hidden()
            && (idle_for > Duration::from_secs(a.mascot_delay_min as u64 * 60)
                || self.mascot.hearts_until.is_some_and(|h| now < h)
                || self.mascot.wave_until.is_some_and(|w| now < w));
        let m = &mut self.mascot;
        m.alpha.set(if show { 1.0 } else { 0.0 });
        let mut moving = m.alpha.step(dt, &self.env);
        if m.alpha.value > 0.01 {
            if now >= m.blink_at && m.blinking.is_none() {
                m.blinking = Some(now);
            }
            if let Some(b) = m.blinking {
                if now - b > Duration::from_millis(140) {
                    m.blinking = None;
                    let jitter = (crate::util::unix_ms() % 4000) as u64;
                    m.blink_at = now + Duration::from_millis(3000 + jitter);
                } else {
                    moving = true;
                }
            }
            moving |= m.look.step(dt, &self.env);
            moving |= m.bounce.step(dt, &self.env);
        }
        moving
    }

    /// The frame rate the current view needs: smooth (vblank), live (~30 fps), ticking (1 Hz) or idle.
    pub fn live_interval(&self) -> Option<Duration> {
        let v = self.isl.view;
        let playing = self.media.snap.playing;
        let mut best: Option<Duration> = None;
        let mut want = |d: Duration| best = Some(best.map_or(d, |b: Duration| b.min(d)));
        match v {
            View::Idle if self.settings.appearance.idle_clock => {
                // wake on the next minute boundary for the clock
                let secs = crate::util::local_now().secs_of_day();
                let to_next = 60.0 - secs.rem_euclid(60.0) + 0.05;
                want(Duration::from_secs_f64(to_next));
            }
            View::Live => {
                if let Some(a) = self.act.main() {
                    match a.kind {
                        ActKind::Music if playing => want(Duration::from_millis(33)),
                        ActKind::Focus
                        | ActKind::TimerEnding
                        | ActKind::Call
                        | ActKind::Event
                        | ActKind::Prayer
                        | ActKind::Transfer
                        | ActKind::Recording => want(Duration::from_millis(500)),
                        ActKind::LiveApi => want(Duration::from_millis(1000)),
                        _ => {}
                    }
                }
                if let Some(a) = self.act.secondary() {
                    if a.kind == ActKind::Focus || a.kind == ActKind::Transfer {
                        want(Duration::from_millis(1000));
                    }
                }
            }
            View::Page(p) => match p {
                // the wavy seek bar wants every frame; lyrics are fine at 30 fps
                PageId::Music if playing => want(Duration::from_millis(16)),
                PageId::Lyrics if playing => want(Duration::from_millis(33)),
                PageId::Dashboard => want(Duration::from_millis(if playing { 100 } else { 1000 })),
                PageId::Timer
                | PageId::Prayer
                | PageId::Meeting
                | PageId::Calendar
                | PageId::Downloads
                | PageId::Live => want(Duration::from_millis(500)),
                PageId::SysMon => want(Duration::from_millis(1000)),
                // gentle ambient motion: 15 fps is visually identical and halves the cost
                PageId::Water | PageId::Weather => want(Duration::from_millis(66)),
                PageId::Ask if self.f.ask.busy() => want(Duration::from_millis(50)),
                PageId::Todo
                | PageId::Clipboard
                | PageId::Notes
                | PageId::Ask
                | PageId::Annotate
                    if self.input.text_focus.is_some() =>
                {
                    want(Duration::from_millis(530))
                }
                _ => {}
            },
            _ => {}
        }
        if self.marquee_visible() {
            want(Duration::from_millis(33));
        }
        best
    }
}
