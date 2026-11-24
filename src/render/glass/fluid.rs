//! Fluid Styles: a slow, living animation inside the glass, under the content.
//!
//! A style is data (its layers), not a code path. [`marks`] turns a style and a moment into
//! plain shapes (pure, tested); [`draw`] paints them clipped to the notch. The island asks
//! for frames only while a style animates (Liquid Glass, motion allowed, not paused while
//! idle) and caps them at [`FLUID_IDLE_FPS`] unless the island itself is moving.

use windows::core::Interface;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;

use crate::render::canvas::v2;
use crate::render::{Canvas, Rect};
use crate::theme::tokens::*;

/// Where a layer takes its colour from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Src {
    Accent(usize),
    White,
    Tint,
}

/// One layer of a style.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Layer {
    /// Soft blobs drifting on Lissajous paths (`rise`: they climb and wrap, like lava).
    Blobs {
        colours: &'static [Src],
        opacity: f32,
        /// Seconds per loop on x and y.
        periods: (f32, f32),
        /// Radius as a fraction of the body height.
        size: f32,
        rise: bool,
    },
    /// Folding gradient ribbons across the upper part (aurora).
    Ribbons { opacity: f32, period: f32 },
    /// Sine bands along the bottom (speeds in loops per 10 s; negative runs left).
    Waves {
        bands: &'static [(f32, Src)],
        amplitude: f32,
        opacity: f32,
    },
    /// Tiny glints rising with a wobble, twinkling now and then.
    Glints {
        count: usize,
        opacity: f32,
        rise_secs: f32,
    },
    /// One bright liquid highlight that leans toward the light (pointer).
    Highlight { opacity: f32 },
}

pub struct FluidStyle {
    pub id: &'static str,
    pub name: &'static str,
    pub layers: &'static [Layer],
    /// The theme it suggests in Theme Studio.
    pub theme: &'static str,
}

impl FluidStyle {
    /// Whether anything in it moves over time (Still and Ripple don't).
    pub fn animates(&self) -> bool {
        !self.layers.is_empty()
    }
}

pub const STYLES: &[FluidStyle] = &[
    FluidStyle {
        id: "still",
        name: "Still",
        layers: &[],
        theme: "midnight",
    },
    // arrivals and presses ripple through the lens (the glass already does this; the
    // style adds nothing that moves while idle)
    FluidStyle {
        id: "ripple",
        name: "Ripple",
        layers: &[],
        theme: "crystal",
    },
    FluidStyle {
        id: "flow",
        name: "Flow",
        layers: &[Layer::Blobs {
            colours: &[Src::Accent(0), Src::Accent(2)],
            opacity: 0.16,
            periods: (18.0, 23.0),
            size: 1.1,
            rise: false,
        }],
        theme: "ocean",
    },
    FluidStyle {
        id: "aurora",
        name: "Aurora",
        layers: &[Layer::Ribbons {
            opacity: 0.20,
            period: 16.0,
        }],
        theme: "aurora_glass",
    },
    FluidStyle {
        id: "lava",
        name: "Lava",
        layers: &[Layer::Blobs {
            colours: &[Src::Accent(0), Src::Accent(1), Src::Accent(2), Src::Tint],
            opacity: 0.18,
            periods: (20.0, 28.0),
            size: 0.7,
            rise: true,
        }],
        theme: "ember_glass",
    },
    FluidStyle {
        id: "ocean",
        name: "Ocean",
        layers: &[
            Layer::Waves {
                bands: &[(1.6, Src::Accent(1)), (-1.1, Src::White)],
                amplitude: 2.5,
                opacity: 0.16,
            },
            Layer::Glints {
                count: 6,
                opacity: 0.10,
                rise_secs: 14.0,
            },
        ],
        theme: "sapphire",
    },
    FluidStyle {
        id: "mercury",
        name: "Mercury",
        layers: &[Layer::Highlight { opacity: 0.22 }],
        theme: "obsidian",
    },
    FluidStyle {
        id: "bubbles",
        name: "Bubbles",
        layers: &[Layer::Glints {
            count: 12,
            opacity: 0.32,
            rise_secs: 9.0,
        }],
        theme: "pearl",
    },
];

/// A style by id; unknown ids are Still.
pub fn style(id: &str) -> &'static FluidStyle {
    STYLES.iter().find(|s| s.id == id).unwrap_or(&STYLES[0])
}

/// What the layers react to this frame.
#[derive(Clone, Copy, Debug)]
pub struct Ctx {
    /// Fluid time, seconds (already scaled by speed; frozen with reduced motion).
    pub t: f32,
    /// The glass body the fluid fills.
    pub rect: Rect,
    pub accent: [Color; 3],
    pub tint: Color,
    /// Overall strength (the Intensity setting, 0-1; previews may go up to 2).
    pub intensity: f32,
    /// The light's direction (-1..1), which leans toward the pointer.
    pub light: (f32, f32),
    /// An arrival or notification just happened: 1 fading to 0 (brightness flash, swell).
    pub kick: f32,
}

/// A shape to paint.
#[derive(Clone, Debug, PartialEq)]
pub enum Mark {
    /// A soft radial blob.
    Blob {
        cx: f32,
        cy: f32,
        r: f32,
        c: Color,
    },
    /// A filled band: its top edge, closed down to `bottom`.
    Band {
        top: Vec<(f32, f32)>,
        bottom: f32,
        c: Color,
    },
    /// A cubic ribbon stroked with a two-colour gradient.
    Ribbon {
        p: [(f32, f32); 4],
        width: f32,
        c0: Color,
        c1: Color,
    },
    Dot {
        cx: f32,
        cy: f32,
        r: f32,
        c: Color,
    },
}

fn colour(src: Src, cx: &Ctx) -> Color {
    match src {
        Src::Accent(i) => cx.accent[i.min(2)],
        Src::White => WHITE,
        Src::Tint => cx.tint,
    }
}

/// A cheap repeatable pseudo-random value in 0..1.
fn hash(n: u32) -> f32 {
    let mut x = n.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xFFFF) as f32 / 65535.0
}

/// The style's shapes at this moment (pure: same input, same output).
pub fn marks(style: &FluidStyle, cx: &Ctx) -> Vec<Mark> {
    use std::f32::consts::TAU;
    let r = cx.rect;
    // up to 2 so previews (small cards) can show a style clearly; the setting stops at 1
    let k = cx.intensity.clamp(0.0, 2.0) * (1.0 + FLUID_KICK_BRIGHT * cx.kick.clamp(0.0, 1.0));
    let mut out = Vec::new();
    if r.w < 4.0 || r.h < 4.0 || k <= 0.0 {
        return out;
    }
    for layer in style.layers {
        match *layer {
            Layer::Blobs {
                colours,
                opacity,
                periods,
                size,
                rise,
            } => {
                for (i, src) in colours.iter().enumerate() {
                    let ph = i as f32 * 2.399; // golden-angle offsets keep blobs apart
                    let ax = (cx.t / periods.0 * TAU + ph).sin();
                    let ay = (cx.t / periods.1 * TAU + ph * 1.3).cos();
                    let x = r.cx() + ax * r.w * 0.36;
                    let y = if rise {
                        // climb from the bottom and wrap, a little sideways sway
                        let p = (cx.t / periods.1 + i as f32 / colours.len() as f32).fract();
                        r.bottom() + r.h * 0.4 - p * r.h * 1.8
                    } else {
                        r.cy() + ay * r.h * 0.28
                    };
                    let breathe = 1.0 + 0.08 * (cx.t / periods.0 * TAU * 0.5 + ph).sin();
                    out.push(Mark::Blob {
                        cx: x,
                        cy: y,
                        r: r.h * size * breathe,
                        c: colour(*src, cx).with_alpha(opacity * k),
                    });
                }
            }
            Layer::Ribbons { opacity, period } => {
                for (i, (a, b)) in [(0usize, 1usize), (1, 2)].iter().enumerate() {
                    let ph = cx.t / period * TAU + i as f32 * 1.7;
                    let y0 = r.y + r.h * (0.22 + 0.10 * i as f32);
                    let fold = r.h * 0.22;
                    out.push(Mark::Ribbon {
                        p: [
                            (r.x - 8.0, y0 + fold * ph.sin()),
                            (r.x + r.w * 0.33, y0 - fold * (ph * 1.3).cos()),
                            (r.x + r.w * 0.66, y0 + fold * (ph * 0.8).sin()),
                            (r.right() + 8.0, y0 - fold * (ph * 1.1).cos()),
                        ],
                        width: (r.h * 0.22).clamp(4.0, 22.0),
                        c0: cx.accent[*a].with_alpha(opacity * k),
                        c1: cx.accent[*b].with_alpha(opacity * k),
                    });
                }
            }
            Layer::Waves {
                bands,
                amplitude,
                opacity,
            } => {
                let amp = amplitude * (1.0 + FLUID_KICK_SWELL * cx.kick.clamp(0.0, 1.0));
                for (i, (speed, src)) in bands.iter().enumerate() {
                    let base = r.bottom() - r.h * (0.16 + 0.09 * i as f32);
                    let steps = 24;
                    let top: Vec<(f32, f32)> = (0..=steps)
                        .map(|j| {
                            let u = j as f32 / steps as f32;
                            let x = r.x + u * r.w;
                            let y = base
                                + amp
                                    * (u * TAU * 1.4 + cx.t * speed / 10.0 * TAU + i as f32).sin();
                            (x, y)
                        })
                        .collect();
                    out.push(Mark::Band {
                        top,
                        bottom: r.bottom() + 2.0,
                        c: colour(*src, cx).with_alpha(opacity * k),
                    });
                }
            }
            Layer::Glints {
                count,
                opacity,
                rise_secs,
            } => {
                for i in 0..count.min(FLUID_MAX_GLINTS) {
                    let seed = i as u32 * 7 + 3;
                    let speed = 0.75 + 0.5 * hash(seed);
                    let p = (cx.t / rise_secs * speed + hash(seed + 1)).fract();
                    let x = r.x
                        + r.w * (0.06 + 0.88 * hash(seed + 2))
                        + 2.5 * (cx.t * 1.3 + i as f32).sin();
                    let y = r.bottom() - p * (r.h + 6.0);
                    // fade in at the bottom, out at the top; an occasional twinkle
                    let fade = (p * 6.0).min(1.0) * ((1.0 - p) * 4.0).min(1.0);
                    let twinkle = 0.7 + 0.3 * (cx.t * 3.1 + hash(seed + 3) * TAU).sin().max(0.0);
                    out.push(Mark::Dot {
                        cx: x,
                        cy: y,
                        r: 0.9 + 1.3 * hash(seed + 4),
                        c: WHITE.with_alpha(opacity * k * fade * twinkle),
                    });
                }
            }
            Layer::Highlight { opacity } => {
                let (lx, ly) = cx.light;
                let x = r.cx() + lx.clamp(-1.0, 1.0) * r.w * 0.32;
                let y = r.cy() + ly.clamp(-1.0, 1.0) * r.h * 0.25;
                let wob = 1.0 + 0.05 * (cx.t * 0.9).sin();
                out.push(Mark::Blob {
                    cx: x,
                    cy: y,
                    r: r.h * 0.55 * wob,
                    c: WHITE.with_alpha(opacity * k),
                });
                out.push(Mark::Blob {
                    cx: x,
                    cy: y,
                    r: r.h * 0.18,
                    c: WHITE.with_alpha((opacity * 1.6 * k).min(0.6)),
                });
            }
        }
    }
    // the GPU budget: never more than this many shapes
    out.truncate(FLUID_MAX_MARKS);
    out
}

/// Paints marks (call inside a clip to the glass shape).
pub fn draw(cv: &mut Canvas, marks: &[Mark]) {
    for m in marks {
        match m {
            Mark::Blob { cx, cy, r, c } => {
                if let Some(b) = cv.radial(*cx, *cy, *r, *r, *c, c.with_alpha(0.0)) {
                    if let Ok(b) = b.cast::<ID2D1Brush>() {
                        cv.rrect_brush(Rect::new(cx - r, cy - r, r * 2.0, r * 2.0), 0.0, &b);
                    }
                }
            }
            Mark::Dot { cx, cy, r, c } => cv.circle(*cx, *cy, *r, *c),
            Mark::Band { top, bottom, c } => {
                if let Some(g) = path(cv, top, Some(*bottom)) {
                    cv.fill_geo(&g, *c);
                }
            }
            Mark::Ribbon { p, width, c0, c1 } => {
                let Ok(g) = bezier(cv, p) else { continue };
                if let Some(b) = cv.gradient(
                    crate::render::canvas::v2(p[0].0, 0.0),
                    crate::render::canvas::v2(p[3].0, 0.0),
                    &[(0.0, *c0), (1.0, *c1)],
                    1.0,
                ) {
                    if let Ok(b) = b.cast::<ID2D1Brush>() {
                        cv.stroke_geo_brush(&g, &b, *width);
                    }
                }
            }
        }
    }
}

fn path(cv: &Canvas, pts: &[(f32, f32)], close_to: Option<f32>) -> Option<ID2D1PathGeometry> {
    let (first, rest) = pts.split_first()?;
    unsafe {
        let geo: ID2D1PathGeometry = cv.gfx.factory.CreatePathGeometry().ok()?.into();
        let sink = geo.Open().ok()?;
        sink.BeginFigure(v2(first.0, first.1), D2D1_FIGURE_BEGIN_FILLED);
        for (x, y) in rest {
            sink.AddLine(v2(*x, *y));
        }
        if let (Some(b), Some(last)) = (close_to, pts.last()) {
            sink.AddLine(v2(last.0, b));
            sink.AddLine(v2(first.0, b));
        }
        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
        sink.Close().ok()?;
        Some(geo)
    }
}

fn bezier(cv: &Canvas, p: &[(f32, f32); 4]) -> windows::core::Result<ID2D1PathGeometry> {
    let pt = |q: (f32, f32)| v2(q.0, q.1);
    unsafe {
        let geo: ID2D1PathGeometry = cv.gfx.factory.CreatePathGeometry()?.into();
        let sink = geo.Open()?;
        sink.BeginFigure(pt(p[0]), D2D1_FIGURE_BEGIN_HOLLOW);
        sink.AddBezier(&D2D1_BEZIER_SEGMENT {
            point1: pt(p[1]),
            point2: pt(p[2]),
            point3: pt(p[3]),
        });
        sink.EndFigure(D2D1_FIGURE_END_OPEN);
        sink.Close()?;
        Ok(geo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(t: f32) -> Ctx {
        Ctx {
            t,
            rect: Rect::new(0.0, 0.0, 300.0, 60.0),
            accent: [
                Color::rgb(0xFF0000),
                Color::rgb(0x00FF00),
                Color::rgb(0x0000FF),
            ],
            tint: Color::rgb(0x101010),
            intensity: 1.0,
            light: (0.0, 0.0),
            kick: 0.0,
        }
    }

    #[test]
    fn eight_styles_with_unique_ids() {
        assert_eq!(STYLES.len(), 8);
        let mut ids: Vec<&str> = STYLES.iter().map(|s| s.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 8);
        assert_eq!(style("nope").id, "still");
        // every suggested theme exists
        for s in STYLES {
            assert!(crate::theme::preset(s.theme).is_some(), "{}", s.theme);
        }
    }

    #[test]
    fn still_styles_draw_nothing_and_moving_ones_move() {
        for s in STYLES {
            let a = marks(s, &ctx(0.0));
            let b = marks(s, &ctx(3.7));
            if s.animates() {
                assert!(!a.is_empty(), "{}", s.id);
                assert_ne!(a, b, "{} should move", s.id);
            } else {
                assert!(a.is_empty(), "{}", s.id);
            }
            // deterministic: a frozen clock (reduced motion) gives the same frame
            assert_eq!(a, marks(s, &ctx(0.0)));
        }
    }

    #[test]
    fn stays_in_budget_and_scales_with_intensity() {
        for s in STYLES {
            let m = marks(s, &ctx(1.0));
            assert!(m.len() <= FLUID_MAX_MARKS, "{}", s.id);
            let mut c = ctx(1.0);
            c.intensity = 0.0;
            assert!(marks(s, &c).is_empty());
        }
        // a kick brightens
        let flow = style("flow");
        let alpha = |m: &[Mark]| match &m[0] {
            Mark::Blob { c, .. } => c.a,
            _ => 0.0,
        };
        let mut k = ctx(1.0);
        k.kick = 1.0;
        assert!(alpha(&marks(flow, &k)) > alpha(&marks(flow, &ctx(1.0))));
    }
}
