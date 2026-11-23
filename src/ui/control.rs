//! Control Center: system switches, big vertical sliders and quick actions.
//!
//! Left: six squircle toggles (Wi-Fi, Bluetooth, Focus, Dark mode, Keep awake, Mic) that fill
//! with their colour on a bouncy spring and pop when switched. Middle: tall capsule sliders for
//! brightness and volume, filled from the bottom like iOS. Right: battery ring and quick actions.

use super::{header, row, Reveal};
use crate::app::state::State;
use crate::app::{Action, SliderKind};
use crate::render::icons::{self, Icon};
use crate::render::text::{self, Align};
use crate::render::{Canvas, Rect};
use crate::theme::tokens::*;

/// Toggle ids (index into `Features::ctl_fx`).
pub const WIFI: u8 = 0;
pub const BLUETOOTH: u8 = 1;
pub const FOCUS: u8 = 2;
pub const DARK: u8 = 3;
pub const AWAKE: u8 = 4;
pub const MIC: u8 = 5;

struct Toggle {
    id: u8,
    icon: Icon,
    label: &'static str,
    on: bool,
    available: bool,
    state: String,
    tint: Color,
}

fn toggles(st: &State) -> Vec<Toggle> {
    let r = st.sys.radios;
    let wifi_on = r.wifi.unwrap_or(st.sys.net.wifi);
    let bt_on = r.bluetooth.unwrap_or(false);
    let wifi_state = match (r.wifi, st.sys.net.wifi && st.sys.net.connected) {
        (Some(false), _) => "Off".to_string(),
        (_, true) if !st.sys.net.name.is_empty() => st.sys.net.name.clone(),
        (Some(true), _) => "Not connected".into(),
        _ => "Unavailable".into(),
    };
    let bt_state = match r.bluetooth {
        Some(true) => st
            .sys
            .bt_connected
            .first()
            .cloned()
            .unwrap_or_else(|| "On".into()),
        Some(false) => "Off".into(),
        None => "Unavailable".into(),
    };
    let on_off = |b: bool| if b { "On" } else { "Off" }.to_string();
    vec![
        Toggle {
            id: WIFI,
            icon: if wifi_on { Icon::Wifi3 } else { Icon::WifiOff },
            label: "Wi-Fi",
            on: wifi_on,
            available: r.wifi.is_some(),
            state: wifi_state,
            tint: Color::rgb(0x0A84FF),
        },
        Toggle {
            id: BLUETOOTH,
            icon: Icon::Bluetooth,
            label: "Bluetooth",
            on: bt_on,
            available: r.bluetooth.is_some(),
            state: bt_state,
            tint: Color::rgb(0x0A84FF),
        },
        Toggle {
            id: FOCUS,
            icon: Icon::MoonStar,
            label: "Focus",
            on: st.settings.quiet.focus,
            available: true,
            state: on_off(st.settings.quiet.focus),
            tint: Color::rgb(0x5E5CE6),
        },
        Toggle {
            id: DARK,
            icon: Icon::Contrast,
            label: "Dark mode",
            on: st.sys.dark_mode,
            available: true,
            state: on_off(st.sys.dark_mode),
            tint: Color::rgb(0xF2F2F7),
        },
        Toggle {
            id: AWAKE,
            icon: Icon::Coffee,
            label: "Keep awake",
            on: st.sys.keep_awake,
            available: true,
            state: on_off(st.sys.keep_awake),
            tint: Color::rgb(0xFF9F0A),
        },
        Toggle {
            id: MIC,
            icon: if st.sys.mic_muted {
                Icon::MicOff
            } else {
                Icon::Mic
            },
            label: "Microphone",
            on: st.sys.mic_muted,
            available: true,
            state: if st.sys.mic_muted { "Muted" } else { "Live" }.into(),
            tint: Color::rgb(0xFF453A),
        },
    ]
}

pub fn draw(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    let body = header(cv, area, Icon::Sliders, "Control Center", "");
    let ts = toggles(st);
    // keep the fill springs pointed at the real switch positions
    for t in &ts {
        let s = &mut st.f.ctl_fx[t.id as usize];
        let target = if t.on { 1.0 } else { 0.0 };
        if (s.target - target).abs() > 0.5 {
            s.set(target);
        }
    }

    // ---------------------------------------------------------------- toggles (3 × 2)
    let cw = 76.0;
    let ch = (body.h / 2.0).min(90.0);
    let tile = 56.0;
    for (i, t) in ts.iter().enumerate() {
        let col = (i % 3) as f32;
        let rown = (i / 3) as f32;
        let cell = Rect::new(body.x + col * cw, body.y + rown * ch, cw, ch);
        let tr = Rect::new(cell.cx() - tile / 2.0, cell.y + 2.0, tile, tile);
        let v = st.f.ctl_fx[t.id as usize].value;
        row(cv, rv, 1 + i, cell, |cv| {
            let a = Action::Ctl(t.id);
            let (hv, pv) = cv.anim(&a, cv.hovered(tr), false);
            // pop on switch (the bouncy spring overshoots past 0 / 1)
            let pop = (v - v.clamp(0.0, 1.0)).abs();
            let scale = (1.0 + 0.04 * hv.min(1.0)) * (1.0 - 0.07 * pv) * (1.0 + 0.6 * pop);
            cv.push_tf(0.0, 0.0, scale, tr.cx(), tr.cy());
            let vc = v.clamp(0.0, 1.0);
            let off = cv.theme.surface_raised.lerp(cv.theme.surface, 0.0);
            let fill = off.lerp(t.tint, vc);
            cv.sq(tr, 18.0, fill);
            if vc > 0.02 {
                // soft inner sheen on the lit tile
                cv.sq(
                    Rect::new(tr.x, tr.y, tr.w, tr.h * 0.5),
                    18.0,
                    WHITE.with_alpha(0.06 * vc),
                );
            }
            cv.stroke_rrect(tr.inset(0.5), 17.5, RING.with_alpha(1.0 - vc), 1.0);
            // light tiles (dark mode) get a dark glyph when lit
            let lit_icon = if t.tint.r + t.tint.g + t.tint.b > 2.4 {
                NOTCH
            } else {
                WHITE
            };
            let ic = cv.theme.ink.lerp(lit_icon, vc);
            let ic = if t.available { ic } else { FAINT };
            cv.icon(t.icon, tr.cx(), tr.cy(), 22.0, ic);
            cv.pop_tf();
            cv.zone_named(tr, a, true, t.label, 18.0);
            cv.label(
                t.label,
                &text::CAPTION.weight(600).size(11.0),
                Rect::new(cell.x, tr.bottom() + 4.0, cell.w, 14.0),
                cv.theme.ink,
                Align::Center,
            );
            cv.label(
                &t.state,
                &text::CAPTION.size(10.0),
                Rect::new(cell.x + 2.0, tr.bottom() + 17.0, cell.w - 4.0, 12.0),
                if t.on {
                    t.tint.lerp(cv.theme.ink, 0.35)
                } else {
                    cv.theme.mist
                },
                Align::Center,
            );
        });
    }

    // ---------------------------------------------------------------- tall sliders
    let sx = body.x + 3.0 * cw + 12.0;
    let sh = body.h - 4.0;
    let sw = 54.0;
    let bright = st.sys.brightness.map(|b| b as f32 / 100.0);
    let vol = if st.sys.muted { 0.0 } else { st.sys.volume };
    let sliders = [
        (
            Rect::new(sx, body.y + 2.0, sw, sh),
            SliderKind::Brightness,
            bright.unwrap_or(0.5),
            Icon::Sun,
            "Brightness",
            bright.is_some(),
        ),
        (
            Rect::new(sx + sw + 12.0, body.y + 2.0, sw, sh),
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

    // ---------------------------------------------------------------- status + quick actions
    let rx = sx + 2.0 * sw + 24.0;
    let rw = body.right() - rx;
    let b = st.sys.battery.clone();
    row(cv, rv, 4, Rect::new(rx, body.y, rw, 64.0), |cv| {
        let cx = rx + rw / 2.0;
        let cy = body.y + 28.0;
        if b.present {
            let c = if b.ac {
                GOOD
            } else if b.percent <= 20 {
                CRITICAL
            } else {
                cv.theme.ink
            };
            cv.ring(cx, cy, 24.0, b.percent as f32 / 100.0, 4.0, Some(c));
            cv.text(
                &format!("{}%", b.percent),
                &text::BODY_MED.tnum(),
                Rect::new(cx - 24.0, cy - 9.0, 48.0, 18.0),
                cv.theme.ink,
                Align::Center,
            );
            if b.ac {
                cv.icon(Icon::Bolt, cx, cy + 32.0, 11.0, GOOD);
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
    let gy = body.y + 76.0;
    let gc = (rw / 2.0).min(48.0);
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
        // icon at the foot: dark over the fill, light over the track
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
