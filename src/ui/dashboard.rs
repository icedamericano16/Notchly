//! Dashboard: clock hero + a 2×2 grid of orderable tiles.

use super::{row, tile, Reveal};
use crate::app::state::State;
use crate::app::{Action, PageId};
use crate::render::icons::Icon;
use crate::render::text::{self, Align};
use crate::render::{Canvas, Rect};
use crate::theme::tokens::*;
use crate::util::{fmt_hm, fmt_mmss, local_now, MONTHS, WEEKDAYS};

pub const TILES: &[(&str, &str)] = &[
    ("music", "Now playing"),
    ("timer", "Timer"),
    ("water", "Water"),
    ("todo", "To-do"),
    ("battery", "Battery"),
    ("prayer", "Next prayer"),
    ("clipboard", "Clipboard"),
    ("weather", "Weather"),
    ("event", "Next event"),
    ("sysmon", "System"),
    ("countdown", "Countdown"),
    ("focus", "Focus stats"),
];

pub fn tile_page(id: &str) -> PageId {
    match id {
        "music" => PageId::Music,
        "timer" | "focus" => PageId::Timer,
        "water" => PageId::Water,
        "todo" => PageId::Todo,
        "prayer" => PageId::Prayer,
        "clipboard" => PageId::Clipboard,
        "weather" => PageId::Weather,
        "event" => PageId::Calendar,
        "sysmon" => PageId::SysMon,
        _ => PageId::Dashboard,
    }
}

pub fn draw(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    let (hero, grid) = area.split_left(area.w * 0.42);
    let now = local_now();
    let h24 = st.clock_24h();
    row(cv, rv, 0, hero, |cv| {
        let t = fmt_hm(now.minutes_of_day(), h24);
        let (main, suffix) = t
            .split_once(' ')
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .unwrap_or((t.clone(), String::new()));
        let w = cv.text(
            &main,
            &text::DISPLAY,
            Rect::new(hero.x, hero.y + 4.0, hero.w, 52.0),
            cv.theme.ink,
            Align::Left,
        );
        if !suffix.is_empty() {
            cv.text(
                &suffix,
                &text::CAPTION,
                Rect::new(hero.x + w + 4.0, hero.y + 26.0, 30.0, 20.0),
                cv.theme.mist,
                Align::Left,
            );
        }
        let date = format!(
            "{}, {} {}",
            WEEKDAYS[now.weekday as usize],
            MONTHS[now.month as usize - 1],
            now.day
        );
        cv.text(
            &date,
            &text::CAPTION,
            Rect::new(hero.x, hero.y + 56.0, hero.w, 16.0),
            cv.theme.mist,
            Align::Left,
        );
    });
    row(cv, rv, 1, hero, |cv| {
        let mut y = hero.y + 74.0;
        if st.settings.general.nepali_date {
            if let Some(bs) = crate::features::bsdate::format(
                now.day_number(),
                st.settings.general.nepali_numerals,
            ) {
                cv.text(
                    &format!("{bs} BS"),
                    &text::CAPTION,
                    Rect::new(hero.x, y, hero.w, 16.0),
                    FAINT,
                    Align::Left,
                );
                y += 16.0;
            }
            if let Some(h) = crate::features::bsdate::holiday_on(now.day_number(), &st.f.holidays) {
                cv.text(
                    &h,
                    &text::CAPTION,
                    Rect::new(hero.x, y, hero.w, 16.0),
                    cv.theme.accent[0],
                    Align::Left,
                );
                y += 16.0;
            }
        }
        for wc in st.settings.general.world_clocks.iter().take(4) {
            if y > hero.bottom() - 12.0 {
                break;
            }
            let utc = crate::util::unix_secs();
            let local = (utc + wc.offset_min as i64 * 60).rem_euclid(86400) / 60;
            let off = wc.offset_min;
            let label = format!(
                "{} {} ({}{}:{:02})",
                wc.city,
                fmt_hm(local as u32, h24),
                if off >= 0 { "+" } else { "-" },
                off.abs() / 60,
                off.abs() % 60
            );
            cv.text(
                &label,
                &text::CAPTION,
                Rect::new(hero.x, y, hero.w, 16.0),
                FAINT,
                Align::Left,
            );
            y += 16.0;
        }
    });
    // tiles: 2 + calendar card, or 2×2 when the calendar is off
    let with_cal = st.settings.widgets.home_calendar;
    let count = if with_cal { 2 } else { 4 };
    let ids: Vec<String> = st
        .settings
        .widgets
        .tiles
        .iter()
        .take(count)
        .cloned()
        .collect();
    let tw = (grid.w - 8.0) / 2.0;
    let th = if with_cal {
        58.0
    } else {
        ((grid.h - 8.0) / 2.0).min(72.0)
    };
    if with_cal {
        let card = Rect::new(
            grid.x,
            grid.y + th + 8.0,
            grid.w,
            grid.bottom() - grid.y - th - 8.0,
        );
        row(cv, rv, 3, card, |cv| calendar_card(st, cv, card));
    }
    for (i, id) in ids.iter().enumerate() {
        let r = Rect::new(
            grid.x + (i % 2) as f32 * (tw + 8.0),
            grid.y + (i / 2) as f32 * (th + 8.0),
            tw,
            th,
        );
        let name = TILES
            .iter()
            .find(|t| t.0 == id)
            .map(|t| t.1)
            .unwrap_or("Tile");
        row(cv, rv, 1 + i, r, |cv| {
            tile(cv, r, Action::Tile(i as u8), name);
            draw_tile(st, cv, id, r.inset_xy(12.0, 8.0));
        });
    }
}

/// Home calendar card: month, a week strip with today lit and event dots, and the next event.
fn calendar_card(st: &mut State, cv: &mut Canvas, r: Rect) {
    tile(cv, r, Action::Open(PageId::Calendar), "Calendar");
    let now = local_now();
    let today = now.day_number();
    let unix = crate::util::unix_secs();
    let off = crate::util::local_offset_secs();
    let inner = r.inset_xy(12.0, 8.0);
    // events + dated to-dos, as (start, title, colour)
    let mut items: Vec<(i64, String, Color)> =
        st.f.calendar
            .events
            .iter()
            .filter(|e| e.end > unix - 7 * 86_400)
            .map(|e| (e.start, e.title.clone(), Color::rgb(e.color)))
            .collect();
    for t in st.f.todo.tasks.iter().filter(|t| !t.done) {
        if let Some(d) = t.due {
            items.push((d, t.title.clone(), TINT_TODO));
        }
    }
    items.sort_by_key(|i| i.0);
    let day_of = |ts: i64| (ts + off).div_euclid(86_400);

    // header: month + Bikram Sambat date when enabled
    cv.text(
        &MONTHS[now.month as usize - 1].to_uppercase(),
        &text::MICRO.weight(700),
        Rect::new(inner.x, inner.y, inner.w * 0.5, 14.0),
        TINT_CALENDAR,
        Align::Left,
    );
    if st.settings.general.nepali_date {
        if let Some(bs) =
            crate::features::bsdate::format(today, st.settings.general.nepali_numerals)
        {
            cv.text(
                &bs,
                &text::MICRO,
                Rect::new(inner.x, inner.y, inner.w, 14.0),
                FAINT,
                Align::Right,
            );
        }
    }

    // week strip (Sunday first)
    let start = today - crate::util::weekday_of(today) as i64;
    let cw = inner.w / 7.0;
    let sy = inner.y + 18.0;
    for i in 0..7 {
        let d = start + i;
        let cx = inner.x + cw * (i as f32 + 0.5);
        let is_today = d == today;
        let (_, _, dd) = crate::util::civil_from_days(d);
        cv.text(
            &WEEKDAYS[i as usize][..1],
            &text::MICRO,
            Rect::new(cx - cw / 2.0, sy, cw, 12.0),
            if is_today { cv.theme.ink } else { FAINT },
            Align::Center,
        );
        let ny = sy + 22.0;
        if is_today {
            cv.circle(cx, ny, 10.5, TINT_CALENDAR);
        }
        cv.text(
            &dd.to_string(),
            &text::CAPTION
                .weight(if is_today { 700 } else { 500 })
                .tnum(),
            Rect::new(cx - 12.0, ny - 9.0, 24.0, 18.0),
            if is_today {
                WHITE
            } else if d < today {
                FAINT
            } else {
                cv.theme.ink
            },
            Align::Center,
        );
        let n = items.iter().filter(|it| day_of(it.0) == d).count().min(3);
        for k in 0..n {
            let dx = (k as f32 - (n as f32 - 1.0) / 2.0) * 5.0;
            cv.circle(
                cx + dx,
                ny + 15.0,
                1.6,
                if is_today {
                    TINT_CALENDAR
                } else {
                    cv.theme.mist
                },
            );
        }
    }

    // next event
    let ey = inner.bottom() - 18.0;
    let next = items.iter().find(|it| it.0 >= unix - 15 * 60);
    match next {
        Some((ts, title, c)) => {
            cv.rrect(Rect::new(inner.x, ey + 2.0, 3.0, 14.0), 1.5, *c);
            let lt = crate::util::unix_to_local(*ts);
            let when = if day_of(*ts) == today {
                let mins = (ts - unix) / 60;
                if mins <= 0 {
                    crate::i18n::tr("Now")
                } else if mins < 60 {
                    format!("{} {mins}m", crate::i18n::tr("in"))
                } else {
                    fmt_hm(lt.minutes_of_day(), st.clock_24h())
                }
            } else if day_of(*ts) == today + 1 {
                format!(
                    "{} {}",
                    crate::i18n::tr("Tomorrow"),
                    fmt_hm(lt.minutes_of_day(), st.clock_24h())
                )
            } else {
                format!("{} {}", &WEEKDAYS[lt.weekday as usize][..3], lt.day)
            };
            let ww = cv.measure(&when, &text::CAPTION) + 4.0;
            cv.text(
                title,
                &text::CAPTION.weight(600),
                Rect::new(inner.x + 9.0, ey, inner.w - ww - 14.0, 18.0),
                cv.theme.ink,
                Align::Left,
            );
            cv.text(
                &when,
                &text::CAPTION,
                Rect::new(inner.right() - ww, ey, ww, 18.0),
                cv.theme.mist,
                Align::Right,
            );
        }
        None => {
            let msg = if st.settings.calendar.enabled {
                "No upcoming events"
            } else {
                "Turn on Calendar in Settings"
            };
            cv.label(
                msg,
                &text::CAPTION,
                Rect::new(inner.x, ey, inner.w, 18.0),
                cv.theme.mist,
                Align::Left,
            );
        }
    }
}

fn label_value(cv: &mut Canvas, r: Rect, icon: Icon, icon_color: Color, label: &str, value: &str) {
    cv.icon(icon, r.x + 8.0, r.y + 9.0, 14.0, icon_color);
    cv.label(
        label,
        &text::MICRO,
        Rect::new(r.x + 20.0, r.y, r.w - 20.0, 18.0),
        cv.theme.mist,
        Align::Left,
    );
    cv.text(
        value,
        &text::BODY_MED,
        Rect::new(r.x, r.y + 18.0, r.w, r.h - 18.0),
        cv.theme.ink,
        Align::Left,
    );
}

fn draw_tile(st: &mut State, cv: &mut Canvas, id: &str, r: Rect) {
    let acc = cv.theme.accent[0];
    let pro = true;
    match id {
        "music" => {
            if st.media.has_session() {
                let art = Rect::new(r.x, r.cy() - 16.0, 32.0, 32.0);
                if let Some(img) = st.media.art.as_mut() {
                    cv.image(img, art, 8.0);
                } else {
                    cv.rrect(art, 8.0, SURFACE_RAISED);
                }
                cv.text(
                    &st.media.snap.title,
                    &text::CAPTION.weight(600),
                    Rect::new(art.right() + 8.0, r.y + 2.0, r.w - 40.0, 18.0),
                    cv.theme.ink,
                    Align::Left,
                );
                cv.text(
                    &st.media.snap.artist,
                    &text::CAPTION,
                    Rect::new(art.right() + 8.0, r.y + 18.0, r.w - 40.0, 18.0),
                    cv.theme.mist,
                    Align::Left,
                );
                cv.progress(
                    Rect::new(art.right() + 8.0, r.bottom() - 4.0, r.w - 40.0, 3.0),
                    st.media.progress(),
                    false,
                );
            } else {
                label_value(cv, r, Icon::Music, TINT_MUSIC, "Music", "Nothing playing");
            }
        }
        "timer" => {
            let t = &st.f.state.timer;
            let v = if t.active() {
                fmt_mmss(t.remaining_ms() / 1000)
            } else {
                format!("{} min", st.settings.productivity.focus_min)
            };
            label_value(cv, r, Icon::Timer, TINT_TIMER, t.phase.label(), &v);
        }
        "water" => {
            let w = &st.f.state.water;
            let goal = st.settings.productivity.water_goal_ml;
            label_value(
                cv,
                r,
                Icon::Droplet,
                TINT_WATER,
                "Water",
                &format!("{} / {} ml", w.ml, goal),
            );
            cv.progress(
                Rect::new(r.x, r.bottom() - 3.0, r.w, 3.0),
                w.ml as f32 / goal.max(1) as f32,
                false,
            );
        }
        "todo" => {
            let n = st.f.todo.open_count();
            label_value(
                cv,
                r,
                Icon::CheckCircle,
                TINT_TODO,
                "To-do",
                &if n == 0 {
                    "All done".into()
                } else {
                    format!("{n} open")
                },
            );
        }
        "battery" => {
            let b = &st.sys.battery;
            if b.present {
                let c = if b.ac {
                    GOOD
                } else if b.percent <= 20 {
                    WARN
                } else {
                    cv.theme.ink
                };
                label_value(
                    cv,
                    r,
                    Icon::Bolt,
                    c,
                    if b.ac { "Charging" } else { "Battery" },
                    &format!("{}%", b.percent),
                );
            } else {
                label_value(cv, r, Icon::Bolt, GOOD, "Power", "Plugged in");
            }
        }
        "prayer" => {
            let settings = st.settings.clone();
            let v = st
                .f
                .prayer_times(&settings)
                .and_then(|t| {
                    crate::features::prayer::next_prayer(&t, local_now().secs_of_day() / 60.0).map(
                        |(i, m)| format!("{} · {}", t.prayers()[i].0, fmt_mmss((m * 60.0) as i64)),
                    )
                })
                .unwrap_or_else(|| "Set location".into());
            label_value(cv, r, Icon::MoonStar, TINT_PRAYER, "Next prayer", &v);
        }
        "clipboard" => {
            let v =
                st.f.clip
                    .items
                    .first()
                    .map(|c| c.text.clone())
                    .unwrap_or_else(|| "Empty".into());
            label_value(cv, r, Icon::Clipboard, TINT_CLIP, "Clipboard", &v);
        }
        "weather" if pro => {
            let v =
                st.f.weather
                    .forecast
                    .as_ref()
                    .map(|f| {
                        format!(
                            "{:.0}° {}",
                            temp(st, f.temp),
                            crate::services::weather::describe(&f.symbol)
                        )
                    })
                    .unwrap_or_else(|| "—".into());
            let ic =
                st.f.weather
                    .forecast
                    .as_ref()
                    .map(|f| crate::services::weather::icon_for(&f.symbol))
                    .unwrap_or(Icon::CloudSun);
            label_value(cv, r, ic, TINT_WEATHER, "Weather", &v);
        }
        "event" if pro => {
            let now = crate::util::unix_secs();
            let v =
                st.f.calendar
                    .events
                    .iter()
                    .filter(|e| e.end > now)
                    .min_by_key(|e| e.start)
                    .map(|e| e.title.clone())
                    .unwrap_or_else(|| "No events".into());
            label_value(cv, r, Icon::Calendar, TINT_CALENDAR, "Next event", &v);
        }
        "sysmon" if pro => {
            let s = st.f.sysmon.last.unwrap_or_default();
            label_value(
                cv,
                r,
                Icon::Cpu,
                TINT_SYSTEM,
                "System",
                &format!("CPU {:.0}% · RAM {:.0}%", s.cpu, s.ram),
            );
        }
        "countdown" => {
            let w = &st.settings.widgets;
            let days = w.countdown_day - local_now().day_number();
            let title = if w.countdown_title.is_empty() {
                "Countdown".to_string()
            } else {
                w.countdown_title.clone()
            };
            label_value(
                cv,
                r,
                Icon::Star,
                TINT_TIMER,
                &title,
                &if w.countdown_day == 0 {
                    "Set a date".into()
                } else {
                    format!("{days} days")
                },
            );
        }
        "focus" => {
            let m = st.f.state.timer.today_minutes();
            label_value(
                cv,
                r,
                Icon::Timer,
                TINT_TIMER,
                "Focus today",
                &format!("{m} min"),
            );
        }
        _ => label_value(cv, r, Icon::Sparkles, acc, "Tile", "—"),
    }
}

pub fn temp(st: &State, c: f32) -> f32 {
    if st.settings.weather.units == crate::settings::Units::Imperial {
        c * 9.0 / 5.0 + 32.0
    } else {
        c
    }
}
