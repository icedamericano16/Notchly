//! Control Center: system switches, tall sliders and quick actions.
//!
//! Switches are wide two-line tiles (One UI / iOS 18 style): an icon disc, the name and a live
//! status line (network name with signal bars, headset with battery, how long Keep awake has
//! been on…). Each switch has its own animated glyph:
//! - Wi-Fi: the arcs light up one after another from the dot outward (and a slash draws when off)
//! - Bluetooth: the rune draws itself, then signal ticks pulse beside it
//! - Focus: the moon swings in and stars twinkle around it
//! - Dark mode: the half-filled disc turns over
//! - Keep awake: steam curls up from the cup while it's on
//! - Microphone: level bars bounce when it goes live; a slash strikes through when muted
//!
//! Every switch also sends a ripple from its disc and pops on a bouncy spring.

use std::f32::consts::{PI, TAU};

use super::{header, row, Reveal};
use crate::app::state::State;
use crate::app::{Action, SliderKind};
use crate::render::icons::{self, Icon};
use crate::render::text::{self, Align};
use crate::render::{shapes, Canvas, Rect};
use crate::theme::tokens::*;

/// Toggle ids (index into `Features::ctl_fx` / `ctl_flip`).
pub const WIFI: u8 = 0;
pub const BLUETOOTH: u8 = 1;
pub const FOCUS: u8 = 2;
pub const DARK: u8 = 3;
pub const AWAKE: u8 = 4;
pub const MIC: u8 = 5;

/// One-shot switch animations run this long (seconds).
pub const FLIP_SECS: f32 = 0.9;

struct Toggle {
    id: u8,
    label: &'static str,
    on: bool,
    available: bool,
    status: String,
    tint: Color,
    /// Wi-Fi signal (0–4) to draw beside the status.
    bars: Option<u8>,
}

fn toggles(st: &State) -> Vec<Toggle> {
    let r = st.sys.radios;
    let net = &st.sys.net;
    let wifi_on = r.wifi.unwrap_or(net.wifi);
    let bt_on = r.bluetooth.unwrap_or(false);
    let (wifi_status, bars) = match (r.wifi, net.wifi && net.connected) {
        (Some(false), _) => ("Off".to_string(), None),
        (_, true) if !net.name.is_empty() => (net.name.clone(), Some(net.bars.min(4))),
        (Some(true), _) => ("Not connected".into(), None),
        _ if !net.wifi && net.connected => ("Ethernet".into(), None),
        _ => ("Unavailable".into(), None),
    };
    let bt_status = match r.bluetooth {
        Some(true) => match st.sys.bt_connected.first() {
            Some(name) => match st.sys.bt_battery {
                Some(b) => format!("{name} · {b}%"),
                None => name.clone(),
            },
            None => "On · No devices".into(),
        },
        Some(false) => "Off".into(),
        None => "Unavailable".into(),
    };
    let focus_status = if st.settings.quiet.focus {
        "On · Pop-ups held".to_string()
    } else if st.quiet_hours() {
        "Quiet hours".into()
    } else {
        "Off".into()
    };
    let awake_status = match st.sys.keep_awake_since {
        Some(t) if st.sys.keep_awake => {
            let m = t.elapsed().as_secs() / 60;
            if m < 1 {
                "On · just now".to_string()
            } else if m < 60 {
                format!("On · {m} min")
            } else {
                format!("On · {}h {:02}m", m / 60, m % 60)
            }
        }
        _ => "Off".into(),
    };
    vec![
        Toggle {
            id: WIFI,
            label: "Wi-Fi",
            on: wifi_on,
            available: r.wifi.is_some(),
            status: wifi_status,
            tint: Color::rgb(0x0A84FF),
            bars,
        },
        Toggle {
            id: BLUETOOTH,
            label: "Bluetooth",
            on: bt_on,
            available: r.bluetooth.is_some(),
            status: bt_status,
            tint: Color::rgb(0x0A84FF),
            bars: None,
        },
        Toggle {
            id: FOCUS,
            label: "Focus",
            on: st.settings.quiet.focus,
            available: true,
            status: focus_status,
            tint: Color::rgb(0x5E5CE6),
            bars: None,
        },
        Toggle {
            id: DARK,
            label: "Dark mode",
            on: st.sys.dark_mode,
            available: true,
            status: if st.sys.dark_mode { "On" } else { "Off" }.into(),
            tint: Color::rgb(0x636366),
            bars: None,
        },
        Toggle {
            id: AWAKE,
            label: "Keep awake",
            on: st.sys.keep_awake,
            available: true,
            status: awake_status,
            tint: Color::rgb(0xFF9F0A),
            bars: None,
        },
        Toggle {
            id: MIC,
            label: "Microphone",
            on: !st.sys.mic_muted,
            available: true,
            status: if st.sys.mic_muted { "Muted" } else { "Live" }.into(),
            tint: Color::rgb(0x30D158),
            bars: None,
        },
    ]
}

pub fn draw(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    let body = header(cv, area, Icon::Sliders, "Control Center", "");
    let ts = toggles(st);
    let now = std::time::Instant::now();
    // springs follow the real switch positions; a change (from here or from Windows) starts
    // that switch's one-shot animation
    for t in &ts {
        let i = t.id as usize;
        let target = if t.on { 1.0 } else { 0.0 };
        // the first value, and anything that arrives while the page is still opening, just
        // appears; later changes animate
        let opening = st
            .isl
            .reveal_at
            .is_some_and(|r| r.elapsed().as_secs_f32() < 0.4);
        let first = st.f.ctl_seen & (1 << i) == 0 || opening;
        let s = &mut st.f.ctl_fx[i];
        if first {
            s.snap(target);
        } else if (s.target - target).abs() > 0.5 {
            s.set(target);
            st.f.ctl_flip[i] = Some(now);
        }
        st.f.ctl_seen |= 1 << i;
    }
    let t_now = st.started.elapsed().as_secs_f32();

    // ---------------------------------------------------------------- switches (2 × 3)
    let gap = 8.0;
    let tw = 126.0;
    let th = ((body.h - 2.0 * gap) / 3.0).min(54.0);
    for (i, t) in ts.iter().enumerate() {
        let col = (i % 2) as f32;
        let rown = (i / 2) as f32;
        let r = Rect::new(
            body.x + col * (tw + gap),
            body.y + rown * (th + gap),
            tw,
            th,
        );
        let v = st.f.ctl_fx[t.id as usize].value;
        let since = st.f.ctl_flip[t.id as usize]
            .map(|f| now.duration_since(f).as_secs_f32())
            .unwrap_or(9.0);
        row(cv, rv, 1 + i, r, |cv| tile(cv, r, t, v, since, t_now));
    }

    // ---------------------------------------------------------------- tall sliders
    let sx = body.x + 2.0 * tw + gap + 12.0;
    let sw = 44.0;
    let sh = body.h - 2.0;
    let bright = st.sys.brightness.map(|b| b as f32 / 100.0);
    let vol = if st.sys.muted { 0.0 } else { st.sys.volume };
    let sliders = [
        (
            Rect::new(sx, body.y + 1.0, sw, sh),
            SliderKind::Brightness,
            bright.unwrap_or(0.5),
            Icon::Sun,
            "Brightness",
            bright.is_some(),
        ),
        (
            Rect::new(sx + sw + 8.0, body.y + 1.0, sw, sh),
            SliderKind::Volume,
            vol,
            icons::volume_icon(vol, st.sys.muted),
            "Volume",
            true,
        ),
    ];
    for (i, (r, kind, value, icon, name, enabled)) in sliders.into_iter().enumerate() {
        let smooth = st.f.smooth_progress(&format!("ctl:{i}"), value);
        row(cv, rv, 2 + i, r, |cv| {
            tall_slider(cv, r, smooth, value, icon, kind, name, enabled);
        });
    }

    // ---------------------------------------------------------------- battery + quick actions
    let rx = sx + 2.0 * sw + 20.0;
    let rw = body.right() - rx;
    let b = st.sys.battery.clone();
    let charge =
        st.f.smooth_progress("ctl:battery", b.percent as f32 / 100.0);
    row(cv, rv, 4, Rect::new(rx, body.y, rw, 64.0), |cv| {
        let cx = rx + rw / 2.0;
        let cy = body.y + 27.0;
        if b.present {
            let c = if b.ac {
                GOOD
            } else if b.percent <= 20 {
                CRITICAL
            } else {
                cv.theme.ink
            };
            cv.ring(cx, cy, 24.0, charge, 4.0, Some(c));
            cv.text(
                &format!("{}%", b.percent),
                &text::BODY_MED.tnum(),
                Rect::new(cx - 24.0, cy - 9.0, 48.0, 18.0),
                cv.theme.ink,
                Align::Center,
            );
            if b.ac {
                // the bolt breathes while charging
                let a = 0.65 + 0.35 * (t_now * 2.4).sin();
                cv.icon(Icon::Bolt, cx, cy + 33.0, 11.0, GOOD.with_alpha(a));
            }
        } else {
            cv.icon(Icon::Bolt, cx, cy, 20.0, GOOD);
            cv.label(
                "Plugged in",
                &text::CAPTION.size(10.0),
                Rect::new(rx, cy + 16.0, rw, 12.0),
                cv.theme.mist,
                Align::Center,
            );
        }
    });
    let actions = [
        (Icon::Camera, Action::Screenshot, "Screenshot"),
        (Icon::ScanText, Action::OcrStart, "Capture text"),
        (Icon::Lock, Action::LockPc, "Lock"),
        (
            Icon::Sun,
            Action::OpenUrl("ms-settings:nightlight".into()),
            "Night light",
        ),
        (Icon::Settings, Action::OpenSettings, "Settings"),
        (
            Icon::Speaker,
            Action::OpenUrl("ms-settings:sound".into()),
            "Sound settings",
        ),
    ];
    let gy = body.y + 78.0;
    let gc = (rw / 2.0).min(46.0);
    for (i, (ic, a, name)) in actions.into_iter().enumerate() {
        let x = rx + rw / 2.0 + (i % 2) as f32 * gc - gc / 2.0;
        let y = gy + (i / 2) as f32 * 38.0;
        row(
            cv,
            rv,
            5 + i / 2,
            Rect::new(x - 16.0, y - 16.0, 32.0, 32.0),
            |cv| {
                cv.circle(x, y, 16.0, cv.theme.surface_raised);
                cv.button(ic, x, y, 32.0, a, name, cv.theme.ink);
            },
        );
    }
}

/// A wide switch tile: icon disc, name, live status.
fn tile(cv: &mut Canvas, r: Rect, t: &Toggle, v: f32, since: f32, now: f32) {
    let a = Action::Ctl(t.id);
    let (hv, pv) = cv.anim(&a, cv.hovered(r), false);
    let vc = v.clamp(0.0, 1.0);
    let pop = (v - vc).abs();
    let scale = (1.0 + 0.025 * hv.min(1.0)) * (1.0 - 0.05 * pv) * (1.0 + 0.35 * pop);
    cv.push_tf(0.0, 0.0, scale, r.cx(), r.cy());

    // surface: raised grey when off, a deep tint wash when on
    let rad = r.h / 2.0;
    cv.sq(r, rad, cv.theme.surface_raised);
    if vc > 0.01 {
        cv.push_alpha(vc);
        if let Some(g) = cv.gradient(
            crate::render::canvas::v2(r.x, r.y),
            crate::render::canvas::v2(r.right(), r.bottom()),
            &[
                (0.0, t.tint.with_alpha(0.95)),
                (1.0, t.tint.lerp(NOTCH, 0.35)),
            ],
            1.0,
        ) {
            use windows::core::Interface;
            cv.rrect_brush(r, rad, &g.cast().expect("brush"));
        }
        // top sheen
        cv.sq(
            Rect::new(r.x, r.y, r.w, r.h * 0.5),
            rad,
            WHITE.with_alpha(0.07),
        );
        cv.pop_alpha();
    }
    cv.stroke_rrect(r.inset(0.5), rad - 0.5, RING.with_alpha(1.0 - vc), 1.0);

    // icon disc: white when on (glyph in the tint), dark when off
    let dr = (rad - 10.0).max(12.0);
    let dc = (r.x + 8.0 + dr, r.cy());
    // ripple from the disc for the first 0.6 s after a switch
    if since < 0.6 {
        let k = since / 0.6;
        let ease = 1.0 - (1.0 - k).powi(3);
        cv.stroke_circle(
            dc.0,
            dc.1,
            dr + 18.0 * ease,
            (if t.on { WHITE } else { t.tint }).with_alpha(0.5 * (1.0 - k)),
            2.0 * (1.0 - k) + 0.5,
        );
    }
    let disc = cv.theme.surface.lerp(WHITE, vc);
    cv.circle(dc.0, dc.1, dr, disc);
    let glyph = if t.available {
        cv.theme.ink.lerp(t.tint, vc)
    } else {
        FAINT
    };
    glyph_for(cv, t.id, dc.0, dc.1, dr, glyph, vc, since, now, t.on);

    // name + status
    let tx = dc.0 + dr + 7.0;
    let text_c = cv.theme.ink.lerp(WHITE, vc);
    cv.label(
        t.label,
        &text::CAPTION.weight(700).size(11.0),
        Rect::new(tx, r.y + 8.0, r.right() - tx - 6.0, 15.0),
        text_c,
        Align::Left,
    );
    let mut sx = tx;
    if let Some(bars) = t.bars {
        // signal strength: four rising bars, lit by strength
        for k in 0..4u8 {
            let h = 3.0 + k as f32 * 2.2;
            let c = if k < bars {
                text_c.with_alpha(0.95)
            } else {
                text_c.with_alpha(0.3)
            };
            cv.rrect(
                Rect::new(sx + k as f32 * 3.2, r.y + 33.0 - h, 2.0, h),
                1.0,
                c,
            );
        }
        sx += 15.0;
    }
    cv.label(
        &t.status,
        &text::CAPTION.size(10.0),
        Rect::new(sx, r.y + 23.0, r.right() - sx - 6.0, 14.0),
        if vc > 0.5 {
            WHITE.with_alpha(0.82)
        } else {
            cv.theme.mist
        },
        Align::Left,
    );
    cv.pop_tf();
    cv.zone_named(r, a, true, t.label, rad);
}

#[allow(clippy::too_many_arguments)]
fn arc(cv: &mut Canvas, cx: f32, cy: f32, r: f32, a0: f32, a1: f32, c: Color, w: f32) {
    if r <= 0.1 || (a1 - a0).abs() < 0.01 {
        return;
    }
    let n = 14;
    let pts: Vec<(f32, f32)> = (0..=n)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            (cx + r * a.cos(), cy + r * a.sin())
        })
        .collect();
    let factory = cv.gfx.factory.clone();
    if let Ok(g) = shapes::poly(&factory, &pts, false) {
        cv.stroke_geo(&g, c, w);
    }
}

fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// The animated glyph for each switch, drawn inside a disc of radius `r`.
#[allow(clippy::too_many_arguments)]
fn glyph_for(
    cv: &mut Canvas,
    id: u8,
    cx: f32,
    cy: f32,
    r: f32,
    c: Color,
    v: f32,
    since: f32,
    now: f32,
    on: bool,
) {
    let s = r / 12.0; // glyph scale
    let settled = since > 2.0;
    match id {
        WIFI => {
            // dot + three arcs; switching on lights them inner → outer
            let base = (cx, cy + 5.0 * s);
            cv.circle(base.0, base.1, 1.6 * s, c);
            for i in 0..3 {
                let p = if on && !settled {
                    ease_out((since - i as f32 * 0.14) / 0.35)
                } else {
                    1.0
                };
                let rad = (4.0 + i as f32 * 3.4) * s;
                let span = 0.78 * p;
                let col = if on {
                    c.with_alpha(p.max(0.15))
                } else {
                    c.with_alpha(0.45)
                };
                if span > 0.02 {
                    arc(
                        cv,
                        base.0,
                        base.1,
                        rad,
                        -PI / 2.0 - span,
                        -PI / 2.0 + span,
                        col,
                        1.7 * s,
                    );
                }
            }
            if !on {
                // a slash draws in when switched off
                let p = if settled {
                    1.0
                } else {
                    ease_out((since - 0.1) / 0.35)
                };
                let (x0, y0, x1, y1) = (cx - 7.0 * s, cy - 6.0 * s, cx + 7.0 * s, cy + 7.0 * s);
                cv.line(x0, y0, x0 + (x1 - x0) * p, y0 + (y1 - y0) * p, 1.7 * s, c);
            }
        }
        BLUETOOTH => {
            // the rune draws itself stroke by stroke
            let pts = [
                (-3.5, -3.5),
                (3.8, 3.2),
                (0.0, 7.0),
                (0.0, -7.0),
                (3.8, -3.2),
                (-3.5, 3.5),
            ];
            let total = pts.len() - 1;
            let p = if on && !settled {
                ease_out(since / 0.55)
            } else {
                1.0
            };
            let drawn = p * total as f32;
            for i in 0..total {
                let seg = (drawn - i as f32).clamp(0.0, 1.0);
                if seg <= 0.0 {
                    break;
                }
                let (a, b) = (pts[i], pts[i + 1]);
                cv.line(
                    cx + a.0 * s,
                    cy + a.1 * s,
                    cx + (a.0 + (b.0 - a.0) * seg) * s,
                    cy + (a.1 + (b.1 - a.1) * seg) * s,
                    1.6 * s,
                    c,
                );
            }
            if on && since < 1.6 {
                // signal ticks pulse out on both sides
                let q = ((since - 0.4) / 0.8).clamp(0.0, 1.0);
                if q > 0.0 {
                    let a = (1.0 - q) * 0.9;
                    for side in [-1.0f32, 1.0] {
                        let (a0, a1) = if side < 0.0 {
                            (PI - 0.5, PI + 0.5)
                        } else {
                            (-0.5, 0.5)
                        };
                        arc(
                            cv,
                            cx,
                            cy,
                            (8.0 + 3.0 * q) * s,
                            a0,
                            a1,
                            c.with_alpha(a),
                            1.3 * s,
                        );
                    }
                }
            }
        }
        FOCUS => {
            // a crescent that swings in; stars twinkle around it while on
            let swing = if on && !settled {
                (1.0 - ease_out(since / 0.7)) * -0.9
            } else {
                0.0
            };
            let rr = 6.8 * s;
            let n = 20;
            let mut pts = Vec::with_capacity(2 * n + 2);
            // outer arc (most of a circle), then inner arc back = crescent outline
            for i in 0..=n {
                let a = 0.55 + swing + (TAU - 1.1) * i as f32 / n as f32;
                pts.push((cx + rr * a.cos(), cy + rr * a.sin()));
            }
            let (ox, oy) = (
                cx + 3.6 * s * (0.0 + swing).cos(),
                cy + 3.6 * s * (swing).sin() - 2.4 * s,
            );
            let ir = 5.4 * s;
            for i in (0..=n).rev() {
                let a = 1.9 + swing + 2.6 * i as f32 / n as f32;
                pts.push((ox + ir * a.cos(), oy + ir * a.sin()));
            }
            let factory = cv.gfx.factory.clone();
            if let Ok(g) = shapes::poly(&factory, &pts, true) {
                cv.fill_geo(&g, c.with_alpha(0.25 + 0.75 * v.min(1.0)));
                cv.stroke_geo(&g, c, 1.2 * s);
            }
            if v > 0.05 {
                for (i, (dx, dy, sz)) in [(6.5, -7.0, 1.5), (9.0, -2.0, 1.0)].iter().enumerate() {
                    let tw = 0.55 + 0.45 * (now * 3.0 + i as f32 * 2.1).sin();
                    let a = v.min(1.0) * tw;
                    let (x, y) = (cx + dx * s, cy + dy * s);
                    let l = sz * s * 1.6;
                    cv.line(x - l, y, x + l, y, 1.0 * s, c.with_alpha(a));
                    cv.line(x, y - l, x, y + l, 1.0 * s, c.with_alpha(a));
                }
            }
        }
        DARK => {
            // a disc whose filled half turns over when switched
            let turn = if settled { 1.0 } else { ease_out(since / 0.7) };
            let angle = if on { PI * turn } else { PI * (1.0 - turn) };
            let rr = 7.0 * s;
            cv.stroke_circle(cx, cy, rr, c, 1.6 * s);
            let n = 16;
            let half: Vec<(f32, f32)> = (0..=n)
                .map(|i| {
                    let a = angle - PI / 2.0 + PI * i as f32 / n as f32;
                    (cx + rr * a.cos(), cy + rr * a.sin())
                })
                .collect();
            let factory = cv.gfx.factory.clone();
            if let Ok(g) = shapes::poly(&factory, &half, true) {
                cv.fill_geo(&g, c);
            }
        }
        AWAKE => {
            // cup + handle; steam curls rise while on
            let cup = Rect::new(cx - 5.5 * s, cy - 1.5 * s, 9.0 * s, 7.5 * s);
            cv.stroke_rrect(cup, 2.5 * s, c, 1.5 * s);
            arc(
                cv,
                cup.right(),
                cup.cy(),
                2.4 * s,
                -PI / 2.0,
                PI / 2.0,
                c,
                1.4 * s,
            );
            if v > 0.05 {
                for i in 0..2 {
                    let x0 = cx - 2.5 * s + i as f32 * 4.0 * s;
                    let phase = now * 1.4 + i as f32 * 0.5;
                    let f = phase.fract();
                    let pts: Vec<(f32, f32)> = (0..8)
                        .map(|j| {
                            let y = cy - 3.0 * s - j as f32 * 0.8 * s - f * 2.5 * s;
                            (x0 + (j as f32 * 0.9 + phase * TAU).sin() * 0.9 * s, y)
                        })
                        .collect();
                    let factory = cv.gfx.factory.clone();
                    if let Ok(g) = shapes::poly(&factory, &pts, false) {
                        cv.stroke_geo(&g, c.with_alpha(v.min(1.0) * (1.0 - f * 0.7)), 1.2 * s);
                    }
                }
            } else {
                for dx in [-2.5f32, 1.5] {
                    cv.line(
                        cx + dx * s,
                        cy - 6.0 * s,
                        cx + dx * s,
                        cy - 4.0 * s,
                        1.2 * s,
                        c.with_alpha(0.5),
                    );
                }
            }
        }
        _ => {
            // microphone capsule + stand
            let cap = Rect::new(cx - 2.6 * s, cy - 7.0 * s, 5.2 * s, 9.0 * s);
            cv.stroke_rrect(cap, 2.6 * s, c, 1.5 * s);
            arc(cv, cx, cy - 0.5 * s, 5.0 * s, 0.15, PI - 0.15, c, 1.4 * s);
            cv.line(cx, cy + 4.5 * s, cx, cy + 7.0 * s, 1.4 * s, c);
            if on && since < 1.2 {
                // live: level bars bounce up beside it
                let q = 1.0 - since / 1.2;
                for (i, dx) in [-8.0f32, 8.0].iter().enumerate() {
                    let h = (2.0 + 3.0 * (now * 14.0 + i as f32).sin().abs()) * s * q;
                    cv.line(
                        cx + dx * s,
                        cy - h,
                        cx + dx * s,
                        cy + h,
                        1.4 * s,
                        c.with_alpha(q),
                    );
                }
            }
            if !on {
                // muted: a slash strikes through, with a tiny shake as it lands
                let p = if settled { 1.0 } else { ease_out(since / 0.3) };
                let shake = if since < 0.5 {
                    (since * 60.0).sin() * (0.5 - since) * 3.0 * s
                } else {
                    0.0
                };
                let (x0, y0) = (cx - 7.0 * s + shake, cy - 7.0 * s);
                let (x1, y1) = (cx + 7.0 * s + shake, cy + 7.0 * s);
                cv.line(x0, y0, x0 + (x1 - x0) * p, y0 + (y1 - y0) * p, 1.8 * s, c);
            }
        }
    }
}

/// iOS-style tall capsule slider: the fill rises from the bottom, the icon sits at the foot
/// and switches to dark when the fill covers it; the value appears at the top while adjusting.
#[allow(clippy::too_many_arguments)]
fn tall_slider(
    cv: &mut Canvas,
    r: Rect,
    smooth: f32,
    value: f32,
    icon: Icon,
    kind: SliderKind,
    name: &str,
    enabled: bool,
) {
    let a = Action::Slider(kind);
    let active = cv.hovered(r) || cv.pressed.as_ref() == Some(&a);
    let (hv, _) = cv.anim(&a, active, false);
    let grow = 1.0 + 0.035 * hv.min(1.0);
    cv.push_tf(0.0, 0.0, grow, r.cx(), r.cy());
    let rad = 20.0;
    cv.sq(r, rad, cv.theme.surface_raised);
    if enabled {
        let h = (r.h * smooth.clamp(0.0, 1.0)).max(0.0);
        if h > 0.5 {
            cv.push_clip(Rect::new(r.x, r.bottom() - h, r.w, h));
            cv.sq(r, rad, WHITE.with_alpha(0.92));
            cv.pop_clip();
        }
        let iy = r.bottom() - 20.0;
        let covered = r.bottom() - h < iy - 8.0;
        cv.icon(
            icon,
            r.cx(),
            iy,
            19.0,
            if covered { NOTCH } else { cv.theme.ink },
        );
        if hv > 0.02 {
            cv.push_alpha(hv.min(1.0));
            cv.text(
                &format!("{:.0}", value * 100.0),
                &text::CAPTION.weight(700).tnum(),
                Rect::new(r.x, r.y + 8.0, r.w, 14.0),
                if r.bottom() - h < r.y + 22.0 {
                    NOTCH
                } else {
                    cv.theme.mist
                },
                Align::Center,
            );
            cv.pop_alpha();
        }
    } else {
        cv.icon(icon, r.cx(), r.bottom() - 20.0, 19.0, FAINT);
    }
    cv.pop_tf();
    if enabled {
        cv.zone_named(r, a, true, name, rad);
    }
}
