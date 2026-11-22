//! Activity Manager (section 6): long-lived activities compete by priority for the
//! collapsed pill; transient pop-ups queue on top with merging and staleness rules.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use super::Action;
use crate::render::icons::Icon;
use crate::theme::tokens::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ActKind {
    Call,
    TimerEnding,
    Recording,
    Focus,
    LiveApi,
    Transfer,
    Music,
    Charging,
    Event,
    Prayer,
    Note,
    SysAlert,
}

impl ActKind {
    pub fn default_priority(self) -> i32 {
        match self {
            ActKind::Call => 100,
            ActKind::TimerEnding => 90,
            ActKind::Recording => 80,
            ActKind::Focus => 70,
            ActKind::LiveApi => 60,
            ActKind::SysAlert => 58,
            ActKind::Transfer => 55,
            ActKind::Music => 50,
            ActKind::Charging => 40,
            ActKind::Event => 30,
            ActKind::Note => 25,
            ActKind::Prayer => 20,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            ActKind::Call => "call",
            ActKind::TimerEnding => "timer_ending",
            ActKind::Recording => "recording",
            ActKind::Focus => "focus",
            ActKind::LiveApi => "live_api",
            ActKind::Transfer => "transfer",
            ActKind::Music => "music",
            ActKind::Charging => "charging",
            ActKind::Event => "event",
            ActKind::Prayer => "prayer",
            ActKind::Note => "note",
            ActKind::SysAlert => "sys_alert",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Activity {
    pub key: String,
    pub kind: ActKind,
    pub priority: i32,
    pub started: u64,
    pub expires: Option<Instant>,
}

#[derive(Default, Debug)]
pub struct ActivityManager {
    list: Vec<Activity>,
    seq: u64,
    /// Locked to the main pill by the user (context menu "Pin").
    pub pinned: Option<String>,
    /// Temporarily promoted by clicking the bubble.
    pub swapped: Option<String>,
}

impl ActivityManager {
    /// Inserts or updates an activity. Updating keeps its start order.
    pub fn upsert(
        &mut self,
        key: &str,
        kind: ActKind,
        priority: i32,
        expires: Option<Instant>,
    ) -> bool {
        if let Some(a) = self.list.iter_mut().find(|a| a.key == key) {
            let changed = a.priority != priority || a.kind != kind;
            a.priority = priority;
            a.kind = kind;
            a.expires = expires;
            return changed;
        }
        self.seq += 1;
        self.list.push(Activity {
            key: key.to_string(),
            kind,
            priority,
            started: self.seq,
            expires,
        });
        true
    }

    pub fn remove(&mut self, key: &str) -> bool {
        let n = self.list.len();
        self.list.retain(|a| a.key != key);
        if self.pinned.as_deref() == Some(key) {
            self.pinned = None;
        }
        if self.swapped.as_deref() == Some(key) {
            self.swapped = None;
        }
        n != self.list.len()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.list.iter().any(|a| a.key == key)
    }

    pub fn get(&self, key: &str) -> Option<&Activity> {
        self.list.iter().find(|a| a.key == key)
    }

    /// Removes expired activities. Returns true if anything changed.
    pub fn expire(&mut self, now: Instant) -> bool {
        let expired: Vec<String> = self
            .list
            .iter()
            .filter(|a| a.expires.is_some_and(|e| now >= e))
            .map(|a| a.key.clone())
            .collect();
        for k in &expired {
            self.remove(k);
        }
        !expired.is_empty()
    }

    pub fn next_expiry(&self) -> Option<Instant> {
        self.list.iter().filter_map(|a| a.expires).min()
    }

    /// Activities in display order: pinned, swapped, then priority, then most recent.
    pub fn ordered(&self) -> Vec<&Activity> {
        let mut v: Vec<&Activity> = self.list.iter().collect();
        v.sort_by(|a, b| {
            let rank = |x: &Activity| {
                if self.pinned.as_deref() == Some(&x.key) {
                    2
                } else if self.swapped.as_deref() == Some(&x.key) {
                    1
                } else {
                    0
                }
            };
            rank(b)
                .cmp(&rank(a))
                .then(b.priority.cmp(&a.priority))
                .then(b.started.cmp(&a.started))
        });
        v
    }

    pub fn main(&self) -> Option<&Activity> {
        self.ordered().into_iter().next()
    }

    pub fn secondary(&self) -> Option<&Activity> {
        self.ordered().into_iter().nth(1)
    }

    /// Clicking the bubble swaps the two.
    pub fn swap(&mut self) {
        if let Some(s) = self.secondary().map(|a| a.key.clone()) {
            self.swapped = Some(s);
        }
    }

    pub fn toggle_pin(&mut self, key: &str) {
        if self.pinned.as_deref() == Some(key) {
            self.pinned = None;
        } else {
            self.pinned = Some(key.to_string());
        }
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }
}

// ---------------------------------------------------------------- pop-ups

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PopKind {
    Volume,
    Brightness,
    Caps,
    Num,
    Language,
    Battery,
    Network,
    AudioDevice,
    Bluetooth,
    Mic,
    Notification,
    TimerDone,
    Reminder,
    Info,
    Approval,
    Download,
    Digest,
}

impl PopKind {
    /// Kinds whose content is stale after 3 s (dropped rather than queued).
    pub fn stale_fast(self) -> bool {
        matches!(
            self,
            PopKind::Volume
                | PopKind::Brightness
                | PopKind::Caps
                | PopKind::Num
                | PopKind::Language
                | PopKind::Mic
        )
    }
}

#[derive(Clone, Debug)]
pub struct Popup {
    pub id: u32,
    pub kind: PopKind,
    pub icon: Icon,
    pub title: String,
    pub subtitle: String,
    /// Optional slider value 0..1.
    pub value: Option<f32>,
    /// Optional badge text drawn instead of the icon (e.g. "EN").
    pub badge: Option<String>,
    pub tint: Option<Color>,
    pub actions: Vec<(String, Action)>,
    pub priority: i32,
    pub duration: Duration,
    pub critical: bool,
    pub created: Instant,
    pub shown_at: Option<Instant>,
    /// App id for notifications (merge key).
    pub source: String,
}

impl Popup {
    pub fn new(
        kind: PopKind,
        icon: Icon,
        title: impl Into<String>,
        subtitle: impl Into<String>,
    ) -> Self {
        let priority = match kind {
            PopKind::TimerDone | PopKind::Approval => 90,
            PopKind::Notification => 60,
            PopKind::Reminder => 70,
            PopKind::Battery => 55,
            _ => 40,
        };
        Self {
            id: 0,
            kind,
            icon,
            title: title.into(),
            subtitle: subtitle.into(),
            value: None,
            badge: None,
            tint: None,
            actions: Vec::new(),
            priority,
            duration: Duration::from_millis(1600),
            critical: false,
            created: Instant::now(),
            shown_at: None,
            source: String::new(),
        }
    }
    pub fn value(mut self, v: f32) -> Self {
        self.value = Some(v);
        self
    }
    pub fn duration_ms(mut self, ms: u64) -> Self {
        self.duration = Duration::from_millis(ms);
        self
    }
    pub fn action(mut self, label: impl Into<String>, a: Action) -> Self {
        self.actions.push((label.into(), a));
        self
    }
    pub fn critical(mut self) -> Self {
        self.critical = true;
        self.priority = self.priority.max(95);
        self
    }
    pub fn tint(mut self, c: Color) -> Self {
        self.tint = Some(c);
        self
    }
    pub fn badge(mut self, b: impl Into<String>) -> Self {
        self.badge = Some(b.into());
        self
    }
    pub fn priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }
    fn merges_with(&self, o: &Popup) -> bool {
        self.kind == o.kind
            && (self.kind != PopKind::Notification || self.source == o.source)
            && self.kind != PopKind::Approval
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum PushResult {
    /// A new pop-up is now showing (play the arrival animation).
    Shown,
    /// The visible pop-up was updated in place (no re-entry animation).
    Merged,
    Queued,
    Dropped,
}

#[derive(Default, Debug)]
pub struct PopupQueue {
    pub current: Option<Popup>,
    pub queue: VecDeque<Popup>,
    next_id: u32,
}

pub const MAX_QUEUE: usize = 3;
const STALE: Duration = Duration::from_secs(3);

impl PopupQueue {
    pub fn push(&mut self, mut p: Popup, now: Instant) -> PushResult {
        self.next_id = self.next_id.wrapping_add(1);
        p.id = self.next_id;
        if let Some(cur) = self.current.as_mut() {
            if cur.merges_with(&p) {
                let id = cur.id;
                let shown = cur.shown_at;
                *cur = p;
                cur.id = id;
                cur.shown_at = shown;
                // the hide timer resets
                cur.shown_at = Some(now);
                return PushResult::Merged;
            }
            if p.priority > cur.priority && !cur.critical {
                // preempt: the current one goes back to the queue front unless stale
                let mut old = self.current.take().expect("current");
                if !old.kind.stale_fast() {
                    old.shown_at = None;
                    self.queue.push_front(old);
                }
                p.shown_at = Some(now);
                self.current = Some(p);
                self.trim();
                return PushResult::Shown;
            }
            // lower priority: merge into queue or drop if it would be stale
            if p.kind.stale_fast() {
                let remaining = cur
                    .shown_at
                    .map(|s| cur.duration.saturating_sub(now - s))
                    .unwrap_or(cur.duration);
                if remaining > STALE {
                    return PushResult::Dropped;
                }
            }
            if let Some(q) = self.queue.iter_mut().find(|q| q.merges_with(&p)) {
                *q = p;
                return PushResult::Queued;
            }
            self.queue.push_back(p);
            self.trim();
            return PushResult::Queued;
        }
        p.shown_at = Some(now);
        self.current = Some(p);
        PushResult::Shown
    }

    fn trim(&mut self) {
        while self.queue.len() > MAX_QUEUE {
            // drop the lowest priority (oldest among equals)
            if let Some((i, _)) = self
                .queue
                .iter()
                .enumerate()
                .min_by_key(|(i, p)| (p.priority, std::cmp::Reverse(*i)))
            {
                self.queue.remove(i);
            }
        }
    }

    /// Advances: hides an expired pop-up and promotes the next fresh one.
    /// Returns true when the visible pop-up changed.
    pub fn tick(&mut self, now: Instant, hold: bool) -> bool {
        let expired = match &self.current {
            Some(c) => !hold && c.shown_at.is_some_and(|s| now >= s + c.duration),
            None => false,
        };
        if !expired && self.current.is_some() {
            return false;
        }
        let had = self.current.take().is_some();
        while let Some(mut n) = self.queue.pop_front() {
            if n.kind.stale_fast() && now.duration_since(n.created) > STALE {
                continue;
            }
            n.shown_at = Some(now);
            self.current = Some(n);
            return true;
        }
        had
    }

    pub fn dismiss(&mut self, now: Instant) -> bool {
        if let Some(c) = self.current.as_mut() {
            c.duration = Duration::ZERO;
            c.shown_at = Some(now - Duration::from_millis(1));
        }
        self.tick(now, false)
    }

    pub fn deadline(&self) -> Option<Instant> {
        self.current
            .as_ref()
            .and_then(|c| c.shown_at.map(|s| s + c.duration))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highest_priority_wins_and_bubble_is_second() {
        let mut m = ActivityManager::default();
        m.upsert("music", ActKind::Music, 50, None);
        m.upsert("timer", ActKind::Focus, 70, None);
        assert_eq!(m.main().unwrap().key, "timer");
        assert_eq!(m.secondary().unwrap().key, "music");
        m.swap();
        assert_eq!(m.main().unwrap().key, "music");
        m.remove("music");
        assert_eq!(m.main().unwrap().key, "timer");
        assert!(m.swapped.is_none());
    }

    #[test]
    fn equal_priority_most_recent_wins_and_pin_locks() {
        let mut m = ActivityManager::default();
        m.upsert("a", ActKind::LiveApi, 60, None);
        m.upsert("b", ActKind::LiveApi, 60, None);
        assert_eq!(m.main().unwrap().key, "b");
        m.toggle_pin("a");
        m.upsert("c", ActKind::Call, 100, None);
        assert_eq!(m.main().unwrap().key, "a");
    }

    #[test]
    fn expiry_removes() {
        let mut m = ActivityManager::default();
        let now = Instant::now();
        m.upsert(
            "x",
            ActKind::Charging,
            40,
            Some(now + Duration::from_secs(5)),
        );
        assert!(!m.expire(now));
        assert!(m.expire(now + Duration::from_secs(6)));
        assert!(m.is_empty());
    }

    #[test]
    fn same_kind_popups_merge() {
        let mut q = PopupQueue::default();
        let now = Instant::now();
        assert_eq!(
            q.push(
                Popup::new(PopKind::Volume, Icon::Volume2, "Volume", "").value(0.3),
                now
            ),
            PushResult::Shown
        );
        for i in 0..5 {
            assert_eq!(
                q.push(
                    Popup::new(PopKind::Volume, Icon::Volume2, "Volume", "")
                        .value(0.3 + i as f32 * 0.02),
                    now
                ),
                PushResult::Merged
            );
        }
        assert!(q.queue.is_empty());
        assert!((q.current.as_ref().unwrap().value.unwrap() - 0.38).abs() < 1e-5);
    }

    #[test]
    fn queue_caps_at_three_and_promotes() {
        let mut q = PopupQueue::default();
        let now = Instant::now();
        q.push(
            Popup::new(PopKind::TimerDone, Icon::Timer, "Done", "").duration_ms(8000),
            now,
        );
        for k in [
            PopKind::Battery,
            PopKind::Network,
            PopKind::AudioDevice,
            PopKind::Bluetooth,
        ] {
            q.push(Popup::new(k, Icon::Bolt, "x", ""), now);
        }
        assert_eq!(q.queue.len(), MAX_QUEUE);
        assert!(q.tick(now + Duration::from_secs(9), false));
        assert_ne!(q.current.as_ref().unwrap().kind, PopKind::TimerDone);
    }

    #[test]
    fn stale_volume_is_dropped_behind_long_popup() {
        let mut q = PopupQueue::default();
        let now = Instant::now();
        q.push(
            Popup::new(PopKind::TimerDone, Icon::Timer, "Done", "").duration_ms(8000),
            now,
        );
        assert_eq!(
            q.push(
                Popup::new(PopKind::Volume, Icon::Volume2, "Volume", ""),
                now
            ),
            PushResult::Dropped
        );
    }

    #[test]
    fn higher_priority_preempts() {
        let mut q = PopupQueue::default();
        let now = Instant::now();
        q.push(Popup::new(PopKind::Network, Icon::Wifi3, "Wi-Fi", ""), now);
        assert_eq!(
            q.push(
                Popup::new(PopKind::Battery, Icon::Bolt, "Low", "").critical(),
                now
            ),
            PushResult::Shown
        );
        assert_eq!(q.queue.len(), 1);
    }

    #[test]
    fn hold_keeps_popup() {
        let mut q = PopupQueue::default();
        let now = Instant::now();
        q.push(Popup::new(PopKind::Info, Icon::Bell, "x", ""), now);
        assert!(!q.tick(now + Duration::from_secs(10), true));
        assert!(q.current.is_some());
    }
}
