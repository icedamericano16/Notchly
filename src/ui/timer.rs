//! Focus timer: a glowing gradient ring with minute ticks, a mode switch (Focus / Short break /
//! Long break) with a sliding pill, a big play button, session progress and today's focus.
//! Breaks breathe: the ring slowly swells and settles with "Breathe in / Breathe out".

use super::{bars, row, Reveal};
use crate::app::state::State;
use crate::app::{Action, TimerCmd};
use crate::features::timer::Phase;
use crate::render::glass::GlassLayer;
use crate::render::icons::Icon;
use crate::render::text::{self, Align};
use crate::render::{shapes, Canvas, Rect};
use crate::theme::tokens::*;
use crate::util::fmt_mmss;

/// Two-stop gradient for each phase.
pub fn phase_colors(p: Phase) -> (Color, Color) {
    match p {
        Phase::Focus => (Color::rgb(0xFF9F0A), Color::rgb(0xFF375F)),
        Phase::ShortBreak => (Color::rgb(0x30D158), Color::rgb(0x64D2FF)),
        Phase::LongBreak => (Color::rgb(0x5E5CE6), Color::rgb(0xBF5AF2)),
        Phase::Custom => (Color::rgb(0x0A84FF), Color::rgb(0x64D2FF)),
    }
}

pub fn draw(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    let t = st.f.state.timer.clone();
    let p = st.settings.productivity.clone();
    let now = st.started.elapsed().as_secs_f32();
    let active = t.active();
    let running = t.running();
    // the start button's icon morphs between play and pause
    let run_t = st.f.smooth_progress("timer:run", running as u8 as f32);
    // when idle, show the mode that Play will start
    let phase = if active { t.phase } else { Phase::Focus };
    let (c0, c1) = phase_colors(phase);
    let left = t.remaining_ms();
    let progress =
        st.f.smooth_progress("timer:ring", if active { t.progress() } else { 0.0 });
    let breathing = running && matches!(t.phase, Phase::ShortBreak | Phase::LongBreak);

    // ---------------------------------------------------------------- ring
    let ring_col = Rect::new(area.x, area.y, 150.0, area.h);
    let (cx, cy) = (ring_col.cx(), ring_col.cy() + 2.0);
    let radius = (ring_col.w.min(ring_col.h) / 2.0 - 16.0).min(56.0);
    let breath = if breathing {
        // 8 s cycle: 4 s in, 4 s out
        ((now * std::f32::consts::TAU / 8.0).sin() * 0.5 + 0.5) * 0.06
    } else {
        0.0
    };
    row(cv, rv, 0, ring_col, |cv| {
        cv.push_tf(0.0, 0.0, 1.0 + breath, cx, cy);
        // soft halo in the phase colour
        if let Some(g) = cv.radial(
            cx,
            cy,
            radius + 22.0,
            radius + 22.0,
            c1.with_alpha(if active { 0.16 } else { 0.07 }),
            c1.with_alpha(0.0),
        ) {
            unsafe {
                cv.ctx.FillEllipse(
                    &windows::Win32::Graphics::Direct2D::D2D1_ELLIPSE {
                        point: crate::render::canvas::v2(cx, cy),
                        radiusX: radius + 22.0,
                        radiusY: radius + 22.0,
                    },
                    &g,
                )
            };
        }
        // minute ticks: lit as the time passes
        let ticks = 60;
        for i in 0..ticks {
            let f = i as f32 / ticks as f32;
            let a = f * std::f32::consts::TAU;
            let (s, c) = a.sin_cos();
            let major = i % 5 == 0;
            let (r0, r1) = (radius + 6.0, radius + if major { 11.0 } else { 9.0 });
            let lit = f < progress;
            let col = if lit {
                c0.lerp(c1, f).with_alpha(0.9)
            } else {
                FAINT.with_alpha(if major { 0.9 } else { 0.45 })
            };
            cv.line(
                cx + r0 * s,
                cy - r0 * c,
                cx + r1 * s,
                cy - r1 * c,
                if major { 1.6 } else { 1.0 },
                col,
            );
        }
        // track + gradient progress arc
        cv.stroke_circle(cx, cy, radius, TRACK.with_alpha(0.6), 7.0);
        if progress > 0.002 {
            if let Ok(g) = shapes::ring_arc(&cv.gfx.factory.clone(), cx, cy, radius, progress) {
                match cv.gradient(
                    crate::render::canvas::v2(cx - radius, cy - radius),
                    crate::render::canvas::v2(cx + radius, cy + radius),
                    &[(0.0, c0), (1.0, c1)],
                    1.0,
                ) {
                    Some(b) => cv.stroke_geo_brush(&g, crate::render::canvas::as_brush(&b), 7.0),
                    None => cv.stroke_geo(&g, c0, 7.0),
                }
            }
            // glowing head
            let a = progress * std::f32::consts::TAU;
            let (hx, hy) = (cx + radius * a.sin(), cy - radius * a.cos());
            let pulse = if running {
                1.0 + 0.15 * (now * 3.0).sin()
            } else {
                1.0
            };
            if let Some(g) = cv.radial(
                hx,
                hy,
                12.0 * pulse,
                12.0 * pulse,
                c1.with_alpha(0.55),
                c1.with_alpha(0.0),
            ) {
                unsafe {
                    cv.ctx.FillEllipse(
                        &windows::Win32::Graphics::Direct2D::D2D1_ELLIPSE {
                            point: crate::render::canvas::v2(hx, hy),
                            radiusX: 12.0 * pulse,
                            radiusY: 12.0 * pulse,
                        },
                        &g,
                    )
                };
            }
            cv.circle(hx, hy, 4.5, WHITE);
        }
        cv.pop_tf();
        // time + phase / breathing cue
        let txt = if active {
            fmt_mmss((left + 999) / 1000)
        } else {
            format!("{}:00", p.focus_min)
        };
        let paused = active && !running;
        let a = if paused {
            0.55 + 0.45 * (now * 2.5).sin().abs()
        } else {
            1.0
        };
        cv.text(
            &txt,
            &text::DISPLAY.size(30.0).tnum(),
            Rect::new(cx - radius, cy - 20.0, radius * 2.0, 34.0),
            cv.theme.ink.with_alpha(a),
            Align::Center,
        );
        let cue = if breathing {
            let inhale = (now % 8.0) < 4.0;
            crate::i18n::tr(if inhale { "Breathe in" } else { "Breathe out" })
        } else if paused {
            crate::i18n::tr("Paused")
        } else {
            crate::i18n::tr(phase.label())
        };
        cv.text(
            &cue,
            &text::CAPTION.weight(600),
            Rect::new(cx - radius, cy + 14.0, radius * 2.0, 16.0),
            c0.lerp(cv.theme.ink, 0.35),
            Align::Center,
        );
    });

    // ---------------------------------------------------------------- right column
    let rx = ring_col.right() + 16.0;
    let rw = area.right() - rx;

    // mode switch with a sliding pill
    let modes = [
        ("Focus", Phase::Focus, p.focus_min),
        ("Short break", Phase::ShortBreak, p.short_break_min),
        ("Long break", Phase::LongBreak, p.long_break_min),
    ];
    let sel = match phase {
        Phase::ShortBreak => 1.0,
        Phase::LongBreak => 2.0,
        _ => 0.0,
    };
    if (st.f.timer_seg.target - sel).abs() > 0.01 {
        st.f.timer_seg.set(sel);
    }
    let pill = st.f.timer_seg.value;
    let seg = Rect::new(rx, area.y, rw, 30.0);
    row(cv, rv, 1, seg, |cv| {
        cv.glass_tile(seg, 15.0, GlassLayer::Base);
        let sw = seg.w / 3.0;
        let pr = Rect::new(seg.x + 2.0 + sw * pill, seg.y + 2.0, sw - 4.0, seg.h - 4.0);
        if let Some(b) = cv.gradient(
            crate::render::canvas::v2(pr.x, 0.0),
            crate::render::canvas::v2(pr.right(), 0.0),
            &[(0.0, c0.with_alpha(0.9)), (1.0, c1.with_alpha(0.9))],
            1.0,
        ) {
            cv.rrect_brush(pr, 13.0, crate::render::canvas::as_brush(&b));
        }
        for (i, (name, ph, mins)) in modes.iter().enumerate() {
            let r = Rect::new(seg.x + sw * i as f32, seg.y, sw, seg.h);
            let on = *ph == phase;
            cv.text(
                &format!(
                    "{} {mins}m",
                    crate::i18n::tr(if i == 0 {
                        "Focus"
                    } else if i == 1 {
                        "Short"
                    } else {
                        "Long"
                    })
                ),
                &text::CAPTION.weight(if on { 700 } else { 500 }),
                r,
                if on { WHITE } else { cv.theme.mist },
                Align::Center,
            );
            cv.zone_named(r, Action::Timer(TimerCmd::Mode(i as u8)), true, name, 13.0);
        }
    });

    // transport: big play/pause + skip, reset, +5
    let by = seg.bottom() + 36.0;
    row(cv, rv, 2, Rect::new(rx, by - 28.0, rw, 56.0), |cv| {
        let px = rx + 28.0;
        let a = if running {
            Action::Timer(TimerCmd::Pause)
        } else if active {
            Action::Timer(TimerCmd::Resume)
        } else {
            Action::Timer(TimerCmd::StartFocus)
        };
        let (hv, pv) = cv.anim(&a, cv.hovered(Rect::centered(px, by, 52.0, 52.0)), false);
        let s = (1.0 + 0.05 * hv.min(1.0)) * (1.0 - 0.08 * pv);
        cv.push_tf(0.0, 0.0, s, px, by);
        if let Some(b) = cv.gradient(
            crate::render::canvas::v2(px - 26.0, by - 26.0),
            crate::render::canvas::v2(px + 26.0, by + 26.0),
            &[(0.0, c0), (1.0, c1)],
            1.0,
        ) {
            cv.rrect_brush(
                Rect::centered(px, by, 52.0, 52.0),
                26.0,
                crate::render::canvas::as_brush(&b),
            );
        }
        cv.icon_morph(
            Icon::Play,
            Icon::Pause,
            run_t,
            px + 1.5 * (1.0 - run_t),
            by,
            22.0,
            WHITE,
        );
        cv.pop_tf();
        cv.zone_named(
            Rect::centered(px, by, 52.0, 52.0),
            a,
            true,
            if running { "Pause" } else { "Start" },
            26.0,
        );
        let small = [
            (Icon::Skip, Action::Timer(TimerCmd::Skip), "Skip"),
            (Icon::Reset, Action::Timer(TimerCmd::Reset), "Reset"),
            (Icon::Plus, Action::Timer(TimerCmd::Plus5), "Add 5 minutes"),
        ];
        for (i, (ic, act, name)) in small.into_iter().enumerate() {
            let x = px + 52.0 + i as f32 * 42.0;
            cv.circle(x, by, 17.0, cv.theme.surface);
            cv.button(ic, x, by, 34.0, act, name, cv.theme.ink);
        }
    });

    // custom lengths
    let cy2 = by + 34.0;
    row(cv, rv, 3, Rect::new(rx, cy2, rw, 22.0), |cv| {
        let presets = [5u32, 10, 15, 30, 45, 60];
        let w = ((rw - 5.0 * 5.0) / presets.len() as f32).min(40.0);
        for (i, m) in presets.iter().enumerate() {
            let label = format!("{m}m");
            cv.pill_button(
                &label,
                Rect::new(rx + i as f32 * (w + 5.0), cy2, w, 22.0),
                Action::Timer(TimerCmd::Preset(*m)),
                false,
            );
        }
    });

    // sessions + today
    let sy = area.bottom() - 18.0;
    row(cv, rv, 4, Rect::new(rx, sy - 4.0, rw, 22.0), |cv| {
        let every = p.long_every.max(1);
        let done = if t.session > 0 && t.session.is_multiple_of(every) && !active {
            every
        } else {
            t.session % every
        };
        for i in 0..every {
            let x = rx + 5.0 + i as f32 * 13.0;
            if i < done {
                cv.circle(x, sy + 6.0, 4.5, c0);
            } else if i == done && running && t.phase == Phase::Focus {
                // the session in progress fills as it runs
                cv.stroke_circle(x, sy + 6.0, 4.0, c0, 1.2);
                cv.circle(x, sy + 6.0, 4.0 * progress, c0.with_alpha(0.8));
            } else {
                cv.stroke_circle(x, sy + 6.0, 4.0, FAINT, 1.2);
            }
        }
        let label = format!(
            "{}/{every} · {} min {}",
            (done
                + if running && t.phase == Phase::Focus {
                    1
                } else {
                    0
                })
            .clamp(1, every),
            t.today_minutes(),
            crate::i18n::tr("today")
        );
        cv.text(
            &label,
            &text::CAPTION,
            Rect::new(
                rx + every as f32 * 13.0 + 8.0,
                sy - 2.0,
                rw - 80.0 - every as f32 * 13.0,
                16.0,
            ),
            cv.theme.mist,
            Align::Left,
        );
        bars(
            cv,
            Rect::new(area.right() - 64.0, sy - 10.0, 64.0, 24.0),
            &t.week(),
            0,
            false,
        );
    });
}
