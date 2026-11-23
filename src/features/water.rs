//! Water reminder: daily goal, reminders spread evenly over the active window.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::settings::Productivity;
use crate::util::{local_now, local_to_unix, unix_secs};

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct WaterData {
    pub day: i64,
    pub ml: u32,
    pub log: Vec<u32>,
    pub history: BTreeMap<i64, u32>,
    /// Unix seconds of the next reminder.
    pub next_reminder: i64,
    /// Snoozed from the page or a pop-up until this time (unix seconds; 0 = not snoozed).
    pub snoozed_until: i64,
    /// Reminders paused for this day ("Off for today"; they resume the next day).
    pub off_day: i64,
}

/// What the reminder bell shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reminders {
    Off,
    On,
    Snoozed,
    OffToday,
    /// On, and today's goal is met: no more reminders until tomorrow.
    Done,
}

impl WaterData {
    /// Rolls over to a new day if needed.
    pub fn roll(&mut self) {
        let today = local_now().day_number();
        if self.day != today {
            if self.day != 0 && self.ml > 0 {
                self.history.insert(self.day, self.ml);
            }
            while self.history.len() > 60 {
                if let Some(k) = self.history.keys().next().copied() {
                    self.history.remove(&k);
                }
            }
            self.day = today;
            self.ml = 0;
            self.log.clear();
            self.next_reminder = 0;
        }
    }
    pub fn add(&mut self, ml: u32) {
        self.roll();
        self.ml += ml;
        self.log.push(ml);
    }
    pub fn undo(&mut self) {
        if let Some(ml) = self.log.pop() {
            self.ml = self.ml.saturating_sub(ml);
        }
    }
    pub fn week(&self) -> [u32; 7] {
        let today = local_now().day_number();
        std::array::from_fn(|i| {
            let d = today - 6 + i as i64;
            if d == self.day {
                self.ml
            } else {
                self.history.get(&d).copied().unwrap_or(0)
            }
        })
    }
    /// Interval between reminders in seconds.
    pub fn interval(p: &Productivity) -> i64 {
        let glasses = (p.water_goal_ml / p.water_glass_ml.max(1)).max(1) as i64;
        let window = (p.water_end_min as i64 - p.water_start_min as i64).max(60) * 60;
        (window / glasses).max(15 * 60)
    }
    /// Schedules the next reminder after now (within the active window).
    pub fn schedule(&mut self, p: &Productivity) {
        self.roll();
        let now = unix_secs();
        let today = local_now().day_number();
        let start = local_to_unix(today, p.water_start_min);
        let end = local_to_unix(today, p.water_end_min);
        let iv = Self::interval(p);
        if self.ml >= p.water_goal_ml {
            self.next_reminder = local_to_unix(today + 1, p.water_start_min) + iv;
            return;
        }
        let mut t = if now < start { start + iv } else { now + iv };
        if t > end {
            t = local_to_unix(today + 1, p.water_start_min) + iv;
        }
        self.next_reminder = t;
    }
    pub fn snooze(&mut self, minutes: i64) {
        self.next_reminder = unix_secs() + minutes * 60;
        self.snoozed_until = self.next_reminder;
    }
    /// Reminders switched off: nothing is scheduled (no wake-ups at all).
    pub fn reminders_off(&mut self) {
        self.next_reminder = 0;
        self.snoozed_until = 0;
    }
    /// Reminders switched on: the next one is scheduled straight away.
    pub fn reminders_on(&mut self, p: &Productivity) {
        self.snoozed_until = 0;
        self.off_day = 0;
        self.schedule(p);
    }
    /// No reminders for the rest of today; tomorrow's active hours start them again.
    pub fn off_for_today(&mut self) {
        self.roll();
        self.off_day = self.day;
        self.reminders_off();
    }
    pub fn paused_today(&self) -> bool {
        self.off_day != 0 && self.off_day == local_now().day_number()
    }
    pub fn snoozed(&self) -> bool {
        self.snoozed_until > unix_secs()
    }
    /// The bell's state for the setting `on`.
    pub fn state(&self, on: bool, p: &Productivity) -> Reminders {
        if !on {
            Reminders::Off
        } else if self.paused_today() {
            Reminders::OffToday
        } else if self.snoozed() {
            Reminders::Snoozed
        } else if self.ml >= p.water_goal_ml {
            Reminders::Done
        } else {
            Reminders::On
        }
    }
    pub fn due(&self) -> bool {
        self.next_reminder > 0 && unix_secs() >= self.next_reminder
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn add_undo() {
        let mut w = WaterData::default();
        w.add(250);
        w.add(250);
        w.undo();
        assert_eq!(w.ml, 250);
    }
    #[test]
    fn interval_spreads() {
        let p = Productivity::default();
        // 12 h window, 8 glasses => 90 min
        assert_eq!(WaterData::interval(&p), 90 * 60);
    }
    #[test]
    fn switching_reminders() {
        let p = Productivity::default();
        let mut w = WaterData::default();
        w.reminders_on(&p);
        assert!(w.next_reminder > unix_secs());
        assert_eq!(w.state(true, &p), Reminders::On);
        // off: nothing scheduled, so nothing wakes the app
        w.reminders_off();
        assert_eq!(w.next_reminder, 0);
        assert_eq!(w.state(false, &p), Reminders::Off);
        // snooze
        w.reminders_on(&p);
        w.snooze(30);
        assert!(w.snoozed());
        assert_eq!(w.state(true, &p), Reminders::Snoozed);
        assert!((w.next_reminder - unix_secs() - 30 * 60).abs() <= 1);
        // off for today: paused now, back tomorrow
        w.off_for_today();
        assert!(w.paused_today() && w.next_reminder == 0);
        assert_eq!(w.state(true, &p), Reminders::OffToday);
        w.off_day -= 1; // as if a day passed
        assert!(!w.paused_today());
        // turning on clears a pause
        w.off_for_today();
        w.reminders_on(&p);
        assert!(!w.paused_today());
        // goal reached
        w.add(p.water_goal_ml);
        assert_eq!(w.state(true, &p), Reminders::Done);
    }

    #[test]
    fn schedules_in_future() {
        let p = Productivity::default();
        let mut w = WaterData::default();
        w.schedule(&p);
        assert!(w.next_reminder > unix_secs());
    }
}
