//! The glass compositor: draws a piece of glass (backdrop → blur → edge lens → colour lift,
//! then tint, depth, sheen and a specular rim) clipped to any geometry, plus thin
//! glass-on-glass tiles. Direct2D effects are created once per device and reused.
//!
//! Every effect input is a bitmap at the drawing context's DPI that starts at the canvas
//! origin, so the backdrop, the lens map and the canvas share one pixel grid.

use std::mem::ManuallyDrop;

use windows::core::Interface;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows_numerics::Matrix3x2;

use super::backend::Tier;
use super::lens::{self, Body};
use crate::render::canvas::v2;
use crate::render::{shapes, Canvas, Rect};
use crate::theme::tokens::*;

/// Where a piece of glass sits in the material stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlassLayer {
    /// A card on the glass (was `theme.surface`).
    Base,
    /// A raised tile or control (was `theme.surface_raised`).
    Raised,
}

/// The pixels behind the glass (Liquid tier, or the Settings preview's sample wallpaper).
#[derive(Clone)]
pub struct Backdrop {
    pub bitmap: ID2D1Bitmap1,
    /// Lens cache slot (0 = island, 1 = Settings preview).
    pub slot: usize,
    /// The transform to draw the backdrop with (identity for the island, so a stretched
    /// island never drags the real desktop along with it). None = the current transform.
    pub transform: Option<Matrix3x2>,
}

/// Everything resolved for this frame (springs already applied).
#[derive(Clone)]
pub struct GlassFrame {
    pub tier: Tier,
    /// Tint colour with its final alpha.
    pub tint: Color,
    pub sigma: f32,
    /// Largest lens displacement in DIP (0 = no lens).
    pub lens: f32,
    pub saturation: f32,
    pub luma_compress: f32,
    pub rim_width: f32,
    pub rim_lit: f32,
    pub rim_unlit: f32,
    pub rim: Color,
    /// Direction the light comes from (unit vector, screen space).
    pub light: (f32, f32),
    /// Sheen centre (canvas DIP) and alpha.
    pub sheen: Option<(f32, f32, f32)>,
    pub sheen_color: Color,
    pub inner_glow: f32,
    pub inner_shadow: f32,
    /// Outer shadow alpha (floating style, Liquid tier).
    pub shadow: f32,
    pub shadow_tint: Color,
    /// Multiplier for glass-on-glass tile fills.
    pub tile: f32,
    pub high_contrast: bool,
    /// Physical pixels per DIP.
    pub scale: f32,
    pub backdrop: Option<Backdrop>,
    /// How much of the captured backdrop shows (it fades in when it arrives, out for tinted
    /// glass).
    pub backdrop_alpha: f32,
    /// How far the glass has turned into the solid notch (material cross-fade).
    pub solid_mix: f32,
    /// Glass (frosted): a milky white haze over the blur, and fine acrylic grain.
    pub haze: f32,
    pub grain: f32,
}

impl GlassFrame {
    /// Glass is drawn (also while it cross-fades into or out of Solid).
    pub fn is_glass(&self) -> bool {
        self.tier != Tier::Solid || self.solid_mix < 0.999
    }
}

/// Runs `f` inside an opacity layer (Direct2D images ignore brush opacity).
fn with_opacity(cv: &mut Canvas, opacity: f32, f: impl FnOnce(&mut Canvas)) {
    if opacity >= 0.999 {
        f(cv);
        return;
    }
    unsafe {
        let params = D2D1_LAYER_PARAMETERS1 {
            contentBounds: D2D_RECT_F {
                left: -1e6,
                top: -1e6,
                right: 1e6,
                bottom: 1e6,
            },
            geometricMask: ManuallyDrop::new(None),
            maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
            maskTransform: Matrix3x2::identity(),
            opacity: opacity.clamp(0.0, 1.0),
            opacityBrush: ManuallyDrop::new(None),
            layerOptions: D2D1_LAYER_OPTIONS1_NONE,
        };
        cv.ctx.PushLayer(&params, None);
    }
    f(cv);
    cv.pop_layer();
}

/// Device-scoped resources.
pub struct GlassRes {
    pub gen: u64,
    border: ID2D1Effect,
    blur: ID2D1Effect,
    disp: ID2D1Effect,
    color: ID2D1Effect,
    /// Shadow effects: outer shadow, inner shadow, inner glow, fake lens.
    shadow: [ID2D1Effect; 4],
    slots: [LensSlot; 2],
    luma: Option<LumaRes>,
    /// The Settings preview's sample wallpaper.
    pub wallpaper: Option<(u32, u32, ID2D1Bitmap1)>,
    /// Acrylic grain (a tiling noise brush), made on first use.
    grain: Option<ID2D1BitmapBrush1>,
}

#[derive(Default)]
struct LensSlot {
    key: Vec<i32>,
    scale: f32,
    /// Full-resolution lens map at the context DPI.
    full: Option<(u32, u32, ID2D1Bitmap1)>,
}

struct LumaRes {
    target: ID2D1Bitmap1,
    read: [ID2D1Bitmap1; 2],
    flip: usize,
    pending: bool,
}

fn set_f32(fx: &ID2D1Effect, prop: u32, v: f32) {
    unsafe {
        let _ = fx.SetValue(prop, D2D1_PROPERTY_TYPE_FLOAT, &v.to_le_bytes());
    }
}
fn set_enum(fx: &ID2D1Effect, prop: u32, v: u32) {
    unsafe {
        let _ = fx.SetValue(prop, D2D1_PROPERTY_TYPE_ENUM, &v.to_le_bytes());
    }
}
fn set_vec4(fx: &ID2D1Effect, prop: u32, v: [f32; 4]) {
    let b: Vec<u8> = v.iter().flat_map(|f| f.to_le_bytes()).collect();
    unsafe {
        let _ = fx.SetValue(prop, D2D1_PROPERTY_TYPE_VECTOR4, &b);
    }
}

/// Saturation lift and luminance compression as a 5x4 colour matrix (row vector form).
pub fn color_matrix(sat: f32, compress: f32) -> [f32; 20] {
    let (lr, lg, lb) = (0.2126, 0.7152, 0.0722);
    let k = 1.0 - compress;
    let row = |c: usize| {
        let lum = [lr, lg, lb];
        let mut r = [0.0f32; 3];
        for (i, l) in lum.iter().enumerate() {
            r[i] = ((1.0 - sat) * l + if i == c { sat } else { 0.0 }) * k;
        }
        r
    };
    let (r, g, b) = (row(0), row(1), row(2));
    // M[i][j]: input channel i contributes to output channel j
    let off = compress * 0.5;
    [
        r[0], g[0], b[0], 0.0, //
        r[1], g[1], b[1], 0.0, //
        r[2], g[2], b[2], 0.0, //
        0.0, 0.0, 0.0, 1.0, //
        off, off, off, 0.0,
    ]
}

impl GlassRes {
    pub fn new(ctx: &ID2D1DeviceContext, gen: u64) -> Option<Self> {
        unsafe {
            let border = ctx.CreateEffect(&CLSID_D2D1Border).ok()?;
            let blur = ctx.CreateEffect(&CLSID_D2D1GaussianBlur).ok()?;
            let disp = ctx.CreateEffect(&CLSID_D2D1DisplacementMap).ok()?;
            let color = ctx.CreateEffect(&CLSID_D2D1ColorMatrix).ok()?;
            let mk = || ctx.CreateEffect(&CLSID_D2D1Shadow).ok();
            let shadow = [mk()?, mk()?, mk()?, mk()?];
            set_enum(
                &border,
                D2D1_BORDER_PROP_EDGE_MODE_X.0 as u32,
                D2D1_BORDER_EDGE_MODE_MIRROR.0 as u32,
            );
            set_enum(
                &border,
                D2D1_BORDER_PROP_EDGE_MODE_Y.0 as u32,
                D2D1_BORDER_EDGE_MODE_MIRROR.0 as u32,
            );
            set_enum(
                &blur,
                D2D1_GAUSSIANBLUR_PROP_BORDER_MODE.0 as u32,
                D2D1_BORDER_MODE_HARD.0 as u32,
            );
            set_enum(
                &blur,
                D2D1_GAUSSIANBLUR_PROP_OPTIMIZATION.0 as u32,
                D2D1_GAUSSIANBLUR_OPTIMIZATION_BALANCED.0 as u32,
            );
            set_enum(
                &disp,
                D2D1_DISPLACEMENTMAP_PROP_X_CHANNEL_SELECT.0 as u32,
                D2D1_CHANNEL_SELECTOR_R.0 as u32,
            );
            set_enum(
                &disp,
                D2D1_DISPLACEMENTMAP_PROP_Y_CHANNEL_SELECT.0 as u32,
                D2D1_CHANNEL_SELECTOR_G.0 as u32,
            );
            for s in &shadow {
                set_enum(
                    s,
                    D2D1_SHADOW_PROP_OPTIMIZATION.0 as u32,
                    D2D1_SHADOW_OPTIMIZATION_BALANCED.0 as u32,
                );
            }
            let _ = color.SetValue(
                D2D1_COLORMATRIX_PROP_CLAMP_OUTPUT.0 as u32,
                D2D1_PROPERTY_TYPE_BOOL,
                &1u32.to_le_bytes(),
            );
            // border → blur → lens → colour
            blur.SetInput(0, &border.GetOutput().ok()?, true);
            disp.SetInput(0, &blur.GetOutput().ok()?, true);
            color.SetInput(0, &disp.GetOutput().ok()?, true);
            Some(Self {
                gen,
                border,
                blur,
                disp,
                color,
                shadow,
                slots: Default::default(),
                luma: None,
                wallpaper: None,
                grain: None,
            })
        }
    }
}

fn bitmap(
    ctx: &ID2D1DeviceContext,
    w: u32,
    h: u32,
    dpi: f32,
    opts: D2D1_BITMAP_OPTIONS,
    data: Option<&[u8]>,
) -> Option<ID2D1Bitmap1> {
    let props = D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: dpi,
        dpiY: dpi,
        bitmapOptions: opts,
        colorContext: ManuallyDrop::new(None),
    };
    unsafe {
        ctx.CreateBitmap(
            D2D_SIZE_U {
                width: w.max(1),
                height: h.max(1),
            },
            data.map(|d| d.as_ptr() as *const _),
            w.max(1) * 4,
            &props,
        )
        .ok()
    }
}

/// Runs `f` with the context drawing into `target` (identity transform), then restores.
fn with_target(cv: &mut Canvas, target: &ID2D1Image, f: impl FnOnce(&ID2D1DeviceContext)) {
    unsafe {
        let old = cv.ctx.GetTarget().ok();
        let mut tf = Matrix3x2::identity();
        cv.ctx.GetTransform(&mut tf);
        cv.ctx.SetTarget(target);
        cv.ctx.SetTransform(&Matrix3x2::identity());
        f(&cv.ctx);
        cv.ctx.SetTarget(old.as_ref());
        cv.ctx.SetTransform(&tf);
    }
}

/// Makes sure the slot's full-resolution lens map matches these bodies.
fn lens_map(cv: &mut Canvas, slot: usize, bodies: &[Body], scale: f32) -> Option<ID2D1Bitmap1> {
    let key = lens::key(bodies);
    let need_w = bodies.iter().map(|b| b.x + b.w).fold(0.0f32, f32::max) + 8.0;
    let need_h = bodies.iter().map(|b| b.y + b.h).fold(0.0f32, f32::max) + 8.0;
    let dpi = 96.0 * scale;
    let res = cv.gfx.glass.as_mut()?;
    let s = &mut res.slots[slot];
    if s.key == key && s.scale == scale {
        if let Some((_, _, b)) = &s.full {
            return Some(b.clone());
        }
    }
    let ctx = cv.ctx.clone();
    let pw = (need_w * scale).ceil() as u32;
    let ph = (need_h * scale).ceil() as u32;
    let full = match &s.full {
        Some((w, h, b)) if *w >= pw && *h >= ph && s.scale == scale => b.clone(),
        _ => {
            let b = bitmap(&ctx, pw, ph, dpi, D2D1_BITMAP_OPTIONS_TARGET, None)?;
            s.full = Some((pw, ph, b.clone()));
            b
        }
    };
    let map = lens::build(bodies, need_w, need_h);
    let small = bitmap(
        &ctx,
        map.w,
        map.h,
        96.0 / map.texel,
        D2D1_BITMAP_OPTIONS_NONE,
        Some(&map.data),
    )?;
    s.key = key;
    s.scale = scale;
    let target: ID2D1Image = full.cast().ok()?;
    with_target(cv, &target, |ctx| unsafe {
        ctx.Clear(Some(&D2D1_COLOR_F {
            r: 0.5,
            g: 0.5,
            b: 0.5,
            a: 1.0,
        }));
        let dest = D2D_RECT_F {
            left: 0.0,
            top: 0.0,
            right: map.w as f32 * map.texel,
            bottom: map.h as f32 * map.texel,
        };
        ctx.DrawBitmap(
            &small,
            Some(&dest),
            1.0,
            D2D1_INTERPOLATION_MODE_LINEAR,
            None,
            None,
        );
    });
    Some(full)
}

/// The processed backdrop (blur, lens, colour lift) as an image, or None (system blur / solid).
fn backdrop_image(cv: &mut Canvas, g: &GlassFrame, bodies: &[Body]) -> Option<ID2D1Image> {
    let bd = g.backdrop.as_ref()?;
    let lens = if g.lens > 0.05 && !bodies.is_empty() {
        lens_map(cv, bd.slot, bodies, g.scale)
    } else {
        None
    };
    let res = cv.gfx.glass.as_ref()?;
    unsafe {
        res.border.SetInput(0, &bd.bitmap, true);
        set_f32(
            &res.blur,
            D2D1_GAUSSIANBLUR_PROP_STANDARD_DEVIATION.0 as u32,
            g.sigma.max(0.0),
        );
        let m = color_matrix(g.saturation, g.luma_compress);
        let b: Vec<u8> = m.iter().flat_map(|f| f.to_le_bytes()).collect();
        let _ = res.color.SetValue(
            D2D1_COLORMATRIX_PROP_COLOR_MATRIX.0 as u32,
            D2D1_PROPERTY_TYPE_MATRIX_5X4,
            &b,
        );
        match lens {
            Some(map) => {
                res.disp.SetInput(1, &map, true);
                // the map stores ±max as 0..1 around 0.5; the effect works in pixels
                set_f32(
                    &res.disp,
                    D2D1_DISPLACEMENTMAP_PROP_SCALE.0 as u32,
                    2.0 * g.lens * g.scale,
                );
            }
            None => {
                res.disp.SetInput(1, &res.border.GetOutput().ok()?, true);
                set_f32(&res.disp, D2D1_DISPLACEMENTMAP_PROP_SCALE.0 as u32, 0.0);
            }
        }
        res.color.GetOutput().ok()
    }
}

fn geo_bounds(geo: &ID2D1Geometry) -> Rect {
    unsafe {
        geo.GetBounds(None)
            .map(|b| Rect::new(b.left, b.top, b.right - b.left, b.bottom - b.top))
            .unwrap_or_default()
    }
}

/// A command list holding `geo` stroked (or filled) in white, for blurred depth effects.
fn geo_list(cv: &Canvas, geo: &ID2D1Geometry, stroke: Option<f32>) -> Option<ID2D1CommandList> {
    unsafe {
        let list = cv.ctx.CreateCommandList().ok()?;
        let old = cv.ctx.GetTarget().ok();
        let mut tf = Matrix3x2::identity();
        cv.ctx.GetTransform(&mut tf);
        cv.ctx.SetTarget(&list);
        let white = cv.solid(WHITE).clone();
        match stroke {
            Some(w) => cv.ctx.DrawGeometry(geo, &white, w, None),
            None => cv.ctx.FillGeometry(geo, &white, None),
        }
        cv.ctx.SetTarget(old.as_ref());
        cv.ctx.SetTransform(&tf);
        list.Close().ok()?;
        Some(list)
    }
}

/// A blurred, coloured copy of `geo`'s stroke (inner shadow / glow / fake lens) drawn at an
/// offset. Callers draw it inside the glass clip so only the inner half shows.
fn soft_stroke(
    cv: &mut Canvas,
    which: usize,
    geo: &ID2D1Geometry,
    width: f32,
    blur: f32,
    c: Color,
    dy: f32,
) {
    if c.a * cv.alpha <= 0.003 {
        return;
    }
    let Some(list) = geo_list(cv, geo, Some(width)) else {
        return;
    };
    let Some(res) = cv.gfx.glass.as_ref() else {
        return;
    };
    let fx = res.shadow[which].clone();
    unsafe {
        fx.SetInput(0, &list, true);
        set_f32(&fx, D2D1_SHADOW_PROP_BLUR_STANDARD_DEVIATION.0 as u32, blur);
        set_vec4(
            &fx,
            D2D1_SHADOW_PROP_COLOR.0 as u32,
            [c.r, c.g, c.b, c.a * cv.alpha],
        );
        if let Ok(out) = fx.GetOutput() {
            cv.ctx.DrawImage(
                &out,
                Some(&v2(0.0, dy)),
                None,
                D2D1_INTERPOLATION_MODE_LINEAR,
                D2D1_COMPOSITE_MODE_SOURCE_OVER,
            );
        }
    }
}

/// A gradient brush along the light direction across `b`: lit edge → nothing → far edge.
fn rim_brush(cv: &Canvas, b: Rect, g: &GlassFrame, lit: f32, unlit: f32) -> Option<ID2D1Brush> {
    let (lx, ly) = g.light;
    let half = (b.w * lx.abs() + b.h * ly.abs()) / 2.0 + 1.0;
    let (cx, cy) = (b.cx(), b.cy());
    let p0 = v2(cx + lx * half, cy + ly * half);
    let p1 = v2(cx - lx * half, cy - ly * half);
    let c = g.rim;
    cv.gradient(
        p0,
        p1,
        &[
            (0.0, c.with_alpha(lit)),
            (0.42, c.with_alpha(lit * 0.12)),
            (0.6, c.with_alpha(0.0)),
            (1.0, c.with_alpha(unlit)),
        ],
        1.0,
    )
    .and_then(|br| br.cast().ok())
}

/// Draws a piece of glass shaped like `geo`. `bodies` describe it for the edge lens.
pub fn surface(
    cv: &mut Canvas,
    geo: &ID2D1Geometry,
    bodies: &[Body],
    g: &GlassFrame,
    floating: bool,
) {
    let b = geo_bounds(geo);
    if b.w <= 0.0 || b.h <= 0.0 {
        return;
    }
    // outer shadow, tinted by the backdrop (floating style, every glass tier)
    let glassy = 1.0 - g.solid_mix;
    if floating && g.shadow > 0.01 && !g.high_contrast && glassy > 0.01 {
        if let Some(list) = geo_list(cv, geo, None) {
            if let Some(res) = cv.gfx.glass.as_ref() {
                let fx = res.shadow[0].clone();
                let c = g.shadow_tint;
                unsafe {
                    fx.SetInput(0, &list, true);
                    set_f32(
                        &fx,
                        D2D1_SHADOW_PROP_BLUR_STANDARD_DEVIATION.0 as u32,
                        GLASS.shadow_blur,
                    );
                    set_vec4(
                        &fx,
                        D2D1_SHADOW_PROP_COLOR.0 as u32,
                        [c.r, c.g, c.b, g.shadow * glassy * cv.alpha],
                    );
                    if let Ok(out) = fx.GetOutput() {
                        cv.ctx.DrawImage(
                            &out,
                            Some(&v2(0.0, GLASS.shadow_dy)),
                            None,
                            D2D1_INTERPOLATION_MODE_LINEAR,
                            D2D1_COMPOSITE_MODE_SOURCE_OVER,
                        );
                    }
                }
            }
        }
    }
    let img = backdrop_image(cv, g, bodies);
    cv.push_geo_clip_any(geo);
    if let (Some(img), Some(bd)) = (img, g.backdrop.as_ref()) {
        with_opacity(cv, g.backdrop_alpha * glassy, |cv| unsafe {
            let mut tf = Matrix3x2::identity();
            cv.ctx.GetTransform(&mut tf);
            if let Some(t) = bd.transform {
                cv.ctx.SetTransform(&t);
            }
            let src = D2D_RECT_F {
                left: b.x - 2.0,
                top: b.y - 2.0,
                right: b.right() + 2.0,
                bottom: b.bottom() + 2.0,
            };
            cv.ctx.DrawImage(
                &img,
                Some(&v2(src.left, src.top)),
                Some(&src),
                D2D1_INTERPOLATION_MODE_LINEAR,
                D2D1_COMPOSITE_MODE_SOURCE_OVER,
            );
            cv.ctx.SetTransform(&tf);
        });
    }
    // Glass: frosted acrylic — a milky haze lifts the blur (under the tint, so text contrast
    // is still governed by the tint floor)
    if g.haze > 0.002 {
        unsafe {
            let c = cv.solid(WHITE.with_alpha(g.haze * glassy)).clone();
            cv.ctx.FillRectangle(&b.to_d2d(), &c);
        }
    }
    // tint: a touch lighter at the top, like light entering the glass
    if g.tint.a > 0.002 {
        let top = g.tint.lerp(WHITE.with_alpha(g.tint.a), 0.05);
        if let Some(br) = cv.gradient(
            v2(b.x, b.y),
            v2(b.x, b.bottom()),
            &[(0.0, top), (1.0, g.tint)],
            1.0,
        ) {
            if let Ok(br) = br.cast::<ID2D1Brush>() {
                unsafe { cv.ctx.FillRectangle(&b.to_d2d(), &br) };
            }
        }
    }
    // Glass: frosted acrylic — fine grain over the tinted blur
    if g.grain > 0.002 && glassy > 0.01 {
        if let Some(br) = grain_brush(cv) {
            unsafe {
                br.SetOpacity(g.grain * glassy * cv.alpha);
                cv.ctx.FillRectangle(&b.to_d2d(), &br);
            }
        }
    }
    // cross-fading into Solid: the notch's black closes over the glass
    if g.solid_mix > 0.001 {
        unsafe {
            let c = cv.solid(NOTCH.with_alpha(g.solid_mix)).clone();
            cv.ctx.FillRectangle(&b.to_d2d(), &c);
        }
    }
    if g.high_contrast {
        cv.pop_layer();
        unsafe {
            let c = cv.solid(cv.theme.ink).clone();
            cv.ctx.DrawGeometry(geo, &c, GLASS.hc_rim * 2.0, None);
        }
        return;
    }
    // tinted glass has no real lens: a soft band of light stands in (it fades as a captured
    // backdrop fades in)
    let fake = (1.0 - g.backdrop_alpha) * glassy;
    if fake > 0.01 {
        let band = lens::band(bodies.first().copied().as_ref().unwrap_or(&Body {
            x: b.x,
            y: b.y,
            w: b.w,
            h: b.h,
            radius: 0.0,
            open_top: false,
        }));
        soft_stroke(
            cv,
            3,
            geo,
            band,
            band * 0.45,
            WHITE.with_alpha(GLASS.fake_lens_alpha * fake),
            0.0,
        );
    }
    // depth: shadow pooling along the bottom edge, light along the top
    soft_stroke(
        cv,
        1,
        geo,
        4.0,
        GLASS.inner_shadow_blur,
        GLASS_INNER_SHADOW.with_alpha(g.inner_shadow * glassy),
        -GLASS.inner_shadow_lift,
    );
    soft_stroke(
        cv,
        2,
        geo,
        2.5,
        2.5,
        WHITE.with_alpha(g.inner_glow * glassy),
        GLASS.inner_shadow_lift * 0.6,
    );
    // sheen
    if let Some((sx, sy, a)) = g.sheen {
        let a = a * glassy;
        if a > 0.003 {
            let r = (b.w * GLASS.sheen_radius).max(24.0);
            if let Some(br) = cv.radial(
                sx,
                sy,
                r,
                r * 0.6,
                g.sheen_color.with_alpha(a),
                g.sheen_color.with_alpha(0.0),
            ) {
                if let Ok(br) = br.cast::<ID2D1Brush>() {
                    unsafe {
                        cv.ctx.FillEllipse(
                            &D2D1_ELLIPSE {
                                point: v2(sx, sy),
                                radiusX: r,
                                radiusY: r * 0.6,
                            },
                            &br,
                        )
                    };
                }
            }
        }
    }
    // specular rim (twice the width: half of it is clipped away outside)
    if let Some(br) = rim_brush(cv, b, g, g.rim_lit * glassy, g.rim_unlit * glassy) {
        unsafe { cv.ctx.DrawGeometry(geo, &br, g.rim_width * 2.0, None) };
    }
    cv.pop_layer();
}

/// The acrylic grain: soft white/black specks (premultiplied), tiled.
fn grain_brush(cv: &mut Canvas) -> Option<ID2D1BitmapBrush1> {
    if let Some(b) = cv.gfx.glass.as_ref()?.grain.as_ref() {
        return Some(b.clone());
    }
    let n = GLASS.grain_size;
    let mut data = vec![0u8; (n * n * 4) as usize];
    let mut seed = 0x9E37_79B9u32;
    for px in data.as_chunks_mut::<4>().0 {
        // xorshift: deterministic noise
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let v = (seed & 0xFF) as f32 / 255.0;
        // half the specks light, half dark; premultiplied
        let a = ((v - 0.5).abs() * 2.0).powf(1.6);
        let c = if v > 0.5 { a } else { 0.0 };
        let (c, a) = ((c * 255.0) as u8, (a * 255.0) as u8);
        *px = [c, c, c, a];
    }
    let dpi = unsafe {
        let (mut x, mut y) = (0.0, 0.0);
        cv.ctx.GetDpi(&mut x, &mut y);
        x
    };
    let ctx = cv.ctx.clone();
    let bmp = bitmap(&ctx, n, n, dpi, D2D1_BITMAP_OPTIONS_NONE, Some(&data))?;
    let props = D2D1_BITMAP_BRUSH_PROPERTIES1 {
        extendModeX: D2D1_EXTEND_MODE_WRAP,
        extendModeY: D2D1_EXTEND_MODE_WRAP,
        interpolationMode: D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
    };
    let br = unsafe { ctx.CreateBitmapBrush(&bmp, Some(&props), None).ok()? };
    cv.gfx.glass.as_mut()?.grain = Some(br.clone());
    Some(br)
}

/// A thin piece of glass on the glass: a light tint, a top light and a 1 DIP rim.
pub fn tile(cv: &mut Canvas, r: Rect, radius: f32, layer: GlassLayer, g: &GlassFrame) {
    let radius = radius * cv.theme.radius_scale;
    let Ok(geo) = shapes::squircle(&cv.gfx.factory, r, radius) else {
        return;
    };
    let Ok(geo) = geo.cast::<ID2D1Geometry>() else {
        return;
    };
    let base = match layer {
        GlassLayer::Raised => GLASS.tile_tint_raised,
        _ => GLASS.tile_tint,
    } * g.tile;
    unsafe {
        let fill = cv.solid(WHITE.with_alpha(base)).clone();
        cv.ctx.FillGeometry(&geo, &fill, None);
    }
    if !g.high_contrast {
        if let Some(br) = cv.gradient(
            v2(r.x, r.y),
            v2(r.x, r.y + r.h * 0.55),
            &[
                (0.0, WHITE.with_alpha(GLASS.tile_top_light * g.tile)),
                (1.0, WHITE.with_alpha(0.0)),
            ],
            1.0,
        ) {
            if let Ok(br) = br.cast::<ID2D1Brush>() {
                unsafe { cv.ctx.FillGeometry(&geo, &br, None) };
            }
        }
    }
    let (lit, unlit) = if g.high_contrast {
        (1.0, 1.0)
    } else {
        (GLASS.tile_rim, GLASS.tile_rim * 0.3)
    };
    if let Some(br) = rim_brush(cv, r, g, lit, unlit) {
        let w = if g.high_contrast { GLASS.hc_rim } else { 1.0 };
        let inner = r.inset(w / 2.0);
        if let Ok(rg) = shapes::squircle(&cv.gfx.factory, inner, (radius - w / 2.0).max(0.0)) {
            unsafe { cv.ctx.DrawGeometry(&rg, &br, w, None) };
        }
    }
}

/// Samples the processed backdrop behind `r` on a small grid (GPU downscale; only the
/// aggregate is read back, a frame later to avoid stalls). Returns (brightest, average).
pub fn sample(cv: &mut Canvas, g: &GlassFrame, r: Rect) -> Option<(Color, Color)> {
    let n = GLASS.luma_grid;
    let ctx = cv.ctx.clone();
    if cv.gfx.glass.as_ref()?.luma.is_none() {
        let target = bitmap(&ctx, n, n, 96.0, D2D1_BITMAP_OPTIONS_TARGET, None)?;
        let read = |_: usize| {
            bitmap(
                &ctx,
                n,
                n,
                96.0,
                D2D1_BITMAP_OPTIONS_CPU_READ | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
                None,
            )
        };
        let read = [read(0)?, read(1)?];
        cv.gfx.glass.as_mut()?.luma = Some(LumaRes {
            target,
            read,
            flip: 0,
            pending: false,
        });
    }
    // read what was copied last time
    let mut result = None;
    {
        let l = cv.gfx.glass.as_mut()?.luma.as_mut()?;
        if l.pending {
            let rb = &l.read[l.flip];
            unsafe {
                if let Ok(m) = rb.Map(D2D1_MAP_OPTIONS_READ) {
                    let mut max = Color::rgb(0);
                    let (mut sr, mut sg, mut sb) = (0.0, 0.0, 0.0);
                    for y in 0..n as usize {
                        for x in 0..n as usize {
                            let p = m.bits.add(y * m.pitch as usize + x * 4);
                            let c = Color {
                                r: *p.add(2) as f32 / 255.0,
                                g: *p.add(1) as f32 / 255.0,
                                b: *p as f32 / 255.0,
                                a: 1.0,
                            };
                            if c.luminance() > max.luminance() {
                                max = c;
                            }
                            sr += c.r;
                            sg += c.g;
                            sb += c.b;
                        }
                    }
                    let k = (n * n) as f32;
                    result = Some((
                        max,
                        Color {
                            r: sr / k,
                            g: sg / k,
                            b: sb / k,
                            a: 1.0,
                        },
                    ));
                    let _ = rb.Unmap();
                }
            }
            l.pending = false;
            l.flip ^= 1;
        }
    }
    let img = backdrop_image(cv, g, &[])?;
    let l = cv.gfx.glass.as_ref()?.luma.as_ref()?;
    let (target, read) = (l.target.clone(), l.read[l.flip].clone());
    let timg: ID2D1Image = target.cast().ok()?;
    with_target(cv, &timg, |ctx| unsafe {
        ctx.Clear(Some(&D2D1_COLOR_F {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        }));
        // map r (DIP, at the context DPI) onto the n x n target
        let s = n as f32 / (r.w * g.scale).max(1.0);
        let sy = n as f32 / (r.h * g.scale).max(1.0);
        ctx.SetTransform(&Matrix3x2 {
            M11: s * g.scale,
            M12: 0.0,
            M21: 0.0,
            M22: sy * g.scale,
            M31: -r.x * s * g.scale,
            M32: -r.y * sy * g.scale,
        });
        ctx.DrawImage(
            &img,
            None,
            None,
            D2D1_INTERPOLATION_MODE_LINEAR,
            D2D1_COMPOSITE_MODE_SOURCE_COPY,
        );
    });
    unsafe {
        let _ = read.CopyFromBitmap(None, &target, None);
    }
    if let Some(l) = cv.gfx.glass.as_mut().and_then(|g| g.luma.as_mut()) {
        l.pending = true;
    }
    result
}

/// The Settings preview's sample wallpaper: soft colour fields with a bright band, so the
/// preview shows blur, lensing and colour lift (procedural: nothing to ship).
pub fn wallpaper(cv: &mut Canvas, w: u32, h: u32) -> Option<ID2D1Bitmap1> {
    if let Some((ww, hh, b)) = cv.gfx.glass.as_ref()?.wallpaper.as_ref() {
        if *ww == w && *hh == h {
            return Some(b.clone());
        }
    }
    let mut data = vec![0u8; (w * h * 4) as usize];
    let blobs = [
        (0.15, 0.2, 0.55, PREVIEW_WALL[0]),
        (0.85, 0.3, 0.6, PREVIEW_WALL[1]),
        (0.5, 1.0, 0.65, PREVIEW_WALL[2]),
    ];
    for y in 0..h {
        for x in 0..w {
            let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
            let mut c = Color::rgb(0x101018);
            for (bx, by, r, col) in blobs {
                let d = ((u - bx).powi(2) + (v - by).powi(2)).sqrt() / r;
                c = c.lerp(col, (1.0 - d).clamp(0.0, 1.0).powf(1.4));
            }
            // a bright diagonal stripe and fine stripes show the lens bending
            let stripe = ((u * 2.0 + v * 0.6 - 0.9).abs() < 0.05) as u8 as f32;
            c = c.lerp(PREVIEW_WALL[3], stripe * 0.85);
            let fine = (((u * w as f32 / 9.0).floor() as i32) % 2 == 0) as u8 as f32;
            c = c.lerp(PREVIEW_WALL[3], fine * 0.06);
            let i = ((y * w + x) * 4) as usize;
            data[i] = (c.b * 255.0) as u8;
            data[i + 1] = (c.g * 255.0) as u8;
            data[i + 2] = (c.r * 255.0) as u8;
            data[i + 3] = 255;
        }
    }
    let dpi = unsafe {
        let (mut x, mut y) = (0.0, 0.0);
        cv.ctx.GetDpi(&mut x, &mut y);
        x
    };
    let ctx = cv.ctx.clone();
    let b = bitmap(&ctx, w, h, dpi, D2D1_BITMAP_OPTIONS_NONE, Some(&data))?;
    cv.gfx.glass.as_mut()?.wallpaper = Some((w, h, b.clone()));
    Some(b)
}

#[cfg(test)]
mod tests {
    use super::color_matrix;

    fn apply(m: &[f32; 20], c: [f32; 3]) -> [f32; 3] {
        let mut o = [0.0; 3];
        for (j, out) in o.iter_mut().enumerate() {
            *out = c[0] * m[j] + c[1] * m[4 + j] + c[2] * m[8 + j] + m[16 + j];
        }
        o
    }

    #[test]
    fn colour_matrix_lifts_saturation_and_keeps_greys() {
        let m = color_matrix(1.6, 0.0);
        let g = apply(&m, [0.5, 0.5, 0.5]);
        assert!(g.iter().all(|v| (v - 0.5).abs() < 1e-4), "grey stays grey");
        let r = apply(&m, [0.8, 0.4, 0.4]);
        assert!(
            r[0] > 0.8 && r[1] < 0.4,
            "colour moves away from grey: {r:?}"
        );
        // compression pulls toward mid grey
        let k = color_matrix(1.0, 0.2);
        let w = apply(&k, [1.0, 1.0, 1.0]);
        assert!((w[0] - 0.9).abs() < 1e-4);
        let b = apply(&k, [0.0, 0.0, 0.0]);
        assert!((b[0] - 0.1).abs() < 1e-4);
    }
}
