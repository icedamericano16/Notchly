//! "Follow the mouse": the notch moves to the display the pointer settles on. Only while the
//! notch is idle (not open, pinned or being dragged), and only after the pointer has stayed
//! on the other display for a moment, so crossing a display on the way somewhere doesn't
//! make it jump.

use std::time::{Duration, Instant};

/// A display, by its monitor handle.
pub type Area = isize;

/// How long the pointer must stay on another display before the notch follows.
pub const SETTLE: Duration = Duration::from_millis(350);

#[derive(Debug, Default)]
pub struct Follow {
    pending: Option<(Area, Instant)>,
}

impl Follow {
    /// `current` is the notch's display, `under` the one under the pointer. Returns the
    /// display to move to, when it's time.
    pub fn update(
        &mut self,
        current: Area,
        under: Option<Area>,
        busy: bool,
        now: Instant,
    ) -> Option<Area> {
        let Some(under) = under.filter(|u| *u != current && !busy) else {
            self.pending = None;
            return None;
        };
        match self.pending {
            Some((a, since)) if a == under => {
                if now.duration_since(since) >= SETTLE {
                    self.pending = None;
                    Some(under)
                } else {
                    None
                }
            }
            _ => {
                self.pending = Some((under, now));
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEFT: Area = 0x10001;
    const RIGHT: Area = 0x10003;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn follows_after_the_pointer_settles() {
        let t0 = Instant::now();
        let mut f = Follow::default();
        assert_eq!(f.update(LEFT, Some(RIGHT), false, t0), None);
        assert_eq!(f.update(LEFT, Some(RIGHT), false, t0 + ms(200)), None);
        assert_eq!(f.update(LEFT, Some(RIGHT), false, t0 + SETTLE), Some(RIGHT));
        // already there: nothing more to do
        assert_eq!(f.update(RIGHT, Some(RIGHT), false, t0 + ms(800)), None);
    }

    #[test]
    fn passing_through_does_not_move_it() {
        let t0 = Instant::now();
        let mut f = Follow::default();
        f.update(LEFT, Some(RIGHT), false, t0);
        // back on the notch's display before it settled
        assert_eq!(f.update(LEFT, Some(LEFT), false, t0 + ms(200)), None);
        assert_eq!(f.update(LEFT, Some(RIGHT), false, t0 + ms(400)), None);
        assert_eq!(f.update(LEFT, Some(RIGHT), false, t0 + ms(600)), None);
        assert_eq!(
            f.update(LEFT, Some(RIGHT), false, t0 + ms(800)),
            Some(RIGHT)
        );
    }

    #[test]
    fn never_while_open_or_pinned() {
        let t0 = Instant::now();
        let mut f = Follow::default();
        for i in 0..10 {
            assert_eq!(f.update(LEFT, Some(RIGHT), true, t0 + ms(100 * i)), None);
        }
        // closed again: the settle time starts over
        assert_eq!(f.update(LEFT, Some(RIGHT), false, t0 + ms(1000)), None);
        assert_eq!(
            f.update(LEFT, Some(RIGHT), false, t0 + ms(1400)),
            Some(RIGHT)
        );
        assert_eq!(f.update(LEFT, None, false, t0 + ms(2000)), None);
    }
}
