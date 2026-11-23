//! Where the notch takes the mouse. Outside these shapes the window is click-through
//! (`WS_EX_TRANSPARENT`), so its margins — there for shadows, glows and tooltips — never block
//! the app underneath. The shapes are the same outlines the notch is drawn with, grown by a
//! small tolerance.

use super::island::View;
use super::state::State;
use crate::render::shapes::notch_outline;
use crate::theme::tokens::HIT_TOLERANCE;

/// Even-odd point-in-polygon.
pub fn point_in(poly: &[(f32, f32)], x: f32, y: f32) -> bool {
    let mut inside = false;
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[j];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// The notch outline grown by the hit tolerance (DIP, canvas coordinates).
#[allow(clippy::too_many_arguments)]
pub fn notch_hit(
    cx: f32,
    top: f32,
    w: f32,
    h: f32,
    radius: f32,
    shoulder: f32,
    floating: bool,
) -> Vec<(f32, f32)> {
    notch_outline(cx, top, w, h, radius, shoulder, floating, -HIT_TOLERANCE)
}

impl State {
    /// The shapes that take the mouse this frame (canvas DIP).
    pub fn hit_shapes(&self) -> Vec<Vec<(f32, f32)>> {
        let g = self.geo();
        let mut v = Vec::new();
        if g.main.h < 0.3 {
            return v;
        }
        let rs = self.theme.radius_scale;
        v.push(notch_hit(
            g.main.cx(),
            g.main.y,
            g.main.w,
            g.main.h,
            g.radius * rs,
            g.shoulder,
            g.floating,
        ));
        if let Some(b) = g.bubble {
            v.push(notch_hit(
                b.cx(),
                b.y,
                b.w,
                b.h,
                b.w / 2.0 - 1.0,
                if g.floating {
                    0.0
                } else {
                    (g.shoulder * 0.6).min(8.0)
                },
                g.floating,
            ));
        }
        // minimal mode: a hover band over the hairline (it's only 5 DIP tall)
        if self.settings.appearance.minimal_idle
            && matches!(self.isl.view, View::Minimal | View::Idle)
        {
            let (cx, w, h) = (g.main.cx(), 220.0, 26.0);
            v.push(vec![
                (cx - w / 2.0, -1.0),
                (cx + w / 2.0, -1.0),
                (cx + w / 2.0, h),
                (cx - w / 2.0, h),
            ]);
        }
        v
    }

    /// Whether the notch takes the mouse at this canvas point.
    pub fn hits(&self, x: f32, y: f32) -> bool {
        self.hit_shapes().iter().any(|p| point_in(p, x, y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // an attached expanded notch: 300 wide, 120 tall, radius 32, shoulder 22, centred at 310
    fn attached() -> Vec<(f32, f32)> {
        notch_hit(310.0, 0.0, 300.0, 120.0, 32.0, 22.0, false)
    }

    #[test]
    fn inside_and_outside_the_body() {
        let p = attached();
        assert!(point_in(&p, 310.0, 60.0), "centre");
        assert!(point_in(&p, 161.0, 60.0), "just inside the left edge (160)");
        // 1 DIP outside the edge is within the 2 DIP tolerance; 3 DIP is not
        assert!(point_in(&p, 159.0, 60.0), "1 outside");
        assert!(!point_in(&p, 157.0, 60.0), "3 outside");
        assert!(point_in(&p, 310.0, 121.0), "1 below the bottom");
        assert!(!point_in(&p, 310.0, 123.0), "3 below the bottom");
    }

    #[test]
    fn shoulder_curve_and_corners() {
        let p = attached();
        // the concave shoulder: centred at (160 - 22, 22); a point under the arc is outside it
        let (scx, scy, s) = (138.0f32, 22.0f32, 22.0f32);
        let a = -std::f32::consts::FRAC_PI_4;
        let on_arc = (scx + s * a.cos(), scy + s * a.sin());
        // just inside the flare (toward the body) and well outside it (toward the arc centre)
        assert!(
            point_in(&p, on_arc.0 + 1.0, on_arc.1 - 1.0),
            "inside the flare"
        );
        assert!(!point_in(&p, scx + 6.0, scy + 6.0), "under the shoulder");
        // the bottom corners are rounded: the rectangle's corner itself is outside
        assert!(!point_in(&p, 161.0, 119.0), "cut corner");
    }

    #[test]
    fn floating_rounds_every_corner() {
        let p = notch_hit(310.0, 10.0, 300.0, 120.0, 32.0, 0.0, true);
        assert!(point_in(&p, 310.0, 70.0));
        assert!(!point_in(&p, 161.0, 11.0), "top-left corner is rounded");
        assert!(!point_in(&p, 310.0, 5.0), "above a floating notch");
        assert!(point_in(&p, 310.0, 9.0), "within tolerance above");
    }
}
