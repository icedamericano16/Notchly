//! Weather: today in detail. Current conditions, a live sun path (the sun sits where it
//! really is in the sky right now, computed from your location) and a 24-hour temperature
//! curve with conditions and rain.

use super::dashboard::temp;
use super::{row, Reveal};
use crate::app::state::State;
use crate::app::Action;
use crate::features::sun;
use crate::render::glass::GlassLayer;
use crate::render::icons::Icon;
use crate::render::text::{self, Align};
use crate::render::{shapes, Canvas, Rect};
use crate::services::weather::{describe, icon_for, Forecast};
use crate::theme::tokens::*;
use crate::util::{fmt_hm, place_offset_secs, unix_secs, unix_to_zone};

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

/// What the sky looks like right now (drives the animated background).
#[derive(Clone, Copy, PartialEq)]
enum Sky {
    Clear,
    Cloudy,
    Rain,
    Snow,
    Storm,
    Fog,
}

fn sky_of(symbol: &str) -> Sky {
    if symbol.contains("thunder") {
        Sky::Storm
    } else if symbol.contains("snow") || symbol.contains("sleet") {
        Sky::Snow
    } else if symbol.contains("rain") || symbol.contains("showers") {
        Sky::Rain
    } else if symbol.contains("fog") {
        Sky::Fog
    } else if symbol.contains("cloudy") {
        Sky::Cloudy
    } else {
        Sky::Clear
    }
}

/// Small deterministic noise for particle placement.
fn hash(i: u32, salt: u32) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xFFFF) as f32 / 65535.0
}

/// A live sky behind the current conditions: gradient by condition and time of day, with
/// sun rays, drifting clouds, rain, snow, lightning, mist or twinkling stars.
fn sky_scene(cv: &mut Canvas, r: Rect, sky: Sky, night: bool, t: f32) {
    let (top, bottom) = match (sky, night) {
        (Sky::Clear, false) => (0x2563C9, 0x5B9BE8),
        (Sky::Clear, true) => (0x0B1433, 0x1C2A5C),
        (Sky::Cloudy, false) => (0x4A5A70, 0x7D8DA3),
        (Sky::Cloudy, true) => (0x161C2B, 0x2A3346),
        (Sky::Rain, _) | (Sky::Storm, _) => (0x1F2733, 0x3A4656),
        (Sky::Snow, _) => (0x5C6D85, 0x9AAAC0),
        (Sky::Fog, _) => (0x4F5864, 0x7A838E),
    };
    if let Some(g) = cv.gradient(
        crate::render::canvas::v2(0.0, r.y),
        crate::render::canvas::v2(0.0, r.bottom()),
        &[(0.0, Color::rgb(top)), (1.0, Color::rgb(bottom))],
        1.0,
    ) {
        use windows::core::Interface;
        cv.rrect_brush(r, 16.0, &g.cast().expect("brush"));
    }
    cv.push_clip(r.inset(1.0));
    match (sky, night) {
        (Sky::Clear, false) => {
            // sun glow with slowly turning rays in the corner
            let (sx, sy) = (r.right() - 26.0, r.y + 22.0);
            if let Some(g) = cv.radial(
                sx,
                sy,
                60.0,
                60.0,
                SUN.with_alpha(0.55),
                SUN.with_alpha(0.0),
            ) {
                unsafe {
                    cv.ctx.FillEllipse(
                        &windows::Win32::Graphics::Direct2D::D2D1_ELLIPSE {
                            point: crate::render::canvas::v2(sx, sy),
                            radiusX: 60.0,
                            radiusY: 60.0,
                        },
                        &g,
                    )
                };
            }
            for i in 0..10 {
                let a = t * 0.15 + i as f32 * std::f32::consts::TAU / 10.0;
                let (s, c) = a.sin_cos();
                cv.line(
                    sx + 18.0 * c,
                    sy + 18.0 * s,
                    sx + 34.0 * c,
                    sy + 34.0 * s,
                    2.0,
                    SUN.with_alpha(0.35),
                );
            }
            cv.circle(sx, sy, 12.0, SUN);
        }
        (_, true) if sky != Sky::Rain && sky != Sky::Storm => {
            // stars twinkle, a crescent moon hangs in the corner
            for i in 0..22u32 {
                let x = r.x + hash(i, 1) * r.w;
                let y = r.y + hash(i, 2) * r.h * 0.7;
                let tw = 0.35 + 0.65 * (t * (1.0 + hash(i, 3) * 2.0) + i as f32).sin().abs();
                cv.circle(x, y, 0.6 + hash(i, 4) * 0.9, WHITE.with_alpha(0.75 * tw));
            }
            let (mx, my) = (r.right() - 26.0, r.y + 22.0);
            cv.circle(mx, my, 10.0, Color::rgb(0xF2F0E6));
            cv.circle(
                mx + 4.5,
                my - 3.0,
                9.0,
                Color::rgb(top).lerp(Color::rgb(bottom), 0.15),
            );
        }
        _ => {}
    }
    if matches!(sky, Sky::Cloudy | Sky::Rain | Sky::Storm | Sky::Snow) {
        // drifting cloud puffs
        for i in 0..3u32 {
            let speed = 6.0 + hash(i, 5) * 6.0;
            let w = 70.0 + hash(i, 6) * 40.0;
            let x = r.x - w + ((t * speed + hash(i, 7) * 400.0) % (r.w + w * 2.0));
            let y = r.y + 10.0 + i as f32 * 16.0;
            let c = WHITE.with_alpha(if night { 0.08 } else { 0.16 });
            cv.circle(x + w * 0.3, y + 10.0, w * 0.22, c);
            cv.circle(x + w * 0.55, y + 4.0, w * 0.28, c);
            cv.circle(x + w * 0.8, y + 11.0, w * 0.2, c);
        }
    }
    match sky {
        Sky::Rain | Sky::Storm => {
            for i in 0..34u32 {
                let speed = 160.0 + hash(i, 8) * 80.0;
                let x0 = r.x + hash(i, 9) * (r.w + 20.0);
                let y = r.y + ((t * speed + hash(i, 10) * r.h) % (r.h + 14.0)) - 14.0;
                let x = x0 - (y - r.y) * 0.18;
                cv.line(
                    x,
                    y,
                    x - 2.0,
                    y + 9.0,
                    1.1,
                    Color::rgb(0xA9C8FF).with_alpha(0.55),
                );
            }
            if sky == Sky::Storm {
                // a lightning flash every few seconds
                let cyc = t % 5.3;
                if cyc < 0.12 || (0.2..0.28).contains(&cyc) {
                    cv.sq(r, 16.0, WHITE.with_alpha(0.22));
                }
            }
        }
        Sky::Snow => {
            for i in 0..26u32 {
                let speed = 14.0 + hash(i, 11) * 14.0;
                let y = r.y + ((t * speed + hash(i, 12) * r.h) % (r.h + 6.0)) - 6.0;
                let x = r.x + hash(i, 13) * r.w + (t * 0.9 + i as f32).sin() * 4.0;
                cv.circle(x, y, 1.0 + hash(i, 14) * 1.2, WHITE.with_alpha(0.85));
            }
        }
        Sky::Fog => {
            for i in 0..4u32 {
                let y = r.y + 20.0 + i as f32 * 20.0;
                let x = r.x - 60.0 + ((t * (8.0 + i as f32 * 3.0)) % (r.w + 60.0));
                cv.rrect(Rect::new(x, y, 120.0, 8.0), 4.0, WHITE.with_alpha(0.10));
                cv.rrect(
                    Rect::new(x - r.w, y + 6.0, 140.0, 6.0),
                    3.0,
                    WHITE.with_alpha(0.07),
                );
            }
        }
        _ => {}
    }
    cv.pop_clip();
}

fn current(st: &State, cv: &mut Canvas, r: Rect, f: &Forecast, t: f32, rv: &Reveal) {
    let wind = if st.settings.weather.units == crate::settings::Units::Imperial {
        format!("{:.0} mph", f.wind * 2.237)
    } else {
        format!("{:.0} km/h", f.wind * 3.6)
    };
    let today = f.days.first();
    let (lat, lon) = (st.settings.weather.lat, st.settings.weather.lon);
    let night = sun::elevation(lat, lon, unix_secs()) < -0.8;
    let sky = sky_of(&f.symbol);
    let city = st.settings.weather.city.clone();
    let zone = st.settings.weather.timezone.clone();
    let age = (unix_secs() - f.fetched).max(0) / 60;
    row(cv, rv, 0, r, |cv| {
        sky_scene(cv, r, sky, night, t);
        let ink = WHITE;
        let ic = icon_for(&f.symbol);
        cv.icon(ic, r.x + 22.0, r.y + 24.0, 26.0, ink);
        cv.text(
            &format!("{:.0}°", temp(st, f.temp)),
            &text::DISPLAY.size(36.0),
            Rect::new(r.x + 40.0, r.y + 4.0, 90.0, 40.0),
            ink,
            Align::Left,
        );
        cv.label(
            describe(&f.symbol),
            &text::BODY_MED,
            Rect::new(r.x + 12.0, r.y + 46.0, r.w - 24.0, 18.0),
            ink,
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
                Rect::new(r.x + 12.0, r.y + 63.0, r.w - 24.0, 15.0),
                WHITE.with_alpha(0.85),
                Align::Left,
            );
        }
        let place = match (city.is_empty(), crate::util::zone_label(&zone)) {
            (true, _) => format!("{} {wind} · {:.0}%", crate::i18n::tr("Wind"), f.humidity),
            // times on this page are the city's; say so when that isn't this PC's clock
            (false, Some(z)) => format!("{city} · {z} · {wind}"),
            (false, None) => format!("{city} · {wind}"),
        };
        cv.text(
            &place,
            &text::CAPTION.size(10.5),
            Rect::new(r.x + 12.0, r.y + 82.0, r.w - 60.0, 14.0),
            WHITE.with_alpha(0.72),
            Align::Left,
        );
        let upd = if age < 1 {
            crate::i18n::tr("now")
        } else {
            format!("{age}m")
        };
        cv.text(
            &upd,
            &text::CAPTION.size(10.0),
            Rect::new(r.right() - 64.0, r.y + 82.0, 30.0, 14.0),
            WHITE.with_alpha(0.6),
            Align::Right,
        );
        cv.button(
            Icon::Reset,
            r.right() - 18.0,
            r.y + 89.0,
            20.0,
            Action::WeatherRefresh,
            "Refresh weather · data: MET Norway",
            WHITE.with_alpha(0.75),
        );
    });
}

/// The sun's real elevation through today, with the sun drawn where it is right now.
fn sun_path(st: &State, cv: &mut Canvas, r: Rect, t: f32) {
    let (lat, lon) = (st.settings.weather.lat, st.settings.weather.lon);
    // the chosen city's own clock, not this PC's
    let off = place_offset_secs(&st.settings.weather.timezone);
    let now = unix_secs();
    let day = (now + off).div_euclid(86_400);
    let midnight = day * 86_400 - off;
    let sd = sun::day(lat, lon, day, off);
    cv.glass_tile(r, 16.0, GlassLayer::Base);
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

    // hover: read the sun's position at any time of day
    if let Some((px, py)) = cv.pointer() {
        if r.contains(px, py) {
            let frac = ((px - plot.x) / plot.w).clamp(0.0, 1.0);
            let ts = midnight + (frac * 86_400.0) as i64;
            let he = sun::elevation(lat, lon, ts);
            let (gx, gy) = (x_of(ts), y_of(he));
            cv.line(
                gx,
                plot.y - 2.0,
                gx,
                plot.bottom(),
                1.0,
                LINE.with_alpha(4.0),
            );
            cv.circle(
                gx,
                gy,
                4.0,
                if he > 0.0 {
                    SUN.with_alpha(0.9)
                } else {
                    cv.theme.mist
                },
            );
            let s = format!(
                "{} · {} {:+.0}°",
                fmt_hm(unix_to_zone(ts, off).minutes_of_day(), h24),
                crate::i18n::tr("Sun"),
                he
            );
            let w = cv.measure(&s, &text::CAPTION.weight(600)) + 16.0;
            let bx = (gx - w / 2.0).clamp(r.x + 4.0, r.right() - w - 4.0);
            let by = (gy - 30.0).max(r.y + 2.0);
            let b = Rect::new(bx, by, w, 20.0);
            cv.glass_tile(b, 10.0, GlassLayer::Raised);
            cv.text(
                &s,
                &text::CAPTION.weight(600),
                b,
                cv.theme.ink,
                Align::Center,
            );
        }
    }

    // labels
    let label = |unix: Option<i64>| {
        unix.map(|u| fmt_hm(unix_to_zone(u, off).minutes_of_day(), h24))
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
    let off = place_offset_secs(&st.settings.weather.timezone);
    let hrs: Vec<_> = f.hours.iter().take(25).collect();
    if hrs.len() < 2 {
        return;
    }
    let h24 = st.clock_24h();
    cv.glass_tile(r, 16.0, GlassLayer::Base);
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
        let lt = unix_to_zone(hrs[i].time, off);
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
    // hover scrubber: details for the hour under the pointer
    if let Some((px, py)) = cv.pointer() {
        if r.contains(px, py) && px >= plot.x - 6.0 && px <= plot.right() + 6.0 {
            let i = (((px - plot.x) / plot.w) * n as f32)
                .round()
                .clamp(0.0, n as f32) as usize;
            let (x, y) = pts[i];
            let h = hrs[i];
            cv.line(
                x,
                r.y + 24.0,
                x,
                r.bottom() - 18.0,
                1.0,
                LINE.with_alpha(5.0),
            );
            cv.circle(x, y, 5.5, cv.theme.surface);
            cv.circle(x, y, 4.0, WARM.lerp(COOL, (y - plot.y) / plot.h.max(1.0)));
            let wind = if st.settings.weather.units == crate::settings::Units::Imperial {
                format!("{:.0} mph", h.wind * 2.237)
            } else {
                format!("{:.0} km/h", h.wind * 3.6)
            };
            let rain = h
                .precip_prob
                .map(|p| format!("{p:.0}%"))
                .unwrap_or_else(|| format!("{:.1} mm", h.precip));
            let lt = unix_to_zone(h.time, off);
            let line1 = format!(
                "{}  {:.0}°  {}",
                fmt_hm(lt.minutes_of_day(), h24),
                temps[i],
                crate::i18n::tr(describe(&h.symbol))
            );
            let line2 = format!(
                "{} {rain} · {} {wind} · {:.0}%",
                crate::i18n::tr("Rain"),
                crate::i18n::tr("Wind"),
                h.humidity
            );
            let w = cv
                .measure(&line1, &text::CAPTION.weight(700))
                .max(cv.measure(&line2, &text::CAPTION))
                + 34.0;
            let bx = (x - w / 2.0).clamp(r.x + 4.0, r.right() - w - 4.0);
            let by = if y - plot.y > 40.0 {
                y - 46.0
            } else {
                y + 12.0
            };
            let b = Rect::new(bx, by, w, 36.0);
            cv.glass_tile(b, 10.0, GlassLayer::Raised);
            cv.stroke_rrect(b.inset(0.5), 9.5, RING, 1.0);
            cv.icon(icon_for(&h.symbol), b.x + 14.0, b.cy(), 14.0, cv.theme.ink);
            cv.text(
                &line1,
                &text::CAPTION.weight(700),
                Rect::new(b.x + 26.0, b.y + 3.0, w - 30.0, 15.0),
                cv.theme.ink,
                Align::Left,
            );
            cv.text(
                &line2,
                &text::CAPTION.size(10.0),
                Rect::new(b.x + 26.0, b.y + 18.0, w - 30.0, 14.0),
                cv.theme.mist,
                Align::Left,
            );
        }
    }
    cv.pop_clip();
}
