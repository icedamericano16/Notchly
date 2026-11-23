//! Legibility over glass: WCAG contrast maths, the tint floor that keeps text readable over a
//! bright backdrop, and the label levels (primary ... quaternary) adapted to what is behind.

use crate::theme::tokens::{Color, GLASS};

pub fn to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// WCAG relative luminance of an opaque sRGB colour.
pub fn luminance(c: Color) -> f32 {
    0.2126 * to_linear(c.r) + 0.7152 * to_linear(c.g) + 0.0722 * to_linear(c.b)
}

pub fn contrast(a: Color, b: Color) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// Source-over in sRGB space (how Direct2D blends): `top` with its alpha over opaque `bottom`.
pub fn over(top: Color, bottom: Color) -> Color {
    let a = top.a.clamp(0.0, 1.0);
    Color {
        r: top.r * a + bottom.r * (1.0 - a),
        g: top.g * a + bottom.g * (1.0 - a),
        b: top.b * a + bottom.b * (1.0 - a),
        a: 1.0,
    }
}

/// Smallest tint alpha that gives `fg` at least `target` contrast against tint-over-backdrop.
/// 1.0 when even a fully opaque tint isn't enough.
pub fn tint_floor(backdrop: Color, tint: Color, fg: Color, target: f32) -> f32 {
    let fg_on = |a: f32| over(fg, over(tint.with_alpha(a), backdrop));
    let ok = |a: f32| contrast(fg_on(a), over(tint.with_alpha(a), backdrop)) >= target;
    if ok(0.0) {
        return 0.0;
    }
    if !ok(1.0) {
        return 1.0;
    }
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..20 {
        let mid = (lo + hi) / 2.0;
        if ok(mid) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    hi
}

/// Text colours for the four label levels over a given (opaque) glass colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Levels {
    pub primary: Color,
    pub secondary: Color,
    pub tertiary: Color,
    pub quaternary: Color,
}

/// Secondary text brightens toward primary (up to `secondary_lift_max`) until it reaches its
/// contrast target; tertiary/quaternary only need the large-text target and stay translucent.
pub fn levels(bg: Color, ink: Color, mist: Color) -> Levels {
    let lift = |base: Color, target: f32, max: f32| {
        let mut k = 0.0;
        while k < max && contrast(over(base.lerp(ink, k), bg), bg) < target {
            k += 0.02;
        }
        base.lerp(ink, k.min(max))
    };
    let secondary = lift(mist, GLASS.contrast_secondary, GLASS.secondary_lift_max);
    let tertiary = {
        let mut a = 0.30f32;
        while a < 0.6 && contrast(over(ink.with_alpha(a), bg), bg) < GLASS.contrast_large {
            a += 0.02;
        }
        ink.with_alpha(a)
    };
    Levels {
        primary: ink,
        secondary,
        tertiary,
        quaternary: ink.with_alpha((tertiary.a * 0.6).max(0.12)),
    }
}

/// The tint alpha the glass needs so primary text meets its target and secondary text can
/// meet its target after lifting. `backdrop` is the brightest colour measured behind the notch.
pub fn glass_floor(backdrop: Color, tint: Color, ink: Color, mist: Color) -> f32 {
    let primary = tint_floor(backdrop, tint, ink, GLASS.contrast_primary);
    let lifted = mist.lerp(ink, GLASS.secondary_lift_max);
    let secondary = tint_floor(backdrop, tint, lifted, GLASS.contrast_secondary);
    primary.max(secondary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::tokens::{GLASS_TINT, INK, MIST};

    #[test]
    fn contrast_basics() {
        let (w, k) = (Color::rgb(0xFFFFFF), Color::rgb(0x000000));
        assert!((contrast(w, k) - 21.0).abs() < 0.01);
        assert!((contrast(w, w) - 1.0).abs() < 1e-4);
        // a known pair: #767676 on white is the classic 4.54:1
        assert!((contrast(Color::rgb(0x767676), w) - 4.54).abs() < 0.02);
    }

    #[test]
    fn floor_keeps_text_legible_on_any_wallpaper() {
        for wall in [
            0xFFFFFF, 0xF2F2F2, 0xFFD60A, 0x7FB2FF, 0x808080, 0x202020, 0x000000,
        ] {
            let b = Color::rgb(wall);
            let a = glass_floor(b, GLASS_TINT, INK, MIST);
            let bg = over(GLASS_TINT.with_alpha(a), b);
            let lv = levels(bg, INK, MIST);
            assert!(contrast(lv.primary, bg) >= 4.5, "primary on {wall:06X}");
            assert!(
                contrast(lv.secondary, bg) >= 4.5 - 0.05,
                "secondary on {wall:06X}"
            );
            assert!(
                contrast(over(lv.tertiary, bg), bg) >= 3.0 - 0.05,
                "tertiary on {wall:06X}"
            );
        }
        // a dark wallpaper needs no tint at all, a white one needs a lot
        assert_eq!(
            glass_floor(Color::rgb(0x000000), GLASS_TINT, INK, MIST),
            0.0
        );
        assert!(glass_floor(Color::rgb(0xFFFFFF), GLASS_TINT, INK, MIST) > 0.6);
    }

    #[test]
    fn tint_floor_is_monotonic_in_brightness() {
        let mut last = 0.0;
        for v in (0..=255).step_by(15) {
            let a = tint_floor(Color::rgb(v * 0x010101), GLASS_TINT, INK, 4.5);
            assert!(a + 1e-4 >= last);
            last = a;
        }
    }
}
