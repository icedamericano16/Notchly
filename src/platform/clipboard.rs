//! Clipboard read/write. Respects clipboard-history exclusion formats used by
//! password managers and a user list of excluded apps.

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HANDLE, HGLOBAL, HWND};
use windows::Win32::Graphics::Gdi::{BITMAPINFOHEADER, BI_BITFIELDS, BI_RGB};
use windows::Win32::System::DataExchange::*;
use windows::Win32::System::Memory::{
    GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE,
};
use windows::Win32::System::Ole::{CF_DIB, CF_HDROP, CF_UNICODETEXT};
use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

use crate::render::image::Pixels;
use crate::util::wide;

pub enum Clip {
    Text(String),
    Image(Pixels),
    Files(Vec<String>),
}

fn fmt(name: &str) -> u32 {
    let w = wide(name);
    unsafe { RegisterClipboardFormatW(PCWSTR(w.as_ptr())) }
}

struct Guard;
impl Drop for Guard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseClipboard();
        }
    }
}

fn open(hwnd: HWND) -> Option<Guard> {
    for _ in 0..5 {
        if unsafe { OpenClipboard(Some(hwnd)) }.is_ok() {
            return Some(Guard);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    None
}

pub fn owner_exe() -> String {
    unsafe {
        let Ok(h) = GetClipboardOwner() else {
            return String::new();
        };
        let mut pid = 0;
        GetWindowThreadProcessId(h, Some(&mut pid));
        crate::services::audio::process_name(pid)
    }
}

/// Reads the clipboard unless the source asked to be excluded from history.
pub fn read(hwnd: HWND, excluded_apps: &[String]) -> Option<Clip> {
    unsafe {
        if IsClipboardFormatAvailable(fmt("ExcludeClipboardContentFromMonitorProcessing")).is_ok()
            || IsClipboardFormatAvailable(fmt("Clipboard Viewer Ignore")).is_ok()
        {
            return None;
        }
        let owner = owner_exe();
        if !owner.is_empty() && excluded_apps.iter().any(|a| a.eq_ignore_ascii_case(&owner)) {
            return None;
        }
        let _g = open(hwnd)?;
        let can_hist = fmt("CanIncludeInClipboardHistory");
        if let Ok(h) = GetClipboardData(can_hist) {
            let g = HGLOBAL(h.0);
            let p = GlobalLock(g) as *const u32;
            let v = if p.is_null() { 1 } else { *p };
            let _ = GlobalUnlock(g);
            if v == 0 {
                return None;
            }
        }
        if let Ok(h) = GetClipboardData(CF_UNICODETEXT.0 as u32) {
            let g = HGLOBAL(h.0);
            let p = GlobalLock(g) as *const u16;
            if !p.is_null() {
                // SAFETY: GlobalLock succeeded, so `p` addresses the whole block, which is
                // GlobalSize bytes long; it stays locked until GlobalUnlock below.
                let n = GlobalSize(g) / 2;
                let s = std::slice::from_raw_parts(p, n);
                let text = crate::util::from_wide(s);
                let _ = GlobalUnlock(g);
                return Some(Clip::Text(text));
            }
        }
        if let Ok(h) = GetClipboardData(CF_HDROP.0 as u32) {
            let files =
                crate::platform::dragdrop::hdrop_files(windows::Win32::UI::Shell::HDROP(h.0));
            if !files.is_empty() {
                return Some(Clip::Files(files));
            }
        }
        if let Ok(h) = GetClipboardData(CF_DIB.0 as u32) {
            let g = HGLOBAL(h.0);
            let p = GlobalLock(g) as *const u8;
            if !p.is_null() {
                // SAFETY: as above: a locked block of GlobalSize bytes, unlocked after use.
                let size = GlobalSize(g);
                let img = dib_to_pixels(std::slice::from_raw_parts(p, size));
                let _ = GlobalUnlock(g);
                return img.map(Clip::Image);
            }
        }
        None
    }
}

/// Converts a CF_DIB (32/24-bit) to premultiplied BGRA.
pub fn dib_to_pixels(d: &[u8]) -> Option<Pixels> {
    if d.len() < std::mem::size_of::<BITMAPINFOHEADER>() {
        return None;
    }
    // SAFETY: `d` holds at least size_of::<BITMAPINFOHEADER>() bytes (checked above) and the
    // header is plain data; read_unaligned because a byte slice has no alignment guarantee.
    let h = unsafe { std::ptr::read_unaligned(d.as_ptr() as *const BITMAPINFOHEADER) };
    let (w, hh) = (h.biWidth, h.biHeight);
    if w <= 0 || hh == 0 || w > 16384 || hh.abs() > 16384 {
        return None;
    }
    let bpp = h.biBitCount as usize;
    if bpp != 32 && bpp != 24 {
        return None;
    }
    let mut off = h.biSize as usize;
    if h.biCompression == BI_BITFIELDS.0 && h.biSize == 40 {
        off += 12;
    } else if h.biCompression != BI_RGB.0 && h.biCompression != BI_BITFIELDS.0 {
        return None;
    }
    off += h.biClrUsed as usize * 4;
    let stride = (w as usize * bpp).div_ceil(32) * 4;
    let rows = hh.unsigned_abs() as usize;
    if d.len() < off + stride * rows {
        return None;
    }
    let mut out = vec![0u8; w as usize * rows * 4];
    let mut any_alpha = false;
    for y in 0..rows {
        let src_row = if hh > 0 { rows - 1 - y } else { y };
        let src = &d[off + src_row * stride..];
        for x in 0..w as usize {
            let o = (y * w as usize + x) * 4;
            let s = x * bpp / 8;
            out[o] = src[s];
            out[o + 1] = src[s + 1];
            out[o + 2] = src[s + 2];
            out[o + 3] = if bpp == 32 { src[s + 3] } else { 255 };
            any_alpha |= bpp == 32 && src[s + 3] != 0;
        }
    }
    if !any_alpha {
        for p in out.as_chunks_mut::<4>().0 {
            p[3] = 255;
        }
    }
    Some(Pixels {
        w: w as u32,
        h: rows as u32,
        data: out,
    })
}

unsafe fn put(format: u32, bytes: &[u8]) -> bool {
    unsafe {
        let Ok(g) = GlobalAlloc(GMEM_MOVEABLE, bytes.len()) else {
            return false;
        };
        let p = GlobalLock(g) as *mut u8;
        if p.is_null() {
            return false;
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
        let _ = GlobalUnlock(g);
        SetClipboardData(format, Some(HANDLE(g.0))).is_ok()
    }
}

pub fn set_text(hwnd: HWND, s: &str) -> bool {
    let w = wide(s);
    let bytes = crate::util::u16_bytes(&w);
    let Some(_g) = open(hwnd) else { return false };
    unsafe {
        let _ = EmptyClipboard();
        put(CF_UNICODETEXT.0 as u32, &bytes)
    }
}

pub fn set_image(hwnd: HWND, px: &Pixels) -> bool {
    let hdr = BITMAPINFOHEADER {
        biSize: 40,
        biWidth: px.w as i32,
        biHeight: -(px.h as i32),
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        ..Default::default()
    };
    // SAFETY: BITMAPINFOHEADER is a #[repr(C)] plain-data struct of exactly 40 bytes with no
    // padding; `hdr` lives across the read.
    let mut bytes =
        unsafe { std::slice::from_raw_parts(&hdr as *const _ as *const u8, 40) }.to_vec();
    bytes.extend_from_slice(&px.data);
    let Some(_g) = open(hwnd) else { return false };
    unsafe {
        let _ = EmptyClipboard();
        put(CF_DIB.0 as u32, &bytes)
    }
}

pub fn sequence() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
}
