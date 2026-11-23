//! Home: a living clock on the left (its card follows the time of day: dawn, day, dusk,
//! night, with the sun or moon travelling a small arc), and on the right your calendar week and
//! the notifications that matter most.

use super::{row, tile, Reveal};
use crate::app::state::State;
use crate::app::{Action, PageId};
use crate::features::store::NotifRecord;
use crate::features::sun;
use crate::render::icons::Icon;
use crate::render::text::{self, Align};
use crate::render::{shapes, Canvas, Rect};
use crate::theme::tokens::*;
use crate::util::{fmt_hm, local_now, MONTHS, WEEKDAYS};

pub fn draw(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    let hero = Rect::new(area.x, area.y, 198.0, area.h);
    let right = Rect::new(
        hero.right() + 10.0,
        area.y,
        area.right() - hero.right() - 10.0,
        area.h,
    );
    row(cv, rv, 0, hero, |cv| clock_card(st, cv, hero));
    let cal = Rect::new(right.x, right.y, right.w, 92.0);
    let notes = Rect::new(
        right.x,
        cal.bottom() + 8.0,
        right.w,
        right.bottom() - cal.bottom() - 8.0,
    );
    row(cv, rv, 1, cal, |cv| calendar_card(st, cv, cal));
    row(cv, rv, 2, notes, |cv| notifications_card(st, cv, notes));
}

// ---------------------------------------------------------------- clock

/// Sky colours for the hour (0..24): night, dawn, day, dusk, blended smoothly.
fn sky(hour: f32) -> (Color, Color) {
    let stops: [(f32, u32, u32); 7] = [
        (0.0, 0x0B1230, 0x1A1F45),
        (5.0, 0x1A1F45, 0x3B2A55),
        (6.5, 0x7A4A6B, 0xE58A5B),
        (9.0, 0x1E5AA8, 0x3F86D1),
        (16.5, 0x2A64B0, 0x5B8FCC),
        (18.5, 0x5B2E6E, 0xE0784B),
        (20.0, 0x141A3A, 0x272E5C),
    ];
    let mut a = stops[stops.len() - 1];
    let mut b = (24.0, stops[0].1, stops[0].2);
    for w in stops.windows(2) {
        if hour >= w[0].0 && hour < w[1].0 {
            a = w[0];
            b = w[1];
            break;
        }
    }
    let span = if b.0 > a.0 {
        b.0 - a.0
    } else {
        24.0 - a.0 + b.0
    };
    let f = (((hour - a.0) + 24.0) % 24.0 / span).clamp(0.0, 1.0);
    let f = f * f * (3.0 - 2.0 * f);
    (
        Color::rgb(a.1).lerp(Color::rgb(b.1), f),
        Color::rgb(a.2).lerp(Color::rgb(b.2), f),
    )
}

fn clock_card(st: &State, cv: &mut Canvas, r: Rect) {
    let now = local_now();
    let secs = now.secs_of_day() as f32;
    let hour = secs / 3600.0;
    let t = st.started.elapsed().as_secs_f32();
    let (top, bottom) = sky(hour);
    // the card: the sky for this hour, deepened so the type stays crisp
    if let Some(g) = cv.gradient(
        crate::render::canvas::v2(r.x, r.y),
        crate::render::canvas::v2(r.right(), r.bottom()),
        &[
            (0.0, top.lerp(NOTCH, 0.35)),
            (1.0, bottom.lerp(NOTCH, 0.55)),
        ],
        1.0,
    ) {
        use windows::core::Interface;
        cv.rrect_brush(r, 20.0, &g.cast().expect("brush"));
    }
    cv.stroke_rrect(r.inset(0.5), 19.5, WHITE.with_alpha(0.08), 1.0);

    // sun / moon arc across the top: the real sun when the location is known
    let (lat, lon) = (st.settings.weather.lat, st.settings.weather.lon);
    let has_loc = lat != 0.0 || lon != 0.0;
    let unix = crate::util::unix_secs();
    let elev = if has_loc {
        sun::elevation(lat, lon, unix) as f32
    } else {
        // no location: a gentle stand-in from the hour
        ((hour - 6.0) / 12.0 * std::f32::consts::PI).sin() * 50.0
    };
    let night = elev < -0.8;
    cv.push_clip(r.inset(1.0));
    if night {
        for i in 0..18u32 {
            let x = r.x + 12.0 + (hash(i, 1) * (r.w - 24.0));
            let y = r.y + 8.0 + hash(i, 2) * r.h * 0.55;
            let tw = 0.3 + 0.7 * (t * (0.8 + hash(i, 3) * 1.6) + i as f32).sin().abs();
            cv.circle(x, y, 0.5 + hash(i, 4) * 0.8, WHITE.with_alpha(0.6 * tw));
        }
    }
    let arc = Rect::new(r.x + 18.0, r.y + 12.0, r.w - 36.0, 40.0);
    let day_frac = hour / 24.0;
    let pts: Vec<(f32, f32)> = (0..=40)
        .map(|i| {
            let f = i as f32 / 40.0;
            (
                arc.x + arc.w * f,
                arc.bottom() - arc.h * (std::f32::consts::PI * f).sin(),
            )
        })
        .collect();
    let factory = cv.gfx.factory.clone();
    if let Ok(g) = shapes::poly(&factory, &pts, false) {
        cv.stroke_geo(&g, WHITE.with_alpha(0.16), 1.2);
    }
    cv.line(
        r.x + 12.0,
        arc.bottom(),
        r.right() - 12.0,
        arc.bottom(),
        1.0,
        WHITE.with_alpha(0.10),
    );
    // body position: along the arc by time of day, height by the real elevation
    let bx = arc.x + arc.w * day_frac;
    let by = arc.bottom() - arc.h * (elev / 60.0).clamp(-0.25, 1.0);
    if night {
        cv.circle(bx, by.min(arc.bottom() - 4.0), 5.5, Color::rgb(0xF2F0E6));
        cv.circle(
            bx + 2.6,
            by.min(arc.bottom() - 4.0) - 1.8,
            4.8,
            top.lerp(NOTCH, 0.35),
        );
    } else {
        let pulse = 1.0 + 0.08 * (t * 1.8).sin();
        if let Some(gl) = cv.radial(
            bx,
            by,
            18.0 * pulse,
            18.0 * pulse,
            Color::rgb(0xFFD60A).with_alpha(0.5),
            Color::rgb(0xFFD60A).with_alpha(0.0),
        ) {
            unsafe {
                cv.ctx.FillEllipse(
                    &windows::Win32::Graphics::Direct2D::D2D1_ELLIPSE {
                        point: crate::render::canvas::v2(bx, by),
                        radiusX: 18.0 * pulse,
                        radiusY: 18.0 * pulse,
                    },
                    &gl,
                )
            };
        }
        cv.circle(bx, by, 5.0, Color::rgb(0xFFD60A));
    }
    cv.pop_clip();

    // time: big tabular digits, the colon breathes with the seconds
    let h24 = st.clock_24h();
    let mins = now.minutes_of_day();
    let (h, m) = (mins / 60, mins % 60);
    let (hh, suffix) = if h24 {
        (format!("{h:02}"), String::new())
    } else {
        let h12 = if h % 12 == 0 { 12 } else { h % 12 };
        (
            h12.to_string(),
            if h < 12 { "AM".into() } else { "PM".into() },
        )
    };
    let ty = r.y + 54.0;
    let big = text::DISPLAY.size(44.0).tnum();
    let wh = cv.measure(&hh, &big);
    let wc = cv.measure(":", &big);
    let wm = cv.measure(&format!("{m:02}"), &big);
    let ws = if suffix.is_empty() {
        0.0
    } else {
        cv.measure(&suffix, &text::CAPTION.weight(700)) + 4.0
    };
    let x0 = r.x + 16.0;
    let _ = ws;
    cv.text(
        &hh,
        &big,
        Rect::new(x0, ty, wh + 2.0, 48.0),
        WHITE,
        Align::Left,
    );
    let colon_a = 0.45 + 0.55 * (0.5 + 0.5 * (secs.fract() * std::f32::consts::TAU).cos());
    cv.text(
        ":",
        &big,
        Rect::new(x0 + wh, ty - 2.0, wc + 2.0, 48.0),
        WHITE.with_alpha(colon_a),
        Align::Left,
    );
    cv.text(
        &format!("{m:02}"),
        &big,
        Rect::new(x0 + wh + wc, ty, wm + 2.0, 48.0),
        WHITE,
        Align::Left,
    );
    if !suffix.is_empty() {
        cv.text(
            &suffix,
            &text::CAPTION.weight(700),
            Rect::new(x0 + wh + wc + wm + 4.0, ty + 24.0, 30.0, 16.0),
            WHITE.with_alpha(0.7),
            Align::Left,
        );
    }
    // the minute fills a hairline under the time
    let bar = Rect::new(x0, ty + 50.0, r.w - 32.0, 2.0);
    cv.rrect(bar, 1.0, WHITE.with_alpha(0.12));
    let sec_frac = (secs % 60.0) / 60.0;
    cv.rrect(
        Rect::new(bar.x, bar.y, bar.w * sec_frac, bar.h),
        1.0,
        WHITE.with_alpha(0.7),
    );

    // date, Bikram Sambat and a holiday
    let mut y = ty + 58.0;
    let date = format!(
        "{}, {} {}",
        crate::i18n::tr(WEEKDAYS[now.weekday as usize]),
        crate::i18n::tr(MONTHS[now.month as usize - 1]),
        now.day
    );
    cv.text(
        &date,
        &text::BODY_MED,
        Rect::new(x0, y, r.w - 32.0, 18.0),
        WHITE.with_alpha(0.92),
        Align::Left,
    );
    y += 18.0;
    if st.settings.general.nepali_date {
        if let Some(bs) =
            crate::features::bsdate::format(now.day_number(), st.settings.general.nepali_numerals)
        {
            cv.text(
                &format!("{bs} BS"),
                &text::CAPTION,
                Rect::new(x0, y, r.w - 32.0, 15.0),
                WHITE.with_alpha(0.6),
                Align::Left,
            );
            y += 16.0;
        }
        if let Some(hol) = crate::features::bsdate::holiday_on(now.day_number(), &st.f.holidays) {
            let w = cv.measure(&hol, &text::CAPTION.weight(600)) + 16.0;
            let c = Rect::new(x0, y + 1.0, w.min(r.w - 32.0), 17.0);
            cv.rrect(c, 8.5, Color::rgb(0xFF9F0A).with_alpha(0.25));
            cv.text(
                &hol,
                &text::CAPTION.weight(600),
                c,
                Color::rgb(0xFFD08A),
                Align::Center,
            );
            y += 20.0;
        }
    }
    // world clocks: small pills with day / night glyphs
    for wc in st.settings.general.world_clocks.iter().take(2) {
        if y > r.bottom() - 58.0 {
            break;
        }
        let utc = crate::util::unix_secs();
        let local = ((utc + wc.offset_min as i64 * 60).rem_euclid(86_400) / 60) as u32;
        let day = (6 * 60..18 * 60).contains(&local);
        let label = format!("{}  {}", wc.city, fmt_hm(local, h24));
        let w = (cv.measure(&label, &text::CAPTION) + 30.0).min(r.w - 32.0);
        let c = Rect::new(x0, y + 2.0, w, 18.0);
        cv.rrect(c, 9.0, WHITE.with_alpha(0.1));
        cv.icon(
            if day { Icon::Sun } else { Icon::MoonStar },
            c.x + 10.0,
            c.cy(),
            10.0,
            WHITE.with_alpha(0.8),
        );
        cv.text(
            &label,
            &text::CAPTION,
            Rect::new(c.x + 19.0, c.y, w - 22.0, c.h),
            WHITE.with_alpha(0.85),
            Align::Left,
        );
        y += 22.0;
    }
    glance(st, cv, r, x0);
}

/// Bottom of the clock card: current weather and the next sunrise / sunset.
fn glance(st: &State, cv: &mut Canvas, r: Rect, x0: f32) {
    let y = r.bottom() - 30.0;
    cv.line(
        x0,
        y - 6.0,
        r.right() - 16.0,
        y - 6.0,
        1.0,
        WHITE.with_alpha(0.08),
    );
    let mut x = x0;
    if let Some(f) = st.f.weather.forecast.as_ref() {
        let ic = crate::services::weather::icon_for(&f.symbol);
        cv.icon(ic, x + 8.0, y + 10.0, 15.0, WHITE.with_alpha(0.9));
        let s = format!("{:.0}°", temp(st, f.temp));
        let w = cv.measure(&s, &text::BODY_MED) + 4.0;
        cv.text(
            &s,
            &text::BODY_MED,
            Rect::new(x + 20.0, y + 1.0, w, 18.0),
            WHITE,
            Align::Left,
        );
        x += 24.0 + w;
    }
    let (lat, lon) = (st.settings.weather.lat, st.settings.weather.lon);
    if lat != 0.0 || lon != 0.0 {
        let off = crate::util::local_offset_secs();
        let now = crate::util::unix_secs();
        let day = (now + off).div_euclid(86_400);
        let today = sun::day(lat, lon, day, off);
        let (label, at) = match (today.rise, today.set) {
            (Some(rise), _) if now < rise => ("Sunrise", Some(rise)),
            (_, Some(set)) if now < set => ("Sunset", Some(set)),
            _ => ("Sunrise", sun::day(lat, lon, day + 1, off).rise),
        };
        if let Some(at) = at {
            let s = format!(
                "{} {}",
                crate::i18n::tr(label),
                fmt_hm(
                    crate::util::unix_to_local(at).minutes_of_day(),
                    st.clock_24h()
                )
            );
            let w = cv.measure(&s, &text::CAPTION) + 18.0;
            let rx = (r.right() - 16.0 - w).max(x + 6.0);
            cv.icon(Icon::Sun, rx + 6.0, y + 10.0, 11.0, Color::rgb(0xFFD08A));
            cv.text(
                &s,
                &text::CAPTION,
                Rect::new(rx + 15.0, y + 2.0, w - 14.0, 16.0),
                WHITE.with_alpha(0.75),
                Align::Left,
            );
        }
    }
}

fn hash(i: u32, salt: u32) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xFFFF) as f32 / 65535.0
}

// ---------------------------------------------------------------- calendar

/// Calendar card: month, a week strip with today lit and event dots, and the next event.
fn calendar_card(st: &mut State, cv: &mut Canvas, r: Rect) {
    tile(cv, r, Action::Open(PageId::Calendar), "Calendar");
    let now = local_now();
    let today = now.day_number();
    let unix = crate::util::unix_secs();
    let off = crate::util::local_offset_secs();
    let inner = r.inset_xy(12.0, 9.0);
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

    cv.icon(
        Icon::Calendar,
        inner.x + 6.0,
        inner.y + 7.0,
        12.0,
        TINT_CALENDAR,
    );
    cv.text(
        &crate::i18n::tr(MONTHS[now.month as usize - 1]).to_uppercase(),
        &text::MICRO.weight(700),
        Rect::new(inner.x + 16.0, inner.y, inner.w * 0.6, 14.0),
        TINT_CALENDAR,
        Align::Left,
    );
    let upcoming = items.iter().filter(|it| it.0 >= unix).count();
    if upcoming > 0 {
        cv.text(
            &format!("{upcoming} {}", crate::i18n::tr("upcoming")),
            &text::MICRO,
            Rect::new(inner.x, inner.y, inner.w, 14.0),
            FAINT,
            Align::Right,
        );
    }
    // week strip (Sunday first)
    let start = today - crate::util::weekday_of(today) as i64;
    let cw = inner.w / 7.0;
    let sy = inner.y + 17.0;
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
        let ny = sy + 21.0;
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
                ny + 14.5,
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
    let ey = inner.bottom() - 16.0;
    match items.iter().find(|it| it.0 >= unix - 15 * 60) {
        Some((ts, title, c)) => {
            cv.rrect(Rect::new(inner.x, ey + 1.0, 3.0, 14.0), 1.5, *c);
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
                Rect::new(inner.x + 9.0, ey, inner.w - ww - 14.0, 16.0),
                cv.theme.ink,
                Align::Left,
            );
            cv.text(
                &when,
                &text::CAPTION,
                Rect::new(inner.right() - ww, ey, ww, 16.0),
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
                Rect::new(inner.x, ey, inner.w, 16.0),
                cv.theme.mist,
                Align::Left,
            );
        }
    }
}

// ---------------------------------------------------------------- notifications

/// How much a notification matters: calls, then people (messages, mail), calendar and
/// reminders, then everything else; system chatter last.
pub fn priority(n: &NotifRecord) -> u8 {
    let text = format!("{} {} {}", n.app, n.title, n.body).to_lowercase();
    let app = n.app.to_lowercase();
    if [
        "incoming call",
        "is calling",
        "calling you",
        "missed call",
        "ringing",
    ]
    .iter()
    .any(|k| text.contains(k))
    {
        return 4;
    }
    if [
        "whatsapp",
        "telegram",
        "messenger",
        "signal",
        "teams",
        "slack",
        "discord",
        "outlook",
        "mail",
        "gmail",
        "phone link",
        "instagram",
        "messages",
    ]
    .iter()
    .any(|k| app.contains(k))
    {
        return 3;
    }
    if ["calendar", "reminder", "alarm", "meeting"]
        .iter()
        .any(|k| text.contains(k))
    {
        return 3;
    }
    if [
        "snipping",
        "windows security",
        "windows update",
        "predator",
        "file explorer",
        "windows",
    ]
    .iter()
    .any(|k| app.contains(k))
    {
        return 1;
    }
    2
}

/// Notifications by priority, then newest first.
pub fn ranked(items: &[NotifRecord]) -> Vec<NotifRecord> {
    let mut v: Vec<NotifRecord> = items.to_vec();
    v.sort_by(|a, b| priority(b).cmp(&priority(a)).then(b.time.cmp(&a.time)));
    v
}

fn ago(time: i64) -> String {
    let d = (crate::util::unix_secs() - time).max(0);
    if d < 60 {
        crate::i18n::tr("now")
    } else if d < 3600 {
        format!("{}m", d / 60)
    } else if d < 86_400 {
        format!("{}h", d / 3600)
    } else {
        format!("{}d", d / 86_400)
    }
}

/// A tinted avatar with the app's initial.
pub fn avatar(cv: &mut Canvas, app: &str, cx: f32, cy: f32, r: f32, dim: bool) {
    let hue = [
        TINT_WEATHER,
        TINT_TODO,
        TINT_MUSIC,
        TINT_TIMER,
        TINT_PRAYER,
        TINT_WATER,
        TINT_SHELF,
    ];
    let c = hue[(crate::util::fnv1a(app) % hue.len() as u64) as usize];
    cv.circle(cx, cy, r, c.with_alpha(if dim { 0.14 } else { 0.24 }));
    let initial: String = app
        .chars()
        .next()
        .map(|ch| ch.to_uppercase().collect())
        .unwrap_or_default();
    cv.text(
        &initial,
        &text::CAPTION.weight(700),
        Rect::new(cx - r, cy - r, r * 2.0, r * 2.0),
        c,
        Align::Center,
    );
}

fn notifications_card(st: &mut State, cv: &mut Canvas, r: Rect) {
    use crate::features::ListKind;
    cv.sq(r, R_TILE, cv.theme.surface);
    let inner = r.inset_xy(12.0, 9.0);
    let live: Vec<NotifRecord> =
        st.f.notifs
            .items
            .iter()
            .filter(|n| !st.f.is_leaving(ListKind::Notif, n.id))
            .cloned()
            .collect();
    let list = ranked(&st.f.notifs.items);
    cv.icon(
        Icon::Bell,
        inner.x + 6.0,
        inner.y + 7.0,
        12.0,
        TINT_CALENDAR,
    );
    cv.label(
        "Notifications",
        &text::MICRO.weight(700),
        Rect::new(inner.x + 16.0, inner.y, 120.0, 14.0),
        cv.theme.ink,
        Align::Left,
    );
    if !live.is_empty() {
        let n = live.len().to_string();
        let nw = cv.measure(&n, &text::MICRO.weight(700)) + 10.0;
        let lw = cv.measure(
            &crate::i18n::tr("Notifications").to_uppercase(),
            &text::MICRO.weight(700),
        );
        let b = Rect::new(inner.x + 20.0 + lw, inner.y, nw.max(16.0), 14.0);
        cv.rrect(b, 7.0, TINT_CALENDAR.with_alpha(0.85));
        cv.text(&n, &text::MICRO.weight(700), b, WHITE, Align::Center);
        // clear all
        let cw = cv.measure(&crate::i18n::tr("Clear all"), &text::MICRO.weight(700)) + 12.0;
        let c = Rect::new(inner.right() - cw, inner.y - 2.0, cw, 18.0);
        let a = Action::NotifClear;
        let (hv, _) = cv.anim(&a, cv.hovered(c), false);
        cv.rrect(c, 9.0, LINE.with_alpha(0.5 + hv.min(1.0)));
        cv.label(
            "Clear all",
            &text::MICRO.weight(700),
            c,
            cv.theme.mist.lerp(cv.theme.ink, hv.min(1.0)),
            Align::Center,
        );
        cv.zone_named(c, a, true, "Clear all", 9.0);
    }
    let body = Rect::new(
        inner.x,
        inner.y + 20.0,
        inner.w,
        inner.bottom() - inner.y - 20.0,
    );
    if live.is_empty() {
        let (title, sub) = match st.f.notifs.access {
            Some(crate::services::notifications::Access::Denied) => (
                "Notifications are off",
                "Allow Notchly in Windows notification settings",
            ),
            Some(crate::services::notifications::Access::NoIdentity) => (
                "Not available",
                "Windows notifications can't be read on this PC",
            ),
            _ => ("You're all caught up", "New notifications appear here"),
        };
        cv.icon(
            Icon::CheckCircle,
            body.x + 12.0,
            body.cy(),
            18.0,
            TINT_TODO.with_alpha(0.8),
        );
        cv.label(
            title,
            &text::CAPTION.weight(600),
            Rect::new(body.x + 28.0, body.cy() - 16.0, body.w - 28.0, 16.0),
            cv.theme.ink,
            Align::Left,
        );
        cv.label(
            sub,
            &text::CAPTION.size(10.5),
            Rect::new(body.x + 28.0, body.cy(), body.w - 28.0, 14.0),
            cv.theme.mist,
            Align::Left,
        );
        return;
    }
    let row_h = 33.0;
    let rows = ((body.h + 2.0) / row_h).floor().max(1.0) as usize;
    let mut y = body.y;
    let mut shown = 0;
    for (i, n) in list.iter().enumerate() {
        if shown >= rows {
            break;
        }
        let p = st.f.presence(ListKind::Notif, n.id);
        let leaving = st.f.is_leaving(ListKind::Notif, n.id);
        let (a, dx, dy, hf) = super::row_presence(p, leaving, i);
        let rr = Rect::new(body.x - 4.0 + dx, y + dy, body.w + 8.0, row_h - 2.0);
        y += row_h * hf;
        if a <= 0.01 {
            continue;
        }
        if !leaving {
            shown += 1;
        }
        cv.push_alpha(a);
        let hv = super::hover_card(cv, rr, Action::NotifOpen(n.id), 10.0);
        let _ = hv;
        avatar(cv, &n.app, rr.x + 16.0, rr.cy(), 11.0, n.suppressed);
        let tx = rr.x + 33.0;
        let hover = cv.hovered(rr) && !leaving;
        let right_pad = if hover { 30.0 } else { 6.0 };
        let when = ago(n.time);
        let ww = cv.measure(&when, &text::MICRO) + 4.0;
        let hidden = st.sup.sharing && st.settings.quiet.hide_when_sharing;
        let title = if hidden {
            crate::i18n::tr("Hidden while sharing")
        } else {
            n.title.clone()
        };
        cv.text(
            &title,
            &text::CAPTION.weight(700),
            Rect::new(tx, rr.y + 2.0, rr.right() - tx - ww - right_pad - 4.0, 15.0),
            cv.theme.ink,
            Align::Left,
        );
        cv.text(
            &when,
            &text::MICRO,
            Rect::new(rr.right() - right_pad - ww, rr.y + 3.0, ww, 13.0),
            FAINT,
            Align::Right,
        );
        let sub = if hidden || n.body.is_empty() {
            n.app.clone()
        } else {
            format!("{} · {}", n.app, n.body.replace('\n', " "))
        };
        cv.text(
            &sub,
            &text::CAPTION.size(10.5),
            Rect::new(tx, rr.y + 16.0, rr.right() - tx - right_pad, 14.0),
            cv.theme.mist,
            Align::Left,
        );
        if hover {
            cv.button(
                Icon::Close,
                rr.right() - 15.0,
                rr.cy(),
                22.0,
                Action::NotifDismiss(n.id),
                "Dismiss",
                cv.theme.mist,
            );
        }
        cv.pop_alpha();
    }
    let more = live.len().saturating_sub(shown);
    if more > 0 && y < r.bottom() - 4.0 {
        let s = format!("+{more} {}", crate::i18n::tr("more"));
        let w = cv.measure(&s, &text::MICRO.weight(700)) + 12.0;
        let c = Rect::new(inner.right() - w, r.bottom() - 18.0, w, 14.0);
        cv.text(&s, &text::MICRO.weight(700), c, TINT_WEATHER, Align::Right);
        cv.zone_named(
            c,
            Action::Open(PageId::Notifications),
            true,
            "All notifications",
            6.0,
        );
    }
}

pub fn temp(st: &State, c: f32) -> f32 {
    if st.settings.weather.units == crate::settings::Units::Imperial {
        c * 9.0 / 5.0 + 32.0
    } else {
        c
    }
}
