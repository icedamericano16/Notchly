//! Update checker for the direct build: a JSON manifest (normally the latest GitHub Release's
//! `update.json`) checked at most daily. The manifest must carry a valid ECDSA P-256
//! signature (`update.json.sig`) from the key baked in at build time; then the installer is
//! downloaded in the background, its SHA-256 checked against the signed manifest, and it runs
//! only when the user confirms.

use serde::{Deserialize, Serialize};

use crate::services::{spawn, Bus, Event};
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
        let Some(key) =
            crate::build_info::UPDATE_PUBKEY.and_then(crate::platform::credentials::b64_decode)
        else {
            return failed("This build can't verify updates; download them from the website");
        };
        let Some(body) = fetch(feed) else {
            return failed("No connection");
        };
        // the manifest must be signed by the release key before anything in it is trusted
        let sig = fetch(&format!("{feed}.sig"))
            .and_then(|s| crate::platform::credentials::b64_decode(&String::from_utf8_lossy(&s)));
        if !sig.is_some_and(|s| crate::platform::credentials::verify_p256(&key, &body, &s)) {
            crate::util::log("update: manifest signature missing or invalid, ignored");
            return failed("The update isn't signed by Notchly");
        }
        let Ok(f) = serde_json::from_slice::<Feed>(&body) else {
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

/// The exact bytes at `url` (a signature covers bytes, not decoded text). None unless 2xx.
fn fetch(url: &str) -> Option<Vec<u8>> {
    use windows::Storage::Streams::DataReader;
    use windows::Web::Http::HttpClient;
    let c = HttpClient::new().ok()?;
    let uri = windows::Foundation::Uri::CreateUri(&windows::core::HSTRING::from(url)).ok()?;
    let resp = c.GetAsync(&uri).ok()?.join().ok()?;
    if !resp.IsSuccessStatusCode().ok()? {
        return None;
    }
    let buf = resp.Content().ok()?.ReadAsBufferAsync().ok()?.join().ok()?;
    let reader = DataReader::FromBuffer(&buf).ok()?;
    let mut bytes = vec![0u8; buf.Length().ok()? as usize];
    reader.ReadBytes(&mut bytes).ok()?;
    Some(bytes)
}

fn download(f: &Feed) -> Option<String> {
    let bytes = fetch(&f.url)?;
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
