//! Album-art adaptive colour: median-cut palette, most vivid swatch with
//! luminance 0.35–0.75, three stops by ±20° hue shift.

use super::tokens::Color;

/// Extracts a palette from premultiplied BGRA pixels.
pub fn palette(bgra: &[u8], max_colors: usize) -> Vec<Color> {
    let mut px: Vec<[u8; 3]> = bgra
        .as_chunks::<4>()
        .0
        .iter()
        .step_by(3)
        .filter(|p| p[3] > 200)
        .map(|p| [p[2], p[1], p[0]])
        .collect();
    if px.is_empty() {
        return Vec::new();
    }
    let mut boxes: Vec<Vec<[u8; 3]>> = vec![std::mem::take(&mut px)];
    while boxes.len() < max_colors {
        // split the box with the widest channel range
        let (idx, ch, range) = boxes
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let (c, r) = widest(b);
                (i, c, r)
            })
            .max_by_key(|t| t.2)
            .unwrap_or((0, 0, 0));
        if range < 8 || boxes[idx].len() < 2 {
            break;
        }
        let mut b = boxes.swap_remove(idx);
        b.sort_unstable_by_key(|p| p[ch]);
        let hi = b.split_off(b.len() / 2);
        boxes.push(b);
        boxes.push(hi);
    }
    boxes
        .iter()
        .filter(|b| !b.is_empty())
        .map(|b| {
            let n = b.len() as f32;
            let s = b.iter().fold([0f32; 3], |a, p| {
                [a[0] + p[0] as f32, a[1] + p[1] as f32, a[2] + p[2] as f32]
            });
            Color {
                r: s[0] / n / 255.0,
                g: s[1] / n / 255.0,
                b: s[2] / n / 255.0,
                a: 1.0,
            }
        })
        .collect()
}

fn widest(b: &[[u8; 3]]) -> (usize, u8) {
    let mut lo = [255u8; 3];
    let mut hi = [0u8; 3];
    for p in b {
        for c in 0..3 {
            lo[c] = lo[c].min(p[c]);
            hi[c] = hi[c].max(p[c]);
        }
    }
    let r = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
    let c = (0..3).max_by_key(|&i| r[i]).unwrap_or(0);
    (c, r[c])
}

pub fn to_hsl(c: Color) -> (f32, f32, f32) {
    let max = c.r.max(c.g).max(c.b);
    let min = c.r.min(c.g).min(c.b);
    let l = (max + min) / 2.0;
    if (max - min).abs() < 1e-6 {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if max == c.r {
        (c.g - c.b) / d + if c.g < c.b { 6.0 } else { 0.0 }
    } else if max == c.g {
        (c.b - c.r) / d + 2.0
    } else {
        (c.r - c.g) / d + 4.0
    };
    (h * 60.0, s, l)
}

pub fn from_hsl(h: f32, s: f32, l: f32) -> Color {
    let h = h.rem_euclid(360.0) / 360.0;
    if s <= 0.0 {
        return Color {
            r: l,
            g: l,
            b: l,
            a: 1.0,
        };
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let f = |t: f32| {
        let t = t.rem_euclid(1.0);
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    Color {
        r: f(h + 1.0 / 3.0),
        g: f(h),
        b: f(h - 1.0 / 3.0),
        a: 1.0,
    }
}

/// Picks the accent stops + surface from a palette. None when nothing is vivid enough.
pub fn accent_from_palette(pal: &[Color]) -> Option<([Color; 3], Color)> {
    let best = pal
        .iter()
        .copied()
        .filter(|c| (0.35..=0.75).contains(&c.luminance()) || to_hsl(*c).1 > 0.5)
        .max_by(|a, b| {
            let (_, sa, la) = to_hsl(*a);
            let (_, sb, lb) = to_hsl(*b);
            let va = sa * (1.0 - (la - 0.55).abs());
            let vb = sb * (1.0 - (lb - 0.55).abs());
            va.partial_cmp(&vb).unwrap_or(std::cmp::Ordering::Equal)
        })?;
    let (h, s, _) = to_hsl(best);
    if s < 0.15 {
        return None;
    }
    let s = s.max(0.65);
    let stops = [
        from_hsl(h - 20.0, s, 0.62),
        from_hsl(h, s, 0.55),
        from_hsl(h + 20.0, s, 0.58),
    ];
    let surface = from_hsl(h, s * 0.5, 0.06);
    Some((stops, surface))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hsl_roundtrip() {
        let c = Color::rgb(0x2D5BFF);
        let (h, s, l) = to_hsl(c);
        let d = from_hsl(h, s, l);
        assert!((c.r - d.r).abs() < 0.01 && (c.g - d.g).abs() < 0.01 && (c.b - d.b).abs() < 0.01);
    }
    #[test]
    fn picks_vivid() {
        let mut px = Vec::new();
        for i in 0..400 {
            if i % 2 == 0 {
                px.extend_from_slice(&[20, 20, 20, 255]);
            } else {
                px.extend_from_slice(&[40, 60, 230, 255]); // BGRA red-ish
            }
        }
        let pal = palette(&px, 8);
        let (stops, _) = accent_from_palette(&pal).unwrap();
        assert!(stops[1].r > stops[1].b);
    }
}
