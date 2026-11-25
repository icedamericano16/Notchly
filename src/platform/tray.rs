//! Tray icon with status tooltip and context menu; re-added on TaskbarCreated.

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::util::wide;

pub const WM_TRAY: u32 = WM_APP + 2;
pub const TRAY_ID: u32 = 1;

pub mod cmd {
    pub const OPEN: usize = 100;
    pub const PAUSE_1H: usize = 101;
    pub const PAUSE_TOMORROW: usize = 102;
    pub const RESUME: usize = 103;
    pub const QUIET: usize = 104;
    pub const SETTINGS: usize = 105;
    pub const UPDATES: usize = 106;
    pub const QUIT: usize = 107;
    pub const DISPLAY_BASE: usize = 200;
    /// "Follow the mouse" in the Display submenu.
    pub const DISPLAY_FOLLOW: usize = 199;
}

/// Colour of the Notchly logo (assets/brand/notchly-logo.svg: violet squircle, black notch,
/// two eyes) at a point in its 1024x1024 design space, or None outside the squircle.
/// The same geometry draws the exe icon (build.rs) and the MSIX logos (make_assets.py).
fn logo_sample(x: f32, y: f32) -> Option<(f32, f32, f32)> {
    let (u, v) = (((x - 512.0) / 512.0).abs(), ((y - 512.0) / 512.0).abs());
    if u.powf(7.0) + v.powf(7.0) > 1.0 {
        return None;
    }
    let in_rrect = |x0: f32, y0: f32, w: f32, h: f32, r: f32| {
        if x < x0 || x > x0 + w || y < y0 || y > y0 + h {
            return false;
        }
        let cx = x.clamp(x0 + r, x0 + w - r);
        let cy = y.clamp(y0 + r, y0 + h - r);
        (x - cx).powi(2) + (y - cy).powi(2) <= r * r
    };
    if in_rrect(364.0, 383.32, 84.0, 114.0, 42.0) || in_rrect(576.0, 383.32, 84.0, 114.0, 42.0) {
        return Some((233.0, 252.0, 248.0));
    }
    if in_rrect(123.0, 302.82, 778.0, 275.0, 137.5) {
        return Some((8.0, 8.0, 12.0));
    }
    let t = ((x + y) / 2048.0).clamp(0.0, 1.0);
    Some((132.0 - 48.0 * t, 56.0 - 24.0 * t, 255.0))
}

/// Draws the Notchly logo into an icon (4x4 supersampled), with an optional orange status dot.
pub fn make_icon(size: i32, dot: bool) -> HICON {
    unsafe {
        let hdc = GetDC(None);
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: 40,
                biWidth: size,
                biHeight: -size,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let Ok(color) = CreateDIBSection(Some(hdc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0)
        else {
            ReleaseDC(None, hdc);
            return HICON::default();
        };
        // SAFETY: CreateDIBSection succeeded for a size x size 32-bit DIB, so `bits` points to
        // that many writable pixels owned by `color`, which outlives this slice.
        let px = std::slice::from_raw_parts_mut(bits as *mut u8, (size * size * 4) as usize);
        let f = size as f32;
        const SS: i32 = 4;
        for y in 0..size {
            for x in 0..size {
                // premultiplied colour + coverage, averaged over SS x SS samples
                let (mut r, mut g, mut b, mut a) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
                for sy in 0..SS {
                    for sx in 0..SS {
                        let fx = x as f32 + (sx as f32 + 0.5) / SS as f32;
                        let fy = y as f32 + (sy as f32 + 0.5) / SS as f32;
                        let dot_hit = dot
                            && ((fx - f * 0.82).powi(2) + (fy - f * 0.2).powi(2)).sqrt() < f * 0.16;
                        let c = if dot_hit {
                            Some((255.0, 159.0, 10.0))
                        } else {
                            logo_sample(fx / f * 1024.0, fy / f * 1024.0)
                        };
                        if let Some((cr, cg, cb)) = c {
                            r += cr;
                            g += cg;
                            b += cb;
                            a += 1.0;
                        }
                    }
                }
                let n = (SS * SS) as f32;
                let o = ((y * size + x) * 4) as usize;
                px[o] = (b / n) as u8;
                px[o + 1] = (g / n) as u8;
                px[o + 2] = (r / n) as u8;
                px[o + 3] = (a / n * 255.0) as u8;
            }
        }
        let mask = CreateBitmap(size, size, 1, 1, None);
        let info = ICONINFO {
            fIcon: true.into(),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        };
        let icon = CreateIconIndirect(&info).unwrap_or_default();
        let _ = DeleteObject(color.into());
        let _ = DeleteObject(mask.into());
        ReleaseDC(None, hdc);
        icon
    }
}

fn data(hwnd: HWND, tip: &str, dot: bool) -> NOTIFYICONDATAW {
    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_ID,
        uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP | NIF_SHOWTIP,
        uCallbackMessage: WM_TRAY,
        hIcon: make_icon(32, dot),
        ..Default::default()
    };
    let w: Vec<u16> = tip.encode_utf16().take(127).collect();
    nid.szTip[..w.len()].copy_from_slice(&w);
    nid.Anonymous.uVersion = NOTIFYICON_VERSION_4;
    nid
}

pub fn add(hwnd: HWND, tip: &str) {
    let nid = data(hwnd, tip, false);
    unsafe {
        let _ = Shell_NotifyIconW(NIM_ADD, &nid);
        let _ = Shell_NotifyIconW(NIM_SETVERSION, &nid);
        let _ = DestroyIcon(nid.hIcon);
    }
}

pub fn update(hwnd: HWND, tip: &str, dot: bool) {
    let nid = data(hwnd, tip, dot);
    unsafe {
        let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
        let _ = DestroyIcon(nid.hIcon);
    }
}

pub fn remove(hwnd: HWND) {
    let nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_ID,
        ..Default::default()
    };
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
    }
}

pub fn taskbar_created_msg() -> u32 {
    let w = wide("TaskbarCreated");
    unsafe { RegisterWindowMessageW(PCWSTR(w.as_ptr())) }
}

/// Shows the context menu; returns the chosen command id.
pub fn menu(
    hwnd: HWND,
    paused: bool,
    quiet: bool,
    monitors: &[String],
    current_monitor: u32,
) -> usize {
    unsafe {
        let Ok(m) = CreatePopupMenu() else { return 0 };
        let add = |m: HMENU, id: usize, text: &str, checked: bool| {
            let w = wide(text);
            let flags = MF_STRING | if checked { MF_CHECKED } else { MF_UNCHECKED };
            let _ = AppendMenuW(m, flags, id, PCWSTR(w.as_ptr()));
        };
        add(m, cmd::OPEN, "Open", false);
        let _ = AppendMenuW(m, MF_SEPARATOR, 0, PCWSTR::null());
        if paused {
            add(m, cmd::RESUME, "Resume pop-ups", false);
        } else {
            add(m, cmd::PAUSE_1H, "Pause pop-ups for 1 hour", false);
            add(
                m,
                cmd::PAUSE_TOMORROW,
                "Pause pop-ups until tomorrow",
                false,
            );
        }
        add(m, cmd::QUIET, "Quiet hours", quiet);
        if let Ok(sub) = CreatePopupMenu() {
            add(
                sub,
                cmd::DISPLAY_BASE,
                "Primary display",
                current_monitor == 0,
            );
            for (i, name) in monitors.iter().enumerate() {
                add(
                    sub,
                    cmd::DISPLAY_BASE + i + 1,
                    name,
                    current_monitor == i as u32 + 1,
                );
            }
            let _ = AppendMenuW(sub, MF_SEPARATOR, 0, PCWSTR::null());
            add(
                sub,
                cmd::DISPLAY_FOLLOW,
                "Follow the mouse",
                current_monitor == crate::settings::MONITOR_FOLLOW,
            );
            let w = wide("Display");
            let _ = AppendMenuW(m, MF_POPUP, sub.0 as usize, PCWSTR(w.as_ptr()));
        }
        let _ = AppendMenuW(m, MF_SEPARATOR, 0, PCWSTR::null());
        add(m, cmd::SETTINGS, "Settings…", false);
        add(m, cmd::UPDATES, "Check for updates", false);
        add(m, cmd::QUIT, "Quit Notchly", false);
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let _ = SetForegroundWindow(hwnd);
        let r = TrackPopupMenu(
            m,
            TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY,
            pt.x,
            pt.y,
            Some(0),
            hwnd,
            None,
        );
        let _ = DestroyMenu(m);
        r.0 as usize
    }
}
