//! Focus timer (Pomodoro). Stores the end timestamp, not a countdown, so it
//! survives sleep and restarts.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::util::{local_now, unix_ms};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Focus,
    ShortBreak,
    LongBreak,
    Custom,
}

impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Phase::Focus => "Focus",
            Phase::ShortBreak => "Short break",
            Phase::LongBreak => "Long break",
            Phase::Custom => "Timer",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct TimerData {
    pub end_ms: Option<i64>,
    pub paused_left_ms: Option<i64>,
    pub total_ms: i64,
    pub phase: Phase,
    /// Completed focus sessions in the current cycle.
    pub session: u32,
    /// Day number -> focus minutes.
    pub history: BTreeMap<i64, u32>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum TimerEvent {
    Finished(Phase),
}

impl TimerData {
    pub fn running(&self) -> bool {
        self.end_ms.is_some()
    }
    pub fn active(&self) -> bool {
        self.end_ms.is_some() || self.paused_left_ms.is_some()
    }
    pub fn remaining_ms(&self) -> i64 {
        if let Some(end) = self.end_ms {
            (end - unix_ms()).max(0)
        } else {
            self.paused_left_ms.unwrap_or(0)
        }
    }
    pub fn progress(&self) -> f32 {
        if self.total_ms <= 0 {
            return 0.0;
        }
        1.0 - self.remaining_ms() as f32 / self.total_ms as f32
    }
    pub fn start(&mut self, phase: Phase, minutes: f64) {
        let ms = (minutes * 60_000.0) as i64;
        self.phase = phase;
        self.total_ms = ms.max(1000);
        self.end_ms = Some(unix_ms() + self.total_ms);
        self.paused_left_ms = None;
    }
    pub fn pause(&mut self) {
        if let Some(_end) = self.end_ms.take() {
            self.paused_left_ms = Some(self.remaining_ms_at_pause(_end));
        }
    }
    fn remaining_ms_at_pause(&self, end: i64) -> i64 {
        (end - unix_ms()).max(0)
    }
    pub fn resume(&mut self) {
        if let Some(left) = self.paused_left_ms.take() {
            self.end_ms = Some(unix_ms() + left);
        }
    }
    pub fn reset(&mut self) {
        self.end_ms = None;
        self.paused_left_ms = None;
        self.total_ms = 0;
    }
    pub fn plus(&mut self, minutes: i64) {
        let add = minutes * 60_000;
        if let Some(e) = self.end_ms.as_mut() {
            *e += add;
            self.total_ms += add;
        } else if let Some(p) = self.paused_left_ms.as_mut() {
            *p += add;
            self.total_ms += add;
        }
    }
    /// Checks for completion. Credits focus minutes to today's history.
    pub fn tick(&mut self) -> Option<TimerEvent> {
        let end = self.end_ms?;
        if unix_ms() < end {
            return None;
        }
        self.end_ms = None;
        self.paused_left_ms = None;
        let phase = self.phase;
        if phase == Phase::Focus {
            self.session += 1;
            let day = local_now().day_number();
            *self.history.entry(day).or_insert(0) += (self.total_ms / 60_000) as u32;
            // keep 60 days
            while self.history.len() > 60 {
                if let Some(k) = self.history.keys().next().copied() {
                    self.history.remove(&k);
                }
            }
        }
        Some(TimerEvent::Finished(phase))
    }
    pub fn today_minutes(&self) -> u32 {
        let mut m = self
            .history
            .get(&local_now().day_number())
            .copied()
            .unwrap_or(0);
        if self.phase == Phase::Focus && self.active() {
            m += ((self.total_ms - self.remaining_ms()) / 60_000) as u32;
        }
        m
    }
    /// Last 7 days of focus minutes, oldest first.
    pub fn week(&self) -> [u32; 7] {
        let today = local_now().day_number();
        std::array::from_fn(|i| {
            self.history
                .get(&(today - 6 + i as i64))
                .copied()
                .unwrap_or(0)
        })
    }
    /// Next phase after the given one finished.
    pub fn next_break(&self, long_every: u32) -> Phase {
        if long_every > 0 && self.session > 0 && self.session.is_multiple_of(long_every) {
            Phase::LongBreak
        } else {
            Phase::ShortBreak
        }
    }
}

/// Parses "12m", "1h30", "90s", "25" (minutes) into minutes.
pub fn parse_duration(s: &str) -> Option<f64> {
    let s = s.trim().to_lowercase();
    if s.is_empty() {
        return None;
    }
    if let Ok(m) = s.parse::<f64>() {
        return (m > 0.0).then_some(m);
    }
    let mut total = 0.0;
    let mut num = String::new();
    let mut any = false;
    for c in s.chars() {
        if c.is_ascii_digit() || c == '.' {
            num.push(c);
        } else if !num.is_empty() {
            let v: f64 = num.parse().ok()?;
            num.clear();
            match c {
                'h' => total += v * 60.0,
                'm' => total += v,
                's' => total += v / 60.0,
                _ => return None,
            }
            any = true;
        }
    }
    if !num.is_empty() {
        // trailing number after hours = minutes ("1h30")
        total += num.parse::<f64>().ok()?;
        any = true;
    }
    (any && total > 0.0).then_some(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn durations() {
        assert_eq!(parse_duration("12m"), Some(12.0));
        assert_eq!(parse_duration("1h30"), Some(90.0));
        assert_eq!(parse_duration("90s"), Some(1.5));
        assert_eq!(parse_duration("25"), Some(25.0));
        assert_eq!(parse_duration("abc"), None);
    }
    #[test]
    fn pause_resume_keeps_remaining() {
        let mut t = TimerData::default();
        t.start(Phase::Focus, 10.0);
        t.pause();
        let left = t.remaining_ms();
        assert!(left > 590_000);
        t.resume();
        assert!(t.running());
        t.plus(5);
        assert!(t.remaining_ms() > 890_000);
    }
    #[test]
    fn finishing_credits_history() {
        let mut t = TimerData::default();
        t.start(Phase::Focus, 25.0);
        t.end_ms = Some(unix_ms() - 1);
        assert_eq!(t.tick(), Some(TimerEvent::Finished(Phase::Focus)));
        assert_eq!(t.session, 1);
        assert!(t.today_minutes() >= 25);
        assert_eq!(t.next_break(4), Phase::ShortBreak);
        t.session = 4;
        assert_eq!(t.next_break(4), Phase::LongBreak);
    }
}
