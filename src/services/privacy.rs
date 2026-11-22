//! Privacy dots: watches the CapabilityAccessManager ConsentStore for
//! microphone, webcam and location use (LastUsedTimeStop == 0 means in use).

use std::time::Duration;

use serde::{Deserialize, Serialize};
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, ERROR_SUCCESS, HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::Registry::*;
use windows::Win32::System::Threading::{CreateEventW, WaitForMultipleObjects, INFINITE};

use super::{spawn, Bus, Event};
use crate::util::{log, wide};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Cap {
    Mic,
    Camera,
    Location,
}

impl Cap {
    pub fn label(self) -> &'static str {
        match self {
            Cap::Mic => "Microphone",
            Cap::Camera => "Camera",
            Cap::Location => "Location",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Use {
    pub cap: Cap,
    pub app: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LogEntry {
    pub cap: Cap,
    pub app: String,
    pub start: i64,
    pub stop: Option<i64>,
}

const BASE: &str =
    r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore";
const CAPS: [(Cap, &str); 3] = [
    (Cap::Mic, "microphone"),
    (Cap::Camera, "webcam"),
    (Cap::Location, "location"),
];

fn open(path: &str) -> Option<HKEY> {
    let w = wide(path);
    let mut k = HKEY::default();
    let r = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(w.as_ptr()),
            Some(0),
            KEY_READ | KEY_NOTIFY,
            &mut k,
        )
    };
    (r == ERROR_SUCCESS).then_some(k)
}

fn subkeys(k: HKEY) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0u32;
    loop {
        let mut buf = [0u16; 512];
        let mut len = buf.len() as u32;
        let r = unsafe {
            RegEnumKeyExW(
                k,
                i,
                Some(PWSTR(buf.as_mut_ptr())),
                &mut len,
                None,
                None,
                None,
                None,
            )
        };
        if r != ERROR_SUCCESS {
            break;
        }
        out.push(String::from_utf16_lossy(&buf[..len as usize]));
        i += 1;
    }
    out
}

fn qword(k: HKEY, sub: &str, name: &str) -> Option<u64> {
    let s = wide(sub);
    let n = wide(name);
    let mut v = 0u64;
    let mut size = 8u32;
    let r = unsafe {
        RegGetValueW(
            k,
            PCWSTR(s.as_ptr()),
            PCWSTR(n.as_ptr()),
            RRF_RT_REG_QWORD,
            None,
            Some(&mut v as *mut u64 as _),
            Some(&mut size),
        )
    };
    (r == ERROR_SUCCESS).then_some(v)
}

fn friendly(key: &str, packaged: bool) -> String {
    if packaged {
        // "Microsoft.WindowsCamera_8wekyb3d8bbwe" -> "WindowsCamera"
        let base = key.split('_').next().unwrap_or(key);
        base.rsplit('.').next().unwrap_or(base).to_string()
    } else {
        // "C:#Program Files#Zoom#bin#Zoom.exe" -> "Zoom"
        let base = key.rsplit('#').next().unwrap_or(key);
        base.trim_end_matches(".exe")
            .trim_end_matches(".EXE")
            .to_string()
    }
}

/// Reads all capabilities currently in use.
pub fn scan() -> Vec<Use> {
    let mut out = Vec::new();
    for (cap, name) in CAPS {
        let Some(k) = open(&format!("{BASE}\\{name}")) else {
            continue;
        };
        for sub in subkeys(k) {
            if sub == "NonPackaged" {
                continue;
            }
            if in_use(k, &sub) {
                out.push(Use {
                    cap,
                    app: friendly(&sub, true),
                });
            }
        }
        if let Some(np) = open(&format!("{BASE}\\{name}\\NonPackaged")) {
            for sub in subkeys(np) {
                if in_use(np, &sub) {
                    out.push(Use {
                        cap,
                        app: friendly(&sub, false),
                    });
                }
            }
            unsafe {
                let _ = RegCloseKey(np);
            }
        }
        unsafe {
            let _ = RegCloseKey(k);
        }
    }
    out.sort_by(|a, b| (a.cap as u8, &a.app).cmp(&(b.cap as u8, &b.app)));
    out.dedup();
    out
}

fn in_use(k: HKEY, sub: &str) -> bool {
    let start = qword(k, sub, "LastUsedTimeStart").unwrap_or(0);
    let stop = qword(k, sub, "LastUsedTimeStop");
    start != 0 && stop == Some(0)
}

pub fn start(bus: Bus) {
    spawn("privacy", move || unsafe {
        let mut keys = Vec::new();
        let mut events: Vec<HANDLE> = Vec::new();
        for (_, name) in CAPS {
            if let (Some(k), Ok(ev)) = (
                open(&format!("{BASE}\\{name}")),
                CreateEventW(None, false, false, None),
            ) {
                keys.push(k);
                events.push(ev);
            }
        }
        if keys.is_empty() {
            log("privacy: consent store unavailable");
            return;
        }
        let mut last = scan();
        bus.send(Event::Privacy(last.clone()));
        loop {
            for (k, ev) in keys.iter().zip(&events) {
                let _ = RegNotifyChangeKeyValue(
                    *k,
                    true,
                    REG_NOTIFY_CHANGE_LAST_SET | REG_NOTIFY_CHANGE_NAME,
                    Some(*ev),
                    true,
                );
            }
            let r = WaitForMultipleObjects(&events, false, INFINITE);
            if r.0 >= WAIT_OBJECT_0.0 + events.len() as u32 {
                break;
            }
            std::thread::sleep(Duration::from_millis(150));
            let now = scan();
            if now != last {
                last = now.clone();
                bus.send(Event::Privacy(now));
            }
        }
        for ev in events {
            let _ = CloseHandle(ev);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names() {
        assert_eq!(
            friendly("C:#Program Files#Zoom#bin#Zoom.exe", false),
            "Zoom"
        );
        assert_eq!(
            friendly("Microsoft.WindowsCamera_8wekyb3d8bbwe", true),
            "WindowsCamera"
        );
    }
}
