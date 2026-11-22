//! Update checker for the direct build: a static JSON feed checked at most daily,
//! background download with SHA-256 verification, install on user confirmation.

use serde::{Deserialize, Serialize};

use crate::services::{http, spawn, Bus, Event};
use crate::util::{load_json, save_json, unix_secs};

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct Feed {
    pub version: String,
    pub url: String,
    pub sha256: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub min_os: String,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Stamp {
    checked: i64,
}

/// Compares dotted versions numerically.
pub fn newer(remote: &str, local: &str) -> bool {
    let p = |s: &str| {
        s.trim_start_matches('v')
            .split('.')
            .map(|x| x.parse::<u32>().unwrap_or(0))
            .collect::<Vec<_>>()
    };
    let (a, b) = (p(remote), p(local));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (
            a.get(i).copied().unwrap_or(0),
            b.get(i).copied().unwrap_or(0),
        );
        if x != y {
            return x > y;
        }
    }
    false
}

pub fn check(bus: Bus, force: bool) {
    if crate::services::notifications::is_store_package() {
        if force {
            bus.send(Event::Notice(
                true,
                "Updates come from the Microsoft Store".into(),
                String::new(),
            ));
        }
        return;
    }
    let Some(feed) = crate::build_info::UPDATE_FEED else {
        if force {
            bus.send(Event::Notice(
                false,
                "Update checks aren't set up".into(),
                "This build has no update feed".into(),
            ));
        }
        return;
    };
    let stamp: Stamp = load_json("cache/update.json");
    if !force && unix_secs() - stamp.checked < 86400 {
        return;
    }
    spawn("updates", move || {
        crate::services::com_mta();
        let _ = crate::util::sub_dir("cache");
        save_json(
            "cache/update.json",
            &Stamp {
                checked: unix_secs(),
            },
        );
        let failed = |why: &str| {
            if force {
                bus.send(Event::Notice(
                    false,
                    "Couldn't check for updates".into(),
                    why.into(),
                ));
            }
        };
        let Ok(r) = http::get(feed, &[]) else {
            return failed("No connection");
        };
        let Ok(f) = serde_json::from_str::<Feed>(&r.body) else {
            return failed("The update feed is unreadable");
        };
        if !newer(&f.version, env!("CARGO_PKG_VERSION")) || !f.url.starts_with("https://") {
            if force {
                bus.send(Event::Notice(
                    true,
                    "You're up to date".into(),
                    format!("Notchly {}", env!("CARGO_PKG_VERSION")),
                ));
            }
            return;
        }
        if force {
            bus.send(Event::Notice(
                true,
                format!("Downloading Notchly {}", f.version),
                "You'll be asked before it installs".into(),
            ));
        }
        // background download + verification; the installer runs only when the user confirms
        match download(&f) {
            Some(path) => bus.send(Event::UpdateAvailable(path)),
            None => failed("The download didn't verify"),
        }
    });
}

fn download(f: &Feed) -> Option<String> {
    use windows::Storage::Streams::DataReader;
    use windows::Web::Http::HttpClient;
    let c = HttpClient::new().ok()?;
    let uri =
        windows::Foundation::Uri::CreateUri(&windows::core::HSTRING::from(f.url.as_str())).ok()?;
    let resp = c.GetAsync(&uri).ok()?.join().ok()?;
    let buf = resp.Content().ok()?.ReadAsBufferAsync().ok()?.join().ok()?;
    let reader = DataReader::FromBuffer(&buf).ok()?;
    let mut bytes = vec![0u8; buf.Length().ok()? as usize];
    reader.ReadBytes(&mut bytes).ok()?;
    if crate::platform::credentials::sha256_hex(&bytes) != f.sha256.to_lowercase() {
        crate::util::log("update: checksum mismatch, discarded");
        return None;
    }
    let path = std::env::temp_dir().join(format!("Notchly-{}-setup.exe", f.version));
    std::fs::write(&path, bytes).ok()?;
    Some(path.to_string_lossy().to_string())
}

/// Runs the verified installer silently and exits.
pub fn install(path: &str) {
    let exe = windows::core::HSTRING::from(path);
    let args = windows::core::HSTRING::from("/VERYSILENT /SUPPRESSMSGBOXES /RESTARTAPPLICATIONS");
    unsafe {
        windows::Win32::UI::Shell::ShellExecuteW(
            None,
            &windows::core::HSTRING::from("open"),
            &exe,
            &args,
            None,
            windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions() {
        assert!(newer("0.2.0", "0.1.9"));
        assert!(newer("1.0", "0.9.9"));
        assert!(!newer("0.1.0", "0.1.0"));
        assert!(!newer("0.1", "0.1.1"));
    }
}
