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

/// Logs live in %LOCALAPPDATA%\Notchly\logs (machine-local, not roamed or synced).
pub fn log_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(data_dir);
    let p = base.join("Notchly").join("logs");
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
    let dir = log_dir();
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

/// A UTF-16 buffer as little-endian bytes (registry and clipboard APIs take bytes).
pub fn u16_bytes(w: &[u16]) -> Vec<u8> {
    w.iter().flat_map(|c| c.to_le_bytes()).collect()
}

/// Decodes a URL query value (`%XX` and `+`). Works on bytes, so no input can make it panic:
/// it reads text from the network (Phone Drop, the OAuth redirect), and a panic aborts the app.
pub fn url_decode(s: &str) -> String {
    let b = s.as_bytes();
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' => match (
                b.get(i + 1).and_then(|&c| hex(c)),
                b.get(i + 2).and_then(|&c| hex(c)),
            ) {
                (Some(h), Some(l)) => {
                    out.push(h << 4 | l);
                    i += 3;
                    continue;
                }
                _ => out.push(b'%'),
            },
            b'+' => out.push(b' '),
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The value of `key` in a `a=1&b=2` query string, decoded.
pub fn query_param(query: &str, key: &str) -> Option<String> {
    query
        .split('&')
        .find_map(|kv| kv.strip_prefix(key)?.strip_prefix('='))
        .map(url_decode)
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
        let _ = fs::write(log_dir().join(name), body);
        log(&format!("panic: {info}"));
    }));
}

/// Replaces the user-name part of `...\Users\<name>\...` paths, so a report pasted into a
/// public issue doesn't carry it.
pub fn redact_user_paths(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.to_ascii_lowercase().find("\\users\\") {
        let start = i + "\\users\\".len();
        out.push_str(&rest[..start]);
        let tail = &rest[start..];
        let end = tail
            .find(['\\', '/', '\n', '"', '\''])
            .unwrap_or(tail.len());
        out.push_str("<user>");
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

/// A "new issue" link on GitHub pre-filled with the crash report. Nothing is sent: the user
/// sees the text in their browser and decides whether to submit it. Kept under GitHub's URL
/// length limit by shortening the backtrace.
pub fn crash_issue_url(repo_url: &str, report: &str, windows_build: u32) -> String {
    let enc = crate::services::http::enc;
    let report = redact_user_paths(report);
    let (head, trace) = match report.find("\n\n") {
        Some(i) => (report[..i].trim(), report[i..].trim()),
        None => (report.trim(), ""),
    };
    let title = head
        .lines()
        .find(|l| l.contains("panicked at") || l.starts_with("panicked"))
        .map(|l| format!("Crash: {}", l.trim()))
        .unwrap_or_else(|| "Crash report".into());
    let title: String = title.chars().take(120).collect();
    let mut lines: Vec<&str> = trace.lines().collect();
    loop {
        let body = format!(
            "**What were you doing when Notchly closed?**\n\n\n\n**Report**\n```\n{head}\n```\nWindows build {windows_build}\n\n<details><summary>Backtrace</summary>\n\n```\n{}\n```\n</details>\n",
            lines.join("\n")
        );
        let url = format!(
            "{repo_url}/issues/new?title={}&body={}",
            enc(&title),
            enc(&body)
        );
        if url.len() <= 7500 || lines.is_empty() {
            return url;
        }
        let keep = lines.len() * 2 / 3;
        lines.truncate(keep);
    }
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

/// UTC offset in seconds of the IANA time zone `tz` (e.g. "Asia/Tokyo") at `unix`, daylight
/// saving included, from Windows.Globalization. None for an empty or unknown zone.
pub fn zone_offset_secs(tz: &str, unix: i64) -> Option<i64> {
    if tz.is_empty() {
        return None;
    }
    calendar_offset(Some(tz), unix)
}

/// This PC's UTC offset at `unix` (which may be months away, across a daylight-saving
/// change), unlike [`local_offset_secs`] which is the offset right now.
pub fn local_offset_at(unix: i64) -> i64 {
    calendar_offset(None, unix).unwrap_or_else(local_offset_secs)
}

/// Offset at `unix` in `tz`, or in this PC's zone for None, from Windows.Globalization.
fn calendar_offset(tz: Option<&str>, unix: i64) -> Option<i64> {
    use windows::core::HSTRING;
    use windows::Foundation::DateTime;
    use windows::Globalization::Calendar;
    let cal = Calendar::new().ok()?;
    cal.ChangeCalendarSystem(&HSTRING::from("GregorianCalendar"))
        .ok()?;
    cal.ChangeClock(&HSTRING::from("24HourClock")).ok()?;
    if let Some(tz) = tz {
        cal.ChangeTimeZone(&HSTRING::from(tz)).ok()?;
    }
    // DateTime counts 100 ns ticks since 1601-01-01 UTC
    cal.SetDateTime(DateTime {
        UniversalTime: (unix + 11_644_473_600) * 10_000_000,
    })
    .ok()?;
    let civil = days_from_civil(
        cal.Year().ok()?,
        cal.Month().ok()? as u32,
        cal.Day().ok()? as u32,
    ) * 86_400
        + cal.Hour().ok()? as i64 * 3600
        + cal.Minute().ok()? as i64 * 60
        + cal.Second().ok()? as i64;
    Some(civil - unix)
}

/// The IANA zone for a Windows zone name ("W. Europe Standard Time" → "Europe/Berlin"), which
/// is what Outlook writes in calendar files. Uses the ICU that ships with Windows 10 1903+,
/// loaded at run time so older systems still start; None when unavailable or unknown.
pub fn iana_for_windows_zone(name: &str) -> Option<String> {
    use windows::core::{s, w};
    use windows::Win32::System::LibraryLoader::{
        GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32,
    };
    type Fn = unsafe extern "C" fn(*const u16, i32, *const u8, *mut u16, i32, *mut i32) -> i32;
    // SAFETY: icu.dll is loaded from System32 only; the symbol is ICU's documented C function
    // `ucal_getTimeZoneIDForWindowsID` with exactly this signature (UChar* is UTF-16, the region
    // may be null, UErrorCode is an i32 where > 0 means failure). The input and output buffers
    // live for the whole call and their lengths are passed. The module stays loaded, which is
    // what makes keeping the function pointer sound.
    unsafe {
        let m = LoadLibraryExW(w!("icu.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32).ok()?;
        let f: Fn = std::mem::transmute(GetProcAddress(m, s!("ucal_getTimeZoneIDForWindowsID"))?);
        let win: Vec<u16> = name.encode_utf16().collect();
        let mut out = [0u16; 64];
        let mut status = 0i32;
        let n = f(
            win.as_ptr(),
            win.len() as i32,
            std::ptr::null(),
            out.as_mut_ptr(),
            out.len() as i32,
            &mut status,
        );
        (status <= 0 && n > 0 && (n as usize) <= out.len())
            .then(|| String::from_utf16_lossy(&out[..n as usize]))
    }
}

/// Offset of a chosen place: its own zone when known, otherwise this PC's. Cached per zone for
/// 15 minutes (daylight-saving changes happen on the hour or half hour), since it's read while
/// painting.
pub fn place_offset_secs(tz: &str) -> i64 {
    use std::cell::RefCell;
    thread_local! {
        static CACHE: RefCell<Option<(String, i64, Option<i64>)>> = const { RefCell::new(None) };
    }
    if tz.is_empty() {
        return local_offset_secs();
    }
    let now = unix_secs();
    let slot = now.div_euclid(900);
    let off = CACHE.with(|c| {
        let mut c = c.borrow_mut();
        match &*c {
            Some((z, s, off)) if z == tz && *s == slot => *off,
            _ => {
                let off = zone_offset_secs(tz, now);
                *c = Some((tz.to_string(), slot, off));
                off
            }
        }
    });
    off.unwrap_or_else(local_offset_secs)
}

/// Civil time now at a chosen place (see [`place_offset_secs`]).
pub fn place_now(tz: &str) -> LocalTime {
    if tz.is_empty() {
        return local_now();
    }
    let mut t = unix_to_zone(unix_secs(), place_offset_secs(tz));
    t.millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_millis())
        .unwrap_or(0);
    t
}

/// "GMT+9" when the chosen place's clock differs from this PC's, so times shown for it are
/// labelled; None when they agree (or no zone is known).
pub fn zone_label(tz: &str) -> Option<String> {
    let off = place_offset_secs(tz);
    (!tz.is_empty() && off != local_offset_secs()).then(|| fmt_offset(off))
}

/// "GMT+5:45" style label for an offset.
pub fn fmt_offset(off: i64) -> String {
    let sign = if off < 0 { '-' } else { '+' };
    let m = off.abs() / 60;
    if m % 60 == 0 {
        format!("GMT{sign}{}", m / 60)
    } else {
        format!("GMT{sign}{}:{:02}", m / 60, m % 60)
    }
}

/// Converts unix seconds to civil time at a fixed UTC offset.
pub fn unix_to_zone(secs: i64, offset: i64) -> LocalTime {
    let l = secs + offset;
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

/// An uncompressed PNG (stored deflate blocks) of 8-bit RGBA pixels. Debug QA only.
#[cfg(debug_assertions)]
pub fn png_rgba(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    fn crc(data: &[u8]) -> u32 {
        let mut c = 0xFFFF_FFFFu32;
        for &b in data {
            c ^= b as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
        }
        !c
    }
    fn chunk(out: &mut Vec<u8>, kind: &[u8], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut c = kind.to_vec();
        c.extend_from_slice(data);
        out.extend_from_slice(&c);
        out.extend_from_slice(&crc(&c).to_be_bytes());
    }
    let stride = w as usize * 4;
    let mut raw = Vec::with_capacity((stride + 1) * h as usize);
    for y in 0..h as usize {
        raw.push(0);
        raw.extend_from_slice(&rgba[y * stride..(y + 1) * stride]);
    }
    let mut z = vec![0x78, 0x01];
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    let blocks: Vec<&[u8]> = raw.chunks(65535).collect();
    for (i, blk) in blocks.iter().enumerate() {
        z.push((i + 1 == blocks.len()) as u8);
        let n = blk.len() as u16;
        z.extend_from_slice(&n.to_le_bytes());
        z.extend_from_slice(&(!n).to_le_bytes());
        z.extend_from_slice(blk);
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

/// QA (debug builds with NOTCHLY_QA=1): an environment value, else None.
#[cfg(debug_assertions)]
pub fn qa_env(name: &str) -> Option<String> {
    if std::env::var("NOTCHLY_QA").is_ok_and(|v| v == "1") {
        std::env::var(name).ok()
    } else {
        None
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

    #[test]
    fn iana_zone_offsets() {
        let jan = days_from_civil(2026, 1, 15) * 86_400 + 12 * 3600;
        let jul = days_from_civil(2026, 7, 15) * 86_400 + 12 * 3600;
        assert_eq!(zone_offset_secs("Asia/Tokyo", jan), Some(9 * 3600));
        assert_eq!(
            zone_offset_secs("Asia/Kathmandu", jan),
            Some(5 * 3600 + 45 * 60)
        );
        // daylight saving follows the instant, not today
        assert_eq!(zone_offset_secs("America/New_York", jan), Some(-5 * 3600));
        assert_eq!(zone_offset_secs("America/New_York", jul), Some(-4 * 3600));
        assert_eq!(zone_offset_secs("Europe/London", jul), Some(3600));
        assert_eq!(zone_offset_secs("Not/AZone", jan), None);
        assert_eq!(zone_offset_secs("", jan), None);
    }

    #[test]
    fn url_decoding_never_panics() {
        assert_eq!(url_decode("hello%20world+x"), "hello world x");
        // a Google authorization code arrives with "/" encoded
        assert_eq!(url_decode("4%2F0AeaYSHB"), "4/0AeaYSHB");
        assert_eq!(url_decode("%E0%A4%A8"), "न");
        // the old decoder sliced text after "%" and panicked when a multi-byte character
        // followed; a panic aborts Notchly, so a LAN request could close it
        assert_eq!(url_decode("%aé"), "%aé");
        assert_eq!(url_decode("%é"), "%é");
        assert_eq!(url_decode("100%"), "100%");
        assert_eq!(url_decode("%zz%4"), "%zz%4");
        for s in ["%", "%%", "%\u{FFFF}", "a%2", "\u{1F600}%F0%9F"] {
            let _ = url_decode(s);
        }
        assert_eq!(
            query_param("code=4%2Fx&state=ab", "code").as_deref(),
            Some("4/x")
        );
        assert_eq!(query_param("xcode=1&code=2", "code").as_deref(), Some("2"));
        assert_eq!(query_param("state=ab", "code"), None);
    }

    #[test]
    fn crash_reports_are_redacted_and_short() {
        assert_eq!(
            redact_user_paths(r"at C:\Users\Jane Doe\src\x.rs and c:\users\bob\a"),
            r"at C:\Users\<user>\src\x.rs and c:\users\<user>\a"
        );
        let trace: String = (0..2000)
            .map(|i| format!("  {i}: notchly::frame::paint_{i}\n"))
            .collect();
        let report = format!(
            "Notchly 0.3.1 crashed\npanicked at src\\app\\events.rs:10:5:\nboom in C:\\Users\\jane\\file\n\n{trace}"
        );
        let url = crash_issue_url("https://github.com/o/r", &report, 26100);
        assert!(url.starts_with("https://github.com/o/r/issues/new?title=Crash"));
        assert!(url.len() <= 7500, "{}", url.len());
        assert!(!url.contains("jane"));
        assert!(url.contains("26100"));
    }

    #[test]
    fn windows_zone_names_map_to_iana() {
        assert_eq!(
            iana_for_windows_zone("W. Europe Standard Time").as_deref(),
            Some("Europe/Berlin")
        );
        assert_eq!(
            iana_for_windows_zone("Pacific Standard Time").as_deref(),
            Some("America/Los_Angeles")
        );
        assert_eq!(iana_for_windows_zone("Nowhere Standard Time"), None);
    }

    #[test]
    fn local_offset_now_matches() {
        assert_eq!(local_offset_at(unix_secs()), local_offset_secs());
    }

    #[test]
    fn place_falls_back_to_this_pc() {
        assert_eq!(place_offset_secs(""), local_offset_secs());
        assert_eq!(place_offset_secs("Not/AZone"), local_offset_secs());
        assert_eq!(place_offset_secs("Asia/Tokyo"), 9 * 3600);
    }

    #[test]
    fn zone_conversion_and_labels() {
        // 2026-01-15 00:30 UTC is 06:15 in Kathmandu and 19:30 the day before in New York
        let t = days_from_civil(2026, 1, 15) * 86_400 + 30 * 60;
        let k = unix_to_zone(t, 5 * 3600 + 45 * 60);
        assert_eq!((k.day, k.hour, k.minute), (15, 6, 15));
        let ny = unix_to_zone(t, -5 * 3600);
        assert_eq!((ny.day, ny.hour, ny.minute), (14, 19, 30));
        assert_eq!(fmt_offset(9 * 3600), "GMT+9");
        assert_eq!(fmt_offset(5 * 3600 + 45 * 60), "GMT+5:45");
        assert_eq!(fmt_offset(-(3 * 3600 + 30 * 60)), "GMT-3:30");
        assert_eq!(fmt_offset(0), "GMT+0");
    }
}
