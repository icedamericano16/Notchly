//! Drawing: the island shell (shape, glow, privacy dots) and content dispatch.

pub mod ask;
pub mod compact;
pub mod control;
pub mod dashboard;
pub mod diagnostics;
pub mod media;
pub mod notes;
pub mod pages;
pub mod productivity;
pub mod timer;
pub mod todo;
pub mod trash;
pub mod weather;
pub mod widgets;

use std::time::Instant;

use crate::app::activity::PopKind;
use crate::app::island::View;
use crate::app::state::State;
use crate::app::{Action, InputId, PageId};
use crate::motion::{choreo, step_response, MotionEnv, GENTLE};
use crate::render::glass::GlassLayer;
use crate::render::icons::Icon;
use crate::render::shapes;
use crate::render::text::{self, Align};
use crate::render::{Canvas, Rect};
use crate::services::privacy::Cap;
use crate::theme::tokens::*;

/// Staggered reveal timing for content rows.
#[derive(Clone, Copy)]
pub struct Reveal {
    pub t: Option<f32>,
    pub env: MotionEnv,
    pub skip_art: bool,
}

impl Reveal {
    pub fn full() -> Self {
        Self {
            t: Some(10.0),
            env: MotionEnv::default(),
            skip_art: false,
        }
    }
    /// Progress 0..~1 of row `i`.
    pub fn row(&self, i: usize) -> f32 {
        match self.t {
            None => 0.0,
            Some(t) => {
                if self.env.reduced {
                    (t * 1000.0 / choreo::REDUCED_FADE_MS).clamp(0.0, 1.0)
                } else {
                    step_response(
                        t - i as f32 * choreo::STAGGER_MS / 1000.0,
                        GENTLE,
                        &self.env,
                    )
                    .clamp(0.0, 1.08)
                }
            }
        }
    }
}

/// Draws `f` as row `i` of a staggered reveal (fade + rise + scale).
pub fn row<F: FnOnce(&mut Canvas)>(cv: &mut Canvas, rv: &Reveal, i: usize, r: Rect, f: F) {
    let p = rv.row(i);
    if p <= 0.001 {
        return;
    }
    let reduced = rv.env.reduced;
    cv.push_alpha(p.min(1.0));
    let dy = if reduced {
        0.0
    } else {
        (1.0 - p) * choreo::RISE_DIP
    };
    let sc = if reduced {
        1.0
    } else {
        choreo::SCALE_FROM + (1.0 - choreo::SCALE_FROM) * p.min(1.0)
    };
    cv.push_tf(0.0, dy, sc, r.cx(), r.cy());
    f(cv);
    cv.pop_tf();
    cv.pop_alpha();
}

/// Main entry: draws the whole island for this frame.
pub fn draw_island(st: &mut State, cv: &mut Canvas) {
    #[cfg(debug_assertions)]
    if std::env::var("NOTCHLY_QA_ICONS").is_ok() {
        icon_sheet(cv);
        return;
    }
    let now = Instant::now();
    let g = st.geo();
    if g.main.h < 0.3 {
        return;
    }
    cv.theme.accent = st.accent();
    let factory = cv.gfx.factory.clone();
    let glass = cv.glass.clone().filter(|g| g.is_glass());
    let radius = g.radius * cv.theme.radius_scale;
    let Ok(path) = shapes::notch_path(
        &factory,
        g.main.cx(),
        g.main.y,
        g.main.w,
        g.main.h,
        radius,
        g.shoulder,
        g.floating,
    ) else {
        return;
    };
    // surface tension: while the shape springs, the glass stretches a little along the
    // way it moves (settles with the shape; none with reduced motion)
    let (sx, sy) = st.glass_stretch();
    let stretched = (sx - 1.0).abs() > 1e-4 || (sy - 1.0).abs() > 1e-4;
    if stretched {
        cv.push_affine(0.0, 0.0, sx, sy, 0.0, g.main.cx(), g.main.y);
    }

    // floating shadow + legibility ring (the glass draws its own, tinted by what's behind)
    if g.floating && glass.is_none() {
        cv.push_tf(0.0, 12.0, 1.0, 0.0, 0.0);
        cv.glow_geo(&path, SHADOW, 16.0, 0.9);
        cv.pop_tf();
    }
    // glow pulses (timer done, low battery breathing)
    if let Some((amt, c)) = st.isl.glow(now) {
        cv.glow_geo(&path, c.with_alpha(0.6), 6.0, amt);
        cv.glow_geo(&path, c.with_alpha(0.28), 14.0, amt);
    }

    // dual island: the bubble and the liquid bridge between them
    let bubble_geo = g.bubble.and_then(|b| {
        shapes::notch_path(
            &factory,
            b.cx(),
            b.y,
            b.w,
            b.h,
            b.w / 2.0 - 1.0,
            if g.floating {
                0.0
            } else {
                (g.shoulder * 0.6).min(8.0)
            },
            g.floating,
        )
        .ok()
    });
    let goo = g.bubble.and_then(|b| {
        shapes::goo_bridge(
            &factory,
            g.main.right(),
            b.x,
            g.main.y,
            g.main.h.min(b.h),
            DUAL_GAP + 0.5,
            g.floating,
        )
        .ok()
        .flatten()
    });
    match &glass {
        Some(gf) => {
            use crate::render::glass::{compositor, lens::Body};
            // how bright is it behind? (aggregate only; drives the legibility floor)
            if gf.tier.captures()
                && gf.backdrop.is_some()
                && st.glass.last_sample.elapsed()
                    >= std::time::Duration::from_millis(GLASS.luma_sample_ms)
            {
                st.glass.last_sample = now;
                if let Some((max, avg)) = compositor::sample(cv, gf, g.main) {
                    st.glass.backdrop_max = max;
                    st.glass.backdrop_avg = avg;
                    st.glass.sampled = true;
                }
            }
            // one piece of glass: main body + bridge + bubble, so the drops read as liquid
            let geo = union(&factory, &path, goo.as_ref(), bubble_geo.as_ref());
            let mut bodies = vec![Body {
                x: g.main.x,
                y: g.main.y,
                w: g.main.w,
                h: g.main.h,
                radius,
                open_top: !g.floating,
            }];
            if let Some(b) = g.bubble {
                bodies.push(Body {
                    x: b.x,
                    y: b.y,
                    w: b.w,
                    h: b.h,
                    radius: b.w / 2.0,
                    open_top: !g.floating,
                });
            }
            if let Some(geo) = geo {
                compositor::surface(cv, &geo, &bodies, gf, g.floating);
            }
            // the Fluid Style, inside the glass and under the content (a still frame in Glass)
            let style = crate::render::glass::fluid::style(&st.settings.appearance.fluid.style);
            if style.animates() && !st.theme.high_contrast {
                let marks = crate::render::glass::fluid::marks(style, &st.fluid_ctx(g.main));
                if !marks.is_empty() {
                    cv.push_geo_clip(&path);
                    crate::render::glass::fluid::draw(cv, &marks);
                    cv.pop_layer();
                }
            }
        }
        None => {
            cv.fill_geo(&path, NOTCH);
            if g.floating {
                cv.stroke_geo(&path, RING, 1.0);
            }
            if let Some(gg) = &goo {
                cv.fill_geo(gg, NOTCH);
            }
            if let Some(bp) = &bubble_geo {
                cv.fill_geo(bp, NOTCH);
            }
        }
    }
    if let (Some(b), Some(_)) = (g.bubble, &bubble_geo) {
        if g.bubble_t > 0.6 {
            cv.push_alpha(((g.bubble_t - 0.6) / 0.4).min(1.0));
            compact::bubble(st, cv, b);
            cv.pop_alpha();
        }
    }

    // content, clipped to the notch shape
    cv.push_geo_clip(&path);
    // the dock grows the notch downward; page content keeps its size and position
    let mut cmain = g.main;
    cmain.h = (cmain.h - st.dock_extra()).max(0.0);
    let area = content_area(&cmain, st.isl.view);
    let rv = Reveal {
        t: st.isl.reveal_at.map(|r| (now - r).as_secs_f32()),
        env: st.env,
        skip_art: st.isl.art_morph.is_some(),
    };
    if let Some((pv, at, fade)) = st.isl.prev {
        let a = 1.0 - ((now - at).as_secs_f32() * 1000.0 / fade.max(1.0)).clamp(0.0, 1.0);
        if a > 0.0 {
            cv.push_alpha(a);
            let parea = content_area(&cmain, pv);
            let prv = Reveal {
                skip_art: st.isl.art_morph.is_some(),
                ..Reveal::full()
            };
            draw_view(st, cv, pv, parea, &prv, true);
            cv.pop_alpha();
        }
    }
    let cur = st.isl.view;
    draw_view(st, cv, cur, area, &rv, false);
    // shared element: album art morphs between its live (20 px) and page (60 px) rects
    if let Some(to_page) = st.isl.art_morph {
        let la = content_area(&g.main, View::Live);
        let live_art = Rect::centered(la.x + 15.0, la.cy(), 20.0, 20.0);
        let pa = content_area(&g.main, View::Page(PageId::Music));
        let page_art = Rect::new(pa.x, pa.y + 2.0, 60.0, 60.0);
        let p = st.isl.progress();
        let (a, b, ra, rb) = if to_page {
            (live_art, page_art, R_ART_LIVE, R_ART)
        } else {
            (page_art, live_art, R_ART, R_ART_LIVE)
        };
        let l = |x: f32, y: f32| x + (y - x) * p;
        let r = Rect::new(l(a.x, b.x), l(a.y, b.y), l(a.w, b.w), l(a.h, b.h));
        match st.media.art.as_mut() {
            Some(img) => cv.image(img, r, l(ra, rb)),
            None => cv.rrect(r, l(ra, rb), SURFACE_RAISED),
        }
    }
    cv.pop_layer();

    // privacy dots on the right shoulder (any state)
    if st.settings.appearance.privacy_dots && !st.f.privacy.uses.is_empty() {
        let caps: Vec<Cap> = {
            let mut c: Vec<Cap> = st.f.privacy.uses.iter().map(|u| u.cap).collect();
            c.sort_by_key(|c| *c as u8);
            c.dedup();
            c
        };
        let y = if g.main.h > 40.0 {
            g.main.y + 12.0
        } else {
            g.main.cy()
        };
        let mut x = if g.main.h > 40.0 {
            g.main.right() - g.radius.min(g.main.h / 2.0) * 0.6 - 4.0
        } else {
            // compact states: the idle layout reserves exactly this room
            g.main.right() - PRIVACY_DOT_INSET
        };
        for c in caps.iter().rev() {
            let col = match c {
                Cap::Mic => PRIVACY_MIC,
                Cap::Camera => PRIVACY_CAM,
                Cap::Location => PRIVACY_LOC,
            };
            // dots fade and scale in over 160 ms (never blink)
            let pin =
                st.f.privacy
                    .since
                    .map(|t| (t.elapsed().as_secs_f32() / 0.16).min(1.0))
                    .unwrap_or(1.0);
            cv.circle(x, y, 3.0 * (0.5 + 0.5 * pin), col.with_alpha(pin));
            x -= 9.0;
        }
        cv.zone_named(
            Rect::new(x, y - 8.0, g.main.right() - x, 16.0),
            Action::Privacy,
            false,
            "Privacy indicators",
            8.0,
        );
    }

    // marquee tooltip for the music live pill
    if st.marquee_visible() {
        let text = format!("{} — {}", st.media.snap.title, st.media.snap.artist);
        let w = (cv.measure(&text, &text::CAPTION) + 28.0).min(300.0);
        let r = Rect::new(g.main.cx() - w / 2.0, g.main.bottom() + 6.0, w, 24.0);
        cv.rrect(r, 12.0, SURFACE_RAISED);
        let t = st.started.elapsed().as_secs_f32();
        let tw = cv.measure(&text, &text::CAPTION);
        if tw > w - 28.0 {
            let span = tw - (w - 28.0) + 30.0;
            let off = ((t * 30.0) % (span * 2.0) - span).abs() - 15.0;
            cv.push_clip(r.inset_xy(14.0, 0.0));
            cv.text(
                &text,
                &text::CAPTION,
                Rect::new(r.x + 14.0 - off.clamp(0.0, span - 30.0), r.y, tw + 4.0, r.h),
                cv.theme.ink,
                Align::Left,
            );
            cv.pop_clip();
        } else {
            cv.text(&text, &text::CAPTION, r, cv.theme.ink, Align::Center);
        }
    }
    if stretched {
        cv.pop_tf();
    }
}

/// The union outline of the island's parts (one geometry, so the glass and its rim are
/// continuous across the liquid bridge).
fn union(
    factory: &windows::Win32::Graphics::Direct2D::ID2D1Factory1,
    main: &windows::Win32::Graphics::Direct2D::ID2D1PathGeometry,
    goo: Option<&windows::Win32::Graphics::Direct2D::ID2D1PathGeometry>,
    bubble: Option<&windows::Win32::Graphics::Direct2D::ID2D1PathGeometry>,
) -> Option<windows::Win32::Graphics::Direct2D::ID2D1Geometry> {
    use windows::core::Interface;
    use windows::Win32::Graphics::Direct2D::*;
    let main_g: ID2D1Geometry = main.cast().ok()?;
    let mut parts: Vec<Option<ID2D1Geometry>> = vec![Some(main_g.clone())];
    for p in [goo, bubble].into_iter().flatten() {
        parts.push(p.cast().ok());
    }
    if parts.len() == 1 {
        return Some(main_g);
    }
    unsafe {
        let base: &ID2D1Factory = factory;
        let group = base
            .CreateGeometryGroup(Common::D2D1_FILL_MODE_WINDING, &parts)
            .ok()?;
        let out: ID2D1PathGeometry = factory.CreatePathGeometry().ok()?.into();
        let sink = out.Open().ok()?;
        group.Outline(None, 0.25, &sink).ok()?;
        sink.Close().ok()?;
        out.cast().ok()
    }
}

/// The bell's swing (radians) just after a notification arrives; 0 otherwise.
pub fn bell_angle(st: &State) -> f32 {
    if st.env.reduced {
        return 0.0;
    }
    match st.f.notifs.arrived {
        Some(t) if t.elapsed().as_secs_f32() < BELL_SHAKE_SECS => {
            let t = t.elapsed().as_secs_f32();
            0.32 * (t * 26.0).sin() * (1.0 - t / BELL_SHAKE_SECS).powi(2)
        }
        _ => 0.0,
    }
}

/// Content rect inside the pill for a view.
pub fn content_area(main: &Rect, v: View) -> Rect {
    match v {
        View::Page(_) => Rect::new(
            main.x + PAD + 4.0,
            main.y + TOP_INSET + 4.0,
            (main.w - 2.0 * (PAD + 4.0)).max(0.0),
            (main.h - TOP_INSET - 4.0 - 14.0).max(0.0),
        ),
        View::Peek(_) => Rect::new(
            main.x + 18.0,
            main.y + 6.0,
            (main.w - 36.0).max(0.0),
            (main.h - 10.0).max(0.0),
        ),
        _ => Rect::new(main.x + 10.0, main.y, (main.w - 20.0).max(0.0), main.h),
    }
}

fn draw_view(st: &mut State, cv: &mut Canvas, v: View, area: Rect, rv: &Reveal, outgoing: bool) {
    match v {
        View::Hidden | View::Minimal => {}
        View::Idle => compact::idle(st, cv, area),
        View::Live => {
            let roll = st.isl.roll_t.value;
            if let (Some(prev), false) = (st.isl.prev_live.clone(), outgoing) {
                if roll < 1.0 {
                    cv.push_alpha(1.0 - roll);
                    cv.push_tf(0.0, -choreo::ROLL_DIP * roll, 1.0, 0.0, 0.0);
                    compact::live(st, cv, area, &prev, rv);
                    cv.pop_tf();
                    cv.pop_alpha();
                }
            }
            if let Some(key) = st.act.main().map(|a| a.key.clone()) {
                let dy = if st.isl.prev_live.is_some() && !outgoing {
                    choreo::ROLL_DIP * (1.0 - roll)
                } else {
                    0.0
                };
                cv.push_alpha(if st.isl.prev_live.is_some() && !outgoing {
                    roll
                } else {
                    1.0
                });
                cv.push_tf(0.0, dy, 1.0, 0.0, 0.0);
                compact::live(st, cv, area, &key, rv);
                cv.pop_tf();
                cv.pop_alpha();
            }
        }
        View::Peek(_) => compact::peek(st, cv, area, rv),
        View::Page(p) => {
            let t = st.isl.page_t.value;
            let dir = st.isl.page_dir;
            let slide = if st.env.reduced {
                0.0
            } else {
                choreo::PAGE_SLIDE_DIP
            };
            if let (Some(prev), false) = (st.isl.prev_page, outgoing) {
                if t < 1.0 {
                    cv.push_alpha((1.0 - t * 1.6).max(0.0));
                    cv.push_tf(-dir * slide * t, 0.0, 1.0, 0.0, 0.0);
                    draw_page(st, cv, prev, area, &Reveal::full());
                    cv.pop_tf();
                    cv.pop_alpha();
                }
                cv.push_alpha(t.min(1.0));
                cv.push_tf(dir * slide * (1.0 - t), 0.0, 1.0, 0.0, 0.0);
                draw_page(st, cv, p, area, &Reveal::full());
                cv.pop_tf();
                cv.pop_alpha();
            } else {
                draw_page(st, cv, p, area, rv);
            }
            if !outgoing {
                page_chrome(st, cv, area, rv);
            }
        }
    }
}

pub fn draw_page(st: &mut State, cv: &mut Canvas, p: PageId, area: Rect, rv: &Reveal) {
    match p {
        PageId::Dashboard => dashboard::draw(st, cv, area, rv),
        PageId::Music => media::music(st, cv, area, rv),
        PageId::Lyrics => media::music(st, cv, area, rv),
        PageId::Timer => timer::draw(st, cv, area, rv),
        PageId::Water => productivity::water(st, cv, area, rv),
        PageId::Todo => todo::draw(st, cv, area, rv),
        PageId::Shelf => productivity::shelf(st, cv, area, rv),
        PageId::Clipboard => productivity::clipboard(st, cv, area, rv),
        PageId::Prayer => productivity::prayer(st, cv, area, rv),
        PageId::Ocr => productivity::ocr(st, cv, area, rv),
        PageId::Ask => ask::draw(st, cv, area, rv),
        PageId::Weather => weather::draw(st, cv, area, rv),
        PageId::Calendar => pages::calendar(st, cv, area, rv),
        PageId::Notifications => pages::notifications(st, cv, area, rv),
        PageId::Privacy => pages::privacy(st, cv, area, rv),
        PageId::SysMon => pages::sysmon(st, cv, area, rv),
        PageId::Notes => notes::draw(st, cv, area, rv),
        PageId::Meeting => pages::meeting(st, cv, area, rv),
        PageId::Downloads => pages::downloads(st, cv, area, rv),
        PageId::Live => pages::live(st, cv, area, rv),
        PageId::Onboarding => pages::onboarding(st, cv, area, rv),
        PageId::Annotate => pages::annotate(st, cv, area, rv),
        PageId::Widgets => widgets::page(st, cv, area, rv),
        PageId::Control => control::draw(st, cv, area, rv),
        PageId::Diagnostics => diagnostics::draw(st, cv, area, rv),
    }
}

/// Page dots + expanded overlays (volume bar, notification banner).
fn page_chrome(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    let car = st.carousel();
    if let Some(idx) = car.iter().position(|p| *p == st.page) {
        let n = car.len();
        let t = st.isl.dock.value.clamp(0.0, 1.15);
        let tc = t.min(1.0);
        let base_y = area.bottom() + 6.0;
        cv.push_alpha(rv.row(4).min(1.0));
        // page dots (fade out as the dock opens)
        if tc < 0.98 {
            cv.push_alpha(1.0 - tc);
            let total = n as f32 * 10.0;
            let x0 = area.cx() - total / 2.0 + 5.0;
            for (i, p) in car.iter().enumerate() {
                let x = x0 + i as f32 * 10.0;
                if i == idx {
                    cv.rrect(Rect::centered(x, base_y, 12.0, 4.0), 2.0, cv.theme.ink);
                } else {
                    cv.circle(x, base_y, 2.0, FAINT);
                }
                if tc < 0.5 {
                    cv.zone_named(
                        Rect::centered(x, base_y, 10.0, 14.0),
                        Action::PageDot(i),
                        false,
                        p.title(),
                        4.0,
                    );
                }
            }
            // pin + close
            let pin_c = if st.pinned { cv.theme.accent[0] } else { FAINT };
            cv.button(
                Icon::Pin,
                area.right() - 24.0,
                base_y,
                18.0,
                Action::TogglePin,
                if st.pinned { "Unpin" } else { "Keep open" },
                pin_c,
            );
            cv.button(
                Icon::Close,
                area.right() - 4.0,
                base_y,
                18.0,
                Action::Collapse,
                "Close",
                FAINT,
            );
            cv.pop_alpha();
        }
        // hovering the dots morphs them into an icon dock (iOS-style capsule)
        // icon spacing adapts so the dock always fits inside the notch
        let gap = ((area.w + 8.0 - 50.0) / n as f32).clamp(22.0, 30.0);
        let dots_w = n as f32 * 10.0 + 8.0;
        let full_w = n as f32 * gap + 50.0;
        let w = dots_w + (full_w - dots_w) * t;
        let h = 6.0 + 26.0 * t;
        // the capsule settles into the space the notch grows below the content
        let dock_y = area.bottom() + (14.0 + crate::app::island::DOCK_EXTRA) / 2.0;
        let y = base_y + (dock_y - base_y) * tc;
        if tc > 0.01 {
            let cap = Rect::centered(area.cx(), y, w, h);
            cv.push_alpha(tc);
            if cv.is_glass() {
                cv.glass_tile(cap, cap.h / 2.0, GlassLayer::Raised);
            } else {
                cv.rrect(cap, cap.h / 2.0, SURFACE);
                cv.stroke_rrect(cap.inset(0.5), cap.h / 2.0, RING, 1.0);
            }
            let ia = ((tc - 0.25) / 0.75).clamp(0.0, 1.0);
            let slot = (cap.w - 50.0).max(1.0) / n as f32;
            cv.push_alpha(ia);
            for (i, p) in car.iter().enumerate() {
                let x = cap.x + 8.0 + slot * (i as f32 + 0.5);
                let (ic, tint) = page_icon(*p);
                let r = Rect::centered(x, cap.cy(), slot, cap.h);
                let (hv, pv) = cv.anim(&Action::PageDot(i), cv.hovered(r), false);
                let active = i == idx;
                let s = (0.75 + 0.25 * tc) * (1.0 + 0.08 * hv) * (1.0 - 0.08 * pv);
                if active {
                    cv.glass_glyph(
                        ic,
                        Rect::centered(x, cap.cy(), 25.0 * s, 25.0 * s),
                        tint,
                        1.0,
                    );
                } else {
                    if hv > 0.01 {
                        cv.circle(x, cap.cy(), 12.0, LINE.with_alpha(hv.min(1.0)));
                    }
                    cv.icon_weight(
                        ic,
                        x,
                        cap.cy(),
                        15.0 * s,
                        cv.theme.mist.lerp(cv.theme.ink, hv.min(1.0)),
                        400,
                    );
                }
                cv.zone_named(r, Action::PageDot(i), true, p.title(), 12.0);
            }
            let sx = cap.right() - 22.0;
            cv.line(
                sx - 12.0,
                cap.y + 8.0,
                sx - 12.0,
                cap.bottom() - 8.0,
                1.0,
                LINE.with_alpha(2.0),
            );
            cv.button(
                Icon::Settings,
                sx,
                cap.cy(),
                26.0,
                Action::OpenSettings,
                "Settings",
                cv.theme.mist,
            );
            cv.pop_alpha();
            cv.pop_alpha();
        }
        st.isl.dock_rect = if tc > 0.3 {
            Rect::centered(area.cx(), dock_y, full_w, 34.0)
        } else {
            Rect::centered(area.cx(), base_y, dots_w + 16.0, 16.0)
        };
        cv.pop_alpha();
    }
    // while expanded, pop-ups don't take over
    if let Some(p) = st.pop.current.clone() {
        match p.kind {
            PopKind::Volume | PopKind::Brightness => {
                let v = st.isl.peek_value.value;
                let r = Rect::new(area.x + 40.0, area.bottom() - 2.0, area.w - 80.0, 4.0);
                cv.icon(p.icon, r.x - 14.0, r.cy(), 14.0, cv.theme.mist);
                cv.progress(r, v, false);
            }
            PopKind::Notification
            | PopKind::Reminder
            | PopKind::TimerDone
            | PopKind::Info
            | PopKind::Download
            | PopKind::Approval
            | PopKind::Digest => {
                let r = Rect::new(area.x, area.y - 2.0, area.w, 44.0);
                cv.sq(r, 14.0, SURFACE_RAISED);
                cv.icon(
                    p.icon,
                    r.x + 20.0,
                    r.cy(),
                    18.0,
                    p.tint.unwrap_or(cv.theme.accent[0]),
                );
                cv.text(
                    &p.title,
                    &text::BODY_MED,
                    Rect::new(r.x + 38.0, r.y + 4.0, r.w - 90.0, 18.0),
                    cv.theme.ink,
                    Align::Left,
                );
                cv.text(
                    &p.subtitle,
                    &text::CAPTION,
                    Rect::new(r.x + 38.0, r.y + 22.0, r.w - 90.0, 16.0),
                    cv.theme.mist,
                    Align::Left,
                );
                if let Some((label, _)) = p.actions.first() {
                    let b = Rect::new(r.right() - 74.0, r.cy() - 12.0, 66.0, 24.0);
                    cv.pill_button(label, b, Action::PopupButton(p.id, 0), true);
                } else {
                    cv.button(
                        Icon::Close,
                        r.right() - 20.0,
                        r.cy(),
                        24.0,
                        Action::PopupDismiss,
                        "Dismiss",
                        cv.theme.mist,
                    );
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------- shared widgets

/// A page header: icon + title on the left, optional trailing text.
pub fn header(cv: &mut Canvas, area: Rect, icon: Icon, title: &str, trailing: &str) -> Rect {
    let tint = icon_tint(icon, cv.theme.accent[0]);
    cv.glass_glyph(icon, Rect::new(area.x, area.y - 1.0, 22.0, 22.0), tint, 1.0);
    cv.text(
        &crate::i18n::tr(title),
        &text::TITLE,
        Rect::new(area.x + 30.0, area.y, area.w * 0.6, 20.0),
        cv.theme.ink,
        Align::Left,
    );
    if !trailing.is_empty() {
        cv.text(
            trailing,
            &text::CAPTION,
            Rect::new(area.x + area.w * 0.4, area.y, area.w * 0.6, 20.0),
            cv.theme.mist,
            Align::Right,
        );
    }
    Rect::new(area.x, area.y + 28.0, area.w, area.h - 28.0)
}

/// Single- or multi-line text field with caret, selection and placeholder.
pub fn text_input(
    st: &State,
    cv: &mut Canvas,
    id: InputId,
    r: Rect,
    placeholder: &str,
    icon: Option<Icon>,
) {
    let focused = st.input.text_focus == Some(id);
    if cv.is_glass() {
        cv.glass_tile(r, R_INPUT, GlassLayer::Raised);
        if focused {
            cv.sq(r, R_INPUT, WHITE.with_alpha(0.04));
        }
    } else {
        cv.sq(
            r,
            R_INPUT,
            if focused {
                cv.theme.surface_raised.lerp(INK, 0.04)
            } else {
                cv.theme.surface_raised
            },
        );
    }
    if focused {
        cv.stroke_rrect(
            r.inset(0.5),
            R_INPUT,
            cv.theme.accent[1].with_alpha(0.6),
            1.0,
        );
    }
    let mut x = r.x + 12.0;
    if let Some(ic) = icon {
        cv.icon(ic, x + 7.0, r.cy(), 15.0, cv.theme.mist);
        x += 22.0;
    }
    let tr = Rect::new(x, r.y, r.right() - x - 10.0, r.h);
    let field = st.input.text_ref(id).cloned().unwrap_or_default();
    if field.text.is_empty() {
        cv.label(placeholder, &text::BODY, tr, FAINT, Align::Left);
    }
    let multiline = r.h > 40.0;
    let style = if multiline {
        text::BODY.lines(20)
    } else {
        text::BODY
    };
    // scroll horizontally so the caret stays visible (single line)
    let prefix: String = field.text.chars().take(field.caret).collect();
    let caret_x = cv.measure(&prefix, &text::BODY);
    let off = if !multiline {
        (caret_x - tr.w + 4.0).max(0.0)
    } else {
        0.0
    };
    cv.push_clip(tr.inset_xy(-2.0, 0.0));
    if let Some((a, b)) = field.selection().filter(|_| !multiline) {
        let pa: String = field.text.chars().take(a).collect();
        let pb: String = field.text.chars().take(b).collect();
        let (xa, xb) = (cv.measure(&pa, &text::BODY), cv.measure(&pb, &text::BODY));
        cv.rect(
            Rect::new(tr.x + xa - off, r.cy() - 9.0, xb - xa, 18.0),
            cv.theme.accent[1].with_alpha(0.4),
        );
    }
    if multiline {
        cv.text_top(
            &field.text,
            &style,
            tr.x,
            r.y + 10.0,
            tr.w,
            cv.theme.ink,
            Align::Left,
        );
    } else {
        cv.text(
            &field.text,
            &text::BODY,
            Rect::new(tr.x - off, tr.y, tr.w + off + 400.0, tr.h),
            cv.theme.ink,
            Align::Left,
        );
    }
    if focused && (st.input.caret_blink.elapsed().as_millis() / 530).is_multiple_of(2) {
        if multiline {
            // caret at end of the laid-out text block (approximation for multi-line)
            let lines = field.text[..field.byte(field.caret)].lines().count().max(1);
            let last = field.text[..field.byte(field.caret)]
                .rsplit('\n')
                .next()
                .unwrap_or("");
            let lx = cv.measure(last, &text::BODY);
            let ly = r.y
                + 10.0
                + (lines as f32
                    - if field.text[..field.byte(field.caret)].ends_with('\n') {
                        0.0
                    } else {
                        1.0
                    })
                    * 17.7;
            cv.rect(
                Rect::new(
                    tr.x + if field.text[..field.byte(field.caret)].ends_with('\n') {
                        0.0
                    } else {
                        lx
                    },
                    ly,
                    1.5,
                    16.0,
                ),
                cv.theme.accent[0],
            );
        } else {
            cv.rect(
                Rect::new(tr.x + caret_x - off, r.cy() - 8.0, 1.5, 16.0),
                cv.theme.accent[0],
            );
        }
    }
    cv.pop_clip();
    cv.zone_named(r, Action::Focus(id), true, placeholder, R_INPUT);
}

/// Compact tile background with hover raise.
pub fn tile(cv: &mut Canvas, r: Rect, a: Action, name: &str) -> bool {
    let hover = cv.hovered(r);
    // surface → surface-raised and scale 1.02 on the snappy spring
    let (hv, _) = cv.anim(&a, hover, false);
    cv.push_tf(0.0, 0.0, 1.0 + 0.02 * hv, r.cx(), r.cy());
    if cv.is_glass() {
        cv.glass_tile(r, R_TILE, GlassLayer::Base);
        cv.sq(r, R_TILE, WHITE.with_alpha(0.05 * hv.clamp(0.0, 1.0)));
    } else {
        let fill = cv
            .theme
            .surface
            .lerp(cv.theme.surface_raised, hv.clamp(0.0, 1.0));
        cv.sq(r, R_TILE, fill);
    }
    cv.pop_tf();
    cv.zone_named(r, a, true, name, R_TILE);
    hover
}

/// Simple bar chart (7-day history etc).
pub fn bars(cv: &mut Canvas, r: Rect, data: &[u32], goal: u32, labels: bool) {
    let max = data.iter().copied().max().unwrap_or(0).max(goal).max(1) as f32;
    let n = data.len().max(1) as f32;
    let bw = (r.w / n) * 0.55;
    let today = crate::util::local_now().weekday as usize;
    for (i, v) in data.iter().enumerate() {
        let x = r.x + r.w * (i as f32 + 0.5) / n;
        let h = (r.h - if labels { 12.0 } else { 0.0 }) * (*v as f32 / max);
        let bar = Rect::new(
            x - bw / 2.0,
            r.y + (r.h - if labels { 12.0 } else { 0.0 }) - h.max(2.0),
            bw,
            h.max(2.0),
        );
        if goal > 0 && *v >= goal || i + 1 == data.len() {
            if let Some(b) = cv.accent_brush(r.x, r.right(), 1.0) {
                cv.rrect_brush(bar, bw / 2.0, crate::render::canvas::as_brush(&b));
            }
        } else {
            cv.rrect(bar, bw / 2.0, TRACK);
        }
        if labels {
            let wd = (today + 7 - (data.len() - 1 - i)) % 7;
            cv.text(
                &crate::util::WEEKDAYS[wd][..1],
                &text::CAPTION.size(9.5),
                Rect::new(x - 8.0, r.bottom() - 11.0, 16.0, 11.0),
                FAINT,
                Align::Center,
            );
        }
    }
}

/// Icon and tint for a page (dock, headers).
pub fn page_icon(p: PageId) -> (Icon, Color) {
    match p {
        PageId::Dashboard => (Icon::Home, TINT_HOME),
        PageId::Music => (Icon::Music, TINT_MUSIC),
        PageId::Lyrics => (Icon::Text, TINT_MUSIC),
        PageId::Timer => (Icon::Timer, TINT_TIMER),
        PageId::Water => (Icon::Droplet, TINT_WATER),
        PageId::Todo => (Icon::List, TINT_TODO),
        PageId::Shelf => (Icon::Shelf, TINT_SHELF),
        PageId::Clipboard => (Icon::Clipboard, TINT_CLIP),
        PageId::Prayer => (Icon::MoonStar, TINT_PRAYER),
        PageId::Weather => (Icon::CloudSun, TINT_WEATHER),
        PageId::Calendar => (Icon::Calendar, TINT_CALENDAR),
        PageId::Notes => (Icon::Note, TINT_CLIP),
        PageId::SysMon => (Icon::Cpu, TINT_SYSTEM),
        PageId::Widgets => (Icon::Widgets, TINT_WIDGETS),
        PageId::Control => (Icon::Sliders, TINT_CONTROL),
        PageId::Diagnostics => (Icon::Info, TINT_SYSTEM),
        PageId::Ask => (Icon::Sparkles, TINT_PRAYER),
        PageId::Notifications => (Icon::Bell, TINT_CALENDAR),
        PageId::Privacy => (Icon::Shield, TINT_TODO),
        PageId::Downloads => (Icon::Download, TINT_WEATHER),
        PageId::Ocr => (Icon::ScanText, TINT_SYSTEM),
        PageId::Annotate => (Icon::Crop, TINT_SYSTEM),
        PageId::Meeting => (Icon::Phone, TINT_TODO),
        PageId::Live => (Icon::Bell, TINT_TIMER),
        PageId::Onboarding => (Icon::Sparkles, TINT_HOME),
    }
}

/// Feature tint for a header icon (falls back to the theme accent).
pub fn icon_tint(icon: Icon, fallback: Color) -> Color {
    match icon {
        Icon::CheckCircle | Icon::List => TINT_TODO,
        Icon::Shelf => TINT_SHELF,
        Icon::Clipboard | Icon::Note => TINT_CLIP,
        Icon::Calendar | Icon::Bell => TINT_CALENDAR,
        Icon::Shield => TINT_TODO,
        Icon::Download | Icon::CloudSun => TINT_WEATHER,
        Icon::ScanText | Icon::Cpu => TINT_SYSTEM,
        Icon::Widgets => TINT_WIDGETS,
        Icon::Sliders => TINT_CONTROL,
        Icon::MoonStar | Icon::Sparkles => TINT_PRAYER,
        Icon::Timer => TINT_TIMER,
        Icon::Music => TINT_MUSIC,
        _ => fallback,
    }
}

/// Motion for one list row: (alpha, dx, dy, height fraction).
/// Entering rows drop in a few DIPs and fade up; leaving rows slide right (cascading by
/// index) and collapse so the rows below glide up.
pub fn row_presence(p: f32, leaving: bool, i: usize) -> (f32, f32, f32, f32) {
    let pc = p.clamp(0.0, 1.0);
    if leaving {
        (pc, (1.0 - pc) * (70.0 + 14.0 * i.min(6) as f32), 0.0, pc)
    } else {
        (pc, 0.0, -(1.0 - pc) * 10.0, pc)
    }
}

/// A passive card row with the shared hover motion (surface -> raised, slight lift on the
/// snappy spring). Returns the hover amount 0..1.
pub fn hover_card(cv: &mut Canvas, r: Rect, key: Action, radius: f32) -> f32 {
    let hover = cv.hovered(r);
    let (hv, _) = cv.anim(&key, hover, false);
    let hv = hv.clamp(0.0, 1.2);
    cv.push_tf(0.0, -hv, 1.0 + 0.008 * hv, r.cx(), r.cy());
    if cv.is_glass() {
        cv.glass_tile(r, radius, GlassLayer::Base);
        cv.sq(r, radius, WHITE.with_alpha(0.05 * hv.min(1.0)));
    } else {
        let fill = cv.theme.surface.lerp(cv.theme.surface_raised, hv.min(1.0));
        cv.sq(r, radius, fill);
    }
    cv.pop_tf();
    cv.zone_named(r, key, false, "", radius);
    hv
}

/// QA (debug builds): every icon at 20 DIP in a 20 DIP guide box, then at 14 DIP, for the
/// optical-size / weight audit.
#[cfg(debug_assertions)]
fn icon_sheet(cv: &mut Canvas) {
    use crate::render::icons::ALL;
    cv.rect(
        Rect::new(0.0, 0.0, CANVAS_W, CANVAS_H),
        Color::rgb(0x111114),
    );
    let cols = 16;
    for (i, ic) in ALL.iter().enumerate() {
        let (c, r) = ((i % cols) as f32, (i / cols) as f32);
        let (x, y) = (18.0 + c * 37.0, 18.0 + r * 50.0);
        cv.stroke_rrect(
            Rect::centered(x, y, 20.0, 20.0),
            1.0,
            WHITE.with_alpha(0.12),
            0.5,
        );
        cv.icon(*ic, x, y, 20.0, INK);
        cv.icon(*ic, x, y + 22.0, 14.0, MIST);
    }
}

#[cfg(test)]
mod tests {
    use super::row_presence;

    #[test]
    fn row_motion() {
        // settled rows are untouched
        assert_eq!(row_presence(1.0, false, 0), (1.0, 0.0, 0.0, 1.0));
        // entering rows start above and transparent
        let (a, dx, dy, h) = row_presence(0.0, false, 3);
        assert_eq!((a, dx, h), (0.0, 0.0, 0.0));
        assert!(dy < 0.0);
        // leaving rows slide right, further for later rows, and collapse
        let (_, d0, _, h0) = row_presence(0.5, true, 0);
        let (_, d3, _, _) = row_presence(0.5, true, 3);
        assert!(d0 > 0.0 && d3 > d0 && h0 < 1.0);
    }
}
