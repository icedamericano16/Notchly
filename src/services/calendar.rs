//! Calendar: ICS URLs, Google Calendar and Microsoft 365 / Outlook.com.
//! OAuth uses the desktop loopback flow with PKCE; refresh tokens live in
//! Windows Credential Manager.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::ics::parse_ics;
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
    /// "google", "outlook" or "ics" (which account it came from).
    #[serde(default)]
    pub source: String,
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

/// Parses "20261122T093000Z", "20261122T093000" (local) or "20261122" (all-day). Used for
/// Google's all-day dates; .ics feeds go through `ics::parse_ics`.
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
    // the narrowest pair that covers the two calls made: calendarList.list and events.list
    // (calendar.readonly would also expose calendar settings and sharing rules)
    scope: "https://www.googleapis.com/auth/calendar.calendarlist.readonly https://www.googleapis.com/auth/calendar.events.readonly",
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

/// The client to use: the user's own ID (Settings) if set, else the publisher's built-in one.
/// Google desktop clients also send a client secret (Google documents it as non-confidential).
fn client(p: &OAuthProvider, user_id: &str) -> Option<(String, Option<String>)> {
    let google = p.id == "google";
    if !user_id.trim().is_empty() {
        let secret = if google {
            credentials::get_secret("google_oauth_secret")
        } else {
            None
        };
        return Some((user_id.trim().to_string(), secret));
    }
    if google {
        crate::build_info::GOOGLE_CLIENT_ID.map(|id| {
            (
                id.to_string(),
                crate::build_info::GOOGLE_CLIENT_SECRET.map(String::from),
            )
        })
    } else {
        crate::build_info::MS_CLIENT_ID.map(|id| (id.to_string(), None))
    }
}

static GOOGLE_PENDING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static MS_PENDING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn pending_flag(p: &OAuthProvider) -> &'static std::sync::atomic::AtomicBool {
    if p.id == "google" {
        &GOOGLE_PENDING
    } else {
        &MS_PENDING
    }
}

/// A sign-in is waiting for the browser.
pub fn signing_in(p: &OAuthProvider) -> bool {
    pending_flag(p).load(std::sync::atomic::Ordering::Relaxed)
}

pub fn set_signing_in(p: &OAuthProvider, on: bool) {
    pending_flag(p).store(on, std::sync::atomic::Ordering::Relaxed);
}

/// Whether sign-in can start without the user entering a client ID.
pub fn has_builtin(p: &OAuthProvider) -> bool {
    client(p, "").is_some()
}

fn secret_param(secret: &Option<String>) -> String {
    secret
        .as_ref()
        .map(|s| format!("&client_secret={}", http::enc(s)))
        .unwrap_or_default()
}

/// Runs the interactive sign-in. Blocks the calling worker up to 5 minutes.
pub fn sign_in(p: &OAuthProvider, user_id: &str) -> Result<(), String> {
    let Some((client_id, secret)) = client(p, user_id) else {
        return Err("This build has no built-in sign-in. Add your own client ID in Settings › Calendar & weather.".into());
    };
    let client_id = client_id.as_str();
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
                // a local program connecting and sending nothing mustn't stall sign-in
                let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let path = req.split_whitespace().nth(1).unwrap_or("").to_string();
                let q = path.split_once('?').map(|x| x.1).unwrap_or("");
                // decoded: Google's codes contain "/", which arrives as %2F; sending it on
                // still encoded made the token exchange fail
                let param = |k: &str| crate::util::query_param(q, k);
                if param("state").as_deref() != Some(state.as_str()) {
                    // not the sign-in redirect (a stray request, or a wrong state)
                    let _ = s.write_all(
                        b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    );
                    continue;
                }
                let page = "<html><body style='font-family:Segoe UI;background:#0A0D18;color:#ECEFF7;text-align:center;padding-top:80px'><h2>Back to Notchly</h2><p>You can close this tab; Notchly finishes signing in.</p></body></html>";
                let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}", page.len()).as_bytes());
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
    let form = format!(
        "client_id={}&code={}&code_verifier={verifier}&grant_type=authorization_code&redirect_uri={}{}",
        http::enc(client_id),
        http::enc(&code),
        http::enc(&redirect),
        secret_param(&secret)
    );
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

fn access_token(p: &OAuthProvider, user_id: &str) -> Result<String, String> {
    let refresh = credentials::cred_read(&format!("Notchly/{}", p.id)).ok_or("not connected")?;
    let (client_id, secret) = client(p, user_id).ok_or("no OAuth client")?;
    let form = format!(
        "client_id={}&refresh_token={}&grant_type=refresh_token{}",
        http::enc(&client_id),
        http::enc(&refresh),
        secret_param(&secret)
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

/// Google: every calendar shown in the user's Google Calendar (primary, holidays, shared…),
/// each in its own colour.
fn google_events(client_id: &str, from: i64, to: i64) -> Result<Vec<Event>, String> {
    let tok = access_token(&GOOGLE, client_id)?;
    let auth = format!("Bearer {tok}");
    let list = http::get(
        "https://www.googleapis.com/calendar/v3/users/me/calendarList?minAccessRole=reader&maxResults=50",
        &[("Authorization", &auth)],
    )?;
    let lv: Value = serde_json::from_str(&list.body).map_err(|e| e.to_string())?;
    if let Some(msg) = lv["error"]["message"].as_str() {
        return Err(msg.to_string());
    }
    let mut cals: Vec<(String, String, u32)> = lv["items"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter(|c| {
            c["selected"].as_bool().unwrap_or(false) || c["primary"].as_bool().unwrap_or(false)
        })
        .map(|c| {
            let color = c["backgroundColor"]
                .as_str()
                .and_then(|h| u32::from_str_radix(h.trim_start_matches('#'), 16).ok())
                .unwrap_or(0x3DDC84);
            let name = if c["primary"].as_bool().unwrap_or(false) {
                "Google".to_string()
            } else {
                c["summaryOverride"]
                    .as_str()
                    .or(c["summary"].as_str())
                    .unwrap_or("Google")
                    .to_string()
            };
            (
                c["id"].as_str().unwrap_or("primary").to_string(),
                name,
                color,
            )
        })
        .collect();
    if cals.is_empty() {
        cals.push(("primary".into(), "Google".into(), 0x3DDC84));
    }
    crate::util::log(&format!(
        "calendar: google has {} visible calendars",
        cals.len()
    ));
    let mut out = Vec::new();
    for (id, name, color) in cals.iter().take(12) {
        let url = format!(
            "https://www.googleapis.com/calendar/v3/calendars/{}/events?singleEvents=true&orderBy=startTime&maxResults=100&timeMin={}&timeMax={}",
            http::enc(id),
            http::enc(&iso(from)),
            http::enc(&iso(to))
        );
        let Ok(r) = http::get(&url, &[("Authorization", &auth)]) else {
            continue;
        };
        let v: Value = serde_json::from_str(&r.body).unwrap_or_default();
        for e in v["items"].as_array().cloned().unwrap_or_default() {
            if e["status"].as_str() == Some("cancelled") {
                continue;
            }
            let date = |k: &str| {
                e[k]["date"]
                    .as_str()
                    .and_then(|d| parse_ics_time(&d.replace('-', ""), false).map(|x| x.0))
            };
            let (start, all_day) = match (e["start"]["dateTime"].as_str(), date("start")) {
                (Some(dt), _) => (parse_iso(dt).unwrap_or(0), false),
                (None, Some(d)) => (d, true),
                _ => continue,
            };
            let end = e["end"]["dateTime"]
                .as_str()
                .and_then(parse_iso)
                .or_else(|| date("end"))
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
                calendar: name.clone(),
                color: *color,
                source: "google".into(),
            });
        }
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
            source: "outlook".into(),
            color: 0x2D5BFF,
        });
    }
    Ok(out)
}

/// Fetches today + 7 days from every connected source.
pub fn refresh(bus: Bus, ics: Vec<String>, google_id: String, ms_id: String) {
    spawn("calendar", move || {
        super::com_mta();
        let (all, errors) = fetch_all(&ics, &google_id, &ms_id);
        if all.is_empty() && !errors.is_empty() {
            bus.send(BusEvent::Calendar(Err(errors.join("; "))));
        } else {
            bus.send(BusEvent::Calendar(Ok(all)));
        }
    });
}

/// Fetches every source for today + 7 days. Returns events and per-source errors; the log
/// gets counts only (never titles).
pub fn fetch_all(ics: &[String], google_id: &str, ms_id: &str) -> (Vec<Event>, Vec<String>) {
    {
        let off = local_offset_secs();
        let today = (unix_secs() + off).div_euclid(86400);
        let from = today * 86400 - off;
        // a month ahead: the agenda groups by day, Home shows the week
        let to = from + 31 * 86400;
        let mut all = Vec::new();
        let mut errors = Vec::new();
        let palette = [0x19B8FF, 0xFF9F0A, 0xFF5E7E, 0x7B4BFF, 0x2EE6C5];
        for (i, url) in ics.iter().enumerate() {
            let url = url.trim();
            if url.is_empty() {
                continue;
            }
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
        let mut counts = format!("ics {}", all.len());
        if is_connected(&GOOGLE) {
            match google_events(google_id, from, to) {
                Ok(v) => {
                    counts.push_str(&format!(", google {}", v.len()));
                    all.extend(v)
                }
                Err(e) => errors.push(format!("Google: {e}")),
            }
        }
        if is_connected(&MICROSOFT) {
            match microsoft_events(ms_id, from, to) {
                Ok(v) => {
                    counts.push_str(&format!(", outlook {}", v.len()));
                    all.extend(v)
                }
                Err(e) => errors.push(format!("Microsoft: {e}")),
            }
        }
        all.sort_by_key(|e| e.start);
        crate::util::log(&format!(
            "calendar: {} events ({counts}){}",
            all.len(),
            if errors.is_empty() {
                String::new()
            } else {
                format!("; errors: {}", errors.join("; "))
            }
        ));
        (all, errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
