//! A press on something that has a long-press action (the water bell): each press ends in
//! exactly one outcome — a click, a long press, or nothing — never two.
//!
//! * down → up before the threshold, still over the target → `Click`
//! * down → held past the threshold → `Long` (fires once, while still held); the release
//!   afterwards does nothing
//! * released somewhere else, or cancelled → nothing
//! * a secondary (right) click is its own outcome and never also clicks

use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome<A> {
    Click(A),
    Long(A),
    Secondary(A),
}

#[derive(Debug)]
pub struct Press<A> {
    target: Option<A>,
    at: Option<Instant>,
    long_fired: bool,
}

impl<A> Default for Press<A> {
    fn default() -> Self {
        Self {
            target: None,
            at: None,
            long_fired: false,
        }
    }
}

impl<A: Clone + PartialEq> Press<A> {
    /// Primary button down on `target` (one with a long-press action).
    pub fn down(&mut self, target: A, now: Instant) {
        self.target = Some(target);
        self.at = Some(now);
        self.long_fired = false;
    }

    /// Called every loop turn while held: fires `Long` once when the threshold passes.
    pub fn tick(&mut self, now: Instant, threshold: Duration) -> Option<Outcome<A>> {
        let (Some(t), Some(at)) = (self.target.as_ref(), self.at) else {
            return None;
        };
        if !self.long_fired && now.saturating_duration_since(at) >= threshold {
            self.long_fired = true;
            return Some(Outcome::Long(t.clone()));
        }
        None
    }

    /// Primary button up over `over` (what's under the pointer, if anything).
    pub fn up(&mut self, over: Option<&A>) -> Option<Outcome<A>> {
        let target = self.target.take();
        self.at = None;
        let long = std::mem::take(&mut self.long_fired);
        match target {
            Some(t) if !long && over == Some(&t) => Some(Outcome::Click(t)),
            _ => None,
        }
    }

    /// A right click on `target`: cancels any press in progress.
    pub fn secondary(&mut self, target: A) -> Outcome<A> {
        self.cancel();
        Outcome::Secondary(target)
    }

    pub fn cancel(&mut self) {
        self.target = None;
        self.at = None;
        self.long_fired = false;
    }

    /// Whether the current press already fired its long press.
    pub fn long_fired(&self) -> bool {
        self.long_fired
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: Duration = Duration::from_millis(500);

    fn ms(base: Instant, n: u64) -> Instant {
        base + Duration::from_millis(n)
    }

    #[test]
    fn quick_press_is_one_click() {
        let t0 = Instant::now();
        let mut p = Press::default();
        p.down("bell", t0);
        assert_eq!(p.tick(ms(t0, 100), T), None);
        assert_eq!(p.up(Some(&"bell")), Some(Outcome::Click("bell")));
        // nothing left over
        assert_eq!(p.up(Some(&"bell")), None);
        assert_eq!(p.tick(ms(t0, 900), T), None);
    }

    #[test]
    fn long_press_fires_once_and_release_does_nothing() {
        let t0 = Instant::now();
        let mut p = Press::default();
        p.down("bell", t0);
        assert_eq!(p.tick(ms(t0, 499), T), None);
        assert_eq!(p.tick(ms(t0, 500), T), Some(Outcome::Long("bell")));
        // held longer: no second long press
        assert_eq!(p.tick(ms(t0, 900), T), None);
        assert!(p.long_fired());
        // the release after a long press never clicks
        assert_eq!(p.up(Some(&"bell")), None);
    }

    #[test]
    fn released_elsewhere_does_nothing() {
        let t0 = Instant::now();
        let mut p = Press::default();
        p.down("bell", t0);
        assert_eq!(p.up(Some(&"other")), None);
        p.down("bell", t0);
        assert_eq!(p.up(None), None);
    }

    #[test]
    fn right_click_is_its_own_action() {
        let t0 = Instant::now();
        let mut p = Press::default();
        // a right click during a press cancels it: no click, no long press afterwards
        p.down("bell", t0);
        assert_eq!(p.secondary("bell"), Outcome::Secondary("bell"));
        assert_eq!(p.tick(ms(t0, 900), T), None);
        assert_eq!(p.up(Some(&"bell")), None);
    }

    #[test]
    fn a_new_press_resets() {
        let t0 = Instant::now();
        let mut p = Press::default();
        p.down("bell", t0);
        assert_eq!(p.tick(ms(t0, 600), T), Some(Outcome::Long("bell")));
        // next press starts clean
        p.down("bell", ms(t0, 1000));
        assert!(!p.long_fired());
        assert_eq!(p.up(Some(&"bell")), Some(Outcome::Click("bell")));
    }
}
