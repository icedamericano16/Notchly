//! Event-driven system integrations. Each service runs on its own worker (or
//! registers OS callbacks) and talks to the UI thread only through `Bus`.

pub mod ai;
pub mod audio;
pub mod bluetooth;
pub mod calendar;
pub mod downloads;
pub mod http;
pub mod ics;
pub mod keyboard;
pub mod live_api;
pub mod lyrics;
pub mod media;
pub mod network;
pub mod notifications;
pub mod ocr;
pub mod phone_drop;
pub mod privacy;
pub mod radios;
pub mod rates;
pub mod sysmon;
pub mod visualiser;
pub mod weather;

use std::sync::mpsc::Sender;
use std::sync::Arc;

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

use crate::render::image::Pixels;
use crate::theme::tokens::Color;

pub const WM_APP_EVENT: u32 = WM_APP + 1;

/// Typed events from services to the UI thread.
#[derive(Debug)]
pub enum Event {
    Media(media::MediaSnapshot),
    MediaArt {
        key: u64,
        pixels: Option<Arc<Pixels>>,
        palette: Vec<Color>,
    },
    Lyrics {
        key: u64,
        result: lyrics::LyricsResult,
        meta: lyrics::LyricsMeta,
    },
    Volume {
        level: f32,
        muted: bool,
    },
    Mic {
        muted: bool,
    },
    AudioDevice {
        name: String,
        headphones: bool,
        initial: bool,
    },
    AppVolume {
        level: Option<f32>,
    },
    Network(network::NetInfo),
    Bluetooth {
        name: String,
        connected: bool,
        battery: Option<u8>,
    },
    LockKey(u32),
    LangHotkey,
    Privacy(Vec<privacy::Use>),
    /// Which live privacy sources work: (microphone sessions, camera activity monitor).
    PrivacySources(bool, bool),
    Live(live_api::LiveMsg),
    LiveClientGone(u64),
    Weather(Result<weather::Forecast, String>),
    Geocode(Result<Vec<weather::Place>, String>),
    /// The time zone found for a saved city (city name, zone).
    PlaceZone(String, Option<String>),
    Rates(rates::Rates),
    Ai(ai::AiEvent),
    Ocr(Result<ocr::OcrResult, String>),
    Calendar(Result<Vec<calendar::Event>, String>),
    Download(downloads::DownloadEvent),
    SysMon(sysmon::Sample),
    Viz(Vec<f32>),
    Notification(notifications::Toast),
    /// Notifications already in the Action Center at start-up (time = Unix seconds).
    NotificationsSeed(Vec<(notifications::Toast, i64)>),
    NotificationAccess(notifications::Access),
    PhoneDrop(phone_drop::DropEvent),
    Location(f64, f64),
    UpdateAvailable(String),
    /// Wi-Fi / Bluetooth radio switch positions.
    Radios(radios::Radios),
    /// A short message for the user (shown as a pop-up): (ok, title, body).
    Notice(bool, String, String),
    /// Calendar sign-in finished: provider name and the result.
    OAuthDone(String, Result<(), String>),
    Log(String),
}

/// Sender handle given to every service. Sending wakes the UI loop.
#[derive(Clone)]
pub struct Bus {
    tx: Sender<Event>,
    hwnd: isize,
}

impl Bus {
    pub fn new(tx: Sender<Event>, hwnd: HWND) -> Self {
        Self {
            tx,
            hwnd: hwnd.0 as isize,
        }
    }
    pub fn send(&self, e: Event) {
        if self.tx.send(e).is_ok() {
            unsafe {
                let _ = PostMessageW(
                    Some(HWND(self.hwnd as _)),
                    WM_APP_EVENT,
                    WPARAM(0),
                    LPARAM(0),
                );
            }
        }
    }
}

/// Initialises COM (MTA) on a worker thread.
pub fn com_mta() {
    unsafe {
        let _ = windows::Win32::System::Com::CoInitializeEx(
            None,
            windows::Win32::System::Com::COINIT_MULTITHREADED,
        );
    }
}

/// Spawns a named worker thread, logging instead of crashing on spawn failure.
pub fn spawn<F: FnOnce() + Send + 'static>(name: &str, f: F) {
    if let Err(e) = std::thread::Builder::new()
        .name(name.to_string())
        .stack_size(1 << 20)
        .spawn(f)
    {
        crate::util::log(&format!("spawn {name} failed: {e}"));
    }
}
