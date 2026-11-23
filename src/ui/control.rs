//! Control Center: system switches, tall sliders, quick actions and what's in use.
//!
//! Switches are wide two-line tiles (One UI / iOS 18 style): an icon disc, the name and a live
//! status line (network name with signal bars, who has the camera or microphone…). Each switch
//! has its own animated glyph:
//! - Wi-Fi: the arcs light up one after another from the dot outward (and a slash draws when off)
//! - Bluetooth: the rune draws itself, then signal ticks pulse beside it
//! - Airplane mode: the plane glides in on a fading contrail
//! - Focus: the moon swings in and stars twinkle around it
//! - Microphone: level bars bounce when it goes live; a slash strikes through when muted
//! - Camera: the lens iris opens when allowed and a shutter closes over it when blocked
//! - Dark mode: the half-filled disc turns over
//! - In use: a shield with a breathing dot for each of camera, microphone and location; it
//!   opens a sheet listing the apps, each with Close (and Force quit if it lingers)
//!
//! Every switch also sends a ripple from its disc and pops on a bouncy spring. The sliders
//! follow the mouse wheel in clean steps and a trackpad 1:1, with an elastic edge.

use std::f32::consts::{PI, TAU};

use super::{header, row, Reveal};
use crate::app::state::State;
use crate::app::{Action, SliderKind};
use crate::render::glass::GlassLayer;
use crate::render::icons::{self, Icon};
use crate::render::text::{self, Align};
use crate::render::{shapes, Canvas, Rect};
use crate::services::privacy::Cap;
use crate::theme::tokens::*;

/// Toggle ids (index into `Features::ctl_fx` / `ctl_flip`).
pub const WIFI: u8 = 0;
pub const BLUETOOTH: u8 = 1;
pub const AIRPLANE: u8 = 2;
pub const FOCUS: u8 = 3;
pub const MIC: u8 = 4;
pub const CAMERA: u8 = 5;
pub const DARK: u8 = 6;
/// The "in use" tile (not a switch: it opens the sheet).
pub const PRIVACY: u8 = 7;
/// Keep awake lives in the quick actions.
pub const KEEP_AWAKE: u8 = 8;

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
    // who has the camera / microphone / location right now
    let users = |cap: Cap| -> Vec<String> {
        let mut v: Vec<String> =
            st.f.privacy
                .uses
                .iter()
                .filter(|u| u.cap == cap)
                .map(|u| u.app.clone())
                .collect();
        v.dedup();
        v
    };
    let who = |v: &[String]| match v.len() {
        0 => String::new(),
        1 => v[0].clone(),
        n => format!("{} +{}", v[0], n - 1),
    };
    let cam_users = users(Cap::Camera);
    let mic_users = users(Cap::Mic);
    let cam_allowed = st.f.privacy.camera_allowed.unwrap_or(true);
    let cam_status = match (cam_allowed, cam_users.is_empty()) {
        (false, true) => "Blocked".to_string(),
        (false, false) => format!("{} · {}", crate::i18n::tr("Blocked"), who(&cam_users)),
        (true, false) => format!("{} · {}", crate::i18n::tr("In use"), who(&cam_users)),
        (true, true) => "Allowed".into(),
    };
    let mic_status = match (st.sys.mic_muted, mic_users.is_empty()) {
        (true, _) => "Muted".to_string(),
        (false, false) => format!("{} · {}", crate::i18n::tr("Live"), who(&mic_users)),
        (false, true) => "Live".into(),
    };
    let airplane = r.airplane.unwrap_or(false);
    // the in-use tile: the most private thing in use sets its colour
    let in_use: Vec<Cap> = [Cap::Camera, Cap::Mic, Cap::Location]
        .into_iter()
        .filter(|c| st.f.privacy.uses.iter().any(|u| u.cap == *c))
        .collect();
    let use_status = match st.f.privacy.uses.first() {
        None => "Nothing in use".to_string(),
        Some(_) => {
            let first =
                st.f.privacy
                    .uses
                    .iter()
                    .find(|u| Some(u.cap) == in_use.first().copied())
                    .map(|u| format!("{} · {}", u.app, crate::i18n::tr(u.cap.label())))
                    .unwrap_or_default();
            let n = st.f.privacy.uses.len();
            if n > 1 {
                format!("{first} +{}", n - 1)
            } else {
                first
            }
        }
    };
    let use_tint = match in_use.first() {
        Some(Cap::Camera) => PRIVACY_CAM,
        Some(Cap::Mic) => PRIVACY_MIC,
        Some(Cap::Location) => PRIVACY_LOC,
        None => CTL_DARK,
    };
    vec![
        Toggle {
            id: WIFI,
            label: "Wi-Fi",
            on: wifi_on && !airplane,
            available: r.wifi.is_some(),
            status: wifi_status,
            tint: CTL_WIFI,
            bars,
        },
        Toggle {
            id: BLUETOOTH,
            label: "Bluetooth",
            on: bt_on && !airplane,
            available: r.bluetooth.is_some(),
            status: bt_status,
            tint: CTL_WIFI,
            bars: None,
        },
        Toggle {
            id: AIRPLANE,
            label: "Airplane",
            on: airplane,
            available: r.airplane.is_some(),
            status: if airplane { "On · Radios off" } else { "Off" }.into(),
            tint: CTL_AIRPLANE,
            bars: None,
        },
        Toggle {
            id: FOCUS,
            label: "Focus",
            on: st.settings.quiet.focus,
            available: true,
            status: focus_status,
            tint: CTL_FOCUS,
            bars: None,
        },
        Toggle {
            id: MIC,
            label: "Microphone",
            on: !st.sys.mic_muted,
            available: true,
            status: mic_status,
            tint: CTL_MIC,
            bars: None,
        },
        Toggle {
            id: CAMERA,
            label: "Camera",
            on: cam_allowed,
            available: st.f.privacy.camera_allowed.is_some(),
            status: cam_status,
            tint: CTL_CAMERA,
            bars: None,
        },
        Toggle {
            id: DARK,
            label: "Dark mode",
            on: st.sys.dark_mode,
            available: true,
            status: if st.sys.dark_mode { "On" } else { "Off" }.into(),
            tint: CTL_DARK,
            bars: None,
        },
        Toggle {
            id: PRIVACY,
            label: "In use",
            on: !in_use.is_empty(),
            available: true,
            status: use_status,
            tint: use_tint,
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
    let lit: Vec<Color> = [
        (Cap::Camera, PRIVACY_CAM),
        (Cap::Mic, PRIVACY_MIC),
        (Cap::Location, PRIVACY_LOC),
    ]
    .into_iter()
    .filter(|(c, _)| st.f.privacy.uses.iter().any(|u| u.cap == *c))
    .map(|(_, col)| col)
    .collect();
    PRIVACY_LIT.with(|c| *c.borrow_mut() = lit);

    // ---------------------------------------------------------------- one 4-row grid
    // left: the switches, 2 x 4. right, rows 1-2: brightness and volume capsules and the
    // battery card; rows 3-4: six quick actions, 3 x 2. Everything shares the row lines.
    let gap = CTL_GAP;
    let th = ((body.h - 3.0 * gap) / 4.0).min(CTL_ROW_MAX);
    let tw = CTL_TILE_W;
    let row_y = |i: usize| body.y + i as f32 * (th + gap);
    for (i, t) in ts.iter().enumerate() {
        let col = (i % 2) as f32;
        let r = Rect::new(body.x + col * (tw + gap), row_y(i / 2), tw, th);
        let v = st.f.ctl_fx[t.id as usize].value;
        let since = st.f.ctl_flip[t.id as usize]
            .map(|f| now.duration_since(f).as_secs_f32())
            .unwrap_or(9.0);
        row(cv, rv, 1 + i, r, |cv| tile(cv, r, t, v, since, t_now));
    }
    let rx = body.x + 2.0 * tw + gap + CTL_BLOCK_GAP;
    let rw = body.right() - rx;
    let half_h = 2.0 * th + gap;

    // ---------------------------------------------------------------- sliders (rows 1-2)
    let sw = CTL_SLIDER_W;
    let bright = st.sys.brightness.map(|b| b as f32 / 100.0);
    let vol = if st.sys.muted { 0.0 } else { st.sys.volume };
    let sliders = [
        (
            Rect::new(rx, row_y(0), sw, half_h),
            SliderKind::Brightness,
            bright.unwrap_or(0.5),
            Icon::Sun,
            "Brightness",
            bright.is_some(),
        ),
        (
            Rect::new(rx + sw + gap, row_y(0), sw, half_h),
            SliderKind::Volume,
            vol,
            icons::volume_icon(vol, st.sys.muted),
            "Volume",
            true,
        ),
    ];
    st.input.ctl_sliders = sliders.iter().map(|s| (s.0, s.1)).collect();
    let wheel = st.input.ctl_wheel;
    for (i, (r, kind, value, icon, name, enabled)) in sliders.into_iter().enumerate() {
        // a trackpad drives the fill directly; wheel notches glide on the snappy spring;
        // anything else (Windows, a drag) eases in gently
        let recent = wheel.filter(|w| {
            w.0 == kind && w.2.elapsed() < std::time::Duration::from_millis(CTL_WHEEL_HOLD_MS)
        });
        let (value, cfg, snap) = match recent {
            Some((_, v, _, true)) => (v, crate::motion::SNAPPY, true),
            Some((_, v, _, false)) => (v, crate::motion::SNAPPY, false),
            None => (value, crate::motion::GENTLE, false),
        };
        let smooth = st.f.progress_with(&format!("ctl:{i}"), value, cfg, snap);
        let band = st.f.ctl_band[i].value;
        let active = recent.is_some();
        row(cv, rv, 2 + i, r, |cv| {
            tall_slider(
                cv, r, smooth, value, icon, kind, name, enabled, band, active,
            );
        });
    }

    // ---------------------------------------------------------------- battery card (rows 1-2)
    let bx = rx + 2.0 * sw + 2.0 * gap;
    let card = Rect::new(bx, row_y(0), body.right() - bx, half_h);
    let b = st.sys.battery.clone();
    let charge =
        st.f.smooth_progress("ctl:battery", b.percent as f32 / 100.0);
    row(cv, rv, 4, card, |cv| {
        battery_card(cv, card, &b, charge, t_now)
    });

    // ---------------------------------------------------------------- quick actions (rows 3-4)
    let actions = [
        (Icon::Camera, Action::Screenshot, "Screenshot"),
        (Icon::ScanText, Action::OcrStart, "Copy text"),
        (Icon::Lock, Action::LockPc, "Lock"),
        (
            Icon::Sun,
            Action::OpenUrl("ms-settings:nightlight".into()),
            "Night light",
        ),
        (Icon::Coffee, Action::Ctl(KEEP_AWAKE), "Keep awake"),
        (Icon::Settings, Action::OpenSettings, "Settings"),
    ];
    let awake = st.sys.keep_awake;
    let awake_t = st.f.smooth_progress("ctl:awake", awake as u8 as f32);
    let qw = (rw - 2.0 * gap) / 3.0;
    for (i, (ic, a, name)) in actions.into_iter().enumerate() {
        let r = Rect::new(rx + (i % 3) as f32 * (qw + gap), row_y(2 + i / 3), qw, th);
        let lit = if ic == Icon::Coffee { awake_t } else { 0.0 };
        let label = if ic == Icon::Coffee && awake {
            "Keep awake: on"
        } else {
            name
        };
        row(cv, rv, 5 + i / 3, r, |cv| {
            quick(cv, r, ic, a, name, label, lit, t_now)
        });
    }
    // ---------------------------------------------------------------- in use now (sheet)
    let open = st.input.ctl_sheet;
    let p =
        st.f.progress_with("ctl:sheet", open as u8 as f32, crate::motion::SNAPPY, false);
    if p > 0.005 {
        sheet(st, cv, body, p);
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
    cv.glass_tile(r, rad, GlassLayer::Raised);
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
        // signal strength: four rising bars, lit by strength; they fill in one after another
        // when Wi-Fi comes on
        let bars = if since < 1.0 {
            ((since / 0.09) as u8).min(bars)
        } else {
            bars
        };
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

/// The glyph is drawn in the tint; the dots show the camera / mic / location colours.
fn privacy_caps_lit(_v: f32) -> Vec<Color> {
    PRIVACY_LIT.with(|c| c.borrow().clone())
}

thread_local! {
    /// The colours of the capabilities in use this frame (set by `draw`, read by the glyph).
    static PRIVACY_LIT: std::cell::RefCell<Vec<Color>> = const { std::cell::RefCell::new(Vec::new()) };
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
        AIRPLANE => {
            // a plane; switching on, it glides in from the lower left on a fading contrail
            let p = if on && !settled {
                ease_out(since / 0.65)
            } else {
                1.0
            };
            let (dx, dy) = (-9.0 * s * (1.0 - p), 9.0 * s * (1.0 - p));
            if p < 0.999 {
                cv.line(
                    cx + dx - 6.0 * s * p,
                    cy + dy + 6.0 * s * p,
                    cx + dx,
                    cy + dy,
                    1.4 * s,
                    c.with_alpha(0.5 * (1.0 - p)),
                );
            }
            cv.push_affine(dx, dy, 1.0, 1.0, PI / 4.0, cx, cy);
            cv.icon(Icon::Airplane, cx, cy, 15.0 * s, c);
            cv.pop_tf();
        }
        CAMERA => {
            // camera body + lens; the iris opens when allowed, a shutter closes when blocked
            let body = Rect::new(cx - 7.5 * s, cy - 4.8 * s, 12.0 * s, 9.6 * s);
            cv.stroke_rrect(body, 2.2 * s, c, 1.4 * s);
            // the small front "hood"
            let pts = [
                (body.right(), cy - 1.8 * s),
                (body.right() + 3.4 * s, cy - 3.8 * s),
                (body.right() + 3.4 * s, cy + 3.8 * s),
                (body.right(), cy + 1.8 * s),
            ];
            let factory = cv.gfx.factory.clone();
            if let Ok(g) = shapes::poly(&factory, &pts, false) {
                cv.stroke_geo(&g, c, 1.4 * s);
            }
            let (lx, ly) = (body.cx(), cy);
            let lr = 2.6 * s;
            let open = if settled {
                on as u8 as f32
            } else if on {
                ease_out(since / 0.45)
            } else {
                1.0 - ease_out(since / 0.3)
            };
            cv.stroke_circle(lx, ly, lr, c, 1.2 * s);
            // iris: a dot that grows as it opens
            cv.circle(lx, ly, lr * 0.45 * open, c);
            if !on {
                // shutter: fills the lens, and a slash strikes through with a tiny shake
                cv.circle(lx, ly, lr * (1.0 - open) * 0.95, c.with_alpha(0.85));
                let q = if settled { 1.0 } else { ease_out(since / 0.3) };
                let shake = if since < 0.5 {
                    (since * 60.0).sin() * (0.5 - since) * 3.0 * s
                } else {
                    0.0
                };
                let (x0, y0) = (cx - 8.0 * s + shake, cy - 7.0 * s);
                let (x1, y1) = (cx + 8.0 * s + shake, cy + 7.0 * s);
                cv.line(x0, y0, x0 + (x1 - x0) * q, y0 + (y1 - y0) * q, 1.8 * s, c);
            }
        }
        PRIVACY => {
            // a shield; each capability in use gets a dot that breathes (never blinks)
            cv.icon(Icon::Shield, cx, cy, 16.0 * s, c);
            if on {
                let caps = privacy_caps_lit(v);
                let breathe = 0.7 + 0.3 * (now * 2.6).sin();
                for (k, col) in caps.iter().enumerate() {
                    let (dx, dy) = [(6.5, -6.5), (8.5, -1.0), (6.5, 4.5)][k];
                    cv.circle(cx + dx * s, cy + dy * s, 2.0 * s * breathe, *col);
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
    band: f32,
    scrolling: bool,
) {
    let a = Action::Slider(kind);
    let active = cv.hovered(r) || cv.pressed.as_ref() == Some(&a) || scrolling;
    let (hv, _) = cv.anim(&a, active, false);
    let hv = if scrolling { 1.0 } else { hv };
    let grow = 1.0 + 0.035 * hv.min(1.0);
    cv.push_tf(0.0, 0.0, grow, r.cx(), r.cy());
    // the elastic edge: pushing past the top stretches it up from the bottom, past the
    // bottom stretches it down from the top
    let anchor = if band >= 0.0 { r.bottom() } else { r.y };
    cv.push_affine(
        0.0,
        0.0,
        1.0 - band.abs() * 0.25,
        1.0 + band.abs(),
        0.0,
        r.cx(),
        anchor,
    );
    let rad = CTL_CARD_R + 2.0;
    cv.glass_tile(r, rad, GlassLayer::Raised);
    if enabled {
        let h = (r.h * smooth.clamp(0.0, 1.0)).max(0.0);
        if h > 0.5 {
            cv.push_clip(Rect::new(r.x, r.bottom() - h, r.w, h));
            // the fill: bright white, a breath of grey toward the bottom, a lit top edge
            if let Some(g) = cv.gradient(
                crate::render::canvas::v2(r.x, r.bottom() - h),
                crate::render::canvas::v2(r.x, r.bottom()),
                &[
                    (0.0, WHITE.with_alpha(0.97)),
                    (1.0, Color::rgb(0xE4E6EC).with_alpha(0.95)),
                ],
                1.0,
            ) {
                use windows::core::Interface;
                if let Ok(br) = g.cast::<windows::Win32::Graphics::Direct2D::ID2D1Brush>() {
                    if let Ok(geo) = shapes::squircle(&cv.gfx.factory.clone(), r, rad) {
                        unsafe { cv.ctx.FillGeometry(&geo, &br, None) };
                    }
                }
            }
            cv.pop_clip();
        }
        let iy = r.bottom() - 18.0;
        let covered = r.bottom() - h < iy - 8.0;
        cv.icon_weight(
            icon,
            r.cx(),
            iy,
            17.0,
            if covered { NOTCH } else { cv.theme.ink },
            WEIGHT_TITLE,
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
    cv.pop_tf();
    if enabled {
        cv.zone_named(r, a, true, name, rad);
    }
}

/// The battery card: a ring that fills with the charge, the percentage in it, and what it's
/// doing (charging, time left, saver). Opens Windows' battery settings.
fn battery_card(cv: &mut Canvas, r: Rect, b: &crate::app::state::Battery, charge: f32, t: f32) {
    let a = Action::OpenUrl("ms-settings:batterysaver".into());
    let hv = super::hover_card(cv, r, a.clone(), CTL_CARD_R);
    let cx = r.cx();
    let cy = r.y + r.h * 0.4;
    let ring_r = (r.w.min(r.h) * 0.27).min(26.0);
    if !b.present {
        cv.icon(Icon::Bolt, cx, cy, 20.0, GOOD);
        cv.label(
            "Plugged in",
            &text::CAPTION.size(10.5),
            Rect::new(r.x + 4.0, r.bottom() - 30.0, r.w - 8.0, 14.0),
            cv.theme.mist,
            Align::Center,
        );
        cv.zone_named(r, a, true, "Power settings", CTL_CARD_R);
        return;
    }
    let c = if b.ac {
        GOOD
    } else if b.saver {
        WARN
    } else if b.percent <= 20 {
        CRITICAL
    } else {
        cv.theme.ink
    };
    // a soft glow of the state colour behind the ring while charging
    if b.ac {
        let g = 0.55 + 0.45 * (t * 1.6).sin().abs();
        if let Some(br) = cv.radial(
            cx,
            cy,
            ring_r * 1.8,
            ring_r * 1.8,
            GOOD.with_alpha(0.16 * g),
            GOOD.with_alpha(0.0),
        ) {
            use windows::core::Interface;
            if let Ok(br) = br.cast::<windows::Win32::Graphics::Direct2D::ID2D1Brush>() {
                cv.rrect_brush(
                    Rect::centered(cx, cy, ring_r * 3.6, ring_r * 3.6),
                    ring_r * 1.8,
                    &br,
                );
            }
        }
    }
    cv.ring(cx, cy, ring_r, charge, 4.5, Some(c));
    cv.text(
        &format!("{}%", b.percent),
        &text::BODY_MED.tnum().size(14.0),
        Rect::new(cx - ring_r, cy - 10.0, ring_r * 2.0, 20.0),
        cv.theme.ink,
        Align::Center,
    );
    let status = if b.ac && b.percent >= 100 {
        crate::i18n::tr("Charged")
    } else if b.ac {
        crate::i18n::tr("Charging")
    } else if b.saver {
        crate::i18n::tr("Saver on")
    } else {
        match b.secs_left {
            Some(s) if s >= 3600 => format!("{}h {:02}m", s / 3600, s / 60 % 60),
            Some(s) => format!("{} min", s / 60),
            None => crate::i18n::tr("On battery"),
        }
    };
    let sy = r.bottom() - 24.0;
    let mut tx = cx;
    if b.ac {
        // the bolt breathes beside "Charging"
        let tw = cv.measure(&status, &text::CAPTION.weight(600).size(11.0));
        tx = cx + 7.0;
        let a = 0.6 + 0.4 * (t * 2.4).sin();
        cv.icon(
            Icon::Bolt,
            cx - tw / 2.0 - 2.0,
            sy,
            11.0,
            GOOD.with_alpha(a),
        );
    }
    cv.text(
        &status,
        &text::CAPTION.weight(600).size(11.0),
        Rect::centered(tx, sy, r.w - 8.0, 14.0),
        if b.ac {
            GOOD
        } else {
            cv.theme.mist.lerp(cv.theme.ink, hv.min(1.0) * 0.4)
        },
        Align::Center,
    );
    cv.zone_named(r, a, true, "Battery settings", CTL_CARD_R);
}

/// A quick action: a glass tile with its icon and a small label. Keep awake glows amber with
/// a wisp of steam while it holds the PC awake.
#[allow(clippy::too_many_arguments)]
fn quick(cv: &mut Canvas, r: Rect, ic: Icon, a: Action, name: &str, label: &str, lit: f32, t: f32) {
    let hover = cv.hovered(r);
    let pressed = cv.pressed.as_ref() == Some(&a) && hover;
    let (hv, pv) = cv.anim(&a, hover, pressed);
    let hv = hv.clamp(0.0, 1.0);
    cv.push_tf(0.0, -hv * 1.0, 1.0 + 0.02 * hv - 0.05 * pv, r.cx(), r.cy());
    cv.glass_tile(r, CTL_CARD_R, GlassLayer::Raised);
    if hv > 0.01 {
        cv.sq(r, CTL_CARD_R, WHITE.with_alpha(0.05 * hv));
    }
    if lit > 0.01 {
        if let Some(g) = cv.gradient(
            crate::render::canvas::v2(r.x, r.y),
            crate::render::canvas::v2(r.right(), r.bottom()),
            &[
                (0.0, CTL_AIRPLANE.with_alpha(0.95 * lit)),
                (1.0, CTL_AIRPLANE.lerp(NOTCH, 0.3).with_alpha(0.95 * lit)),
            ],
            1.0,
        ) {
            use windows::core::Interface;
            cv.rrect_brush(r, CTL_CARD_R, &g.cast().expect("brush"));
        }
    }
    let iy = r.y + r.h * 0.4;
    let ink = cv.theme.ink.lerp(WHITE, lit);
    if lit > 0.01 {
        // steam rising from the cup
        for k in 0..2 {
            let f = (t * 0.9 + k as f32 * 0.5).fract();
            cv.circle(
                r.cx() - 2.5 + k as f32 * 5.0 + (t * 3.0 + k as f32).sin() * 0.8,
                iy - 9.0 - f * 5.0,
                1.0,
                WHITE.with_alpha(lit * 0.65 * (1.0 - f)),
            );
        }
    }
    cv.icon_weight(ic, r.cx(), iy, 17.0, ink, WEIGHT_TITLE);
    cv.label(
        name,
        &text::CAPTION.size(10.0).weight(WEIGHT_CAPTION),
        Rect::new(r.x + 3.0, r.bottom() - 19.0, r.w - 6.0, 13.0),
        cv.theme.mist.lerp(WHITE, lit),
        Align::Center,
    );
    cv.pop_tf();
    cv.zone_named(r, a, true, label, CTL_CARD_R);
}

/// "In use now": who has the camera, microphone or location, with Close / Force quit.
fn sheet(st: &mut State, cv: &mut Canvas, body: Rect, p: f32) {
    let uses = st.f.privacy.uses.clone();
    let now = std::time::Instant::now();
    let t = st.started.elapsed().as_secs_f32();
    // the page dims behind it; a click outside closes it
    cv.sq(body.inset(-4.0), 18.0, NOTCH.with_alpha(0.55 * p));
    cv.zone_named(
        body.inset(-4.0),
        Action::CtlSheet(false),
        false,
        "Close",
        18.0,
    );
    let rows = uses.len().max(1) as f32;
    let w = (body.w - 40.0).min(SHEET_W);
    let h = (52.0 + rows * SHEET_ROW + 30.0).min(body.h - 6.0);
    let r = Rect::new(
        body.cx() - w / 2.0,
        body.y + (body.h - h) / 2.0 + (1.0 - p) * 14.0,
        w,
        h,
    );
    cv.push_alpha(p);
    cv.push_tf(0.0, 0.0, 0.96 + 0.04 * p, r.cx(), r.cy());
    cv.sq(r, 18.0, cv.theme.surface.with_alpha(0.97));
    cv.glass_tile(r, 18.0, GlassLayer::Raised);
    cv.zone(r, Action::None);
    // header
    cv.icon(Icon::Shield, r.x + 22.0, r.y + 22.0, 16.0, cv.theme.ink);
    cv.label(
        "In use now",
        &text::BODY_MED,
        Rect::new(r.x + 38.0, r.y + 12.0, r.w - 80.0, 20.0),
        cv.theme.ink,
        Align::Left,
    );
    cv.button(
        Icon::Close,
        r.right() - 20.0,
        r.y + 22.0,
        26.0,
        Action::CtlSheet(false),
        "Close",
        cv.theme.mist,
    );
    let list_y = r.y + 44.0;
    if uses.is_empty() {
        cv.icon(
            Icon::CheckCircle,
            r.cx(),
            list_y + 14.0,
            20.0,
            GOOD.with_alpha(0.85),
        );
        cv.label(
            "Nothing is using your camera, microphone or location",
            &text::CAPTION,
            Rect::new(r.x + 12.0, list_y + 28.0, r.w - 24.0, 16.0),
            cv.theme.mist,
            Align::Center,
        );
    }
    let log = st.f.state.privacy_log.clone();
    let h24 = st.clock_24h();
    for (i, u) in uses.iter().enumerate() {
        let y = list_y + i as f32 * SHEET_ROW;
        let rr = Rect::new(r.x + 10.0, y, r.w - 20.0, SHEET_ROW - 4.0);
        // rows rise in one after another
        let k = ((p - 0.15 * i as f32).max(0.0) / 0.85).min(1.0);
        cv.push_alpha(k);
        cv.push_tf(0.0, (1.0 - k) * 6.0, 1.0, 0.0, 0.0);
        let (col, ic) = match u.cap {
            Cap::Camera => (PRIVACY_CAM, Icon::Camera),
            Cap::Mic => (PRIVACY_MIC, Icon::Mic),
            Cap::Location => (PRIVACY_LOC, Icon::Location),
        };
        let dc = (rr.x + 18.0, rr.cy());
        cv.circle(dc.0, dc.1, 15.0, col.with_alpha(0.18));
        cv.icon(ic, dc.0, dc.1, 15.0, col);
        // the live dot breathes
        let br = 0.75 + 0.25 * (t * 2.6 + i as f32).sin();
        cv.circle(dc.0 + 10.5, dc.1 - 10.5, 3.2, cv.theme.surface);
        cv.circle(dc.0 + 10.5, dc.1 - 10.5, 2.4 * br, col);
        let since = log
            .iter()
            .rev()
            .find(|l| l.stop.is_none() && l.cap == u.cap && l.app == u.app)
            .map(|l| {
                let m = (crate::util::unix_secs() - l.start).max(0) / 60;
                if m < 1 {
                    crate::i18n::tr("just now")
                } else if m < 60 {
                    format!("{m} min")
                } else {
                    crate::util::fmt_hm(crate::util::unix_to_local(l.start).minutes_of_day(), h24)
                }
            })
            .unwrap_or_default();
        let tx = rr.x + 42.0;
        let bw = 92.0;
        cv.text(
            &u.app,
            &text::BODY_MED,
            Rect::new(tx, rr.y + 3.0, rr.right() - tx - bw - 8.0, 18.0),
            cv.theme.ink,
            Align::Left,
        );
        let sub = if since.is_empty() {
            crate::i18n::tr(u.cap.label())
        } else {
            format!("{} · {since}", crate::i18n::tr(u.cap.label()))
        };
        cv.text(
            &sub,
            &text::CAPTION.size(10.5),
            Rect::new(tx, rr.y + 21.0, rr.right() - tx - bw - 8.0, 14.0),
            cv.theme.mist,
            Align::Left,
        );
        let b = Rect::new(rr.right() - bw, rr.cy() - 13.0, bw, 26.0);
        if !crate::platform::apps::closable(&u.key, u.packaged) {
            cv.label("Windows", &text::CAPTION, b, FAINT, Align::Right);
        } else {
            match st.f.privacy.closing.get(&u.key) {
                Some(at) if now.duration_since(*at).as_secs_f32() < CLOSE_GRACE_SECS => {
                    // waiting for the app to close: three soft dots
                    cv.label(
                        "Closing",
                        &text::CAPTION,
                        Rect::new(b.x, b.y, b.w - 18.0, b.h),
                        cv.theme.mist,
                        Align::Right,
                    );
                    for d in 0..3 {
                        let a = 0.3 + 0.6 * ((t * 5.0 - d as f32 * 0.8).sin() * 0.5 + 0.5);
                        cv.circle(
                            b.right() - 12.0 + d as f32 * 5.0,
                            b.cy() + 1.0,
                            1.4,
                            cv.theme.mist.with_alpha(a),
                        );
                    }
                }
                Some(_) => {
                    // it's still there: Force quit, in red, morphing in
                    let a = Action::AppForceQuit(u.key.clone(), u.packaged);
                    let (hv, pv) = cv.anim(&a, cv.hovered(b), false);
                    cv.push_tf(0.0, 0.0, 1.0 + 0.03 * hv - 0.05 * pv, b.cx(), b.cy());
                    cv.rrect(b, 13.0, CRITICAL.with_alpha(0.85 + 0.15 * hv));
                    cv.label(
                        "Force quit",
                        &text::CAPTION.weight(700),
                        b,
                        WHITE,
                        Align::Center,
                    );
                    cv.pop_tf();
                    cv.zone_named(b, a, true, "Force quit", 13.0);
                }
                None => {
                    let label = format!("{} {}", crate::i18n::tr("Close"), u.app);
                    let a = Action::AppClose(u.key.clone(), u.packaged);
                    let (hv, pv) = cv.anim(&a, cv.hovered(b), false);
                    cv.push_tf(0.0, 0.0, 1.0 + 0.03 * hv - 0.05 * pv, b.cx(), b.cy());
                    cv.rrect(b, 13.0, LINE.with_alpha(1.4 + 1.2 * hv));
                    cv.label(
                        "Close app",
                        &text::CAPTION.weight(700),
                        b,
                        cv.theme.ink,
                        Align::Center,
                    );
                    cv.pop_tf();
                    cv.zone_named(b, a, true, &label, 13.0);
                }
            }
        }
        cv.pop_tf();
        cv.pop_alpha();
    }
    // footer: the full history and Windows' own switches
    let fy = r.bottom() - 24.0;
    cv.line(r.x + 14.0, fy - 6.0, r.right() - 14.0, fy - 6.0, 1.0, LINE);
    let link = |cv: &mut Canvas, label: &str, x: f32, a: Action, align: Align| {
        let w = cv.measure(&crate::i18n::tr(label), &text::CAPTION.weight(600)) + 8.0;
        let lr = match align {
            Align::Right => Rect::new(x - w, fy - 4.0, w, 22.0),
            _ => Rect::new(x, fy - 4.0, w, 22.0),
        };
        let hv = cv.hovered(lr);
        cv.label(
            label,
            &text::CAPTION.weight(600),
            lr,
            if hv { cv.theme.ink } else { cv.theme.accent[0] },
            Align::Center,
        );
        cv.zone_named(lr, a, true, label, 6.0);
    };
    link(
        cv,
        "History",
        r.x + 12.0,
        Action::Open(crate::app::PageId::Privacy),
        Align::Left,
    );
    link(
        cv,
        "Windows privacy settings",
        r.right() - 12.0,
        Action::OpenUrl("ms-settings:privacy".into()),
        Align::Right,
    );
    cv.pop_tf();
    cv.pop_alpha();
}
