//! QA helper: a bright test pattern window (stripes, a diagonal, colour blocks) to sit behind
//! the notch while checking blur, lens and alignment. Run: glass_backdrop <seconds>

use std::sync::atomic::{AtomicI32, Ordering};
use std::time::{Duration, Instant};

/// Stripe offset (animated mode scrolls it every frame, like video behind the notch).
static OFFSET: AtomicI32 = AtomicI32::new(0);
/// Plain light backdrop instead of stripes (a grey box shows best on white).
static WHITE_MODE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

use windows::core::w;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::*;
use windows::Win32::UI::WindowsAndMessaging::*;

unsafe extern "system" fn wndproc(h: HWND, m: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        if m == WM_PAINT {
            let mut ps = PAINTSTRUCT::default();
            let dc = BeginPaint(h, &mut ps);
            let mut rc = RECT::default();
            let _ = GetClientRect(h, &mut rc);
            let cols = [
                0x003C8CFF, 0x00FF9E3D, 0x00FFFFFF, 0x00C8D02A, 0x005A2AE8, 0x0020C0F0,
            ];
            if WHITE_MODE.load(Ordering::Relaxed) {
                let b = CreateSolidBrush(COLORREF(0x00F4F4F4));
                FillRect(dc, &rc, b);
                let _ = DeleteObject(b.into());
                let _ = EndPaint(h, &ps);
                return LRESULT(0);
            }
            let off = OFFSET.load(Ordering::Relaxed) % 216;
            let mut x = -off;
            let mut i = 0;
            while x < rc.right {
                let r = RECT {
                    left: x,
                    top: 0,
                    right: x + 36,
                    bottom: rc.bottom,
                };
                let b = CreateSolidBrush(COLORREF(cols[i % cols.len()]));
                FillRect(dc, &r, b);
                let _ = DeleteObject(b.into());
                x += 36;
                i += 1;
            }
            // dark band and a white diagonal to show lensing at the edges
            let band = RECT {
                left: 0,
                top: 120,
                right: rc.right,
                bottom: 150,
            };
            let b = CreateSolidBrush(COLORREF(0x00101010));
            FillRect(dc, &band, b);
            let _ = DeleteObject(b.into());
            let pen = CreatePen(PS_SOLID, 10, COLORREF(0x00FFFFFF));
            let old = SelectObject(dc, pen.into());
            let _ = MoveToEx(dc, 0, 0, None);
            let _ = LineTo(dc, rc.right, rc.bottom);
            SelectObject(dc, old);
            let _ = DeleteObject(pen.into());
            let _ = EndPaint(h, &ps);
            return LRESULT(0);
        }
        DefWindowProcW(h, m, wp, lp)
    }
}

fn main() {
    let secs: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(12);
    let anim = std::env::args().any(|a| a == "anim");
    WHITE_MODE.store(std::env::args().any(|a| a == "white"), Ordering::Relaxed);
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let hinst: HINSTANCE = GetModuleHandleW(None).unwrap().into();
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: hinst,
            lpszClassName: w!("GlassBackdrop"),
            ..Default::default()
        };
        RegisterClassW(&wc);
        let h = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            w!("GlassBackdrop"),
            w!("backdrop"),
            WS_POPUP | WS_VISIBLE,
            0,
            0,
            1100,
            560,
            None,
            None,
            Some(hinst),
            None,
        )
        .unwrap();
        let _ = UpdateWindow(h);
        let until = Instant::now() + Duration::from_secs(secs);
        while Instant::now() < until {
            let mut m = MSG::default();
            while PeekMessageW(&mut m, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&m);
                DispatchMessageW(&m);
            }
            if anim {
                OFFSET.fetch_add(3, Ordering::Relaxed);
                let _ = InvalidateRect(Some(h), None, false);
            }
            std::thread::sleep(Duration::from_millis(16));
        }
        let _ = DestroyWindow(h);
    }
}
