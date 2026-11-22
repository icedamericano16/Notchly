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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::{Duration, Instant};

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::Graphics::Gdi::{CreateRoundRectRgn, SetWindowRgn, HRGN};
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
use render::icons::Icon;
use render::{Canvas, Gfx, Surface};
use services::{Bus, Event, WM_APP_EVENT};
use settings::window::SettingsWin;
use theme::tokens::*;
use util::{log, wide};

const WM_OPEN_REQUEST: u32 = WM_APP + 3;
const WM_OUTSIDE_CLICK: u32 = WM_APP + 4;

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
                    let s = unsafe { &*(lp.0 as *const POWERBROADCAST_SETTING) };
                    if s.PowerSetting == GUID_BRIGHTNESS || s.PowerSetting == GUID_BRIGHTNESS_POLICY
                    {
                        let v = unsafe { *(s.Data.as_ptr() as *const u32) };
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
        WM_DPICHANGED => {
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

static mut OUTSIDE_TARGET: isize = 0;

unsafe extern "system" fn mouse_hook(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code >= 0 && (wp.0 as u32 == WM_LBUTTONDOWN || wp.0 as u32 == WM_RBUTTONDOWN) {
        let m = unsafe { &*(lp.0 as *const MSLLHOOKSTRUCT) };
        let target = HWND(unsafe { OUTSIDE_TARGET } as _);
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
    glass_on: bool,
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
        let ex = WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOREDIRECTIONBITMAP | WS_EX_NOACTIVATE;
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
            glass_on: false,
            last_widget_draw: Instant::now(),
            perf_last: Instant::now(),
            perf_frames: 0,
            perf_cpu_ms: 0.0,
            last_battery_poll: Instant::now(),
            quiet_since: None,
            trimmed: false,
        };
        app.gfx.force_warp = platform::remote_session();
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
        let dir = util::sub_dir("logs");
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
        let mut p = Popup::new(
            PopKind::Info,
            Icon::Info,
            "Notchly restarted after a problem",
            "Sorry about that. The report stays on this PC.",
        )
        .action("Copy report", app::Action::CopyText(report))
        .duration_ms(10_000);
        p = match build_info::SUPPORT_URL {
            Some(u) => p.action("Report", app::Action::OpenUrl(u.to_string())),
            None => p.action(
                "Open folder",
                app::Action::OpenUrl(dir.to_string_lossy().to_string()),
            ),
        };
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
                sw.message = Some((msg, Instant::now()));
            }
        }
    }

    fn update_tray(&mut self) {
        let show = self.st.settings.general.show_tray;
        let tip = self.tray_tip();
        if show && !self.tray_shown {
            platform::tray::add(self.hwnd, &tip);
            self.tray_shown = true;
        } else if !show && self.tray_shown {
            platform::tray::remove(self.hwnd);
            self.tray_shown = false;
        } else if show {
            platform::tray::update(self.hwnd, &tip, self.st.f.update_url.is_some());
        }
    }

    fn tray_tip(&self) -> String {
        let s = &self.st;
        if s.media.snap.playing {
            format!("Notchly · {} — {}", s.media.snap.title, s.media.snap.artist)
        } else if s.f.state.timer.running() {
            format!(
                "Notchly · {} {}",
                s.f.state.timer.phase.label(),
                util::fmt_mmss(s.f.state.timer.remaining_ms() / 1000)
            )
        } else {
            "Notchly".into()
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

    fn check_fullscreen(&mut self) {
        FG_HOOK_FLAG.store(false, Ordering::Relaxed);
        let mut own = vec![self.hwnd];
        if let Some(s) = &self.settings_win {
            own.push(s.hwnd);
        }
        let (fs, exe) = platform::foreground_fullscreen(self.monitor.handle, &own);
        let q = &self.st.settings.quiet;
        let allowed = q
            .fullscreen_allow
            .iter()
            .any(|a| a.eq_ignore_ascii_case(&exe));
        let denied = !exe.is_empty()
            && q.fullscreen_deny
                .iter()
                .any(|a| a.eq_ignore_ascii_case(&exe));
        let busy = platform::shell_busy();
        if std::env::var("NOTCHLY_TRACE").is_ok() {
            let fg = unsafe { GetForegroundWindow() };
            let mut r = RECT::default();
            unsafe {
                let _ = GetWindowRect(fg, &mut r);
            }
            let q = unsafe { windows::Win32::UI::Shell::SHQueryUserNotificationState() };
            log(&format!(
                "fs-check fg={:?} class={} exe={} rect={:?} mon={:?} fs={fs} busy={busy} quns={:?}",
                fg.0,
                platform::window_class(fg),
                exe,
                (r.left, r.top, r.right, r.bottom),
                (
                    self.monitor.rect.left,
                    self.monitor.rect.top,
                    self.monitor.rect.right,
                    self.monitor.rect.bottom
                ),
                q.map(|v| v.0)
            ));
        }
        let hidden = (fs && !allowed) || denied;
        if hidden != self.st.sup.fullscreen || busy != self.st.sup.presentation {
            self.st.sup.fullscreen = hidden;
            self.st.sup.presentation = busy && fs;
            self.st.sup.fullscreen_app = exe;
            self.st.dirty = true;
        }
        // re-assert topmost on foreground change; track the foreground window's moves
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER,
            );
            if let Some(h) = self.loc_hook.take() {
                let _ = UnhookWinEvent(h);
            }
            let fg = GetForegroundWindow();
            if !fg.0.is_null() && fg != self.hwnd {
                let mut pid = 0;
                let tid = GetWindowThreadProcessId(fg, Some(&mut pid));
                let h = SetWinEventHook(
                    EVENT_OBJECT_LOCATIONCHANGE,
                    EVENT_OBJECT_LOCATIONCHANGE,
                    None,
                    Some(win_event),
                    pid,
                    tid,
                    WINEVENT_OUTOFCONTEXT,
                );
                if !h.is_invalid() {
                    self.loc_hook = Some(h);
                }
            }
        }
        self.st.sup.sharing = self.st.f.meeting.is_some();
    }

    fn apply_glass(&mut self) {
        let a = &self.st.settings.appearance;
        let want = a.material == settings::Material::Glass
            && platform::is_windows_11()
            && platform::transparency_enabled()
            && !self.st.sys.battery.saver
            && !platform::remote_session();
        if want == self.glass_on {
            return;
        }
        self.glass_on = want;
        unsafe {
            let v: i32 = if want { 3 } else { 1 }; // DWMSBT_TRANSIENTWINDOW / DWMSBT_NONE
            let _ =
                DwmSetWindowAttribute(self.hwnd, DWMWA_SYSTEMBACKDROP_TYPE, &v as *const _ as _, 4);
            let m = if want {
                MARGINS {
                    cxLeftWidth: -1,
                    cxRightWidth: -1,
                    cyTopHeight: -1,
                    cyBottomHeight: -1,
                }
            } else {
                MARGINS::default()
            };
            let _ = DwmExtendFrameIntoClientArea(self.hwnd, &m);
            if !want {
                SetWindowRgn(self.hwnd, Some(HRGN::default()), true);
            }
        }
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
            anchor - 560.0 * s
        } else {
            anchor
        };
        // never overlap a taskbar docked at the top
        let top = if m.work.top > m.rect.top {
            m.work.top
        } else {
            m.rect.top
        } as f32;
        let b = g.bounds;
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
            if self.glass_on {
                // shape the DWM backdrop to the pill body
                let mx = ((g.main.x - b.x) * s) as i32;
                let my = ((g.main.y - b.y) * s) as i32;
                let rad = (g.radius * s * 2.0) as i32;
                unsafe {
                    let rgn = CreateRoundRectRgn(
                        mx,
                        my - rad,
                        mx + (g.main.w * s) as i32 + 1,
                        my + (g.main.h * s) as i32 + 1,
                        rad,
                        rad,
                    );
                    SetWindowRgn(self.hwnd, Some(rgn), false);
                }
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
        let Some(ctx) = self.gfx.begin(&mut self.surface) else {
            return;
        };
        let mut theme = self.st.theme.clone();
        theme.accent = self.st.accent();
        if let Some(mut cv) = Canvas::new(&mut self.gfx, ctx, theme) {
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

    fn render_settings(&mut self, dt: f32) {
        let Some(sw) = self.settings_win.as_mut() else {
            return;
        };
        let moving = sw.step(dt, &self.st);
        if sw.dirty || moving {
            sw.draw(&mut self.st, &mut self.gfx);
        }
    }

    fn render_widgets(&mut self) {
        if self.widgets.is_empty() || self.last_widget_draw.elapsed() < Duration::from_millis(1000)
        {
            return;
        }
        self.last_widget_draw = Instant::now();
        for w in self.widgets.iter_mut() {
            let Some(ctx) = self.gfx.begin(&mut w.surface) else {
                continue;
            };
            let mut theme = self.st.theme.clone();
            theme.accent = self.st.accent();
            if let Some(mut cv) = Canvas::new(&mut self.gfx, ctx, theme) {
                let (ww, hh) = ui::widgets::size(&w.kind);
                ui::widgets::draw_desktop(
                    &mut self.st,
                    &mut cv,
                    &w.kind,
                    render::Rect::new(1.0, 1.0, ww - 2.0, hh - 2.0),
                );
            }
            self.gfx.end(&mut w.surface);
        }
    }

    fn sync_widgets(&mut self) {
        let want: Vec<settings::DesktopWidget> = self.st.settings.widgets.desktop.clone();
        self.widgets.retain(|w| {
            let keep = want.iter().any(|d| d.kind == w.kind);
            if !keep {
                unsafe {
                    let _ = DestroyWindow(w.hwnd);
                }
            }
            keep
        });
        for d in want {
            if self.widgets.iter().any(|w| w.kind == d.kind) {
                continue;
            }
            let (ww, hh) = ui::widgets::size(&d.kind);
            let s = self.monitor.dpi as f32 / 96.0;
            let (pw, ph) = ((ww * s) as i32, (hh * s) as i32);
            let inst = unsafe { GetModuleHandleW(None).unwrap_or_default() };
            let Ok(h) = (unsafe {
                CreateWindowExW(
                    WS_EX_TOOLWINDOW | WS_EX_NOREDIRECTIONBITMAP | WS_EX_NOACTIVATE,
                    w!("NotchlyWidget"),
                    w!("Notchly widget"),
                    WS_POPUP,
                    d.x,
                    d.y,
                    pw,
                    ph,
                    None,
                    None,
                    Some(inst.into()),
                    None,
                )
            }) else {
                continue;
            };
            unsafe {
                let _ = ShowWindow(h, SW_SHOWNOACTIVATE);
                let _ = SetWindowPos(
                    h,
                    Some(HWND_BOTTOM),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
            self.widgets.push(Widget {
                hwnd: h,
                kind: d.kind.clone(),
                surface: Surface::new(h, pw as u32, ph as u32, 96.0 * s),
            });
        }
        self.last_widget_draw = Instant::now() - Duration::from_secs(5);
    }

    fn open_settings(&mut self) {
        if let Some(sw) = &self.settings_win {
            unsafe {
                let _ = ShowWindow(sw.hwnd, SW_RESTORE);
                let _ = SetForegroundWindow(sw.hwnd);
            }
            return;
        }
        let dpi = self.monitor.dpi as f32;
        let s = dpi / 96.0;
        let (w, h) = ((820.0 * s) as i32, (600.0 * s) as i32);
        let m = &self.monitor;
        let x = m.work.left + ((m.work.right - m.work.left) - w) / 2;
        let y = m.work.top + ((m.work.bottom - m.work.top) - h) / 2;
        let inst = unsafe { GetModuleHandleW(None).unwrap_or_default() };
        let Ok(hwnd) = (unsafe {
            CreateWindowExW(
                WS_EX_NOREDIRECTIONBITMAP | WS_EX_APPWINDOW,
                w!("NotchlySettings"),
                w!("Notchly Settings"),
                WS_OVERLAPPEDWINDOW,
                x,
                y,
                w,
                h,
                None,
                None,
                Some(inst.into()),
                None,
            )
        }) else {
            return;
        };
        unsafe {
            let dark: i32 = 1;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &dark as *const _ as _,
                4,
            );
            if platform::is_windows_11() {
                let mica: i32 = 2; // DWMSBT_MAINWINDOW
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_SYSTEMBACKDROP_TYPE,
                    &mica as *const _ as _,
                    4,
                );
                let mg = MARGINS {
                    cxLeftWidth: -1,
                    cxRightWidth: -1,
                    cyTopHeight: -1,
                    cyBottomHeight: -1,
                };
                let _ = DwmExtendFrameIntoClientArea(hwnd, &mg);
            }
            let mut rc = RECT::default();
            let _ = GetClientRect(hwnd, &mut rc);
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
            self.settings_win = Some(SettingsWin::new(
                hwnd,
                (rc.right - rc.left).max(1) as u32,
                (rc.bottom - rc.top).max(1) as u32,
                dpi,
            ));
        }
    }

    // ------------------------------------------------------------ input routing

    fn is_settings(&self, h: HWND) -> bool {
        self.settings_win.as_ref().is_some_and(|s| s.hwnd == h)
    }

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
                    self.st.on_mouse_move(cx, cy);
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
                        unsafe {
                            let _ = DestroyWindow(sw.hwnd);
                        }
                    }
                    settings::store::save(&self.st.settings);
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

    fn capture(&mut self, mode: CaptureMode) {
        let was_visible = self.visible;
        if was_visible {
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
            }
        }
        let hint = match mode {
            CaptureMode::Ocr => {
                "Drag to select text · click a window · Enter for full screen · Esc to cancel"
            }
            CaptureMode::Screenshot => {
                "Drag to select · click a window · Enter for full screen · Esc to cancel"
            }
        };
        let px = platform::capture::capture(hint);
        if was_visible {
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            }
        }
        let Some(px) = px else { return };
        let px = Arc::new(px);
        match mode {
            CaptureMode::Ocr => {
                self.st.f.ocr.busy = true;
                self.st.f.ocr.image = Some(px.clone());
                services::ocr::recognize(
                    self.bus.clone(),
                    px,
                    self.st.settings.productivity.ocr_language.clone(),
                );
                self.st.upsert_activity(
                    "ocr",
                    app::activity::ActKind::Recording,
                    Some(Instant::now() + Duration::from_secs(20)),
                );
            }
            CaptureMode::Screenshot => {
                self.st.f.annotate = Default::default();
                self.st.f.annotate.image = Some(px);
                self.st.open(PageId::Annotate);
                self.st.pinned = true;
            }
        }
    }

    fn context_menu(&mut self) {
        unsafe {
            let Ok(m) = CreatePopupMenu() else { return };
            let add = |id: usize, t: &str| {
                let w = wide(t);
                let _ = AppendMenuW(m, MF_STRING, id, PCWSTR(w.as_ptr()));
            };
            if let Some(a) = self.st.act.main() {
                let pinned = self.st.act.pinned.as_deref() == Some(a.key.as_str());
                add(
                    10,
                    if pinned {
                        "Unpin activity"
                    } else {
                        "Pin this activity"
                    },
                );
                if a.kind == app::activity::ActKind::Music {
                    add(11, "Copy “Title — Artist”");
                    add(12, "Search lyrics on the web");
                }
                let _ = AppendMenuW(m, MF_SEPARATOR, 0, PCWSTR::null());
            }
            add(1, if self.st.expanded { "Close" } else { "Open" });
            add(
                2,
                if self.st.pinned {
                    "Unpin notch"
                } else {
                    "Pin notch open"
                },
            );
            add(3, "Settings…");
            let _ = AppendMenuW(m, MF_SEPARATOR, 0, PCWSTR::null());
            add(4, "Quit Notchly");
            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);
            let _ = SetForegroundWindow(self.hwnd);
            let r = TrackPopupMenu(
                m,
                TPM_RETURNCMD | TPM_RIGHTBUTTON,
                pt.x,
                pt.y,
                Some(0),
                self.hwnd,
                None,
            )
            .0;
            let _ = DestroyMenu(m);
            match r {
                1 => {
                    if self.st.expanded {
                        self.st.collapse()
                    } else {
                        let p = self.st.default_page();
                        self.st.open(p)
                    }
                }
                2 => self.st.pinned = !self.st.pinned,
                3 => self.open_settings(),
                4 => self.quit(),
                10 => {
                    if let Some(k) = self.st.act.main().map(|a| a.key.clone()) {
                        self.st.act.toggle_pin(&k);
                    }
                }
                11 => {
                    let t = format!(
                        "{} — {}",
                        self.st.media.snap.title, self.st.media.snap.artist
                    );
                    platform::clipboard::set_text(self.hwnd, &t);
                }
                12 => {
                    let q = format!(
                        "{} {} lyrics",
                        self.st.media.snap.title, self.st.media.snap.artist
                    );
                    platform::shell_open(&format!(
                        "https://www.bing.com/search?q={}",
                        services::http::enc(&q)
                    ));
                }
                _ => {}
            }
        }
    }

    fn tray_menu(&mut self) {
        let mons: Vec<String> = platform::monitors()
            .iter()
            .enumerate()
            .map(|(i, m)| {
                format!(
                    "Display {} ({}×{})",
                    i + 1,
                    m.rect.right - m.rect.left,
                    m.rect.bottom - m.rect.top
                )
            })
            .collect();
        let r = platform::tray::menu(
            self.hwnd,
            self.st.popups_paused(),
            self.st.settings.quiet.focus,
            &mons,
            self.st.settings.general.monitor,
        );
        use platform::tray::cmd;
        match r {
            cmd::OPEN => {
                let p = self.st.default_page();
                self.st.open(p);
                self.st.pinned = true;
            }
            cmd::PAUSE_1H => {
                self.st.sup.paused_until = Some(Instant::now() + Duration::from_secs(3600))
            }
            cmd::PAUSE_TOMORROW => {
                let now = util::local_now();
                let secs = (24 * 3600 - now.secs_of_day() as u64) + 7 * 3600;
                self.st.sup.paused_until = Some(Instant::now() + Duration::from_secs(secs));
            }
            cmd::RESUME => self.st.sup.paused_until = None,
            cmd::QUIET => {
                self.st.settings.quiet.focus = !self.st.settings.quiet.focus;
                self.st.save_settings_soon();
            }
            cmd::SETTINGS => self.open_settings(),
            cmd::UPDATES => {
                if let Some(p) = self.st.f.update_url.clone() {
                    updates::install(&p);
                    self.quit();
                } else {
                    updates::check(self.bus.clone(), true);
                }
            }
            cmd::QUIT => self.quit(),
            n if (cmd::DISPLAY_BASE..cmd::DISPLAY_BASE + 32).contains(&n) => {
                self.st.settings.general.monitor = (n - cmd::DISPLAY_BASE) as u32;
                self.st.save_settings_soon();
                self.update_monitor();
            }
            _ => {}
        }
    }

    fn on_clipboard(&mut self) {
        let seq = platform::clipboard::sequence();
        if seq == self.st.f.clip.ignore_seq {
            return;
        }
        if !self.st.settings.productivity.clipboard {
            return;
        }
        let excluded = self.st.settings.productivity.clipboard_excluded.clone();
        let max = self.st.settings.productivity.clipboard_max as usize;
        match platform::clipboard::read(self.hwnd, &excluded) {
            Some(platform::clipboard::Clip::Text(t)) => {
                if self.st.f.clip.add_text(&t, max) {
                    if let Some(id) = self.st.f.clip.items.first().map(|i| i.id) {
                        self.st.f.appear(features::ListKind::Clip, id);
                    }
                    self.st.f.clip_dirty = true;
                    self.st.f.mark();
                }
            }
            Some(platform::clipboard::Clip::Files(files)) => {
                if self.st.f.clip.add_text(&files.join("\n"), max) {
                    self.st.f.clip_dirty = true;
                    self.st.f.mark();
                }
            }
            Some(platform::clipboard::Clip::Image(px)) => {
                if let Ok(png) = render::image::encode_png(&px) {
                    if png.len() < 20 * 1024 * 1024 {
                        let name = format!("{}.png", util::unix_ms());
                        if std::fs::write(util::sub_dir("clipboard").join(&name), png).is_ok() {
                            self.st.f.clip.add_image(name, px.w, px.h, max);
                            self.st.f.clip_dirty = true;
                            self.st.f.mark();
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn process_requests(&mut self) {
        let reqs = std::mem::take(&mut self.st.requests);
        for r in reqs {
            match r {
                Request::OpenSettings => self.open_settings(),
                Request::TakeFocus => {
                    // only after a click or hotkey, so hover never steals focus
                    if self.last_click.elapsed() < Duration::from_millis(800)
                        || self.st.input.keyboard_mode
                    {
                        self.take_focus();
                    }
                }
                Request::RestoreFocus => self.restore_focus(),
                Request::StartCapture(m) => self.capture(m),
                Request::ApplySettings => {
                    if !self.st.settings.popups.keep_history {
                        features::forget_notification_history();
                    }
                    if let Some(tx) = &self.st.svc.media {
                        let _ = tx.send(services::media::MediaCmd::Prefer(
                            self.st.settings.music.preferred_player.clone(),
                        ));
                    }
                    self.update_monitor();
                    self.apply_glass();
                    self.update_tray();
                    self.sync_widgets();
                    self.gfx
                        .text
                        .set_override(self.st.theme.font_override.clone());
                    if let Some(sw) = self.settings_win.as_mut() {
                        sw.dirty = true;
                    }
                }
                Request::RegisterHotkeys => self.register_hotkeys(),
                Request::PasteToPrevious => {
                    self.restore_focus();
                    self.pending_paste = Some(Instant::now() + Duration::from_millis(150));
                }
                Request::CopyText(t) => {
                    self.st.f.clip.ignore_seq = platform::clipboard::sequence().wrapping_add(1);
                    platform::clipboard::set_text(self.hwnd, &t);
                }
                Request::CopyImage(id) => {
                    let px = if id == u64::MAX {
                        ui::pages::flatten_annotations(&self.st)
                    } else {
                        self.st
                            .f
                            .clip
                            .items
                            .iter()
                            .find(|i| i.id == id)
                            .and_then(|i| i.image.clone())
                            .and_then(|f| {
                                render::image::decode_file(
                                    &util::sub_dir("clipboard").join(f),
                                    8192,
                                )
                                .ok()
                            })
                    };
                    if let Some(px) = px {
                        self.st.f.clip.ignore_seq = platform::clipboard::sequence().wrapping_add(1);
                        platform::clipboard::set_image(self.hwnd, &px);
                        if id == u64::MAX {
                            // screenshots also land on the shelf
                            if let Ok(png) = render::image::encode_png(&px) {
                                let name = format!("shot-{}.png", util::unix_ms());
                                if std::fs::write(util::sub_dir("shelf").join(&name), png).is_ok() {
                                    self.st.f.shelf.add(
                                        features::store::ShelfKind::Image { file: name.clone() },
                                        name,
                                    );
                                    self.st.f.shelf_dirty = true;
                                    self.st.f.mark();
                                }
                            }
                            self.st.popup(
                                Popup::new(
                                    PopKind::Info,
                                    Icon::Image,
                                    "Screenshot copied",
                                    "Also saved to the shelf",
                                )
                                .duration_ms(2000),
                            );
                            self.st.collapse();
                        }
                    }
                }
                Request::SaveImage(id) => {
                    let px = if id == u64::MAX {
                        ui::pages::flatten_annotations(&self.st)
                    } else {
                        self.st.f.ocr.image.as_ref().map(|p| (**p).clone())
                    };
                    if let Some(px) = px {
                        let dir = std::env::var_os("USERPROFILE")
                            .map(|p| std::path::PathBuf::from(p).join("Pictures").join("Notchly"))
                            .unwrap_or_else(|| util::sub_dir("screenshots"));
                        let _ = std::fs::create_dir_all(&dir);
                        let lt = util::local_now();
                        let path = dir.join(format!(
                            "Notchly {:04}-{:02}-{:02} {:02}{:02}{:02}.png",
                            lt.year, lt.month, lt.day, lt.hour, lt.minute, lt.second
                        ));
                        if let Ok(png) = render::image::encode_png(&px) {
                            if std::fs::write(&path, png).is_ok() {
                                platform::reveal(&path.to_string_lossy());
                            }
                        }
                    }
                }
                Request::OpenPath(p) => platform::shell_open(&p),
                Request::RevealPath(p) => platform::reveal(&p),
                Request::InstallUpdate => {
                    if let Some(p) = self.st.f.update_url.clone() {
                        updates::install(&p);
                        self.quit();
                    }
                }
                Request::OpenUrl(u) => platform::shell_open(&u),
                Request::ShareShelf(id) => {
                    let paths: Vec<String> = self
                        .st
                        .f
                        .shelf
                        .items
                        .iter()
                        .filter(|i| i.id == id || self.st.f.shelf.selected.contains(&i.id))
                        .filter_map(|i| i.path())
                        .collect();
                    if !paths.is_empty() {
                        platform::dragdrop::share(self.hwnd, paths);
                    }
                }
                Request::DragOutShelf(ids) => {
                    let mut paths = Vec::new();
                    for id in ids {
                        if let Some(it) = self.st.f.shelf.get(id) {
                            match &it.kind {
                                features::store::ShelfKind::Text(t)
                                | features::store::ShelfKind::Link(t) => {
                                    // virtual items become temp files so any app accepts them
                                    let ext =
                                        if matches!(it.kind, features::store::ShelfKind::Link(_)) {
                                            "url"
                                        } else {
                                            "txt"
                                        };
                                    let body = if ext == "url" {
                                        format!("[InternetShortcut]\r\nURL={t}\r\n")
                                    } else {
                                        t.clone()
                                    };
                                    let name: String = it
                                        .name
                                        .chars()
                                        .filter(|c| c.is_alphanumeric() || *c == ' ')
                                        .take(30)
                                        .collect();
                                    let p = std::env::temp_dir().join(format!(
                                        "{}.{ext}",
                                        if name.trim().is_empty() {
                                            "Notchly"
                                        } else {
                                            name.trim()
                                        }
                                    ));
                                    if std::fs::write(&p, body).is_ok() {
                                        paths.push(p.to_string_lossy().to_string());
                                    }
                                }
                                _ => {
                                    if let Some(p) = it.path() {
                                        paths.push(p);
                                    }
                                }
                            }
                        }
                    }
                    if !paths.is_empty() {
                        platform::dragdrop::drag_out(&paths);
                    }
                }
                Request::Chime(c) => platform::sound::play(c),
                Request::Announce(t) => platform::uia::announce(self.hwnd, &t),
                Request::UpdateTray => self.update_tray(),
                Request::TrimMemory => {
                    self.gfx.trim();
                    self.gfx.text.clear();
                    unsafe {
                        let _ = windows::Win32::System::Threading::SetProcessWorkingSetSize(
                            GetCurrentProcess(),
                            usize::MAX,
                            usize::MAX,
                        );
                    }
                }
                Request::SendKeys(k) => {
                    self.restore_focus();
                    std::thread::sleep(Duration::from_millis(60));
                    platform::send_keys(&k);
                }
                Request::DesktopWidgets => self.sync_widgets(),
            }
        }
    }

    fn drain_drops(&mut self) {
        let drops: Vec<platform::dragdrop::DropMsg> =
            platform::dragdrop::DROPS.with(|d| std::mem::take(&mut *d.borrow_mut()));
        for d in drops {
            use platform::dragdrop::DropMsg;
            match d {
                DropMsg::Enter => {
                    self.st.input.drag_near = true;
                    self.st.isl.swell.set_cfg(1.0, motion::SNAPPY);
                }
                DropMsg::Leave => {
                    self.st.input.drag_near = false;
                    self.st.isl.swell.set(0.0);
                }
                DropMsg::Files(files) => {
                    let mut ids = Vec::new();
                    for f in files {
                        if let Some(id) = self.st.f.shelf.add_path(&f) {
                            self.st.f.appear(features::ListKind::Shelf, id);
                            ids.push(id);
                        }
                    }
                    self.after_drop(ids);
                }
                DropMsg::Text(t) => {
                    let mut ids = Vec::new();
                    if let Some(id) = self.st.f.shelf.add_text(&t) {
                        self.st.f.appear(features::ListKind::Shelf, id);
                        ids.push(id);
                    }
                    self.after_drop(ids);
                }
            }
            self.st.dirty = true;
        }
    }

    fn after_drop(&mut self, ids: Vec<u64>) {
        // the notch jiggles like jelly as it swallows the drop; the shelf celebrates
        self.st.isl.swell.set_cfg(0.0, motion::BOUNCY);
        self.st.isl.swell.kick(-4.0);
        if !ids.is_empty() {
            self.st.f.drop_fx = Some((std::time::Instant::now(), ids));
        }
        self.st.f.shelf_dirty = true;
        self.st.f.mark();
        self.st.open(PageId::Shelf);
        self.st.pinned = false;
        if self.st.f.phone.running {
            self.st.popup(
                Popup::new(
                    PopKind::Info,
                    Icon::Send,
                    "Send to phone?",
                    "Your phone can download it now",
                )
                .action("Send", app::Action::ShelfSendPhone)
                .duration_ms(5000),
            );
        }
    }

    fn quit(&mut self) {
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
                OUTSIDE_TARGET = self.hwnd.0 as isize;
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
            "notes" => Some(PageId::Notes),
            "widgets" => Some(PageId::Widgets),
            "ask" => Some(PageId::Ask),
            "home" | "dashboard" => Some(PageId::Dashboard),
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
            if qa_mode() && args.iter().any(|a| a == "--qa-trash") {
                app.st.qa_trash = Some((Instant::now() + Duration::from_millis(1500), 0));
            }
        }
    }
    if args.iter().any(|a| a == "--settings") {
        app.open_settings();
    }
    app.run();
    log("Notchly exiting");
}
