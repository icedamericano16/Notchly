//! Small shared helpers: wide strings, data paths, atomic file writes, logging,
//! the crash hook and civil-time arithmetic.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{de::DeserializeOwned, Serialize};
use windows::core::PCWSTR;
use windows::Win32::Storage::FileSystem::{
    MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
};

/// UTF-16, NUL terminated.
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

pub fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// %APPDATA%\Notchly (created on demand).
pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let p = base.join("Notchly");
    let _ = fs::create_dir_all(&p);
    p
}

pub fn sub_dir(name: &str) -> PathBuf {
    let p = data_dir().join(name);
    let _ = fs::create_dir_all(&p);
    p
}

/// Writes `bytes` to `path` atomically: temp file, then MoveFileExW(REPLACE_EXISTING | WRITE_THROUGH).
pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    let a = wide(&tmp.to_string_lossy());
    let b = wide(&path.to_string_lossy());
    unsafe {
        MoveFileExW(
            PCWSTR(a.as_ptr()),
            PCWSTR(b.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(|e| std::io::Error::other(e.to_string()))
}

/// Set when any data file was corrupt and reset; the UI shows a one-time Peek.
pub static DATA_RESET: AtomicBool = AtomicBool::new(false);

/// Loads `%APPDATA%\Notchly\<name>` as JSON. Corrupt files are renamed `.bad` and defaults are used.
pub fn load_json<T: DeserializeOwned + Default>(name: &str) -> T {
    let path = data_dir().join(name);
    match fs::read(&path) {
        Ok(bytes) => match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(e) => {
                log(&format!("corrupt {name}: {e}; resetting"));
                let _ = fs::rename(&path, path.with_extension("bad"));
                DATA_RESET.store(true, Ordering::Relaxed);
                T::default()
            }
        },
        Err(_) => T::default(),
    }
}

pub fn save_json<T: Serialize>(name: &str, value: &T) {
    let path = data_dir().join(name);
    match serde_json::to_vec_pretty(value) {
        Ok(bytes) => {
            if let Err(e) = atomic_write(&path, &bytes) {
                log(&format!("save {name} failed: {e}"));
            }
        }
        Err(e) => log(&format!("serialize {name} failed: {e}")),
    }
}

pub fn unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn unix_secs() -> i64 {
    unix_ms() / 1000
}

// ---------------------------------------------------------------- logging

static LOG_LOCK: Mutex<()> = Mutex::new(());

/// Appends a line to logs\notchly.log (rolling 3 × 1 MB). Never pass personal content here.
pub fn log(msg: &str) {
    let _g = LOG_LOCK.lock();
    let dir = sub_dir("logs");
    let path = dir.join("notchly.log");
    if let Ok(meta) = fs::metadata(&path) {
        if meta.len() > 1_000_000 {
            let _ = fs::remove_file(dir.join("notchly.2.log"));
            let _ = fs::rename(dir.join("notchly.1.log"), dir.join("notchly.2.log"));
            let _ = fs::rename(&path, dir.join("notchly.1.log"));
        }
    }
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
        let t = local_now();
        let _ = writeln!(
            f,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02} {}",
            t.year, t.month, t.day, t.hour, t.minute, t.second, msg
        );
    }
    #[cfg(debug_assertions)]
    eprintln!("{msg}");
}

#[macro_export]
macro_rules! logf {
    ($($t:tt)*) => { $crate::util::log(&format!($($t)*)) };
}

/// Installs a panic hook that writes logs\crash-<time>.txt.
pub fn install_crash_hook() {
    std::panic::set_hook(Box::new(|info| {
        let t = local_now();
        let name = format!(
            "crash-{:04}{:02}{:02}-{:02}{:02}{:02}.txt",
            t.year, t.month, t.day, t.hour, t.minute, t.second
        );
        let bt = std::backtrace::Backtrace::force_capture();
        let body = format!(
            "Notchly {} crashed\n{info}\n\n{bt}",
            env!("CARGO_PKG_VERSION")
        );
        let _ = fs::write(sub_dir("logs").join(name), body);
        log(&format!("panic: {info}"));
    }));
}

// ---------------------------------------------------------------- time

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LocalTime {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub millis: u32,
    /// 0 = Sunday
    pub weekday: u32,
}

impl LocalTime {
    pub fn minutes_of_day(&self) -> u32 {
        self.hour * 60 + self.minute
    }
    pub fn day_number(&self) -> i64 {
        days_from_civil(self.year, self.month, self.day)
    }
    pub fn secs_of_day(&self) -> f64 {
        (self.hour * 3600 + self.minute * 60 + self.second) as f64 + self.millis as f64 / 1000.0
    }
}

pub fn local_now() -> LocalTime {
    let st = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    LocalTime {
        year: st.wYear as i32,
        month: st.wMonth as u32,
        day: st.wDay as u32,
        hour: st.wHour as u32,
        minute: st.wMinute as u32,
        second: st.wSecond as u32,
        millis: st.wMilliseconds as u32,
        weekday: st.wDayOfWeek as u32,
    }
}

/// Local UTC offset in seconds (local = utc + offset).
pub fn local_offset_secs() -> i64 {
    let l = local_now();
    let local_secs = days_from_civil(l.year, l.month, l.day) * 86400
        + (l.hour * 3600 + l.minute * 60 + l.second) as i64;
    let utc = unix_secs();
    // round to nearest 15 minutes to remove the second boundary race
    let off = local_secs - utc;
    ((off as f64 / 900.0).round() * 900.0) as i64
}

/// Days since 1970-01-01 for a proleptic Gregorian date.
pub fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y } as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m = m as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

pub fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    ((if m <= 2 { y + 1 } else { y }) as i32, m, d)
}

/// Weekday (0 = Sunday) of a day number.
pub fn weekday_of(days: i64) -> u32 {
    (((days % 7) + 7 + 4) % 7) as u32
}

/// Local unix-seconds for a local date + minutes-of-day.
pub fn local_to_unix(day: i64, minutes: u32) -> i64 {
    day * 86400 + minutes as i64 * 60 - local_offset_secs()
}

/// Converts unix seconds to local civil time.
pub fn unix_to_local(secs: i64) -> LocalTime {
    let l = secs + local_offset_secs();
    let day = l.div_euclid(86400);
    let rem = l.rem_euclid(86400) as u32;
    let (y, m, d) = civil_from_days(day);
    LocalTime {
        year: y,
        month: m,
        day: d,
        hour: rem / 3600,
        minute: rem / 60 % 60,
        second: rem % 60,
        millis: 0,
        weekday: weekday_of(day),
    }
}

pub fn fmt_mmss(secs: i64) -> String {
    let s = secs.max(0);
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

pub fn fmt_hm(minutes: u32, h24: bool) -> String {
    let (h, m) = (minutes / 60 % 24, minutes % 60);
    if h24 {
        format!("{h:02}:{m:02}")
    } else {
        let hh = if h % 12 == 0 { 12 } else { h % 12 };
        format!("{hh}:{m:02} {}", if h < 12 { "AM" } else { "PM" })
    }
}

/// Whether the user's locale uses a 24-hour clock.
pub fn locale_is_24h() -> bool {
    use windows::Win32::Globalization::{GetLocaleInfoEx, LOCALE_STIMEFORMAT};
    let mut buf = [0u16; 80];
    let n = unsafe { GetLocaleInfoEx(PCWSTR::null(), LOCALE_STIMEFORMAT, Some(&mut buf)) };
    if n <= 0 {
        return false;
    }
    from_wide(&buf).contains('H')
}

/// FNV-1a hash, used for cache keys.
pub fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

pub const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
pub const WEEKDAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

/// Prunes a cache directory to `cap` bytes, removing least-recently-modified files first.
pub fn prune_dir(dir: &Path, cap: u64) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut files: Vec<(SystemTime, u64, PathBuf)> = rd
        .flatten()
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            Some((m.modified().unwrap_or(UNIX_EPOCH), m.len(), e.path()))
        })
        .collect();
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    if total <= cap {
        return;
    }
    files.sort_by_key(|f| f.0);
    for (_, len, p) in files {
        if total <= cap {
            break;
        }
        if fs::remove_file(&p).is_ok() {
            total -= len;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn civil_roundtrip() {
        for d in [-1000i64, 0, 1, 365, 19000, 20000, 60000] {
            let (y, m, dd) = civil_from_days(d);
            assert_eq!(days_from_civil(y, m, dd), d);
        }
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(weekday_of(0), 4); // Thursday
        assert_eq!(days_from_civil(2000, 3, 1), 11017);
    }
    #[test]
    fn mmss() {
        assert_eq!(fmt_mmss(65), "1:05");
        assert_eq!(fmt_mmss(3725), "1:02:05");
        assert_eq!(fmt_hm(13 * 60 + 5, false), "1:05 PM");
    }
}
