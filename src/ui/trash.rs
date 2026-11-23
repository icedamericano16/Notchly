//! "Thrown in the trash" delete choreography and the shelf drop burst.
//!
//! A deleted item lifts with a small pop, crumples into a chip and flies along an arc into a
//! trash can that springs up in the corner. The lid opens as the item approaches, the can gulps
//! (squash + wobble) when it lands, a puff of dust escapes, and the can springs away again.

use std::f32::consts::{PI, TAU};
use std::time::Instant;

use crate::app::state::State;
use crate::features::{Flight, ListKind};
use crate::render::text::{self, Align};
use crate::render::{Canvas, Rect};
use crate::theme::tokens::*;

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Where the can sits inside a page.
fn can_pos(area: Rect) -> (f32, f32) {
    (area.right() - 24.0, area.bottom() - 34.0)
}

/// Small deterministic jitter from an id (no RNG needed).
fn hash01(id: u64, salt: u64) -> f32 {
    let mut x = id
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(salt.wrapping_mul(0xBF58_476D_1CE4_E5B9));
    x ^= x >> 31;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 29;
    (x & 0xFFFF) as f32 / 65535.0
}

/// Draws the can and every flight of `kind`, on top of the page.
pub fn draw(st: &State, cv: &mut Canvas, kind: ListKind, area: Rect) {
    let tr = &st.f.trash;
    if tr.kind != Some(kind) {
        return;
    }
    let shown = tr.shown.value.max(0.0);
    let flying = st.f.flights.iter().any(|f| f.kind == kind);
    if shown <= 0.01 && !flying {
        return;
    }
    let now = Instant::now();
    let (tx, ty) = can_pos(area);
    let bump = tr.bump.value;
    let lid = tr.lid.value.max(-0.15);

    // the can scales about its base; squash-and-stretch from the gulp spring
    let sx = shown * (1.0 + 0.16 * bump);
    let sy = shown * (1.0 - 0.14 * bump);
    let wobble = 0.10 * bump;
    let base_y = ty + 11.0;

    // back: disc, glow and the lid (behind the falling item)
    cv.push_affine(0.0, 0.0, sx, sy, wobble, tx, base_y);
    cv.circle(tx, ty, 21.0, NOTCH.with_alpha(0.92));
    cv.stroke_circle(
        tx,
        ty,
        21.0,
        CRITICAL.with_alpha((0.22 + 0.55 * bump.abs()).min(0.8)),
        1.2,
    );
    // lid hinged on its right edge, swinging up and over
    let hinge = (tx + 8.5, ty - 6.0);
    cv.push_affine(0.0, 0.0, 1.0, 1.0, lid * 1.05, hinge.0, hinge.1);
    let lid_c = cv.theme.ink.lerp(CRITICAL, 0.25 * lid.clamp(0.0, 1.0));
    cv.rrect(Rect::new(tx - 9.0, ty - 7.6, 18.0, 2.6), 1.3, lid_c);
    cv.rrect(Rect::new(tx - 2.6, ty - 10.0, 5.2, 3.0), 1.2, lid_c);
    cv.pop_tf();
    cv.pop_tf();

    // flying items
    for f in st.f.flights.iter().filter(|f| f.kind == kind) {
        ghost(cv, f, f.progress(now), (tx, ty - 6.0), area);
    }

    // front: the can body, so items vanish into it
    cv.push_affine(0.0, 0.0, sx, sy, wobble, tx, base_y);
    let body = Rect::new(tx - 7.0, ty - 4.0, 14.0, 15.0);
    cv.rrect(body, 3.0, cv.theme.ink.with_alpha(0.95));
    for dx in [-3.2_f32, 0.0, 3.2] {
        cv.line(
            tx + dx,
            ty - 0.5,
            tx + dx * 0.9,
            ty + 7.5,
            1.3,
            NOTCH.with_alpha(0.55),
        );
    }
    cv.pop_tf();

    // a puff of dust escapes on each landing
    if let Some(until) = tr.hold_until {
        let since = 0.52 - until.saturating_duration_since(now).as_secs_f32();
        if (0.0..0.4).contains(&since) {
            let k = since / 0.4;
            for i in 0..7 {
                let a = -PI / 2.0 + (i as f32 - 3.0) * 0.42;
                let d = 6.0 + 18.0 * ease_out(k);
                let x = tx + a.cos() * d;
                let y = ty - 8.0 + a.sin() * d + 10.0 * k * k;
                cv.circle(
                    x,
                    y,
                    1.8 * (1.0 - k) + 0.4,
                    cv.theme.mist.with_alpha(0.7 * (1.0 - k)),
                );
            }
        }
    }
}

/// One deleted item on its way into the can.
fn ghost(cv: &mut Canvas, f: &Flight, p: f32, to: (f32, f32), area: Rect) {
    let from = f.from;
    let (x0, y0) = (from.cx(), from.cy());
    let u = p.clamp(0.0, 1.0);
    // lift pop in the first ~20 %, then crumple into a chip while flying
    let pop = (1.0 - ((u - 0.1) / 0.1).powi(2)).max(0.0);
    let e = smooth(u * 0.85 + u * u * 0.15);
    let chip = 13.0;
    // wide clipboard rows fold up sideways first; shelf cards shrink evenly
    let ex = if f.kind == ListKind::Shelf {
        e
    } else {
        ease_out(u * 1.4)
    };
    let sx = (1.0 + (chip / from.w - 1.0) * ex) * (1.0 + 0.07 * pop);
    let sy = (1.0 + (chip / from.h - 1.0) * e) * (1.0 + 0.07 * pop);
    // quadratic arc over the top
    let cxp = x0 + (to.0 - x0) * 0.42;
    let cyp = (y0.min(to.1) - 52.0).max(area.y + 6.0);
    let it = 1.0 - e;
    let x = it * it * x0 + 2.0 * it * e * cxp + e * e * to.0;
    let y = it * it * y0 + 2.0 * it * e * cyp + e * e * to.1 - 5.0 * pop;
    let spin = if f.kind == ListKind::Shelf {
        -0.9
    } else {
        -0.45
    };
    let rot = spin * e + 0.06 * pop;
    let alpha = 1.0 - ((u - 0.84) / 0.16).clamp(0.0, 1.0);
    if alpha <= 0.0 {
        return;
    }
    cv.push_alpha(alpha);
    // soft shadow that tightens as the item lifts away from the surface
    if p > 0.0 {
        cv.push_affine(x - x0, y - y0 + 6.0, sx, sy, rot, x0, y0);
        cv.rrect(from, 12.0, NOTCH.with_alpha(0.35 * (1.0 - e)));
        cv.pop_tf();
    }
    cv.push_affine(x - x0, y - y0, sx, sy, rot, x0, y0);
    cv.sq(from, 12.0, cv.theme.surface_raised);
    cv.stroke_rrect(from, 12.0, f.tint.with_alpha(0.35), 1.0);
    match f.kind {
        ListKind::Shelf => {
            let thumb_r = Rect::new(from.x + 10.0, from.y + 10.0, from.w - 20.0, from.h - 52.0);
            match f.thumb.clone() {
                Some(mut img) => cv.image(&mut img, thumb_r, 8.0),
                None => cv.icon(f.icon, thumb_r.cx(), thumb_r.cy(), 26.0, f.tint),
            }
            cv.text(
                &f.title,
                &text::CAPTION.weight(600),
                Rect::new(from.x + 8.0, from.bottom() - 40.0, from.w - 16.0, 16.0),
                cv.theme.ink,
                Align::Center,
            );
        }
        _ => {
            cv.icon(f.icon, from.x + 18.0, from.cy(), 15.0, f.tint);
            cv.text(
                &f.title,
                &text::BODY,
                Rect::new(from.x + 36.0, from.y, from.w - 48.0, from.h),
                cv.theme.ink,
                Align::Left,
            );
        }
    }
    // a red wash as it nears the can
    let wash = smooth((u - 0.45) / 0.4) * 0.35;
    if wash > 0.0 {
        cv.sq(from, 12.0, CRITICAL.with_alpha(wash));
    }
    cv.pop_tf();
    cv.pop_alpha();
}

/// Ripple rings and a spray of sparks around a card that was just dropped on the shelf.
pub fn drop_burst(cv: &mut Canvas, x: f32, y: f32, t: f32, id: u64) {
    let total = crate::features::DROP_FX_SECS - DROP_IMPACT - 0.2;
    if !(0.0..total).contains(&t) {
        return;
    }
    // two shockwave rings
    for (i, delay) in [0.0_f32, 0.12].iter().enumerate() {
        let k = (t - delay) / 0.6;
        if (0.0..1.0).contains(&k) {
            let r = 22.0 + (46.0 - i as f32 * 12.0) * ease_out(k);
            let a = (1.0 - k) * if i == 0 { 0.45 } else { 0.25 };
            cv.stroke_circle(x, y, r, TINT_SHELF.with_alpha(a), 2.4 * (1.0 - k) + 0.6);
        }
    }
    // sparks: fly out, arc down under gravity, twinkle and fade
    let palette = [TINT_SHELF, TINT_WATER, cv.theme.ink, TINT_PRAYER];
    let k = t / total;
    for i in 0..14u64 {
        let ang = i as f32 / 14.0 * TAU + hash01(id, i) * 0.4;
        let speed = 120.0 + 90.0 * hash01(id, i + 100);
        let px = x + ang.cos() * speed * t;
        let py = y + ang.sin() * speed * t + 0.5 * 420.0 * t * t;
        let twinkle = 0.75 + 0.25 * (t * 30.0 + i as f32).sin();
        let size = (2.8 - 2.2 * k) * twinkle;
        let c = palette[(i % 4) as usize];
        cv.circle(px, py, size.max(0.3), c.with_alpha((1.0 - k) * 0.95));
    }
    // a soft glow under the card at impact
    let g = (1.0 - t / 0.35).max(0.0);
    if g > 0.0 {
        cv.circle(
            x,
            y,
            40.0 + 20.0 * (1.0 - g),
            TINT_SHELF.with_alpha(0.12 * g),
        );
    }
}

/// A dropped card's fall: a real gravity drop from above the shelf, then three bounces that
/// lose energy (each about 42% of the last speed), with a squash on every landing and a
/// slight stretch while falling fast. Returns (vertical offset, squash, alpha) at `t` seconds.
/// Squash > 0 means wider and shorter (anchored at the card's bottom).
pub fn drop_bounce(t: f32) -> (f32, f32, f32) {
    const H: f32 = 44.0; // fall height, DIPs
    const T0: f32 = 0.30; // time to fall
    const E: f32 = 0.42; // speed kept per bounce
    if t <= 0.0 {
        return (-H, 0.0, 0.0);
    }
    let alpha = (t / 0.08).min(1.0);
    // landing times: T0, then each bounce lasts 2·T0·E^k
    let mut impacts = [0.0f32; 4];
    impacts[0] = T0;
    for k in 1..4 {
        impacts[k] = impacts[k - 1] + 2.0 * T0 * E.powi(k as i32);
    }
    // squash: a short pulse at each landing, softer every time
    let mut squash = 0.0;
    for (k, ti) in impacts.iter().enumerate() {
        let a = 0.16 * E.powi(k as i32);
        let d = (t - ti) / 0.045;
        squash += a * (-d * d).exp();
    }
    if t < T0 {
        let u = t / T0;
        // falling: accelerating, a little taller as it speeds up
        return (-H * (1.0 - u * u), squash - 0.08 * u * u, alpha);
    }
    for k in 1..4 {
        let (a, b) = (impacts[k - 1], impacts[k]);
        if t < b {
            let s = (t - a) / (b - a);
            let h = H * E.powi(2 * k as i32);
            return (-h * 4.0 * s * (1.0 - s), squash, alpha);
        }
    }
    (0.0, squash, 1.0)
}

/// When the first landing happens (the burst starts there).
pub const DROP_IMPACT: f32 = 0.30;
/// Total length of the drop (fall + bounces), seconds.
#[cfg(test)]
pub const DROP_SECS: f32 = 0.75;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounce_settles_with_shrinking_hops() {
        // starts above, lands at the first impact, rests at the end
        assert!(drop_bounce(0.0).0 < -40.0);
        assert!(drop_bounce(DROP_IMPACT).0.abs() < 0.5);
        assert!(drop_bounce(DROP_SECS + 0.1).0.abs() < 0.01);
        // each hop is lower than the last
        let peak = |a: f32, b: f32| {
            (0..100)
                .map(|i| drop_bounce(a + (b - a) * i as f32 / 100.0).0)
                .fold(0.0f32, |m, v| m.min(v))
        };
        let h1 = peak(0.30, 0.552);
        let h2 = peak(0.552, 0.658);
        assert!(h1 < -5.0 && h2 > h1 && h2 < -0.5, "{h1} {h2}");
        // squash peaks at the landing
        assert!(drop_bounce(DROP_IMPACT).1 > 0.12);
    }
}
