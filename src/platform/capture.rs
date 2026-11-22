//! Capture overlay for OCR and screenshots: snapshots every monitor (GDI, in
//! physical pixels so mixed-DPI setups work), dims them, and lets the user drag
//! a rectangle or click a window. Runs a modal loop and returns the pixels.

use std::cell::RefCell;

use windows::core::{BOOL, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::{
    DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS,
};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, SetFocus, VK_ESCAPE, VK_RETURN,
};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::render::image::Pixels;
use crate::util::wide;

struct Cap {
    hwnd: HWND,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    bright: Vec<u8>,
    dim: Vec<u8>,
    start: Option<(i32, i32)>,
    cur: (i32, i32),
    hover: Option<RECT>,
    result: Option<RECT>,
    done: bool,
    hint: String,
}

thread_local! {
    static CAP: RefCell<Option<Cap>> = const { RefCell::new(None) };
}

fn snapshot(x: i32, y: i32, w: i32, h: i32) -> Option<Vec<u8>> {
    unsafe {
        let screen = GetDC(None);
        let mem = CreateCompatibleDC(Some(screen));
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: 40,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let dib = CreateDIBSection(Some(mem), &bmi, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
        let old = SelectObject(mem, dib.into());
        let _ = BitBlt(mem, 0, 0, w, h, Some(screen), x, y, SRCCOPY | CAPTUREBLT);
        let mut v = std::slice::from_raw_parts(bits as *const u8, (w * h * 4) as usize).to_vec();
        for p in v.as_chunks_mut::<4>().0 {
            p[3] = 255;
        }
        SelectObject(mem, old);
        let _ = DeleteObject(dib.into());
        let _ = DeleteDC(mem);
        ReleaseDC(None, screen);
        Some(v)
    }
}

fn norm(a: (i32, i32), b: (i32, i32)) -> RECT {
    RECT {
        left: a.0.min(b.0),
        top: a.1.min(b.1),
        right: a.0.max(b.0),
        bottom: a.1.max(b.1),
    }
}

/// Topmost visible window under a screen point, excluding the overlay.
fn window_at(pt: POINT, exclude: HWND) -> Option<RECT> {
    struct Ctx {
        pt: POINT,
        exclude: HWND,
        found: Option<RECT>,
    }
    unsafe extern "system" fn cb(h: HWND, lp: LPARAM) -> BOOL {
        let c = unsafe { &mut *(lp.0 as *mut Ctx) };
        if h == c.exclude
            || !unsafe { IsWindowVisible(h) }.as_bool()
            || unsafe { IsIconic(h) }.as_bool()
        {
            return true.into();
        }
        let mut cloaked = 0u32;
        let _ = unsafe { DwmGetWindowAttribute(h, DWMWA_CLOAKED, &mut cloaked as *mut _ as _, 4) };
        if cloaked != 0 {
            return true.into();
        }
        let ex = unsafe { GetWindowLongW(h, GWL_EXSTYLE) } as u32;
        if ex & WS_EX_TRANSPARENT.0 != 0
            || ex & WS_EX_NOACTIVATE.0 != 0 && ex & WS_EX_TOPMOST.0 != 0
        {
            return true.into();
        }
        let mut r = RECT::default();
        if unsafe {
            DwmGetWindowAttribute(
                h,
                DWMWA_EXTENDED_FRAME_BOUNDS,
                &mut r as *mut _ as _,
                std::mem::size_of::<RECT>() as u32,
            )
        }
        .is_err()
        {
            let _ = unsafe { GetWindowRect(h, &mut r) };
        }
        if c.pt.x >= r.left
            && c.pt.x < r.right
            && c.pt.y >= r.top
            && c.pt.y < r.bottom
            && r.right - r.left > 40
        {
            c.found = Some(r);
            return false.into();
        }
        true.into()
    }
    let mut c = Ctx {
        pt,
        exclude,
        found: None,
    };
    unsafe {
        let _ = EnumWindows(Some(cb), LPARAM(&mut c as *mut _ as isize));
    }
    c.found
}

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let pt = || {
        (
            ((lp.0 & 0xffff) as i16) as i32,
            (((lp.0 >> 16) & 0xffff) as i16) as i32,
        )
    };
    match msg {
        WM_PAINT => {
            CAP.with(|c| {
                if let Some(c) = c.borrow().as_ref() {
                    unsafe { paint(hwnd, c) };
                }
            });
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_LBUTTONDOWN => {
            CAP.with(|c| {
                if let Some(c) = c.borrow_mut().as_mut() {
                    c.start = Some(pt());
                    c.cur = pt();
                }
            });
            unsafe {
                SetCapture(hwnd);
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            CAP.with(|c| {
                if let Some(c) = c.borrow_mut().as_mut() {
                    c.cur = pt();
                    if c.start.is_none() {
                        let p = POINT {
                            x: c.cur.0 + c.x,
                            y: c.cur.1 + c.y,
                        };
                        c.hover = window_at(p, hwnd).map(|r| RECT {
                            left: r.left - c.x,
                            top: r.top - c.y,
                            right: r.right - c.x,
                            bottom: r.bottom - c.y,
                        });
                    }
                }
            });
            unsafe {
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            unsafe {
                let _ = ReleaseCapture();
            }
            CAP.with(|c| {
                if let Some(c) = c.borrow_mut().as_mut() {
                    if let Some(s) = c.start.take() {
                        let r = norm(s, pt());
                        if r.right - r.left > 6 && r.bottom - r.top > 6 {
                            c.result = Some(r);
                        } else {
                            c.result = c.hover;
                        }
                        c.done = true;
                    }
                }
            });
            LRESULT(0)
        }
        WM_RBUTTONUP => {
            CAP.with(|c| {
                if let Some(c) = c.borrow_mut().as_mut() {
                    c.done = true;
                }
            });
            LRESULT(0)
        }
        WM_KEYDOWN => {
            CAP.with(|c| {
                if let Some(c) = c.borrow_mut().as_mut() {
                    if wp.0 as u16 == VK_ESCAPE.0 {
                        c.done = true;
                    } else if wp.0 as u16 == VK_RETURN.0 {
                        // full screen of the monitor under the cursor
                        c.result = Some(RECT {
                            left: 0,
                            top: 0,
                            right: c.w,
                            bottom: c.h,
                        });
                        c.done = true;
                    }
                }
            });
            LRESULT(0)
        }
        WM_SETCURSOR => {
            unsafe {
                let _ = SetCursor(LoadCursorW(None, IDC_CROSS).ok());
            }
            LRESULT(1)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
    }
}

unsafe fn paint(hwnd: HWND, c: &Cap) {
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);
        let mem = CreateCompatibleDC(Some(hdc));
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: 40,
                biWidth: c.w,
                biHeight: -c.h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        if let Ok(dib) = CreateDIBSection(Some(mem), &bmi, DIB_RGB_COLORS, &mut bits, None, 0) {
            let dst = std::slice::from_raw_parts_mut(bits as *mut u8, c.dim.len());
            dst.copy_from_slice(&c.dim);
            let sel = match c.start {
                Some(s) => Some(norm(s, c.cur)),
                None => c.hover,
            };
            if let Some(r) = sel {
                let (l, t) = (r.left.clamp(0, c.w), r.top.clamp(0, c.h));
                let (rr, b) = (r.right.clamp(0, c.w), r.bottom.clamp(0, c.h));
                for y in t..b {
                    let o = ((y * c.w + l) * 4) as usize;
                    let e = ((y * c.w + rr) * 4) as usize;
                    dst[o..e].copy_from_slice(&c.bright[o..e]);
                }
            }
            let old = SelectObject(mem, dib.into());
            if let Some(r) = sel {
                let pen = CreatePen(PS_SOLID, 2, COLORREF(0x00FFB819));
                let op = SelectObject(mem, pen.into());
                let ob = SelectObject(mem, GetStockObject(NULL_BRUSH));
                let _ = Rectangle(mem, r.left, r.top, r.right, r.bottom);
                SelectObject(mem, op);
                SelectObject(mem, ob);
                let _ = DeleteObject(pen.into());
            }
            SetBkMode(mem, TRANSPARENT);
            SetTextColor(mem, COLORREF(0x00F7EFEC));
            let mut hint: Vec<u16> = c.hint.encode_utf16().collect();
            let mut tr = RECT {
                left: 0,
                top: 24,
                right: c.w,
                bottom: 60,
            };
            DrawTextW(mem, &mut hint, &mut tr, DT_CENTER | DT_SINGLELINE);
            let _ = BitBlt(hdc, 0, 0, c.w, c.h, Some(mem), 0, 0, SRCCOPY);
            SelectObject(mem, old);
            let _ = DeleteObject(dib.into());
        }
        let _ = DeleteDC(mem);
        let _ = EndPaint(hwnd, &ps);
    }
}

/// Runs the capture UI and returns the selected pixels (None if cancelled).
pub fn capture(hint: &str) -> Option<Pixels> {
    unsafe {
        let (x, y) = (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
        );
        let (w, h) = (
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        );
        let bright = snapshot(x, y, w, h)?;
        let dim: Vec<u8> = bright
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                if i % 4 == 3 {
                    255
                } else {
                    (v as u32 * 45 / 100) as u8
                }
            })
            .collect();
        let inst = GetModuleHandleW(None).ok()?;
        let class = wide("NotchlyCapture");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: inst.into(),
            lpszClassName: PCWSTR(class.as_ptr()),
            hCursor: LoadCursorW(None, IDC_CROSS).ok()?,
            ..Default::default()
        };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            PCWSTR(class.as_ptr()),
            PCWSTR::null(),
            WS_POPUP,
            x,
            y,
            w,
            h,
            None,
            None,
            Some(inst.into()),
            None,
        )
        .ok()?;
        CAP.with(|c| {
            *c.borrow_mut() = Some(Cap {
                hwnd,
                x,
                y,
                w,
                h,
                bright,
                dim,
                start: None,
                cur: (0, 0),
                hover: None,
                result: None,
                done: false,
                hint: hint.to_string(),
            })
        });
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(Some(hwnd));
        let mut msg = MSG::default();
        loop {
            let done = CAP.with(|c| c.borrow().as_ref().map(|c| c.done).unwrap_or(true));
            if done {
                break;
            }
            if !GetMessageW(&mut msg, None, 0, 0).as_bool() {
                PostQuitMessage(0);
                break;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let cap = CAP.with(|c| c.borrow_mut().take())?;
        let _ = DestroyWindow(cap.hwnd);
        let r = cap.result?;
        let (l, t) = (r.left.clamp(0, cap.w), r.top.clamp(0, cap.h));
        let (rw, rh) = (
            (r.right.clamp(0, cap.w) - l).max(1),
            (r.bottom.clamp(0, cap.h) - t).max(1),
        );
        let mut out = Vec::with_capacity((rw * rh * 4) as usize);
        for yy in t..t + rh {
            let o = ((yy * cap.w + l) * 4) as usize;
            out.extend_from_slice(&cap.bright[o..o + (rw * 4) as usize]);
        }
        Some(Pixels {
            w: rw as u32,
            h: rh as u32,
            data: out,
        })
    }
}
