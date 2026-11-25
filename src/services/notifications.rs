//! Notification mirror via UserNotificationListener. Requires package identity
//! (MSIX/Store or a registered sparse package); degrades gracefully otherwise.

use std::collections::HashSet;
use std::time::Duration;

use windows::UI::Notifications::Management::{
    UserNotificationListener, UserNotificationListenerAccessStatus,
};
use windows::UI::Notifications::{KnownNotificationBindings, NotificationKinds};

use super::{com_mta, spawn, Bus, Event};
use crate::util::log;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Allowed,
    Denied,
    /// The process has no package identity, so the API is unavailable.
    NoIdentity,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Toast {
    pub id: u32,
    pub app: String,
    pub app_id: String,
    pub title: String,
    pub body: String,
}

pub fn has_identity() -> bool {
    windows::ApplicationModel::Package::Current().is_ok()
}

/// True for the Microsoft Store package (updates come from the Store), false for the direct
/// installer, whose sparse identity package only exists to enable notification mirroring.
pub fn is_store_package() -> bool {
    windows::ApplicationModel::Package::Current()
        .and_then(|p| p.Id())
        .and_then(|id| id.Name())
        .is_ok_and(|n| n != "Notchly.Sparse")
}

pub fn start(bus: Bus) {
    spawn("notifications", move || {
        com_mta();
        if !has_identity() {
            store_mirror(bus);
            return;
        }
        let Ok(listener) = UserNotificationListener::Current() else {
            store_mirror(bus);
            return;
        };
        let status = listener.RequestAccessAsync().and_then(|o| o.join());
        match status {
            Ok(UserNotificationListenerAccessStatus::Allowed) => {
                bus.send(Event::NotificationAccess(Access::Allowed))
            }
            Ok(_) => {
                bus.send(Event::NotificationAccess(Access::Denied));
                return;
            }
            Err(e) => {
                log(&format!(
                    "notification access failed: {e}; reading the notification store"
                ));
                store_mirror(bus);
                return;
            }
        }
        // Prefer the NotificationChanged event; if this configuration refuses it
        // (it can require a background task), fall back to a cheap poll.
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let evented = listener
            .NotificationChanged(&windows::Foundation::TypedEventHandler::new(move |_, _| {
                let _ = tx.send(());
                Ok(())
            }))
            .is_ok();
        let mut seen: HashSet<u32> = HashSet::new();
        let mut first = true;
        loop {
            if !first {
                if evented {
                    if rx.recv().is_err() {
                        return;
                    }
                    while rx.try_recv().is_ok() {}
                } else {
                    std::thread::sleep(Duration::from_millis(1500));
                }
            }
            if let Ok(list) = listener
                .GetNotificationsAsync(NotificationKinds::Toast)
                .and_then(|o| o.join())
            {
                let mut current = HashSet::new();
                for i in 0..list.Size().unwrap_or(0) {
                    let Ok(n) = list.GetAt(i) else { continue };
                    let id = n.Id().unwrap_or(0);
                    current.insert(id);
                    if seen.contains(&id) || first {
                        continue;
                    }
                    let app = n
                        .AppInfo()
                        .and_then(|a| a.DisplayInfo())
                        .and_then(|d| d.DisplayName())
                        .map(|s| s.to_string())
                        .unwrap_or_default();
                    let app_id = n
                        .AppInfo()
                        .and_then(|a| a.AppUserModelId())
                        .map(|s| s.to_string())
                        .unwrap_or_default();
                    let mut texts = Vec::new();
                    if let Ok(binding) = n.Notification().and_then(|t| t.Visual()).and_then(|v| {
                        v.GetBinding(&KnownNotificationBindings::ToastGeneric().unwrap_or_default())
                    }) {
                        if let Ok(els) = binding.GetTextElements() {
                            for j in 0..els.Size().unwrap_or(0) {
                                if let Ok(t) = els.GetAt(j).and_then(|e| e.Text()) {
                                    texts.push(t.to_string());
                                }
                            }
                        }
                    }
                    let title = texts.first().cloned().unwrap_or_else(|| app.clone());
                    let body = texts.iter().skip(1).cloned().collect::<Vec<_>>().join("\n");
                    bus.send(Event::Notification(Toast {
                        id,
                        app,
                        app_id,
                        title,
                        body,
                    }));
                }
                seen = current;
            }
            first = false;
        }
    });
}

// ---------------------------------------------------------------- notification store mirror

/// Windows keeps the Action Center's notifications in a local SQLite store. When the listener
/// API isn't available (no package identity), Notchly reads that store read-only: the same
/// toasts, on this PC only, nothing written.
fn store_path() -> std::path::PathBuf {
    // the real Local AppData from the shell, not %LOCALAPPDATA% (which a launcher can change)
    let base = unsafe {
        use windows::Win32::UI::Shell::{
            FOLDERID_LocalAppData, SHGetKnownFolderPath, KF_FLAG_DEFAULT,
        };
        SHGetKnownFolderPath(&FOLDERID_LocalAppData, KF_FLAG_DEFAULT, None)
            .ok()
            .and_then(|p| {
                let s = p.to_string().ok();
                windows::Win32::System::Com::CoTaskMemFree(Some(p.0 as _));
                s
            })
    }
    .unwrap_or_else(|| std::env::var("LOCALAPPDATA").unwrap_or_default());
    std::path::Path::new(&base).join(r"Microsoft\Windows\Notifications\wpndatabase.db")
}

/// FILETIME (100 ns since 1601) to Unix seconds.
fn filetime_to_unix(t: i64) -> i64 {
    t / 10_000_000 - 11_644_473_600
}

/// The text lines of a toast's XML payload, in order (title first).
pub fn toast_texts(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find("<text") {
        rest = &rest[i + 5..];
        // `<textBox` and friends aren't text elements
        if !rest.starts_with([' ', '>', '/', '\t', '\n']) {
            continue;
        }
        let Some(gt) = rest.find('>') else { break };
        if rest[..gt].ends_with('/') {
            rest = &rest[gt + 1..];
            continue;
        }
        let body = &rest[gt + 1..];
        let Some(end) = body.find("</text>") else {
            break;
        };
        let t = decode_entities(body[..end].trim());
        if !t.is_empty() {
            out.push(t);
        }
        rest = &body[end + 7..];
    }
    out
}

fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(semi) = rest.find(';').filter(|&j| j <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let ent = &rest[1..semi];
        let ch = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ if ent.starts_with("#x") || ent.starts_with("#X") => {
                u32::from_str_radix(&ent[2..], 16)
                    .ok()
                    .and_then(char::from_u32)
            }
            _ if ent.starts_with('#') => ent[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => out.push(c),
            None => out.push_str(&rest[..=semi]),
        }
        rest = &rest[semi + 1..];
    }
    out.push_str(rest);
    out
}

/// The app's own display name, asked from Windows (packaged apps), cached per id.
fn display_name(aumid: &str) -> Option<String> {
    use std::collections::HashMap;
    use std::sync::Mutex;
    static CACHE: Mutex<Option<HashMap<String, Option<String>>>> = Mutex::new(None);
    let mut g = CACHE.lock().ok()?;
    let map = g.get_or_insert_with(HashMap::new);
    if let Some(v) = map.get(aumid) {
        return v.clone();
    }
    let name = windows::ApplicationModel::AppInfo::GetFromAppUserModelId(
        &windows::core::HSTRING::from(aumid),
    )
    .and_then(|a| a.DisplayInfo())
    .and_then(|d| d.DisplayName())
    .map(|s| s.to_string())
    .ok()
    .filter(|s| !s.trim().is_empty() && !s.starts_with("ms-resource:"));
    map.insert(aumid.to_string(), name.clone());
    name
}

/// A readable app name from an app user model id.
pub fn app_name(aumid: &str) -> String {
    if let Some(n) = display_name(aumid) {
        return n;
    }
    let l = aumid.to_lowercase();
    let known = [
        ("whatsapp", "WhatsApp"),
        ("telegram", "Telegram"),
        ("discord", "Discord"),
        ("slack", "Slack"),
        ("teams", "Microsoft Teams"),
        ("outlook", "Outlook"),
        ("windowscommunicationsapps", "Mail"),
        ("chrome", "Google Chrome"),
        ("msedge", "Microsoft Edge"),
        ("microsoftedge", "Microsoft Edge"),
        ("firefox", "Firefox"),
        ("screensketch", "Snipping Tool"),
        ("windows.explorer", "File Explorer"),
        ("securityhealth", "Windows Security"),
        ("windows.defender", "Windows Security"),
        ("spotify", "Spotify"),
        ("yourphone", "Phone Link"),
        ("phonelink", "Phone Link"),
        ("zoom", "Zoom"),
        ("messenger", "Messenger"),
        ("instagram", "Instagram"),
        ("notchly", "Notchly"),
        ("windows.systemtoast", "Windows"),
        ("windowsupdate", "Windows Update"),
    ];
    for (k, v) in known {
        if l.contains(k) {
            return v.to_string();
        }
    }
    // "Publisher.AppName_hash!App" or a path to an exe
    let base = if aumid.contains('\\') {
        aumid
            .rsplit('\\')
            .next()
            .unwrap_or(aumid)
            .trim_end_matches(".exe")
            .to_string()
    } else {
        let no_suffix = aumid.split(['_', '!']).next().unwrap_or(aumid);
        no_suffix
            .rsplit('.')
            .next()
            .unwrap_or(no_suffix)
            .to_string()
    };
    // split CamelCase: "PredatorSenseforNotebook" -> "Predator Sensefor Notebook" is ugly;
    // only split on lower→upper boundaries
    let mut out = String::new();
    let mut prev_lower = false;
    for c in base.chars() {
        if c.is_uppercase() && prev_lower {
            out.push(' ');
        }
        prev_lower = c.is_lowercase();
        out.push(c);
    }
    if out.is_empty() {
        "App".into()
    } else {
        out
    }
}

type StoreRow = (i64, i64, Vec<u8>, String);

fn store_rows(conn: &rusqlite::Connection, since: i64) -> rusqlite::Result<Vec<StoreRow>> {
    let mut st = conn.prepare_cached(
        "SELECT n.Id, n.ArrivalTime, n.Payload, h.PrimaryId
         FROM Notification n JOIN NotificationHandler h ON h.RecordId = n.HandlerId
         WHERE n.Type = 'toast' AND n.ArrivalTime > ?1
         ORDER BY n.ArrivalTime LIMIT 50",
    )?;
    let rows = st.query_map([since], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, Option<Vec<u8>>>(2)?.unwrap_or_default(),
            r.get::<_, String>(3)?,
        ))
    })?;
    Ok(rows.flatten().collect())
}

fn to_toast(row: &StoreRow) -> Option<(Toast, i64)> {
    let (id, arrival, payload, aumid) = row;
    let xml = String::from_utf8_lossy(payload);
    let texts = toast_texts(&xml);
    if texts.is_empty() {
        return None;
    }
    let app = app_name(aumid);
    Some((
        Toast {
            id: *id as u32,
            app: app.clone(),
            app_id: aumid.clone(),
            title: texts[0].clone(),
            body: texts[1..].join("\n"),
        },
        filetime_to_unix(*arrival),
    ))
}

fn store_mirror(bus: Bus) {
    use rusqlite::{Connection, OpenFlags};
    let path = store_path();
    let open = || {
        Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .ok()
    };
    let Some(mut conn) = open() else {
        log("notifications: store unavailable");
        bus.send(Event::NotificationAccess(Access::NoIdentity));
        return;
    };
    let _ = conn.busy_timeout(Duration::from_millis(500));
    // what's already in the Action Center fills the history (no pop-ups for old ones)
    let existing = store_rows(&conn, 0).unwrap_or_default();
    let mut last = existing.iter().map(|r| r.1).max().unwrap_or(0);
    let seed: Vec<(Toast, i64)> = existing
        .iter()
        .rev()
        .take(12)
        .filter_map(to_toast)
        .collect();
    log(&format!(
        "notifications: reading the notification store ({} in Action Center)",
        existing.len()
    ));
    bus.send(Event::NotificationAccess(Access::Allowed));
    bus.send(Event::NotificationsSeed(seed));
    let mut fails = 0;
    loop {
        std::thread::sleep(Duration::from_millis(1500));
        match store_rows(&conn, last) {
            Ok(rows) => {
                fails = 0;
                for r in &rows {
                    last = last.max(r.1);
                    if let Some((t, _)) = to_toast(r) {
                        bus.send(Event::Notification(t));
                    }
                }
            }
            Err(_) => {
                fails += 1;
                if fails > 3 {
                    std::thread::sleep(Duration::from_secs(5));
                    if let Some(c) = open() {
                        conn = c;
                        let _ = conn.busy_timeout(Duration::from_millis(500));
                        fails = 0;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod store_tests {
    use super::*;

    /// Reads this PC's real notification store read-only (prints a count, never content).
    /// Machine-dependent, so ignored by default: `cargo test real_store -- --ignored`.
    #[test]
    #[ignore]
    fn real_store_opens_and_reads() {
        use rusqlite::{Connection, OpenFlags};
        let conn = Connection::open_with_flags(
            store_path(),
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .expect("the store opens read-only");
        let rows = store_rows(&conn, 0).expect("the query runs");
        println!("notification store rows: {}", rows.len());
    }

    #[test]
    fn parses_toast_xml() {
        let xml = r#"<toast launch="x"><visual><binding template="ToastGeneric"><text hint-maxLines="1">Ravi &amp; Asha</text><text>See you at 5 &#x1F44B;</text><text placement="attribution"/><image src="a.png"/></binding></visual></toast>"#;
        assert_eq!(toast_texts(xml), vec!["Ravi & Asha", "See you at 5 👋"]);
    }

    #[test]
    fn app_names() {
        assert_eq!(
            app_name("5319275A.WhatsAppDesktop_cv1g1gvanyjgm!App"),
            "WhatsApp"
        );
        assert_eq!(
            app_name("Microsoft.ScreenSketch_8wekyb3d8bbwe!App"),
            "Snipping Tool"
        );
        assert_eq!(app_name(r"C:\Tools\MyApp.exe"), "My App");
        assert_eq!(filetime_to_unix(134_398_715_344_077_828), 1_795_397_934);
    }
}
