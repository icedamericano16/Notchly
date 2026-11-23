//! "Thrown in the trash" delete choreography and the shelf drop burst.
//!
//! A deleted item lifts with a small pop, crumples into a chip and flies along an arc into a
//! trash can that springs up in the corner. The lid opens as the item approaches, the can gulps
//! (squash + wobble) when it lands, a puff of dust escapes, and the can springs away again.

use std::f32::consts::{PI, TAU};
use std::time::Instant;

use crate::app::state::State;
use crate::features::{Flight, ListKind};
use crate::render::glass::GlassLayer;
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
    cv.glass_tile(from, 12.0, GlassLayer::Raised);
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

/// The landing on the soft shelf: the surface dimples outward in a flat ripple, a few soft
/// puffs drift along it and slow down, and a faint glow spreads under the card. `(x, y)` is
/// the middle of the card's bottom edge, `w` its width; `t` is seconds since it touched down.
pub fn drop_burst(cv: &mut Canvas, x: f32, y: f32, w: f32, t: f32, id: u64) {
    let d = &SOFT_DROP;
    if !(0.0..d.burst_secs).contains(&t) {
        return;
    }
    // a soft glow spreading under the card
    let g = 1.0 - (t / d.glow_secs).clamp(0.0, 1.0);
    if g > 0.0 {
        let rx = w * (0.55 + 0.25 * (1.0 - g));
        if let Some(br) = cv.radial(
            x,
            y,
            rx,
            rx * 0.28,
            TINT_SHELF.with_alpha(d.glow_alpha * g * g),
            TINT_SHELF.with_alpha(0.0),
        ) {
            use windows::core::Interface;
            if let Ok(br) = br.cast::<windows::Win32::Graphics::Direct2D::ID2D1Brush>() {
                cv.rrect_brush(Rect::centered(x, y, rx * 2.0, rx * 0.56), rx * 0.28, &br);
            }
        }
    }
    // the cushion dimples: a flat ring that widens along the surface and fades
    for (i, delay) in [0.0f32, d.ripple_gap].iter().enumerate() {
        let k = (t - delay) / d.ripple_secs;
        if (0.0..1.0).contains(&k) {
            let e = ease_out(k);
            let rx = w * (0.42 + 0.5 * e);
            let ry = 3.0 + 4.0 * e;
            let a = (1.0 - k).powi(2)
                * if i == 0 {
                    d.ripple_alpha
                } else {
                    d.ripple_alpha * 0.5
                };
            stroke_ellipse(
                cv,
                x,
                y + 1.0,
                rx,
                ry,
                TINT_SHELF.lerp(WHITE, 0.35).with_alpha(a),
                1.2,
            );
        }
    }
    // soft puffs pushed out sideways from under the card; drag slows them, they rise a touch
    for i in 0..d.puffs {
        let side = if i % 2 == 0 { -1.0 } else { 1.0 };
        let delay = hash01(id, i as u64) * 0.08;
        let tt = t - delay;
        if tt <= 0.0 {
            continue;
        }
        let k = (tt / d.puff_secs).min(1.0);
        if k >= 1.0 {
            continue;
        }
        let speed = d.puff_speed * (0.6 + 0.4 * hash01(id, i as u64 + 50));
        let tau = d.puff_drag;
        let travel = speed * tau * (1.0 - (-tt / tau).exp());
        let x0 = x + side * w * (0.28 + 0.14 * hash01(id, i as u64 + 90));
        let px = x0 + side * travel;
        let py =
            y - 2.0 - d.puff_rise * (1.0 - (-tt / 0.35).exp()) * (0.5 + hash01(id, i as u64 + 7));
        let r = 1.6 + 3.6 * ease_out(k);
        let a = (1.0 - k).powi(2) * d.puff_alpha;
        cv.circle(px, py, r, TINT_SHELF.lerp(WHITE, 0.55).with_alpha(a));
    }
}

fn stroke_ellipse(cv: &mut Canvas, x: f32, y: f32, rx: f32, ry: f32, c: Color, w: f32) {
    let e = windows::Win32::Graphics::Direct2D::D2D1_ELLIPSE {
        point: crate::render::canvas::v2(x, y),
        radiusX: rx,
        radiusY: ry,
    };
    unsafe { cv.ctx.DrawEllipse(&e, cv.solid(c), w, None) };
}

/// A dropped card settling onto a soft cushion. It falls (eased from rest, then accelerating),
/// and on touching down the cushion gives way like a damped spring: the card presses in and
/// squashes softly, the cushion pushes it back up in one low, slow rebound, and it settles.
/// Returns (vertical offset, squash, alpha) at `t` seconds; squash > 0 = wider and shorter
/// (anchored at the card's bottom), < 0 = a slight vertical stretch.
pub fn drop_bounce(t: f32) -> (f32, f32, f32) {
    let d = &SOFT_DROP;
    if t <= 0.0 {
        return (-d.fall, 0.0, 0.0);
    }
    let alpha = (t / d.fade_secs).min(1.0);
    if t < d.fall_secs {
        let u = t / d.fall_secs;
        // a gentle start, then gravity; it stretches a little as it speeds up
        let p = u * u;
        return (-d.fall * (1.0 - p), -d.stretch * p, alpha);
    }
    let s = t - d.fall_secs;
    let (c, _) = cushion(s);
    let dy = if c > 0.0 { c * d.sink } else { c * d.lift };
    // the fall's stretch melts into the squash over the first instant of contact
    let squash = c * d.squash_per_dip - d.stretch * (-s / d.stretch_release).exp();
    (dy, squash, alpha)
}

/// The cushion's compression (DIP, > 0 pressed in) and its envelope, `s` seconds after
/// touchdown: an under-damped spring started with the landing speed.
fn cushion(s: f32) -> (f32, f32) {
    let d = &SOFT_DROP;
    let v0 = 2.0 * d.fall / d.fall_secs;
    let w = TAU * d.freq;
    let z = d.damping;
    let wd = w * (1.0 - z * z).sqrt();
    let env = (-z * w * s).exp();
    (v0 / wd * env * (wd * s).sin(), env)
}

/// When the card first touches the cushion (the ripple starts there).
pub const DROP_IMPACT: f32 = SOFT_DROP.fall_secs;

/// How strongly a dropped card's contact shadow shows (0..1) at `t`: it gathers as the card
/// nears the surface, deepens while it presses in, and fades once it has settled.
pub fn drop_shadow(t: f32) -> f32 {
    let d = &SOFT_DROP;
    if t <= 0.0 {
        return 0.0;
    }
    let near = if t < d.fall_secs {
        let u = t / d.fall_secs;
        u * u * u
    } else {
        1.0
    };
    let s = (t - d.fall_secs).max(0.0);
    let settle = 1.0 - ((s - d.settle_secs) / d.shadow_fade).clamp(0.0, 1.0);
    near * settle
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soft_landing() {
        let d = &SOFT_DROP;
        // starts above, touches down at the impact, rests at the end
        assert!(drop_bounce(0.0).0 <= -d.fall + 0.01);
        assert!(drop_bounce(DROP_IMPACT).0.abs() < 0.01);
        let end = DROP_IMPACT + d.settle_secs + 0.2;
        let (dy, sq, a) = drop_bounce(end);
        assert!(dy.abs() < 0.3 && sq.abs() < 0.006 && a == 1.0, "{dy} {sq}");
        // the motion is continuous through touchdown (no jump in position)
        let before = drop_bounce(DROP_IMPACT - 0.002).0;
        let after = drop_bounce(DROP_IMPACT + 0.002).0;
        assert!((after - before).abs() < 1.0, "{before} {after}");
        // it presses into the cushion (squash) and rises in ONE low rebound
        let samples: Vec<(f32, f32)> = (0..400)
            .map(|i| {
                let t = DROP_IMPACT + i as f32 * 0.004;
                let (dy, sq, _) = drop_bounce(t);
                (dy, sq)
            })
            .collect();
        let max_squash = samples.iter().map(|s| s.1).fold(0.0f32, f32::max);
        assert!((0.08..0.16).contains(&max_squash), "squash {max_squash}");
        let lift = samples.iter().map(|s| s.0).fold(0.0f32, f32::min);
        assert!(lift < -2.0 && lift > -d.fall * 0.15, "rebound {lift}");
        // only one rebound is visible (any later hop is under 1 DIP)
        let mut hops = 0;
        let mut up = false;
        for (dy, _) in &samples {
            let now_up = *dy < -1.0;
            if now_up && !up {
                hops += 1;
            }
            up = now_up;
        }
        assert_eq!(hops, 1);
        // slower and softer than a hard floor: the rebound comes well after touchdown
        let first_up = samples.iter().position(|s| s.0 < -0.5).unwrap() as f32 * 0.004;
        assert!(first_up > 0.18, "{first_up}");
    }

    #[test]
    fn contact_shadow_gathers_and_fades() {
        let d = &SOFT_DROP;
        assert_eq!(drop_shadow(0.0), 0.0);
        assert!(drop_shadow(d.fall_secs * 0.5) < drop_shadow(d.fall_secs * 0.9));
        assert!((drop_shadow(DROP_IMPACT + 0.1) - 1.0).abs() < 1e-4);
        assert_eq!(
            drop_shadow(DROP_IMPACT + d.settle_secs + d.shadow_fade + 0.01),
            0.0
        );
    }
}
