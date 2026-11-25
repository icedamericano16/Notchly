//! Platform glue: monitors, fullscreen detection, hotkeys, autostart, shell,
//! sounds, clipboard, drag & drop, screen capture and the tray.

pub mod apps;
pub mod capture;
pub mod clipboard;
pub mod credentials;
pub mod dragdrop;
pub mod tray;
pub mod uia;

use windows::core::{BOOL, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::Registry::*;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Shell::{
    SHQueryUserNotificationState, ShellExecuteW, QUNS_BUSY, QUNS_PRESENTATION_MODE,
    QUNS_RUNNING_D3D_FULL_SCREEN,
};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::util::{from_wide, wide};

// ---------------------------------------------------------------- monitors

#[derive(Clone, Debug, PartialEq)]
pub struct Monitor {
    pub handle: isize,
    pub rect: RECT,
    pub work: RECT,
    pub dpi: u32,
    pub key: String,
    pub primary: bool,
}

pub fn monitors() -> Vec<Monitor> {
    unsafe extern "system" fn cb(h: HMONITOR, _: HDC, _: *mut RECT, lp: LPARAM) -> BOOL {
        // SAFETY: `lp` is the address of the `Vec` passed to EnumDisplayMonitors below, alive for the whole call,
        // which runs this callback synchronously on this thread.
        let v = unsafe { &mut *(lp.0 as *mut Vec<Monitor>) };
        let mut mi = MONITORINFOEXW::default();
        mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        // SAFETY: MONITORINFOEXW starts with a MONITORINFO, and cbSize says the extended struct
        // is being passed, which is how GetMonitorInfoW is meant to be called for it.
        if unsafe { GetMonitorInfoW(h, &mut mi as *mut _ as *mut MONITORINFO) }.as_bool() {
            let (mut dx, mut dy) = (96u32, 96u32);
            let _ = unsafe { GetDpiForMonitor(h, MDT_EFFECTIVE_DPI, &mut dx, &mut dy) };
            v.push(Monitor {
                handle: h.0 as isize,
                rect: mi.monitorInfo.rcMonitor,
                work: mi.monitorInfo.rcWork,
                dpi: dx,
                key: from_wide(&mi.szDevice),
                primary: mi.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
            });
        }
        true.into()
    }
    let mut v: Vec<Monitor> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(cb), LPARAM(&mut v as *mut _ as isize));
    }
    v
}

/// The monitor chosen in settings (1-based index), falling back to primary.
pub fn chosen_monitor(index: u32) -> Option<Monitor> {
    let all = monitors();
    if index == crate::settings::MONITOR_FOLLOW {
        if let Some(m) = monitor_at_cursor(&all) {
            return Some(m);
        }
    }
    if index > 0 {
        if let Some(m) = all.get(index as usize - 1) {
            return Some(m.clone());
        }
    }
    all.iter()
        .find(|m| m.primary)
        .cloned()
        .or_else(|| all.first().cloned())
}

/// The handle of the display under the mouse pointer (cheap: no enumeration).
pub fn monitor_handle_at_cursor() -> Option<isize> {
    let mut p = windows::Win32::Foundation::POINT::default();
    // SAFETY: GetCursorPos writes the POINT it is given; MonitorFromPoint only reads it.
    unsafe {
        GetCursorPos(&mut p).ok()?;
        let h = MonitorFromPoint(p, MONITOR_DEFAULTTONULL);
        (!h.is_invalid()).then_some(h.0 as isize)
    }
}

/// The display under the mouse pointer.
pub fn monitor_at_cursor(all: &[Monitor]) -> Option<Monitor> {
    let mut p = windows::Win32::Foundation::POINT::default();
    // SAFETY: GetCursorPos writes the POINT it is given; `p` lives across the call.
    unsafe { GetCursorPos(&mut p) }.ok()?;
    all.iter()
        .find(|m| {
            p.x >= m.rect.left && p.x < m.rect.right && p.y >= m.rect.top && p.y < m.rect.bottom
        })
        .cloned()
}

// ---------------------------------------------------------------- fullscreen

pub fn window_class(h: HWND) -> String {
    let mut buf = [0u16; 128];
    let n = unsafe { GetClassNameW(h, &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

pub fn window_exe(h: HWND) -> String {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(h, Some(&mut pid)) };
    crate::services::audio::process_name(pid)
}

pub fn window_title(h: HWND) -> String {
    let mut buf = [0u16; 256];
    let n = unsafe { GetWindowTextW(h, &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

/// Titles of every visible top-level window whose process `keep` accepts (by exe name),
/// in or out of focus. Used to find a call running in any browser window.
pub fn window_titles(keep: fn(&str) -> bool) -> Vec<String> {
    struct Acc {
        keep: fn(&str) -> bool,
        out: Vec<String>,
    }
    unsafe extern "system" fn cb(h: HWND, lp: LPARAM) -> windows::core::BOOL {
        // SAFETY: `lp` is the address of the `Acc` below, alive for the whole EnumWindows call,
        // which runs this callback synchronously on this thread.
        let acc = unsafe { &mut *(lp.0 as *mut Acc) };
        if unsafe { IsWindowVisible(h) }.as_bool() {
            let title = window_title(h);
            if !title.is_empty() && (acc.keep)(&window_exe(h)) {
                acc.out.push(title);
            }
        }
        true.into()
    }
    let mut acc = Acc {
        keep,
        out: Vec::new(),
    };
    unsafe {
        let _ = EnumWindows(Some(cb), LPARAM(&mut acc as *mut Acc as isize));
    }
    acc.out
}

/// Whether the foreground window covers `monitor` entirely. Returns its exe.
pub fn foreground_fullscreen(monitor: isize, own: &[HWND]) -> (bool, String) {
    unsafe {
        let fg = GetForegroundWindow();
        if fg.0.is_null() || own.contains(&fg) {
            return (false, String::new());
        }
        let class = window_class(fg);
        if matches!(
            class.as_str(),
            "Progman"
                | "WorkerW"
                | "Shell_TrayWnd"
                | "Shell_SecondaryTrayWnd"
                | "Windows.UI.Core.CoreWindow"
                | "XamlExplorerHostIslandWindow"
                | "TopLevelWindowForOverflowXamlIsland"
        ) {
            return (false, String::new());
        }
        let hm = MonitorFromWindow(fg, MONITOR_DEFAULTTONEAREST);
        if hm.0 as isize != monitor {
            return (false, String::new());
        }
        let mut r = RECT::default();
        if GetWindowRect(fg, &mut r).is_err() {
            return (false, String::new());
        }
        let mut mi = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(hm, &mut mi).as_bool() {
            return (false, String::new());
        }
        let m = mi.rcMonitor;
        let covers =
            r.left <= m.left && r.top <= m.top && r.right >= m.right && r.bottom >= m.bottom;
        (
            covers,
            if covers {
                window_exe(fg)
            } else {
                String::new()
            },
        )
    }
}

/// Presentation / D3D fullscreen / busy state from the shell.
pub fn shell_busy() -> bool {
    unsafe {
        SHQueryUserNotificationState()
            .map(|s| {
                s == QUNS_BUSY || s == QUNS_RUNNING_D3D_FULL_SCREEN || s == QUNS_PRESENTATION_MODE
            })
            .unwrap_or(false)
    }
}

// ---------------------------------------------------------------- hotkeys

/// Parses "Win+Alt+N" into (modifiers, virtual key).
pub fn parse_hotkey(s: &str) -> Option<(HOT_KEY_MODIFIERS, u32)> {
    let mut mods = HOT_KEY_MODIFIERS(0);
    let mut vk = None;
    for part in s.split('+').map(|p| p.trim().to_lowercase()) {
        match part.as_str() {
            "win" | "windows" => mods |= MOD_WIN,
            "ctrl" | "control" => mods |= MOD_CONTROL,
            "alt" => mods |= MOD_ALT,
            "shift" => mods |= MOD_SHIFT,
            "space" => vk = Some(VK_SPACE.0 as u32),
            "enter" | "return" => vk = Some(VK_RETURN.0 as u32),
            "tab" => vk = Some(VK_TAB.0 as u32),
            "esc" | "escape" => vk = Some(VK_ESCAPE.0 as u32),
            p if p.len() == 1 => {
                let c = p.chars().next()?.to_ascii_uppercase();
                if c.is_ascii_alphanumeric() {
                    vk = Some(c as u32);
                } else {
                    return None;
                }
            }
            p if p.starts_with('f') && p.len() <= 3 => {
                let n: u32 = p[1..].parse().ok()?;
                if (1..=24).contains(&n) {
                    vk = Some(VK_F1.0 as u32 + n - 1);
                }
            }
            _ => return None,
        }
    }
    let vk = vk?;
    if mods.0 == 0 && !(VK_F1.0 as u32..=VK_F24.0 as u32).contains(&vk) {
        return None;
    }
    Some((mods, vk))
}

/// Formats a hotkey from a key event for the recorder.
pub fn format_hotkey(vk: u32) -> Option<String> {
    let down = |k: VIRTUAL_KEY| unsafe { GetKeyState(k.0 as i32) } < 0;
    let mut parts = Vec::new();
    if down(VK_LWIN) || down(VK_RWIN) {
        parts.push("Win");
    }
    if down(VK_CONTROL) {
        parts.push("Ctrl");
    }
    if down(VK_MENU) {
        parts.push("Alt");
    }
    if down(VK_SHIFT) {
        parts.push("Shift");
    }
    let key = match vk {
        0x20 => "Space".to_string(),
        0x30..=0x39 | 0x41..=0x5A => (vk as u8 as char).to_string(),
        v if (VK_F1.0 as u32..=VK_F24.0 as u32).contains(&v) => {
            format!("F{}", v - VK_F1.0 as u32 + 1)
        }
        _ => return None,
    };
    if parts.is_empty() && !key.starts_with('F') {
        return None;
    }
    Some(format!(
        "{}{}{}",
        parts.join("+"),
        if parts.is_empty() { "" } else { "+" },
        key
    ))
}

/// Registers hotkeys; returns the names that failed (conflicts).
pub fn register_hotkeys(hwnd: HWND, keys: &[(i32, &str, &str)]) -> Vec<String> {
    let mut failed = Vec::new();
    for (id, name, combo) in keys {
        unsafe {
            let _ = UnregisterHotKey(Some(hwnd), *id);
        }
        if combo.is_empty() {
            continue;
        }
        match parse_hotkey(combo) {
            Some((m, vk)) => {
                if unsafe { RegisterHotKey(Some(hwnd), *id, m | MOD_NOREPEAT, vk) }.is_err() {
                    failed.push(format!("{name} ({combo})"));
                }
            }
            None => failed.push(format!("{name} ({combo}: invalid)")),
        }
    }
    failed
}

// ---------------------------------------------------------------- autostart

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

pub fn set_autostart(on: bool) {
    unsafe {
        let mut k = HKEY::default();
        let path = wide(RUN_KEY);
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(path.as_ptr()),
            Some(0),
            KEY_SET_VALUE,
            &mut k,
        )
        .is_err()
        {
            return;
        }
        let name = wide("Notchly");
        if on {
            if let Ok(exe) = std::env::current_exe() {
                let v = format!("\"{}\" --autostart", exe.display());
                let w = wide(&v);
                let bytes = crate::util::u16_bytes(&w);
                let _ = RegSetValueExW(k, PCWSTR(name.as_ptr()), Some(0), REG_SZ, Some(&bytes));
            }
        } else {
            let _ = RegDeleteValueW(k, PCWSTR(name.as_ptr()));
        }
        let _ = RegCloseKey(k);
    }
}

// ---------------------------------------------------------------- shell

pub fn shell_open(target: &str) {
    let t = wide(target);
    let verb = wide("open");
    unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(t.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}

pub fn reveal(path: &str) {
    let args = wide(&format!("/select,\"{path}\""));
    let exe = wide("explorer.exe");
    let verb = wide("open");
    unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(exe.as_ptr()),
            PCWSTR(args.as_ptr()),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}

/// Runs a user-defined command line (palette custom commands only, never Live API payloads).
pub fn run_command(cmd: &str) {
    let cmd = cmd.trim();
    if cmd.starts_with("http://") || cmd.starts_with("https://") || cmd.starts_with("ms-settings:")
    {
        shell_open(cmd);
        return;
    }
    let (exe, args) = if let Some(rest) = cmd.strip_prefix('"') {
        match rest.split_once('"') {
            Some((e, a)) => (e.to_string(), a.trim().to_string()),
            None => (rest.to_string(), String::new()),
        }
    } else {
        match cmd.split_once(' ') {
            Some((e, a)) => (e.to_string(), a.to_string()),
            None => (cmd.to_string(), String::new()),
        }
    };
    let (e, a, verb) = (wide(&exe), wide(&args), wide("open"));
    unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(e.as_ptr()),
            PCWSTR(a.as_ptr()),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}

// ---------------------------------------------------------------- input synthesis

pub fn send_keys(vks: &[u16]) {
    let mut inputs = Vec::new();
    for &vk in vks {
        inputs.push(key_input(vk, false));
    }
    for &vk in vks.iter().rev() {
        inputs.push(key_input(vk, true));
    }
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

fn key_input(vk: u16, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: if up {
                    KEYEVENTF_KEYUP
                } else {
                    KEYBD_EVENT_FLAGS(0)
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// The current desktop wallpaper's file (None for a solid colour or a slideshow without one).
pub fn wallpaper_path() -> Option<std::path::PathBuf> {
    let mut buf = [0u16; 520];
    unsafe {
        SystemParametersInfoW(
            SPI_GETDESKWALLPAPER,
            buf.len() as u32,
            Some(buf.as_mut_ptr() as _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .ok()?;
    }
    let p = from_wide(&buf);
    (!p.is_empty()).then(|| std::path::PathBuf::from(p))
}

/// Windows "show animations" setting.
pub fn os_animations() -> bool {
    let mut on = BOOL(1);
    unsafe {
        let _ = SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some(&mut on as *mut _ as _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
    }
    on.as_bool()
}

pub fn high_contrast() -> bool {
    use windows::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
    let mut hc = HIGHCONTRASTW {
        cbSize: std::mem::size_of::<HIGHCONTRASTW>() as u32,
        ..Default::default()
    };
    unsafe {
        let _ = SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            hc.cbSize,
            Some(&mut hc as *mut _ as _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
    }
    hc.dwFlags & HCF_HIGHCONTRASTON == HCF_HIGHCONTRASTON
}

pub fn transparency_enabled() -> bool {
    unsafe {
        let mut v = 1u32;
        let mut size = 4u32;
        let k = wide(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
        let n = wide("EnableTransparency");
        let r = RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(k.as_ptr()),
            PCWSTR(n.as_ptr()),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut v as *mut u32 as _),
            Some(&mut size),
        );
        r.is_err() || v != 0
    }
}

/// Windows is in light mode (Settings › Personalisation › Colours › "Choose your mode").
pub fn system_light() -> bool {
    unsafe {
        let mut v = 0u32;
        let mut size = 4u32;
        let k = wide(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
        let n = wide("SystemUsesLightTheme");
        let r = RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(k.as_ptr()),
            PCWSTR(n.as_ptr()),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut v as *mut u32 as _),
            Some(&mut size),
        );
        r.is_ok() && v != 0
    }
}

pub fn is_windows_11() -> bool {
    windows_build() >= 22000
}

pub fn windows_build() -> u32 {
    unsafe {
        let mut buf = [0u16; 32];
        let mut size = (buf.len() * 2) as u32;
        let k = wide(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion");
        let n = wide("CurrentBuildNumber");
        let r = RegGetValueW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(k.as_ptr()),
            PCWSTR(n.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as _),
            Some(&mut size),
        );
        if r.is_err() {
            return 0;
        }
        from_wide(&buf).parse().unwrap_or(0)
    }
}

pub fn remote_session() -> bool {
    unsafe { GetSystemMetrics(SM_REMOTESESSION) != 0 }
}

// ---------------------------------------------------------------- sound

pub mod sound {
    use std::sync::OnceLock;

    use windows::core::PCWSTR;
    use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};

    use crate::app::Chime;

    /// Synthesises a soft FM/sine chime as a 16-bit mono WAV at 30 % volume.
    fn synth(notes: &[(f32, f32, f32)], len_s: f32) -> Vec<u8> {
        let rate = 44100u32;
        let n = (len_s * rate as f32) as usize;
        let mut pcm = vec![0f32; n];
        for &(freq, start, dur) in notes {
            let s0 = (start * rate as f32) as usize;
            let len = (dur * rate as f32) as usize;
            for i in 0..len {
                let idx = s0 + i;
                if idx >= n {
                    break;
                }
                let t = i as f32 / rate as f32;
                let attack = (t / 0.008).min(1.0);
                let env = attack * (-t * 4.5 / dur).exp();
                let fm =
                    (2.0 * std::f32::consts::PI * freq * 2.0 * t).sin() * 0.6 * (-t * 8.0).exp();
                pcm[idx] += (2.0 * std::f32::consts::PI * freq * t + fm).sin() * env * 0.3;
            }
        }
        let data: Vec<u8> = pcm
            .iter()
            .flat_map(|s| ((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())
            .collect();
        let mut w = Vec::with_capacity(44 + data.len());
        w.extend_from_slice(b"RIFF");
        w.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        w.extend_from_slice(b"WAVEfmt ");
        w.extend_from_slice(&16u32.to_le_bytes());
        w.extend_from_slice(&1u16.to_le_bytes());
        w.extend_from_slice(&1u16.to_le_bytes());
        w.extend_from_slice(&rate.to_le_bytes());
        w.extend_from_slice(&(rate * 2).to_le_bytes());
        w.extend_from_slice(&2u16.to_le_bytes());
        w.extend_from_slice(&16u16.to_le_bytes());
        w.extend_from_slice(b"data");
        w.extend_from_slice(&(data.len() as u32).to_le_bytes());
        w.extend_from_slice(&data);
        w
    }

    pub fn play(c: Chime) {
        static TIMER: OnceLock<Vec<u8>> = OnceLock::new();
        static REMIND: OnceLock<Vec<u8>> = OnceLock::new();
        static TAP: OnceLock<Vec<u8>> = OnceLock::new();
        static SUCCESS: OnceLock<Vec<u8>> = OnceLock::new();
        let buf = match c {
            Chime::TimerEnd => TIMER.get_or_init(|| {
                synth(
                    &[(659.3, 0.0, 0.9), (880.0, 0.18, 0.9), (1318.5, 0.36, 1.2)],
                    1.7,
                )
            }),
            Chime::Reminder => {
                REMIND.get_or_init(|| synth(&[(784.0, 0.0, 0.7), (1046.5, 0.14, 0.9)], 1.2))
            }
            Chime::Tap => TAP.get_or_init(|| synth(&[(1567.98, 0.0, 0.08)], 0.12)),
            Chime::Success => {
                SUCCESS.get_or_init(|| synth(&[(1046.5, 0.0, 0.25), (1568.0, 0.08, 0.4)], 0.6))
            }
        };
        unsafe {
            let _ = PlaySoundW(
                PCWSTR(buf.as_ptr() as *const u16),
                None,
                SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
            );
        }
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn wav_header() {
            let w = super::synth(&[(440.0, 0.0, 0.1)], 0.1);
            assert_eq!(&w[0..4], b"RIFF");
            assert_eq!(w.len(), 44 + 4410 * 2);
        }
    }
}

/// Shell thumbnail for a file (IShellItemImageFactory), as premultiplied BGRA.
pub fn dragdrop_thumb(path: &str, size: i32) -> Option<crate::render::image::Pixels> {
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::UI::Shell::{
        IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_BIGGERSIZEOK,
        SIIGBF_RESIZETOFIT,
    };
    unsafe {
        let w = wide(path);
        let f: IShellItemImageFactory =
            SHCreateItemFromParsingName(PCWSTR(w.as_ptr()), None).ok()?;
        let hbmp = f
            .GetImage(
                SIZE { cx: size, cy: size },
                SIIGBF_RESIZETOFIT | SIIGBF_BIGGERSIZEOK,
            )
            .ok()?;
        let mut bm = BITMAP::default();
        GetObjectW(
            hbmp.into(),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bm as *mut _ as _),
        );
        let (bw, bh) = (bm.bmWidth, bm.bmHeight);
        if bw <= 0 || bh <= 0 {
            let _ = DeleteObject(hbmp.into());
            return None;
        }
        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: 40,
                biWidth: bw,
                biHeight: -bh,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut data = vec![0u8; (bw * bh * 4) as usize];
        let hdc = GetDC(None);
        GetDIBits(
            hdc,
            hbmp,
            0,
            bh as u32,
            Some(data.as_mut_ptr() as _),
            &mut bmi,
            DIB_RGB_COLORS,
        );
        ReleaseDC(None, hdc);
        let _ = DeleteObject(hbmp.into());
        // thumbnails without alpha come back with a=0: treat as opaque
        if data.as_chunks::<4>().0.iter().all(|p| p[3] == 0) {
            for p in data.as_chunks_mut::<4>().0 {
                p[3] = 255;
            }
        }
        Some(crate::render::image::Pixels {
            w: bw as u32,
            h: bh as u32,
            data,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hotkeys_parse() {
        assert_eq!(
            parse_hotkey("Win+Alt+N"),
            Some((MOD_WIN | MOD_ALT, 'N' as u32))
        );
        assert_eq!(
            parse_hotkey("Ctrl+Alt+Space").map(|h| h.1),
            Some(VK_SPACE.0 as u32)
        );
        assert!(parse_hotkey("N").is_none());
        assert!(parse_hotkey("F9").is_some());
        assert!(parse_hotkey("Ctrl+Banana").is_none());
    }
}

// ---------------------------------------------------------------- system switches (Control Center)

pub mod system {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::System::Registry::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    use crate::util::wide;

    const PERSONALIZE: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";

    fn read_dword(name: &str) -> Option<u32> {
        let k = wide(PERSONALIZE);
        let n = wide(name);
        let mut v = 0u32;
        let mut size = 4u32;
        unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                PCWSTR(k.as_ptr()),
                PCWSTR(n.as_ptr()),
                RRF_RT_REG_DWORD,
                None,
                Some(&mut v as *mut u32 as _),
                Some(&mut size),
            )
            .ok()
            .ok()?;
        }
        Some(v)
    }

    /// Windows apps use the dark theme.
    pub fn dark_mode() -> bool {
        read_dword("AppsUseLightTheme") == Some(0)
    }

    /// Switches apps and the system (taskbar, Start) between light and dark, like Settings does.
    pub fn set_dark_mode(dark: bool) {
        let k = wide(PERSONALIZE);
        let light: u32 = if dark { 0 } else { 1 };
        for name in ["AppsUseLightTheme", "SystemUsesLightTheme"] {
            let n = wide(name);
            unsafe {
                let _ = RegSetKeyValueW(
                    HKEY_CURRENT_USER,
                    PCWSTR(k.as_ptr()),
                    PCWSTR(n.as_ptr()),
                    REG_DWORD.0,
                    Some(&light as *const u32 as _),
                    4,
                );
            }
        }
        // tell running apps, Explorer and the taskbar to re-read the colour set
        let topic = wide("ImmersiveColorSet");
        unsafe {
            let _ = SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_SETTINGCHANGE,
                WPARAM(0),
                LPARAM(topic.as_ptr() as isize),
                SMTO_ABORTIFHUNG,
                300,
                None,
            );
        }
    }

    /// Keeps the PC and display awake (like a "caffeine" switch) until turned off.
    pub fn keep_awake(on: bool) {
        use windows::Win32::System::Power::*;
        unsafe {
            let flags = if on {
                ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED
            } else {
                ES_CONTINUOUS
            };
            let _ = SetThreadExecutionState(flags);
        }
    }

    /// Locks the PC (Win+L).
    pub fn lock() {
        unsafe {
            let _ = windows::Win32::System::Shutdown::LockWorkStation();
        }
    }
}
