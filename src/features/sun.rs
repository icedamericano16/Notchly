//! Solar position (NOAA low-precision algorithm, accurate to about a minute): sunrise,
//! sunset, solar noon and the sun's elevation at any instant, fully offline.

use std::f64::consts::PI;

const RAD: f64 = PI / 180.0;

fn julian(unix: f64) -> f64 {
    unix / 86400.0 + 2_440_587.5
}

/// Solar declination (radians) and equation of time (minutes) for a Julian day.
fn declination_eqt(jd: f64) -> (f64, f64) {
    let n = jd - 2_451_545.0;
    let l = (280.460 + 0.985_647_4 * n).rem_euclid(360.0);
    let g = ((357.528 + 0.985_600_3 * n).rem_euclid(360.0)) * RAD;
    let lambda = (l + 1.915 * g.sin() + 0.020 * (2.0 * g).sin()) * RAD;
    let eps = (23.439 - 0.000_000_4 * n) * RAD;
    let decl = (eps.sin() * lambda.sin()).asin();
    let ra = (eps.cos() * lambda.sin()).atan2(lambda.cos()) / RAD;
    let mut eqt = 4.0 * (l - ra.rem_euclid(360.0));
    if eqt > 720.0 {
        eqt -= 1440.0;
    } else if eqt < -720.0 {
        eqt += 1440.0;
    }
    (decl, eqt)
}

/// The sun's elevation above the horizon in degrees (negative at night).
pub fn elevation(lat: f64, lon: f64, unix: i64) -> f64 {
    let (decl, eqt) = declination_eqt(julian(unix as f64));
    let utc_min = (unix.rem_euclid(86_400)) as f64 / 60.0;
    let solar = utc_min + eqt + 4.0 * lon;
    let ha = (solar / 4.0 - 180.0) * RAD;
    let phi = lat * RAD;
    let cz = phi.sin() * decl.sin() + phi.cos() * decl.cos() * ha.cos();
    90.0 - cz.clamp(-1.0, 1.0).acos() / RAD
}

/// One day of sun times, in Unix seconds. `rise`/`set` are `None` during polar day or night.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SunDay {
    pub rise: Option<i64>,
    pub set: Option<i64>,
    pub noon: i64,
}

impl SunDay {
    pub fn daylight_secs(&self) -> Option<i64> {
        Some(self.set? - self.rise?)
    }
}

/// Sun times for the local calendar day `day` (days since 1970-01-01 in local time);
/// `utc_offset` is the local UTC offset in seconds.
pub fn day(lat: f64, lon: f64, day: i64, utc_offset: i64) -> SunDay {
    // evaluate at local noon of that day
    let local_noon = day * 86_400 + 43_200 - utc_offset;
    let (decl, eqt) = declination_eqt(julian(local_noon as f64));
    let day_utc = local_noon.div_euclid(86_400) * 86_400;
    let noon_min = 720.0 - 4.0 * lon - eqt;
    let noon = day_utc + (noon_min * 60.0) as i64;
    let phi = lat * RAD;
    // 90.833° = refraction + solar disc radius
    let cos_h = ((90.833 * RAD).cos() - phi.sin() * decl.sin()) / (phi.cos() * decl.cos());
    if !(-1.0..=1.0).contains(&cos_h) {
        return SunDay {
            rise: None,
            set: None,
            noon,
        };
    }
    let h = cos_h.acos() / RAD;
    SunDay {
        rise: Some(noon - (4.0 * h * 60.0) as i64),
        set: Some(noon + (4.0 * h * 60.0) as i64),
        noon,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local_hm(unix: i64, off: i64) -> (i64, i64) {
        let s = (unix + off).rem_euclid(86_400);
        (s / 3600, s / 60 % 60)
    }

    #[test]
    fn kathmandu_november() {
        // 2026-11-23, Nepal Time (UTC+5:45): sunrise ≈ 06:30, sunset ≈ 17:08 (solar noon ≈ 11:50)
        let off = 5 * 3600 + 45 * 60;
        let d = crate::util::days_from_civil(2026, 11, 23);
        let s = day(27.7172, 85.3240, d, off);
        let (rh, rm) = local_hm(s.rise.unwrap(), off);
        let (sh, sm) = local_hm(s.set.unwrap(), off);
        let rise = rh * 60 + rm;
        let set = sh * 60 + sm;
        assert!(
            (rise - (6 * 60 + 30)).abs() <= 6,
            "rise {rh}:{rm:02} set {sh}:{sm:02}"
        );
        assert!(
            (set - (17 * 60 + 8)).abs() <= 6,
            "rise {rh}:{rm:02} set {sh}:{sm:02}"
        );
        // the sun is up at noon and down at midnight
        assert!(elevation(27.7172, 85.3240, s.noon) > 35.0);
        assert!(elevation(27.7172, 85.3240, s.noon + 12 * 3600) < -30.0);
    }

    #[test]
    fn london_midsummer_and_polar() {
        // 2026-06-21, BST (UTC+1): sunrise ≈ 04:43, sunset ≈ 21:21
        let off = 3600;
        let d = crate::util::days_from_civil(2026, 6, 21);
        let s = day(51.5074, -0.1278, d, off);
        let (rh, rm) = local_hm(s.rise.unwrap(), off);
        let (sh, sm) = local_hm(s.set.unwrap(), off);
        assert!(
            ((rh * 60 + rm) - (4 * 60 + 43)).abs() <= 6,
            "rise {rh}:{rm:02}"
        );
        assert!(
            ((sh * 60 + sm) - (21 * 60 + 21)).abs() <= 6,
            "set {sh}:{sm:02}"
        );
        // Tromsø in midwinter: polar night
        let d = crate::util::days_from_civil(2026, 12, 21);
        assert!(day(69.65, 18.96, d, 3600).rise.is_none());
    }
}
