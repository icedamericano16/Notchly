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
    // 2×2 tiles
    let ids: Vec<String> = st.settings.widgets.tiles.iter().take(4).cloned().collect();
    let tw = (grid.w - 8.0) / 2.0;
    let th = ((grid.h - 8.0) / 2.0).min(60.0);
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
