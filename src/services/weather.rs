//! Weather from MET Norway Locationforecast 2.0 (compact), with Expires /
//! If-Modified-Since handling, and Open-Meteo geocoding for city search.

use serde::{Deserialize, Serialize};

use super::{http, spawn, Bus, Event};
use crate::util::{load_json, save_json, unix_secs};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Hour {
    /// Unix seconds.
    pub time: i64,
    pub temp: f32,
    pub symbol: String,
    pub precip: f32,
    pub precip_prob: Option<f32>,
    /// Wind speed, m/s.
    #[serde(default)]
    pub wind: f32,
    /// Relative humidity, %.
    #[serde(default)]
    pub humidity: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Day {
    /// Local day number.
    pub day: i64,
    pub min: f32,
    pub max: f32,
    pub symbol: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Forecast {
    pub temp: f32,
    pub feels: f32,
    pub wind: f32,
    pub humidity: f32,
    pub symbol: String,
    pub precip_next_hour: f32,
    pub hours: Vec<Hour>,
    pub days: Vec<Day>,
    pub fetched: i64,
    pub expires: i64,
    pub last_modified: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Place {
    pub name: String,
    pub country: String,
    pub lat: f64,
    pub lon: f64,
}

#[derive(Deserialize)]
struct Met {
    properties: MetProps,
}
#[derive(Deserialize)]
struct MetProps {
    timeseries: Vec<MetTs>,
}
#[derive(Deserialize)]
struct MetTs {
    time: String,
    data: MetData,
}
#[derive(Deserialize)]
struct MetData {
    instant: MetInstant,
    next_1_hours: Option<MetNext>,
    next_6_hours: Option<MetNext>,
}
#[derive(Deserialize)]
struct MetInstant {
    details: MetDetails,
}
#[derive(Deserialize, Default)]
struct MetDetails {
    air_temperature: Option<f32>,
    wind_speed: Option<f32>,
    relative_humidity: Option<f32>,
    precipitation_amount: Option<f32>,
    probability_of_precipitation: Option<f32>,
}
#[derive(Deserialize)]
struct MetNext {
    summary: MetSummary,
    #[serde(default)]
    details: MetDetails,
}
#[derive(Deserialize)]
struct MetSummary {
    symbol_code: String,
}

/// Parses "2026-11-22T10:00:00Z" to unix seconds.
pub fn parse_iso(s: &str) -> Option<i64> {
    let y: i32 = s.get(0..4)?.parse().ok()?;
    let mo: u32 = s.get(5..7)?.parse().ok()?;
    let d: u32 = s.get(8..10)?.parse().ok()?;
    let h: i64 = s.get(11..13).and_then(|x| x.parse().ok()).unwrap_or(0);
    let mi: i64 = s.get(14..16).and_then(|x| x.parse().ok()).unwrap_or(0);
    let se: i64 = s.get(17..19).and_then(|x| x.parse().ok()).unwrap_or(0);
    let mut t = crate::util::days_from_civil(y, mo, d) * 86400 + h * 3600 + mi * 60 + se;
    // offset suffix (+05:45 / -03:00)
    if let Some(rest) = s.get(19..) {
        let rest = rest.trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
        if let Some(sign) = rest.chars().next().filter(|c| *c == '+' || *c == '-') {
            let oh: i64 = rest.get(1..3).and_then(|x| x.parse().ok()).unwrap_or(0);
            let om: i64 = rest.get(4..6).and_then(|x| x.parse().ok()).unwrap_or(0);
            let off = oh * 3600 + om * 60;
            t -= if sign == '+' { off } else { -off };
        }
    }
    Some(t)
}

/// Feels-like: wind chill below 10 °C, heat index above 27 °C.
pub fn feels_like(t: f32, wind_ms: f32, rh: f32) -> f32 {
    let v = wind_ms * 3.6;
    if t <= 10.0 && v > 4.8 {
        13.12 + 0.6215 * t - 11.37 * v.powf(0.16) + 0.3965 * t * v.powf(0.16)
    } else if t >= 27.0 && rh >= 40.0 {
        let f = t * 9.0 / 5.0 + 32.0;
        let hi = -42.379 + 2.049_015_3 * f + 10.143_332 * rh
            - 0.224_755_4 * f * rh
            - 0.00683783 * f * f
            - 0.05481717 * rh * rh
            + 0.00122874 * f * f * rh
            + 0.00085282 * f * rh * rh
            - 0.00000199 * f * f * rh * rh;
        (hi - 32.0) * 5.0 / 9.0
    } else {
        t
    }
}

fn build(m: Met, last_modified: String, expires: i64) -> Forecast {
    let off = crate::util::local_offset_secs();
    let ts = &m.properties.timeseries;
    let mut f = Forecast {
        fetched: unix_secs(),
        expires,
        last_modified,
        ..Default::default()
    };
    if let Some(first) = ts.first() {
        let d = &first.data.instant.details;
        f.temp = d.air_temperature.unwrap_or(0.0);
        f.wind = d.wind_speed.unwrap_or(0.0);
        f.humidity = d.relative_humidity.unwrap_or(0.0);
        f.feels = feels_like(f.temp, f.wind, f.humidity);
        if let Some(n) = &first.data.next_1_hours {
            f.symbol = n.summary.symbol_code.clone();
            f.precip_next_hour = n.details.precipitation_amount.unwrap_or(0.0);
        } else if let Some(n) = &first.data.next_6_hours {
            f.symbol = n.summary.symbol_code.clone();
        }
    }
    for t in ts.iter().take(25) {
        let Some(time) = parse_iso(&t.time) else {
            continue;
        };
        let n = t
            .data
            .next_1_hours
            .as_ref()
            .or(t.data.next_6_hours.as_ref());
        f.hours.push(Hour {
            time,
            temp: t.data.instant.details.air_temperature.unwrap_or(0.0),
            symbol: n.map(|n| n.summary.symbol_code.clone()).unwrap_or_default(),
            precip: n
                .and_then(|n| n.details.precipitation_amount)
                .unwrap_or(0.0),
            precip_prob: n.and_then(|n| n.details.probability_of_precipitation),
            wind: t.data.instant.details.wind_speed.unwrap_or(0.0),
            humidity: t.data.instant.details.relative_humidity.unwrap_or(0.0),
        });
    }
    for t in ts {
        let Some(time) = parse_iso(&t.time) else {
            continue;
        };
        let day = (time + off).div_euclid(86400);
        let temp = t.data.instant.details.air_temperature.unwrap_or(0.0);
        let local_hour = (time + off).rem_euclid(86400) / 3600;
        match f.days.iter_mut().find(|d| d.day == day) {
            Some(d) => {
                d.min = d.min.min(temp);
                d.max = d.max.max(temp);
                if (11..=14).contains(&local_hour) {
                    if let Some(n) = t
                        .data
                        .next_6_hours
                        .as_ref()
                        .or(t.data.next_1_hours.as_ref())
                    {
                        d.symbol = n.summary.symbol_code.clone();
                    }
                }
            }
            None => {
                if f.days.len() < 7 {
                    let sym = t
                        .data
                        .next_6_hours
                        .as_ref()
                        .or(t.data.next_1_hours.as_ref())
                        .map(|n| n.summary.symbol_code.clone())
                        .unwrap_or_default();
                    f.days.push(Day {
                        day,
                        min: temp,
                        max: temp,
                        symbol: sym,
                    });
                }
            }
        }
    }
    f
}

/// Fetches the forecast (respecting Expires) on a worker.
pub fn fetch(bus: Bus, lat: f64, lon: f64, force: bool) {
    spawn("weather", move || {
        super::com_mta();
        let (lat, lon) = ((lat * 100.0).round() / 100.0, (lon * 100.0).round() / 100.0);
        let cached: Forecast = load_json("cache/weather.json");
        if !force && cached.expires > unix_secs() && !cached.hours.is_empty() {
            bus.send(Event::Weather(Ok(cached)));
            return;
        }
        let url = format!(
            "https://api.met.no/weatherapi/locationforecast/2.0/compact?lat={lat}&lon={lon}"
        );
        let ims = cached.last_modified.clone();
        let headers: Vec<(&str, &str)> = if !ims.is_empty() && !cached.hours.is_empty() {
            vec![("If-Modified-Since", ims.as_str())]
        } else {
            vec![]
        };
        let result = match http::get(&url, &headers) {
            Ok(r) if r.status == 304 => Ok(cached),
            Ok(r) if r.ok() => {
                let lm = r.header("Last-Modified").unwrap_or("").to_string();
                let exp = r
                    .header("Expires")
                    .and_then(parse_http_date)
                    .unwrap_or(unix_secs() + 1800);
                match serde_json::from_str::<Met>(&r.body) {
                    Ok(m) => {
                        let f = build(m, lm, exp);
                        let _ = crate::util::sub_dir("cache");
                        save_json("cache/weather.json", &f);
                        Ok(f)
                    }
                    Err(e) => Err(format!("bad forecast: {e}")),
                }
            }
            Ok(r) => Err(format!("HTTP {}", r.status)),
            Err(e) => Err(e),
        };
        bus.send(Event::Weather(result));
    });
}

/// RFC 1123 date ("Sun, 22 Nov 2026 10:30:00 GMT").
pub fn parse_http_date(s: &str) -> Option<i64> {
    let p: Vec<&str> = s.split_whitespace().collect();
    if p.len() < 5 {
        return None;
    }
    let d: u32 = p[1].parse().ok()?;
    let m = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ]
    .iter()
    .position(|x| *x == p[2])? as u32
        + 1;
    let y: i32 = p[3].parse().ok()?;
    let hms: Vec<i64> = p[4].split(':').filter_map(|x| x.parse().ok()).collect();
    if hms.len() != 3 {
        return None;
    }
    Some(crate::util::days_from_civil(y, m, d) * 86400 + hms[0] * 3600 + hms[1] * 60 + hms[2])
}

#[derive(Deserialize)]
struct GeoResp {
    #[serde(default)]
    results: Vec<GeoItem>,
}
#[derive(Deserialize)]
struct GeoItem {
    name: String,
    #[serde(default)]
    country: String,
    latitude: f64,
    longitude: f64,
}

pub fn geocode(bus: Bus, query: String) {
    spawn("geocode", move || {
        super::com_mta();
        let url = format!("https://geocoding-api.open-meteo.com/v1/search?name={}&count=5&language=en&format=json", http::enc(&query));
        let r = http::get(&url, &[])
            .and_then(|r| serde_json::from_str::<GeoResp>(&r.body).map_err(|e| e.to_string()))
            .map(|g| {
                g.results
                    .into_iter()
                    .map(|i| Place {
                        name: i.name,
                        country: i.country,
                        lat: i.latitude,
                        lon: i.longitude,
                    })
                    .collect()
            });
        bus.send(Event::Geocode(r));
    });
}

/// Maps a MET symbol code to an icon.
pub fn icon_for(symbol: &str) -> crate::render::icons::Icon {
    use crate::render::icons::Icon;
    let s = symbol;
    if s.contains("thunder") {
        Icon::Storm
    } else if s.contains("snow") || s.contains("sleet") {
        Icon::Snow
    } else if s.contains("rain") || s.contains("showers") {
        Icon::Rain
    } else if s.contains("fog") {
        Icon::Fog
    } else if s.ends_with("_night") && (s.starts_with("clearsky") || s.starts_with("fair")) {
        Icon::MoonStar
    } else if s.starts_with("partlycloudy") && s.ends_with("_night") {
        Icon::Cloud
    } else if s.starts_with("partlycloudy") || s.starts_with("fair") {
        Icon::CloudSun
    } else if s.contains("cloudy") {
        Icon::Cloud
    } else if s.starts_with("clearsky") {
        Icon::Sun
    } else {
        Icon::CloudSun
    }
}

pub fn describe(symbol: &str) -> &'static str {
    let s = symbol;
    if s.contains("thunder") {
        "Thunder"
    } else if s.contains("heavysnow") {
        "Heavy snow"
    } else if s.contains("snow") {
        "Snow"
    } else if s.contains("sleet") {
        "Sleet"
    } else if s.contains("heavyrain") {
        "Heavy rain"
    } else if s.contains("lightrain") {
        "Light rain"
    } else if s.contains("rain") || s.contains("showers") {
        "Rain"
    } else if s.contains("fog") {
        "Fog"
    } else if s.starts_with("partlycloudy") {
        "Partly cloudy"
    } else if s.starts_with("fair") {
        "Fair"
    } else if s.contains("cloudy") {
        "Cloudy"
    } else if s.starts_with("clearsky") {
        "Clear"
    } else {
        "—"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn iso_and_http_dates() {
        assert_eq!(parse_iso("1970-01-02T00:00:00Z"), Some(86400));
        assert_eq!(parse_iso("1970-01-01T05:45:00+05:45"), Some(0));
        assert_eq!(parse_http_date("Thu, 01 Jan 1970 00:01:00 GMT"), Some(60));
    }
    #[test]
    fn feels() {
        assert!(feels_like(0.0, 10.0, 50.0) < -3.0);
        assert_eq!(feels_like(18.0, 1.0, 50.0), 18.0);
    }
}
