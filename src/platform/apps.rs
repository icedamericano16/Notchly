//! The app behind a camera / microphone / location use, and closing it.
//!
//! Windows' consent store names each app by a key: a desktop app's path with `#` for `\`
//! (`C:#Program Files#Zoom#bin#Zoom.exe`), or a packaged app's family name
//! (`Microsoft.WindowsCamera_8wekyb3d8bbwe`). Closing first asks politely (WM_CLOSE to the
//! app's windows, so it can save); *Force quit* ends the processes. System processes and
//! Notchly itself are never touched.

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, WPARAM};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW, TerminateProcess,
    PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindow, GetWindowThreadProcessId, IsWindowVisible, PostMessageW, GW_OWNER,
    WM_CLOSE,
};

/// Whether a process image path belongs to a consent-store key.
pub fn path_matches(path: &str, key: &str, packaged: bool) -> bool {
    let p = path.to_lowercase();
    if packaged {
        // installed under ...\WindowsApps\<Name>_<version>_<arch>__<PublisherId>\
        let Some((name, publisher)) = key.rsplit_once('_') else {
            return false;
        };
        p.contains("\\windowsapps\\")
            && p.contains(&format!("\\{}_", name.to_lowercase()))
            && p.contains(&format!("__{}\\", publisher.to_lowercase()))
    } else {
        p == key.replace('#', "\\").to_lowercase()
    }
}

/// Apps Notchly will offer to close (not Windows' own processes).
pub fn closable(key: &str, packaged: bool) -> bool {
    if packaged {
        return !key.to_lowercase().starts_with("microsoft.windows.");
    }
    let p = key.replace('#', "\\").to_lowercase();
    let windir = std::env::var("WINDIR")
        .unwrap_or_else(|_| r"C:\Windows".into())
        .to_lowercase();
    !p.starts_with(&windir)
}

/// A process's full image path.
pub fn image(pid: u32) -> Option<String> {
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok =
            QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len)
                .is_ok();
        let _ = CloseHandle(h);
        ok.then(|| String::from_utf16_lossy(&buf[..len as usize]))
    }
}

/// Running processes that belong to the key.
pub fn processes(key: &str, packaged: bool) -> Vec<u32> {
    let me = unsafe { GetCurrentProcessId() };
    let mut out = Vec::new();
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return out;
        };
        let mut e = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut more = Process32FirstW(snap, &mut e).is_ok();
        while more {
            let pid = e.th32ProcessID;
            if pid != me && pid > 4 {
                if let Some(p) = image(pid) {
                    if path_matches(&p, key, packaged) {
                        out.push(pid);
                    }
                }
            }
            more = Process32NextW(snap, &mut e).is_ok();
        }
        let _ = CloseHandle(snap);
    }
    out
}

struct Ask {
    pids: Vec<u32>,
    asked: usize,
}

unsafe extern "system" fn ask_window(h: HWND, lp: LPARAM) -> windows::core::BOOL {
    // SAFETY: `lp` is the address of the `Ask` passed to EnumWindows below, alive for the whole call,
    // which runs this callback synchronously on this thread.
    let a = unsafe { &mut *(lp.0 as *mut Ask) };
    let mut pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(h, Some(&mut pid));
        // top-level, visible, unowned windows: the app's main windows
        if a.pids.contains(&pid)
            && IsWindowVisible(h).as_bool()
            && GetWindow(h, GW_OWNER).map_or(true, |o| o.0.is_null())
            && PostMessageW(Some(h), WM_CLOSE, WPARAM(0), LPARAM(0)).is_ok()
        {
            a.asked += 1;
        }
    }
    true.into()
}

/// Asks the app to close its windows (it may save or ask the user first). Returns how many
/// windows were asked; 0 means it has no window to close (Force quit is then the only way).
pub fn close(key: &str, packaged: bool) -> usize {
    if !closable(key, packaged) {
        return 0;
    }
    let mut a = Ask {
        pids: processes(key, packaged),
        asked: 0,
    };
    if a.pids.is_empty() {
        return 0;
    }
    unsafe {
        let _ = EnumWindows(Some(ask_window), LPARAM(&mut a as *mut Ask as isize));
    }
    a.asked
}

/// Ends the app's processes at once. Returns how many were ended.
pub fn force_quit(key: &str, packaged: bool) -> usize {
    if !closable(key, packaged) {
        return 0;
    }
    let mut n = 0;
    for pid in processes(key, packaged) {
        unsafe {
            if let Ok(h) = OpenProcess(PROCESS_TERMINATE, false, pid) {
                if TerminateProcess(h, 1).is_ok() {
                    n += 1;
                }
                let _ = CloseHandle(h);
            }
        }
    }
    n
}

/// Helper processes that do work on behalf of the app that started them (an app built on
/// Edge WebView2 — WhatsApp, for one — captures the camera and microphone in its WebView2
/// child). Uses are credited to the host.
const HELPERS: [&str; 2] = ["msedgewebview2.exe", "webviewhost.exe"];

/// pid -> (parent pid, exe file name, lowercase).
pub fn process_table() -> std::collections::HashMap<u32, (u32, String)> {
    let mut out = std::collections::HashMap::new();
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return out;
        };
        let mut e = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut more = Process32FirstW(snap, &mut e).is_ok();
        while more {
            let n = e
                .szExeFile
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(e.szExeFile.len());
            let name = String::from_utf16_lossy(&e.szExeFile[..n]).to_lowercase();
            out.insert(e.th32ProcessID, (e.th32ParentProcessID, name));
            more = Process32NextW(snap, &mut e).is_ok();
        }
        let _ = CloseHandle(snap);
    }
    out
}

/// The app a process works for: itself, or the nearest ancestor that isn't a helper.
pub fn host(pid: u32, table: &std::collections::HashMap<u32, (u32, String)>) -> u32 {
    let mut cur = pid;
    for _ in 0..8 {
        match table.get(&cur) {
            Some((parent, name))
                if HELPERS.contains(&name.as_str()) && table.contains_key(parent) =>
            {
                cur = *parent;
            }
            _ => break,
        }
    }
    cur
}

/// A friendly app name from the exe's version info ("WhatsApp", "Google Chrome"), else the
/// file name without ".exe". Cached per path.
pub fn app_name(path: &str) -> String {
    use std::sync::Mutex;
    static CACHE: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());
    if let Ok(c) = CACHE.lock() {
        if let Some((_, n)) = c.iter().find(|(p, _)| p == path) {
            return n.clone();
        }
    }
    let stem = path
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(path)
        .trim_end_matches(".exe")
        .trim_end_matches(".EXE")
        .to_string();
    let name = version_string(path, "FileDescription")
        .or_else(|| version_string(path, "ProductName"))
        .filter(|n| !n.trim().is_empty() && n.len() <= 48)
        .unwrap_or(stem);
    if let Ok(mut c) = CACHE.lock() {
        if c.len() > 64 {
            c.clear();
        }
        c.push((path.to_string(), name.clone()));
    }
    name
}

fn version_string(path: &str, field: &str) -> Option<String> {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
    };
    let wpath = crate::util::wide(path);
    unsafe {
        let size = GetFileVersionInfoSizeW(PCWSTR(wpath.as_ptr()), None);
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        GetFileVersionInfoW(PCWSTR(wpath.as_ptr()), None, size, buf.as_mut_ptr() as _).ok()?;
        // the first language / code page the file declares
        let mut p: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut len = 0u32;
        let tr = crate::util::wide("\\VarFileInfo\\Translation");
        let (lang, cp) = if VerQueryValueW(buf.as_ptr() as _, PCWSTR(tr.as_ptr()), &mut p, &mut len)
            .as_bool()
            && len >= 4
        {
            // SAFETY: VerQueryValueW succeeded with len >= 4, so `p` points to at least two
            // u16 (language, code page) inside `buf`, which outlives this read.
            let w = std::slice::from_raw_parts(p as *const u16, 2);
            (w[0], w[1])
        } else {
            (0x0409, 0x04B0)
        };
        let q = crate::util::wide(&format!("\\StringFileInfo\\{lang:04x}{cp:04x}\\{field}"));
        if !VerQueryValueW(buf.as_ptr() as _, PCWSTR(q.as_ptr()), &mut p, &mut len).as_bool()
            || len == 0
        {
            return None;
        }
        // SAFETY: VerQueryValueW succeeded; for a string value `p` points to `len` u16
        // characters inside `buf`, which outlives this read.
        let w = std::slice::from_raw_parts(p as *const u16, len as usize);
        let n = w.iter().position(|c| *c == 0).unwrap_or(w.len());
        Some(String::from_utf16_lossy(&w[..n]).trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_match_their_processes() {
        assert!(path_matches(
            r"C:\Program Files\Zoom\bin\Zoom.exe",
            "C:#Program Files#Zoom#bin#Zoom.exe",
            false
        ));
        assert!(!path_matches(
            r"C:\Program Files\Zoom\bin\ZoomOther.exe",
            "C:#Program Files#Zoom#bin#Zoom.exe",
            false
        ));
        assert!(path_matches(
            r"C:\Program Files\WindowsApps\Microsoft.WindowsCamera_2024.2405.5.0_x64__8wekyb3d8bbwe\WindowsCamera.exe",
            "Microsoft.WindowsCamera_8wekyb3d8bbwe",
            true
        ));
        assert!(!path_matches(
            r"C:\Program Files\WindowsApps\Microsoft.WindowsCameraPlus_1.0_x64__8wekyb3d8bbwe\x.exe",
            "Microsoft.WindowsCamera_8wekyb3d8bbwe",
            true
        ));
    }

    #[test]
    fn helpers_credit_their_host() {
        let mut t = std::collections::HashMap::new();
        t.insert(10, (1, "whatsapp.root.exe".to_string()));
        t.insert(20, (10, "msedgewebview2.exe".to_string()));
        t.insert(21, (20, "msedgewebview2.exe".to_string()));
        t.insert(30, (1, "discord.exe".to_string()));
        assert_eq!(host(21, &t), 10);
        assert_eq!(host(20, &t), 10);
        assert_eq!(host(30, &t), 30);
        // an orphaned helper stays itself
        t.insert(40, (999, "msedgewebview2.exe".to_string()));
        assert_eq!(host(40, &t), 40);
    }

    #[test]
    fn windows_itself_is_never_closed() {
        assert!(!closable("C:#Windows#System32#svchost.exe", false));
        assert!(closable("C:#Program Files#Zoom#bin#Zoom.exe", false));
        assert!(!closable(
            "Microsoft.Windows.ShellExperienceHost_cw5n1h2txyewy",
            true
        ));
        assert!(closable("Microsoft.WindowsCamera_8wekyb3d8bbwe", true));
    }
}
