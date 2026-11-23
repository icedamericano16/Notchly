//! The edge lens: a displacement map built from the signed distance to the notch's edge.
//! Inside a band along the edge the backdrop is sampled from further out along the edge
//! normal (like light bending through the thick rim of a lens); the centre is untouched.

use crate::theme::tokens::GLASS;

/// A rounded body the lens follows (DIP, canvas coordinates).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub radius: f32,
    /// Attached bodies continue past the top (the screen edge isn't a lens edge).
    pub open_top: bool,
}

/// Signed distance to the body's edge (negative inside).
pub fn sdf(b: &Body, px: f32, py: f32) -> f32 {
    let (mut y0, mut h) = (b.y, b.h);
    if b.open_top {
        // push the top edge far away so only the sides and bottom count
        y0 -= b.h + 64.0;
        h += b.h + 64.0;
    }
    let r = b.radius.min(b.w / 2.0).min(h / 2.0).max(0.0);
    let (cx, cy) = (b.x + b.w / 2.0, y0 + h / 2.0);
    let qx = (px - cx).abs() - b.w / 2.0 + r;
    let qy = (py - cy).abs() - h / 2.0 + r;
    let out = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    out + qx.max(qy).min(0.0) - r
}

/// Width of the lens band for a body of this size.
pub fn band(b: &Body) -> f32 {
    let (lo, hi) = GLASS.lens_band;
    let t = ((b.w.min(b.h * 3.0) - 60.0) / 300.0).clamp(0.0, 1.0);
    lo + (hi - lo) * t
}

/// Displacement (in units of the maximum, -1..1 per axis) at a point: outward along the
/// edge normal, strongest right at the edge and fading as `(1 - depth/band)^exponent`.
pub fn displacement(bodies: &[Body], px: f32, py: f32) -> (f32, f32) {
    let Some((b, d)) = bodies
        .iter()
        .map(|b| (b, sdf(b, px, py)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
    else {
        return (0.0, 0.0);
    };
    let depth = -d;
    let band = band(b);
    if depth <= 0.0 || depth >= band {
        return (0.0, 0.0);
    }
    let e = 0.5;
    let nx = sdf(b, px + e, py) - sdf(b, px - e, py);
    let ny = sdf(b, px, py + e) - sdf(b, px, py - e);
    let len = (nx * nx + ny * ny).sqrt();
    if len < 1e-5 {
        return (0.0, 0.0);
    }
    let m = (1.0 - depth / band).powf(GLASS.lens_exponent);
    (nx / len * m, ny / len * m)
}

/// A lens map covering the canvas: BGRA texels, red = x, green = y, 128 = no displacement.
pub struct LensMap {
    pub w: u32,
    pub h: u32,
    pub texel: f32,
    pub data: Vec<u8>,
}

/// Cache key: shapes rounded to the texel so animation frames reuse maps.
pub fn key(bodies: &[Body]) -> Vec<i32> {
    let q = |v: f32| (v / GLASS.lens_texel).round() as i32;
    bodies
        .iter()
        .flat_map(|b| {
            [
                q(b.x),
                q(b.y),
                q(b.w),
                q(b.h),
                q(b.radius),
                b.open_top as i32,
            ]
        })
        .collect()
}

pub fn build(bodies: &[Body], canvas_w: f32, canvas_h: f32) -> LensMap {
    let texel = GLASS.lens_texel;
    let w = (canvas_w / texel).ceil() as u32;
    let h = (canvas_h / texel).ceil() as u32;
    let mut data = vec![0u8; (w * h * 4) as usize];
    for px in data.as_chunks_mut::<4>().0 {
        *px = [128, 128, 128, 255];
    }
    for b in bodies {
        // only texels inside the band of some body can move
        let x0 = ((b.x / texel).floor() as i32).max(0) as u32;
        let x1 = (((b.x + b.w) / texel).ceil() as u32).min(w);
        let y0 = ((b.y / texel).floor() as i32).max(0) as u32;
        let y1 = (((b.y + b.h) / texel).ceil() as u32).min(h);
        for ty in y0..y1 {
            for tx in x0..x1 {
                let (dx, dy) =
                    displacement(bodies, (tx as f32 + 0.5) * texel, (ty as f32 + 0.5) * texel);
                if dx == 0.0 && dy == 0.0 {
                    continue;
                }
                let i = ((ty * w + tx) * 4) as usize;
                data[i + 1] = encode(dy);
                data[i + 2] = encode(dx);
            }
        }
    }
    LensMap { w, h, texel, data }
}

fn encode(v: f32) -> u8 {
    ((0.5 + v.clamp(-1.0, 1.0) * 0.5) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(open_top: bool) -> Body {
        Body {
            x: 100.0,
            y: 0.0,
            w: 200.0,
            h: 80.0,
            radius: 20.0,
            open_top,
        }
    }

    #[test]
    fn signed_distance() {
        let b = body(false);
        assert!(sdf(&b, 200.0, 40.0) < -30.0); // centre: well inside
        assert!((sdf(&b, 100.0, 40.0)).abs() < 0.01); // on the left edge
        assert!((sdf(&b, 90.0, 40.0) - 10.0).abs() < 0.01); // 10 outside
        assert!(sdf(&b, 101.0, 1.0) > -1.0); // corner: rounded off
                                             // an attached body has no lens at its top edge
        let a = body(true);
        assert!(sdf(&a, 200.0, 1.0) < -30.0);
        assert!((sdf(&a, 200.0, 80.0)).abs() < 0.01);
    }

    #[test]
    fn lens_is_strongest_at_the_edge_and_zero_in_the_middle() {
        let b = [body(true)];
        assert_eq!(displacement(&b, 200.0, 30.0), (0.0, 0.0));
        let (dx, dy) = displacement(&b, 101.0, 40.0);
        assert!(
            dx < -0.7 && dy.abs() < 0.05,
            "left edge pushes left: {dx} {dy}"
        );
        let (dx2, _) = displacement(&b, 106.0, 40.0);
        assert!(dx2 > dx && dx2 < 0.0, "falls off inward");
        let (_, dy3) = displacement(&b, 200.0, 79.0);
        assert!(dy3 > 0.7, "bottom edge pushes down");
        // outside the body: nothing
        assert_eq!(displacement(&b, 50.0, 40.0), (0.0, 0.0));
    }

    #[test]
    fn map_encodes_neutral_and_edges() {
        let b = [body(true)];
        let m = build(&b, 400.0, 120.0);
        assert_eq!((m.w, m.h), (200, 60));
        let at = |x: f32, y: f32| {
            let i = (((y / m.texel) as u32 * m.w + (x / m.texel) as u32) * 4) as usize;
            (m.data[i + 2], m.data[i + 1])
        };
        assert_eq!(at(10.0, 10.0), (128, 128)); // outside
        assert_eq!(at(200.0, 30.0), (128, 128)); // centre
        assert!(at(101.0, 40.0).0 < 60); // left rim samples from the left
        assert!(at(200.0, 79.0).1 > 200); // bottom rim samples from below
                                          // rounding to the texel lets animation frames share maps
        let mut b2 = b;
        b2[0].w += 0.4;
        assert_eq!(key(&b), key(&b2));
    }
}
