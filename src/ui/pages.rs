//! Pro pages, notifications, onboarding and the screenshot annotator.

use super::dashboard::temp;
use super::productivity::clamp_scroll;
use super::{header, row, text_input, Reveal};
use crate::app::state::State;
use crate::app::{Action, InputId, PageId};
use crate::render::icons::Icon;
use crate::render::text::{self, Align};
use crate::render::{Canvas, Rect};
use crate::services::weather::{describe, icon_for};
use crate::theme::tokens::*;
use crate::util::{fmt_hm, fmt_mmss, local_now, unix_secs, unix_to_local, WEEKDAYS};

pub fn weather(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    let Some(f) = st.f.weather.forecast.clone() else {
        let msg = if st.settings.weather.lat == 0.0 && st.settings.weather.lon == 0.0 {
            "Set your city in Settings › Calendar & weather".to_string()
        } else if st.f.weather.fetching {
            "Loading weather…".into()
        } else {
            st.f.weather
                .error
                .clone()
                .unwrap_or_else(|| "No data yet".into())
        };
        row(cv, rv, 0, area, |cv| {
            cv.icon(
                Icon::CloudSun,
                area.cx(),
                area.cy() - 16.0,
                26.0,
                cv.theme.mist,
            );
            cv.text(
                &msg,
                &text::BODY,
                Rect::new(area.x, area.cy() + 6.0, area.w, 18.0),
                cv.theme.mist,
                Align::Center,
            );
        });
        return;
    };
    let unit = if st.settings.weather.units == crate::settings::Units::Imperial {
        "°F"
    } else {
        "°C"
    };
    let wind = if st.settings.weather.units == crate::settings::Units::Imperial {
        format!("{:.0} mph", f.wind * 2.237)
    } else {
        format!("{:.0} km/h", f.wind * 3.6)
    };
    let t = st.started.elapsed().as_secs_f32();
    let (now_r, rest) = area.split_left(150.0);
    row(cv, rv, 0, now_r, |cv| {
        // animated icon: sun rotates slowly, clouds drift
        let ic = icon_for(&f.symbol);
        let (dx, rot) = match ic {
            Icon::Cloud | Icon::CloudSun | Icon::Rain | Icon::Snow | Icon::Storm => {
                ((t * 0.8).sin() * 2.0, 0.0)
            }
            _ => (0.0, 1.0),
        };
        let _ = rot;
        cv.icon(ic, now_r.x + 22.0 + dx, now_r.y + 22.0, 36.0, cv.theme.ink);
        cv.text(
            &format!("{:.0}{unit}", temp(st, f.temp)),
            &text::DISPLAY.size(34.0),
            Rect::new(now_r.x + 48.0, now_r.y + 2.0, 100.0, 40.0),
            cv.theme.ink,
            Align::Left,
        );
        cv.text(
            describe(&f.symbol),
            &text::BODY_MED,
            Rect::new(now_r.x, now_r.y + 46.0, now_r.w, 18.0),
            cv.theme.ink,
            Align::Left,
        );
        let lines = [
            format!("Feels like {:.0}°", temp(st, f.feels)),
            format!("Wind {wind}"),
            format!("Humidity {:.0}%", f.humidity),
        ];
        for (i, l) in lines.iter().enumerate() {
            cv.text(
                l,
                &text::CAPTION,
                Rect::new(now_r.x, now_r.y + 66.0 + i as f32 * 15.0, now_r.w, 15.0),
                cv.theme.mist,
                Align::Left,
            );
        }
        if let Some(p) = f.hours.first().and_then(|h| h.precip_prob) {
            cv.text(
                &format!("Rain chance {p:.0}%"),
                &text::CAPTION,
                Rect::new(now_r.x, now_r.y + 111.0, now_r.w, 15.0),
                cv.theme.mist,
                Align::Left,
            );
        }
    });
    // 12-hour strip
    let strip = Rect::new(rest.x, rest.y, rest.w, 58.0);
    row(cv, rv, 1, strip, |cv| {
        let n = f.hours.len().clamp(1, 12);
        let cw = strip.w / n as f32;
        for (i, h) in f.hours.iter().take(12).enumerate() {
            let x = strip.x + i as f32 * cw;
            let lt = unix_to_local(h.time);
            cv.text(
                &format!("{}", lt.hour),
                &text::CAPTION.size(10.5),
                Rect::new(x, strip.y, cw, 14.0),
                FAINT,
                Align::Center,
            );
            cv.icon(
                icon_for(&h.symbol),
                x + cw / 2.0,
                strip.y + 26.0,
                15.0,
                cv.theme.ink,
            );
            cv.text(
                &format!("{:.0}°", temp(st, h.temp)),
                &text::CAPTION,
                Rect::new(x, strip.y + 38.0, cw, 16.0),
                cv.theme.ink,
                Align::Center,
            );
        }
    });
    let days = Rect::new(rest.x, strip.bottom() + 6.0, rest.w, rest.h - 64.0);
    row(cv, rv, 2, days, |cv| {
        let n = f.days.len().clamp(1, 7);
        let cw = days.w / n as f32;
        for (i, d) in f.days.iter().take(7).enumerate() {
            let x = days.x + i as f32 * cw;
            let wd = crate::util::weekday_of(d.day) as usize;
            cv.text(
                if i == 0 { "Today" } else { &WEEKDAYS[wd][..3] },
                &text::CAPTION.size(10.5),
                Rect::new(x, days.y, cw, 14.0),
                cv.theme.mist,
                Align::Center,
            );
            cv.icon(
                icon_for(&d.symbol),
                x + cw / 2.0,
                days.y + 26.0,
                15.0,
                cv.theme.ink,
            );
            cv.text(
                &format!("{:.0}°/{:.0}°", temp(st, d.max), temp(st, d.min)),
                &text::CAPTION.size(10.5),
                Rect::new(x, days.y + 38.0, cw, 14.0),
                cv.theme.ink,
                Align::Center,
            );
        }
        cv.button(
            Icon::Reset,
            days.right() - 10.0,
            days.bottom() - 30.0,
            22.0,
            Action::WeatherRefresh,
            "Refresh weather",
            cv.theme.mist,
        );
        cv.text(
            "Weather data: MET Norway",
            &text::CAPTION.size(9.5),
            Rect::new(days.x, days.bottom() - 12.0, days.w, 12.0),
            FAINT,
            Align::Right,
        );
    });
}

pub fn calendar(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    let body = header(cv, area, Icon::Calendar, "Agenda", "");
    row(cv, rv, 0, area, |cv| {
        cv.button(
            Icon::Reset,
            area.right() - 14.0,
            area.y + 10.0,
            26.0,
            Action::CalendarRefresh,
            "Refresh calendars",
            cv.theme.mist,
        );
    });
    let now = unix_secs();
    let h24 = st.clock_24h();
    // merge events with dated to-dos
    let mut items: Vec<(i64, String, String, Option<u64>, u32, bool)> =
        st.f.calendar
            .events
            .iter()
            .filter(|e| e.end > now)
            .map(|e| {
                (
                    e.start,
                    e.title.clone(),
                    e.calendar.clone(),
                    e.join_url.as_ref().map(|_| crate::util::fnv1a(&e.id)),
                    e.color,
                    e.all_day,
                )
            })
            .collect();
    for t in st.f.todo.tasks.iter().filter(|t| !t.done) {
        if let Some(d) = t.due {
            if d > now - 3600 && d < now + 8 * 86400 {
                items.push((d, t.title.clone(), "To-do".into(), None, 0x8C94A8, false));
            }
        }
    }
    items.sort_by_key(|i| i.0);
    if items.is_empty() {
        let msg =
            st.f.calendar
                .error
                .clone()
                .unwrap_or_else(|| "No upcoming events. Connect a calendar in Settings.".into());
        row(cv, rv, 0, body, |cv| {
            cv.text(&msg, &text::BODY, body, cv.theme.mist, Align::Center);
        });
        return;
    }
    let scroll = st.input.scroll_value(PageId::Calendar);
    cv.push_clip(body);
    let mut y = body.y - scroll;
    let mut last_day = -1;
    for (i, (start, title, cal, join, color, all_day)) in items.iter().enumerate() {
        let lt = unix_to_local(*start);
        if lt.day_number() != last_day {
            last_day = lt.day_number();
            let today = local_now().day_number();
            let label = if last_day == today {
                "Today".to_string()
            } else if last_day == today + 1 {
                "Tomorrow".into()
            } else {
                format!(
                    "{}, {} {}",
                    WEEKDAYS[lt.weekday as usize],
                    crate::util::MONTHS[lt.month as usize - 1],
                    lt.day
                )
            };
            cv.text(
                &label,
                &text::MICRO,
                Rect::new(body.x, y, body.w, 18.0),
                FAINT,
                Align::Left,
            );
            y += 20.0;
        }
        let r = Rect::new(body.x, y, body.w, 34.0);
        y += 36.0;
        if !cv.visible(r) {
            continue;
        }
        let key = crate::util::fnv1a(title) ^ (*start as u64);
        row(cv, rv, 1 + i.min(6), r, |cv| {
            super::hover_card(cv, r, Action::Row(2, key), 10.0);
            let c = Color::rgb(*color);
            // a soft colour bar instead of a bare dot; it glows for what's happening now
            let live = *start <= now && !*all_day;
            cv.rrect(Rect::new(r.x + 8.0, r.y + 7.0, 4.0, r.h - 14.0), 2.0, c);
            if live {
                let t = crate::util::unix_ms() as f32 / 1000.0;
                let a = 0.25 + 0.2 * (t * 2.4).sin();
                cv.rrect(
                    Rect::new(r.x + 6.0, r.y + 5.0, 8.0, r.h - 10.0),
                    4.0,
                    c.with_alpha(a),
                );
            }
            let time = if *all_day {
                "All day".into()
            } else {
                fmt_hm(lt.minutes_of_day(), h24)
            };
            cv.text(
                &time,
                &text::CAPTION_TNUM,
                Rect::new(r.x + 26.0, r.y, 62.0, r.h),
                cv.theme.mist,
                Align::Left,
            );
            cv.text(
                title,
                &text::BODY_MED,
                Rect::new(r.x + 90.0, r.y, r.w - 190.0, r.h),
                cv.theme.ink,
                Align::Left,
            );
            if let Some(h) = join {
                cv.pill_button(
                    "Join",
                    Rect::new(r.right() - 60.0, r.cy() - 11.0, 52.0, 22.0),
                    Action::CalendarJoin(*h),
                    true,
                );
            } else {
                cv.text(
                    cal,
                    &text::CAPTION,
                    Rect::new(r.right() - 100.0, r.y, 92.0, r.h),
                    FAINT,
                    Align::Right,
                );
            }
        });
    }
    cv.pop_clip();
    clamp_scroll(st, PageId::Calendar, y + scroll - body.bottom());
}

pub fn notifications(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    let body = header(cv, area, Icon::Bell, "Notifications", "");
    row(cv, rv, 0, area, |cv| {
        if !st.f.notifs.items.is_empty() {
            cv.button(
                Icon::Trash,
                area.right() - 14.0,
                area.y + 10.0,
                26.0,
                Action::NotifClear,
                "Clear history",
                cv.theme.mist,
            );
        }
    });
    if st.f.notifs.access == Some(crate::services::notifications::Access::NoIdentity)
        && st.f.notifs.items.is_empty()
    {
        row(cv, rv, 1, body, |cv| {
            cv.text_top("Mirroring Windows notifications needs the Microsoft Store build or the installer's notification component (a sparse package). Everything else works without it.", &text::BODY.lines(4), body.x, body.y + 4.0, body.w, cv.theme.mist, Align::Left);
            cv.pill_button(
                "Notification settings",
                Rect::new(body.x, body.y + 80.0, 170.0, 28.0),
                Action::NotifEnable,
                false,
            );
        });
        return;
    }
    let items = st.f.notifs.items.clone();
    if items.is_empty() {
        row(cv, rv, 1, body, |cv| {
            cv.text(
                "No notifications yet",
                &text::BODY,
                body,
                cv.theme.mist,
                Align::Center,
            );
        });
        return;
    }
    let scroll = st.input.scroll_value(PageId::Notifications);
    let h24 = st.clock_24h();
    cv.push_clip(body);
    let mut y = body.y - scroll;
    for (i, n) in items.iter().enumerate() {
        use crate::features::ListKind;
        let p = st.f.presence(ListKind::Notif, n.id);
        let leaving = st.f.is_leaving(ListKind::Notif, n.id);
        let (a, dx, dy, hf) = super::row_presence(p, leaving, i);
        let r = Rect::new(body.x + dx, y + dy, body.w, 48.0);
        y += 52.0 * hf;
        if !cv.visible(r) || a <= 0.01 {
            continue;
        }
        cv.push_alpha(a);
        row(cv, rv, 1 + i.min(6), r, |cv| {
            super::hover_card(cv, r, Action::Row(1, n.id), 12.0);
            // app avatar: a tinted circle with the app's initial
            let hue = [
                TINT_WEATHER,
                TINT_TODO,
                TINT_MUSIC,
                TINT_TIMER,
                TINT_PRAYER,
                TINT_WATER,
            ];
            let c = hue[(crate::util::fnv1a(&n.app) % hue.len() as u64) as usize];
            let ax = r.x + 22.0;
            cv.circle(
                ax,
                r.cy(),
                13.0,
                c.with_alpha(if n.suppressed { 0.12 } else { 0.22 }),
            );
            let initial: String = n
                .app
                .chars()
                .next()
                .map(|ch| ch.to_uppercase().collect())
                .unwrap_or_default();
            cv.text(
                &initial,
                &text::BODY_MED,
                Rect::new(ax - 13.0, r.cy() - 13.0, 26.0, 26.0),
                c,
                Align::Center,
            );
            let r = Rect::new(r.x + 30.0, r.y, r.w - 30.0, r.h);
            cv.text(
                &n.app,
                &text::MICRO,
                Rect::new(r.x + 12.0, r.y + 4.0, r.w - 90.0, 14.0),
                if n.suppressed {
                    FAINT
                } else {
                    cv.theme.accent[0]
                },
                Align::Left,
            );
            cv.text(
                &fmt_hm(unix_to_local(n.time).minutes_of_day(), h24),
                &text::CAPTION,
                Rect::new(r.right() - 80.0, r.y + 4.0, 70.0, 14.0),
                FAINT,
                Align::Right,
            );
            let body_text = if st.sup.sharing && st.settings.quiet.hide_when_sharing {
                "Hidden while sharing".to_string()
            } else {
                format!("{} — {}", n.title, n.body)
            };
            cv.text(
                &body_text,
                &text::BODY,
                Rect::new(r.x + 12.0, r.y + 20.0, r.w - 24.0, 22.0),
                cv.theme.ink,
                Align::Left,
            );
        });
        cv.pop_alpha();
    }
    cv.pop_clip();
    clamp_scroll(st, PageId::Notifications, y + scroll - body.bottom());
}

pub fn privacy(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    let body = header(cv, area, Icon::Shield, "Privacy", "Last 7 days");
    let active = st.f.privacy.uses.clone();
    let mut y = body.y;
    if !active.is_empty() {
        for u in &active {
            let r = Rect::new(body.x, y, body.w, 28.0);
            row(cv, rv, 0, r, |cv| {
                let (c, ic) = match u.cap {
                    crate::services::privacy::Cap::Mic => (PRIVACY_MIC, Icon::Mic),
                    crate::services::privacy::Cap::Camera => (PRIVACY_CAM, Icon::Camera),
                    crate::services::privacy::Cap::Location => (PRIVACY_LOC, Icon::Location),
                };
                cv.circle(r.x + 6.0, r.cy(), 4.0, c);
                cv.icon(ic, r.x + 24.0, r.cy(), 14.0, c);
                cv.text(
                    &format!("{} is using your {}", u.app, u.cap.label().to_lowercase()),
                    &text::BODY_MED,
                    Rect::new(r.x + 38.0, r.y, r.w - 40.0, r.h),
                    cv.theme.ink,
                    Align::Left,
                );
            });
            y += 30.0;
        }
        y += 6.0;
    }
    let log = st.f.state.privacy_log.clone();
    let list = Rect::new(body.x, y, body.w, body.bottom() - y);
    let scroll = st.input.scroll_value(PageId::Privacy);
    let h24 = st.clock_24h();
    cv.push_clip(list);
    for (i, l) in log.iter().rev().enumerate() {
        let r = Rect::new(list.x, list.y + i as f32 * 26.0 - scroll, list.w, 24.0);
        if !cv.visible(r) {
            continue;
        }
        let lt = unix_to_local(l.start);
        let span = match l.stop {
            Some(s) => format!(
                "{}–{}",
                fmt_hm(lt.minutes_of_day(), h24),
                fmt_hm(unix_to_local(s).minutes_of_day(), h24)
            ),
            None => format!("{} – now", fmt_hm(lt.minutes_of_day(), h24)),
        };
        row(cv, rv, 1 + i.min(6), r, |_| {});
        super::hover_card(cv, r, Action::Row(3, i as u64), 8.0);
        let cc = match l.cap {
            crate::services::privacy::Cap::Mic => PRIVACY_MIC,
            crate::services::privacy::Cap::Camera => PRIVACY_CAM,
            crate::services::privacy::Cap::Location => PRIVACY_LOC,
        };
        cv.circle(r.x + 10.0, r.cy(), 3.5, cc);
        let r = Rect::new(r.x + 20.0, r.y, r.w - 26.0, r.h);
        cv.text(
            &format!("{} · {}", l.app, l.cap.label()),
            &text::BODY,
            Rect::new(r.x, r.y, r.w * 0.6, r.h),
            cv.theme.ink,
            Align::Left,
        );
        cv.text(
            &format!(
                "{} {} · {span}",
                &crate::util::MONTHS[lt.month as usize - 1][..3],
                lt.day
            ),
            &text::CAPTION,
            Rect::new(r.x + r.w * 0.4, r.y, r.w * 0.6, r.h),
            cv.theme.mist,
            Align::Right,
        );
    }
    if log.is_empty() && active.is_empty() {
        cv.text(
            "No app has used your mic, camera or location recently",
            &text::BODY,
            list,
            cv.theme.mist,
            Align::Center,
        );
    }
    cv.pop_clip();
    clamp_scroll(st, PageId::Privacy, log.len() as f32 * 26.0 - list.h);
}

pub fn notes(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    let body = header(cv, area, Icon::Note, "Notes", "");
    let notes = st.f.notes.notes.clone();
    if st.f.notes.current.is_none() {
        if let Some(n) = notes.first() {
            st.f.notes.current = Some(n.id);
            st.input.text(InputId::Note).set(&n.text);
        }
    }
    let cur = st.f.notes.current;
    row(cv, rv, 0, area, |cv| {
        let y = area.y + 10.0;
        cv.button(
            Icon::Plus,
            area.right() - 14.0,
            y,
            26.0,
            Action::NoteNew,
            "New note",
            cv.theme.ink,
        );
        if let Some(id) = cur {
            let pinned = notes.iter().any(|n| n.id == id && n.pinned);
            cv.button(
                Icon::Pin,
                area.right() - 44.0,
                y,
                26.0,
                Action::NotePin(id),
                "Pin to notch",
                if pinned {
                    cv.theme.accent[0]
                } else {
                    cv.theme.mist
                },
            );
            cv.button(
                Icon::Download,
                area.right() - 74.0,
                y,
                26.0,
                Action::NoteExport(id),
                "Export as Markdown",
                cv.theme.mist,
            );
            cv.button(
                Icon::Trash,
                area.right() - 104.0,
                y,
                26.0,
                Action::NoteDelete(id),
                "Delete note",
                cv.theme.mist,
            );
        }
    });
    let (list, editor) = body.split_left(120.0);
    use crate::features::ListKind;
    // the selection pill glides between rows on the snappy spring
    let mut yy = 0.0;
    let mut sel_y = None;
    for n in notes.iter().take(8) {
        if Some(n.id) == cur {
            sel_y = Some(yy);
        }
        yy += 30.0 * st.f.presence(ListKind::Note, n.id).clamp(0.0, 1.0);
    }
    if let Some(t) = sel_y {
        if (st.f.note_sel.target - t).abs() > 0.1 {
            st.f.note_sel.set(t);
        }
        let sy = list.y + st.f.note_sel.value;
        row(cv, rv, 1, list, |cv| {
            cv.sq(
                Rect::new(list.x, sy, list.w - 8.0, 28.0),
                9.0,
                cv.theme.surface_raised,
            );
        });
    }
    let mut y = list.y;
    for (i, n) in notes.iter().take(8).enumerate() {
        let p = st.f.presence(ListKind::Note, n.id);
        let leaving = st.f.is_leaving(ListKind::Note, n.id);
        let (a, dx, dy, hf) = super::row_presence(p, leaving, i);
        let r = Rect::new(list.x + dx, y + dy, list.w - 8.0, 28.0);
        y += 30.0 * hf;
        if a <= 0.01 {
            continue;
        }
        cv.push_alpha(a);
        row(cv, rv, 1 + i.min(5), r, |cv| {
            let (hv, _) = cv.anim(&Action::NoteSelect(n.id), cv.hovered(r), false);
            if Some(n.id) != cur && hv > 0.01 {
                cv.sq(r, 9.0, cv.theme.surface.with_alpha(hv.min(1.0)));
            }
            cv.text(
                &crate::features::store::Notes::first_line(n),
                &text::CAPTION,
                r.inset_xy(10.0, 0.0),
                cv.theme.ink,
                Align::Left,
            );
            if !leaving {
                cv.zone_named(r, Action::NoteSelect(n.id), true, "Note", 9.0);
            }
        });
        cv.pop_alpha();
    }
    row(cv, rv, 2, editor, |cv| {
        text_input(
            st,
            cv,
            InputId::Note,
            editor,
            "Write a note… lines starting with [ ] become checkboxes",
            None,
        );
    });
}

pub fn meeting(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    let Some(m) = st.f.meeting.clone() else {
        row(cv, rv, 0, area, |cv| {
            cv.text(
                "No call in progress",
                &text::BODY,
                area,
                cv.theme.mist,
                Align::Center,
            );
        });
        return;
    };
    let cam =
        st.f.privacy
            .uses
            .iter()
            .any(|u| u.cap == crate::services::privacy::Cap::Camera);
    row(cv, rv, 0, area, |cv| {
        cv.circle(area.x + 24.0, area.y + 24.0, 20.0, GOOD);
        cv.icon(Icon::Phone, area.x + 24.0, area.y + 24.0, 20.0, NOTCH);
        cv.text(
            &m.app,
            &text::TITLE,
            Rect::new(area.x + 56.0, area.y + 4.0, area.w - 60.0, 20.0),
            cv.theme.ink,
            Align::Left,
        );
        cv.text(
            &fmt_mmss(m.started.elapsed().as_secs() as i64),
            &text::CAPTION_TNUM,
            Rect::new(area.x + 56.0, area.y + 24.0, 100.0, 16.0),
            GOOD,
            Align::Left,
        );
    });
    row(cv, rv, 1, area, |cv| {
        let y = area.bottom() - 30.0;
        let muted = st.sys.mic_muted;
        cv.button(
            if muted { Icon::MicOff } else { Icon::Mic },
            area.cx() - 70.0,
            y,
            44.0,
            Action::MeetingMute,
            if muted { "Unmute" } else { "Mute" },
            if muted { CRITICAL } else { cv.theme.ink },
        );
        cv.button(
            Icon::Camera,
            area.cx(),
            y,
            44.0,
            Action::MeetingCamera,
            "Camera",
            if cam { PRIVACY_CAM } else { cv.theme.mist },
        );
        cv.button(
            Icon::Phone,
            area.cx() + 70.0,
            y,
            44.0,
            Action::MeetingLeave,
            "Leave",
            CRITICAL,
        );
    });
}

pub fn downloads(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    let body = header(cv, area, Icon::Download, "Downloads", "");
    let items = st.f.downloads.items.clone();
    if items.is_empty() {
        row(cv, rv, 0, body, |cv| {
            cv.text(
                "Downloads in progress appear here",
                &text::BODY,
                body,
                cv.theme.mist,
                Align::Center,
            );
        });
        return;
    }
    let t = st.started.elapsed().as_secs_f32();
    for (i, d) in items.iter().take(6).enumerate() {
        let r = Rect::new(body.x, body.y + i as f32 * 44.0, body.w, 40.0);
        let frac = match (d.done, d.total) {
            (true, _) => Some(st.f.smooth_progress(&format!("dl:{}", d.id), 1.0)),
            (false, Some(tot)) if tot > 0 => {
                Some(st.f.smooth_progress(&format!("dl:{}", d.id), d.bytes as f32 / tot as f32))
            }
            _ => None,
        };
        row(cv, rv, i, r, |cv| {
            super::hover_card(cv, r, Action::Row(4, d.id), 12.0);
            let bar = Rect::new(r.x + 36.0, r.bottom() - 5.0, r.w - 150.0, 2.5);
            if !d.done || frac.is_some_and(|f| f < 0.999) {
                cv.rrect(bar, 1.25, TRACK);
                match frac {
                    Some(f) => cv.rrect(
                        Rect::new(bar.x, bar.y, bar.w * f, bar.h),
                        1.25,
                        cv.theme.accent[0],
                    ),
                    None => {
                        // unknown size: a soft shimmer sweeps along the track
                        let seg = bar.w * 0.28;
                        let x = bar.x + (t * 0.8).fract() * (bar.w + seg) - seg;
                        let x0 = x.max(bar.x);
                        let x1 = (x + seg).min(bar.right());
                        if x1 > x0 {
                            cv.rrect(
                                Rect::new(x0, bar.y, x1 - x0, bar.h),
                                1.25,
                                cv.theme.accent[0],
                            );
                        }
                    }
                }
            }
            cv.icon(
                if d.done {
                    Icon::CheckCircle
                } else {
                    Icon::Download
                },
                r.x + 18.0,
                r.cy(),
                16.0,
                if d.done { GOOD } else { cv.theme.accent[0] },
            );
            cv.text(
                &d.name,
                &text::BODY_MED,
                Rect::new(r.x + 36.0, r.y + 2.0, r.w - 150.0, 20.0),
                cv.theme.ink,
                Align::Left,
            );
            let sub = if d.done {
                super::productivity::fmt_size(d.bytes)
            } else {
                format!(
                    "{} · {}",
                    super::productivity::fmt_size(d.bytes),
                    crate::services::sysmon::fmt_rate(d.speed)
                )
            };
            cv.text(
                &sub,
                &text::CAPTION,
                Rect::new(r.x + 36.0, r.y + 20.0, r.w - 150.0, 16.0),
                cv.theme.mist,
                Align::Left,
            );
            if d.done {
                cv.button(
                    Icon::ExternalLink,
                    r.right() - 70.0,
                    r.cy(),
                    26.0,
                    Action::DownloadOpen(d.id),
                    "Open",
                    cv.theme.ink,
                );
                cv.button(
                    Icon::Folder,
                    r.right() - 42.0,
                    r.cy(),
                    26.0,
                    Action::DownloadReveal(d.id),
                    "Show in folder",
                    cv.theme.ink,
                );
                cv.button(
                    Icon::Shelf,
                    r.right() - 14.0,
                    r.cy(),
                    26.0,
                    Action::DownloadShelf(d.id),
                    "Add to shelf",
                    cv.theme.ink,
                );
            }
        });
    }
}

pub fn live(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    let key = st
        .act
        .ordered()
        .into_iter()
        .find(|a| a.key.starts_with("live:"))
        .map(|a| a.key.clone());
    let Some(key) = key else {
        row(cv, rv, 0, area, |cv| {
            cv.text_top("Apps and scripts can post here with the Live Activities API:\nnotchly live start --id build --title Build --progress 0.4", &text::CAPTION.lines(4), area.x, area.y + 20.0, area.w, cv.theme.mist, Align::Center);
        });
        return;
    };
    let id = key.trim_start_matches("live:").to_string();
    let target =
        st.f.live
            .items
            .iter()
            .find(|i| i.id == id)
            .and_then(|i| i.progress);
    let smooth = target.map(|p| st.f.smooth_progress(&key, p));
    let Some(it) = st.f.live.items.iter_mut().find(|i| i.id == id) else {
        return;
    };
    let color = it.color.unwrap_or(cv.theme.accent[0]);
    let (title, sub, prog, actions, app, fin) = (
        it.title.clone(),
        it.subtitle.clone(),
        it.progress,
        it.actions.clone(),
        it.app.clone(),
        it.final_text.clone(),
    );
    row(cv, rv, 0, area, |cv| {
        match (&mut it.image, it.icon) {
            (Some(img), _) => cv.image(img, Rect::new(area.x, area.y, 40.0, 40.0), 10.0),
            (None, Some(ic)) => cv.icon(ic, area.x + 20.0, area.y + 20.0, 28.0, color),
            _ => cv.circle(area.x + 20.0, area.y + 20.0, 8.0, color),
        }
        cv.text(
            &title,
            &text::TITLE,
            Rect::new(area.x + 52.0, area.y, area.w - 60.0, 20.0),
            cv.theme.ink,
            Align::Left,
        );
        cv.text(
            &fin.clone().unwrap_or(sub.clone()),
            &text::BODY,
            Rect::new(area.x + 52.0, area.y + 20.0, area.w - 60.0, 18.0),
            cv.theme.mist,
            Align::Left,
        );
        cv.text(
            &app,
            &text::CAPTION,
            Rect::new(area.x, area.y + 46.0, area.w, 14.0),
            FAINT,
            Align::Left,
        );
    });
    row(cv, rv, 1, area, |cv| {
        if let (Some(p), Some(sp)) = (prog, smooth) {
            cv.progress(Rect::new(area.x, area.y + 70.0, area.w, 6.0), sp, true);
            cv.text(
                &format!("{:.0}%", p * 100.0),
                &text::CAPTION_TNUM,
                Rect::new(area.x, area.y + 78.0, area.w, 14.0),
                cv.theme.mist,
                Align::Right,
            );
        }
        let mut x = area.x;
        for (aid, label) in &actions {
            let w = cv.measure(label, &text::BODY_MED) + 26.0;
            cv.pill_button(
                label,
                Rect::new(x, area.bottom() - 28.0, w, 26.0),
                Action::LiveButton(id.clone(), aid.clone()),
                x == area.x,
            );
            x += w + 6.0;
        }
    });
}

pub fn onboarding(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    let step = st.onboarding_step;
    let toggle = |cv: &mut Canvas, r: Rect, label: &str, on: bool, a: Action| {
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
            if on { Icon::CheckCircle } else { Icon::Plus },
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
            label,
            &text::CAPTION.weight(600),
            Rect::new(r.x + 30.0, r.y, r.w - 34.0, r.h),
            cv.theme.ink,
            Align::Left,
        );
        cv.zone_named(r, a, true, label, 12.0);
    };
    match step {
        0 => {
            row(cv, rv, 0, area, |cv| {
                // the mascot waves
                let t = st.started.elapsed().as_secs_f32();
                let cx = area.x + 40.0;
                let cy = area.y + 34.0;
                cv.rrect(
                    Rect::centered(cx, cy, 64.0, 34.0),
                    17.0,
                    NOTCH.lerp(SURFACE_RAISED, 0.6),
                );
                for dx in [-9.0, 9.0] {
                    cv.rrect(
                        Rect::centered(cx + dx + (t * 3.0).sin() * 1.5, cy, EYE_W, EYE_H),
                        4.0,
                        cv.theme.ink,
                    );
                }
                cv.text(
                    &crate::i18n::tr("Hi, I'm Notchly"),
                    &text::TITLE.size(18.0),
                    Rect::new(area.x + 84.0, area.y + 14.0, area.w - 90.0, 24.0),
                    cv.theme.ink,
                    Align::Left,
                );
                cv.text(
                    &crate::i18n::tr("How should I open?"),
                    &text::BODY,
                    Rect::new(area.x + 84.0, area.y + 38.0, area.w - 90.0, 18.0),
                    cv.theme.mist,
                    Align::Left,
                );
            });
            row(cv, rv, 1, area, |cv| {
                let hover = st.settings.general.open_mode == crate::settings::OpenMode::Hover;
                let w = (area.w - 8.0) / 2.0;
                toggle(
                    cv,
                    Rect::new(area.x, area.y + 74.0, w, 34.0),
                    &crate::i18n::tr("Open on hover"),
                    hover,
                    Action::OnboardingToggle(0),
                );
                toggle(
                    cv,
                    Rect::new(area.x + w + 8.0, area.y + 74.0, w, 34.0),
                    &crate::i18n::tr("Open on click"),
                    !hover,
                    Action::OnboardingToggle(1),
                );
                cv.pill_button(
                    "Next",
                    Rect::new(area.right() - 90.0, area.bottom() - 30.0, 90.0, 28.0),
                    Action::Onboarding(1),
                    true,
                );
                cv.pill_button(
                    "Skip",
                    Rect::new(area.x, area.bottom() - 30.0, 70.0, 28.0),
                    Action::Onboarding(3),
                    false,
                );
            });
        }
        1 => {
            row(cv, rv, 0, area, |cv| {
                cv.text(
                    &crate::i18n::tr("Pick your features"),
                    &text::TITLE,
                    Rect::new(area.x, area.y, area.w, 20.0),
                    cv.theme.ink,
                    Align::Left,
                );
                let s = &st.settings;
                let items = [
                    ("Music & lyrics", s.music.enabled, 2u8),
                    ("System pop-ups", s.popups.volume, 3),
                    ("Water reminder", s.productivity.water, 4),
                    ("Prayer times", s.productivity.prayer, 5),
                    ("Clipboard history", s.productivity.clipboard, 6),
                    ("Focus timer", true, 99),
                ];
                let w = (area.w - 16.0) / 3.0;
                for (i, (label, on, id)) in items.iter().enumerate() {
                    let r = Rect::new(
                        area.x + (i % 3) as f32 * (w + 8.0),
                        area.y + 28.0 + (i / 3) as f32 * 40.0,
                        w,
                        34.0,
                    );
                    toggle(
                        cv,
                        r,
                        label,
                        *on,
                        if *id == 99 {
                            Action::None
                        } else {
                            Action::OnboardingToggle(*id)
                        },
                    );
                }
                cv.pill_button(
                    "Next",
                    Rect::new(area.right() - 90.0, area.bottom() - 30.0, 90.0, 28.0),
                    Action::Onboarding(2),
                    true,
                );
            });
        }
        _ => {
            row(cv, rv, 0, area, |cv| {
                cv.text(
                    &crate::i18n::tr("Almost done"),
                    &text::TITLE,
                    Rect::new(area.x, area.y, area.w, 20.0),
                    cv.theme.ink,
                    Align::Left,
                );
                toggle(
                    cv,
                    Rect::new(area.x, area.y + 30.0, 220.0, 34.0),
                    "Start with Windows",
                    st.settings.general.start_with_windows,
                    Action::OnboardingToggle(7),
                );
                cv.text(
                    "Try it: play a song or press a volume key.",
                    &text::BODY,
                    Rect::new(area.x, area.y + 76.0, area.w, 18.0),
                    cv.theme.mist,
                    Align::Left,
                );
                cv.pill_button(
                    "Done",
                    Rect::new(area.right() - 90.0, area.bottom() - 30.0, 90.0, 28.0),
                    Action::Onboarding(3),
                    true,
                );
            });
        }
    }
}

pub const ANNOTATE_TOOLS: &[(Icon, &str)] = &[
    (Icon::Arrow, "Arrow"),
    (Icon::Square, "Rectangle"),
    (Icon::Highlighter, "Highlighter"),
    (Icon::Eye, "Blur"),
    (Icon::Text, "Text"),
    (Icon::Crop, "Crop"),
];

/// Screenshot editor: canvas area with shapes, tool bar, undo, copy/save.
pub fn annotate(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    let Some(px) = st.f.annotate.image.clone() else {
        row(cv, rv, 0, area, |cv| {
            cv.text(
                "Press Ctrl+Alt+S to capture a screenshot",
                &text::BODY,
                area,
                cv.theme.mist,
                Align::Center,
            );
        });
        return;
    };
    let bar = Rect::new(area.x, area.bottom() - 30.0, area.w, 30.0);
    let img_area = Rect::new(area.x, area.y, area.w, area.h - 36.0);
    let scale = (img_area.w / px.w as f32).min(img_area.h / px.h as f32);
    let (w, h) = (px.w as f32 * scale, px.h as f32 * scale);
    let ir = Rect::new(img_area.cx() - w / 2.0, img_area.cy() - h / 2.0, w, h);
    st.f.annotate.img_rect = ir;
    if st.f.annotate.bitmap.is_none() {
        st.f.annotate.bitmap = Some(crate::render::image::CachedBitmap::new(px.clone()));
    }
    if let Some(b) = st.f.annotate.bitmap.as_mut() {
        cv.image(b, ir, 6.0);
    }
    let shapes: Vec<crate::features::AnnoShape> =
        st.f.annotate
            .shapes
            .iter()
            .chain(st.f.annotate.drawing.iter())
            .cloned()
            .collect();
    for s in &shapes {
        let a = (ir.x + s.a.0 * w, ir.y + s.a.1 * h);
        let b = (ir.x + s.b.0 * w, ir.y + s.b.1 * h);
        let r = Rect::new(
            a.0.min(b.0),
            a.1.min(b.1),
            (b.0 - a.0).abs(),
            (b.1 - a.1).abs(),
        );
        match s.tool {
            0 => {
                cv.line(a.0, a.1, b.0, b.1, 2.5, CRITICAL);
                let ang = (b.1 - a.1).atan2(b.0 - a.0);
                for d in [2.6f32, -2.6] {
                    cv.line(
                        b.0,
                        b.1,
                        b.0 - 10.0 * (ang + d * 0.2).cos(),
                        b.1 - 10.0 * (ang + d * 0.2).sin(),
                        2.5,
                        CRITICAL,
                    );
                }
            }
            1 => cv.stroke_rrect(r, 3.0, CRITICAL, 2.0),
            2 => cv.rect(r, WARN.with_alpha(0.35)),
            3 => cv.rect(r, NOTCH.lerp(SURFACE_RAISED, 0.5)),
            4 => {
                cv.text(
                    if s.text.is_empty() { "Text" } else { &s.text },
                    &text::BODY_MED,
                    Rect::new(a.0, a.1 - 10.0, 200.0, 20.0),
                    CRITICAL,
                    Align::Left,
                );
            }
            _ => cv.stroke_rrect(r, 0.0, WHITE, 1.5),
        }
    }
    cv.zone(ir, Action::ScrollTo(0));
    row(cv, rv, 1, bar, |cv| {
        let mut x = bar.x + 14.0;
        for (i, (ic, name)) in ANNOTATE_TOOLS.iter().enumerate() {
            let on = st.f.annotate.tool == i as u8;
            cv.button(
                *ic,
                x,
                bar.cy(),
                28.0,
                Action::AnnotateTool(i as u8),
                name,
                if on {
                    cv.theme.accent[0]
                } else {
                    cv.theme.mist
                },
            );
            x += 32.0;
        }
        cv.button(
            Icon::Undo,
            x + 8.0,
            bar.cy(),
            28.0,
            Action::AnnotateUndo,
            "Undo",
            cv.theme.mist,
        );
        cv.pill_button(
            "Save",
            Rect::new(bar.right() - 150.0, bar.y + 2.0, 66.0, 26.0),
            Action::AnnotateSave,
            false,
        );
        cv.pill_button(
            "Copy",
            Rect::new(bar.right() - 78.0, bar.y + 2.0, 78.0, 26.0),
            Action::AnnotateDone,
            true,
        );
    });
    st.f.annotate.crop =
        st.f.annotate
            .shapes
            .iter()
            .rev()
            .find(|s| s.tool == 5)
            .map(|s| {
                (
                    s.a.0.min(s.b.0),
                    s.a.1.min(s.b.1),
                    s.a.0.max(s.b.0),
                    s.a.1.max(s.b.1),
                )
            });
}

/// Renders annotations into the screenshot pixels (CPU) for copy/save.
pub fn flatten_annotations(st: &State) -> Option<crate::render::image::Pixels> {
    let px = st.f.annotate.image.as_ref()?;
    let mut out = (**px).clone();
    let (w, h) = (out.w as f32, out.h as f32);
    let put = |x: i32, y: i32, c: [u8; 3], a: f32, out: &mut crate::render::image::Pixels| {
        if x < 0 || y < 0 || x >= out.w as i32 || y >= out.h as i32 {
            return;
        }
        let o = ((y as u32 * out.w + x as u32) * 4) as usize;
        for k in 0..3 {
            out.data[o + k] = (out.data[o + k] as f32 * (1.0 - a) + c[2 - k] as f32 * a) as u8;
        }
    };
    for s in &st.f.annotate.shapes {
        let (ax, ay, bx, by) = (s.a.0 * w, s.a.1 * h, s.b.0 * w, s.b.1 * h);
        let (x0, y0, x1, y1) = (
            ax.min(bx) as i32,
            ay.min(by) as i32,
            ax.max(bx) as i32,
            ay.max(by) as i32,
        );
        match s.tool {
            0 => {
                let n = ((bx - ax).hypot(by - ay)) as i32 + 1;
                for i in 0..=n {
                    let t = i as f32 / n as f32;
                    let (x, y) = (ax + (bx - ax) * t, ay + (by - ay) * t);
                    for dx in -1..=1 {
                        for dy in -1..=1 {
                            put(x as i32 + dx, y as i32 + dy, [255, 70, 70], 1.0, &mut out);
                        }
                    }
                }
            }
            1 => {
                for x in x0..=x1 {
                    for t in 0..2 {
                        put(x, y0 + t, [255, 70, 70], 1.0, &mut out);
                        put(x, y1 - t, [255, 70, 70], 1.0, &mut out);
                    }
                }
                for y in y0..=y1 {
                    for t in 0..2 {
                        put(x0 + t, y, [255, 70, 70], 1.0, &mut out);
                        put(x1 - t, y, [255, 70, 70], 1.0, &mut out);
                    }
                }
            }
            2 => {
                for y in y0..y1 {
                    for x in x0..x1 {
                        put(x, y, [255, 176, 32], 0.35, &mut out);
                    }
                }
            }
            3 => {
                // pixelate in 12 px blocks
                let b = 12;
                let mut by_ = y0;
                while by_ < y1 {
                    let mut bx_ = x0;
                    while bx_ < x1 {
                        let (cx, cy) = (
                            (bx_ + b / 2).min(out.w as i32 - 1).max(0),
                            (by_ + b / 2).min(out.h as i32 - 1).max(0),
                        );
                        let o = ((cy as u32 * out.w + cx as u32) * 4) as usize;
                        let c = [out.data[o + 2], out.data[o + 1], out.data[o]];
                        for y in by_..(by_ + b).min(y1) {
                            for x in bx_..(bx_ + b).min(x1) {
                                put(x, y, c, 1.0, &mut out);
                            }
                        }
                        bx_ += b;
                    }
                    by_ += b;
                }
            }
            _ => {}
        }
    }
    if let Some((cx0, cy0, cx1, cy1)) = st.f.annotate.crop {
        let (x0, y0) = ((cx0 * w) as u32, (cy0 * h) as u32);
        let (x1, y1) = (((cx1 * w) as u32).min(out.w), ((cy1 * h) as u32).min(out.h));
        if x1 > x0 + 4 && y1 > y0 + 4 {
            let mut data = Vec::with_capacity(((x1 - x0) * (y1 - y0) * 4) as usize);
            for y in y0..y1 {
                let o = ((y * out.w + x0) * 4) as usize;
                data.extend_from_slice(&out.data[o..o + ((x1 - x0) * 4) as usize]);
            }
            out = crate::render::image::Pixels {
                w: x1 - x0,
                h: y1 - y0,
                data,
            };
        }
    }
    Some(out)
}

/// Filled area chart with a vertical gradient and a crisp top line.
fn area_chart(cv: &mut Canvas, r: Rect, data: &[f32], max: f32, color: Color) {
    use windows::core::Interface;
    if data.len() < 2 {
        return;
    }
    let max = data.iter().copied().fold(max, f32::max).max(1e-3);
    let pts: Vec<(f32, f32)> = data
        .iter()
        .enumerate()
        .map(|(i, v)| {
            (
                r.x + r.w * i as f32 / (data.len() - 1) as f32,
                r.bottom() - r.h * (v / max).clamp(0.0, 1.0),
            )
        })
        .collect();
    let factory = cv.gfx.factory.clone();
    let mut fill = pts.clone();
    fill.push((r.right(), r.bottom()));
    fill.push((r.x, r.bottom()));
    if let Ok(g) = crate::render::shapes::poly(&factory, &fill, true) {
        if let Some(b) = cv.gradient(
            crate::render::canvas::v2(0.0, r.y),
            crate::render::canvas::v2(0.0, r.bottom()),
            &[(0.0, color.with_alpha(0.38)), (1.0, color.with_alpha(0.0))],
            1.0,
        ) {
            let brush: windows::Win32::Graphics::Direct2D::ID2D1Brush = b.cast().expect("brush");
            unsafe { cv.ctx.FillGeometry(&g, &brush, None) };
        }
    }
    if let Ok(g) = crate::render::shapes::poly(&factory, &pts, false) {
        cv.stroke_geo(&g, color, 1.6);
    }
}

pub fn sysmon(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    let s = st.f.sysmon.last.unwrap_or_default();
    let g =
        st.f.sysmon
            .gauges
            .iter()
            .map(|sp| sp.value)
            .collect::<Vec<f32>>();
    let gauges = [
        ("CPU", g[0], TINT_SYSTEM),
        ("Memory", g[1], TINT_PRAYER),
        ("GPU", g[2], TINT_TODO),
    ];
    let left_w = area.w * 0.56;
    let gw = (left_w - 16.0) / 3.0;
    for (i, (label, v, tint)) in gauges.iter().enumerate() {
        let r = Rect::new(area.x + i as f32 * (gw + 8.0), area.y, gw, area.h - 4.0);
        row(cv, rv, i, r, |cv| {
            cv.sq(r, R_TILE, cv.theme.surface);
            let rad = (gw.min(r.h - 30.0) / 2.0 - 8.0).clamp(14.0, 26.0);
            let (cx, cy) = (r.cx(), r.y + 12.0 + rad);
            cv.stroke_circle(cx, cy, rad, tint.with_alpha(0.16), 5.0);
            if *v > 0.2 {
                if let Ok(arc) = crate::render::shapes::ring_arc(
                    &cv.gfx.factory.clone(),
                    cx,
                    cy,
                    rad,
                    (v / 100.0).clamp(0.0, 1.0),
                ) {
                    cv.stroke_geo(&arc, *tint, 5.0);
                }
            }
            cv.text(
                &format!("{v:.0}%"),
                &text::TITLE.size(14.0).tnum(),
                Rect::new(cx - rad, cy - 10.0, rad * 2.0, 20.0),
                cv.theme.ink,
                Align::Center,
            );
            cv.text(
                label,
                &text::MICRO,
                Rect::new(r.x, r.bottom() - 20.0, r.w, 14.0),
                cv.theme.mist,
                Align::Center,
            );
        });
    }
    let rx = area.x + left_w + 8.0;
    let rw = area.right() - rx;
    let rh = (area.h - 12.0) / 2.0;
    let rows = [
        (
            "Disk",
            crate::services::sysmon::fmt_rate(s.disk_bps),
            st.f.sysmon.disk.iter().copied().collect::<Vec<f32>>(),
            1e6,
            TINT_TIMER,
        ),
        (
            "Network",
            crate::services::sysmon::fmt_rate(s.net_bps),
            st.f.sysmon.net.iter().copied().collect::<Vec<f32>>(),
            1e5,
            TINT_WEATHER,
        ),
    ];
    for (i, (label, val, data, max, tint)) in rows.iter().enumerate() {
        let r = Rect::new(rx, area.y + i as f32 * (rh + 8.0), rw, rh);
        row(cv, rv, 3 + i, r, |cv| {
            cv.sq(r, 14.0, cv.theme.surface);
            cv.circle(r.x + 14.0, r.y + 14.0, 3.0, *tint);
            cv.text(
                label,
                &text::MICRO,
                Rect::new(r.x + 22.0, r.y + 6.0, 70.0, 16.0),
                cv.theme.mist,
                Align::Left,
            );
            cv.text(
                val,
                &text::BODY_MED.tnum(),
                Rect::new(r.x + 60.0, r.y + 6.0, r.w - 70.0, 16.0),
                cv.theme.ink,
                Align::Right,
            );
            cv.push_clip(r.inset(1.0));
            area_chart(
                cv,
                Rect::new(r.x + 6.0, r.y + 24.0, r.w - 12.0, r.h - 28.0),
                data,
                *max,
                *tint,
            );
            cv.pop_clip();
        });
    }
}
