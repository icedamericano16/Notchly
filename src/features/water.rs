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
    fn schedules_in_future() {
        let p = Productivity::default();
        let mut w = WaterData::default();
        w.schedule(&p);
        assert!(w.next_reminder > unix_secs());
    }
}
