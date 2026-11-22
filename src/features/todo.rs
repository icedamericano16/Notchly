//! To-do list with local natural-language time parsing.

use serde::{Deserialize, Serialize};

use crate::util::{local_now, local_to_unix, unix_secs, weekday_of, LocalTime};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Repeat {
    Daily,
    Weekly,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Task {
    pub id: u64,
    pub title: String,
    /// Unix seconds.
    pub due: Option<i64>,
    pub repeat: Option<Repeat>,
    pub done: bool,
    pub created: i64,
    pub done_at: Option<i64>,
    #[serde(default)]
    pub notified: bool,
    #[serde(default)]
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct TodoList {
    pub tasks: Vec<Task>,
    pub next_id: u64,
}

impl TodoList {
    pub fn add(&mut self, input: &str) -> Option<u64> {
        let (title, due, repeat) = parse_task(input, local_now());
        if title.trim().is_empty() {
            return None;
        }
        self.next_id += 1;
        let now = unix_secs();
        self.tasks.insert(
            0,
            Task {
                id: self.next_id,
                title,
                due,
                repeat,
                done: false,
                created: now,
                done_at: None,
                notified: false,
                updated_at: now,
            },
        );
        Some(self.next_id)
    }
    pub fn toggle(&mut self, id: u64) {
        let now = unix_secs();
        let Some(i) = self.tasks.iter().position(|t| t.id == id) else {
            return;
        };
        let t = &mut self.tasks[i];
        t.updated_at = now;
        if !t.done {
            if let (Some(rep), Some(due)) = (t.repeat, t.due) {
                // repeating: advance the due date instead of completing
                let step = if rep == Repeat::Daily {
                    86400
                } else {
                    7 * 86400
                };
                let mut d = due + step;
                while d < now {
                    d += step;
                }
                t.due = Some(d);
                t.notified = false;
                t.done_at = Some(now);
                return;
            }
            t.done = true;
            t.done_at = Some(now);
        } else {
            t.done = false;
            t.done_at = None;
        }
    }
    pub fn delete(&mut self, id: u64) {
        self.tasks.retain(|t| t.id != id);
    }
    pub fn snooze(&mut self, id: u64, minutes: i64) {
        if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
            t.due = Some(unix_secs() + minutes * 60);
            t.notified = false;
            t.updated_at = unix_secs();
        }
    }
    pub fn move_item(&mut self, from: usize, to: usize) {
        if from < self.tasks.len() && to < self.tasks.len() && from != to {
            let t = self.tasks.remove(from);
            self.tasks.insert(to, t);
        }
    }
    pub fn open_count(&self) -> usize {
        self.tasks.iter().filter(|t| !t.done).count()
    }
    pub fn done_today(&self) -> usize {
        let today = local_now().day_number();
        self.tasks
            .iter()
            .filter(|t| {
                t.done_at
                    .is_some_and(|d| crate::util::unix_to_local(d).day_number() == today)
            })
            .count()
    }
    /// Tasks that just became due (not yet notified).
    pub fn due_now(&mut self) -> Vec<Task> {
        let now = unix_secs();
        let mut out = Vec::new();
        for t in self.tasks.iter_mut() {
            if !t.done && !t.notified && t.due.is_some_and(|d| d <= now && now - d < 3600) {
                t.notified = true;
                out.push(t.clone());
            }
        }
        out
    }
    pub fn next_due(&self) -> Option<i64> {
        self.tasks
            .iter()
            .filter(|t| !t.done && !t.notified)
            .filter_map(|t| t.due)
            .filter(|d| *d > unix_secs() - 3600)
            .min()
    }
}

/// Parses "call mom 6pm", "tomorrow 9", "dentist fri 14:30", "standup every day 9am",
/// "pay rent in 2 hours" into (title, due, repeat). Purely local.
pub fn parse_task(input: &str, now: LocalTime) -> (String, Option<i64>, Option<Repeat>) {
    let words: Vec<&str> = input.split_whitespace().collect();
    let mut keep = vec![true; words.len()];
    let mut day_off: Option<i64> = None;
    let mut minutes: Option<u32> = None;
    let mut rel_secs: Option<i64> = None;
    let mut repeat = None;
    let lw: Vec<String> = words
        .iter()
        .map(|w| {
            w.to_lowercase()
                .trim_matches(|c: char| c == ',' || c == '.')
                .to_string()
        })
        .collect();
    let days = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];
    let mut i = 0;
    while i < lw.len() {
        let w = lw[i].as_str();
        match w {
            "today" | "tonight" => {
                day_off = Some(0);
                keep[i] = false;
                if w == "tonight" && minutes.is_none() {
                    minutes = Some(20 * 60);
                }
            }
            "tomorrow" | "tmr" | "tmrw" => {
                day_off = Some(1);
                keep[i] = false;
            }
            "every" if i + 1 < lw.len() => {
                let n = lw[i + 1].as_str();
                if n == "day" || n == "morning" || n == "evening" {
                    repeat = Some(Repeat::Daily);
                    keep[i] = false;
                    keep[i + 1] = false;
                    i += 1;
                } else if n == "week" || days.iter().any(|d| n.starts_with(d)) {
                    repeat = Some(Repeat::Weekly);
                    keep[i] = false;
                    if n == "week" {
                        keep[i + 1] = false;
                        i += 1;
                    }
                }
            }
            "daily" => {
                repeat = Some(Repeat::Daily);
                keep[i] = false;
            }
            "weekly" => {
                repeat = Some(Repeat::Weekly);
                keep[i] = false;
            }
            "in" if i + 2 < lw.len() => {
                if let Ok(n) = lw[i + 1].parse::<i64>() {
                    let unit = lw[i + 2].as_str();
                    let secs = if unit.starts_with("min") || unit == "m" {
                        Some(n * 60)
                    } else if unit.starts_with("hour") || unit.starts_with("hr") || unit == "h" {
                        Some(n * 3600)
                    } else if unit.starts_with("day") {
                        Some(n * 86400)
                    } else {
                        None
                    };
                    if let Some(s) = secs {
                        rel_secs = Some(s);
                        keep[i] = false;
                        keep[i + 1] = false;
                        keep[i + 2] = false;
                        i += 2;
                    }
                }
            }
            "at" | "by" | "on" => {
                // only drop if followed by a time/day word
                if i + 1 < lw.len()
                    && (parse_clock(&lw[i + 1]).is_some()
                        || days.iter().any(|d| lw[i + 1].starts_with(d))
                        || lw[i + 1] == "noon")
                {
                    keep[i] = false;
                }
            }
            "noon" => {
                minutes = Some(12 * 60);
                keep[i] = false;
            }
            "midnight" => {
                minutes = Some(0);
                keep[i] = false;
            }
            "morning" if !keep.is_empty() && day_off.is_some() => {
                minutes = Some(9 * 60);
                keep[i] = false;
            }
            "evening" if day_off.is_some() => {
                minutes = Some(18 * 60);
                keep[i] = false;
            }
            _ => {
                if let Some(di) = days.iter().position(|d| {
                    w.starts_with(d)
                        && w.len() <= 9
                        && (w.len() == 3
                            || w.ends_with("day")
                            || w == "tues"
                            || w == "thur"
                            || w == "thurs")
                }) {
                    let cur = now.weekday as i64;
                    let mut off = (di as i64 - cur + 7) % 7;
                    if off == 0 {
                        off = 7;
                    }
                    day_off = Some(off);
                    keep[i] = false;
                } else if let Some(m) = parse_clock(w) {
                    // a bare number only counts as a time right after a day word or "at"
                    let bare = w.chars().all(|c| c.is_ascii_digit());
                    let prev_is_marker =
                        i > 0 && (!keep[i - 1] || matches!(lw[i - 1].as_str(), "at" | "by"));
                    if !bare || prev_is_marker {
                        // "5 pm" split across two words
                        minutes = Some(m);
                        keep[i] = false;
                    }
                } else if (w == "am" || w == "pm") && i > 0 {
                    if let Ok(h) = lw[i - 1].parse::<u32>() {
                        if (1..=12).contains(&h) {
                            let h24 = (h % 12) + if w == "pm" { 12 } else { 0 };
                            minutes = Some(h24 * 60);
                            keep[i] = false;
                            keep[i - 1] = false;
                        }
                    }
                }
            }
        }
        i += 1;
    }
    let title: Vec<&str> = words
        .iter()
        .zip(&keep)
        .filter(|(_, k)| **k)
        .map(|(w, _)| *w)
        .collect();
    let title = title.join(" ");
    let today = now.day_number();
    let due = if let Some(s) = rel_secs {
        Some(unix_secs() + s)
    } else if day_off.is_some() || minutes.is_some() {
        let mut d = today + day_off.unwrap_or(0);
        let m = minutes.unwrap_or(9 * 60);
        // a time earlier than now with no day word means tomorrow
        if day_off.is_none() && m <= now.minutes_of_day() {
            d += 1;
        }
        Some(local_to_unix(d, m))
    } else {
        None
    };
    let _ = weekday_of(today);
    (title, due, repeat)
}

/// "6pm", "6:30pm", "18:30", "9am", "9" → minutes of day.
pub fn parse_clock(w: &str) -> Option<u32> {
    let w = w.trim().to_lowercase();
    let (body, ampm) = if let Some(b) = w.strip_suffix("am") {
        (b, Some(false))
    } else if let Some(b) = w.strip_suffix("pm") {
        (b, Some(true))
    } else {
        (w.as_str(), None)
    };
    if body.is_empty() {
        return None;
    }
    let (h, m) = match body.split_once([':', '.']) {
        Some((h, m)) => (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?),
        None => {
            if !body.chars().all(|c| c.is_ascii_digit()) || body.len() > 2 {
                return None;
            }
            (body.parse::<u32>().ok()?, 0)
        }
    };
    if m >= 60 {
        return None;
    }
    let h = match ampm {
        Some(pm) => {
            if !(1..=12).contains(&h) {
                return None;
            }
            (h % 12) + if pm { 12 } else { 0 }
        }
        None => {
            if h > 23 {
                return None;
            }
            h
        }
    };
    Some(h * 60 + m)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn at(hour: u32, weekday: u32) -> LocalTime {
        LocalTime {
            year: 2026,
            month: 11,
            day: 22,
            hour,
            minute: 0,
            second: 0,
            millis: 0,
            weekday,
        }
    }
    fn mins(due: i64) -> u32 {
        crate::util::unix_to_local(due).minutes_of_day()
    }
    #[test]
    fn clock_words() {
        assert_eq!(parse_clock("6pm"), Some(18 * 60));
        assert_eq!(parse_clock("6:30pm"), Some(18 * 60 + 30));
        assert_eq!(parse_clock("18:30"), Some(18 * 60 + 30));
        assert_eq!(parse_clock("12am"), Some(0));
        assert_eq!(parse_clock("25"), None);
    }
    #[test]
    fn call_mom_6pm() {
        let (t, due, r) = parse_task("call mom 6pm", at(10, 0));
        assert_eq!(t, "call mom");
        assert_eq!(mins(due.unwrap()), 18 * 60);
        assert_eq!(r, None);
    }
    #[test]
    fn tomorrow_9() {
        let now = at(10, 0);
        let (t, due, _) = parse_task("tomorrow 9 standup", now);
        assert_eq!(t, "standup");
        let lt = crate::util::unix_to_local(due.unwrap());
        assert_eq!(lt.minutes_of_day(), 9 * 60);
        assert_eq!(lt.day_number(), now.day_number() + 1);
    }
    #[test]
    fn weekday_and_repeat() {
        let now = at(10, 0); // Sunday
        let (t, due, _) = parse_task("dentist fri 14:30", now);
        assert_eq!(t, "dentist");
        assert_eq!(
            crate::util::unix_to_local(due.unwrap()).day_number(),
            now.day_number() + 5
        );
        let (t, _, r) = parse_task("water plants every day 8am", now);
        assert_eq!(t, "water plants");
        assert_eq!(r, Some(Repeat::Daily));
    }
    #[test]
    fn plain_title_has_no_due() {
        let (t, due, _) = parse_task("buy 2 apples", at(10, 0));
        assert_eq!(t, "buy 2 apples");
        assert_eq!(due, None);
    }
    #[test]
    fn split_am_pm() {
        let (t, due, _) = parse_task("gym at 7 pm", at(10, 0));
        assert_eq!(t, "gym");
        assert_eq!(mins(due.unwrap()), 19 * 60);
    }
    #[test]
    fn relative() {
        let (t, due, _) = parse_task("check oven in 20 min", at(10, 0));
        assert_eq!(t, "check oven");
        assert!((due.unwrap() - unix_secs() - 1200).abs() < 5);
    }
    #[test]
    fn repeating_toggle_advances() {
        let mut l = TodoList::default();
        let id = l.add("standup every day 9am").unwrap();
        let before = l.tasks[0].due.unwrap();
        l.toggle(id);
        assert!(!l.tasks[0].done);
        assert!(l.tasks[0].due.unwrap() > before);
    }
}
