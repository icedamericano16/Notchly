//! Diagnostics (hidden page: open it from Ask › "Diagnostics", Settings › General, or
//! `--page diagnostics`): live input events on the left; camera / microphone detection,
//! glass and press settings on the right. For checking behaviour on real hardware.

use super::{header, row, Reveal};
use crate::app::state::State;
use crate::render::icons::Icon;
use crate::render::text::{self, Align};
use crate::render::{Canvas, Rect};
use crate::services::privacy::Cap;
use crate::theme::tokens::*;

pub fn draw(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    let body = header(cv, area, Icon::Info, "Diagnostics", "");
    let gap = 12.0;
    let left = Rect::new(body.x, body.y, (body.w - gap) * 0.55, body.h);
    let right = Rect::new(
        left.right() + gap,
        body.y,
        body.right() - left.right() - gap,
        body.h,
    );

    // ---------------------------------------------------------------- input events
    let events: Vec<(std::time::Instant, String)> = st.diag.iter().rev().cloned().collect();
    row(cv, rv, 1, left, |cv| {
        cv.glass_tile(left, 14.0, crate::render::glass::GlassLayer::Base);
        cv.label(
            "Input and detection events",
            &text::MICRO,
            Rect::new(left.x + 12.0, left.y + 8.0, left.w - 24.0, 14.0),
            FAINT,
            Align::Left,
        );
        if events.is_empty() {
            cv.label(
                "Click, long-press, right-click or scroll to see events",
                &text::CAPTION,
                Rect::new(left.x + 12.0, left.y + 28.0, left.w - 24.0, 16.0),
                cv.theme.mist,
                Align::Left,
            );
        }
        let lh = 15.0;
        for (i, (t, s)) in events.iter().enumerate() {
            let y = left.y + 26.0 + i as f32 * lh;
            if y + lh > left.bottom() - 6.0 {
                break;
            }
            let ago = t.elapsed().as_secs_f32();
            let when = if ago < 60.0 {
                format!("{ago:>4.1}s")
            } else {
                format!("{:>3.0}m", ago / 60.0)
            };
            // the newest stands out
            let ink = if i == 0 { cv.theme.ink } else { cv.theme.mist };
            cv.text(
                &when,
                &text::MONO.size(10.5),
                Rect::new(left.x + 12.0, y, 40.0, lh),
                FAINT,
                Align::Left,
            );
            cv.text(
                s,
                &text::MONO.size(10.5),
                Rect::new(left.x + 54.0, y, left.w - 64.0, lh),
                ink,
                Align::Left,
            );
        }
    });

    // ---------------------------------------------------------------- detection + state
    let p = &st.f.privacy;
    let src = |live: Option<bool>| match live {
        Some(true) => ("live", GOOD),
        Some(false) => ("consent store (fallback)", WARN),
        None => ("starting…", FAINT),
    };
    let (mic_src, mic_c) = src(p.sources.map(|s| s.0));
    let (cam_src, cam_c) = src(p.sources.map(|s| s.1));
    let mut lines: Vec<(String, String, Color)> = vec![
        ("Microphone".into(), mic_src.into(), mic_c),
        ("Camera".into(), cam_src.into(), cam_c),
        ("Location".into(), "consent store".into(), FAINT),
        (
            "Camera access".into(),
            match p.camera_allowed {
                Some(true) => "allowed".into(),
                Some(false) => "blocked".into(),
                None => "unknown".into(),
            },
            cv.theme.mist,
        ),
    ];
    for cap in [Cap::Camera, Cap::Mic, Cap::Location] {
        let who: Vec<&str> = p
            .uses
            .iter()
            .filter(|u| u.cap == cap)
            .map(|u| u.app.as_str())
            .collect();
        lines.push((
            format!("{} in use", cap.label()),
            if who.is_empty() {
                "—".into()
            } else {
                who.join(", ")
            },
            if who.is_empty() { FAINT } else { cv.theme.ink },
        ));
    }
    lines.push((
        "Last change".into(),
        p.changed
            .map(|t| format!("{:.0} s ago", t.elapsed().as_secs_f32()))
            .unwrap_or_else(|| "—".into()),
        cv.theme.mist,
    ));
    lines.push((
        "Glass".into(),
        format!(
            "{}{}",
            st.glass.tier.name(),
            st.glass
                .reason
                .map(|r| format!(" · {}", r.message()))
                .unwrap_or_default()
        ),
        cv.theme.mist,
    ));
    lines.push((
        "Long press".into(),
        format!("{} ms", st.settings.general.long_press().as_millis()),
        cv.theme.mist,
    ));
    lines.push((
        "Debug log".into(),
        if st.settings.general.debug_log {
            crate::util::log_dir().to_string_lossy().to_string()
        } else {
            "off".into()
        },
        cv.theme.mist,
    ));
    row(cv, rv, 2, right, |cv| {
        cv.glass_tile(right, 14.0, crate::render::glass::GlassLayer::Base);
        cv.label(
            "Camera, microphone and app state",
            &text::MICRO,
            Rect::new(right.x + 12.0, right.y + 8.0, right.w - 24.0, 14.0),
            FAINT,
            Align::Left,
        );
        let lh = 17.0;
        for (i, (k, v, c)) in lines.iter().enumerate() {
            let y = right.y + 26.0 + i as f32 * lh;
            if y + lh > right.bottom() - 4.0 {
                break;
            }
            cv.text(
                k,
                &text::CAPTION.size(10.5),
                Rect::new(right.x + 12.0, y, 92.0, lh),
                cv.theme.mist,
                Align::Left,
            );
            cv.text(
                v,
                &text::CAPTION.size(10.5).weight(600),
                Rect::new(right.x + 104.0, y, right.w - 114.0, lh),
                *c,
                Align::Left,
            );
        }
    });
}
