//! The island's glass on the UI thread: every glass parameter rides a spring (so strength,
//! preset and theme changes glide), the light and sheen drift toward the pointer, presses
//! flash, arrivals ripple through the lens, and the result is resolved into a
//! [`GlassFrame`] for the compositor each frame.

use std::time::Instant;

use super::island::View;
use super::state::State;
use crate::motion::{Spring, GENTLE, SNAPPY};
use crate::render::glass::backend::{Reason, Tier};
use crate::render::glass::{vibrancy, Backdrop, GlassFrame};
use crate::settings::TintSource;
use crate::theme::tokens::*;

pub struct GlassState {
    pub tier: Tier,
    pub reason: Option<Reason>,
    pub strength: Spring,
    pub refraction: Spring,
    pub blur: Spring,
    pub rim: Spring,
    /// Tint, rim and accent colours cross-fade (theme switches, adaptive tint).
    pub tint: [Spring; 3],
    pub rim_color: [Spring; 3],
    pub accent: [Spring; 9],
    /// Legibility: the least tint the backdrop needs.
    pub floor: Spring,
    /// The final tint alpha (tiers use different tables; switching material glides).
    pub alpha: Spring,
    /// Cross-fades between materials: the lens (Liquid only), the captured backdrop
    /// (fades in once it arrives, out for tinted glass) and the solid notch.
    pub mix_lens: Spring,
    pub mix_backdrop: Spring,
    pub mix_solid: Spring,
    /// The renderer has a captured frame this frame.
    pub backdrop_ready: bool,
    pub sheen: [Spring; 2],
    pub hover: Spring,
    pub flash: Spring,
    pub light: [Spring; 2],
    pub ripple_at: Option<Instant>,
    /// Brightest and average colour behind the notch (Liquid tier; aggregates only).
    pub backdrop_max: Color,
    pub backdrop_avg: Color,
    pub sampled: bool,
    pub last_sample: Instant,
    last_view: View,
    was_pressed: bool,
    seeded: bool,
    started: Instant,
}

fn rgb(s: &[Spring]) -> Color {
    Color {
        r: s[0].value,
        g: s[1].value,
        b: s[2].value,
        a: 1.0,
    }
}

fn set_rgb(s: &mut [Spring], c: Color, snap: bool) {
    for (sp, v) in s.iter_mut().zip([c.r, c.g, c.b]) {
        if snap {
            sp.snap(v);
        } else if (sp.target - v).abs() > 1e-4 {
            sp.set(v);
        }
    }
}

fn lerp3(v: [f32; 3], t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        v[0] + (v[1] - v[0]) * t * 2.0
    } else {
        v[1] + (v[2] - v[1]) * (t - 0.5) * 2.0
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

/// The ripple's lens boost at `t` seconds after an arrival: a damped wave (0 after it ends).
pub fn ripple(t: f32) -> f32 {
    let len = GLASS.ripple_ms / 1000.0;
    if !(0.0..len).contains(&t) {
        return 0.0;
    }
    let u = t / len;
    (u * std::f32::consts::TAU * 1.5).sin() * (1.0 - u).powi(2)
}

impl Default for GlassState {
    fn default() -> Self {
        let u = |v: f32| Spring::unit(v, GENTLE);
        Self {
            tier: Tier::Solid,
            reason: None,
            strength: u(0.5),
            refraction: u(0.5),
            blur: u(0.5),
            rim: u(0.5),
            tint: std::array::from_fn(|_| u(0.0)),
            rim_color: std::array::from_fn(|_| u(1.0)),
            accent: std::array::from_fn(|_| u(0.0)),
            floor: u(0.0),
            alpha: u(0.0),
            mix_lens: u(0.0),
            mix_backdrop: u(0.0),
            mix_solid: u(1.0),
            backdrop_ready: false,
            sheen: [Spring::new(0.0, GENTLE), Spring::new(0.0, GENTLE)],
            hover: u(0.0),
            flash: Spring::unit(0.0, SNAPPY),
            light: [u(GLASS.light.0), u(GLASS.light.1)],
            ripple_at: None,
            backdrop_max: Color::rgb(0x202020),
            backdrop_avg: Color::rgb(0x202020),
            sampled: false,
            last_sample: Instant::now(),
            last_view: View::Hidden,
            was_pressed: false,
            seeded: false,
            started: Instant::now(),
        }
    }
}

impl State {
    /// The tint the settings ask for (before strength and the legibility floor).
    fn glass_tint_target(&self) -> Color {
        let g = &self.settings.appearance.glass;
        match g.tint_source {
            TintSource::Theme => self.theme.glass.tint,
            TintSource::Custom => Color::parse(&g.custom_tint).unwrap_or(GLASS_TINT),
            TintSource::Adaptive => match (&self.adaptive, self.media.snap.playing) {
                (Some((_, pal)), true) => GLASS_TINT.lerp(pal[1], 0.22),
                _ if self.glass.sampled => GLASS_TINT.lerp(self.glass.backdrop_avg, 0.3),
                _ => self.theme.glass.tint,
            },
        }
    }

    /// Advances every glass spring. Returns true while anything moves.
    pub fn step_glass(&mut self, dt: f32, now: Instant) -> bool {
        let gs = self.settings.appearance.glass.sanitized();
        let tint = self.glass_tint_target();
        let rim_c = self.theme.glass.rim;
        let accent = self.theme.accent;
        let snap = !self.glass.seeded;
        let env = self.env;
        let reduced = env.reduced;
        let g = self.geo();
        let pointer = self.input.cursor;
        let pressed = self.input.pressed.is_some();
        let view = self.isl.view;
        let tier = self.glass.tier;
        let floor = match tier {
            Tier::Liquid | Tier::Frosted if self.glass.sampled => {
                // frosted glass lifts what's behind with a milky haze before the tint
                let behind = if tier == Tier::Frosted {
                    self.glass.backdrop_max.lerp(WHITE, GLASS.frost_haze.0)
                } else {
                    self.glass.backdrop_max
                };
                let f = vibrancy::glass_floor(behind, tint, INK, MIST);
                // in steps, and only ever up by a full step or down by two (hysteresis)
                let cur = self.glass.floor.target;
                let q = (f / 0.06).ceil() * 0.06;
                if q > cur + 0.001 || q < cur - 0.11 {
                    q
                } else {
                    cur
                }
            }
            Tier::Liquid | Tier::Frosted => GLASS.unmeasured_floor,
            // nothing to measure: legible even over a white window
            Tier::Tinted => vibrancy::glass_floor(GLASS_TINTED_WORST, tint, INK, MIST),
            Tier::Solid => 0.0,
        };
        let table = match (tier, self.settings.appearance.material) {
            (Tier::Tinted, _) => GLASS.tint_alpha_tinted,
            (Tier::Frosted, _) => GLASS.frost_tint_alpha,
            _ => GLASS.tint_alpha,
        };
        let mut alpha = lerp3(table, gs.strength).max(floor);
        let ready = self.glass.backdrop_ready;
        if tier.captures() && !ready {
            // the first frame from the screen hasn't arrived: never show unblurred desktop
            alpha = alpha.max(0.92);
        }
        let mixes = [
            (tier == Tier::Liquid) as u8 as f32,
            (tier.captures() && ready) as u8 as f32,
            (tier == Tier::Solid) as u8 as f32,
        ];
        let gl = &mut self.glass;
        for (sp, v) in [
            (&mut gl.strength, gs.strength),
            (&mut gl.refraction, gs.refraction),
            (&mut gl.blur, gs.blur),
            (&mut gl.rim, gs.rim_light),
            (&mut gl.floor, floor),
            (&mut gl.alpha, alpha),
            (&mut gl.mix_lens, mixes[0]),
            (&mut gl.mix_backdrop, mixes[1]),
            (&mut gl.mix_solid, mixes[2]),
        ] {
            if snap {
                sp.snap(v);
            } else if (sp.target - v).abs() > 1e-4 {
                sp.set(v);
            }
        }
        set_rgb(&mut gl.tint, tint, snap);
        set_rgb(&mut gl.rim_color, rim_c, snap);
        for (i, c) in accent.iter().enumerate() {
            set_rgb(&mut gl.accent[i * 3..i * 3 + 3], *c, snap);
        }
        // light and sheen: resting top-left, drifting toward the pointer over the glass
        let over = pointer.filter(|(x, y)| g.main.contains(*x, *y));
        let follow = gs.specular_follows_pointer && !reduced;
        let (sx, sy, light) = match over.filter(|_| follow) {
            Some((x, y)) => {
                let (dx, dy) = (x - g.main.cx(), y - g.main.cy());
                let len = (dx * dx + dy * dy).sqrt().max(1.0);
                let p = GLASS.pointer_pull;
                let lx = GLASS.light.0 * (1.0 - p) + dx / len * p;
                let ly = GLASS.light.1 * (1.0 - p) + dy / len * p;
                (x, y, (lx, ly))
            }
            None => (
                g.main.x + g.main.w * 0.28,
                g.main.y + g.main.h * 0.1,
                GLASS.light,
            ),
        };
        let first_sheen = gl.sheen[0].value == 0.0 && gl.sheen[1].value == 0.0;
        for (sp, v) in gl.sheen.iter_mut().zip([sx, sy]) {
            if snap || first_sheen {
                sp.snap(v);
            } else if (sp.target - v).abs() > 0.5 {
                sp.set(v);
            }
        }
        for (sp, v) in gl.light.iter_mut().zip([light.0, light.1]) {
            if (sp.target - v).abs() > 1e-3 {
                sp.set(v);
            }
        }
        let hover = if over.is_some() && follow { 1.0 } else { 0.0 };
        if (gl.hover.target - hover).abs() > 1e-3 {
            gl.hover.set(hover);
        }
        // press: a flash of light from the press point
        let press_now = pressed && over.is_some();
        if press_now && !gl.was_pressed && !reduced {
            gl.flash.snap(1.0);
            gl.flash.set(0.0);
        }
        gl.was_pressed = press_now;
        // arrivals ripple through the lens
        if view != gl.last_view {
            if matches!(view, View::Peek(_) | View::Live) && !reduced && gs.motion_response > 0.01 {
                gl.ripple_at = Some(now);
            }
            gl.last_view = view;
        }
        gl.seeded = true;
        let mut moving = false;
        for sp in [
            &mut gl.strength,
            &mut gl.refraction,
            &mut gl.blur,
            &mut gl.rim,
            &mut gl.floor,
            &mut gl.alpha,
            &mut gl.mix_lens,
            &mut gl.mix_backdrop,
            &mut gl.mix_solid,
            &mut gl.hover,
            &mut gl.flash,
        ] {
            moving |= sp.step(dt, &env);
        }
        for sp in gl
            .tint
            .iter_mut()
            .chain(gl.rim_color.iter_mut())
            .chain(gl.accent.iter_mut())
            .chain(gl.sheen.iter_mut())
            .chain(gl.light.iter_mut())
        {
            moving |= sp.step(dt, &env);
        }
        if let Some(t) = gl.ripple_at {
            if (now - t).as_secs_f32() * 1000.0 < GLASS.ripple_ms {
                moving = true;
            } else {
                gl.ripple_at = None;
            }
        }
        if gs.idle_shimmer && !reduced && gl.tier == Tier::Liquid {
            let t = gl.started.elapsed().as_secs_f32() % GLASS.shimmer_period;
            moving |= t < GLASS.shimmer_sweep;
        }
        moving
    }

    /// The accent stops after cross-fading (themes switch on springs, never a hard cut).
    pub fn glass_accent(&self) -> [Color; 3] {
        std::array::from_fn(|i| rgb(&self.glass.accent[i * 3..i * 3 + 3]))
    }

    /// Surface-tension stretch from the shape's velocity: (scale x, scale y), both ~1.
    /// Liquid Glass only: Glass is calm.
    pub fn glass_stretch(&self) -> (f32, f32) {
        let gs = &self.settings.appearance.glass;
        let k = self.glass.mix_lens.value.clamp(0.0, 1.0);
        if self.env.reduced || k < 0.01 || gs.motion_response < 0.01 {
            return (1.0, 1.0);
        }
        let k = GLASS.stretch_max * gs.motion_response.clamp(0.0, 1.0) * k;
        let vw = (self.isl.w.velocity / GLASS.stretch_velocity).clamp(-1.0, 1.0);
        let vh = (self.isl.h.velocity / GLASS.stretch_velocity).clamp(-1.0, 1.0);
        // stretch along the axis that's moving faster, preserve area-ish on the other
        let sx = 1.0 + k * vw - k * 0.5 * vh.abs();
        let sy = 1.0 + k * vh - k * 0.5 * vw.abs();
        (sx, sy)
    }

    /// Resolves this frame's glass. `backdrop` is the captured desktop (Liquid tier).
    pub fn glass_frame(&self, scale: f32, backdrop: Option<Backdrop>) -> GlassFrame {
        self.glass_frame_mix(scale, backdrop, self.glass.mix_lens.value)
    }

    /// The frame for a mix between Glass (0, frosted acrylic) and Liquid Glass (1, clear and
    /// refractive). Switching material glides through the mix.
    pub fn glass_frame_mix(&self, scale: f32, backdrop: Option<Backdrop>, mix: f32) -> GlassFrame {
        let gl = &self.glass;
        let gs = &self.settings.appearance.glass;
        let strength = gl.strength.value.clamp(0.0, 1.0);
        let lens_mix = mix.clamp(0.0, 1.0);
        let frost = 1.0 - lens_mix;
        let tint = rgb(&gl.tint).with_alpha(gl.alpha.value.clamp(0.0, 1.0));
        let blur = gl.blur.value.clamp(0.0, 1.0);
        let liquid_sigma = lerp(GLASS.sigma.0, GLASS.sigma.1, blur)
            * lerp(GLASS.sigma_strength.0, GLASS.sigma_strength.1, strength);
        let frost_sigma = lerp(GLASS.frost_sigma.0, GLASS.frost_sigma.1, blur);
        let sigma = lerp(frost_sigma, liquid_sigma, lens_mix);
        let ripple = gl
            .ripple_at
            .map(|t| ripple(t.elapsed().as_secs_f32()) * gs.motion_response * lens_mix)
            .unwrap_or(0.0);
        // the lens swells a little with the shape's speed (surface tension)
        let speed = if self.env.reduced {
            0.0
        } else {
            ((self.isl.w.velocity.abs() + self.isl.h.velocity.abs()) / GLASS.stretch_velocity)
                .min(1.0)
                * gs.motion_response
                * lens_mix
        };
        let lens = GLASS.lens_max
            * gl.refraction.value.clamp(0.0, 1.0)
            * (1.0 + GLASS.ripple_boost * ripple + 0.4 * speed);
        let rim = gl.rim.value.clamp(0.0, 1.0);
        let flash = gl.flash.value.clamp(0.0, 1.0);
        let (lx, ly) = (gl.light[0].value, gl.light[1].value);
        let ll = (lx * lx + ly * ly).sqrt().max(1e-3);
        let g = self.geo();
        let shimmer = if gs.idle_shimmer && !self.env.reduced {
            let t = gl.started.elapsed().as_secs_f32() % GLASS.shimmer_period;
            if t < GLASS.shimmer_sweep {
                (t / GLASS.shimmer_sweep * std::f32::consts::PI).sin()
            } else {
                0.0
            }
        } else {
            0.0
        };
        let (sx, sy) = if shimmer > 0.0 {
            let t =
                (gl.started.elapsed().as_secs_f32() % GLASS.shimmer_period) / GLASS.shimmer_sweep;
            (g.main.x + g.main.w * t, g.main.y + g.main.h * 0.2)
        } else {
            (gl.sheen[0].value, gl.sheen[1].value)
        };
        // frosted glass keeps a still, soft top light; liquid glass's light follows the pointer
        // and flashes on press
        let sheen_a = GLASS.sheen_alpha * (0.35 + 0.65 * gl.hover.value.clamp(0.0, 1.0) * lens_mix)
            + (GLASS.sheen_flash_alpha * flash + GLASS.sheen_flash_alpha * 0.6 * shimmer)
                * lens_mix;
        let (sx, sy) = (
            lerp(g.main.x + g.main.w * 0.3, sx, lens_mix),
            lerp(g.main.y, sy, lens_mix),
        );
        let liquid_lit =
            GLASS.rim_alpha_lit * (0.35 + 0.65 * rim) * (1.0 + 0.6 * flash + 0.5 * speed);
        let liquid_unlit = GLASS.rim_alpha_unlit * (0.5 + 0.5 * rim);
        let frost_lit = GLASS.frost_rim_lit * (0.5 + 0.5 * rim);
        let frost_unlit = GLASS.frost_rim_unlit * (0.5 + 0.5 * rim);
        GlassFrame {
            tier: gl.tier,
            tint,
            sigma,
            lens: lens * lens_mix,
            saturation: lerp(
                GLASS.frost_saturation,
                lerp(GLASS.saturation.1, GLASS.saturation.0, strength),
                lens_mix,
            ),
            luma_compress: GLASS.luma_compress,
            rim_width: lerp(
                GLASS.frost_rim_width,
                lerp(GLASS.rim_width.0, GLASS.rim_width.1, rim),
                lens_mix,
            ),
            rim_lit: lerp(frost_lit, liquid_lit, lens_mix),
            rim_unlit: lerp(frost_unlit, liquid_unlit, lens_mix),
            rim: rgb(&gl.rim_color),
            light: (lx / ll, ly / ll),
            sheen: Some((sx, sy, sheen_a)),
            sheen_color: self.theme.glass.sheen,
            inner_glow: GLASS.inner_glow_alpha * (0.5 + 0.5 * rim),
            inner_shadow: GLASS.inner_shadow_alpha * (0.45 + 0.55 * strength),
            shadow: if g.floating { GLASS.shadow_alpha } else { 0.0 },
            shadow_tint: if gl.sampled {
                self.theme.glass.shadow_tint.lerp(gl.backdrop_avg, 0.25)
            } else {
                self.theme.glass.shadow_tint
            },
            tile: 1.0 + (1.0 - strength) * 0.5,
            high_contrast: self.theme.high_contrast,
            scale,
            backdrop,
            backdrop_alpha: gl.mix_backdrop.value.clamp(0.0, 1.0),
            solid_mix: gl.mix_solid.value.clamp(0.0, 1.0),
            // the frosted look needs a backdrop to frost; tinted glass already carries a tint
            haze: frost
                * gl.mix_backdrop.value.clamp(0.0, 1.0)
                * lerp(GLASS.frost_haze.0, GLASS.frost_haze.1, strength),
            grain: frost * GLASS.frost_grain,
        }
    }
}

/// The Settings preview: the chosen material over a sample wallpaper, whatever tier the
/// island itself is on right now.
pub fn preview_frame(
    st: &State,
    tier: Tier,
    scale: f32,
    backdrop: Option<Backdrop>,
    sheen_at: (f32, f32),
    floating: bool,
) -> GlassFrame {
    let liquid = tier == Tier::Liquid;
    let mut f = st.glass_frame_mix(scale, backdrop, liquid as u8 as f32);
    let strength = st.glass.strength.value.clamp(0.0, 1.0);
    f.tier = tier;
    f.tint.a = lerp3(
        if liquid {
            GLASS.tint_alpha
        } else {
            GLASS.frost_tint_alpha
        },
        strength,
    );
    f.backdrop_alpha = 1.0;
    f.solid_mix = 0.0;
    if liquid {
        f.lens = GLASS.lens_max * st.glass.refraction.value.clamp(0.0, 1.0);
    } else {
        f.lens = 0.0;
        f.haze = lerp(GLASS.frost_haze.0, GLASS.frost_haze.1, strength);
    }
    f.sheen = Some((sheen_at.0, sheen_at.1, GLASS.sheen_alpha * 1.6));
    f.shadow = if floating { GLASS.shadow_alpha } else { 0.0 };
    f
}

#[cfg(test)]
mod tests {
    use super::{lerp3, ripple};
    use crate::theme::tokens::GLASS;

    #[test]
    fn strength_maps_through_the_three_stops() {
        let t = GLASS.tint_alpha;
        assert_eq!(lerp3(t, 0.0), t[0]);
        assert_eq!(lerp3(t, 0.5), t[1]);
        assert_eq!(lerp3(t, 1.0), t[2]);
        // continuous and monotonic: no jumps along the slider
        let mut last = lerp3(t, 0.0);
        for i in 1..=100 {
            let v = lerp3(t, i as f32 / 100.0);
            assert!(v >= last && v - last < 0.02);
            last = v;
        }
    }

    #[test]
    fn ripple_settles() {
        assert_eq!(ripple(-0.1), 0.0);
        assert_eq!(ripple(GLASS.ripple_ms / 1000.0 + 0.01), 0.0);
        assert!(ripple(0.06).abs() > 0.1);
        assert!(ripple(GLASS.ripple_ms / 1000.0 - 0.01).abs() < 0.01);
    }
}
