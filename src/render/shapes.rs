//! Notch outline, continuous-corner (squircle) rectangles and the gooey bridge.

use windows::core::Result;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows_numerics::Vector2;

use super::Rect;

/// Continuous-curvature corner as (along-incoming, along-outgoing) multiples of r.
/// Three cubics per corner; extent 1.528 r gives curvature continuity with the edges.
const CORNER: [(f32, f32); 10] = [
    (1.528_665, 0.0),
    (1.088_493, 0.0),
    (0.868_407, 0.0),
    (0.631_494, 0.074_911),
    (0.372_824, 0.169_054),
    (0.169_054, 0.372_824),
    (0.074_911, 0.631_494),
    (0.0, 0.868_407),
    (0.0, 1.088_493),
    (0.0, 1.528_665),
];
pub const CORNER_EXTENT: f32 = 1.528_665;

fn v(x: f32, y: f32) -> Vector2 {
    Vector2 { X: x, Y: y }
}

/// Emits a corner at `c`: `a` = unit direction travelled along the incoming edge,
/// `b` = unit direction of the outgoing edge. Assumes the current point is the corner start.
fn corner(sink: &ID2D1GeometrySink, c: (f32, f32), a: (f32, f32), b: (f32, f32), r: f32) {
    let p = |i: usize| {
        let (pa, pb) = CORNER[i];
        v(
            c.0 - a.0 * pa * r + b.0 * pb * r,
            c.1 - a.1 * pa * r + b.1 * pb * r,
        )
    };
    unsafe {
        sink.AddBezier(&D2D1_BEZIER_SEGMENT {
            point1: p(1),
            point2: p(2),
            point3: p(3),
        });
        sink.AddBezier(&D2D1_BEZIER_SEGMENT {
            point1: p(4),
            point2: p(5),
            point3: p(6),
        });
        sink.AddBezier(&D2D1_BEZIER_SEGMENT {
            point1: p(7),
            point2: p(8),
            point3: p(9),
        });
    }
}

fn corner_start(c: (f32, f32), a: (f32, f32), r: f32) -> Vector2 {
    v(c.0 - a.0 * CORNER_EXTENT * r, c.1 - a.1 * CORNER_EXTENT * r)
}

/// Clamps a radius so the extended corner fits the available edge lengths.
fn fit_radius(r: f32, w: f32, h: f32) -> f32 {
    r.min(w / (2.0 * CORNER_EXTENT))
        .min(h / (2.0 * CORNER_EXTENT))
        .max(0.0)
}

#[allow(clippy::too_many_arguments)]
/// The attached notch: flat top at `top`, concave shoulders, squircle bottom corners.
/// `cx` is the horizontal centre. With `shoulder == 0` and `float_top`, all four corners round.
pub fn notch_path(
    factory: &ID2D1Factory1,
    cx: f32,
    top: f32,
    w: f32,
    h: f32,
    radius: f32,
    shoulder: f32,
    floating: bool,
) -> Result<ID2D1PathGeometry> {
    let geo: ID2D1PathGeometry = unsafe { factory.CreatePathGeometry()?.into() };
    let sink = unsafe { geo.Open()? };
    let w = w.max(1.0);
    let h = h.max(0.5);
    let l = cx - w / 2.0;
    let r = cx + w / 2.0;
    let b = top + h;
    unsafe {
        if floating {
            let rr = fit_radius(radius, w, h);
            sink.BeginFigure(
                corner_start((l, top), (0.0, -1.0), rr),
                D2D1_FIGURE_BEGIN_FILLED,
            );
            corner(&sink, (l, top), (0.0, -1.0), (1.0, 0.0), rr);
            sink.AddLine(corner_start((r, top), (1.0, 0.0), rr));
            corner(&sink, (r, top), (1.0, 0.0), (0.0, 1.0), rr);
            sink.AddLine(corner_start((r, b), (0.0, 1.0), rr));
            corner(&sink, (r, b), (0.0, 1.0), (-1.0, 0.0), rr);
            sink.AddLine(corner_start((l, b), (-1.0, 0.0), rr));
            corner(&sink, (l, b), (-1.0, 0.0), (0.0, -1.0), rr);
        } else {
            let s = shoulder.min(h * 0.6).max(0.0);
            let rr = radius
                .min((h - s).max(0.0) / CORNER_EXTENT)
                .min(w / (2.0 * CORNER_EXTENT))
                .max(0.0);
            sink.BeginFigure(v(l - s, top), D2D1_FIGURE_BEGIN_FILLED);
            if s > 0.01 {
                sink.AddArc(&D2D1_ARC_SEGMENT {
                    point: v(l, top + s),
                    size: D2D_SIZE_F {
                        width: s,
                        height: s,
                    },
                    rotationAngle: 0.0,
                    sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                    arcSize: D2D1_ARC_SIZE_SMALL,
                });
            }
            sink.AddLine(corner_start((l, b), (0.0, 1.0), rr));
            corner(&sink, (l, b), (0.0, 1.0), (1.0, 0.0), rr);
            sink.AddLine(corner_start((r, b), (1.0, 0.0), rr));
            corner(&sink, (r, b), (1.0, 0.0), (0.0, -1.0), rr);
            sink.AddLine(v(r, top + s));
            if s > 0.01 {
                sink.AddArc(&D2D1_ARC_SEGMENT {
                    point: v(r + s, top),
                    size: D2D_SIZE_F {
                        width: s,
                        height: s,
                    },
                    rotationAngle: 0.0,
                    sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                    arcSize: D2D1_ARC_SIZE_SMALL,
                });
            }
            // extend slightly above the top edge so no seam shows against the bezel
            sink.AddLine(v(r + s, top - 1.0));
            sink.AddLine(v(l - s, top - 1.0));
        }
        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
        sink.Close()?;
    }
    Ok(geo)
}

/// The same outline as [`notch_path`], flattened to a polygon (for window regions).
/// `inset` pulls every edge in (the top of an attached notch stays at the screen edge).
#[allow(clippy::too_many_arguments)]
pub fn notch_outline(
    cx: f32,
    top: f32,
    w: f32,
    h: f32,
    radius: f32,
    shoulder: f32,
    floating: bool,
    inset: f32,
) -> Vec<(f32, f32)> {
    const STEPS: usize = 6;
    let mut pts: Vec<(f32, f32)> = Vec::with_capacity(64);
    let cubic =
        |pts: &mut Vec<(f32, f32)>, p0: (f32, f32), p1: Vector2, p2: Vector2, p3: Vector2| {
            for i in 1..=STEPS {
                let t = i as f32 / STEPS as f32;
                let u = 1.0 - t;
                let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                pts.push((
                    a * p0.0 + b * p1.X + c * p2.X + d * p3.X,
                    a * p0.1 + b * p1.Y + c * p2.Y + d * p3.Y,
                ));
            }
        };
    let corner_pts =
        |pts: &mut Vec<(f32, f32)>, c: (f32, f32), a: (f32, f32), b: (f32, f32), r: f32| {
            let p = |i: usize| {
                let (pa, pb) = CORNER[i];
                v(
                    c.0 - a.0 * pa * r + b.0 * pb * r,
                    c.1 - a.1 * pa * r + b.1 * pb * r,
                )
            };
            let start = p(0);
            pts.push((start.X, start.Y));
            for k in 0..3 {
                let last = *pts.last().expect("start pushed");
                cubic(pts, last, p(1 + 3 * k), p(2 + 3 * k), p(3 + 3 * k));
            }
        };
    let w = (w - 2.0 * inset).max(1.0);
    let (top, h) = if floating {
        (top + inset, (h - 2.0 * inset).max(0.5))
    } else {
        (top, (h - inset).max(0.5))
    };
    let radius = (radius - inset).max(0.0);
    let l = cx - w / 2.0;
    let r = cx + w / 2.0;
    let b = top + h;
    if floating {
        let rr = fit_radius(radius, w, h);
        corner_pts(&mut pts, (l, top), (0.0, -1.0), (1.0, 0.0), rr);
        corner_pts(&mut pts, (r, top), (1.0, 0.0), (0.0, 1.0), rr);
        corner_pts(&mut pts, (r, b), (0.0, 1.0), (-1.0, 0.0), rr);
        corner_pts(&mut pts, (l, b), (-1.0, 0.0), (0.0, -1.0), rr);
    } else {
        let s = shoulder.min(h * 0.6).max(0.0);
        let rr = radius
            .min((h - s).max(0.0) / CORNER_EXTENT)
            .min(w / (2.0 * CORNER_EXTENT))
            .max(0.0);
        // left shoulder: concave quarter circle centred at (l - s, top + s)
        for i in 0..=STEPS {
            let a = -std::f32::consts::FRAC_PI_2 * (1.0 - i as f32 / STEPS as f32);
            pts.push((l - s + s * a.cos(), top + s + s * a.sin()));
        }
        corner_pts(&mut pts, (l, b), (0.0, 1.0), (1.0, 0.0), rr);
        corner_pts(&mut pts, (r, b), (1.0, 0.0), (0.0, -1.0), rr);
        // right shoulder centred at (r + s, top + s), from its left point up to the top
        for i in 0..=STEPS {
            let a = std::f32::consts::PI + std::f32::consts::FRAC_PI_2 * i as f32 / STEPS as f32;
            pts.push((r + s + s * a.cos(), top + s + s * a.sin()));
        }
        pts.push((r + s, top - 1.0));
        pts.push((l - s, top - 1.0));
    }
    pts.dedup_by(|a, b| (a.0 - b.0).abs() < 0.05 && (a.1 - b.1).abs() < 0.05);
    pts
}

/// A rectangle with continuous (squircle) corners.
pub fn squircle(factory: &ID2D1Factory1, rc: Rect, radius: f32) -> Result<ID2D1PathGeometry> {
    let geo: ID2D1PathGeometry = unsafe { factory.CreatePathGeometry()?.into() };
    let sink = unsafe { geo.Open()? };
    let (l, t, r, b) = (rc.x, rc.y, rc.right(), rc.bottom());
    let rr = fit_radius(radius, rc.w, rc.h);
    unsafe {
        sink.BeginFigure(
            corner_start((l, t), (0.0, -1.0), rr),
            D2D1_FIGURE_BEGIN_FILLED,
        );
        corner(&sink, (l, t), (0.0, -1.0), (1.0, 0.0), rr);
        sink.AddLine(corner_start((r, t), (1.0, 0.0), rr));
        corner(&sink, (r, t), (1.0, 0.0), (0.0, 1.0), rr);
        sink.AddLine(corner_start((r, b), (0.0, 1.0), rr));
        corner(&sink, (r, b), (0.0, 1.0), (-1.0, 0.0), rr);
        sink.AddLine(corner_start((l, b), (-1.0, 0.0), rr));
        corner(&sink, (l, b), (-1.0, 0.0), (0.0, -1.0), rr);
        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
        sink.Close()?;
    }
    Ok(geo)
}

/// Metaball-style bridge between the main pill's right edge (`x1`) and the bubble's
/// left edge (`x2`). Its depth shrinks as the gap grows, so the shapes separate like liquid.
pub fn goo_bridge(
    factory: &ID2D1Factory1,
    x1: f32,
    x2: f32,
    top: f32,
    h: f32,
    max_gap: f32,
    floating: bool,
) -> Result<Option<ID2D1PathGeometry>> {
    let gap = x2 - x1;
    if gap >= max_gap || h < 4.0 {
        return Ok(None);
    }
    let t = (1.0 - gap.max(0.0) / max_gap).clamp(0.0, 1.0);
    let geo: ID2D1PathGeometry = unsafe { factory.CreatePathGeometry()?.into() };
    let sink = unsafe { geo.Open()? };
    let overlap = 8.0;
    let mid = (x1 + x2) / 2.0;
    unsafe {
        if floating {
            let cy = top + h / 2.0;
            let half = h * 0.5 * t * 0.85;
            sink.BeginFigure(v(x1 - overlap, cy - h * 0.38), D2D1_FIGURE_BEGIN_FILLED);
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: v(x1 + gap * 0.25, cy - h * 0.38),
                point2: v(mid - gap * 0.2, cy - half),
                point3: v(mid, cy - half),
            });
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: v(mid + gap * 0.2, cy - half),
                point2: v(x2 - gap * 0.25, cy - h * 0.38),
                point3: v(x2 + overlap, cy - h * 0.38),
            });
            sink.AddLine(v(x2 + overlap, cy + h * 0.38));
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: v(x2 - gap * 0.25, cy + h * 0.38),
                point2: v(mid + gap * 0.2, cy + half),
                point3: v(mid, cy + half),
            });
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: v(mid - gap * 0.2, cy + half),
                point2: v(x1 + gap * 0.25, cy + h * 0.38),
                point3: v(x1 - overlap, cy + h * 0.38),
            });
        } else {
            // attached: the liquid hangs from the top edge, depth shrinking with distance
            let depth = h * (0.25 + 0.7 * t);
            sink.BeginFigure(v(x1 - overlap, top - 1.0), D2D1_FIGURE_BEGIN_FILLED);
            sink.AddLine(v(x2 + overlap, top - 1.0));
            sink.AddLine(v(x2 + overlap, top + h * 0.8));
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: v(x2 - gap * 0.1, top + h * 0.8),
                point2: v(mid + gap * 0.25, top + depth),
                point3: v(mid, top + depth),
            });
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: v(mid - gap * 0.25, top + depth),
                point2: v(x1 + gap * 0.1, top + h * 0.8),
                point3: v(x1 - overlap, top + h * 0.8),
            });
        }
        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
        sink.Close()?;
    }
    Ok(Some(geo))
}

/// An arc ring segment path (for progress rings), from angle 0 at 12 o'clock clockwise.
pub fn ring_arc(
    factory: &ID2D1Factory1,
    cx: f32,
    cy: f32,
    r: f32,
    frac: f32,
) -> Result<ID2D1PathGeometry> {
    let geo: ID2D1PathGeometry = unsafe { factory.CreatePathGeometry()?.into() };
    let sink = unsafe { geo.Open()? };
    let frac = frac.clamp(0.0, 0.9999);
    let a = frac * std::f32::consts::TAU;
    let end = v(cx + r * a.sin(), cy - r * a.cos());
    unsafe {
        sink.BeginFigure(v(cx, cy - r), D2D1_FIGURE_BEGIN_HOLLOW);
        if frac > 0.0001 {
            sink.AddArc(&D2D1_ARC_SEGMENT {
                point: end,
                size: D2D_SIZE_F {
                    width: r,
                    height: r,
                },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                arcSize: if frac > 0.5 {
                    D2D1_ARC_SIZE_LARGE
                } else {
                    D2D1_ARC_SIZE_SMALL
                },
            });
        }
        sink.EndFigure(D2D1_FIGURE_END_OPEN);
        sink.Close()?;
    }
    Ok(geo)
}

/// A polyline/polygon from points.
pub fn poly(
    factory: &ID2D1Factory1,
    pts: &[(f32, f32)],
    closed: bool,
) -> Result<ID2D1PathGeometry> {
    let geo: ID2D1PathGeometry = unsafe { factory.CreatePathGeometry()?.into() };
    let sink = unsafe { geo.Open()? };
    unsafe {
        if let Some(&(x, y)) = pts.first() {
            sink.BeginFigure(
                v(x, y),
                if closed {
                    D2D1_FIGURE_BEGIN_FILLED
                } else {
                    D2D1_FIGURE_BEGIN_HOLLOW
                },
            );
            for &(x, y) in &pts[1..] {
                sink.AddLine(v(x, y));
            }
            sink.EndFigure(if closed {
                D2D1_FIGURE_END_CLOSED
            } else {
                D2D1_FIGURE_END_OPEN
            });
        }
        sink.Close()?;
    }
    Ok(geo)
}

#[cfg(test)]
mod tests {
    use super::notch_outline;

    fn area(p: &[(f32, f32)]) -> f32 {
        let mut a = 0.0;
        for i in 0..p.len() {
            let (x0, y0) = p[i];
            let (x1, y1) = p[(i + 1) % p.len()];
            a += x0 * y1 - x1 * y0;
        }
        (a / 2.0).abs()
    }

    #[test]
    fn outline_follows_the_notch() {
        // attached: shoulders flare past the body, corners round the bottom
        let p = notch_outline(300.0, 0.0, 200.0, 80.0, 20.0, 10.0, false, 0.0);
        let (minx, maxx) = p
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, b), q| (a.min(q.0), b.max(q.0)));
        let maxy = p.iter().fold(f32::MIN, |m, q| m.max(q.1));
        assert!((minx - 190.0).abs() < 0.01 && (maxx - 410.0).abs() < 0.01);
        assert!((maxy - 80.0).abs() < 0.01);
        // bottom corners cut a little off the rectangle; shoulders add a little
        let a = area(&p);
        assert!(
            a > 200.0 * 80.0 * 0.95 && a < 200.0 * 81.0 + 2.0 * 100.0,
            "{a}"
        );
        // floating: a closed rounded rectangle inside its box
        let f = notch_outline(300.0, 10.0, 200.0, 60.0, 30.0, 0.0, true, 1.0);
        assert!(f.iter().all(|q| q.0 >= 201.0 - 0.01 && q.0 <= 399.0 + 0.01));
        assert!(f.iter().all(|q| q.1 >= 11.0 - 0.01 && q.1 <= 69.0 + 0.01));
        assert!(area(&f) < 198.0 * 58.0);
    }
}
