//! iCalendar (.ics) subscriptions: events, recurrence rules, exceptions and time zones.
//!
//! Covers what calendar exports actually use: DAILY / WEEKLY / MONTHLY / YEARLY rules with
//! INTERVAL, COUNT, UNTIL, BYDAY (also "2TU" / "-1FR"), BYMONTHDAY and BYMONTH; EXDATE;
//! moved or cancelled single instances (RECURRENCE-ID); DURATION; and TZID given as an IANA
//! name, a Windows name (Outlook) or a VTIMEZONE with X-LIC-LOCATION. Recurrences are expanded
//! in the event's own wall-clock time, so a 09:00 meeting stays at 09:00 across daylight
//! saving. Floating and all-day times use this PC's zone.

use std::collections::{HashMap, HashSet};

use super::calendar::{find_meeting_link, Event};
use crate::util::{civil_from_days, days_from_civil, weekday_of};

/// Hard stop for runaway rules (e.g. FREQ=DAILY with no end, started decades ago).
const MAX_PERIODS: i64 = 200_000;
/// Widest UTC offset, used for cheap "clearly outside the window" checks on wall times.
const MAX_OFFSET: i64 = 14 * 3600;

#[derive(Debug, Clone)]
struct Prop {
    name: String,
    params: Vec<(String, String)>,
    value: String,
}

impl Prop {
    fn param(&self, key: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }
}

fn unfold(ics: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in ics.lines() {
        if (raw.starts_with(' ') || raw.starts_with('\t')) && !out.is_empty() {
            if let Some(last) = out.last_mut() {
                last.push_str(&raw[1..]);
            }
        } else {
            out.push(raw.to_string());
        }
    }
    out
}

/// `NAME;P1=a;P2="x:y":value`; a colon inside a quoted parameter doesn't end the name part.
fn parse_line(line: &str) -> Option<Prop> {
    let mut quoted = false;
    let colon = line.char_indices().find_map(|(i, c)| {
        if c == '"' {
            quoted = !quoted;
        }
        (c == ':' && !quoted).then_some(i)
    })?;
    let (head, value) = (&line[..colon], &line[colon + 1..]);
    let mut parts = head.split(';');
    let name = parts.next()?.trim().to_ascii_uppercase();
    let params = parts
        .filter_map(|p| p.split_once('='))
        .map(|(k, v)| {
            (
                k.trim().to_ascii_uppercase(),
                v.trim_matches('"').to_string(),
            )
        })
        .collect();
    Some(Prop {
        name,
        params,
        value: value.to_string(),
    })
}

fn unescape(s: &str) -> String {
    s.replace("\\n", "\n")
        .replace("\\N", "\n")
        .replace("\\,", ",")
        .replace("\\;", ";")
        .replace("\\\\", "\\")
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Zone {
    Utc,
    /// This PC's zone: floating times and all-day dates.
    Local,
    /// An IANA name.
    Named(String),
}

/// A time as written: wall-clock seconds (day * 86400 + seconds of day) and how to read it.
#[derive(Debug, Clone, PartialEq)]
struct Stamp {
    wall: i64,
    all_day: bool,
    zone: Zone,
}

/// Turns wall times into UTC, with the zone lookups and offsets cached.
#[derive(Default)]
struct Clock {
    /// TZID → zone, from VTIMEZONE X-LIC-LOCATION, then resolved names.
    tzids: HashMap<String, Zone>,
    offsets: HashMap<(Zone, i64), i64>,
}

impl Clock {
    fn zone_for(&mut self, tzid: &str) -> Zone {
        if let Some(z) = self.tzids.get(tzid) {
            return z.clone();
        }
        let z = resolve_tzid(tzid);
        self.tzids.insert(tzid.to_string(), z.clone());
        z
    }

    fn offset(&mut self, zone: &Zone, unix: i64) -> i64 {
        let key = (zone.clone(), unix.div_euclid(3600));
        if let Some(o) = self.offsets.get(&key) {
            return *o;
        }
        let o = match zone {
            Zone::Utc => 0,
            Zone::Local => crate::util::local_offset_at(unix),
            Zone::Named(tz) => crate::util::zone_offset_secs(tz, unix)
                .unwrap_or_else(|| crate::util::local_offset_at(unix)),
        };
        self.offsets.insert(key, o);
        o
    }

    fn utc(&mut self, wall: i64, zone: &Zone) -> i64 {
        if *zone == Zone::Utc {
            return wall;
        }
        // guess with the offset at the wall time read as UTC, then correct once: right
        // everywhere except inside the skipped hour of a daylight-saving change
        let first = self.offset(zone, wall);
        wall - self.offset(zone, wall - first)
    }
}

/// An IANA name as is; a Windows name through ICU; "/mozilla.org/.../Europe/Berlin" style
/// prefixes by their last two parts; otherwise this PC's zone.
fn resolve_tzid(tzid: &str) -> Zone {
    let t = tzid.trim();
    if t.eq_ignore_ascii_case("UTC") || t.eq_ignore_ascii_case("Etc/UTC") || t == "Z" {
        return Zone::Utc;
    }
    let valid = |z: &str| crate::util::zone_offset_secs(z, 0).is_some();
    if t.contains('/') && valid(t) {
        return Zone::Named(t.to_string());
    }
    if let Some(iana) = crate::util::iana_for_windows_zone(t) {
        return Zone::Named(iana);
    }
    let parts: Vec<&str> = t.trim_matches('/').split('/').collect();
    if parts.len() >= 2 {
        let tail = parts[parts.len() - 2..].join("/");
        if valid(&tail) {
            return Zone::Named(tail);
        }
    }
    Zone::Local
}

/// "20261122T093000Z", "20261122T093000" with an optional TZID, or "20261122" (a date).
fn parse_stamp(value: &str, tzid: Option<&str>, clock: &mut Clock) -> Option<Stamp> {
    let v = value.trim();
    let y: i32 = v.get(0..4)?.parse().ok()?;
    let m: u32 = v.get(4..6)?.parse().ok()?;
    let d: u32 = v.get(6..8)?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let day = days_from_civil(y, m, d);
    if v.len() < 15 {
        return Some(Stamp {
            wall: day * 86_400,
            all_day: true,
            zone: Zone::Local,
        });
    }
    let h: i64 = v.get(9..11)?.parse().ok()?;
    let mi: i64 = v.get(11..13)?.parse().ok()?;
    let s: i64 = v.get(13..15)?.parse().ok()?;
    let zone = if v.ends_with('Z') {
        Zone::Utc
    } else if let Some(t) = tzid {
        clock.zone_for(t)
    } else {
        Zone::Local
    };
    Some(Stamp {
        wall: day * 86_400 + h * 3600 + mi * 60 + s,
        all_day: false,
        zone,
    })
}

/// "P1D", "PT1H30M", "P2W", "-PT15M" → seconds.
fn parse_duration(v: &str) -> Option<i64> {
    let v = v.trim();
    let (sign, v) = match v.strip_prefix('-') {
        Some(r) => (-1, r),
        None => (1, v.strip_prefix('+').unwrap_or(v)),
    };
    let v = v.strip_prefix('P')?;
    let mut total = 0i64;
    let mut num = String::new();
    let mut in_time = false;
    for c in v.chars() {
        match c {
            'T' => in_time = true,
            '0'..='9' => num.push(c),
            _ => {
                let n: i64 = num.parse().ok()?;
                num.clear();
                total += n * match (c, in_time) {
                    ('W', _) => 7 * 86_400,
                    ('D', _) => 86_400,
                    ('H', true) => 3600,
                    ('M', true) => 60,
                    ('S', true) => 1,
                    _ => return None,
                };
            }
        }
    }
    num.is_empty().then_some(sign * total)
}

// ---------------------------------------------------------------- recurrence rules

#[derive(Debug, Clone, Copy, PartialEq)]
enum Freq {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

#[derive(Debug, Clone, PartialEq)]
struct Rule {
    freq: Freq,
    interval: i64,
    count: Option<u32>,
    /// The last allowed start as written (UTC, or the event's wall clock for a date).
    until: Option<Stamp>,
    /// (n, weekday 0 = Sunday); n = 0 means every such weekday in the period.
    byday: Vec<(i32, u32)>,
    bymonthday: Vec<i32>,
    bymonth: Vec<u32>,
}

fn weekday_code(s: &str) -> Option<u32> {
    ["SU", "MO", "TU", "WE", "TH", "FR", "SA"]
        .iter()
        .position(|d| s.eq_ignore_ascii_case(d))
        .map(|i| i as u32)
}

fn parse_rule(v: &str, clock: &mut Clock) -> Option<Rule> {
    let mut r = Rule {
        freq: Freq::Daily,
        interval: 1,
        count: None,
        until: None,
        byday: Vec::new(),
        bymonthday: Vec::new(),
        bymonth: Vec::new(),
    };
    let mut freq = None;
    for part in v.split(';') {
        let Some((k, val)) = part.split_once('=') else {
            continue;
        };
        match k.trim().to_ascii_uppercase().as_str() {
            "FREQ" => {
                freq = match val.to_ascii_uppercase().as_str() {
                    "DAILY" => Some(Freq::Daily),
                    "WEEKLY" => Some(Freq::Weekly),
                    "MONTHLY" => Some(Freq::Monthly),
                    "YEARLY" => Some(Freq::Yearly),
                    _ => None, // HOURLY etc.: not expanded
                }
            }
            "INTERVAL" => r.interval = val.parse().ok().filter(|n: &i64| *n > 0).unwrap_or(1),
            "COUNT" => r.count = val.parse().ok(),
            "UNTIL" => r.until = parse_stamp(val, None, clock),
            "BYDAY" => {
                r.byday = val
                    .split(',')
                    .filter_map(|d| {
                        let d = d.trim();
                        let (n, code) = d.split_at(d.len().saturating_sub(2));
                        Some((n.parse().unwrap_or(0), weekday_code(code)?))
                    })
                    .collect()
            }
            "BYMONTHDAY" => {
                r.bymonthday = val
                    .split(',')
                    .filter_map(|d| d.trim().parse().ok())
                    .collect()
            }
            "BYMONTH" => {
                r.bymonth = val
                    .split(',')
                    .filter_map(|d| d.trim().parse().ok())
                    .collect()
            }
            _ => {}
        }
    }
    r.freq = freq?;
    Some(r)
}

fn month_len(y: i32, m: u32) -> u32 {
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    (days_from_civil(ny, nm, 1) - days_from_civil(y, m, 1)) as u32
}

/// Days of month (y, m) matching BYDAY entries like "2TU", "-1FR" or "MO" (every Monday).
fn month_days_by_weekday(y: i32, m: u32, byday: &[(i32, u32)]) -> Vec<i64> {
    let first = days_from_civil(y, m, 1);
    let len = month_len(y, m) as i64;
    let mut out = Vec::new();
    for &(n, wd) in byday {
        let offset = (wd as i64 - weekday_of(first) as i64).rem_euclid(7);
        let all: Vec<i64> = (0..)
            .map(|k| first + offset + 7 * k)
            .take_while(|d| *d < first + len)
            .collect();
        match n {
            0 => out.extend(all),
            n if n > 0 => out.extend(all.get(n as usize - 1)),
            n => out.extend(
                all.len()
                    .checked_sub(n.unsigned_abs() as usize)
                    .map(|i| all[i]),
            ),
        }
    }
    out
}

/// Days of month (y, m) for BYMONTHDAY (negative counts from the end), else `dom`.
fn month_days_by_number(y: i32, m: u32, bymonthday: &[i32], dom: u32) -> Vec<i64> {
    let len = month_len(y, m) as i32;
    let first = days_from_civil(y, m, 1);
    let wanted: Vec<i32> = if bymonthday.is_empty() {
        vec![dom as i32]
    } else {
        bymonthday.to_vec()
    };
    wanted
        .into_iter()
        .map(|d| if d < 0 { len + d + 1 } else { d })
        .filter(|d| (1..=len).contains(d))
        .map(|d| first + d as i64 - 1)
        .collect()
}

/// Calls `f` with every day the rule produces, in order, starting at `start_day`, until `f`
/// returns false.
fn for_each_day(start_day: i64, rule: &Rule, mut f: impl FnMut(i64) -> bool) {
    let (y0, m0, d0) = civil_from_days(start_day);
    let monday0 = start_day - (weekday_of(start_day) as i64 + 6) % 7;
    for k in 0..MAX_PERIODS {
        let step = k * rule.interval;
        let mut days: Vec<i64> = match rule.freq {
            Freq::Daily => vec![start_day + step],
            Freq::Weekly => {
                let monday = monday0 + 7 * step;
                if rule.byday.is_empty() {
                    vec![monday + (weekday_of(start_day) as i64 + 6) % 7]
                } else {
                    rule.byday
                        .iter()
                        .map(|&(_, wd)| monday + (wd as i64 + 6) % 7)
                        .collect()
                }
            }
            Freq::Monthly => {
                let mi = (y0 as i64) * 12 + (m0 as i64 - 1) + step;
                let (y, m) = (mi.div_euclid(12) as i32, (mi.rem_euclid(12) + 1) as u32);
                if !rule.byday.is_empty() && rule.bymonthday.is_empty() {
                    month_days_by_weekday(y, m, &rule.byday)
                } else {
                    month_days_by_number(y, m, &rule.bymonthday, d0)
                }
            }
            Freq::Yearly => {
                let y = y0 + step as i32;
                let months = if rule.bymonth.is_empty() {
                    vec![m0]
                } else {
                    rule.bymonth.clone()
                };
                months
                    .into_iter()
                    .filter(|m| (1..=12).contains(m))
                    .flat_map(|m| {
                        if !rule.byday.is_empty() && rule.bymonthday.is_empty() {
                            month_days_by_weekday(y, m, &rule.byday)
                        } else {
                            month_days_by_number(y, m, &rule.bymonthday, d0)
                        }
                    })
                    .collect()
            }
        };
        // BYDAY / BYMONTH narrow the daily and weekly-by-month cases
        if rule.freq == Freq::Daily && !rule.byday.is_empty() {
            days.retain(|d| rule.byday.iter().any(|&(_, wd)| wd == weekday_of(*d)));
        }
        if matches!(rule.freq, Freq::Daily | Freq::Weekly) && !rule.bymonth.is_empty() {
            days.retain(|d| rule.bymonth.contains(&civil_from_days(*d).1));
        }
        days.sort_unstable();
        days.dedup();
        for d in days.into_iter().filter(|d| *d >= start_day) {
            if !f(d) {
                return;
            }
        }
    }
}

// ---------------------------------------------------------------- events

#[derive(Default)]
struct Raw {
    props: Vec<Prop>,
}

impl Raw {
    fn get(&self, name: &str) -> Option<&Prop> {
        self.props.iter().find(|p| p.name == name)
    }
    fn all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Prop> + 'a {
        self.props.iter().filter(move |p| p.name == name)
    }
    fn text(&self, name: &str) -> String {
        self.get(name)
            .map(|p| unescape(&p.value))
            .unwrap_or_default()
    }
    fn stamp(&self, name: &str, clock: &mut Clock) -> Option<Stamp> {
        let p = self.get(name)?;
        parse_stamp(&p.value, p.param("TZID"), clock)
    }
}

/// Splits the file into VEVENTs (ignoring nested VALARMs) and learns VTIMEZONE locations.
fn components(ics: &str, clock: &mut Clock) -> (Vec<Raw>, Option<String>) {
    let mut events = Vec::new();
    let mut cal_name = None;
    let mut stack: Vec<String> = Vec::new();
    let mut cur: Option<Raw> = None;
    let mut tz: (Option<String>, Option<String>) = (None, None);
    for line in unfold(ics) {
        let Some(p) = parse_line(&line) else {
            continue;
        };
        match p.name.as_str() {
            "BEGIN" => {
                let c = p.value.trim().to_ascii_uppercase();
                if c == "VEVENT" && stack.last().is_some_and(|s| s == "VCALENDAR") {
                    cur = Some(Raw::default());
                }
                if c == "VTIMEZONE" {
                    tz = (None, None);
                }
                stack.push(c);
            }
            "END" => {
                let c = stack.pop().unwrap_or_default();
                if c == "VEVENT" {
                    if let Some(r) = cur.take() {
                        events.push(r);
                    }
                } else if c == "VTIMEZONE" {
                    if let (Some(id), Some(loc)) = tz.clone() {
                        let z = resolve_tzid(&loc);
                        clock.tzids.insert(id, z);
                    }
                }
            }
            _ => match stack.last().map(String::as_str) {
                Some("VEVENT") => {
                    if let Some(r) = cur.as_mut() {
                        r.props.push(p);
                    }
                }
                Some("VTIMEZONE") => match p.name.as_str() {
                    "TZID" => tz.0 = Some(p.value.trim().to_string()),
                    "X-LIC-LOCATION" => tz.1 = Some(p.value.trim().to_string()),
                    _ => {}
                },
                Some("VCALENDAR") if p.name == "X-WR-CALNAME" => {
                    let n = unescape(p.value.trim());
                    if !n.is_empty() {
                        cal_name = Some(n);
                    }
                }
                _ => {}
            },
        }
    }
    (events, cal_name)
}

/// Parses an .ics feed into the events that overlap [from, to] (unix seconds). Uses the
/// feed's own name (X-WR-CALNAME) when it has one.
pub fn parse_ics(ics: &str, cal_name: &str, color: u32, from: i64, to: i64) -> Vec<Event> {
    let mut clock = Clock::default();
    let (raws, own_name) = components(ics, &mut clock);
    let cal_name = own_name.unwrap_or_else(|| cal_name.to_string());

    // single instances that were moved or cancelled replace the series' own occurrence
    let mut replaced: HashMap<String, HashSet<i64>> = HashMap::new();
    for r in &raws {
        if let (Some(uid), Some(rid)) = (r.get("UID"), r.stamp("RECURRENCE-ID", &mut clock)) {
            let at = clock.utc(rid.wall, &rid.zone);
            replaced.entry(uid.value.clone()).or_default().insert(at);
        }
    }

    let mut out = Vec::new();
    for r in &raws {
        if r.get("STATUS")
            .is_some_and(|p| p.value.trim().eq_ignore_ascii_case("CANCELLED"))
        {
            continue;
        }
        let Some(start) = r.stamp("DTSTART", &mut clock) else {
            continue;
        };
        let start_utc = clock.utc(start.wall, &start.zone);
        let end_utc = r
            .stamp("DTEND", &mut clock)
            .map(|e| clock.utc(e.wall, &e.zone))
            .or_else(|| {
                r.get("DURATION")
                    .and_then(|p| parse_duration(&p.value))
                    .map(|d| start_utc + d)
            })
            .unwrap_or(start_utc + if start.all_day { 86_400 } else { 0 });
        let dur = (end_utc - start_utc).max(0);

        let title = Some(r.text("SUMMARY"))
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| "(no title)".into());
        let location = r.text("LOCATION");
        let join = find_meeting_link(&format!(
            "{location} {} {}",
            r.text("URL"),
            r.text("DESCRIPTION")
        ));
        let uid = r
            .get("UID")
            .map(|p| p.value.clone())
            .unwrap_or_else(|| format!("{start_utc}{title}"));
        let mut push = |s: i64| {
            if s + dur.max(1) > from && s <= to {
                out.push(Event {
                    id: format!("{uid}#{s}"),
                    title: title.clone(),
                    start: s,
                    end: s + dur,
                    all_day: start.all_day,
                    location: location.clone(),
                    join_url: join.clone(),
                    calendar: cal_name.clone(),
                    source: "ics".into(),
                    color,
                });
            }
        };

        let rule = r
            .get("RRULE")
            .and_then(|p| parse_rule(&p.value, &mut clock));
        let Some(rule) = rule.filter(|_| r.get("RECURRENCE-ID").is_none()) else {
            push(start_utc);
            continue;
        };
        let mut skip: HashSet<i64> = replaced.get(&uid).cloned().unwrap_or_default();
        for p in r.all("EXDATE") {
            for v in p.value.split(',') {
                if let Some(x) = parse_stamp(v, p.param("TZID"), &mut clock) {
                    // a date-only EXDATE on a timed series means that day's occurrence
                    let wall = if x.all_day && !start.all_day {
                        x.wall + start.wall.rem_euclid(86_400)
                    } else {
                        x.wall
                    };
                    let zone = if x.all_day {
                        start.zone.clone()
                    } else {
                        x.zone
                    };
                    skip.insert(clock.utc(wall, &zone));
                }
            }
        }
        let until_utc = rule.until.as_ref().map(|u| {
            if u.all_day {
                // a date: through the end of that day in the event's own clock
                clock.utc(u.wall + 86_399, &start.zone)
            } else {
                clock.utc(u.wall, &u.zone)
            }
        });
        let time_of_day = start.wall.rem_euclid(86_400);
        let mut n = 0u32;
        for_each_day(start.wall.div_euclid(86_400), &rule, |day| {
            if rule.count.is_some_and(|c| n >= c) {
                return false;
            }
            n += 1;
            let wall = day * 86_400 + time_of_day;
            if wall - MAX_OFFSET > to {
                return false;
            }
            if let Some(u) = until_utc {
                if wall - MAX_OFFSET > u {
                    return false;
                }
            }
            // far before the window: no need for the exact UTC time
            if wall + dur + MAX_OFFSET < from {
                return true;
            }
            let s = clock.utc(wall, &start.zone);
            if until_utc.is_some_and(|u| s > u) {
                return false;
            }
            if !skip.contains(&s) {
                push(s);
            }
            true
        });
    }
    out.sort_by_key(|e| e.start);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(y: i32, m: u32, d: u32, h: i64, mi: i64) -> i64 {
        days_from_civil(y, m, d) * 86_400 + h * 3600 + mi * 60
    }

    fn cal(body: &str) -> String {
        format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\n{body}END:VCALENDAR\r\n")
    }

    fn event(lines: &[&str]) -> String {
        let mut s = String::from("BEGIN:VEVENT\r\n");
        for l in lines {
            s.push_str(l);
            s.push_str("\r\n");
        }
        s.push_str("END:VEVENT\r\n");
        s
    }

    fn starts(ics: &str, from: i64, to: i64) -> Vec<i64> {
        parse_ics(ics, "c", 0, from, to)
            .into_iter()
            .map(|e| e.start)
            .collect()
    }

    #[test]
    fn basic_event_with_folding_escapes_and_link() {
        let ics = cal(&event(&[
            "UID:1",
            "DTSTART:20300101T090000Z",
            "DTEND:20300101T100000Z",
            "SUMMARY:Standup\\, daily",
            "DESCRIPTION:Join https://meet.google.com/abc-defg-hij",
            "  here",
            "RRULE:FREQ=DAILY;COUNT=3",
        ]));
        let t = utc(2030, 1, 1, 9, 0);
        let ev = parse_ics(&ics, "c", 0, t - 3600, t + 10 * 86_400);
        assert_eq!(ev.len(), 3);
        assert_eq!(ev[0].title, "Standup, daily");
        assert_eq!(ev[0].start, t);
        assert_eq!(ev[0].end, t + 3600);
        assert_eq!(ev[1].start - ev[0].start, 86_400);
        assert_eq!(
            ev[0].join_url.as_deref(),
            Some("https://meet.google.com/abc-defg-hij")
        );
    }

    #[test]
    fn weekly_by_day_with_interval_and_until() {
        // every other week on Mon and Wed, until 2030-01-31
        let ics = cal(&event(&[
            "UID:w",
            "DTSTART:20300107T080000Z", // a Monday
            "DURATION:PT30M",
            "RRULE:FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,WE;UNTIL=20300131T235959Z",
        ]));
        let got = starts(&ics, utc(2030, 1, 1, 0, 0), utc(2030, 3, 1, 0, 0));
        let want: Vec<i64> = [7, 9, 21, 23]
            .iter()
            .map(|d| utc(2030, 1, *d, 8, 0))
            .collect();
        assert_eq!(got, want);
    }

    #[test]
    fn monthly_by_nth_weekday_and_by_month_day() {
        // second Tuesday of each month
        let ics = cal(&event(&[
            "UID:m",
            "DTSTART:20300108T100000Z",
            "RRULE:FREQ=MONTHLY;BYDAY=2TU;COUNT=3",
        ]));
        let got = starts(&ics, 0, utc(2031, 1, 1, 0, 0));
        assert_eq!(
            got,
            vec![
                utc(2030, 1, 8, 10, 0),
                utc(2030, 2, 12, 10, 0),
                utc(2030, 3, 12, 10, 0)
            ]
        );
        // last Friday
        let ics = cal(&event(&[
            "UID:l",
            "DTSTART:20300125T100000Z",
            "RRULE:FREQ=MONTHLY;BYDAY=-1FR;COUNT=2",
        ]));
        assert_eq!(
            starts(&ics, 0, utc(2031, 1, 1, 0, 0)),
            vec![utc(2030, 1, 25, 10, 0), utc(2030, 2, 22, 10, 0)]
        );
        // the 31st skips months without one
        let ics = cal(&event(&[
            "UID:d",
            "DTSTART:20300131T100000Z",
            "RRULE:FREQ=MONTHLY;COUNT=3",
        ]));
        assert_eq!(
            starts(&ics, 0, utc(2031, 1, 1, 0, 0)),
            vec![
                utc(2030, 1, 31, 10, 0),
                utc(2030, 3, 31, 10, 0),
                utc(2030, 5, 31, 10, 0)
            ]
        );
    }

    #[test]
    fn yearly_birthday_all_day() {
        let ics = cal(&event(&[
            "UID:b",
            "DTSTART;VALUE=DATE:20280229",
            "RRULE:FREQ=YEARLY",
            "SUMMARY:Leap day",
        ]));
        let ev = parse_ics(&ics, "c", 0, utc(2029, 1, 1, 0, 0), utc(2033, 1, 1, 0, 0));
        // only leap years have the day
        assert_eq!(ev.len(), 1);
        assert!(ev[0].all_day);
        let (y, m, d) = civil_from_days(
            (ev[0].start + crate::util::local_offset_at(ev[0].start)).div_euclid(86_400),
        );
        assert_eq!((y, m, d), (2032, 2, 29));
        assert_eq!(ev[0].end - ev[0].start, 86_400);
    }

    #[test]
    fn exdate_and_moved_or_cancelled_instances() {
        let ics = cal(&format!(
            "{}{}{}",
            event(&[
                "UID:s",
                "DTSTART:20300101T090000Z",
                "DTEND:20300101T093000Z",
                "RRULE:FREQ=DAILY;COUNT=5",
                "EXDATE:20300102T090000Z,20300103T090000Z",
                "SUMMARY:Sync",
            ]),
            // the 4th moved to the afternoon
            event(&[
                "UID:s",
                "RECURRENCE-ID:20300104T090000Z",
                "DTSTART:20300104T150000Z",
                "DTEND:20300104T153000Z",
                "SUMMARY:Sync (moved)",
            ]),
            // the 5th cancelled
            event(&[
                "UID:s",
                "RECURRENCE-ID:20300105T090000Z",
                "DTSTART:20300105T090000Z",
                "STATUS:CANCELLED",
            ]),
        ));
        let ev = parse_ics(&ics, "c", 0, 0, utc(2031, 1, 1, 0, 0));
        let got: Vec<(i64, &str)> = ev.iter().map(|e| (e.start, e.title.as_str())).collect();
        assert_eq!(
            got,
            vec![
                (utc(2030, 1, 1, 9, 0), "Sync"),
                (utc(2030, 1, 4, 15, 0), "Sync (moved)"),
            ]
        );
    }

    #[test]
    fn tzid_iana_windows_and_vtimezone_and_dst() {
        // 09:00 in New York: 14:00 UTC in winter, 13:00 UTC in summer (same wall time)
        let iana = cal(&event(&[
            "UID:ny",
            "DTSTART;TZID=America/New_York:20300301T090000",
            "RRULE:FREQ=MONTHLY;COUNT=2",
        ]));
        assert_eq!(
            starts(&iana, 0, utc(2031, 1, 1, 0, 0)),
            vec![utc(2030, 3, 1, 14, 0), utc(2030, 4, 1, 13, 0)]
        );
        // Outlook writes Windows names
        let win = cal(&event(&[
            "UID:be",
            "DTSTART;TZID=\"W. Europe Standard Time\":20300701T090000",
        ]));
        assert_eq!(
            starts(&win, 0, utc(2031, 1, 1, 0, 0)),
            vec![utc(2030, 7, 1, 7, 0)]
        );
        // a custom TZID explained by its VTIMEZONE
        let custom = cal(&format!(
            "BEGIN:VTIMEZONE\r\nTZID:My Zone\r\nX-LIC-LOCATION:Asia/Tokyo\r\nEND:VTIMEZONE\r\n{}",
            event(&["UID:tk", "DTSTART;TZID=My Zone:20300101T090000"])
        ));
        assert_eq!(
            starts(&custom, 0, utc(2031, 1, 1, 0, 0)),
            vec![utc(2030, 1, 1, 0, 0)]
        );
    }

    #[test]
    fn long_running_daily_series_still_shows_up() {
        // started 20 years before the window: the old 2000-occurrence cap hid it
        let ics = cal(&event(&[
            "UID:old",
            "DTSTART:20100101T070000Z",
            "RRULE:FREQ=DAILY",
        ]));
        let from = utc(2030, 6, 1, 0, 0);
        let got = starts(&ics, from, from + 3 * 86_400);
        assert_eq!(got.len(), 3);
        assert_eq!(got[0], utc(2030, 6, 1, 7, 0));
    }

    #[test]
    fn alarms_and_calendar_name() {
        let ics = cal(&format!(
            "X-WR-CALNAME:Team\r\n{}",
            "BEGIN:VEVENT\r\nUID:a\r\nDTSTART:20300101T090000Z\r\nSUMMARY:Real\r\n\
             BEGIN:VALARM\r\nSUMMARY:Alarm text\r\nTRIGGER:-PT10M\r\nEND:VALARM\r\nEND:VEVENT\r\n"
        ));
        let ev = parse_ics(&ics, "fallback", 0, 0, utc(2031, 1, 1, 0, 0));
        assert_eq!(ev.len(), 1);
        assert_eq!(ev[0].title, "Real");
        assert_eq!(ev[0].calendar, "Team");
    }

    #[test]
    fn durations_and_lines() {
        assert_eq!(parse_duration("PT1H30M"), Some(5400));
        assert_eq!(parse_duration("P1D"), Some(86_400));
        assert_eq!(parse_duration("P2W"), Some(14 * 86_400));
        assert_eq!(parse_duration("-PT15M"), Some(-900));
        assert_eq!(parse_duration("1H"), None);
        let p = parse_line("DTSTART;TZID=\"a:b\";X=1:20300101T090000").unwrap();
        assert_eq!(p.name, "DTSTART");
        assert_eq!(p.param("tzid"), Some("a:b"));
        assert_eq!(p.value, "20300101T090000");
    }
}
