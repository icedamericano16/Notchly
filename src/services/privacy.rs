//! Who is using the camera, microphone and location.
//!
//! * Microphone: the audio engine's capture sessions (any app recording from any input).
//! * Camera: Media Foundation's sensor activity monitor (the processes streaming a camera).
//! * Location (and the others on Windows builds without the live sources): the
//!   CapabilityAccessManager consent store (LastUsedTimeStop == 0 means in use). Newer
//!   Windows builds (25H2/26H2) stopped writing usage times there, which is why the live
//!   sources come first.
//!
//! A use found in a helper process (an app's embedded WebView2) is credited to its host app.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use windows::core::Interface;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{ERROR_SUCCESS, HANDLE};
use windows::Win32::System::Registry::*;
use windows::Win32::System::Threading::{CreateEventW, SetEvent, WaitForMultipleObjects};

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
    /// The consent-store key (desktop app path with `#`, or package family name).
    pub key: String,
    pub packaged: bool,
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

/// Reads the consent store's in-use records.
fn registry_scan() -> Vec<Use> {
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
                    key: sub.clone(),
                    packaged: true,
                });
            }
        }
        if let Some(np) = open(&format!("{BASE}\\{name}\\NonPackaged")) {
            for sub in subkeys(np) {
                if in_use(np, &sub) {
                    out.push(Use {
                        cap,
                        app: friendly(&sub, false),
                        key: sub.clone(),
                        packaged: false,
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

/// Windows' "Let apps access your camera" switch (and the desktop-apps one under it).
/// None when it can't be read.
pub fn camera_allowed() -> Option<bool> {
    let read = |path: &str| -> Option<bool> {
        let k = wide(&format!("{BASE}\\{path}"));
        let n = wide("Value");
        let mut buf = [0u16; 16];
        let mut size = (buf.len() * 2) as u32;
        let r = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                PCWSTR(k.as_ptr()),
                PCWSTR(n.as_ptr()),
                RRF_RT_REG_SZ,
                None,
                Some(buf.as_mut_ptr() as _),
                Some(&mut size),
            )
        };
        (r == ERROR_SUCCESS).then(|| {
            let s = String::from_utf16_lossy(&buf[..(size as usize / 2).saturating_sub(1)]);
            !s.eq_ignore_ascii_case("deny")
        })
    };
    // unset means allowed
    Some(read("webcam").unwrap_or(true) && read("webcam\\NonPackaged").unwrap_or(true))
}

/// Turns camera access for apps (Store and desktop) on or off, like the Windows privacy
/// switches. Returns false if Windows refused the change.
pub fn set_camera_allowed(on: bool) -> bool {
    let v = wide(if on { "Allow" } else { "Deny" });
    let bytes = unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * 2) };
    let mut ok = true;
    for path in ["webcam", "webcam\\NonPackaged"] {
        let k = wide(&format!("{BASE}\\{path}"));
        let mut h = HKEY::default();
        let r = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(k.as_ptr()),
                None,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                None,
                &mut h,
                None,
            )
        };
        if r != ERROR_SUCCESS {
            ok = false;
            continue;
        }
        let n = wide("Value");
        let r = unsafe { RegSetValueExW(h, PCWSTR(n.as_ptr()), None, REG_SZ, Some(bytes)) };
        ok &= r == ERROR_SUCCESS;
        unsafe {
            let _ = RegCloseKey(h);
        }
    }
    ok
}

fn in_use(k: HKEY, sub: &str) -> bool {
    let start = qword(k, sub, "LastUsedTimeStart").unwrap_or(0);
    let stop = qword(k, sub, "LastUsedTimeStop");
    start != 0 && stop == Some(0)
}

// ---------------------------------------------------------------- live sources

/// Processes with an active capture (recording) session on any input. None if the audio
/// engine can't be asked.
fn mic_pids() -> Option<Vec<u32>> {
    use windows::Win32::Media::Audio::*;
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
    unsafe {
        let en: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
        let devs = en.EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE).ok()?;
        let mut out = Vec::new();
        for i in 0..devs.GetCount().unwrap_or(0) {
            let Ok(dev) = devs.Item(i) else { continue };
            let Ok(mgr) = dev.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else {
                continue;
            };
            let Ok(sessions) = mgr.GetSessionEnumerator() else {
                continue;
            };
            for j in 0..sessions.GetCount().unwrap_or(0) {
                let Ok(c) = sessions.GetSession(j) else {
                    continue;
                };
                if c.GetState().ok() != Some(AudioSessionStateActive) {
                    continue;
                }
                let Ok(c2) = c.cast::<IAudioSessionControl2>() else {
                    continue;
                };
                // S_OK = the system sounds session
                if c2.IsSystemSoundsSession().0 == 0 {
                    continue;
                }
                if let Ok(pid) = c2.GetProcessId() {
                    if pid != 0 {
                        out.push(pid);
                    }
                }
            }
        }
        Some(out)
    }
}

/// Processes streaming a camera, kept current by Media Foundation's sensor activity monitor.
struct CameraWatch {
    pids: Arc<Mutex<Vec<u32>>>,
    _monitor: windows::Win32::Media::MediaFoundation::IMFSensorActivityMonitor,
}

#[windows::core::implement(
    windows::Win32::Media::MediaFoundation::IMFSensorActivitiesReportCallback
)]
struct CamCb {
    pids: Arc<Mutex<Vec<u32>>>,
    wake: isize,
}

impl windows::Win32::Media::MediaFoundation::IMFSensorActivitiesReportCallback_Impl for CamCb_Impl {
    fn OnActivitiesReport(
        &self,
        report: windows::core::Ref<
            windows::Win32::Media::MediaFoundation::IMFSensorActivitiesReport,
        >,
    ) -> windows::core::Result<()> {
        let mut v = Vec::new();
        if let Ok(r) = report.ok() {
            unsafe {
                for i in 0..r.GetCount().unwrap_or(0) {
                    let Ok(a) = r.GetActivityReport(i) else {
                        continue;
                    };
                    for j in 0..a.GetProcessCount().unwrap_or(0) {
                        let Ok(p) = a.GetProcessActivity(j) else {
                            continue;
                        };
                        if p.GetStreamingState().is_ok_and(|b| b.as_bool()) {
                            if let Ok(pid) = p.GetProcessId() {
                                v.push(pid);
                            }
                        }
                    }
                }
            }
        }
        if let Ok(mut g) = self.pids.lock() {
            *g = v;
        }
        unsafe {
            let _ = SetEvent(HANDLE(self.wake as _));
        }
        Ok(())
    }
}

fn camera_watch(wake: HANDLE) -> Option<CameraWatch> {
    use windows::Win32::Media::MediaFoundation::*;
    unsafe {
        MFStartup(MF_VERSION, MFSTARTUP_LITE).ok()?;
        let pids = Arc::new(Mutex::new(Vec::new()));
        let cb: IMFSensorActivitiesReportCallback = CamCb {
            pids: pids.clone(),
            wake: wake.0 as isize,
        }
        .into();
        let monitor = MFCreateSensorActivityMonitor(&cb).ok()?;
        monitor.Start().ok()?;
        Some(CameraWatch {
            pids,
            _monitor: monitor,
        })
    }
}

/// Turns process ids into uses, credited to the app they work for.
fn uses_for(
    cap: Cap,
    pids: &[u32],
    table: &std::collections::HashMap<u32, (u32, String)>,
) -> Vec<Use> {
    let me = std::process::id();
    let mut out: Vec<Use> = Vec::new();
    for pid in pids {
        let host = crate::platform::apps::host(*pid, table);
        if host == me {
            continue;
        }
        let Some(path) = crate::platform::apps::image(host) else {
            continue;
        };
        let key = path.replace('\\', "#");
        if out.iter().any(|u| u.key.eq_ignore_ascii_case(&key)) {
            continue;
        }
        out.push(Use {
            cap,
            app: crate::platform::apps::app_name(&path),
            key,
            packaged: false,
        });
    }
    out
}

/// Everything in use now: live sources where they work, the consent store otherwise.
fn scan_all(camera: Option<&CameraWatch>) -> Vec<Use> {
    let reg = registry_scan();
    let mic = mic_pids();
    let cam = camera.and_then(|c| c.pids.lock().ok().map(|g| g.clone()));
    let table = if mic.as_ref().is_some_and(|v| !v.is_empty())
        || cam.as_ref().is_some_and(|v| !v.is_empty())
    {
        crate::platform::apps::process_table()
    } else {
        Default::default()
    };
    let mut out = Vec::new();
    match &cam {
        Some(p) => out.extend(uses_for(Cap::Camera, p, &table)),
        None => out.extend(reg.iter().filter(|u| u.cap == Cap::Camera).cloned()),
    }
    match &mic {
        Some(p) => out.extend(uses_for(Cap::Mic, p, &table)),
        None => out.extend(reg.iter().filter(|u| u.cap == Cap::Mic).cloned()),
    }
    out.extend(reg.into_iter().filter(|u| u.cap == Cap::Location));
    out.sort_by(|a, b| (a.cap as u8, &a.app).cmp(&(b.cap as u8, &b.app)));
    out.dedup_by(|a, b| a.cap == b.cap && a.app.eq_ignore_ascii_case(&b.app));
    out
}

/// Which live sources work on this PC: (microphone sessions, camera activity monitor).
pub fn sources() -> (bool, bool) {
    super::com_mta();
    let wake = unsafe { CreateEventW(None, false, false, None) }.unwrap_or_default();
    (mic_pids().is_some(), camera_watch(wake).is_some())
}

/// Reads everything in use once (for diagnostics).
pub fn scan() -> Vec<Use> {
    super::com_mta();
    let wake = unsafe { CreateEventW(None, false, false, None) }.unwrap_or_default();
    let cam = camera_watch(wake);
    // give the monitor a moment for its first report
    std::thread::sleep(Duration::from_millis(400));
    scan_all(cam.as_ref())
}

/// Microphone sessions are polled this often (s); the camera and the consent store notify.
const POLL: Duration = Duration::from_millis(1500);

pub fn start(bus: Bus) {
    spawn("privacy", move || unsafe {
        super::com_mta();
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
        // the camera monitor wakes the loop the moment streaming starts or stops
        let cam_wake = CreateEventW(None, false, false, None).unwrap_or_default();
        let cam = camera_watch(cam_wake);
        if cam.is_some() {
            events.push(cam_wake);
        } else {
            log("privacy: camera activity monitor unavailable; using the consent store");
        }
        let mut last = scan_all(cam.as_ref());
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
            if events.is_empty() {
                std::thread::sleep(POLL);
            } else {
                let _ = WaitForMultipleObjects(&events, false, POLL.as_millis() as u32);
                // let a burst of changes settle
                std::thread::sleep(Duration::from_millis(120));
            }
            let now = scan_all(cam.as_ref());
            if now != last {
                last = now.clone();
                bus.send(Event::Privacy(now));
            }
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
