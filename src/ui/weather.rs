//! Weather: today in detail. Current conditions, a live sun path (the sun sits where it
//! really is in the sky right now, computed from your location) and a 24-hour temperature
//! curve with conditions and rain.

use super::dashboard::temp;
use super::{row, Reveal};
use crate::app::state::State;
use crate::app::Action;
use crate::features::sun;
use crate::render::icons::Icon;
use crate::render::text::{self, Align};
use crate::render::{shapes, Canvas, Rect};
use crate::services::weather::{describe, icon_for, Forecast};
use crate::theme::tokens::*;
use crate::util::{fmt_hm, unix_secs, unix_to_local};

const WARM: Color = Color::rgb(0xFF9F0A);
const COOL: Color = Color::rgb(0x64D2FF);
const SUN: Color = Color::rgb(0xFFD60A);

pub fn draw(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    let Some(f) = st.f.weather.forecast.clone() else {
        empty(st, cv, area, rv);
        return;
    };
    let t = st.started.elapsed().as_secs_f32();
    let top = Rect::new(area.x, area.y, area.w, 104.0);
    let (now_r, sun_r) = top.split_left(178.0);
    current(st, cv, now_r, &f, t, rv);
    let sun_card = Rect::new(sun_r.x + 6.0, sun_r.y, sun_r.w - 6.0, sun_r.h);
    row(cv, rv, 1, sun_card, |cv| sun_path(st, cv, sun_card, t));
    let chart = Rect::new(
        area.x,
        top.bottom() + 10.0,
        area.w,
        area.bottom() - top.bottom() - 10.0,
    );
    row(cv, rv, 2, chart, |cv| hours(st, cv, chart, &f, rv.row(2)));
}

fn empty(st: &State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
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
            28.0,
            TINT_WEATHER,
        );
        cv.label(
            &msg,
            &text::BODY,
            Rect::new(area.x, area.cy() + 8.0, area.w, 18.0),
            cv.theme.mist,
            Align::Center,
        );
    });
}

fn current(st: &State, cv: &mut Canvas, r: Rect, f: &Forecast, t: f32, rv: &Reveal) {
    let unit = "°";
    let wind = if st.settings.weather.units == crate::settings::Units::Imperial {
        format!("{:.0} mph", f.wind * 2.237)
    } else {
        format!("{:.0} km/h", f.wind * 3.6)
    };
    let today = f.days.first();
    row(cv, rv, 0, r, |cv| {
        // the condition icon breathes: the sun turns slowly, clouds drift
        let ic = icon_for(&f.symbol);
        let drift = match ic {
            Icon::Sun => 0.0,
            _ => (t * 0.8).sin() * 2.0,
        };
        let pulse = if ic == Icon::Sun {
            1.0 + 0.04 * (t * 1.6).sin()
        } else {
            1.0
        };
        cv.push_tf(drift, 0.0, pulse, r.x + 20.0, r.y + 22.0);
        let tint = match ic {
            Icon::Sun => SUN,
            Icon::MoonStar => Color::rgb(0xD1D1F0),
            _ => cv.theme.ink,
        };
        cv.icon(ic, r.x + 20.0, r.y + 22.0, 34.0, tint);
        cv.pop_tf();
        cv.text(
            &format!("{:.0}{unit}", temp(st, f.temp)),
            &text::DISPLAY.size(40.0),
            Rect::new(r.x + 44.0, r.y, r.w - 44.0, 44.0),
            cv.theme.ink,
            Align::Left,
        );
        cv.label(
            describe(&f.symbol),
            &text::BODY_MED,
            Rect::new(r.x, r.y + 46.0, r.w, 18.0),
            cv.theme.ink,
            Align::Left,
        );
        if let Some(d) = today {
            cv.text(
                &format!(
                    "H {:.0}°  L {:.0}°  ·  {} {:.0}°",
                    temp(st, d.max),
                    temp(st, d.min),
                    crate::i18n::tr("Feels"),
                    temp(st, f.feels)
                ),
                &text::CAPTION,
                Rect::new(r.x, r.y + 64.0, r.w, 15.0),
                cv.theme.mist,
                Align::Left,
            );
        }
        // compact stat chips
        let chips = [
            (Icon::Leaf, wind),
            (Icon::Droplet, format!("{:.0}%", f.humidity)),
        ];
        let mut x = r.x;
        for (ic, v) in chips {
            let w = cv.measure(&v, &text::CAPTION) + 26.0;
            let c = Rect::new(x, r.y + 82.0, w, 20.0);
            cv.rrect(c, 10.0, cv.theme.surface);
            cv.icon(ic, c.x + 10.0, c.cy(), 11.0, cv.theme.mist);
            cv.text(
                &v,
                &text::CAPTION,
                Rect::new(c.x + 18.0, c.y, w - 20.0, c.h),
                cv.theme.ink,
                Align::Left,
            );
            x += w + 6.0;
        }
        cv.button(
            Icon::Reset,
            x + 10.0,
            r.y + 92.0,
            20.0,
            Action::WeatherRefresh,
            "Refresh weather · data: MET Norway",
            FAINT,
        );
    });
}

/// The sun's real elevation through today, with the sun drawn where it is right now.
fn sun_path(st: &State, cv: &mut Canvas, r: Rect, t: f32) {
    let (lat, lon) = (st.settings.weather.lat, st.settings.weather.lon);
    let off = crate::util::local_offset_secs();
    let now = unix_secs();
    let day = (now + off).div_euclid(86_400);
    let midnight = day * 86_400 - off;
    let sd = sun::day(lat, lon, day, off);
    cv.sq(r, 16.0, cv.theme.surface);
    let h24 = st.clock_24h();

    let plot = Rect::new(r.x + 12.0, r.y + 22.0, r.w - 24.0, r.h - 44.0);
    let max_e = sun::elevation(lat, lon, sd.noon).max(5.0);
    let min_e = sun::elevation(lat, lon, sd.noon + 12 * 3600).min(-5.0);
    let horizon = plot.y + plot.h * (max_e / (max_e - min_e * 0.45)) as f32;
    let y_of = |e: f64| -> f32 {
        if e >= 0.0 {
            horizon - (horizon - plot.y) * (e / max_e) as f32
        } else {
            horizon + (plot.bottom() - horizon) * (e / min_e) as f32
        }
    };
    let x_of = |unix: i64| plot.x + plot.w * ((unix - midnight) as f32 / 86_400.0);
    let pts: Vec<(f32, f32)> = (0..=96)
        .map(|i| {
            let ts = midnight + i * 900;
            (x_of(ts), y_of(sun::elevation(lat, lon, ts)))
        })
        .collect();
    let factory = cv.gfx.factory.clone();
    // daylight glow under the curve, above the horizon
    let mut day_poly: Vec<(f32, f32)> = pts.iter().map(|&(x, y)| (x, y.min(horizon))).collect();
    day_poly.push((plot.right(), horizon));
    day_poly.push((plot.x, horizon));
    if let Ok(g) = shapes::poly(&factory, &day_poly, true) {
        if let Some(b) = cv.gradient(
            crate::render::canvas::v2(0.0, plot.y),
            crate::render::canvas::v2(0.0, horizon),
            &[(0.0, SUN.with_alpha(0.30)), (1.0, WARM.with_alpha(0.04))],
            1.0,
        ) {
            use windows::core::Interface;
            let brush: windows::Win32::Graphics::Direct2D::ID2D1Brush = b.cast().expect("brush");
            unsafe { cv.ctx.FillGeometry(&g, &brush, None) };
        }
    }
    if let Ok(g) = shapes::poly(&factory, &pts, false) {
        cv.stroke_geo(&g, cv.theme.mist.with_alpha(0.55), 1.4);
    }
    cv.line(
        plot.x,
        horizon,
        plot.right(),
        horizon,
        1.0,
        LINE.with_alpha(3.0),
    );

    // the sun (or the night marker) right now
    let e = sun::elevation(lat, lon, now);
    let (sx, sy) = (x_of(now), y_of(e));
    if e > -0.8 {
        let glow = 1.0 + 0.12 * (t * 2.0).sin();
        if let Some(g) = cv.radial(
            sx,
            sy,
            16.0 * glow,
            16.0 * glow,
            SUN.with_alpha(0.45),
            SUN.with_alpha(0.0),
        ) {
            unsafe {
                cv.ctx.FillEllipse(
                    &windows::Win32::Graphics::Direct2D::D2D1_ELLIPSE {
                        point: crate::render::canvas::v2(sx, sy),
                        radiusX: 16.0 * glow,
                        radiusY: 16.0 * glow,
                    },
                    &g,
                )
            };
        }
        cv.circle(sx, sy, 6.0, SUN);
    } else {
        cv.circle(sx, sy, 4.5, cv.theme.ink.with_alpha(0.9));
        cv.circle(sx + 2.0, sy - 1.5, 4.0, cv.theme.surface);
    }

    // labels
    let label = |unix: Option<i64>| {
        unix.map(|u| fmt_hm(unix_to_local(u).minutes_of_day(), h24))
            .unwrap_or_else(|| "—".into())
    };
    let small = text::CAPTION.size(10.5);
    cv.icon(Icon::Sun, r.x + 18.0, r.bottom() - 12.0, 11.0, WARM);
    cv.text(
        &format!("{} {}", crate::i18n::tr("Sunrise"), label(sd.rise)),
        &small,
        Rect::new(r.x + 28.0, r.bottom() - 20.0, 120.0, 16.0),
        cv.theme.mist,
        Align::Left,
    );
    cv.text(
        &format!("{} {}", crate::i18n::tr("Sunset"), label(sd.set)),
        &small,
        Rect::new(r.right() - 132.0, r.bottom() - 20.0, 120.0, 16.0),
        cv.theme.mist,
        Align::Right,
    );
    let headline = match (sd.rise, sd.set) {
        (Some(rise), Some(_)) if now < rise => {
            format!("{} {}", crate::i18n::tr("Sunrise in"), dur(rise - now))
        }
        (Some(_), Some(set)) if now < set => {
            format!("{} {}", dur(set - now), crate::i18n::tr("of daylight left"))
        }
        (Some(_), Some(_)) => {
            let next = sun::day(lat, lon, day + 1, off);
            match next.rise {
                Some(nr) => format!("{} {}", crate::i18n::tr("Sunrise in"), dur(nr - now)),
                None => crate::i18n::tr("Polar night"),
            }
        }
        _ if e > 0.0 => crate::i18n::tr("Midnight sun"),
        _ => crate::i18n::tr("Polar night"),
    };
    cv.text(
        &headline,
        &text::CAPTION.weight(600),
        Rect::new(r.x + 12.0, r.y + 4.0, r.w - 24.0, 16.0),
        cv.theme.ink,
        Align::Left,
    );
    if let Some(d) = sd.daylight_secs() {
        cv.text(
            &format!("{} {}", crate::i18n::tr("Daylight"), dur(d)),
            &small,
            Rect::new(r.x + 12.0, r.y + 4.0, r.w - 24.0, 16.0),
            cv.theme.mist,
            Align::Right,
        );
    }
}

fn dur(secs: i64) -> String {
    let m = (secs.max(0) + 59) / 60;
    if m >= 60 {
        format!("{}h {:02}m", m / 60, m % 60)
    } else {
        format!("{m}m")
    }
}

/// 24-hour temperature curve with conditions every three hours and rain bars.
fn hours(st: &State, cv: &mut Canvas, r: Rect, f: &Forecast, reveal: f32) {
    let hrs: Vec<_> = f.hours.iter().take(25).collect();
    if hrs.len() < 2 {
        return;
    }
    let h24 = st.clock_24h();
    cv.sq(r, 16.0, cv.theme.surface);
    let plot = Rect::new(r.x + 14.0, r.y + 30.0, r.w - 28.0, r.h - 62.0);
    let temps: Vec<f32> = hrs.iter().map(|h| temp(st, h.temp)).collect();
    let lo = temps.iter().copied().fold(f32::MAX, f32::min);
    let hi = temps.iter().copied().fold(f32::MIN, f32::max);
    let span = (hi - lo).max(4.0);
    let n = hrs.len() - 1;
    let pt = |i: usize| {
        (
            plot.x + plot.w * i as f32 / n as f32,
            plot.bottom() - plot.h * ((temps[i] - lo) / span),
        )
    };
    let pts: Vec<(f32, f32)> = (0..=n).map(pt).collect();
    // draw in from the left as the page reveals
    let shown = plot.x + plot.w * reveal.clamp(0.0, 1.0);
    cv.push_clip(Rect::new(r.x, r.y, shown - r.x + 14.0, r.h));
    let factory = cv.gfx.factory.clone();
    let mut fill = pts.clone();
    fill.push((plot.right(), plot.bottom() + 6.0));
    fill.push((plot.x, plot.bottom() + 6.0));
    if let Ok(g) = shapes::poly(&factory, &fill, true) {
        if let Some(b) = cv.gradient(
            crate::render::canvas::v2(0.0, plot.y),
            crate::render::canvas::v2(0.0, plot.bottom() + 6.0),
            &[(0.0, WARM.with_alpha(0.28)), (1.0, COOL.with_alpha(0.0))],
            1.0,
        ) {
            use windows::core::Interface;
            let brush: windows::Win32::Graphics::Direct2D::ID2D1Brush = b.cast().expect("brush");
            unsafe { cv.ctx.FillGeometry(&g, &brush, None) };
        }
    }
    if let Ok(g) = shapes::poly(&factory, &pts, false) {
        match cv.gradient(
            crate::render::canvas::v2(0.0, plot.y),
            crate::render::canvas::v2(0.0, plot.bottom()),
            &[(0.0, WARM), (1.0, COOL)],
            1.0,
        ) {
            Some(b) => {
                use windows::core::Interface;
                cv.stroke_geo_brush(&g, &b.cast().expect("brush"), 2.2)
            }
            None => cv.stroke_geo(&g, WARM, 2.2),
        }
    }
    // rain: thin bars along the bottom, height by probability (or amount)
    let bar_w = (plot.w / n as f32 * 0.45).max(2.0);
    for (i, h) in hrs.iter().enumerate().take(n) {
        let p = h
            .precip_prob
            .map(|p| p / 100.0)
            .unwrap_or((h.precip / 2.0).min(1.0));
        if p < 0.05 {
            continue;
        }
        let x = plot.x + plot.w * (i as f32 + 0.5) / n as f32;
        let bh = 12.0 * p;
        cv.rrect(
            Rect::new(x - bar_w / 2.0, r.bottom() - 20.0 - bh, bar_w, bh),
            1.0,
            COOL.with_alpha(0.35 + 0.5 * p),
        );
    }
    // conditions + temperature every 3 hours, hour labels underneath
    for i in (0..=n).step_by(3) {
        let (x, y) = pts[i];
        cv.circle(x, y, 2.6, cv.theme.ink);
        cv.text(
            &format!("{:.0}°", temps[i]),
            &text::CAPTION.weight(600).size(10.5),
            Rect::new(x - 18.0, y - 20.0, 36.0, 14.0),
            cv.theme.ink,
            Align::Center,
        );
        cv.icon(icon_for(&hrs[i].symbol), x, r.y + 14.0, 13.0, cv.theme.mist);
        let lt = unix_to_local(hrs[i].time);
        let lbl = if i == 0 {
            crate::i18n::tr("Now")
        } else if h24 {
            format!("{:02}", lt.hour)
        } else {
            let h = lt.hour % 12;
            format!(
                "{}{}",
                if h == 0 { 12 } else { h },
                if lt.hour < 12 { "a" } else { "p" }
            )
        };
        cv.text(
            &lbl,
            &text::CAPTION.size(10.0),
            Rect::new(x - 18.0, r.bottom() - 16.0, 36.0, 12.0),
            if i == 0 { cv.theme.ink } else { FAINT },
            Align::Center,
        );
    }
    cv.pop_clip();
}
