//! Prayer times computed locally with the standard astronomical algorithm
//! (solar declination + equation of time), 12 methods, Shafi/Hanafi Asr and
//! high-latitude adjustment.

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IshaRule {
    Angle(f64),
    MinutesAfterMaghrib(f64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Method {
    pub id: &'static str,
    pub name: &'static str,
    pub fajr: f64,
    pub isha: IshaRule,
    /// Maghrib angle (Tehran) or None = sunset.
    pub maghrib: Option<f64>,
}

pub const METHODS: &[Method] = &[
    Method {
        id: "MWL",
        name: "Muslim World League",
        fajr: 18.0,
        isha: IshaRule::Angle(17.0),
        maghrib: None,
    },
    Method {
        id: "ISNA",
        name: "ISNA (North America)",
        fajr: 15.0,
        isha: IshaRule::Angle(15.0),
        maghrib: None,
    },
    Method {
        id: "Egypt",
        name: "Egyptian Authority",
        fajr: 19.5,
        isha: IshaRule::Angle(17.5),
        maghrib: None,
    },
    Method {
        id: "UmmAlQura",
        name: "Umm al-Qura, Makkah",
        fajr: 18.5,
        isha: IshaRule::MinutesAfterMaghrib(90.0),
        maghrib: None,
    },
    Method {
        id: "Karachi",
        name: "Karachi",
        fajr: 18.0,
        isha: IshaRule::Angle(18.0),
        maghrib: None,
    },
    Method {
        id: "Dubai",
        name: "Dubai",
        fajr: 18.2,
        isha: IshaRule::Angle(18.2),
        maghrib: None,
    },
    Method {
        id: "Tehran",
        name: "Tehran",
        fajr: 17.7,
        isha: IshaRule::Angle(14.0),
        maghrib: Some(4.5),
    },
    Method {
        id: "Kuwait",
        name: "Kuwait",
        fajr: 18.0,
        isha: IshaRule::Angle(17.5),
        maghrib: None,
    },
    Method {
        id: "Qatar",
        name: "Qatar",
        fajr: 18.0,
        isha: IshaRule::MinutesAfterMaghrib(90.0),
        maghrib: None,
    },
    Method {
        id: "Singapore",
        name: "Singapore",
        fajr: 20.0,
        isha: IshaRule::Angle(18.0),
        maghrib: None,
    },
    Method {
        id: "Turkey",
        name: "Turkey (Diyanet)",
        fajr: 18.0,
        isha: IshaRule::Angle(17.0),
        maghrib: None,
    },
    Method {
        id: "Moonsighting",
        name: "Moonsighting Committee",
        fajr: 18.0,
        isha: IshaRule::Angle(18.0),
        maghrib: None,
    },
];

pub fn method(id: &str) -> Method {
    METHODS
        .iter()
        .copied()
        .find(|m| m.id.eq_ignore_ascii_case(id))
        .unwrap_or(METHODS[0])
}

/// Times in minutes after local midnight (may be NaN if undefined).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Times {
    pub fajr: f64,
    pub sunrise: f64,
    pub dhuhr: f64,
    pub asr: f64,
    pub maghrib: f64,
    pub isha: f64,
}

impl Times {
    #[cfg(test)]
    pub fn as_array(&self) -> [f64; 6] {
        [
            self.fajr,
            self.sunrise,
            self.dhuhr,
            self.asr,
            self.maghrib,
            self.isha,
        ]
    }
    /// The five prayers (no sunrise) with names.
    pub fn prayers(&self) -> [(&'static str, f64); 5] {
        [
            ("Fajr", self.fajr),
            ("Dhuhr", self.dhuhr),
            ("Asr", self.asr),
            ("Maghrib", self.maghrib),
            ("Isha", self.isha),
        ]
    }
}

fn d2r(d: f64) -> f64 {
    d.to_radians()
}
fn r2d(r: f64) -> f64 {
    r.to_degrees()
}
fn fix(a: f64, b: f64) -> f64 {
    let r = a - b * (a / b).floor();
    if r < 0.0 {
        r + b
    } else {
        r
    }
}

fn julian(y: i32, m: u32, d: u32) -> f64 {
    let (mut y, mut m) = (y as f64, m as f64);
    if m <= 2.0 {
        y -= 1.0;
        m += 12.0;
    }
    let a = (y / 100.0).floor();
    let b = 2.0 - a + (a / 4.0).floor();
    (365.25 * (y + 4716.0)).floor() + (30.6001 * (m + 1.0)).floor() + d as f64 + b - 1524.5
}

/// (declination degrees, equation of time hours)
fn sun_position(jd: f64) -> (f64, f64) {
    let d = jd - 2451545.0;
    let g = fix(357.529 + 0.98560028 * d, 360.0);
    let q = fix(280.459 + 0.98564736 * d, 360.0);
    let l = fix(q + 1.915 * d2r(g).sin() + 0.020 * d2r(2.0 * g).sin(), 360.0);
    let e = 23.439 - 0.00000036 * d;
    let ra = r2d((d2r(e).cos() * d2r(l).sin()).atan2(d2r(l).cos())) / 15.0;
    let eqt = q / 15.0 - fix(ra, 24.0);
    let decl = r2d((d2r(e).sin() * d2r(l).sin()).asin());
    (decl, eqt)
}

struct Ctx {
    jd: f64,
    lat: f64,
}

impl Ctx {
    fn mid_day(&self, t: f64) -> f64 {
        let (_, eqt) = sun_position(self.jd + t);
        fix(12.0 - eqt, 24.0)
    }
    /// Time when the sun reaches `angle` degrees below the horizon (negative = above).
    fn sun_angle_time(&self, angle: f64, t: f64, ccw: bool) -> f64 {
        let (decl, _) = sun_position(self.jd + t);
        let noon = self.mid_day(t);
        let x = (-d2r(angle).sin() - d2r(decl).sin() * d2r(self.lat).sin())
            / (d2r(decl).cos() * d2r(self.lat).cos());
        if !(-1.0..=1.0).contains(&x) {
            return f64::NAN;
        }
        let tt = r2d(x.acos()) / 15.0;
        noon + if ccw { -tt } else { tt }
    }
    fn asr(&self, factor: f64, t: f64) -> f64 {
        let (decl, _) = sun_position(self.jd + t);
        let angle = -r2d((1.0 / (factor + d2r((self.lat - decl).abs()).tan())).atan());
        self.sun_angle_time(angle, t, false)
    }
}

#[allow(clippy::too_many_arguments)]
/// Computes prayer times for a local date at (lat, lon) with UTC offset `tz_hours`.
pub fn compute(
    y: i32,
    m: u32,
    d: u32,
    lat: f64,
    lon: f64,
    tz_hours: f64,
    method: Method,
    hanafi: bool,
) -> Times {
    let ctx = Ctx {
        jd: julian(y, m, d) - lon / (15.0 * 24.0),
        lat,
    };
    // initial day portions, one refinement iteration (as in the reference algorithm)
    let p = [5.0, 6.0, 12.0, 13.0, 18.0, 18.0].map(|h: f64| h / 24.0);
    let fajr = ctx.sun_angle_time(method.fajr, p[0], true);
    let sunrise = ctx.sun_angle_time(0.833, p[1], true);
    let dhuhr = ctx.mid_day(p[2]);
    let asr = ctx.asr(if hanafi { 2.0 } else { 1.0 }, p[3]);
    let sunset = ctx.sun_angle_time(0.833, p[4], false);
    let maghrib = match method.maghrib {
        Some(a) => ctx.sun_angle_time(a, p[4], false),
        None => sunset,
    };
    let mut isha = match method.isha {
        IshaRule::Angle(a) => ctx.sun_angle_time(a, p[5], false),
        IshaRule::MinutesAfterMaghrib(mins) => maghrib + mins / 60.0,
    };
    let mut fajr = fajr;
    // high latitude: angle-based portion of the night
    let night = fix(sunrise - sunset, 24.0);
    if night.is_finite() {
        let fajr_portion = method.fajr / 60.0 * night;
        if fajr.is_nan() || fix(sunrise - fajr, 24.0) > fajr_portion {
            fajr = sunrise - fajr_portion;
        }
        if let IshaRule::Angle(a) = method.isha {
            let isha_portion = a / 60.0 * night;
            if isha.is_nan() || fix(isha - sunset, 24.0) > isha_portion {
                isha = sunset + isha_portion;
            }
        }
    }
    let adj = tz_hours - lon / 15.0;
    let to_min = |h: f64| {
        if h.is_nan() {
            f64::NAN
        } else {
            fix(h + adj, 24.0) * 60.0
        }
    };
    Times {
        fajr: to_min(fajr),
        sunrise: to_min(sunrise),
        dhuhr: to_min(dhuhr),
        asr: to_min(asr),
        maghrib: to_min(maghrib),
        isha: to_min(isha),
    }
}

/// Next prayer after `now_min` today: (index into prayers(), minutes until).
pub fn next_prayer(t: &Times, now_min: f64) -> Option<(usize, f64)> {
    t.prayers()
        .iter()
        .enumerate()
        .filter(|(_, (_, m))| m.is_finite() && *m > now_min)
        .map(|(i, (_, m))| (i, m - now_min))
        .next()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn order_is_sane() {
        // Makkah, 2026-03-20, UTC+3
        let t = compute(
            2026,
            3,
            20,
            21.4225,
            39.8262,
            3.0,
            method("UmmAlQura"),
            false,
        );
        let a = t.as_array();
        for w in a.windows(2) {
            assert!(w[0] < w[1], "{a:?}");
        }
        assert!((t.isha - t.maghrib - 90.0).abs() < 0.01);
    }
    #[test]
    fn dhuhr_matches_solar_noon() {
        // On the March equinox the equation of time is about -7.5 min, so at lon 0 / UTC
        // solar noon is ~12:07.
        let t = compute(2026, 3, 20, 51.4769, 0.0, 0.0, method("MWL"), false);
        assert!(
            (t.dhuhr - (12.0 * 60.0 + 7.4)).abs() < 1.5,
            "dhuhr {}",
            t.dhuhr
        );
        // Sunrise in Greenwich on the equinox is ~06:03 UTC.
        assert!(
            (t.sunrise - (6.0 * 60.0 + 3.0)).abs() < 5.0,
            "sunrise {}",
            t.sunrise
        );
    }
    #[test]
    fn hanafi_asr_is_later() {
        let s = compute(2026, 6, 1, 24.86, 67.0, 5.0, method("Karachi"), false);
        let h = compute(2026, 6, 1, 24.86, 67.0, 5.0, method("Karachi"), true);
        assert!(h.asr > s.asr + 30.0);
    }
    #[test]
    fn high_latitude_is_finite() {
        // Oslo in midsummer: twilight never ends, adjustment kicks in
        let t = compute(2026, 6, 21, 59.91, 10.75, 2.0, method("MWL"), false);
        assert!(t.fajr.is_finite() && t.isha.is_finite());
    }
}
