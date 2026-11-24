//! Mouse, wheel and keyboard handling: hover intent, zones, sliders, focus ring
//! and text editing.

use std::time::{Duration, Instant};

use super::island::View;
use super::state::State;
use super::{Action, InputId, PageId, Request, SliderKind};
use crate::features::AskMode;
use crate::render::Rect;
use crate::services::audio::AudioCmd;
use crate::services::media::MediaCmd;
use crate::settings::OpenMode;
use crate::theme::tokens::{
    CTL_BAND_GAIN, CTL_BAND_MAX, CTL_BRIGHT_MS, CTL_PAD_GAIN, CTL_WHEEL_HOLD_MS, CTL_WHEEL_STEP,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
    Middle,
}

/// Result of a click the platform must handle itself.
#[derive(Debug, PartialEq)]
pub enum ClickOutcome {
    Handled,
    ContextMenu,
    None,
}

pub mod vk {
    pub const BACK: u32 = 0x08;
    pub const TAB: u32 = 0x09;
    pub const RETURN: u32 = 0x0D;
    pub const ESCAPE: u32 = 0x1B;
    pub const SPACE: u32 = 0x20;
    pub const PRIOR: u32 = 0x21;
    pub const NEXT: u32 = 0x22;
    pub const END: u32 = 0x23;
    pub const HOME: u32 = 0x24;
    pub const LEFT: u32 = 0x25;
    pub const UP: u32 = 0x26;
    pub const RIGHT: u32 = 0x27;
    pub const DOWN: u32 = 0x28;
    pub const DELETE: u32 = 0x2E;
}

/// Things that have a long-press (and right-click) action.
pub fn long_pressable(a: &Action) -> bool {
    matches!(a, Action::WaterReminders)
}

impl State {
    fn pill_rect(&self) -> Rect {
        let g = self.geo();
        if self.settings.appearance.minimal_idle
            && matches!(self.isl.view, View::Minimal | View::Idle)
        {
            // the hairline (and its pill) get a generous invisible hover band
            let band = Rect::new(g.main.cx() - 110.0, 0.0, 220.0, 26.0);
            return if self.isl.view == View::Idle {
                band.union(&g.main)
            } else {
                band
            };
        }
        let mut r = g.main;
        if let Some(b) = g.bubble {
            r = r.union(&b);
        }
        // a few DIPs of slop above (bezel) and around
        Rect::new(
            r.x - g.shoulder,
            r.y - 4.0,
            r.w + 2.0 * g.shoulder,
            r.h + 8.0,
        )
    }

    fn zone_at(&self, x: f32, y: f32) -> Option<(usize, Action, Rect)> {
        self.zones
            .iter()
            .enumerate()
            .rev()
            .find(|(_, z)| z.rect.contains(x, y))
            .map(|(i, z)| (i, z.action.clone(), z.rect))
    }

    pub fn on_mouse_move(&mut self, x: f32, y: f32) {
        let now = Instant::now();
        if let Some((lx, ly, lt)) = self.input.last_move {
            let dt = (now - lt).as_secs_f32() * 1000.0;
            if dt > 0.5 {
                let d = ((x - lx).powi(2) + (y - ly).powi(2)).sqrt();
                self.input.speed = self.input.speed * 0.5 + (d / dt) * 0.5;
            }
        }
        self.input.last_move = Some((x, y, now));
        self.input.cursor = Some((x, y));
        // dragging in the notes editor selects text (the editor hit-tests each frame)
        if self.input.pressed == Some(Action::Focus(InputId::Note)) {
            if let Some(i) = self.input.note_hit {
                let t = self.input.text(InputId::Note);
                if t.anchor.is_none() && i != t.caret {
                    t.anchor = Some(t.caret);
                }
                t.caret = i;
                self.input.caret_blink = now;
                self.dirty = true;
            }
        }
        let inside = self.pill_rect().contains(x, y) || self.expanded;
        if inside && !self.input.inside {
            self.input.inside = true;
            self.input.hover_since = Some(now);
            self.input.close_due = None;
            let can_open = !self.expanded
                && matches!(
                    self.isl.view,
                    View::Idle | View::Live | View::Minimal | View::Peek(_)
                );
            if can_open && self.settings.general.open_mode == OpenMode::Hover {
                // minimal mode: the pill appears at once, the full notch after a longer dwell
                let mut delay = self.settings.general.hover_delay_ms as u64;
                if self.settings.appearance.minimal_idle {
                    delay = delay.max(super::state::MINIMAL_EXPAND_DWELL);
                }
                self.input.open_due = Some(now + Duration::from_millis(delay));
            }
            self.wake_mascot();
        } else if !inside && self.input.inside {
            self.on_mouse_leave();
            self.input.cursor = Some((x, y));
            return;
        }
        // mascot looks toward the cursor
        let g = self.geo();
        self.mascot
            .look
            .set(((x - g.main.cx()) / 80.0).clamp(-1.0, 1.0));
        // drags
        if let Some((kind, r)) = self.input.slider {
            let v = slider_value(r, x, y);
            self.on_slider(kind, v, false);
        } else if let (Some(Action::ShelfSelect(id)), Some((px, py, _))) =
            (self.input.pressed.clone(), self.input.press_at)
        {
            if (x - px).abs() + (y - py).abs() > 8.0 {
                self.input.pressed = None;
                let mut ids: Vec<u64> = self.f.shelf.selected.iter().copied().collect();
                if !ids.contains(&id) {
                    ids = vec![id];
                }
                self.requests.push(Request::DragOutShelf(ids));
            }
        } else if let Some(d) = self.f.annotate.drawing.as_mut() {
            // screenshot annotation: drag out the shape
            let r = self.f.annotate.img_rect;
            d.b = (
                ((x - r.x) / r.w.max(1.0)).clamp(0.0, 1.0),
                ((y - r.y) / r.h.max(1.0)).clamp(0.0, 1.0),
            );
        } else if let (Some(Action::TodoRow(id)), Some((_, py, _))) =
            (self.input.pressed.clone(), self.input.press_at)
        {
            // drag to reorder to-dos (in the unfiltered list, where rows map to positions)
            if self.input.todo_filter == 0 && (self.f.todo_drag.is_some() || (y - py).abs() > 6.0) {
                self.f.todo_drag = Some((id, py));
                let rows = self.f.todo.tasks.len();
                if let Some(from) = self.f.todo.tasks.iter().position(|t| t.id == id) {
                    let to = (((y - self.input.todo_list_top) / crate::ui::todo::ROW_PITCH)
                        .floor()
                        .max(0.0) as usize)
                        .min(rows.saturating_sub(1));
                    if to != from {
                        self.f.todo.move_item(from, to);
                        self.f.todo_dirty = true;
                    }
                }
            }
        }
        self.dirty = true;
    }

    pub fn on_mouse_leave(&mut self) {
        if self.settings.appearance.minimal_idle && !self.expanded {
            self.input.pill_until = Some(Instant::now() + super::state::PILL_LINGER);
        }
        self.input.inside = false;
        self.input.hover_since = None;
        self.input.open_due = None;
        self.input.cursor = None;
        self.input.last_move = None;
        let busy =
            self.input.text_focus.is_some() || self.input.slider.is_some() || self.input.drag_near;
        if self.expanded
            && !self.pinned
            && !busy
            && self.settings.general.open_mode == OpenMode::Hover
        {
            self.input.close_due = Some(
                Instant::now() + Duration::from_millis(self.settings.general.close_delay_ms as u64),
            );
        }
        self.dirty = true;
    }

    /// The long-press (and right-click) action of something long-pressable.
    fn long_press(&mut self, a: Action) {
        if a == Action::WaterReminders {
            self.input.water_menu = true;
        }
        self.dirty = true;
    }

    /// A wheel / trackpad step on a Control Center slider. A mouse wheel moves in clean 5 %
    /// notches that glide; a trackpad (or free-spinning wheel) sends fine deltas that follow
    /// the fingers 1:1. Past either end the slider stretches elastically and springs back.
    fn ctl_wheel(&mut self, k: SliderKind, delta: f32) {
        let now = Instant::now();
        // precise devices send deltas that aren't whole notches
        let precise = delta.abs() % 120.0 > 0.5;
        self.diag(format!(
            "wheel {delta:+.0} on {k:?} ({})",
            if precise { "trackpad" } else { "mouse notch" }
        ));
        let cur = match k {
            SliderKind::Brightness => match self.sys.brightness {
                Some(b) => b as f32 / 100.0,
                None => return,
            },
            _ => {
                if self.sys.muted {
                    0.0
                } else {
                    self.sys.volume
                }
            }
        };
        // a gesture continues from its own target (the system value lags behind)
        let base = self
            .input
            .ctl_wheel
            .filter(|w| w.0 == k && now - w.2 < Duration::from_millis(CTL_WHEEL_HOLD_MS))
            .map(|w| w.1)
            .unwrap_or(cur);
        let notches = delta / 120.0;
        let raw = if precise {
            base + notches * CTL_PAD_GAIN
        } else {
            ((base / CTL_WHEEL_STEP).round() + notches.round()) * CTL_WHEEL_STEP
        };
        let v = raw.clamp(0.0, 1.0);
        // overscroll: the elastic edge
        let over = raw - v;
        if over.abs() > 1e-4 {
            let i = (k == SliderKind::Volume) as usize;
            let band = &mut self.f.ctl_band[i];
            let push = (over * CTL_BAND_GAIN).clamp(-CTL_BAND_MAX, CTL_BAND_MAX);
            band.snap((band.value + push).clamp(-CTL_BAND_MAX, CTL_BAND_MAX));
            band.set(0.0);
        }
        self.input.ctl_wheel = Some((k, v, now, precise));
        match k {
            SliderKind::Brightness => {
                let pct = (v * 100.0).round() as u32;
                self.sys.brightness = Some(pct);
                // the display is slow to write: at most every CTL_BRIGHT_MS, the last value always
                let due = self
                    .input
                    .bright_applied
                    .is_none_or(|t| now - t >= Duration::from_millis(CTL_BRIGHT_MS));
                if due {
                    super::state::set_brightness(pct);
                    self.input.bright_applied = Some(now);
                    self.input.bright_pending = None;
                } else {
                    self.input.bright_pending = Some(pct);
                }
            }
            _ => self.on_slider(k, v, false),
        }
        self.dirty = true;
    }

    /// Fires due hover timers. Called from the loop.
    pub fn check_hover(&mut self, now: Instant) {
        // a long press fires once while held; the release then does nothing
        let threshold = self.settings.general.long_press();
        if let Some(super::press::Outcome::Long(a)) = self.input.press.tick(now, threshold) {
            self.input.pressed = None;
            self.diag(format!("long press {a:?}"));
            self.long_press(a);
        }
        if let (Some(p), Some(t)) = (self.input.bright_pending, self.input.bright_applied) {
            if now - t >= Duration::from_millis(CTL_BRIGHT_MS) {
                super::state::set_brightness(p);
                self.input.bright_applied = Some(now);
                self.input.bright_pending = None;
            }
        }
        if self.input.open_due.is_some_and(|d| now >= d) {
            self.input.open_due = None;
            // only open if still inside and the cursor isn't flicking past
            if self.input.inside && !self.expanded {
                if self.input.speed < 1.2 {
                    let p = self.default_page();
                    self.open(p);
                } else {
                    // still moving fast: wait for the pointer to settle instead of giving up
                    self.input.open_due = Some(now + Duration::from_millis(120));
                    self.input.speed *= 0.5;
                }
            }
        }
        if self.input.close_due.is_some_and(|d| now >= d) {
            self.input.close_due = None;
            if !self.input.inside && !self.pinned && self.input.text_focus.is_none() {
                self.collapse();
            }
        }
    }

    fn wake_mascot(&mut self) {
        if self.mascot.alpha.value > 0.3 {
            self.mascot.bounce.kick(-80.0);
        }
        self.last_input = Instant::now();
    }

    pub fn on_mouse_down(&mut self, x: f32, y: f32, b: Button) {
        self.last_input = Instant::now();
        self.input.keyboard_mode = false;
        if b != Button::Left {
            return;
        }
        self.input.press_at = Some((x, y, Instant::now()));
        // screenshot annotation: start a shape on the image
        if self.expanded && self.page == PageId::Annotate && self.f.annotate.image.is_some() {
            let r = self.f.annotate.img_rect;
            if r.w > 1.0 && r.contains(x, y) {
                let n = ((x - r.x) / r.w, (y - r.y) / r.h);
                let tool = self.f.annotate.tool;
                self.f.annotate.typing = false;
                self.f.annotate.drawing = Some(crate::features::AnnoShape {
                    tool,
                    a: n,
                    b: n,
                    text: String::new(),
                });
                self.dirty = true;
                return;
            }
        }
        if let Some((_, a, r)) = self.zone_at(x, y) {
            if let Action::Slider(k) = a {
                self.input.slider = Some((k, r));
                self.on_slider(k, slider_value(r, x, y), false);
            }
            if let Action::Focus(id) = a {
                self.focus_input(id);
                self.place_caret(id, x, r);
            } else if !matches!(a, Action::Slider(_))
                && self.input.text_focus.is_some()
                && !matches!(
                    a,
                    Action::AskSend
                        | Action::AskMode(_)
                        | Action::AskChip(_)
                        | Action::PaletteItem(_)
                        | Action::NoteCheck(..)
                        | Action::NoteChecklist
                )
            {
                self.input.text_focus = None;
            }
            if long_pressable(&a) {
                self.input.press.down(a.clone(), Instant::now());
            }
            self.diag(format!("down {a:?}"));
            self.input.pressed = Some(a);
        } else {
            self.input.pressed = None;
            if self.floating() && !self.expanded {
                self.input.drag_float = Some((x, y));
            }
        }
        self.dirty = true;
    }

    fn place_caret(&mut self, id: InputId, x: f32, r: Rect) {
        // the notes editor hit-tests its layout each frame: exact placement
        if id == InputId::Note {
            if let Some(i) = self.input.note_hit {
                let t = self.input.text(id);
                t.caret = i.min(t.text.chars().count());
                t.anchor = None;
                return;
            }
        }
        // approximate caret placement by proportional position
        let t = self.input.text(id);
        let n = t.text.chars().count();
        let frac = ((x - r.x - 12.0) / (r.w - 24.0).max(1.0)).clamp(0.0, 1.0);
        t.caret = ((frac * n as f32).round() as usize).min(n);
        t.anchor = None;
    }

    pub fn on_mouse_up(&mut self, x: f32, y: f32, b: Button) -> ClickOutcome {
        self.last_input = Instant::now();
        self.input.drag_float = None;
        match b {
            Button::Right => {
                if let Some(a) = self.zone_at(x, y).map(|z| z.1).filter(long_pressable) {
                    // the secondary action, never also a click
                    let super::press::Outcome::Secondary(a) = self.input.press.secondary(a) else {
                        return ClickOutcome::Handled;
                    };
                    self.diag(format!("right click {a:?}"));
                    self.long_press(a);
                    return ClickOutcome::Handled;
                }
                if self.pill_rect().contains(x, y) || self.expanded {
                    return ClickOutcome::ContextMenu;
                }
                return ClickOutcome::None;
            }
            Button::Middle => {
                self.media_cmd(MediaCmd::PlayPause);
                return ClickOutcome::Handled;
            }
            Button::Left => {}
        }
        if let Some(d) = self.f.annotate.drawing.take() {
            let size = (d.b.0 - d.a.0).abs() + (d.b.1 - d.a.1).abs();
            if d.tool == 4 {
                // text: place a label and start typing into it
                self.f.annotate.shapes.push(d);
                self.f.annotate.typing = true;
            } else if size > 0.01 {
                self.f.annotate.shapes.push(d);
            }
            self.input.pressed = None;
            self.dirty = true;
            return ClickOutcome::Handled;
        }
        if self.f.todo_drag.take().is_some() {
            self.input.pressed = None;
            self.f.mark();
            return ClickOutcome::Handled;
        }
        if let Some((k, r)) = self.input.slider.take() {
            self.on_slider(k, slider_value(r, x, y), true);
            self.input.pressed = None;
            return ClickOutcome::Handled;
        }
        let pressed = self.input.pressed.take();
        // things with a long press: the press machine decides (click, or nothing after a
        // long press / when released elsewhere)
        if pressed.as_ref().is_some_and(long_pressable) || self.input.press.long_fired() {
            let over = self.zone_at(x, y).map(|z| z.1);
            if let Some(super::press::Outcome::Click(a)) = self.input.press.up(over.as_ref()) {
                self.diag(format!("click {a:?}"));
                self.dispatch(a);
            }
            return ClickOutcome::Handled;
        }
        if let Some((_, a, _)) = self.zone_at(x, y) {
            if pressed.as_ref() == Some(&a) {
                self.diag(format!("click {a:?}"));
                if !matches!(a, Action::Focus(_)) {
                    self.dispatch(a);
                }
                return ClickOutcome::Handled;
            }
        }
        if !self.pill_rect().contains(x, y) && !self.expanded {
            return ClickOutcome::None;
        }
        // click on the pill itself
        let g = self.geo();
        if let Some(bub) = g.bubble {
            if bub.contains(x, y) {
                self.act.swap();
                self.dirty = true;
                return ClickOutcome::Handled;
            }
        }
        if !self.expanded {
            if self.mascot.alpha.value > 0.5 {
                self.dispatch(Action::Mascot);
            }
            let p = self.default_page();
            self.open(p);
            return ClickOutcome::Handled;
        }
        ClickOutcome::None
    }

    /// Wheel: volume (Shift = seek) when collapsed, scroll / page switch when expanded.
    pub fn on_wheel(&mut self, delta: f32, horizontal: bool, shift: bool) {
        self.last_input = Instant::now();
        let notches = delta / 120.0;
        if !self.expanded {
            if shift {
                let pos = self.media.position_ms() + (notches * 5000.0) as i64;
                self.media_cmd(MediaCmd::SeekMs(pos.max(0)));
            } else {
                self.audio_cmd(AudioCmd::Step(notches * 0.02));
            }
            return;
        }
        // page changes: horizontal wheel / touchpad swipe, Shift+wheel, or the wheel over the
        // page dots. Travel accumulates (touchpads send many small deltas) and a short
        // cooldown turns one flick into one page.
        let over_dots = self
            .input
            .cursor
            .is_some_and(|(x, y)| self.isl.dock_rect.inset(-8.0).contains(x, y));
        if horizontal || shift || over_dots {
            let step = if horizontal { notches } else { -notches };
            let now = Instant::now();
            let cooling = self
                .input
                .page_wheel_at
                .is_some_and(|t| now.duration_since(t) < Duration::from_millis(260));
            if cooling {
                self.input.page_wheel = 0.0;
                return;
            }
            self.input.page_wheel += step;
            if self.input.page_wheel.abs() >= 0.9 {
                let d = if self.input.page_wheel > 0.0 { 1 } else { -1 };
                self.input.page_wheel = 0.0;
                self.input.page_wheel_at = Some(now);
                self.switch_page(d);
            }
            return;
        }
        let page = self.page;
        // Control Center: scrolling or swiping over a slider changes it
        if page == PageId::Control {
            let hit = self.input.cursor.and_then(|(x, y)| {
                self.input
                    .ctl_sliders
                    .iter()
                    .find(|(r, _)| r.inset(-6.0).contains(x, y))
                    .map(|(_, k)| *k)
            });
            if let Some(k) = hit {
                self.ctl_wheel(k, delta);
                return;
            }
        }
        if page == PageId::Ask {
            let s = &mut self.f.ask.scroll;
            s.set((s.target - notches * 40.0).max(0.0));
        } else if page == PageId::Notes
            && self
                .input
                .cursor
                .is_some_and(|(x, y)| self.input.note_editor.is_some_and(|r| r.contains(x, y)))
        {
            // over the note: scroll the text, not the card list
            let s = &mut self.f.note_scroll;
            s.set((s.target - notches * 40.0).max(0.0));
        } else if page == PageId::Lyrics {
            let s = &mut self.lyrics.scroll;
            s.set((s.target - notches).max(0.0));
        } else {
            let s = self.input.scroll(page);
            s.set((s.target - notches * 48.0).max(0.0));
        }
        self.dirty = true;
    }

    pub fn on_char(&mut self, c: char) {
        if self.f.annotate.typing
            && self.expanded
            && self.page == PageId::Annotate
            && !c.is_control()
        {
            if let Some(s) = self.f.annotate.shapes.last_mut().filter(|s| s.tool == 4) {
                s.text.push(c);
                self.dirty = true;
            }
            return;
        }
        let Some(id) = self.input.text_focus else {
            return;
        };
        if c.is_control() {
            return;
        }
        self.input.text(id).insert(&c.to_string());
        self.input.caret_blink = Instant::now();
        self.after_text_change(id);
    }

    fn after_text_change(&mut self, id: InputId) {
        match id {
            InputId::Ask => self.update_palette(),
            InputId::Note => {
                let text = self.input.text(InputId::Note).text.clone();
                if self.f.notes.current.is_none() {
                    self.f.notes.new_note();
                }
                if let Some(n) = self.f.notes.current_mut() {
                    n.text = text;
                    n.updated_at = crate::util::unix_secs();
                }
                self.f.notes_dirty = true;
                self.f.mark();
            }
            InputId::ClipSearch => self.input.scroll(PageId::Clipboard).snap(0.0),
            InputId::NoteSearch => self.input.scroll(PageId::Notes).snap(0.0),
            _ => {}
        }
        self.dirty = true;
    }

    /// Key handling while the island has keyboard focus. Returns true if consumed.
    pub fn on_key(
        &mut self,
        key: u32,
        ctrl: bool,
        shift: bool,
        clipboard: &dyn Fn() -> Option<String>,
    ) -> bool {
        self.last_input = Instant::now();
        self.dirty = true;
        if self.f.annotate.typing && self.page == PageId::Annotate {
            match key {
                vk::BACK => {
                    if let Some(s) = self.f.annotate.shapes.last_mut().filter(|s| s.tool == 4) {
                        s.text.pop();
                    }
                    return true;
                }
                vk::RETURN | vk::ESCAPE => {
                    self.f.annotate.typing = false;
                    return true;
                }
                _ => {}
            }
        }
        if let Some(id) = self.input.text_focus {
            if self.text_key(id, key, ctrl, shift, clipboard) {
                return true;
            }
        }
        self.input.keyboard_mode = true;
        match key {
            vk::ESCAPE => {
                if self.pop.current.is_some() && !self.expanded {
                    self.pop.dismiss(Instant::now());
                } else if self.expanded && self.page == PageId::Control && self.input.ctl_sheet {
                    // Esc closes the in-use sheet first
                    self.input.ctl_sheet = false;
                } else {
                    self.collapse();
                }
            }
            vk::LEFT | vk::RIGHT if matches!(self.input.focus, Some(Action::Slider(_))) => {
                if let Some(Action::Slider(k)) = self.input.focus.clone() {
                    self.nudge_slider(k, key == vk::RIGHT);
                }
            }
            vk::LEFT => self.switch_page(-1),
            vk::RIGHT => self.switch_page(1),
            vk::TAB | vk::DOWN | vk::UP => {
                let back = key == vk::UP || (key == vk::TAB && shift);
                self.move_focus(back);
            }
            vk::RETURN | vk::SPACE => {
                if let Some(a) = self.input.focus.clone() {
                    match a {
                        Action::Focus(id) => self.focus_input(id),
                        Action::Slider(_) => {}
                        other => self.dispatch(other),
                    }
                }
            }
            0x50 if ctrl => self.pinned = !self.pinned,
            0x31..=0x39 if !ctrl => {
                let i = (key - 0x31) as usize;
                if let Some(p) = self.carousel().get(i).copied() {
                    self.open(p);
                }
            }
            _ => return false,
        }
        true
    }

    fn move_focus(&mut self, back: bool) {
        let focusable: Vec<Action> = self
            .zones
            .iter()
            .filter(|z| z.focusable)
            .map(|z| z.action.clone())
            .collect();
        if focusable.is_empty() {
            return;
        }
        let idx = self
            .input
            .focus
            .as_ref()
            .and_then(|f| focusable.iter().position(|a| a == f));
        let n = focusable.len();
        let next = match (idx, back) {
            (None, false) => 0,
            (None, true) => n - 1,
            (Some(i), false) => (i + 1) % n,
            (Some(i), true) => (i + n - 1) % n,
        };
        self.input.focus = Some(focusable[next].clone());
        if let Some(z) = self.zones.iter().find(|z| z.action == focusable[next]) {
            self.requests.push(Request::Announce(z.name.clone()));
        }
    }

    fn text_key(
        &mut self,
        id: InputId,
        key: u32,
        ctrl: bool,
        shift: bool,
        clipboard: &dyn Fn() -> Option<String>,
    ) -> bool {
        self.input.caret_blink = Instant::now();
        let multiline = id == InputId::Note;
        let t = self.input.text(id);
        let len = t.text.chars().count();
        let mv = |t: &mut super::state::TextField, to: usize, shift: bool| {
            if shift {
                if t.anchor.is_none() {
                    t.anchor = Some(t.caret);
                }
            } else {
                t.anchor = None;
            }
            t.caret = to;
        };
        match key {
            vk::LEFT => {
                let to = if ctrl {
                    prev_word(&t.text, t.caret)
                } else {
                    t.caret.saturating_sub(1)
                };
                mv(t, to, shift);
            }
            vk::RIGHT => {
                let to = if ctrl {
                    next_word(&t.text, t.caret)
                } else {
                    (t.caret + 1).min(len)
                };
                mv(t, to, shift);
            }
            vk::HOME if multiline && !ctrl => {
                let to = crate::features::notetext::line_bounds(&t.text, t.caret).0;
                mv(t, to, shift);
            }
            vk::END if multiline && !ctrl => {
                let to = crate::features::notetext::line_bounds(&t.text, t.caret).1;
                mv(t, to, shift);
            }
            vk::UP | vk::DOWN if multiline => {
                let to = crate::features::notetext::vertical(&t.text, t.caret, key == vk::UP);
                mv(t, to, shift);
            }
            vk::HOME => mv(t, 0, shift),
            vk::END => mv(t, len, shift),
            vk::BACK => {
                t.backspace();
                self.after_text_change(id);
            }
            vk::DELETE => {
                t.delete();
                self.after_text_change(id);
            }
            0x41 if ctrl => {
                t.anchor = Some(0);
                t.caret = len;
            }
            0x43 | 0x58 if ctrl => {
                if let Some((a, b)) = t.selection() {
                    let s: String = t.text.chars().skip(a).take(b - a).collect();
                    if key == 0x58 {
                        t.delete_selection();
                    }
                    self.requests.push(Request::CopyText(s));
                    self.after_text_change(id);
                }
            }
            0x56 if ctrl => {
                if let Some(c) = clipboard() {
                    let c = if multiline {
                        c
                    } else {
                        c.replace(['\r', '\n'], " ")
                    };
                    self.input.text(id).insert(&c);
                    self.after_text_change(id);
                }
            }
            // Ctrl+Shift+Z or Ctrl+Y: redo
            0x5A if ctrl && shift => {
                t.redo();
                self.after_text_change(id);
            }
            0x59 if ctrl => {
                t.redo();
                self.after_text_change(id);
            }
            0x5A if ctrl => {
                t.undo();
                self.after_text_change(id);
            }
            vk::RETURN => {
                if multiline && !ctrl {
                    // inside a checklist or bullet list Enter starts the next item (and an
                    // empty item ends the list)
                    let cont = if t.selection().is_none() {
                        crate::features::notetext::on_enter(&t.text, t.caret)
                    } else {
                        None
                    };
                    match cont {
                        Some((s, c)) => {
                            t.undo.push(t.text.clone());
                            t.text = s;
                            t.caret = c;
                            t.anchor = None;
                        }
                        None => t.insert("\n"),
                    }
                    self.after_text_change(id);
                } else {
                    self.submit_text(id);
                }
            }
            vk::UP | vk::DOWN if id == InputId::Ask => {
                if self.f.ask.mode == AskMode::Palette && !self.f.ask.palette.is_empty() {
                    let n = self.f.ask.palette.len();
                    self.f.ask.palette_sel = if key == vk::UP {
                        (self.f.ask.palette_sel + n - 1) % n
                    } else {
                        (self.f.ask.palette_sel + 1) % n
                    };
                } else {
                    t.history_step(key == vk::UP);
                }
            }
            vk::TAB if id == InputId::Ask => {
                let next = match self.f.ask.mode {
                    AskMode::Ask => 1,
                    AskMode::Translate => 2,
                    AskMode::Palette => 0,
                };
                self.dispatch(Action::AskMode(next));
            }
            vk::ESCAPE => {
                if id == InputId::Ask && self.f.ask.streaming {
                    self.f.ask.stop();
                } else {
                    self.input.text_focus = None;
                    if self.expanded
                        && matches!(id, InputId::Ask | InputId::Todo | InputId::ClipSearch)
                    {
                        self.collapse();
                    }
                }
            }
            _ => return false,
        }
        self.dirty = true;
        true
    }

    pub fn submit_text(&mut self, id: InputId) {
        match id {
            InputId::Todo => {
                let t = self.input.text(InputId::Todo).take();
                let pick = std::mem::take(&mut self.input.todo_pick);
                if let Some(id) = self.f.todo.add_with(&t, pick) {
                    self.f.appear(crate::features::ListKind::Todo, id);
                    self.f.todo_dirty = true;
                    self.f.mark();
                    self.input.todo_picker = false;
                } else {
                    self.input.todo_pick = pick;
                }
            }
            InputId::Ask => self.ask_submit(),
            InputId::NoteSearch => {
                // Enter opens the first match
                let q = self.input.text(InputId::NoteSearch).text.clone();
                let mut hits: Vec<_> = self
                    .f
                    .notes
                    .notes
                    .iter()
                    .filter(|n| crate::features::notetext::matches(&n.text, &q))
                    .collect();
                hits.sort_by_key(|n| !n.pinned);
                if let Some(id) = hits.first().map(|n| n.id) {
                    self.dispatch(Action::NoteSelect(id));
                }
            }
            InputId::ClipSearch => {
                let q = self.input.text(InputId::ClipSearch).text.clone();
                if let Some(first) = self.f.clip.filtered(&q).first().map(|i| i.id) {
                    self.dispatch(Action::ClipCopy(first));
                }
            }
            _ => {}
        }
        self.dirty = true;
    }

    /// A slider value from the keyboard (arrow keys while a slider is focused).
    pub fn nudge_slider(&mut self, k: SliderKind, up: bool) {
        let cur = match k {
            SliderKind::Volume => self.sys.volume,
            SliderKind::Brightness => self.sys.brightness.unwrap_or(50) as f32 / 100.0,
            SliderKind::Seek => self.media.progress(),
            SliderKind::AppVolume => self.media.app_volume.unwrap_or(1.0),
            _ => return,
        };
        self.on_slider(k, cur + if up { 0.05 } else { -0.05 }, true);
    }
}

/// Slider value under the pointer: tall sliders run bottom-to-top, the rest left-to-right.
fn slider_value(r: Rect, x: f32, y: f32) -> f32 {
    if r.h > r.w * 1.5 {
        (r.bottom() - y) / r.h.max(1.0)
    } else {
        (x - r.x) / r.w.max(1.0)
    }
}

fn prev_word(s: &str, caret: usize) -> usize {
    let chars: Vec<char> = s.chars().collect();
    let mut i = caret.min(chars.len());
    while i > 0 && chars[i - 1].is_whitespace() {
        i -= 1;
    }
    while i > 0 && !chars[i - 1].is_whitespace() {
        i -= 1;
    }
    i
}

fn next_word(s: &str, caret: usize) -> usize {
    let chars: Vec<char> = s.chars().collect();
    let mut i = caret;
    while i < chars.len() && !chars[i].is_whitespace() {
        i += 1;
    }
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::state::TextField;
    #[test]
    fn word_nav() {
        assert_eq!(prev_word("hello big world", 15), 10);
        assert_eq!(next_word("hello big world", 0), 6);
    }
    #[test]
    fn text_field_editing() {
        let mut t = TextField::default();
        t.insert("héllo");
        assert_eq!(t.caret, 5);
        t.backspace();
        assert_eq!(t.text, "héll");
        t.anchor = Some(0);
        t.caret = 2;
        t.insert("X");
        assert_eq!(t.text, "Xll");
        t.undo();
        assert_eq!(t.text, "héll");
        let s = t.take();
        assert_eq!(s, "héll");
        t.history_step(true);
        assert_eq!(t.text, "héll");
    }

    #[test]
    fn tall_sliders_run_bottom_to_top() {
        let tall = Rect::new(0.0, 0.0, 50.0, 200.0);
        assert!((slider_value(tall, 25.0, 200.0) - 0.0).abs() < 1e-4);
        assert!((slider_value(tall, 25.0, 50.0) - 0.75).abs() < 1e-4);
        let wide = Rect::new(0.0, 0.0, 200.0, 14.0);
        assert!((slider_value(wide, 50.0, 7.0) - 0.25).abs() < 1e-4);
    }
}
