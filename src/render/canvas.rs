//! Immediate-mode drawing helpers over a Direct2D device context.
//! Pages draw through a `Canvas`, which also records interactive zones.

use std::mem::ManuallyDrop;

use windows::core::Interface;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows_numerics::{Matrix3x2, Vector2};

use super::gfx::Gfx;
use super::icons::{self, Icon};
use super::image::CachedBitmap;
use super::shapes;
use super::text::{Align, Laid, TextStyle};
use super::Rect;
use crate::app::Action;
use crate::theme::tokens::*;
use crate::theme::Theme;

#[derive(Clone, Debug)]
pub struct Zone {
    pub rect: Rect,
    pub action: Action,
    pub focusable: bool,
    /// Accessible name (UI Automation).
    pub name: String,
}

pub struct Canvas<'a> {
    pub gfx: &'a mut Gfx,
    pub ctx: ID2D1DeviceContext,
    pub brush: ID2D1SolidColorBrush,
    pub gen: u64,
    pub alpha: f32,
    pub theme: Theme,
    pub zones: Vec<Zone>,
    pub cursor: (f32, f32),
    pub pressed: Option<Action>,
    pub focused: Option<Action>,
    pub reduced: bool,
    /// Hover / press spring values per action (filled by the island each frame).
    pub anims: std::collections::HashMap<Action, (f32, f32)>,
    tf: Vec<Matrix3x2>,
    alpha_stack: Vec<f32>,
    clips: Vec<Rect>,
}

fn c2d(c: Color, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a * a,
    }
}

pub fn v2(x: f32, y: f32) -> Vector2 {
    Vector2 { X: x, Y: y }
}

impl<'a> Canvas<'a> {
    pub fn new(gfx: &'a mut Gfx, ctx: ID2D1DeviceContext, theme: Theme) -> Option<Self> {
        let dev = gfx.dev.as_ref()?;
        let brush = dev.brush.clone();
        let gen = gfx.generation;
        Some(Self {
            gfx,
            ctx,
            brush,
            gen,
            alpha: 1.0,
            theme,
            zones: Vec::new(),
            cursor: (-1000.0, -1000.0),
            pressed: None,
            focused: None,
            reduced: false,
            tf: vec![Matrix3x2::identity()],
            alpha_stack: Vec::new(),
            clips: Vec::new(),
            anims: std::collections::HashMap::new(),
        })
    }

    // ------------------------------------------------------------ state

    pub fn push_alpha(&mut self, a: f32) {
        self.alpha_stack.push(self.alpha);
        self.alpha *= a.clamp(0.0, 1.0);
    }
    pub fn pop_alpha(&mut self) {
        if let Some(a) = self.alpha_stack.pop() {
            self.alpha = a;
        }
    }

    fn cur_tf(&self) -> Matrix3x2 {
        *self.tf.last().unwrap_or(&Matrix3x2::identity())
    }

    /// Pushes a translation + uniform scale about (cx, cy).
    pub fn push_tf(&mut self, dx: f32, dy: f32, scale: f32, cx: f32, cy: f32) {
        let m = Matrix3x2::translation(-cx, -cy)
            * Matrix3x2 {
                M11: scale,
                M12: 0.0,
                M21: 0.0,
                M22: scale,
                M31: 0.0,
                M32: 0.0,
            }
            * Matrix3x2::translation(cx + dx, cy + dy);
        let full = m * self.cur_tf();
        self.tf.push(full);
        unsafe { self.ctx.SetTransform(&full) };
    }
    /// Pushes a non-uniform scale and rotation (radians) about (cx, cy), then a translation.
    /// Zones registered under a rotation are not hit-tested correctly; use it for decoration.
    #[allow(clippy::too_many_arguments)]
    pub fn push_affine(&mut self, dx: f32, dy: f32, sx: f32, sy: f32, rot: f32, cx: f32, cy: f32) {
        let (s, c) = rot.sin_cos();
        let m = Matrix3x2::translation(-cx, -cy)
            * Matrix3x2 {
                M11: sx * c,
                M12: sx * s,
                M21: -sy * s,
                M22: sy * c,
                M31: 0.0,
                M32: 0.0,
            }
            * Matrix3x2::translation(cx + dx, cy + dy);
        let full = m * self.cur_tf();
        self.tf.push(full);
        unsafe { self.ctx.SetTransform(&full) };
    }
    pub fn pop_tf(&mut self) {
        if self.tf.len() > 1 {
            self.tf.pop();
        }
        let m = self.cur_tf();
        unsafe { self.ctx.SetTransform(&m) };
    }

    /// Hover / press amounts (0..1) for an action: springs when available, else instant.
    pub fn anim(&self, a: &Action, hover: bool, pressed: bool) -> (f32, f32) {
        self.anims
            .get(a)
            .copied()
            .unwrap_or((hover as u8 as f32, pressed as u8 as f32))
    }

    /// Canvas point to local coordinates (assumes no rotation).
    fn unmap(&self, x: f32, y: f32) -> (f32, f32) {
        let m = self.cur_tf();
        ((x - m.M31) / m.M11.max(1e-4), (y - m.M32) / m.M22.max(1e-4))
    }

    /// Maps a point through the current transform (for zone registration).
    fn map(&self, x: f32, y: f32) -> (f32, f32) {
        let m = self.cur_tf();
        (x * m.M11 + y * m.M21 + m.M31, x * m.M12 + y * m.M22 + m.M32)
    }

    pub fn push_clip(&mut self, r: Rect) {
        let (x0, y0) = self.map(r.x, r.y);
        let (x1, y1) = self.map(r.right(), r.bottom());
        let mut m = Rect::new(x0.min(x1), y0.min(y1), (x1 - x0).abs(), (y1 - y0).abs());
        if let Some(c) = self.clips.last() {
            m = intersect(&m, c);
        }
        self.clips.push(m);
        unsafe {
            self.ctx
                .PushAxisAlignedClip(&r.to_d2d(), D2D1_ANTIALIAS_MODE_PER_PRIMITIVE)
        };
    }
    pub fn pop_clip(&mut self) {
        self.clips.pop();
        unsafe { self.ctx.PopAxisAlignedClip() };
    }

    /// Visible part of a rect in local coordinates (for culling list rows).
    pub fn visible(&self, r: Rect) -> bool {
        let (x0, y0) = self.map(r.x, r.y);
        let (x1, y1) = self.map(r.right(), r.bottom());
        let m = Rect::new(x0.min(x1), y0.min(y1), (x1 - x0).abs(), (y1 - y0).abs());
        self.clips
            .last()
            .is_none_or(|c| intersect(&m, c).w > 0.0 && intersect(&m, c).h > 0.0)
    }

    /// Clips subsequent drawing to `geo` (e.g. the notch shape).
    pub fn push_geo_clip(&mut self, geo: &ID2D1PathGeometry) {
        let g: ID2D1Geometry = geo.cast().expect("path is a geometry");
        let params = D2D1_LAYER_PARAMETERS1 {
            contentBounds: D2D_RECT_F {
                left: -1e6,
                top: -1e6,
                right: 1e6,
                bottom: 1e6,
            },
            geometricMask: ManuallyDrop::new(Some(g)),
            maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
            maskTransform: Matrix3x2::identity(),
            opacity: 1.0,
            opacityBrush: ManuallyDrop::new(None),
            layerOptions: D2D1_LAYER_OPTIONS1_NONE,
        };
        unsafe { self.ctx.PushLayer(&params, None) };
        // release the geometry reference held by the params copy
        let _ = ManuallyDrop::into_inner(params.geometricMask);
    }
    pub fn pop_layer(&mut self) {
        unsafe { self.ctx.PopLayer() };
    }

    // ------------------------------------------------------------ brushes

    pub fn solid(&self, c: Color) -> &ID2D1SolidColorBrush {
        unsafe { self.brush.SetColor(&c2d(c, self.alpha)) };
        &self.brush
    }

    /// Horizontal accent gradient laid over [x0, x1].
    pub fn accent_brush(&self, x0: f32, x1: f32, opacity: f32) -> Option<ID2D1LinearGradientBrush> {
        let a = self.theme.accent;
        self.gradient(
            v2(x0, 0.0),
            v2(x1, 0.0),
            &[
                (ACCENT_STOPS[0], a[0]),
                (ACCENT_STOPS[1], a[1]),
                (ACCENT_STOPS[2], a[2]),
            ],
            opacity,
        )
    }

    pub fn gradient(
        &self,
        p0: Vector2,
        p1: Vector2,
        stops: &[(f32, Color)],
        opacity: f32,
    ) -> Option<ID2D1LinearGradientBrush> {
        let st: Vec<D2D1_GRADIENT_STOP> = stops
            .iter()
            .map(|(p, c)| D2D1_GRADIENT_STOP {
                position: *p,
                color: c2d(*c, self.alpha * opacity),
            })
            .collect();
        unsafe {
            let rt: &ID2D1RenderTarget = &self.ctx;
            let coll = rt
                .CreateGradientStopCollection(&st, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP)
                .ok()?;
            self.ctx
                .CreateLinearGradientBrush(
                    &D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
                        startPoint: p0,
                        endPoint: p1,
                    },
                    None,
                    &coll,
                )
                .ok()
        }
    }

    pub fn radial(
        &self,
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        inner: Color,
        outer: Color,
    ) -> Option<ID2D1RadialGradientBrush> {
        let st = [
            D2D1_GRADIENT_STOP {
                position: 0.0,
                color: c2d(inner, self.alpha),
            },
            D2D1_GRADIENT_STOP {
                position: 1.0,
                color: c2d(outer, self.alpha),
            },
        ];
        unsafe {
            let rt: &ID2D1RenderTarget = &self.ctx;
            let coll = rt
                .CreateGradientStopCollection(&st, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP)
                .ok()?;
            self.ctx
                .CreateRadialGradientBrush(
                    &D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES {
                        center: v2(cx, cy),
                        gradientOriginOffset: v2(0.0, 0.0),
                        radiusX: rx,
                        radiusY: ry,
                    },
                    None,
                    &coll,
                )
                .ok()
        }
    }

    // ------------------------------------------------------------ primitives

    pub fn rect(&mut self, r: Rect, c: Color) {
        unsafe { self.ctx.FillRectangle(&r.to_d2d(), self.solid(c)) };
    }

    pub fn rrect(&mut self, r: Rect, radius: f32, c: Color) {
        let rr = D2D1_ROUNDED_RECT {
            rect: r.to_d2d(),
            radiusX: radius,
            radiusY: radius,
        };
        unsafe { self.ctx.FillRoundedRectangle(&rr, self.solid(c)) };
    }

    pub fn rrect_brush(&mut self, r: Rect, radius: f32, b: &ID2D1Brush) {
        let rr = D2D1_ROUNDED_RECT {
            rect: r.to_d2d(),
            radiusX: radius,
            radiusY: radius,
        };
        unsafe { self.ctx.FillRoundedRectangle(&rr, b) };
    }

    pub fn stroke_rrect(&mut self, r: Rect, radius: f32, c: Color, w: f32) {
        let rr = D2D1_ROUNDED_RECT {
            rect: r.to_d2d(),
            radiusX: radius,
            radiusY: radius,
        };
        unsafe { self.ctx.DrawRoundedRectangle(&rr, self.solid(c), w, None) };
    }

    /// Continuous-corner filled rectangle.
    pub fn sq(&mut self, r: Rect, radius: f32, c: Color) {
        let radius = radius * self.theme.radius_scale;
        if let Ok(g) = shapes::squircle(&self.gfx.factory, r, radius) {
            unsafe { self.ctx.FillGeometry(&g, self.solid(c), None) };
        }
    }

    pub fn circle(&mut self, cx: f32, cy: f32, r: f32, c: Color) {
        let e = D2D1_ELLIPSE {
            point: v2(cx, cy),
            radiusX: r,
            radiusY: r,
        };
        unsafe { self.ctx.FillEllipse(&e, self.solid(c)) };
    }

    pub fn stroke_circle(&mut self, cx: f32, cy: f32, r: f32, c: Color, w: f32) {
        let e = D2D1_ELLIPSE {
            point: v2(cx, cy),
            radiusX: r,
            radiusY: r,
        };
        unsafe { self.ctx.DrawEllipse(&e, self.solid(c), w, None) };
    }

    pub fn line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, w: f32, c: Color) {
        let ss = self.round_stroke();
        unsafe {
            self.ctx
                .DrawLine(v2(x1, y1), v2(x2, y2), self.solid(c), w, ss.as_ref())
        };
    }

    pub fn fill_geo(&mut self, g: &ID2D1PathGeometry, c: Color) {
        unsafe { self.ctx.FillGeometry(g, self.solid(c), None) };
    }

    pub fn stroke_geo(&mut self, g: &ID2D1PathGeometry, c: Color, w: f32) {
        let ss = self.round_stroke();
        unsafe { self.ctx.DrawGeometry(g, self.solid(c), w, ss.as_ref()) };
    }

    pub fn stroke_geo_brush(&mut self, g: &ID2D1PathGeometry, b: &ID2D1Brush, w: f32) {
        let ss = self.round_stroke();
        unsafe { self.ctx.DrawGeometry(g, b, w, ss.as_ref()) };
    }

    /// Round-cap stroke style, created once per factory and reused.
    fn round_stroke(&self) -> Option<ID2D1StrokeStyle> {
        self.gfx.round_stroke.clone()
    }

    /// Soft glow behind a geometry using the D2D shadow effect.
    pub fn glow_geo(&mut self, g: &ID2D1PathGeometry, c: Color, blur: f32, opacity: f32) {
        if self.theme.high_contrast || opacity <= 0.01 {
            return;
        }
        unsafe {
            let Ok(list) = self.ctx.CreateCommandList() else {
                return;
            };
            let old = self.ctx.GetTarget().ok();
            self.ctx.SetTarget(&list);
            self.ctx.FillGeometry(g, self.solid(WHITE), None);
            self.ctx.SetTarget(old.as_ref());
            if list.Close().is_err() {
                return;
            }
            let Ok(fx) = self.ctx.CreateEffect(&CLSID_D2D1Shadow) else {
                return;
            };
            fx.SetInput(0, &list, true);
            let _ = fx.SetValue(
                D2D1_SHADOW_PROP_BLUR_STANDARD_DEVIATION.0 as u32,
                D2D1_PROPERTY_TYPE_FLOAT,
                &blur.to_le_bytes(),
            );
            let col = [c.r, c.g, c.b, c.a * opacity * self.alpha];
            let bytes: Vec<u8> = col.iter().flat_map(|f| f.to_le_bytes()).collect();
            let _ = fx.SetValue(
                D2D1_SHADOW_PROP_COLOR.0 as u32,
                D2D1_PROPERTY_TYPE_VECTOR4,
                &bytes,
            );
            if let Ok(out) = fx.GetOutput() {
                self.ctx.DrawImage(
                    &out,
                    None,
                    None,
                    D2D1_INTERPOLATION_MODE_LINEAR,
                    D2D1_COMPOSITE_MODE_SOURCE_OVER,
                );
            }
        }
    }

    // ------------------------------------------------------------ text

    pub fn laid(&mut self, s: &str, st: &TextStyle, max_w: f32, align: Align) -> Option<Laid> {
        self.gfx.text.layout(s, st, max_w, align)
    }

    pub fn measure(&mut self, s: &str, st: &TextStyle) -> f32 {
        self.gfx.text.measure(s, st)
    }

    /// Draws single- or multi-line text vertically centred in `r`. Returns the drawn width.
    pub fn text(&mut self, s: &str, st: &TextStyle, r: Rect, c: Color, align: Align) -> f32 {
        if s.is_empty() || r.w <= 0.0 {
            return 0.0;
        }
        let Some(l) = self.laid(s, st, r.w, align) else {
            return 0.0;
        };
        let y = r.cy() - l.height / 2.0;
        unsafe {
            self.ctx.DrawTextLayout(
                v2(r.x, y),
                &l.layout,
                self.solid(c),
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
            )
        };
        l.width
    }

    /// Draws a UI string, translated for the current language (`text` draws content as-is).
    pub fn label(&mut self, s: &str, st: &TextStyle, r: Rect, c: Color, align: Align) -> f32 {
        let t = crate::i18n::tr(s);
        self.text(&t, st, r, c, align)
    }

    /// Top-aligned, wrapping version of `label`.
    #[allow(clippy::too_many_arguments)]
    pub fn label_top(
        &mut self,
        s: &str,
        st: &TextStyle,
        x: f32,
        y: f32,
        w: f32,
        c: Color,
        align: Align,
    ) -> f32 {
        let t = crate::i18n::tr(s);
        self.text_top(&t, st, x, y, w, c, align)
    }

    /// Draws text with its top at `y`. Returns the height.
    #[allow(clippy::too_many_arguments)]
    pub fn text_top(
        &mut self,
        s: &str,
        st: &TextStyle,
        x: f32,
        y: f32,
        w: f32,
        c: Color,
        align: Align,
    ) -> f32 {
        if s.is_empty() {
            return 0.0;
        }
        let Some(l) = self.laid(s, st, w, align) else {
            return 0.0;
        };
        unsafe {
            self.ctx.DrawTextLayout(
                v2(x, y),
                &l.layout,
                self.solid(c),
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
            )
        };
        l.height
    }

    pub fn text_brush(
        &mut self,
        s: &str,
        st: &TextStyle,
        r: Rect,
        b: &ID2D1Brush,
        align: Align,
    ) -> f32 {
        let Some(l) = self.laid(s, st, r.w, align) else {
            return 0.0;
        };
        let y = r.cy() - l.height / 2.0;
        unsafe {
            self.ctx
                .DrawTextLayout(v2(r.x, y), &l.layout, b, D2D1_DRAW_TEXT_OPTIONS_CLIP)
        };
        l.width
    }

    // ------------------------------------------------------------ icons & images

    /// Draws an icon centred in `r` at `size` DIPs.
    pub fn icon(&mut self, icon: Icon, cx: f32, cy: f32, size: f32, c: Color) {
        let factory = self.gfx.factory.clone();
        let Some(g) = self.gfx.icons.get(&factory, icon) else {
            return;
        };
        let s = size / 20.0;
        self.push_tf(cx - 10.0, cy - 10.0, 1.0, 0.0, 0.0);
        self.push_tf(0.0, 0.0, s, 10.0, 10.0);
        if icons::is_filled(icon) {
            self.fill_geo(&g, c);
        } else {
            self.stroke_geo(&g, c, ICON_STROKE);
        }
        self.pop_tf();
        self.pop_tf();
    }

    /// Draws an image filling `r` (cover) with rounded corners.
    pub fn image(&mut self, img: &mut CachedBitmap, r: Rect, radius: f32) {
        let Some(bmp) = img.get(&self.ctx, self.gen) else {
            return;
        };
        let (pw, ph) = (img.pixels.w as f32, img.pixels.h as f32);
        let scale = (r.w / pw).max(r.h / ph);
        let (dw, dh) = (pw * scale, ph * scale);
        let ox = r.x + (r.w - dw) / 2.0;
        let oy = r.y + (r.h - dh) / 2.0;
        unsafe {
            let props = D2D1_BITMAP_BRUSH_PROPERTIES1 {
                extendModeX: D2D1_EXTEND_MODE_CLAMP,
                extendModeY: D2D1_EXTEND_MODE_CLAMP,
                interpolationMode: D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC,
            };
            let bp = D2D1_BRUSH_PROPERTIES {
                opacity: self.alpha,
                transform: Matrix3x2 {
                    M11: scale,
                    M12: 0.0,
                    M21: 0.0,
                    M22: scale,
                    M31: ox,
                    M32: oy,
                },
            };
            if let Ok(b) = self.ctx.CreateBitmapBrush(&bmp, Some(&props), Some(&bp)) {
                let rr = D2D1_ROUNDED_RECT {
                    rect: r.to_d2d(),
                    radiusX: radius,
                    radiusY: radius,
                };
                self.ctx.FillRoundedRectangle(&rr, &b);
            }
        }
    }

    // ------------------------------------------------------------ widgets

    /// Gradient progress bar: the gradient spans the whole track so colour reveals as it fills.
    pub fn progress(&mut self, r: Rect, frac: f32, glow: bool) {
        let rad = r.h / 2.0;
        self.rrect(r, rad, TRACK);
        let f = frac.clamp(0.0, 1.0);
        if f <= 0.0 {
            return;
        }
        let fill = Rect::new(r.x, r.y, (r.w * f).max(r.h), r.h);
        if let Some(b) = self.accent_brush(r.x, r.right(), 1.0) {
            self.rrect_brush(fill, rad, &b.cast().expect("brush"));
        }
        if glow && !self.theme.high_contrast {
            let hx = fill.right() - rad;
            let a = self.theme.accent[1];
            if let Some(g) =
                self.radial(hx, r.cy(), 10.0, 6.0, a.with_alpha(0.4), a.with_alpha(0.0))
            {
                unsafe {
                    self.ctx.FillEllipse(
                        &D2D1_ELLIPSE {
                            point: v2(hx, r.cy()),
                            radiusX: 10.0,
                            radiusY: 6.0,
                        },
                        &g,
                    )
                };
            }
        }
    }

    /// Ring progress (12 o'clock, clockwise) with gradient stroke.
    pub fn ring(&mut self, cx: f32, cy: f32, r: f32, frac: f32, w: f32, color: Option<Color>) {
        self.stroke_circle(cx, cy, r, TRACK, w);
        if frac <= 0.0 {
            return;
        }
        let Ok(g) = shapes::ring_arc(&self.gfx.factory, cx, cy, r, frac) else {
            return;
        };
        match color {
            Some(c) => self.stroke_geo(&g, c, w),
            None => {
                if let Some(b) = self.accent_brush(cx - r, cx + r, 1.0) {
                    self.stroke_geo_brush(&g, &b.cast().expect("brush"), w);
                }
            }
        }
    }

    /// Slider: track + gradient fill + thumb on hover/drag. Registers a zone.
    pub fn slider(&mut self, r: Rect, value: f32, action: Action, name: &str) {
        let hover = self.hovered(r.inset_xy(0.0, -8.0)) || self.pressed.as_ref() == Some(&action);
        let track = Rect::new(r.x, r.cy() - 3.0, r.w, 6.0);
        self.progress(track, value, false);
        if hover {
            let x = r.x + r.w * value.clamp(0.0, 1.0);
            self.circle(x, r.cy(), 7.0, INK);
        }
        self.zone_named(r.inset_xy(0.0, -8.0), action, true, name, 6.0);
    }

    /// Circular icon button with hover fill and press scale. Registers a zone.
    #[allow(clippy::too_many_arguments)]
    pub fn button(
        &mut self,
        icon: Icon,
        cx: f32,
        cy: f32,
        size: f32,
        action: Action,
        name: &str,
        color: Color,
    ) {
        let r = Rect::centered(cx, cy, size, size);
        let hover = self.hovered(r);
        let pressed = self.pressed.as_ref() == Some(&action) && hover;
        let (hv, pv) = self.anim(&action, hover, pressed);
        // press: 0.92 on the snappy spring; hover: a circular fill revealed from the cursor
        let scale = 1.0 - 0.08 * pv;
        if hv > 0.01 {
            let (lx, ly) = self.unmap(self.cursor.0, self.cursor.1);
            let half = size / 2.0;
            let ox = cx + (lx - cx).clamp(-half, half) * (1.0 - hv);
            let oy = cy + (ly - cy).clamp(-half, half) * (1.0 - hv);
            self.push_tf(0.0, 0.0, scale, cx, cy);
            self.circle(ox, oy, half * hv.min(1.0), LINE.with_alpha(hv.min(1.0)));
            self.pop_tf();
        }
        self.push_tf(0.0, 0.0, scale, cx, cy);
        self.icon(icon, cx, cy, (size * 0.5).clamp(14.0, 26.0), color);
        self.pop_tf();
        self.zone_named(r, action, true, name, size / 2.0);
    }

    /// Text capsule button.
    /// Width a pill button needs for its (translated) label, at least `min`.
    pub fn pill_width(&mut self, label: &str, min: f32) -> f32 {
        let t = crate::i18n::tr(label);
        (self.measure(&t, &super::text::BODY_MED) + 26.0).max(min)
    }

    pub fn pill_button(&mut self, label: &str, r: Rect, action: Action, primary: bool) {
        let label_owned = crate::i18n::tr(label);
        let label = label_owned.as_str();
        // long translations shrink to fit instead of being cut off
        let natural = self.measure(label, &super::text::BODY_MED);
        let room = (r.w - 14.0).max(1.0);
        let style = if natural > room {
            let s = super::text::BODY_MED;
            s.size((s.size * room / natural).max(9.5))
        } else {
            super::text::BODY_MED
        };
        let hover = self.hovered(r);
        let pressed = self.pressed.as_ref() == Some(&action) && hover;
        let (hv, pv) = self.anim(&action, hover, pressed);
        let scale = 1.0 - 0.04 * pv;
        self.push_tf(0.0, 0.0, scale, r.cx(), r.cy());
        if primary {
            if let Some(b) = self.accent_brush(r.x, r.right(), 0.9 + 0.1 * hv) {
                self.rrect_brush(r, r.h / 2.0, &b.cast().expect("brush"));
            }
            self.text(label, &style, r, WHITE, Align::Center);
        } else {
            self.rrect(r, r.h / 2.0, LINE.with_alpha(1.0 + 0.8 * hv));
            self.text(label, &style, r, self.theme.ink, Align::Center);
        }
        self.pop_tf();
        self.zone_named(r, action, true, label, r.h / 2.0);
    }

    /// Focus ring: 2 px accent gradient outline with radius + 3.
    pub fn focus_ring(&mut self, r: Rect, radius: f32) {
        let rr = r.inset(-3.0);
        if let Some(b) = self.accent_brush(rr.x, rr.right(), 1.0) {
            let d = D2D1_ROUNDED_RECT {
                rect: rr.to_d2d(),
                radiusX: radius + 3.0,
                radiusY: radius + 3.0,
            };
            unsafe { self.ctx.DrawRoundedRectangle(&d, &b, FOCUS_RING, None) };
        }
    }

    // ------------------------------------------------------------ zones

    pub fn hovered(&self, r: Rect) -> bool {
        let (x0, y0) = self.map(r.x, r.y);
        let (x1, y1) = self.map(r.right(), r.bottom());
        let (cx, cy) = self.cursor;
        cx >= x0.min(x1) && cx < x0.max(x1) && cy >= y0.min(y1) && cy < y0.max(y1)
    }

    pub fn zone(&mut self, r: Rect, action: Action) {
        self.zone_named(r, action, false, "", 0.0);
    }

    pub fn zone_named(
        &mut self,
        r: Rect,
        action: Action,
        focusable: bool,
        name: &str,
        radius: f32,
    ) {
        let (x0, y0) = self.map(r.x, r.y);
        let (x1, y1) = self.map(r.right(), r.bottom());
        let mut rect = Rect::new(x0.min(x1), y0.min(y1), (x1 - x0).abs(), (y1 - y0).abs());
        if let Some(c) = self.clips.last() {
            rect = intersect(&rect, c);
            if rect.w <= 0.0 || rect.h <= 0.0 {
                return;
            }
        }
        if focusable && self.focused.as_ref() == Some(&action) {
            self.focus_ring(r, radius);
        }
        self.zones.push(Zone {
            rect,
            action,
            focusable,
            // accessible names and tooltips follow the UI language
            name: if name.is_empty() {
                String::new()
            } else {
                crate::i18n::tr(name)
            },
        });
    }
}

fn intersect(a: &Rect, b: &Rect) -> Rect {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    Rect::new(
        x,
        y,
        (a.right().min(b.right()) - x).max(0.0),
        (a.bottom().min(b.bottom()) - y).max(0.0),
    )
}
