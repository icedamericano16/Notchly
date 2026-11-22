//! Calendar: ICS URLs, Google Calendar and Microsoft 365 / Outlook.com.
//! OAuth uses the desktop loopback flow with PKCE; refresh tokens live in
//! Windows Credential Manager.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{http, spawn, Bus, Event as BusEvent};
use crate::platform::credentials;
use crate::services::weather::parse_iso;
use crate::util::{days_from_civil, local_offset_secs, unix_secs};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub title: String,
    pub start: i64,
    pub end: i64,
    pub all_day: bool,
    pub location: String,
    pub join_url: Option<String>,
    pub calendar: String,
    pub color: u32,
}

/// Finds a Teams / Zoom / Meet / Webex link in free text.
pub fn find_meeting_link(text: &str) -> Option<String> {
    for w in text.split(|c: char| {
        c.is_whitespace() || c == '<' || c == '>' || c == '"' || c == '(' || c == ')'
    }) {
        let l = w.to_lowercase();
        if l.starts_with("https://")
            && (l.contains("teams.microsoft.com")
                || l.contains("teams.live.com")
                || l.contains("zoom.us/")
                || l.contains("meet.google.com")
                || l.contains("webex.com"))
        {
            return Some(w.trim_end_matches(['.', ',', ';']).to_string());
        }
    }
    None
}

// ---------------------------------------------------------------- ICS

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

/// Parses "20261122T093000Z", "20261122T093000" (local) or "20261122" (all-day).
pub fn parse_ics_time(v: &str, tz_utc: bool) -> Option<(i64, bool)> {
    let v = v.trim();
    let y: i32 = v.get(0..4)?.parse().ok()?;
    let m: u32 = v.get(4..6)?.parse().ok()?;
    let d: u32 = v.get(6..8)?.parse().ok()?;
    let day = days_from_civil(y, m, d);
    if v.len() < 15 {
        return Some((day * 86400 - local_offset_secs(), true));
    }
    let h: i64 = v.get(9..11)?.parse().ok()?;
    let mi: i64 = v.get(11..13)?.parse().ok()?;
    let s: i64 = v.get(13..15)?.parse().ok()?;
    let mut t = day * 86400 + h * 3600 + mi * 60 + s;
    if !(v.ends_with('Z') || tz_utc) {
        t -= local_offset_secs();
    }
    Some((t, false))
}

fn unescape(s: &str) -> String {
    s.replace("\\n", "\n")
        .replace("\\N", "\n")
        .replace("\\,", ",")
        .replace("\\;", ";")
        .replace("\\\\", "\\")
}

/// Parses VEVENTs, expanding simple DAILY/WEEKLY RRULEs inside [from, to].
pub fn parse_ics(ics: &str, cal_name: &str, color: u32, from: i64, to: i64) -> Vec<Event> {
    let mut out = Vec::new();
    let mut cur: Option<Vec<(String, String, String)>> = None;
    for line in unfold(ics) {
        if line == "BEGIN:VEVENT" {
            cur = Some(Vec::new());
        } else if line == "END:VEVENT" {
            if let Some(props) = cur.take() {
                let get = |k: &str| props.iter().find(|p| p.0 == k);
                let Some((_, sp, sv)) = get("DTSTART") else {
                    continue;
                };
                let Some((start, all_day)) = parse_ics_time(sv, sp.contains("UTC")) else {
                    continue;
                };
                let end = get("DTEND")
                    .and_then(|(_, p, v)| parse_ics_time(v, p.contains("UTC")))
                    .map(|e| e.0)
                    .unwrap_or(start + if all_day { 86400 } else { 3600 });
                let title = get("SUMMARY")
                    .map(|p| unescape(&p.2))
                    .unwrap_or_else(|| "(no title)".into());
                let location = get("LOCATION").map(|p| unescape(&p.2)).unwrap_or_default();
                let desc = get("DESCRIPTION")
                    .map(|p| unescape(&p.2))
                    .unwrap_or_default();
                let url = get("URL").map(|p| p.2.clone()).unwrap_or_default();
                let join = find_meeting_link(&format!("{location} {url} {desc}"));
                let uid = get("UID")
                    .map(|p| p.2.clone())
                    .unwrap_or_else(|| format!("{start}{title}"));
                let rrule = get("RRULE").map(|p| p.2.clone()).unwrap_or_default();
                let dur = end - start;
                let mut push = |s: i64, n: usize| {
                    if s + dur >= from && s <= to {
                        out.push(Event {
                            id: format!("{uid}#{n}"),
                            title: title.clone(),
                            start: s,
                            end: s + dur,
                            all_day,
                            location: location.clone(),
                            join_url: join.clone(),
                            calendar: cal_name.into(),
                            color,
                        });
                    }
                };
                if rrule.is_empty() {
                    push(start, 0);
                } else {
                    let step = if rrule.contains("FREQ=DAILY") {
                        86400
                    } else if rrule.contains("FREQ=WEEKLY") {
                        7 * 86400
                    } else {
                        0
                    };
                    let interval: i64 = rrule
                        .split(';')
                        .find_map(|p| p.strip_prefix("INTERVAL="))
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(1);
                    let count: Option<usize> = rrule
                        .split(';')
                        .find_map(|p| p.strip_prefix("COUNT="))
                        .and_then(|v| v.parse().ok());
                    let until = rrule
                        .split(';')
                        .find_map(|p| p.strip_prefix("UNTIL="))
                        .and_then(|v| parse_ics_time(v, false))
                        .map(|u| u.0);
                    if step == 0 {
                        push(start, 0);
                    } else {
                        let mut s = start;
                        let mut n = 0;
                        while s <= to
                            && count.is_none_or(|c| n < c)
                            && until.is_none_or(|u| s <= u)
                            && n < 2000
                        {
                            push(s, n);
                            s += step * interval;
                            n += 1;
                        }
                    }
                }
            }
        } else if let Some(c) = cur.as_mut() {
            if let Some((k, v)) = line.split_once(':') {
                let (name, params) = k.split_once(';').unwrap_or((k, ""));
                c.push((name.to_uppercase(), params.to_uppercase(), v.to_string()));
            }
        }
    }
    out
}

// ---------------------------------------------------------------- OAuth

pub struct OAuthProvider {
    pub id: &'static str,
    pub auth_url: &'static str,
    pub token_url: &'static str,
    pub scope: &'static str,
}

pub const GOOGLE: OAuthProvider = OAuthProvider {
    id: "google",
    auth_url: "https://accounts.google.com/o/oauth2/v2/auth",
    token_url: "https://oauth2.googleapis.com/token",
    scope: "https://www.googleapis.com/auth/calendar.readonly",
};
pub const MICROSOFT: OAuthProvider = OAuthProvider {
    id: "microsoft",
    auth_url: "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
    token_url: "https://login.microsoftonline.com/common/oauth2/v2.0/token",
    scope: "offline_access Calendars.Read",
};

fn random_string(n: usize) -> String {
    let chars = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
    let bytes = crate::platform::credentials::random_bytes(n);
    bytes
        .iter()
        .map(|b| chars[*b as usize % chars.len()] as char)
        .collect()
}

fn b64url(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        if c.len() > 1 {
            out.push(T[(n >> 6) as usize & 63] as char);
        }
        if c.len() > 2 {
            out.push(T[n as usize & 63] as char);
        }
    }
    out
}

/// Runs the interactive sign-in. Blocks the calling worker up to 5 minutes.
pub fn sign_in(p: &OAuthProvider, client_id: &str) -> Result<(), String> {
    if client_id.is_empty() {
        return Err("Enter your OAuth client ID first (see the user guide).".into());
    }
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let redirect = format!("http://127.0.0.1:{port}/");
    let verifier = random_string(64);
    let challenge = b64url(&crate::platform::credentials::sha256(verifier.as_bytes()));
    let state = random_string(24);
    let url = format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&code_challenge={challenge}&code_challenge_method=S256&state={state}&access_type=offline&prompt=consent",
        p.auth_url,
        http::enc(client_id),
        http::enc(&redirect),
        http::enc(p.scope)
    );
    crate::platform::shell_open(&url);
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(300);
    let code = loop {
        if Instant::now() > deadline {
            return Err("Sign-in timed out".into());
        }
        match listener.accept() {
            Ok((mut s, _)) => {
                let _ = s.set_nonblocking(false);
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let path = req.split_whitespace().nth(1).unwrap_or("").to_string();
                let q = path.split_once('?').map(|x| x.1).unwrap_or("");
                let param = |k: &str| {
                    q.split('&')
                        .find_map(|kv| kv.strip_prefix(&format!("{k}=")))
                        .map(String::from)
                };
                let page = "<html><body style='font-family:Segoe UI;background:#0A0D18;color:#ECEFF7;text-align:center;padding-top:80px'><h2>Notchly is connected.</h2><p>You can close this tab.</p></body></html>";
                let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}", page.len()).as_bytes());
                if param("state").as_deref() != Some(state.as_str()) {
                    continue;
                }
                match param("code") {
                    Some(c) => break c,
                    None => {
                        return Err(param("error").unwrap_or_else(|| "Sign-in cancelled".into()))
                    }
                }
            }
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    };
    let form = format!("client_id={}&code={}&code_verifier={verifier}&grant_type=authorization_code&redirect_uri={}", http::enc(client_id), http::enc(&code), http::enc(&redirect));
    let r = http::post_form(p.token_url, &form)?;
    let v: Value = serde_json::from_str(&r.body).map_err(|e| e.to_string())?;
    let refresh = v["refresh_token"].as_str().ok_or_else(|| {
        format!(
            "No refresh token: {}",
            v["error_description"].as_str().unwrap_or(&r.body)
        )
    })?;
    credentials::cred_write(&format!("Notchly/{}", p.id), refresh)?;
    Ok(())
}

pub fn sign_out(p: &OAuthProvider) {
    credentials::cred_delete(&format!("Notchly/{}", p.id));
}

pub fn is_connected(p: &OAuthProvider) -> bool {
    credentials::cred_read(&format!("Notchly/{}", p.id)).is_some()
}

fn access_token(p: &OAuthProvider, client_id: &str) -> Result<String, String> {
    let refresh = credentials::cred_read(&format!("Notchly/{}", p.id)).ok_or("not connected")?;
    let form = format!(
        "client_id={}&refresh_token={}&grant_type=refresh_token",
        http::enc(client_id),
        http::enc(&refresh)
    );
    let r = http::post_form(p.token_url, &form)?;
    let v: Value = serde_json::from_str(&r.body).map_err(|e| e.to_string())?;
    if let Some(new_refresh) = v["refresh_token"].as_str() {
        let _ = credentials::cred_write(&format!("Notchly/{}", p.id), new_refresh);
    }
    v["access_token"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| "token refresh failed".into())
}

fn iso(t: i64) -> String {
    let (y, m, d) = crate::util::civil_from_days(t.div_euclid(86400));
    let s = t.rem_euclid(86400);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        s / 3600,
        s / 60 % 60,
        s % 60
    )
}

fn google_events(client_id: &str, from: i64, to: i64) -> Result<Vec<Event>, String> {
    let tok = access_token(&GOOGLE, client_id)?;
    let url = format!("https://www.googleapis.com/calendar/v3/calendars/primary/events?singleEvents=true&orderBy=startTime&maxResults=100&timeMin={}&timeMax={}", http::enc(&iso(from)), http::enc(&iso(to)));
    let r = http::get(&url, &[("Authorization", &format!("Bearer {tok}"))])?;
    let v: Value = serde_json::from_str(&r.body).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for e in v["items"].as_array().cloned().unwrap_or_default() {
        let (start, all_day) = match (e["start"]["dateTime"].as_str(), e["start"]["date"].as_str())
        {
            (Some(dt), _) => (parse_iso(dt).unwrap_or(0), false),
            (None, Some(d)) => (
                parse_ics_time(&d.replace('-', ""), false)
                    .map(|x| x.0)
                    .unwrap_or(0),
                true,
            ),
            _ => continue,
        };
        let end = e["end"]["dateTime"]
            .as_str()
            .and_then(parse_iso)
            .unwrap_or(start + 3600);
        let text = format!(
            "{} {} {}",
            e["hangoutLink"].as_str().unwrap_or(""),
            e["location"].as_str().unwrap_or(""),
            e["description"].as_str().unwrap_or("")
        );
        out.push(Event {
            id: e["id"].as_str().unwrap_or("").into(),
            title: e["summary"].as_str().unwrap_or("(no title)").into(),
            start,
            end,
            all_day,
            location: e["location"].as_str().unwrap_or("").into(),
            join_url: e["hangoutLink"]
                .as_str()
                .map(String::from)
                .or_else(|| find_meeting_link(&text)),
            calendar: "Google".into(),
            color: 0x3DDC84,
        });
    }
    Ok(out)
}

fn microsoft_events(client_id: &str, from: i64, to: i64) -> Result<Vec<Event>, String> {
    let tok = access_token(&MICROSOFT, client_id)?;
    let url = format!("https://graph.microsoft.com/v1.0/me/calendarview?startDateTime={}&endDateTime={}&$top=100&$orderby=start/dateTime", http::enc(&iso(from)), http::enc(&iso(to)));
    let r = http::get(
        &url,
        &[
            ("Authorization", &format!("Bearer {tok}")),
            ("Prefer", "outlook.timezone=\"UTC\""),
        ],
    )?;
    let v: Value = serde_json::from_str(&r.body).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for e in v["value"].as_array().cloned().unwrap_or_default() {
        let start = e["start"]["dateTime"]
            .as_str()
            .and_then(|s| parse_iso(&format!("{}Z", &s[..19.min(s.len())])))
            .unwrap_or(0);
        let end = e["end"]["dateTime"]
            .as_str()
            .and_then(|s| parse_iso(&format!("{}Z", &s[..19.min(s.len())])))
            .unwrap_or(start + 3600);
        let join = e["onlineMeeting"]["joinUrl"]
            .as_str()
            .map(String::from)
            .or_else(|| find_meeting_link(e["bodyPreview"].as_str().unwrap_or("")));
        out.push(Event {
            id: e["id"].as_str().unwrap_or("").into(),
            title: e["subject"].as_str().unwrap_or("(no title)").into(),
            start,
            end,
            all_day: e["isAllDay"].as_bool().unwrap_or(false),
            location: e["location"]["displayName"].as_str().unwrap_or("").into(),
            join_url: join,
            calendar: "Outlook".into(),
            color: 0x2D5BFF,
        });
    }
    Ok(out)
}

/// Fetches today + 7 days from every connected source.
pub fn refresh(bus: Bus, ics: Vec<String>, google_id: String, ms_id: String) {
    spawn("calendar", move || {
        super::com_mta();
        let off = local_offset_secs();
        let today = (unix_secs() + off).div_euclid(86400);
        let from = today * 86400 - off;
        let to = from + 8 * 86400;
        let mut all = Vec::new();
        let mut errors = Vec::new();
        let palette = [0x19B8FF, 0xFF9F0A, 0xFF5E7E, 0x7B4BFF, 0x2EE6C5];
        for (i, url) in ics.iter().enumerate() {
            let u = url.replace("webcal://", "https://");
            match http::get(&u, &[]) {
                Ok(r) if r.ok() => all.extend(parse_ics(
                    &r.body,
                    "Calendar",
                    palette[i % palette.len()],
                    from,
                    to,
                )),
                Ok(r) => errors.push(format!("ICS HTTP {}", r.status)),
                Err(e) => errors.push(e),
            }
        }
        if is_connected(&GOOGLE) {
            match google_events(&google_id, from, to) {
                Ok(v) => all.extend(v),
                Err(e) => errors.push(format!("Google: {e}")),
            }
        }
        if is_connected(&MICROSOFT) {
            match microsoft_events(&ms_id, from, to) {
                Ok(v) => all.extend(v),
                Err(e) => errors.push(format!("Microsoft: {e}")),
            }
        }
        all.sort_by_key(|e| e.start);
        if all.is_empty() && !errors.is_empty() {
            bus.send(BusEvent::Calendar(Err(errors.join("; "))));
        } else {
            bus.send(BusEvent::Calendar(Ok(all)));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_ics() {
        let ics = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:1\r\nDTSTART:20300101T090000Z\r\nDTEND:20300101T100000Z\r\nSUMMARY:Standup\\, daily\r\nDESCRIPTION:Join https://meet.google.com/abc-defg-hij\r\n  here\r\nRRULE:FREQ=DAILY;COUNT=3\r\nEND:VEVENT\r\nEND:VCALENDAR";
        let start = days_from_civil(2030, 1, 1) * 86400;
        let ev = parse_ics(ics, "c", 0, start - 10, start + 10 * 86400);
        assert_eq!(ev.len(), 3);
        assert_eq!(ev[0].title, "Standup, daily");
        assert_eq!(ev[0].start, start + 9 * 3600);
        assert_eq!(ev[1].start - ev[0].start, 86400);
        assert_eq!(
            ev[0].join_url.as_deref(),
            Some("https://meet.google.com/abc-defg-hij")
        );
    }
    #[test]
    fn meeting_links() {
        assert!(find_meeting_link("x https://us02web.zoom.us/j/123 y").is_some());
        assert!(find_meeting_link("no link").is_none());
    }
    #[test]
    fn b64url_works() {
        assert_eq!(b64url(b"hello"), "aGVsbG8");
    }
}
