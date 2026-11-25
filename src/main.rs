//! Notchly: a living Dynamic Island for Windows.
//! One UI thread owns the windows, renderer and state; services post events.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod app_location;
mod build_info;
mod features;
mod i18n;
mod motion;
mod platform;
mod platform_dialogs;
mod render;
mod services;
mod settings;
mod sync;
mod theme;
mod ui;
mod updates;
mod util;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::{Duration, Instant};

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Ole::OleInitialize;
use windows::Win32::System::Power::*;
use windows::Win32::System::SystemServices::{
    GUID_ACDC_POWER_SOURCE, GUID_BATTERY_PERCENTAGE_REMAINING, GUID_POWER_SAVING_STATUS,
};
use windows::Win32::System::Threading::INFINITE;
use windows::Win32::System::Threading::{CreateMutexW, GetCurrentProcess};
use windows::Win32::UI::Accessibility::*;
use windows::Win32::UI::Controls::{MARGINS, WM_MOUSELEAVE};
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use app::activity::{PopKind, Popup};
use app::events::PowerSignal;
use app::input::{Button, ClickOutcome};
use app::state::{Battery, State};
use app::{CaptureMode, InputId, PageId, Request};
use render::glass::backend::{self as glass_backend, Reason, Tier};
use render::glass::duplication::{self as dup, Capture, Receiver};
use render::icons::Icon;
use render::{Canvas, Gfx, Surface};
use services::{Bus, Event, WM_APP_EVENT};
use settings::window::SettingsWin;
use theme::tokens::*;
use util::{log, wide};

mod host_glass;

mod host_requests;

mod host_menus;

mod host_windows;

const WM_OPEN_REQUEST: u32 = WM_APP + 3;
const WM_OUTSIDE_CLICK: u32 = WM_APP + 4;
/// The glass capture thread has a new backdrop frame.
const WM_GLASS_FRAME: u32 = WM_APP + 5;
/// Cursor poll while the margins are click-through.
const TIMER_HIT: usize = 77;

/// `NOTCHLY_TRACE=glass` (or `all`): glass tier / region diagnostics in the log.
fn trace_glass() -> bool {
    std::env::var("NOTCHLY_TRACE").is_ok_and(|v| v == "glass" || v == "all")
}

/// Windows switches the glass depends on (re-read on settings / power / display changes).
#[derive(Clone, Copy)]
struct GlassEnv {
    build: u32,
    transparency: bool,
    remote: bool,
}

const HK_TOGGLE: i32 = 1;
const HK_PALETTE: i32 = 2;
const HK_OCR: i32 = 3;
const HK_CLIPBOARD: i32 = 4;
const HK_FOCUS: i32 = 5;
const HK_TRANSLATE: i32 = 6;
const HK_SCREENSHOT: i32 = 7;

/// Messages captured by the window procedures, processed on the next loop turn.
#[derive(Debug, Clone)]
enum Msg {
    Move(HWND, i32, i32),
    Leave(HWND),
    Down(HWND, i32, i32, Button),
    Up(HWND, i32, i32, Button),
    Wheel(HWND, i32, bool),
    Key(HWND, u32),
    Char(HWND, char),
    Hotkey(i32),
    Power(PowerSignal),
    Display,
    Dpi(HWND, u32, RECT),
    Setting,
    Clipboard,
    Tray(u32),
    TaskbarCreated,
    Activate(HWND, bool),
    Close(HWND),
    Size(HWND, u32, u32),
    Foreground,
    OpenRequest,
    OutsideClick,
    WidgetMoved(HWND),
}

thread_local! {
    static QUEUE: RefCell<VecDeque<Msg>> = const { RefCell::new(VecDeque::new()) };
    static TASKBAR_MSG: RefCell<u32> = const { RefCell::new(0) };
}

fn push(m: Msg) {
    QUEUE.with(|q| q.borrow_mut().push_back(m));
}

fn lo(lp: LPARAM) -> i32 {
    (lp.0 & 0xffff) as i16 as i32
}
fn hi(lp: LPARAM) -> i32 {
    ((lp.0 >> 16) & 0xffff) as i16 as i32
}

const GUID_BRIGHTNESS: windows::core::GUID =
    windows::core::GUID::from_u128(0x8ffee2c6_2d01_46be_adb9_398addc5b4ff);
const GUID_BRIGHTNESS_POLICY: windows::core::GUID =
    windows::core::GUID::from_u128(0xaded5e82_b909_4619_9949_f5d71dac0bcb);

#[cfg(debug_assertions)]
thread_local!(static QA_SETTINGS_DUMP: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) });

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let tb = TASKBAR_MSG.with(|t| *t.borrow());
    if msg == tb && tb != 0 {
        push(Msg::TaskbarCreated);
        return LRESULT(0);
    }
    match msg {
        WM_MOUSEACTIVATE => {
            // the island never steals focus on click
            if unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32 & WS_EX_NOACTIVATE.0 != 0 {
                return LRESULT(MA_NOACTIVATE as isize);
            }
        }
        WM_MOUSEMOVE => {
            push(Msg::Move(hwnd, lo(lp), hi(lp)));
            let mut tme = TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            unsafe {
                let _ = TrackMouseEvent(&mut tme);
            }
            return LRESULT(0);
        }
        WM_MOUSELEAVE => {
            push(Msg::Leave(hwnd));
            return LRESULT(0);
        }
        WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN => {
            let b = match msg {
                WM_LBUTTONDOWN => Button::Left,
                WM_RBUTTONDOWN => Button::Right,
                _ => Button::Middle,
            };
            unsafe { SetCapture(hwnd) };
            push(Msg::Down(hwnd, lo(lp), hi(lp), b));
            return LRESULT(0);
        }
        WM_LBUTTONUP | WM_RBUTTONUP | WM_MBUTTONUP => {
            let b = match msg {
                WM_LBUTTONUP => Button::Left,
                WM_RBUTTONUP => Button::Right,
                _ => Button::Middle,
            };
            unsafe {
                let _ = ReleaseCapture();
            }
            push(Msg::Up(hwnd, lo(lp), hi(lp), b));
            return LRESULT(0);
        }
        WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
            push(Msg::Wheel(
                hwnd,
                ((wp.0 >> 16) & 0xffff) as i16 as i32,
                msg == WM_MOUSEHWHEEL,
            ));
            return LRESULT(0);
        }
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            push(Msg::Key(hwnd, wp.0 as u32));
            if msg == WM_KEYDOWN {
                return LRESULT(0);
            }
        }
        WM_CHAR => {
            if let Some(c) = char::from_u32(wp.0 as u32) {
                push(Msg::Char(hwnd, c));
            }
            return LRESULT(0);
        }
        WM_HOTKEY => {
            push(Msg::Hotkey(wp.0 as i32));
            return LRESULT(0);
        }
        WM_POWERBROADCAST => {
            match wp.0 as u32 {
                PBT_POWERSETTINGCHANGE => {
                    // SAFETY: for PBT_POWERSETTINGCHANGE, lParam points to a
                    // POWERBROADCAST_SETTING that Windows keeps valid for this message.
                    let s = unsafe { &*(lp.0 as *const POWERBROADCAST_SETTING) };
                    if s.PowerSetting == GUID_BRIGHTNESS || s.PowerSetting == GUID_BRIGHTNESS_POLICY
                    {
                        // SAFETY: both brightness settings carry a DWORD in Data (DataLength 4);
                        // read_unaligned because Data is a byte array.
                        let v = unsafe { (s.Data.as_ptr() as *const u32).read_unaligned() };
                        push(Msg::Power(PowerSignal::Brightness(v)));
                    } else {
                        push(Msg::Power(PowerSignal::Status));
                    }
                }
                PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND => {
                    push(Msg::Power(PowerSignal::Resume))
                }
                PBT_APMPOWERSTATUSCHANGE => push(Msg::Power(PowerSignal::Status)),
                _ => {}
            }
            return LRESULT(1);
        }
        WM_DISPLAYCHANGE => push(Msg::Display),
        WM_GETMINMAXINFO => {
            // the Settings window (the only resizable one) has a minimum size
            if unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32 & WS_THICKFRAME.0 != 0 {
                let dpi = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd) }.max(96)
                    as f32
                    / 96.0;
                // SAFETY: for WM_GETMINMAXINFO, lParam points to a writable MINMAXINFO owned
                // by Windows for the duration of the message.
                let mm = unsafe { &mut *(lp.0 as *mut MINMAXINFO) };
                mm.ptMinTrackSize.x = (theme::tokens::SET_MIN_W * dpi) as i32;
                mm.ptMinTrackSize.y = (theme::tokens::SET_MIN_H * dpi) as i32;
                return LRESULT(0);
            }
        }
        WM_DPICHANGED => {
            // SAFETY: for WM_DPICHANGED, lParam points to the suggested RECT; copied out.
            let r = unsafe { *(lp.0 as *const RECT) };
            push(Msg::Dpi(hwnd, (wp.0 & 0xffff) as u32, r));
            return LRESULT(0);
        }
        WM_SETTINGCHANGE | WM_THEMECHANGED => push(Msg::Setting),
        WM_CLIPBOARDUPDATE => {
            push(Msg::Clipboard);
            return LRESULT(0);
        }
        platform::tray::WM_TRAY => {
            push(Msg::Tray((lp.0 & 0xffff) as u32));
            return LRESULT(0);
        }
        WM_ACTIVATE => push(Msg::Activate(hwnd, (wp.0 & 0xffff) != 0)),
        WM_CLOSE => {
            push(Msg::Close(hwnd));
            return LRESULT(0);
        }
        WM_SIZE => push(Msg::Size(
            hwnd,
            lo(lp) as u32 & 0xffff,
            hi(lp) as u32 & 0xffff,
        )),
        WM_OPEN_REQUEST => {
            push(Msg::OpenRequest);
            return LRESULT(0);
        }
        WM_OUTSIDE_CLICK => {
            push(Msg::OutsideClick);
            return LRESULT(0);
        }
        WM_GETOBJECT => {
            if let Some(r) = platform::uia::on_get_object(hwnd, wp, lp) {
                return r;
            }
        }
        WM_APP_EVENT | WM_TIMER => return LRESULT(0),
        WM_ERASEBKGND => return LRESULT(1),
        _ => {}
    }
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

/// Desktop widgets: draggable by their whole surface.
unsafe extern "system" fn widget_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_NCHITTEST => LRESULT(HTCAPTION as isize),
        WM_EXITSIZEMOVE => {
            push(Msg::WidgetMoved(hwnd));
            LRESULT(0)
        }
        WM_WINDOWPOSCHANGING => {
            // keep widgets at the bottom of the z-order
            // SAFETY: for WM_WINDOWPOSCHANGING, lParam points to a writable WINDOWPOS that
            // Windows reads back after the message; only valid during this call.
            let p = unsafe { &mut *(lp.0 as *mut WINDOWPOS) };
            p.hwndInsertAfter = HWND_BOTTOM;
            LRESULT(0)
        }
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        _ => unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
    }
}

static FG_HOOK_FLAG: AtomicBool = AtomicBool::new(false);

unsafe extern "system" fn win_event(
    _h: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    id_object: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) {
    if (event == EVENT_SYSTEM_FOREGROUND
        || (event == EVENT_OBJECT_LOCATIONCHANGE
            && id_object == OBJID_WINDOW.0
            && hwnd == unsafe { GetForegroundWindow() }))
        && !FG_HOOK_FLAG.swap(true, Ordering::Relaxed)
    {
        push(Msg::Foreground);
    }
}

/// The window the low-level mouse hook compares clicks against. The hook runs on the thread
/// that installed it, but an atomic keeps this sound without `static mut`.
static OUTSIDE_TARGET: AtomicIsize = AtomicIsize::new(0);

unsafe extern "system" fn mouse_hook(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code >= 0 && (wp.0 as u32 == WM_LBUTTONDOWN || wp.0 as u32 == WM_RBUTTONDOWN) {
        // SAFETY: for WH_MOUSE_LL with code >= 0, lParam points to an MSLLHOOKSTRUCT valid for
        // this call.
        let m = unsafe { &*(lp.0 as *const MSLLHOOKSTRUCT) };
        let target = HWND(OUTSIDE_TARGET.load(Ordering::Relaxed) as _);
        let mut r = RECT::default();
        unsafe {
            let _ = GetWindowRect(target, &mut r);
        }
        if m.pt.x < r.left || m.pt.x >= r.right || m.pt.y < r.top || m.pt.y >= r.bottom {
            unsafe {
                let _ = PostMessageW(Some(target), WM_OUTSIDE_CLICK, WPARAM(0), LPARAM(0));
            }
        }
    }
    unsafe { CallNextHookEx(None, code, wp, lp) }
}

struct Widget {
    hwnd: HWND,
    kind: String,
    surface: Surface,
}

struct App {
    gfx: Gfx,
    hwnd: HWND,
    surface: Surface,
    st: State,
    rx: std::sync::mpsc::Receiver<Event>,
    bus: Bus,
    settings_win: Option<SettingsWin>,
    widgets: Vec<Widget>,
    monitor: platform::Monitor,
    scale: f32,
    win_rect: (i32, i32, i32, i32),
    offset_px: (f32, f32),
    visible: bool,
    prev_fg: HWND,
    last_click: Instant,
    pending_paste: Option<Instant>,
    pending_translate: Option<(Instant, Option<String>)>,
    mouse_hook: Option<HHOOK>,
    loc_hook: Option<HWINEVENTHOOK>,
    tray_shown: bool,
    /// The glass tier applied to the window (DWM backdrop, region).
    glass_tier: Tier,
    glass_env: GlassEnv,
    /// Liquid tier: the capture worker and the render-side copy of its frames.
    capture: Option<Capture>,
    receiver: Option<Receiver>,
    capture_seen: u64,
    adapter: Option<(u64, (u32, i32))>,
    /// The window is excluded from screen capture (while we capture).
    excluded: bool,
    /// The window passes the mouse through (pointer outside the notch).
    click_through: bool,
    /// Window bounds held while the shape springs (grow at once, shrink when settled).
    hold: Option<render::Rect>,
    /// Recent window rectangles (screen px), so the capture can tell our own presents apart.
    own_rects: [(i32, i32, i32, i32); 4],
    perf_backdrop: u64,
    last_widget_draw: Instant,
    perf_last: Instant,
    perf_frames: u32,
    perf_cpu_ms: f32,
    last_battery_poll: Instant,
    /// When the island last went quiet; the working set is trimmed once it stays quiet.
    quiet_since: Option<Instant>,
    trimmed: bool,
}

impl App {
    fn new(settings: settings::Settings, demo: bool, perf: bool) -> windows::core::Result<Self> {
        let inst = unsafe { GetModuleHandleW(None)? };
        let class = w!("NotchlyIsland");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: inst.into(),
            lpszClassName: class,
            hCursor: unsafe { LoadCursorW(None, IDC_ARROW)? },
            ..Default::default()
        };
        unsafe { RegisterClassW(&wc) };
        let wcs = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: inst.into(),
            lpszClassName: w!("NotchlySettings"),
            hCursor: unsafe { LoadCursorW(None, IDC_ARROW)? },
            hIcon: unsafe { LoadIconW(Some(inst.into()), PCWSTR(1 as _)).unwrap_or_default() },
            ..Default::default()
        };
        unsafe { RegisterClassW(&wcs) };
        let wcw = WNDCLASSW {
            lpfnWndProc: Some(widget_proc),
            hInstance: inst.into(),
            lpszClassName: w!("NotchlyWidget"),
            hCursor: unsafe { LoadCursorW(None, IDC_ARROW)? },
            ..Default::default()
        };
        unsafe { RegisterClassW(&wcw) };
        // layered so the margins can be made click-through (WS_EX_TRANSPARENT); the content
        // is a composition swap chain, so no layered attributes are needed
        let ex = WS_EX_TOPMOST
            | WS_EX_TOOLWINDOW
            | WS_EX_NOREDIRECTIONBITMAP
            | WS_EX_NOACTIVATE
            | WS_EX_LAYERED;
        let hwnd = unsafe {
            CreateWindowExW(
                ex,
                class,
                w!("Notchly"),
                WS_POPUP,
                0,
                0,
                10,
                10,
                None,
                None,
                Some(inst.into()),
                None,
            )?
        };
        // a layered window takes no mouse input (and may not show) until it has layered
        // attributes; fully opaque, the composition swap chain provides the real alpha
        unsafe {
            let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA);
        }
        let monitor = platform::chosen_monitor(settings.general.monitor)
            .ok_or_else(|| windows::core::Error::from(E_FAIL))?;
        let gfx = Gfx::new()?;
        let (tx, rx) = channel();
        let bus = Bus::new(tx, hwnd);
        let mut st = State::new(settings);
        st.svc.bus = Some(bus.clone());
        st.perf = perf;
        st.os_animations = platform::os_animations();
        st.high_contrast = platform::high_contrast();
        st.system_light = platform::system_light();
        st.apply_settings();
        if demo {
            st.demo = Some((Instant::now(), 0));
        }
        let mut app = App {
            gfx,
            hwnd,
            surface: Surface::new(hwnd, 1, 1, 96.0),
            st,
            rx,
            bus,
            settings_win: None,
            widgets: Vec::new(),
            monitor,
            scale: 1.0,
            win_rect: (0, 0, 0, 0),
            offset_px: (0.0, 0.0),
            visible: false,
            prev_fg: HWND::default(),
            last_click: Instant::now() - Duration::from_secs(60),
            pending_paste: None,
            pending_translate: None,
            mouse_hook: None,
            loc_hook: None,
            tray_shown: false,
            glass_tier: Tier::Solid,
            glass_env: GlassEnv {
                build: platform::windows_build(),
                transparency: platform::transparency_enabled(),
                remote: platform::remote_session(),
            },
            capture: None,
            receiver: None,
            capture_seen: 0,
            adapter: None,
            excluded: false,
            click_through: false,
            hold: None,
            own_rects: [(0, 0, 0, 0); 4],
            perf_backdrop: 0,
            last_widget_draw: Instant::now(),
            perf_last: Instant::now(),
            perf_frames: 0,
            perf_cpu_ms: 0.0,
            last_battery_poll: Instant::now(),
            quiet_since: None,
            trimmed: false,
        };
        app.gfx.force_warp = platform::remote_session();
        app.gfx.timing = perf;
        app.update_monitor();
        app.start_services();
        app.register_os();
        Ok(app)
    }

    fn start_services(&mut self) {
        let s = &self.st.settings;
        let b = self.bus.clone();
        self.st.svc.media = Some(services::media::start(
            b.clone(),
            s.music.session_choice.clone(),
            s.music.preferred_player.clone(),
        ));
        self.st.svc.audio = Some(services::audio::start(b.clone()));
        services::audio::start_meter(
            self.st.f.meter_level.clone(),
            self.st.f.meter_active.clone(),
        );
        self.st.svc.radios = Some(services::radios::start(b.clone()));
        self.st.sys.dark_mode = platform::system::dark_mode();
        self.st.svc.lyrics = Some(services::lyrics::start(b.clone()));
        services::keyboard::start(b.clone());
        services::network::start(b.clone());
        services::bluetooth::start(b.clone());
        services::privacy::start(b.clone());
        services::notifications::start(b.clone());
        services::live_api::start(b.clone());
        services::visualiser::start(b.clone(), self.st.f.viz_active.clone());
        let alert = self.st.f.sysmon_alert.clone();
        services::sysmon::start(b.clone(), self.st.f.sysmon_active.clone(), alert);
        services::downloads::start(b.clone(), s.productivity.download_folders.clone());
        if s.privacy.net_currency {
            services::rates::refresh(b.clone());
        }
        if s.privacy.net_updates {
            updates::check(b.clone(), false);
        }
        if s.weather.use_windows_location {
            app_location::locate(b.clone());
        }
        self.crash_notice();
    }

    /// After an unexpected exit, offers the crash report once (copy / open folder / report).
    fn crash_notice(&mut self) {
        let dir = util::log_dir();
        let Ok(rd) = std::fs::read_dir(&dir) else {
            return;
        };
        let mut crashes: Vec<std::path::PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("crash-") && n.ends_with(".txt"))
            })
            .collect();
        if crashes.is_empty() {
            return;
        }
        crashes.sort();
        let newest = crashes.last().cloned().unwrap_or_default();
        let report: String = std::fs::read_to_string(&newest)
            .unwrap_or_default()
            .chars()
            .take(6000)
            .collect();
        for c in &crashes {
            let _ = std::fs::rename(c, c.with_extension("seen"));
        }
        // a pre-filled GitHub issue, user names taken out of paths; it opens in the browser,
        // where the user reads it and decides whether to submit it
        let issue = util::crash_issue_url(
            services::http::CONTACT_URL,
            &report,
            platform::windows_build(),
        );
        let p = Popup::new(
            PopKind::Info,
            Icon::Info,
            "Notchly restarted after a problem",
            "Sorry about that. The report stays on this PC.",
        )
        .action("Report on GitHub", app::Action::OpenUrl(issue))
        .action(
            "Copy report",
            app::Action::CopyText(util::redact_user_paths(&report)),
        )
        .action(
            "Open folder",
            app::Action::OpenUrl(dir.to_string_lossy().to_string()),
        )
        .duration_ms(12_000);
        self.st.popup(p);
    }

    fn register_os(&mut self) {
        let h = self.hwnd;
        unsafe {
            for g in [
                GUID_ACDC_POWER_SOURCE,
                GUID_BATTERY_PERCENTAGE_REMAINING,
                GUID_POWER_SAVING_STATUS,
                GUID_BRIGHTNESS,
                GUID_BRIGHTNESS_POLICY,
            ] {
                let _ =
                    RegisterPowerSettingNotification(HANDLE(h.0), &g, DEVICE_NOTIFY_WINDOW_HANDLE);
            }
            let _ = windows::Win32::System::DataExchange::AddClipboardFormatListener(h);
            SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                None,
                Some(win_event),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            );
        }
        TASKBAR_MSG.with(|t| *t.borrow_mut() = platform::tray::taskbar_created_msg());
        platform::dragdrop::register(h);
        self.register_hotkeys();
        self.update_tray();
        self.poll_battery();
        self.check_fullscreen();
        self.apply_glass();
        self.sync_widgets();
    }

    fn register_hotkeys(&mut self) {
        if qa_mode() {
            return;
        }
        let k = self.st.settings.shortcuts.clone();
        let failed = platform::register_hotkeys(
            self.hwnd,
            &[
                (HK_TOGGLE, "Open Notchly", &k.toggle),
                (HK_PALETTE, "Ask", &k.palette),
                (HK_OCR, "OCR", &k.ocr),
                (HK_CLIPBOARD, "Clipboard", &k.clipboard),
                (HK_FOCUS, "Focus timer", &k.focus),
                (HK_TRANSLATE, "Translate", &k.translate),
                (HK_SCREENSHOT, "Screenshot", &k.screenshot),
            ],
        );
        if !failed.is_empty() {
            let msg = format!("Shortcut in use: {}", failed.join(", "));
            log(&msg);
            self.st.popup(
                Popup::new(
                    PopKind::Info,
                    Icon::Keyboard,
                    "Shortcut conflict",
                    failed.join(", "),
                )
                .duration_ms(5000),
            );
            if let Some(sw) = self.settings_win.as_mut() {
                sw.show_toast(msg);
            }
        }
    }

    fn update_monitor(&mut self) {
        if let Some(m) = platform::chosen_monitor(self.st.settings.general.monitor) {
            self.monitor = m;
        }
        let preset = match self.st.settings.appearance.size {
            0 => {
                let logical_w = (self.monitor.rect.right - self.monitor.rect.left) as f32 * 96.0
                    / self.monitor.dpi as f32;
                if logical_w >= 2400.0 {
                    1.15
                } else {
                    1.0
                }
            }
            p => p as f32 / 100.0,
        };
        self.scale = self.monitor.dpi as f32 / 96.0 * preset;
        let (w, h) = (
            (CANVAS_W * self.scale).ceil() as u32,
            (CANVAS_H * self.scale).ceil() as u32,
        );
        self.surface.resize(w, h, 96.0 * self.scale);
        self.win_rect = (0, 0, 0, 0);
        self.st.dirty = true;
    }

    fn poll_battery(&mut self) {
        let mut s = SYSTEM_POWER_STATUS::default();
        if unsafe { GetSystemPowerStatus(&mut s) }.is_ok() {
            let present = s.BatteryFlag != 128 && s.BatteryFlag != 255;
            let b = Battery {
                present,
                percent: if s.BatteryLifePercent <= 100 {
                    s.BatteryLifePercent as u32
                } else {
                    0
                },
                ac: s.ACLineStatus == 1,
                charging: s.BatteryFlag & 8 != 0,
                saver: s.SystemStatusFlag == 1,
                secs_left: if s.BatteryLifeTime != u32::MAX && s.ACLineStatus != 1 {
                    Some(s.BatteryLifeTime)
                } else {
                    s.BatteryFullLifeTime
                        .checked_sub(0)
                        .filter(|v| *v != u32::MAX && s.ACLineStatus == 1)
                        .map(|f| f * (100 - s.BatteryLifePercent.min(100) as u32) / 100)
                },
            };
            self.st.on_battery(b);
        }
        self.last_battery_poll = Instant::now();
    }

    // ------------------------------------------------------------ frame

    fn place_window(&mut self) -> bool {
        let g = self.st.geo();
        let s = self.scale;
        let m = &self.monitor;
        let mon_w = m.rect.right - m.rect.left;
        let anchor = if self.st.floating() {
            let key = m.key.clone();
            let f = self
                .st
                .settings
                .appearance
                .floating_pos
                .get(&key)
                .copied()
                .unwrap_or(0.5);
            m.rect.left as f32 + mon_w as f32 * f
        } else {
            m.rect.left as f32 + mon_w as f32 / 2.0
        };
        let anchor = if qa_mode() {
            anchor - 400.0 * s
        } else {
            anchor
        };
        // never overlap a taskbar docked at the top
        let top = if m.work.top > m.rect.top {
            m.work.top
        } else {
            m.rect.top
        } as f32;
        let mut b = g.bounds;
        if self.st.was_moving {
            if let Some(h) = self.hold {
                b = b.union(&h);
            }
            self.hold = Some(b);
        } else {
            self.hold = None;
        }
        let bx = (b.x * s).floor();
        let by = (b.y * s).floor();
        let left = (anchor - CANVAS_W / 2.0 * s + bx).round() as i32;
        let r = (
            left,
            (top + by) as i32,
            (b.w * s).ceil() as i32 + 1,
            (b.h * s).ceil() as i32 + 1,
        );
        self.offset_px = (bx, by);
        self.surface.offset = (-bx, -by);
        if r != self.win_rect {
            self.win_rect = r;
            unsafe {
                let _ = SetWindowPos(
                    self.hwnd,
                    Some(HWND_TOPMOST),
                    r.0,
                    r.1,
                    r.2,
                    r.3,
                    SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_NOREDRAW,
                );
            }
            return true;
        }
        false
    }

    fn render(&mut self) {
        let t0 = Instant::now();
        let hide = self.st.isl.window_hidden;
        if std::env::var("NOTCHLY_TRACE").is_ok() {
            log(&format!(
                "render view={:?} hide={hide} h={:.1} rect={:?} vis={} fs={} pres={}",
                self.st.isl.view,
                self.st.isl.h.value,
                self.win_rect,
                self.visible,
                self.st.sup.fullscreen,
                self.st.sup.presentation
            ));
        }
        if hide {
            if self.visible {
                unsafe {
                    let _ = ShowWindow(self.hwnd, SW_HIDE);
                }
                self.visible = false;
            }
            return;
        }
        self.place_window();
        self.update_glass();
        // glass: the backdrop (taken before drawing starts) and this frame's parameters
        let backdrop = self.glass_backdrop();
        let frame = self.st.glass_frame(self.scale, backdrop);
        let Some(ctx) = self.gfx.begin(&mut self.surface) else {
            return;
        };
        let mut theme = self.st.theme.clone();
        theme.accent = self.st.accent();
        if frame.is_glass() {
            // vibrancy: secondary text brightens just enough for what is behind
            let behind = if self.st.glass.sampled {
                self.st.glass.backdrop_max
            } else {
                Color::rgb(0x404040)
            };
            let bg = render::glass::vibrancy::over(frame.tint, behind.lerp(WHITE, frame.haze));
            theme.mist = render::glass::vibrancy::levels(bg, theme.ink, theme.mist).secondary;
        }
        if let Some(mut cv) = Canvas::new(&mut self.gfx, ctx, theme) {
            cv.glass = Some(std::rc::Rc::new(frame));
            cv.cursor = self.st.input.cursor.unwrap_or((-1e4, -1e4));
            cv.pressed = self.st.input.pressed.clone();
            cv.focused = if self.st.input.keyboard_mode {
                self.st.input.focus.clone()
            } else {
                None
            };
            cv.reduced = self.st.env.reduced;
            cv.anims = self
                .st
                .anims
                .iter()
                .map(|(k, (h, p))| (k.clone(), (h.value, p.value)))
                .collect();
            ui::draw_island(&mut self.st, &mut cv);
            self.st.zones = std::mem::take(&mut cv.zones);
        }
        self.gfx.end(&mut self.surface);
        if !self.visible {
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            }
            self.visible = true;
        }
        self.st.dirty = false;
        if self.st.perf {
            self.perf_frames += 1;
            self.perf_cpu_ms += t0.elapsed().as_secs_f32() * 1000.0;
        }
        // accessibility tree snapshot
        let nodes: Vec<platform::uia::Node> = self
            .st
            .zones
            .iter()
            .filter(|z| z.focusable)
            .map(|z| {
                let (x0, y0) = self.to_screen(z.rect.x, z.rect.y);
                let (x1, y1) = self.to_screen(z.rect.right(), z.rect.bottom());
                platform::uia::Node {
                    name: z.name.clone(),
                    rect: (x0, y0, x1 - x0, y1 - y0),
                }
            })
            .collect();
        let title = if self.st.expanded {
            self.st.page.title().to_string()
        } else {
            "Notchly".to_string()
        };
        platform::uia::set_tree(self.hwnd, &title, nodes);
    }

    fn to_screen(&self, x: f32, y: f32) -> (i32, i32) {
        (
            self.win_rect.0 + (x * self.scale - self.offset_px.0) as i32,
            self.win_rect.1 + (y * self.scale - self.offset_px.1) as i32,
        )
    }

    fn to_canvas(&self, x: i32, y: i32) -> (f32, f32) {
        (
            (x as f32 + self.offset_px.0) / self.scale,
            (y as f32 + self.offset_px.1) / self.scale,
        )
    }

    // ------------------------------------------------------------ input routing

    fn handle(&mut self, m: Msg) {
        match m {
            Msg::Move(h, x, y) => {
                if self.is_settings(h) {
                    let s = self
                        .settings_win
                        .as_ref()
                        .map(|s| s.dpi / 96.0)
                        .unwrap_or(1.0);
                    if let Some(sw) = self.settings_win.as_mut() {
                        sw.on_move(&mut self.st, x as f32 / s, y as f32 / s);
                    }
                } else if h == self.hwnd {
                    let (cx, cy) = self.to_canvas(x, y);
                    if !self.st.hits(cx, cy) && !self.mouse_busy() {
                        // over the margin: let the mouse through to the app underneath
                        self.set_click_through(true);
                        self.st.on_mouse_leave();
                    } else {
                        self.st.on_mouse_move(cx, cy);
                    }
                }
            }
            Msg::Leave(h) => {
                if self.is_settings(h) {
                    if let Some(sw) = self.settings_win.as_mut() {
                        sw.cursor = None;
                        sw.dirty = true;
                    }
                } else if h == self.hwnd {
                    self.st.on_mouse_leave();
                }
            }
            Msg::Down(h, x, y, b) => {
                if self.is_settings(h) {
                    let s = self
                        .settings_win
                        .as_ref()
                        .map(|s| s.dpi / 96.0)
                        .unwrap_or(1.0);
                    if b == Button::Left {
                        if let Some(sw) = self.settings_win.as_mut() {
                            sw.on_down(&mut self.st, x as f32 / s, y as f32 / s);
                        }
                    }
                } else if h == self.hwnd {
                    self.last_click = Instant::now();
                    let (cx, cy) = self.to_canvas(x, y);
                    self.st.on_mouse_down(cx, cy, b);
                }
            }
            Msg::Up(h, x, y, b) => {
                if self.is_settings(h) {
                    let s = self
                        .settings_win
                        .as_ref()
                        .map(|s| s.dpi / 96.0)
                        .unwrap_or(1.0);
                    if b == Button::Left {
                        if let Some(sw) = self.settings_win.as_mut() {
                            sw.on_up(&mut self.st, x as f32 / s, y as f32 / s);
                        }
                    }
                } else if h == self.hwnd {
                    let (cx, cy) = self.to_canvas(x, y);
                    if self.st.on_mouse_up(cx, cy, b) == ClickOutcome::ContextMenu {
                        self.context_menu();
                    }
                    // floating style: snap horizontal position after a drag
                    if self.st.floating() && !self.st.expanded {
                        if let Some((px, _, _)) = self.st.input.press_at {
                            if (cx - px).abs() > 6.0 {
                                self.snap_float(cx - px);
                            }
                        }
                    }
                }
            }
            Msg::Wheel(h, d, horiz) => {
                let shift = unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0;
                if self.is_settings(h) {
                    if let Some(sw) = self.settings_win.as_mut() {
                        sw.on_wheel(d as f32);
                    }
                } else {
                    self.st.on_wheel(d as f32, horiz, shift);
                }
            }
            Msg::Key(h, vk) => {
                let ctrl = unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0;
                let shift = unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0;
                let hw = self.hwnd;
                let clip = move || match platform::clipboard::read(hw, &[]) {
                    Some(platform::clipboard::Clip::Text(t)) => Some(t),
                    _ => None,
                };
                if self.is_settings(h) {
                    if let Some(sw) = self.settings_win.as_mut() {
                        sw.on_key(&mut self.st, vk, ctrl, shift, &clip);
                    }
                } else {
                    self.st.on_key(vk, ctrl, shift, &clip);
                }
            }
            Msg::Char(h, c) => {
                if self.is_settings(h) {
                    if let Some(sw) = self.settings_win.as_mut() {
                        sw.on_char(c);
                    }
                } else {
                    self.st.on_char(c);
                }
            }
            Msg::Hotkey(id) => self.hotkey(id),
            Msg::Power(p) => match p {
                PowerSignal::Brightness(v) => self.st.on_brightness(v.min(100)),
                PowerSignal::Status => {
                    self.poll_battery();
                    self.apply_glass();
                }
                PowerSignal::Resume => {
                    log("resumed from sleep");
                    self.gfx.device_lost();
                    self.update_monitor();
                    self.poll_battery();
                    if let Some(tx) = &self.st.svc.audio {
                        let _ = tx.send(services::audio::AudioCmd::Rebind);
                    }
                    self.st.f.prayer_cache = None;
                    self.st.f.state.water.roll();
                }
            },
            Msg::Display | Msg::Dpi(..) => {
                if let Msg::Dpi(h, dpi, r) = &m {
                    if self.is_settings(*h) {
                        unsafe {
                            let _ = SetWindowPos(
                                *h,
                                None,
                                r.left,
                                r.top,
                                r.right - r.left,
                                r.bottom - r.top,
                                SWP_NOZORDER | SWP_NOACTIVATE,
                            );
                        }
                        let mut rc = RECT::default();
                        unsafe {
                            let _ = GetClientRect(*h, &mut rc);
                        }
                        if let Some(sw) = self.settings_win.as_mut() {
                            sw.resize(rc.right as u32, rc.bottom as u32, *dpi as f32);
                        }
                        return;
                    }
                }
                self.update_monitor();
            }
            Msg::Setting => {
                self.st.os_animations = platform::os_animations();
                self.st.high_contrast = platform::high_contrast();
                self.st.system_light = platform::system_light();
                self.st.apply_settings();
                self.update_monitor();
                self.apply_glass();
            }
            Msg::Clipboard => self.on_clipboard(),
            Msg::Tray(ev) => match ev {
                WM_LBUTTONUP | windows::Win32::UI::Shell::NIN_SELECT | 1025 => {
                    let p = self.st.default_page();
                    self.st.open(p);
                    self.st.pinned = true;
                }
                WM_RBUTTONUP | WM_CONTEXTMENU => self.tray_menu(),
                _ => {}
            },
            Msg::TaskbarCreated => {
                self.tray_shown = false;
                self.update_tray();
                self.win_rect = (0, 0, 0, 0);
            }
            Msg::Activate(h, active) => {
                if h == self.hwnd {
                    self.st.input.has_focus = active;
                    if !active
                        && self.st.expanded
                        && !self.st.pinned
                        && self.st.settings.general.open_mode == settings::OpenMode::Click
                    {
                        self.st.collapse();
                    }
                }
            }
            Msg::Close(h) => {
                if self.is_settings(h) {
                    if let Some(mut sw) = self.settings_win.take() {
                        sw.commit_focus(&mut self.st);
                        // remember its size and place on the monitor it's on
                        let mut rc = RECT::default();
                        if unsafe { GetWindowRect(sw.hwnd, &mut rc) }.is_ok()
                            && unsafe { !IsIconic(sw.hwnd).as_bool() }
                        {
                            let (cx, cy) = ((rc.left + rc.right) / 2, (rc.top + rc.bottom) / 2);
                            if let Some(m) = platform::monitors().into_iter().find(|m| {
                                cx >= m.rect.left
                                    && cx < m.rect.right
                                    && cy >= m.rect.top
                                    && cy < m.rect.bottom
                            }) {
                                settings::store::save_window_place(
                                    &m.key,
                                    [rc.left, rc.top, rc.right - rc.left, rc.bottom - rc.top],
                                );
                            }
                        }
                        unsafe {
                            let _ = DestroyWindow(sw.hwnd);
                        }
                    }
                    settings::store::save(&self.st.settings);
                    self.st.studio.flush(true);
                }
            }
            Msg::Size(h, w, hh) => {
                if self.is_settings(h) && w > 0 && hh > 0 {
                    if let Some(sw) = self.settings_win.as_mut() {
                        let dpi = sw.dpi;
                        sw.resize(w, hh, dpi);
                    }
                }
            }
            Msg::Foreground => self.check_fullscreen(),
            Msg::OpenRequest => {
                self.prev_fg = unsafe { GetForegroundWindow() };
                let p = self.st.default_page();
                self.st.open(p);
                self.st.input.keyboard_mode = true;
                self.take_focus();
            }
            Msg::OutsideClick => {
                if self.st.expanded && !self.st.pinned {
                    self.st.collapse();
                }
            }
            Msg::WidgetMoved(h) => {
                let mut r = RECT::default();
                unsafe {
                    let _ = GetWindowRect(h, &mut r);
                }
                // snap to a 20 px grid
                let (sx, sy) = ((r.left + 10) / 20 * 20, (r.top + 10) / 20 * 20);
                unsafe {
                    let _ = SetWindowPos(
                        h,
                        Some(HWND_BOTTOM),
                        sx,
                        sy,
                        0,
                        0,
                        SWP_NOSIZE | SWP_NOACTIVATE,
                    );
                }
                if let Some(w) = self.widgets.iter().find(|w| w.hwnd == h) {
                    if let Some(d) = self
                        .st
                        .settings
                        .widgets
                        .desktop
                        .iter_mut()
                        .find(|d| d.kind == w.kind)
                    {
                        d.x = sx;
                        d.y = sy;
                    }
                    self.st.save_settings_soon();
                }
            }
        }
        self.st.dirty = true;
    }

    fn snap_float(&mut self, dx_dip: f32) {
        let m = &self.monitor;
        let mon_w = (m.rect.right - m.rect.left) as f32;
        let key = m.key.clone();
        let cur = self
            .st
            .settings
            .appearance
            .floating_pos
            .get(&key)
            .copied()
            .unwrap_or(0.5);
        let target = cur + dx_dip * self.scale / mon_w;
        let snaps = [1.0 / 6.0, 0.5, 5.0 / 6.0];
        let snapped = snaps
            .iter()
            .copied()
            .min_by(|a, b| {
                (a - target)
                    .abs()
                    .partial_cmp(&(b - target).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or(0.5);
        self.st
            .settings
            .appearance
            .floating_pos
            .insert(key, snapped);
        self.st.save_settings_soon();
    }

    fn take_focus(&mut self) {
        unsafe {
            let fg = GetForegroundWindow();
            if fg != self.hwnd {
                self.prev_fg = fg;
            }
            let _ = SetForegroundWindow(self.hwnd);
            let _ = SetFocus(Some(self.hwnd));
        }
        self.st.input.has_focus = true;
    }

    fn restore_focus(&mut self) {
        unsafe {
            if !self.prev_fg.0.is_null() && IsWindow(Some(self.prev_fg)).as_bool() {
                let _ = SetForegroundWindow(self.prev_fg);
            }
        }
        self.st.input.has_focus = false;
    }

    fn hotkey(&mut self, id: i32) {
        self.st.last_input = Instant::now();
        self.prev_fg = unsafe { GetForegroundWindow() };
        match id {
            HK_TOGGLE => {
                if self.st.expanded {
                    self.st.collapse();
                } else {
                    let p = self.st.default_page();
                    self.st.open(p);
                    self.st.input.keyboard_mode = true;
                    self.take_focus();
                }
            }
            HK_PALETTE => {
                self.st.open(PageId::Ask);
                self.st.f.ask.mode = features::AskMode::Ask;
                self.st.focus_input(InputId::Ask);
                self.take_focus();
            }
            HK_OCR => self.capture(CaptureMode::Ocr),
            HK_CLIPBOARD => {
                self.st.open(PageId::Clipboard);
                self.st.focus_input(InputId::ClipSearch);
                self.take_focus();
            }
            HK_FOCUS => {
                let t = &self.st.f.state.timer;
                let cmd = if t.running() {
                    app::TimerCmd::Pause
                } else if t.active() {
                    app::TimerCmd::Resume
                } else {
                    app::TimerCmd::StartFocus
                };
                self.st.timer_cmd(cmd);
            }
            HK_TRANSLATE => {
                // copy the selection, then translate it once the clipboard updates
                let old = match platform::clipboard::read(self.hwnd, &[]) {
                    Some(platform::clipboard::Clip::Text(t)) => Some(t),
                    _ => None,
                };
                self.st.f.clip.ignore_seq = platform::clipboard::sequence().wrapping_add(1);
                platform::send_keys(&[VK_CONTROL.0, 0x43]);
                self.pending_translate = Some((Instant::now() + Duration::from_millis(180), old));
            }
            HK_SCREENSHOT => self.capture(CaptureMode::Screenshot),
            _ => {}
        }
    }

    fn quit(&mut self) {
        platform::system::keep_awake(false);
        self.st.f.save(true);
        settings::store::save(&self.st.settings);
        if self.st.settings.sync.enabled {
            sync::push(&self.st.settings.sync.folder);
        }
        platform::tray::remove(self.hwnd);
        if let Some(srv) = self.st.f.phone_server.take() {
            srv.stop();
        }
        unsafe { PostQuitMessage(0) };
    }

    fn update_mouse_hook(&mut self) {
        let want =
            self.st.expanded && self.st.settings.general.open_mode == settings::OpenMode::Click;
        match (want, self.mouse_hook) {
            (true, None) => unsafe {
                OUTSIDE_TARGET.store(self.hwnd.0 as isize, Ordering::Relaxed);
                self.mouse_hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), None, 0).ok();
            },
            (false, Some(h)) => unsafe {
                let _ = UnhookWindowsHookEx(h);
                self.mouse_hook = None;
            },
            _ => {}
        }
    }

    fn perf_report(&mut self) {
        if !self.st.perf || self.perf_last.elapsed() < Duration::from_secs(5) {
            return;
        }
        let secs = self.perf_last.elapsed().as_secs_f32();
        let mut pmc = windows::Win32::System::ProcessStatus::PROCESS_MEMORY_COUNTERS::default();
        unsafe {
            let _ = windows::Win32::System::ProcessStatus::GetProcessMemoryInfo(
                GetCurrentProcess(),
                &mut pmc,
                std::mem::size_of_val(&pmc) as u32,
            );
        }
        let (mut c, mut e, mut k, mut u) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        unsafe {
            let _ = windows::Win32::System::Threading::GetProcessTimes(
                GetCurrentProcess(),
                &mut c,
                &mut e,
                &mut k,
                &mut u,
            );
        }
        let cpu = ((k.dwHighDateTime as u64) << 32 | k.dwLowDateTime as u64)
            + ((u.dwHighDateTime as u64) << 32 | u.dwLowDateTime as u64);
        thread_local!(static LAST: RefCell<u64> = const { RefCell::new(0) });
        let prev = LAST.with(|l| std::mem::replace(&mut *l.borrow_mut(), cpu));
        let ncpu = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1) as f32;
        let pct = if prev == 0 {
            0.0
        } else {
            (cpu - prev) as f32 / 10_000.0 / (secs * 1000.0) * 100.0 / ncpu
        };
        let avg = if self.perf_frames > 0 {
            self.perf_cpu_ms / self.perf_frames as f32
        } else {
            0.0
        };
        println!("[perf] frames {:>4} ({:.0} fps)  avg cpu/frame {:.2} ms  process CPU {:.2}%  RAM {:.1} MB", self.perf_frames, self.perf_frames as f32 / secs, avg, pct, pmc.WorkingSetSize as f32 / 1048576.0);
        let frames = self.capture.as_ref().map(|c| c.frames()).unwrap_or(0);
        let backdrop = frames.saturating_sub(self.perf_backdrop);
        self.perf_backdrop = frames;
        let gpu = self.gfx.gpu_ms();
        println!(
            "[perf] glass {}{}  backdrop {:.1}/s  capture {:?}  gpu/frame {}",
            self.glass_tier.name(),
            self.st
                .glass
                .reason
                .map(|r| format!(" ({})", r.message()))
                .unwrap_or_default(),
            backdrop as f32 / secs,
            self.capture.as_ref().map(|c| c.status()),
            gpu.map(|g| format!("{g:.2} ms"))
                .unwrap_or_else(|| "n/a".into())
        );
        self.perf_frames = 0;
        self.perf_cpu_ms = 0.0;
        self.perf_last = Instant::now();
    }

    /// Main loop: smooth (vblank) while springs move, ~30 fps for live visuals,
    /// otherwise blocks until a message or the next deadline.
    fn run(&mut self) {
        let mut animating = true;
        loop {
            let now = Instant::now();
            let wait_ms = if animating || self.st.dirty {
                0
            } else {
                let mut deadline = self.st.next_deadline();
                if let Some(iv) = self.st.live_interval() {
                    let t = now + iv;
                    deadline = Some(deadline.map_or(t, |d| d.min(t)));
                }
                if !self.widgets.is_empty() {
                    let t = self.last_widget_draw + Duration::from_millis(1000);
                    deadline = Some(deadline.map_or(t, |d| d.min(t)));
                }
                for p in [
                    self.pending_paste,
                    self.pending_translate.as_ref().map(|p| p.0),
                ]
                .into_iter()
                .flatten()
                {
                    deadline = Some(deadline.map_or(p, |d| d.min(p)));
                }
                if self.st.perf {
                    let t = self.perf_last + Duration::from_secs(5);
                    deadline = Some(deadline.map_or(t, |d| d.min(t)));
                }
                // QA frame dump: keep the loop turning until it has fired
                if cfg!(debug_assertions)
                    && qa_mode()
                    && std::env::var("NOTCHLY_QA_DUMP").is_ok()
                    && self.st.started.elapsed() < Duration::from_secs(30)
                {
                    let t = now + Duration::from_millis(200);
                    deadline = Some(deadline.map_or(t, |d| d.min(t)));
                }
                match deadline {
                    Some(d) => d
                        .saturating_duration_since(now)
                        .as_millis()
                        .min(u32::MAX as u128 - 1) as u32,
                    None => INFINITE,
                }
            };
            if wait_ms > 0 {
                unsafe {
                    MsgWaitForMultipleObjectsEx(None, wait_ms, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
                }
            }
            let mut msg = MSG::default();
            while unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE) }.as_bool() {
                if msg.message == WM_QUIT {
                    return;
                }
                unsafe {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            while let Ok(e) = self.rx.try_recv() {
                self.st.on_event(e);
            }
            #[cfg(debug_assertions)]
            if qa_mode() {
                if let Ok(p) = std::env::var("NOTCHLY_QA_DUMP") {
                    let after: u64 = std::env::var("NOTCHLY_QA_DUMP_AFTER")
                        .ok()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(5000);
                    // NOTCHLY_QA_DUMP_EVERY=<ms> and _COUNT=<n>: a filmstrip, p_0.png, p_1.png ...
                    let every: u64 = std::env::var("NOTCHLY_QA_DUMP_EVERY")
                        .ok()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0);
                    let count: u32 = std::env::var("NOTCHLY_QA_DUMP_COUNT")
                        .ok()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(1);
                    thread_local!(static DONE: std::cell::Cell<u32> = const { std::cell::Cell::new(0) });
                    let k = DONE.with(|d| d.get());
                    if k < count
                        && self.st.started.elapsed()
                            >= Duration::from_millis(after + every * k as u64)
                    {
                        DONE.with(|d| d.set(k + 1));
                        let path = if count > 1 {
                            p.replace(".png", &format!("_{k}.png"))
                        } else {
                            p
                        };
                        // NOTCHLY_QA_DUMP_TARGET=settings dumps the Settings window instead
                        if std::env::var("NOTCHLY_QA_DUMP_TARGET").as_deref() == Ok("settings") {
                            QA_SETTINGS_DUMP.with(|d| d.replace(Some(path)));
                        } else {
                            self.gfx.dump = Some(path.into());
                        }
                        self.st.dirty = true;
                    }
                }
            }
            self.poll_click_through();
            // a new backdrop from the glass capture worker
            if let Some(c) = &self.capture {
                let n = c.frames();
                if n != self.capture_seen {
                    self.capture_seen = n;
                    self.st.dirty = true;
                }
            }
            loop {
                let m = QUEUE.with(|q| q.borrow_mut().pop_front());
                match m {
                    Some(m) => self.handle(m),
                    None => break,
                }
            }
            self.drain_drops();
            for i in platform::uia::take_invoked() {
                if let Some(a) = self
                    .st
                    .zones
                    .iter()
                    .filter(|z| z.focusable)
                    .nth(i)
                    .map(|z| z.action.clone())
                {
                    self.st.dispatch(a);
                }
            }
            let now = Instant::now();
            if self.pending_paste.is_some_and(|p| now >= p) {
                self.pending_paste = None;
                platform::send_keys(&[VK_CONTROL.0, 0x56]);
            }
            if self.pending_translate.as_ref().is_some_and(|p| now >= p.0) {
                let (_, old) = self.pending_translate.take().unwrap_or((now, None));
                if let Some(platform::clipboard::Clip::Text(sel)) =
                    platform::clipboard::read(self.hwnd, &[])
                {
                    if let Some(o) = old {
                        self.st.f.clip.ignore_seq = platform::clipboard::sequence().wrapping_add(1);
                        platform::clipboard::set_text(self.hwnd, &o);
                    }
                    self.st.f.ask.mode = features::AskMode::Translate;
                    self.st.input.text(InputId::Ask).set(&sel);
                    self.st.input.text_focus = None;
                    self.st.ask_submit();
                }
            }
            self.st.check_hover(now);
            self.st.tick(now);
            if self.last_battery_poll.elapsed() > Duration::from_secs(60)
                && self.st.sys.battery.present
            {
                self.poll_battery();
            }
            self.process_requests();
            self.update_mouse_hook();
            let before = self.st.last_frame;
            animating = self.st.frame(now);
            let live_due = self
                .st
                .live_interval()
                .is_some_and(|iv| now.duration_since(before) >= iv);
            if animating || self.st.dirty || live_due {
                self.render();
            } else {
                self.st.last_frame = now;
            }
            let dt = now.duration_since(before).as_secs_f32();
            self.render_settings(dt.min(0.05));
            self.render_widgets();
            self.perf_report();
            // trim the working set once things have been quiet for a few seconds
            let quiet = !animating && !self.st.expanded && self.st.pop.current.is_none();
            if quiet {
                let since = *self.quiet_since.get_or_insert(now);
                if !self.trimmed && now.duration_since(since) > Duration::from_secs(4) {
                    self.trimmed = true;
                    self.gfx.trim();
                    unsafe {
                        let _ = windows::Win32::System::Threading::SetProcessWorkingSetSize(
                            GetCurrentProcess(),
                            usize::MAX,
                            usize::MAX,
                        );
                    }
                }
            } else {
                self.quiet_since = None;
                self.trimmed = false;
            }
            if self.settings_win.as_ref().is_some_and(|s| s.dirty) {
                animating = true;
            }
            if animating {
                // pace to the compositor
                unsafe {
                    let _ = DwmFlush();
                }
            }
        }
    }
}

fn qa_mode() -> bool {
    cfg!(debug_assertions) && std::env::var("NOTCHLY_QA").is_ok_and(|v| v == "1")
}

fn single_instance() -> bool {
    unsafe {
        // debug-only QA mode runs beside a normal instance (separate lock, shifted notch)
        let qa = qa_mode();
        let _m = CreateMutexW(
            None,
            true,
            if qa {
                w!("Local\\Notchly.QA")
            } else {
                w!("Local\\Notchly.SingleInstance")
            },
        );
        if GetLastError() == ERROR_ALREADY_EXISTS && !qa {
            if let Ok(h) = FindWindowW(w!("NotchlyIsland"), None) {
                let _ = PostMessageW(Some(h), WM_OPEN_REQUEST, WPARAM(0), LPARAM(0));
            }
            return false;
        }
        std::mem::forget(_m);
    }
    true
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        unsafe {
            let _ = windows::Win32::System::Console::AttachConsole(
                windows::Win32::System::Console::ATTACH_PARENT_PROCESS,
            );
        }
    }
    if args.first().map(|a| a.as_str()) == Some("live") {
        std::process::exit(services::live_api::cli(&args[1..]));
    }
    // troubleshooting: is the speaker level meter delivering? (prints the peak over 2 s)
    if args.iter().any(|a| a == "--meter-check") {
        let level = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let active = Arc::new(AtomicBool::new(true));
        services::audio::start_meter(level.clone(), active);
        let mut max = 0f32;
        for _ in 0..40 {
            std::thread::sleep(Duration::from_millis(50));
            max = max.max(f32::from_bits(level.load(Ordering::Relaxed)));
        }
        services::com_mta();
        println!(
            "Meter: {}
Speaker peak over 2 s: {max:.3}",
            match services::audio::meter_channels() {
                Some(n) => format!("open ({n} channels)"),
                None => "unavailable".into(),
            }
        );
        std::process::exit(0);
    }
    // troubleshooting: run the lyrics search for a title / artist / duration
    if let Some(i) = args.iter().position(|a| a == "--lyrics-check") {
        services::com_mta();
        let title = args.get(i + 1).cloned().unwrap_or_default();
        let artist = args.get(i + 2).cloned().unwrap_or_default();
        let dur = args.get(i + 3).and_then(|d| d.parse().ok()).unwrap_or(0);
        let (cands, q) = services::lyrics::candidates(&services::lyrics::LyricsRequest {
            key: 0,
            title: title.clone(),
            artist: artist.clone(),
            album: String::new(),
            duration_s: dur,
            raw_title: title.clone(),
            raw_artist: artist.clone(),
            pick: 0,
            force: true,
        });
        println!(
            "Candidates: {cands:?}
Query: {q}"
        );
        let (r, meta) = services::lyrics::check(&title, &artist, dur);
        let what = match &r {
            services::lyrics::LyricsResult::Synced(l) => format!("synced, {} lines", l.len()),
            services::lyrics::LyricsResult::Plain(l) => format!("plain, {} lines", l.len()),
            other => format!("{other:?}"),
        };
        println!(
            "Result: {what}
Source: {}
Matched: {}
Matches: {}",
            meta.source, meta.matched, meta.alts
        );
        std::process::exit(0);
    }
    // troubleshooting: fetch calendars once and print what came back (counts, no titles)
    if args.iter().any(|a| a == "--privacy-check") {
        // what the In use tile would show right now
        let (mic, cam) = services::privacy::sources();
        let ok = |b: bool| {
            if b {
                "live"
            } else {
                "consent store (fallback)"
            }
        };
        println!(
            "microphone: {}
camera: {}
location: consent store",
            ok(mic),
            ok(cam)
        );
        let uses = services::privacy::scan();
        if uses.is_empty() {
            println!("Nothing is using the camera, microphone or location.");
        }
        for u in uses {
            println!("{:<10} {}  ({})", u.cap.label(), u.app, u.key);
        }
        return;
    }
    if args.iter().any(|a| a == "--calendar-check") {
        services::com_mta();
        let s = settings::store::load();
        use services::calendar::{has_builtin, is_connected, GOOGLE, MICROSOFT};
        println!(
            "Google: {} (built-in client: {})
Outlook: {} (built-in client: {})
ICS links: {}",
            if is_connected(&GOOGLE) {
                "connected"
            } else {
                "not connected"
            },
            has_builtin(&GOOGLE),
            if is_connected(&MICROSOFT) {
                "connected"
            } else {
                "not connected"
            },
            has_builtin(&MICROSOFT),
            s.calendar.ics_urls.len()
        );
        let (events, errors) = services::calendar::fetch_all(
            &s.calendar.ics_urls,
            &s.calendar.google_client_id,
            &s.calendar.microsoft_client_id,
        );
        println!("Events in the next 31 days: {}", events.len());
        for e in &errors {
            println!("Error: {e}");
        }
        std::process::exit(if errors.is_empty() { 0 } else { 1 });
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("Notchly {}\n  notchly                 start the app\n  notchly live ...        Live Activities CLI (notchly live --help)\n  notchly --demo          cycle through states for visual QA\n  notchly --perf          print frame time, CPU and RAM every 5 s\n  notchly --settings      open the settings window", env!("CARGO_PKG_VERSION"));
        return;
    }
    util::install_crash_hook();
    if !single_instance() {
        return;
    }
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let _ = OleInitialize(None);
    }
    log(&format!("Notchly {} starting", env!("CARGO_PKG_VERSION")));
    let first = settings::store::load();
    if first.sync.enabled {
        sync::pull(&first.sync.folder);
    }
    let mut settings = settings::store::load();
    if settings.general.start_with_windows {
        platform::set_autostart(true);
    }
    settings.version = settings::SETTINGS_VERSION;
    let demo = args.iter().any(|a| a == "--demo");
    let perf = args.iter().any(|a| a == "--perf");
    let onboarded = settings.general.onboarded;
    let mut app = match App::new(settings, demo, perf) {
        Ok(a) => a,
        Err(e) => {
            log(&format!("fatal: could not create the window: {e}"));
            return;
        }
    };
    if util::DATA_RESET.load(Ordering::Relaxed) {
        app.st.popup(
            Popup::new(
                PopKind::Info,
                Icon::Settings,
                "Settings were reset",
                "A data file was damaged",
            )
            .duration_ms(4000),
        );
    }
    if !onboarded && !demo {
        app.st.open(PageId::Onboarding);
        app.st.pinned = true;
    }
    // QA: open and pin a page directly, e.g. `notchly --page sysmon`
    if let Some(name) = args
        .iter()
        .position(|a| a == "--page")
        .and_then(|i| args.get(i + 1))
    {
        let page = match name.as_str() {
            "music" => Some(PageId::Music),
            "lyrics" => Some(PageId::Lyrics),
            "timer" => Some(PageId::Timer),
            "water" => Some(PageId::Water),
            "todo" => Some(PageId::Todo),
            "shelf" => Some(PageId::Shelf),
            "clipboard" => Some(PageId::Clipboard),
            "prayer" => Some(PageId::Prayer),
            "weather" => Some(PageId::Weather),
            "calendar" => Some(PageId::Calendar),
            "sysmon" => Some(PageId::SysMon),
            "control" => Some(PageId::Control),
            "diagnostics" => Some(PageId::Diagnostics),
            "notes" => Some(PageId::Notes),
            "widgets" => Some(PageId::Widgets),
            "ask" => Some(PageId::Ask),
            "home" | "dashboard" => Some(PageId::Dashboard),
            // the quick tour; NOTCHLY_QA_TOUR_STEP=<0..3> picks the card
            "tour" => {
                let k: u8 = std::env::var("NOTCHLY_QA_TOUR_STEP")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                app.st.onboarding_step = ui::pages::TOUR_START + k.min(3);
                Some(PageId::Onboarding)
            }
            _ => None,
        };
        if let Some(p) = page {
            app.st.open(p);
            app.st.pinned = true;
            if args.iter().any(|a| a == "--dock") {
                app.st.isl.dock.snap(1.0);
                app.st.isl.dock_rect = render::Rect::new(0.0, 0.0, CANVAS_W, CANVAS_H);
                app.st.input.cursor = Some((-1000.0, -1000.0));
            }
        }
    }
    // QA: open and pin a page directly, e.g. `notchly --page sysmon --dock`
    if let Some(name) = args
        .iter()
        .position(|a| a == "--page")
        .and_then(|i| args.get(i + 1))
    {
        let page = match name.as_str() {
            "music" => Some(PageId::Music),
            "lyrics" => Some(PageId::Lyrics),
            "timer" => Some(PageId::Timer),
            "water" => Some(PageId::Water),
            "todo" => Some(PageId::Todo),
            "shelf" => Some(PageId::Shelf),
            "clipboard" => Some(PageId::Clipboard),
            "prayer" => Some(PageId::Prayer),
            "weather" => Some(PageId::Weather),
            "calendar" => Some(PageId::Calendar),
            "sysmon" => Some(PageId::SysMon),
            "control" => Some(PageId::Control),
            "diagnostics" => Some(PageId::Diagnostics),
            "notes" => Some(PageId::Notes),
            "widgets" => Some(PageId::Widgets),
            "ask" => Some(PageId::Ask),
            "home" | "dashboard" => Some(PageId::Dashboard),
            _ => None,
        };
        if let Some(p) = page {
            app.st.open(p);
            app.st.pinned = true;
            app.st.qa_dock = args.iter().any(|a| a == "--dock");
            if qa_mode() && args.iter().any(|a| a == "--qa-picker") {
                app.st.dispatch(app::Action::TodoPicker);
                app.st.dispatch(app::Action::TodoPickQuick(1));
                app.st.dispatch(app::Action::TodoPickTime(18 * 60));
            }
            if qa_mode() && args.iter().any(|a| a == "--qa-drop") {
                // QA: a file lands on the shelf (to watch the soft landing)
                let ids: Vec<u64> = app
                    .st
                    .f
                    .shelf
                    .add_text("Soft landing")
                    .into_iter()
                    .collect();
                app.after_drop(ids);
                app.st.pinned = true;
            }
            if qa_mode() && args.iter().any(|a| a == "--qa-trash") {
                app.st.qa_trash = Some((Instant::now() + Duration::from_millis(1500), 0));
            }
        }
    }
    if args.iter().any(|a| a == "--settings") {
        app.open_settings();
        // QA (debug builds): NOTCHLY_QA_SECTION=<n> opens a section, e.g. 1 = Appearance
        if cfg!(debug_assertions) && qa_mode() {
            if let Some(n) = std::env::var("NOTCHLY_QA_SECTION")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
            {
                if let Some(sw) = app.settings_win.as_mut() {
                    sw.section = n;
                    sw.dirty = true;
                }
            }
            if let Ok(q) = std::env::var("NOTCHLY_QA_SEARCH") {
                if let Some(sw) = app.settings_win.as_mut() {
                    sw.qa_search(&q);
                }
            }
        }
    }
    app.run();
    log("Notchly exiting");
}
