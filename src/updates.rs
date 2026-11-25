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
    /// The last version announced as "available from the download page", so the daily
    /// check doesn't repeat it.
    announced: String,
}

/// What a check leads to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    UpToDate,
    /// A newer version that this build can verify: download, check, then offer to install.
    Install,
    /// A newer version, but this build has no update key to verify downloads with: send the
    /// user to the download page instead of running anything.
    DownloadPage,
}

/// Decides from a feed. Without a key only the version is read (nothing is downloaded or
/// run), so an unverifiable build still learns that an update exists.
pub fn decide(f: &Feed, current: &str, can_verify: bool) -> Decision {
    if !newer(&f.version, current) {
        Decision::UpToDate
    } else if can_verify && f.url.starts_with("https://") {
        Decision::Install
    } else {
        Decision::DownloadPage
    }
}

/// Where a build without an update key sends people for a new version.
pub fn download_page() -> String {
    match crate::build_info::WEBSITE {
        Some(w) => format!("{}/download", w.trim_end_matches('/')),
        None => "https://notchly.web.app/download".into(),
    }
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

/// Installed by Scoop (`...\scoop\apps\notchly\...`): Scoop owns updates, and the
/// self-updater would install a second copy with the setup program.
pub fn is_scoop_path(exe: &str) -> bool {
    exe.to_ascii_lowercase()
        .replace('/', "\\")
        .contains(r"\scoop\apps\")
}

fn is_scoop_install() -> bool {
    std::env::current_exe()
        .map(|p| is_scoop_path(&p.to_string_lossy()))
        .unwrap_or(false)
}

pub fn check(bus: Bus, force: bool) {
    if is_scoop_install() {
        if force {
            bus.send(Event::Notice(
                true,
                "Updates come from Scoop".into(),
                "Run: scoop update notchly".into(),
            ));
        }
        return;
    }
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
        let mut stamp = stamp;
        stamp.checked = unix_secs();
        save_json("cache/update.json", &stamp);
        let failed = |why: &str| {
            if force {
                bus.send(Event::Notice(
                    false,
                    "Couldn't check for updates".into(),
                    why.into(),
                ));
            }
        };
        // a build without the release public key can't verify downloads: it still reads the
        // version (harmless) and points to the download page, but never installs anything
        let key =
            crate::build_info::UPDATE_PUBKEY.and_then(crate::platform::credentials::b64_decode);
        let Some(body) = fetch(feed) else {
            return failed("Couldn't reach the update feed. Check your connection, or get the latest version from the website");
        };
        if let Some(key) = &key {
            // the manifest must be signed by the release key before anything in it is trusted
            let sig = fetch(&format!("{feed}.sig")).and_then(|s| {
                crate::platform::credentials::b64_decode(&String::from_utf8_lossy(&s))
            });
            if !sig.is_some_and(|s| crate::platform::credentials::verify_p256(key, &body, &s)) {
                crate::util::log("update: manifest signature missing or invalid, ignored");
                return failed("The update isn't signed by Notchly");
            }
        }
        let Ok(f) = serde_json::from_slice::<Feed>(&body) else {
            return failed("The update feed is unreadable");
        };
        match decide(&f, env!("CARGO_PKG_VERSION"), key.is_some()) {
            Decision::UpToDate => {
                if force {
                    bus.send(Event::Notice(
                        true,
                        "You're up to date".into(),
                        format!("Notchly {}", env!("CARGO_PKG_VERSION")),
                    ));
                }
                return;
            }
            Decision::DownloadPage => {
                // automatic checks announce each new version once; a manual check always does
                if force || stamp.announced != f.version {
                    stamp.announced = f.version.clone();
                    save_json("cache/update.json", &stamp);
                    bus.send(Event::UpdateManual(f.version.clone(), download_page()));
                }
                return;
            }
            Decision::Install => {}
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
    fn feed(version: &str, url: &str) -> Feed {
        Feed {
            version: version.into(),
            url: url.into(),
            sha256: "00".into(),
            notes: String::new(),
            min_os: String::new(),
        }
    }

    /// The reported bug: a build without the update key showed "Couldn't check for updates"
    /// and nothing else. It now learns about new versions and points to the download page,
    /// without ever downloading or running anything it can't verify.
    #[test]
    fn builds_without_a_key_point_to_the_download_page() {
        let f = feed(
            "0.3.1",
            "https://github.com/o/r/releases/download/v0.3.1/x.exe",
        );
        assert_eq!(decide(&f, "0.3.0", false), Decision::DownloadPage);
        assert_eq!(decide(&f, "0.3.0", true), Decision::Install);
        assert_eq!(decide(&f, "0.3.1", false), Decision::UpToDate);
        assert_eq!(decide(&f, "0.4.0", true), Decision::UpToDate);
        // never install from a non-HTTPS link, even with a valid signature
        let plain = feed("0.3.1", "http://example.com/x.exe");
        assert_eq!(decide(&plain, "0.3.0", true), Decision::DownloadPage);
        assert!(download_page().ends_with("/download"));
    }

    #[test]
    fn scoop_installs_are_recognised() {
        assert!(is_scoop_path(
            r"C:\Users\a\scoop\apps\notchly\current\notchly.exe"
        ));
        assert!(is_scoop_path(r"D:\Scoop\Apps\notchly\0.3.1\notchly.exe"));
        assert!(!is_scoop_path(
            r"C:\Users\a\AppData\Local\Programs\Notchly\notchly.exe"
        ));
    }
    #[test]
    fn versions() {
        assert!(newer("0.2.0", "0.1.9"));
        assert!(newer("1.0", "0.9.9"));
        assert!(!newer("0.1.0", "0.1.0"));
        assert!(!newer("0.1", "0.1.1"));
    }
}
