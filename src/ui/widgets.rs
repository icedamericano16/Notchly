//! Widgets: the picker page and the desktop widget renderer.

use super::{row, Reveal};
use crate::app::state::State;
use crate::app::Action;
use crate::render::icons::Icon;
use crate::render::text::{self, Align};
use crate::render::{Canvas, Rect};
use crate::theme::tokens::*;
use crate::util::{fmt_hm, local_now, MONTHS, WEEKDAYS};

pub const KINDS: &[&str] = &[
    "clock",
    "worldclocks",
    "weather",
    "calendar",
    "sysmon",
    "battery",
    "nowplaying",
    "countdown",
    "notes",
    "focus",
];

fn label(k: &str) -> (&'static str, Icon) {
    match k {
        "clock" => ("Clock", Icon::Timer),
        "worldclocks" => ("World clocks", Icon::Globe),
        "weather" => ("Weather", Icon::CloudSun),
        "calendar" => ("Calendar", Icon::Calendar),
        "sysmon" => ("System monitor", Icon::Cpu),
        "battery" => ("Battery", Icon::Bolt),
        "nowplaying" => ("Now playing", Icon::Music),
        "countdown" => ("Countdown", Icon::Star),
        "notes" => ("Sticky note", Icon::Note),
        _ => ("Focus stats", Icon::Timer),
    }
}

pub fn page(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    let body = super::header(
        cv,
        area,
        Icon::Widgets,
        "Desktop widgets",
        "Drag to move on the desktop",
    );
    let w = (body.w - 16.0) / 3.0;
    for (i, k) in KINDS.iter().enumerate() {
        let r = Rect::new(
            body.x + (i % 3) as f32 * (w + 8.0),
            body.y + (i / 3) as f32 * 42.0,
            w,
            36.0,
        );
        let on = st.settings.widgets.desktop.iter().any(|d| d.kind == *k);
        let (name, icon) = label(k);
        row(cv, rv, i.min(6), r, |cv| {
            cv.sq(
                r,
                12.0,
                if on {
                    cv.theme.accent[1].with_alpha(0.3)
                } else {
                    cv.theme.surface
                },
            );
            cv.icon(
                icon,
                r.x + 16.0,
                r.cy(),
                14.0,
                if on {
                    cv.theme.accent[0]
                } else {
                    cv.theme.mist
                },
            );
            cv.text(
                name,
                &text::CAPTION.weight(600),
                Rect::new(r.x + 32.0, r.y, r.w - 36.0, r.h),
                cv.theme.ink,
                Align::Left,
            );
            cv.zone_named(r, Action::WidgetToggle(i as u8), true, name, 12.0);
        });
    }
}

/// Size of a desktop widget in DIPs.
pub fn size(_kind: &str) -> (f32, f32) {
    (220.0, 130.0)
}

/// Draws a desktop widget panel filling `r`.
pub fn draw_desktop(st: &mut State, cv: &mut Canvas, kind: &str, r: Rect) {
    cv.sq(r, R_TILE, cv.theme.surface.with_alpha(0.92));
    cv.stroke_rrect(r.inset(0.5), R_TILE, RING, 1.0);
    let a = r.inset(16.0);
    let h24 = st.clock_24h();
    let now = local_now();
    let (name, icon) = label(kind);
    match kind {
        "clock" => {
            cv.text(
                &fmt_hm(now.minutes_of_day(), h24),
                &text::DISPLAY,
                Rect::new(a.x, a.y, a.w, 48.0),
                cv.theme.ink,
                Align::Left,
            );
            cv.text(
                &format!(
                    "{}, {} {}",
                    WEEKDAYS[now.weekday as usize],
                    MONTHS[now.month as usize - 1],
                    now.day
                ),
                &text::BODY,
                Rect::new(a.x, a.y + 52.0, a.w, 18.0),
                cv.theme.mist,
                Align::Left,
            );
            if st.settings.general.nepali_date {
                if let Some(bs) = crate::features::bsdate::format(
                    now.day_number(),
                    st.settings.general.nepali_numerals,
                ) {
                    cv.text(
                        &bs,
                        &text::CAPTION,
                        Rect::new(a.x, a.y + 72.0, a.w, 16.0),
                        FAINT,
                        Align::Left,
                    );
                }
            }
        }
        "worldclocks" => {
            cv.text(
                name,
                &text::MICRO,
                Rect::new(a.x, a.y, a.w, 14.0),
                cv.theme.mist,
                Align::Left,
            );
            let utc = crate::util::unix_secs();
            for (i, wc) in st.settings.general.world_clocks.iter().take(4).enumerate() {
                let m = ((utc + wc.offset_min as i64 * 60).rem_euclid(86400) / 60) as u32;
                cv.text(
                    &wc.city,
                    &text::BODY,
                    Rect::new(a.x, a.y + 18.0 + i as f32 * 20.0, a.w * 0.6, 18.0),
                    cv.theme.ink,
                    Align::Left,
                );
                cv.text(
                    &fmt_hm(m, h24),
                    &text::BODY_MED,
                    Rect::new(a.x, a.y + 18.0 + i as f32 * 20.0, a.w, 18.0),
                    cv.theme.ink,
                    Align::Right,
                );
            }
            if st.settings.general.world_clocks.is_empty() {
                cv.text(
                    "Add cities in Settings › General",
                    &text::CAPTION,
                    Rect::new(a.x, a.y + 20.0, a.w, 16.0),
                    FAINT,
                    Align::Left,
                );
            }
        }
        "weather" => match st.f.weather.forecast.clone() {
            Some(f) => {
                cv.icon(
                    crate::services::weather::icon_for(&f.symbol),
                    a.x + 18.0,
                    a.y + 22.0,
                    32.0,
                    cv.theme.ink,
                );
                cv.text(
                    &format!("{:.0}°", super::dashboard::temp(st, f.temp)),
                    &text::DISPLAY.size(34.0),
                    Rect::new(a.x + 44.0, a.y, 120.0, 44.0),
                    cv.theme.ink,
                    Align::Left,
                );
                cv.text(
                    crate::services::weather::describe(&f.symbol),
                    &text::BODY,
                    Rect::new(a.x, a.y + 54.0, a.w, 18.0),
                    cv.theme.mist,
                    Align::Left,
                );
            }
            None => {
                cv.text(
                    "Weather unavailable",
                    &text::BODY,
                    a,
                    cv.theme.mist,
                    Align::Center,
                );
            }
        },
        "calendar" => {
            cv.text(
                name,
                &text::MICRO,
                Rect::new(a.x, a.y, a.w, 14.0),
                cv.theme.mist,
                Align::Left,
            );
            let now_s = crate::util::unix_secs();
            for (i, e) in
                st.f.calendar
                    .events
                    .iter()
                    .filter(|e| e.end > now_s)
                    .take(4)
                    .enumerate()
            {
                let lt = crate::util::unix_to_local(e.start);
                cv.circle(
                    a.x + 4.0,
                    a.y + 27.0 + i as f32 * 20.0,
                    3.0,
                    Color::rgb(e.color),
                );
                cv.text(
                    &format!("{} {}", fmt_hm(lt.minutes_of_day(), h24), e.title),
                    &text::CAPTION,
                    Rect::new(a.x + 14.0, a.y + 18.0 + i as f32 * 20.0, a.w - 14.0, 18.0),
                    cv.theme.ink,
                    Align::Left,
                );
            }
        }
        "sysmon" => {
            let s = st.f.sysmon.last.unwrap_or_default();
            cv.text(
                name,
                &text::MICRO,
                Rect::new(a.x, a.y, a.w, 14.0),
                cv.theme.mist,
                Align::Left,
            );
            for (i, (l, v)) in [("CPU", s.cpu), ("RAM", s.ram), ("GPU", s.gpu)]
                .iter()
                .enumerate()
            {
                let y = a.y + 22.0 + i as f32 * 22.0;
                cv.text(
                    l,
                    &text::CAPTION,
                    Rect::new(a.x, y, 40.0, 14.0),
                    cv.theme.mist,
                    Align::Left,
                );
                cv.progress(
                    Rect::new(a.x + 40.0, y + 5.0, a.w - 80.0, 5.0),
                    v / 100.0,
                    false,
                );
                cv.text(
                    &format!("{v:.0}%"),
                    &text::CAPTION_TNUM,
                    Rect::new(a.right() - 36.0, y, 36.0, 14.0),
                    cv.theme.ink,
                    Align::Right,
                );
            }
        }
        "battery" => {
            let b = &st.sys.battery;
            cv.icon(
                Icon::Bolt,
                a.x + 12.0,
                a.y + 20.0,
                24.0,
                if b.ac { GOOD } else { cv.theme.ink },
            );
            cv.text(
                &format!("{}%", b.percent),
                &text::DISPLAY.size(34.0),
                Rect::new(a.x + 36.0, a.y, 140.0, 44.0),
                cv.theme.ink,
                Align::Left,
            );
            cv.progress(
                Rect::new(a.x, a.y + 60.0, a.w, 6.0),
                b.percent as f32 / 100.0,
                false,
            );
        }
        "nowplaying" => {
            if st.media.has_session() {
                let art = Rect::new(a.x, a.y, 56.0, 56.0);
                if let Some(img) = st.media.art.as_mut() {
                    cv.image(img, art, R_ART);
                }
                cv.text(
                    &st.media.snap.title,
                    &text::BODY_MED,
                    Rect::new(art.right() + 10.0, a.y + 6.0, a.w - 66.0, 18.0),
                    cv.theme.ink,
                    Align::Left,
                );
                cv.text(
                    &st.media.snap.artist,
                    &text::CAPTION,
                    Rect::new(art.right() + 10.0, a.y + 26.0, a.w - 66.0, 16.0),
                    cv.theme.mist,
                    Align::Left,
                );
                cv.progress(
                    Rect::new(a.x, a.y + 70.0, a.w, 4.0),
                    st.media.progress(),
                    false,
                );
            } else {
                cv.text(
                    "Nothing playing",
                    &text::BODY,
                    a,
                    cv.theme.mist,
                    Align::Center,
                );
            }
        }
        "countdown" => {
            let w = &st.settings.widgets;
            let days = w.countdown_day - now.day_number();
            cv.text(
                if w.countdown_title.is_empty() {
                    "Countdown"
                } else {
                    &w.countdown_title
                },
                &text::MICRO,
                Rect::new(a.x, a.y, a.w, 14.0),
                cv.theme.mist,
                Align::Left,
            );
            cv.text(
                &if w.countdown_day == 0 {
                    "—".into()
                } else {
                    format!("{days}")
                },
                &text::DISPLAY,
                Rect::new(a.x, a.y + 16.0, a.w, 48.0),
                cv.theme.ink,
                Align::Left,
            );
            cv.text(
                "days",
                &text::CAPTION,
                Rect::new(a.x, a.y + 66.0, a.w, 14.0),
                cv.theme.mist,
                Align::Left,
            );
        }
        "notes" => {
            let text_note =
                st.f.notes
                    .notes
                    .iter()
                    .find(|n| n.pinned)
                    .or(st.f.notes.notes.first())
                    .map(|n| n.text.clone())
                    .unwrap_or_else(|| "Write a note in the notch".into());
            cv.text_top(
                &text_note,
                &text::BODY.lines(5),
                a.x,
                a.y,
                a.w,
                cv.theme.ink,
                Align::Left,
            );
        }
        _ => {
            cv.icon(icon, a.x + 10.0, a.y + 10.0, 16.0, cv.theme.accent[0]);
            cv.text(
                &format!("{} min focused today", st.f.state.timer.today_minutes()),
                &text::BODY_MED,
                Rect::new(a.x + 24.0, a.y, a.w - 24.0, 20.0),
                cv.theme.ink,
                Align::Left,
            );
            super::bars(
                cv,
                Rect::new(a.x, a.y + 30.0, a.w, a.h - 30.0),
                &st.f.state.timer.week(),
                0,
                true,
            );
        }
    }
}
