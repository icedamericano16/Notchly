//! Service events, timers and action dispatch.

use std::time::{Duration, Instant};

use super::activity::{ActKind, PopKind, Popup};
use super::state::{Battery, LyricsStatus, State};
use super::{Action, CaptureMode, Chime, InputId, PageId, Request, SliderKind, TimerCmd};
use crate::features::timer::{Phase, TimerEvent};
use crate::features::{AskMode, ChatMsg, LiveItem, Meeting};
use crate::motion::{Spring, BOUNCY};
use crate::render::icons::{self, Icon};
use crate::render::image::CachedBitmap;
use crate::services::ai::{self, AiEvent, AiRequest};
use crate::services::audio::{exe_for_app, AudioCmd};
use crate::services::lyrics::{request_key, LyricsRequest};
use crate::services::media::MediaCmd;
use crate::services::{
    downloads::DownloadEvent, live_api, phone_drop::DropEvent, privacy::Cap, Event,
};
use crate::theme::tokens::*;
use crate::util::{fmt_hm, local_now, unix_secs};

/// Power signals parsed from WM_POWERBROADCAST.
#[derive(Debug, Clone, Copy)]
pub enum PowerSignal {
    Status,
    Brightness(u32),
    Resume,
}

const MEETING_APPS: &[(&str, &str)] = &[
    ("teams", "Teams"),
    ("ms-teams", "Teams"),
    ("zoom", "Zoom"),
    ("discord", "Discord"),
    ("slack", "Slack"),
    ("whatsapp", "WhatsApp"),
    ("skype", "Skype"),
    ("webex", "Webex"),
];
const BROWSERS: &[&str] = &["chrome", "msedge", "firefox", "brave", "opera", "vivaldi"];

impl State {
    pub fn on_event(&mut self, e: Event) {
        self.dirty = true;
        match e {
            Event::Media(snap) => self.on_media(snap),
            Event::MediaArt {
                key,
                pixels,
                palette,
            } => {
                if key == self.media.snap.art_key {
                    self.media.art = pixels.map(CachedBitmap::new);
                    self.media.art_key = key;
                    self.media.palette = palette;
                    if self.settings.appearance.adaptive {
                        self.update_adaptive();
                    }
                }
            }
            Event::Lyrics { key, result } => {
                if key == self.lyrics.key {
                    let synced = matches!(result, crate::services::lyrics::LyricsResult::Synced(_));
                    self.lyrics.status = LyricsStatus::Ready(result);
                    self.lyrics.current = None;
                    self.lyrics.scroll.snap(0.0);
                    // "Lyrics translation" setting: translate each new song automatically
                    if synced && self.settings.music.lyrics_translate {
                        self.lyrics.show_translation = true;
                        if self.lyrics.translation.is_none() {
                            self.translate_lyrics();
                        }
                    }
                }
            }
            Event::Volume { level, muted } => {
                let changed = (level - self.sys.volume).abs() > 0.004 || muted != self.sys.muted;
                let first = !self.sys.volume_known;
                self.sys.volume = level;
                self.sys.muted = muted;
                self.sys.volume_known = true;
                if changed && !first && self.settings.popups.volume && self.input.slider.is_none() {
                    let pct = (level * 100.0).round();
                    self.popup(
                        Popup::new(
                            PopKind::Volume,
                            icons::volume_icon(level, muted),
                            "Volume",
                            if muted {
                                "Muted".to_string()
                            } else {
                                format!("{pct:.0}%")
                            },
                        )
                        .value(if muted { 0.0 } else { level })
                        .duration_ms(self.settings.popups.duration_ms as u64),
                    );
                }
            }
            Event::Mic { muted } => {
                if muted != self.sys.mic_muted {
                    self.sys.mic_muted = muted;
                    if self.settings.popups.mic {
                        self.popup(
                            Popup::new(
                                PopKind::Mic,
                                if muted { Icon::MicOff } else { Icon::Mic },
                                if muted {
                                    "Microphone muted"
                                } else {
                                    "Microphone on"
                                },
                                "",
                            )
                            .tint(if muted { CRITICAL } else { GOOD }),
                        );
                    }
                }
            }
            Event::AudioDevice {
                name,
                headphones,
                initial,
            } => {
                let changed = name != self.sys.audio_device;
                self.sys.audio_device = name.clone();
                self.sys.headphones = headphones;
                if changed && !initial && self.settings.popups.audio_device {
                    self.popup(Popup::new(
                        PopKind::AudioDevice,
                        if headphones {
                            Icon::Headphones
                        } else {
                            Icon::Speaker
                        },
                        name,
                        "Now playing through",
                    ));
                }
            }
            Event::AppVolume { level } => self.media.app_volume = level,
            Event::Network(n) => {
                let prev = std::mem::replace(&mut self.sys.net, n.clone());
                let known = std::mem::replace(&mut self.sys.net_known, true);
                if known && self.settings.popups.network {
                    if !n.connected && prev.connected {
                        self.popup(
                            Popup::new(
                                PopKind::Network,
                                Icon::WifiOff,
                                "Offline",
                                "No internet connection",
                            )
                            .tint(WARN),
                        );
                    } else if n.captive && !prev.captive {
                        self.popup(
                            Popup::new(
                                PopKind::Network,
                                Icon::Globe,
                                "Sign in to network",
                                n.name.clone(),
                            )
                            .action("Open", Action::Open(PageId::Dashboard))
                            .tint(WARN),
                        );
                    } else if n.connected && (!prev.connected || n.name != prev.name) {
                        let icon = if n.wifi {
                            icons::wifi_icon(n.bars.min(3))
                        } else {
                            Icon::Ethernet
                        };
                        let sub = if prev.connected {
                            "Switched network"
                        } else {
                            "Connected"
                        };
                        self.popup(Popup::new(PopKind::Network, icon, n.name.clone(), sub));
                    }
                }
            }
            Event::Bluetooth {
                name,
                connected,
                battery,
            } => {
                self.sys.bt_connected.retain(|n| n != &name);
                if connected {
                    self.sys.bt_connected.push(name.clone());
                }
                if self.settings.popups.bluetooth {
                    let sub = match (connected, battery) {
                        (true, Some(b)) => format!("Connected · {b}%"),
                        (true, None) => "Connected".into(),
                        (false, _) => "Disconnected".into(),
                    };
                    let low = battery.is_some_and(|b| b <= 15);
                    let mut p = Popup::new(PopKind::Bluetooth, Icon::Bluetooth, name.clone(), sub);
                    if let Some(b) = battery {
                        p = p.value(b as f32 / 100.0);
                    }
                    if low {
                        p = p.tint(WARN);
                    }
                    self.popup(p);
                    if low {
                        self.popup(
                            Popup::new(
                                PopKind::Battery,
                                Icon::Headphones,
                                format!("{name} battery low"),
                                format!("{}%", battery.unwrap_or(0)),
                            )
                            .tint(WARN),
                        );
                    }
                }
            }
            Event::LockKey(vk) => {
                self.lock_check = Some((Instant::now() + Duration::from_millis(60), vk))
            }
            Event::LangHotkey => {
                let now = Instant::now();
                self.lang_check = vec![
                    now + Duration::from_millis(150),
                    now + Duration::from_millis(500),
                ];
            }
            Event::Privacy(uses) => self.on_privacy(uses),
            Event::Live(m) => self.on_live(m),
            Event::LiveClientGone(client) => {
                // Fire-and-forget clients (the CLI) disconnect right after sending, so
                // activities outlive the connection; stale ones expire after 10 minutes
                // unless a later update/end (from any connection) refreshes them.
                let stale = Instant::now() + Duration::from_secs(600);
                let orphans: Vec<(String, i32)> = self
                    .f
                    .live
                    .items
                    .iter()
                    .filter(|i| i.client == client && i.final_text.is_none())
                    .map(|i| (i.id.clone(), i.priority))
                    .collect();
                for (id, prio) in orphans {
                    let key = format!("live:{id}");
                    let keep_expiry = self
                        .act
                        .get(&key)
                        .and_then(|a| a.expires)
                        .is_some_and(|e| e < stale);
                    if !keep_expiry {
                        self.act.upsert(&key, ActKind::LiveApi, prio, Some(stale));
                    }
                }
            }
            Event::Weather(r) => {
                self.f.weather.fetching = false;
                match r {
                    Ok(f) => {
                        self.f.weather.backoff.ok();
                        self.f.weather.fetched = Some(Instant::now());
                        self.f.weather.error = None;
                        // rain-soon from next-hour precipitation
                        let dry_now = !f.symbol.contains("rain") && !f.symbol.contains("showers");
                        if f.precip_next_hour >= 0.2
                            && dry_now
                            && self.f.weather.rain_warned_for != f.fetched / 3600
                        {
                            self.f.weather.rain_warned_for = f.fetched / 3600;
                            self.popup(Popup::new(
                                PopKind::Info,
                                Icon::Rain,
                                "Rain expected soon",
                                format!("{:.1} mm in the next hour", f.precip_next_hour),
                            ));
                        }
                        self.f.weather.forecast = Some(f);
                    }
                    Err(e) => {
                        self.f.weather.backoff.fail();
                        self.f.weather.error = Some(e);
                    }
                }
            }
            Event::Geocode(r) => match r {
                Ok(p) => {
                    // apply the best match: weather and prayer times use it right away
                    if let Some(first) = p.first().cloned() {
                        self.settings.weather.lat = (first.lat * 100.0).round() / 100.0;
                        self.settings.weather.lon = (first.lon * 100.0).round() / 100.0;
                        self.settings.weather.city = first.name.clone();
                        self.f.prayer_cache = None;
                        self.f.weather.fetched = None;
                        self.save_settings_soon();
                        if self.settings.weather.enabled && self.settings.privacy.net_weather {
                            self.refresh_weather(true);
                        }
                        self.popup(
                            Popup::new(
                                PopKind::Info,
                                Icon::Location,
                                format!("{}, {}", first.name, first.country),
                                "Location set",
                            )
                            .duration_ms(2500),
                        );
                    } else {
                        self.popup(
                            Popup::new(
                                PopKind::Info,
                                Icon::Location,
                                "City not found",
                                "Try another spelling",
                            )
                            .duration_ms(2500),
                        );
                    }
                    self.f.weather.places = p;
                }
                Err(e) => self.f.weather.error = Some(e),
            },
            Event::Rates(r) => self.f.rates = r.rates,
            Event::Ai(ev) => self.on_ai(ev),
            Event::Ocr(r) => {
                self.f.ocr.busy = false;
                self.remove_activity("ocr");
                match r {
                    Ok(res) => {
                        if let Some(q) = res.qr.first().cloned() {
                            self.requests.push(Request::CopyText(q.clone()));
                            let is_url = q.starts_with("http");
                            self.f.ocr.result = Some(res);
                            let mut p = Popup::new(
                                PopKind::Info,
                                Icon::Qr,
                                q.chars().take(60).collect::<String>(),
                                "QR code · copied",
                            )
                            .duration_ms(5000);
                            if is_url {
                                p = p.action("Open", Action::OcrOpenUrl(0));
                            }
                            self.popup(p.action("Details", Action::Open(PageId::Ocr)));
                        } else if res.text.trim().is_empty() {
                            self.popup(Popup::new(
                                PopKind::Info,
                                Icon::ScanText,
                                "No text found",
                                "Try a larger selection",
                            ));
                        } else {
                            self.requests.push(Request::CopyText(res.text.clone()));
                            let n = res.words;
                            self.f.ocr.result = Some(res);
                            self.popup(
                                Popup::new(
                                    PopKind::Info,
                                    Icon::ScanText,
                                    format!("Copied {n} word{}", if n == 1 { "" } else { "s" }),
                                    "Text from screen",
                                )
                                .action("Open", Action::Open(PageId::Ocr))
                                .duration_ms(4000),
                            );
                        }
                    }
                    Err(e) => {
                        self.f.ocr.error = Some(e.clone());
                        let mut p =
                            Popup::new(PopKind::Info, Icon::ScanText, "OCR failed", e.clone())
                                .duration_ms(5000);
                        if e.contains("No OCR language") {
                            p = p.action(
                                "Add language",
                                Action::OpenUrl("ms-settings:regionlanguage".into()),
                            );
                        }
                        self.popup(p);
                    }
                }
            }
            Event::Calendar(r) => {
                self.f.calendar.fetching = false;
                self.f.calendar.fetched = Some(Instant::now());
                match r {
                    Ok(ev) => {
                        self.f.calendar.events = ev;
                        self.f.calendar.error = None;
                    }
                    Err(e) => self.f.calendar.error = Some(e),
                }
            }
            Event::Download(d) => self.on_download(d),
            Event::SysMon(s) => {
                self.f.sysmon.push(s);
                if self.settings.productivity.sysmon_alert {
                    if s.cpu > 90.0 {
                        let since = *self.f.sysmon.high_since.get_or_insert(Instant::now());
                        if since.elapsed() > Duration::from_secs(30) && !self.f.sysmon.alerted {
                            self.f.sysmon.alerted = true;
                            let top = crate::services::sysmon::top_process().unwrap_or_default();
                            self.upsert_activity(
                                "sysalert",
                                ActKind::SysAlert,
                                Some(Instant::now() + Duration::from_secs(20)),
                            );
                            self.popup(
                                Popup::new(
                                    PopKind::Info,
                                    Icon::Cpu,
                                    "High CPU",
                                    if top.is_empty() {
                                        format!("{:.0}%", s.cpu)
                                    } else {
                                        top
                                    },
                                )
                                .tint(WARN)
                                .duration_ms(4000),
                            );
                        }
                    } else {
                        self.f.sysmon.high_since = None;
                        self.f.sysmon.alerted = false;
                    }
                }
            }
            Event::Viz(b) => self.media.bands = Some(b),
            Event::Notification(t) => {
                if !self.settings.popups.notifications
                    || self
                        .settings
                        .popups
                        .muted_apps
                        .iter()
                        .any(|a| a.eq_ignore_ascii_case(&t.app))
                {
                    return;
                }
                self.f.notifs.record(&t.app, &t.title, &t.body, false);
                let nid = self.f.notifs.next_id;
                self.f.appear(crate::features::ListKind::Notif, nid);
                if self.settings.popups.keep_history {
                    self.f.notifs_dirty = true;
                    self.f.mark();
                }
                let mut p = Popup::new(
                    PopKind::Notification,
                    Icon::Bell,
                    t.title.clone(),
                    t.body.clone(),
                )
                .duration_ms(self.settings.popups.notification_ms as u64)
                .priority(60);
                p.source = t.app.clone();
                p.badge = Some(
                    t.app
                        .chars()
                        .next()
                        .map(|c| c.to_uppercase().to_string())
                        .unwrap_or_default(),
                );
                self.popup(p);
            }
            Event::NotificationAccess(a) => self.f.notifs.access = Some(a),
            Event::PhoneDrop(d) => match d {
                DropEvent::Started { url, pin, qr } => {
                    self.f.phone.running = true;
                    self.f.phone.url = url;
                    self.f.phone.pin = pin;
                    self.f.phone.qr = Some(qr);
                    self.f.phone.status = "Scan with your phone's camera".into();
                }
                DropEvent::Progress { name, frac } => {
                    self.f.phone.progress = Some(frac);
                    self.f.phone.status = format!("Receiving {name}");
                    self.upsert_activity(
                        "phonedrop",
                        ActKind::Transfer,
                        Some(Instant::now() + Duration::from_secs(10)),
                    );
                }
                DropEvent::Received { path } => {
                    self.f.phone.progress = None;
                    self.remove_activity("phonedrop");
                    if let Some(id) = self.f.shelf.add_path(&path) {
                        self.f.appear(crate::features::ListKind::Shelf, id);
                        self.f.shelf_dirty = true;
                        self.f.mark();
                    }
                    let name = std::path::Path::new(&path)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    self.f.phone.status = format!("Received {name}");
                    self.popup(
                        Popup::new(PopKind::Download, Icon::Shelf, "Added to shelf", name)
                            .action("Open shelf", Action::Open(PageId::Shelf)),
                    );
                }
                DropEvent::Stopped => {
                    self.f.phone = Default::default();
                    self.f.phone_server = None;
                }
                DropEvent::Error(e) => self.f.phone.status = e,
            },
            Event::Location(lat, lon) => {
                self.settings.weather.lat = lat;
                self.settings.weather.lon = lon;
                self.f.prayer_cache = None;
                self.save_settings_soon();
            }
            Event::UpdateAvailable(url) => {
                self.f.update_url = Some(url);
                self.requests.push(Request::UpdateTray);
                self.popup(
                    Popup::new(
                        PopKind::Info,
                        Icon::Download,
                        "Update ready",
                        "Downloaded and verified",
                    )
                    .action("Install & restart", Action::InstallUpdate)
                    .duration_ms(8000),
                );
            }
            Event::Radios(r) => self.sys.radios = r,
            Event::Notice(ok, title, body) => {
                self.popup(
                    Popup::new(
                        PopKind::Info,
                        if ok { Icon::CheckCircle } else { Icon::Info },
                        title,
                        body,
                    )
                    .duration_ms(3500),
                );
            }
            Event::OAuthDone(name, r) => {
                let (icon, title, body) = match r {
                    Ok(()) => {
                        self.f.calendar.fetched = None;
                        self.f.calendar.error = None;
                        (
                            Icon::CheckCircle,
                            format!("{name} connected"),
                            "Your events will appear in a moment".to_string(),
                        )
                    }
                    Err(e) => (Icon::Info, format!("{name} sign-in failed"), e),
                };
                self.popup(Popup::new(PopKind::Info, icon, title, body).duration_ms(4500));
                self.requests.push(Request::ApplySettings);
            }
            Event::Log(s) => crate::util::log(&s),
        }
    }

    fn on_media(&mut self, snap: crate::services::media::MediaSnapshot) {
        let track_changed = snap.art_key != self.media.snap.art_key;
        let was_playing = self.media.snap.playing;
        self.media.snap = snap;
        let has = self.media.has_session() && self.settings.music.enabled;
        if !has {
            self.remove_activity("music");
            self.media.art = None;
        } else if self.media.snap.playing {
            self.media.paused_since = None;
            self.upsert_activity("music", ActKind::Music, None);
        } else if was_playing {
            self.media.paused_since = Some(Instant::now());
        }
        if track_changed && has {
            self.media.track_changed = Some(Instant::now());
            self.media.art = None;
            self.lyrics.translation = None;
            let s = &self.media.snap;
            let key = request_key(&s.title, &s.artist);
            if key != self.lyrics.key {
                self.lyrics.key = key;
                if self.settings.music.lyrics {
                    self.lyrics.status = LyricsStatus::Loading;
                    if let Some(tx) = &self.svc.lyrics {
                        let _ = tx.send((
                            LyricsRequest {
                                key,
                                title: s.title.clone(),
                                artist: s.artist.clone(),
                                album: s.album.clone(),
                                duration_s: s.duration_ms / 1000,
                            },
                            self.settings.privacy.net_lyrics,
                        ));
                    }
                } else {
                    self.lyrics.status = LyricsStatus::Idle;
                }
            }
        }
        if self.expanded && self.page == PageId::Music && self.settings.music.app_volume {
            self.audio_cmd(AudioCmd::QueryAppVolume(exe_for_app(
                &self.media.snap.app_id,
            )));
        }
    }

    /// Battery/AC/saver changes from WM_POWERBROADCAST.
    pub fn on_battery(&mut self, b: Battery) {
        let prev = std::mem::replace(&mut self.sys.battery, b.clone());
        if !b.present || prev == b || self.quiet_start() && prev.percent == 0 {
            return;
        }
        self.dirty = true;
        let p = &self.settings.popups;
        if !p.battery {
            return;
        }
        let low = self.settings.popups.low_battery;
        if b.ac && !prev.ac && prev.percent > 0 {
            self.upsert_activity(
                "charging",
                ActKind::Charging,
                Some(Instant::now() + Duration::from_secs(5)),
            );
            self.isl.sweep = Some(Instant::now());
            let sub = b
                .secs_left
                .map(|s| format!("{}% · {} to full", b.percent, fmt_dur(s)))
                .unwrap_or_else(|| format!("{}%", b.percent));
            self.popup(
                Popup::new(PopKind::Battery, Icon::Bolt, "Charging", sub)
                    .value(b.percent as f32 / 100.0)
                    .tint(GOOD),
            );
        } else if !b.ac && prev.ac && prev.percent > 0 {
            self.remove_activity("charging");
            let sub = b
                .secs_left
                .map(|s| format!("{}% · {} left", b.percent, fmt_dur(s)))
                .unwrap_or_else(|| format!("{}%", b.percent));
            self.popup(
                Popup::new(PopKind::Battery, Icon::Bolt, "On battery", sub)
                    .value(b.percent as f32 / 100.0),
            );
        } else if !b.ac && b.percent <= 10 && prev.percent > 10 {
            self.isl.breathe(CRITICAL);
            self.popup(
                Popup::new(
                    PopKind::Battery,
                    Icon::Bolt,
                    "Battery critical",
                    format!("{}% · plug in now", b.percent),
                )
                .value(b.percent as f32 / 100.0)
                .tint(CRITICAL)
                .critical()
                .duration_ms(5000),
            );
        } else if !b.ac && b.percent <= low && prev.percent > low {
            self.isl.breathe(WARN);
            self.popup(
                Popup::new(
                    PopKind::Battery,
                    Icon::Bolt,
                    "Battery low",
                    format!("{}%", b.percent),
                )
                .value(b.percent as f32 / 100.0)
                .tint(WARN)
                .duration_ms(3000),
            );
        } else if b.ac && b.percent >= 100 && prev.percent < 100 && prev.percent > 0 {
            self.popup(
                Popup::new(PopKind::Battery, Icon::Bolt, "Fully charged", "100%")
                    .value(1.0)
                    .tint(GOOD),
            );
        }
        if b.saver != prev.saver && prev.percent > 0 {
            self.popup(Popup::new(
                PopKind::Battery,
                Icon::Leaf,
                if b.saver {
                    "Battery saver on"
                } else {
                    "Battery saver off"
                },
                "",
            ));
        }
    }

    pub fn on_brightness(&mut self, pct: u32) {
        let prev = self.sys.brightness.replace(pct);
        if prev.is_some()
            && prev != Some(pct)
            && self.settings.popups.brightness
            && self.input.slider.is_none()
        {
            self.popup(
                Popup::new(
                    PopKind::Brightness,
                    Icon::Brightness,
                    "Brightness",
                    format!("{pct}%"),
                )
                .value(pct as f32 / 100.0)
                .duration_ms(self.settings.popups.duration_ms as u64),
            );
        }
    }

    fn on_privacy(&mut self, uses: Vec<crate::services::privacy::Use>) {
        let now = unix_secs();
        // usage log (kept 7 days)
        let log = &mut self.f.state.privacy_log;
        for u in &uses {
            if !log
                .iter()
                .any(|l| l.stop.is_none() && l.cap == u.cap && l.app == u.app)
            {
                log.push(crate::services::privacy::LogEntry {
                    cap: u.cap,
                    app: u.app.clone(),
                    start: now,
                    stop: None,
                });
            }
        }
        for l in log.iter_mut().filter(|l| l.stop.is_none()) {
            if !uses.iter().any(|u| u.cap == l.cap && u.app == l.app) {
                l.stop = Some(now);
            }
        }
        log.retain(|l| now - l.start < 7 * 86400);
        if log.len() > 500 {
            let n = log.len() - 500;
            log.drain(..n);
        }
        self.f.state_dirty = true;
        self.f.mark();
        // meeting detection
        {
            let mic: Vec<&crate::services::privacy::Use> =
                uses.iter().filter(|u| u.cap == Cap::Mic).collect();
            let found = mic.iter().find_map(|u| {
                let l = u.app.to_lowercase();
                MEETING_APPS
                    .iter()
                    .find(|(k, _)| l.contains(k))
                    .map(|(_, n)| (n.to_string(), l.clone()))
                    .or_else(|| {
                        if BROWSERS.iter().any(|b| l.contains(b)) {
                            let t = crate::platform::window_title(unsafe {
                                windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow()
                            });
                            if t.contains("Meet") || t.contains("Zoom") || t.contains("Teams") {
                                return Some(("Google Meet".to_string(), l.clone()));
                            }
                        }
                        None
                    })
            });
            match (found, &self.f.meeting) {
                (Some((app, _exe)), None) => {
                    self.f.meeting = Some(Meeting {
                        app,
                        started: Instant::now(),
                        from_calendar: false,
                    });
                    self.upsert_activity("call", ActKind::Call, None);
                    if self.settings.quiet.auto_meetings {
                        self.sup.meeting_quiet = true;
                    }
                }
                (None, Some(m)) if !m.from_calendar => {
                    self.f.meeting = None;
                    self.remove_activity("call");
                    self.sup.meeting_quiet = false;
                }
                _ => {}
            }
        }
        if uses.is_empty() {
            self.f.privacy.since = None;
        } else if self.f.privacy.uses.is_empty() {
            self.f.privacy.since = Some(Instant::now());
        }
        self.f.privacy.uses = uses;
    }

    fn on_live(&mut self, m: live_api::LiveMsg) {
        let err = |c: u64, msg: &str| {
            live_api::send(
                c,
                &serde_json::json!({"event":"error","message":msg}).to_string(),
            )
        };
        if !self.settings.live.api_enabled {
            err(
                m.client,
                "Live Activities are turned off in Notchly settings",
            );
            return;
        }
        let exe = m.exe.to_lowercase();
        match self.settings.live.api_apps.get(&exe).copied() {
            Some(false) => {
                err(m.client, "This app was denied by the user");
                return;
            }
            None => {
                if !self.f.live.pending_approval.iter().any(|(_, e)| *e == exe) {
                    self.f.live.pending_approval.push((m.client, exe.clone()));
                    let name = if exe.is_empty() {
                        "An app".to_string()
                    } else {
                        exe.trim_end_matches(".exe").to_string()
                    };
                    let p = Popup::new(
                        PopKind::Approval,
                        Icon::Bell,
                        format!("Allow {name} to show live activities?"),
                        "It can post progress to your notch",
                    )
                    .action("Allow", Action::LiveAllow(exe.clone(), true))
                    .action("Deny", Action::LiveAllow(exe.clone(), false))
                    .duration_ms(15000);
                    self.popup(p);
                }
                live_api::send(
                    m.client,
                    &serde_json::json!({"event":"pending_approval"}).to_string(),
                );
                if self.f.live.queued.len() < 32 {
                    self.f.live.queued.push(m);
                }
                return;
            }
            Some(true) => {}
        }
        let key = format!("live:{}", m.id);
        match m.op.as_str() {
            "start" | "update" => {
                let exists = self.f.live.items.iter().any(|i| i.id == m.id);
                if !exists && self.f.live.items.len() >= live_api::MAX_ACTIVITIES {
                    err(m.client, "too many live activities (max 8)");
                    return;
                }
                if !exists && m.op == "update" {
                    err(m.client, "unknown id; send start first");
                    return;
                }
                let now = Instant::now();
                if !exists {
                    self.f.live.items.push(LiveItem {
                        id: m.id.clone(),
                        client: m.client,
                        app: m.exe.clone(),
                        title: String::new(),
                        subtitle: String::new(),
                        icon: None,
                        image: None,
                        progress: None,
                        priority: 60,
                        color: None,
                        actions: Vec::new(),
                        final_text: None,
                        updated: now,
                        update_count: (now, 0),
                    });
                }
                let Some(it) = self.f.live.items.iter_mut().find(|i| i.id == m.id) else {
                    return;
                };
                it.client = m.client;
                // rate limit: 10 updates/s per activity (excess coalesced into the next)
                if (now - it.update_count.0) > Duration::from_secs(1) {
                    it.update_count = (now, 0);
                }
                it.update_count.1 += 1;
                if it.update_count.1 > live_api::MAX_UPDATES_PER_SEC
                    && m.op == "update"
                    && m.progress.is_some()
                    && m.title.is_none()
                {
                    it.progress = m.progress;
                    return;
                }
                if let Some(t) = m.title {
                    it.title = t;
                }
                if let Some(s) = m.subtitle {
                    it.subtitle = s;
                }
                if let Some(ic) = m.icon {
                    if let Some(data) = ic
                        .strip_prefix("data:")
                        .and_then(|d| d.split_once(";base64,"))
                        .map(|x| x.1.to_string())
                    {
                        if let Some(bytes) = live_api::b64decode(&data)
                            .filter(|b| b.len() <= live_api::MAX_ICON_BYTES)
                        {
                            if let Ok(px) = crate::render::image::decode(&bytes, 64) {
                                it.image = Some(CachedBitmap::new(std::sync::Arc::new(px)));
                            }
                        }
                    } else {
                        it.icon = icons::by_name(&ic);
                    }
                }
                if m.progress.is_some() {
                    it.progress = m.progress;
                }
                if let Some(p) = m.priority {
                    it.priority = p;
                }
                if let Some(c) = m.color.as_deref().and_then(Color::parse) {
                    it.color = Some(c);
                }
                if let Some(a) = m.actions {
                    it.actions = a.into_iter().map(|a| (a.id, a.label)).collect();
                }
                it.updated = now;
                let prio = it.priority;
                let expires = m.expires_in.map(|s| now + Duration::from_secs(s));
                self.act.upsert(&key, ActKind::LiveApi, prio, expires);
            }
            "end" => {
                let linger = m.linger.unwrap_or(4);
                if let Some(it) = self.f.live.items.iter_mut().find(|i| i.id == m.id) {
                    it.final_text = Some(m.final_text.unwrap_or_else(|| "Done".into()));
                    it.progress = Some(1.0);
                    it.actions.clear();
                    let prio = it.priority;
                    self.act.upsert(
                        &key,
                        ActKind::LiveApi,
                        prio,
                        Some(Instant::now() + Duration::from_secs(linger)),
                    );
                }
            }
            _ => {}
        }
        self.dirty = true;
    }

    fn on_ai(&mut self, ev: AiEvent) {
        let ask = &mut self.f.ask;
        let id = match &ev {
            AiEvent::Provider { id, .. }
            | AiEvent::Token { id, .. }
            | AiEvent::Done { id }
            | AiEvent::Error { id, .. } => *id,
        };
        if id == u64::MAX - 1 {
            // lyrics translation job: tokens accumulate into the translation buffer
            match ev {
                AiEvent::Token { text, .. } => ask.translation.push_str(&text),
                AiEvent::Done { .. } => {
                    let lines: Vec<String> = std::mem::take(&mut ask.translation)
                        .lines()
                        .map(String::from)
                        .collect();
                    let key = self.lyrics.key;
                    let dir = crate::util::sub_dir("cache/lyrics");
                    let _ = std::fs::write(
                        dir.join(format!("{key:016x}.tr.json")),
                        serde_json::to_vec(&lines).unwrap_or_default(),
                    );
                    self.lyrics.translation = Some(lines);
                }
                _ => {}
            }
            return;
        }
        if id != ask.request_id {
            return;
        }
        match ev {
            AiEvent::Provider { label, .. } => ask.provider = label,
            AiEvent::Token { text, .. } => match ask.mode {
                AskMode::Translate => ask.translation.push_str(&text),
                _ => {
                    if let Some(last) = ask.msgs.last_mut().filter(|m| !m.user) {
                        last.text.push_str(&text);
                    } else {
                        ask.msgs.push(ChatMsg { user: false, text });
                    }
                }
            },
            AiEvent::Done { .. } => {
                ask.streaming = false;
                if ask.mode == AskMode::Translate
                    && self.input.text_focus.is_none()
                    && !self.expanded
                {
                    // translate-selection result shown as a pop-up
                    let t = ask.translation.clone();
                    self.popup(
                        Popup::new(
                            PopKind::Info,
                            Icon::Translate,
                            t.chars().take(80).collect::<String>(),
                            "Translation",
                        )
                        .action("Copy", Action::AskCopy)
                        .duration_ms(8000),
                    );
                }
            }
            AiEvent::Error { message, .. } => {
                ask.streaming = false;
                ask.error = Some(message);
            }
        }
    }

    fn on_download(&mut self, d: DownloadEvent) {
        if !self.settings.productivity.downloads || false {
            return;
        }
        match d {
            DownloadEvent::Progress(dl) => {
                let key = format!("dl:{}", dl.id);
                match self.f.downloads.items.iter_mut().find(|x| x.id == dl.id) {
                    Some(x) => *x = dl,
                    None => self.f.downloads.items.insert(0, dl),
                }
                self.upsert_activity(
                    &key,
                    ActKind::Transfer,
                    Some(Instant::now() + Duration::from_secs(30)),
                );
            }
            DownloadEvent::Done(dl) => {
                self.remove_activity(&format!("dl:{}", dl.id));
                let id = dl.id;
                let name = dl.name.clone();
                match self.f.downloads.items.iter_mut().find(|x| x.id == id) {
                    Some(x) => *x = dl,
                    None => self.f.downloads.items.insert(0, dl),
                }
                self.f.downloads.items.truncate(20);
                self.popup(
                    Popup::new(PopKind::Download, Icon::Download, name, "Download complete")
                        .action("Open", Action::DownloadOpen(id))
                        .action("Show", Action::DownloadReveal(id))
                        .action("Shelf", Action::DownloadShelf(id))
                        .duration_ms(6000),
                );
            }
            DownloadEvent::Cancelled(id) => {
                self.remove_activity(&format!("dl:{id}"));
                self.f.downloads.items.retain(|x| x.id != id || x.done);
            }
        }
    }

    // ------------------------------------------------------------ timers

    pub fn tick(&mut self, now: Instant) {
        let hold = self.input.inside && matches!(self.isl.view, super::island::View::Peek(_));
        if self.pop.tick(now, hold) {
            self.dirty = true;
        }
        if self.act.expire(now) {
            self.dirty = true;
            let live: Vec<String> = self.f.live.items.iter().map(|i| i.id.clone()).collect();
            for id in live {
                if !self.act.contains(&format!("live:{id}")) {
                    self.f.live.items.retain(|i| i.id != id);
                }
            }
        }
        if self
            .media
            .paused_since
            .is_some_and(|p| now - p > Duration::from_secs(30))
        {
            self.media.paused_since = None;
            self.remove_activity("music");
        }
        if let Some((at, vk)) = self.lock_check {
            if now >= at {
                self.lock_check = None;
                self.check_lock(vk);
            }
        }
        if self.lang_check.first().is_some_and(|t| now >= *t) {
            self.lang_check.remove(0);
            self.check_language();
        }
        self.tick_timer();
        self.tick_water();
        self.tick_todo();
        self.tick_prayer();
        self.tick_calendar();
        self.tick_weather();
        // notification digest when suppression ends
        let suppressed = self.quiet_hours() || self.fullscreen_hidden() || self.popups_paused();
        if self.f.was_suppressed && !suppressed {
            if let Some((n, a)) = self.f.notifs.take_digest() {
                self.popup(
                    Popup::new(
                        PopKind::Digest,
                        Icon::Bell,
                        format!(
                            "While you were away: {n} notification{}",
                            if n == 1 { "" } else { "s" }
                        ),
                        format!("from {a} app{}", if a == 1 { "" } else { "s" }),
                    )
                    .action("Open", Action::Open(PageId::Notifications))
                    .duration_ms(5000),
                );
            }
        }
        self.f.was_suppressed = suppressed;
        if self.settings_save_due.is_some_and(|d| now >= d) {
            self.settings_save_due = None;
            crate::settings::store::save(&self.settings);
        }
        self.f.save(false);
        if let Some((start, step)) = self.demo {
            self.tick_demo(start, step, now);
        }
        if let Some((at, step)) = self.qa_trash {
            if now >= at {
                self.tick_qa_trash(step, now);
            }
        }
    }

    /// Debug QA: drop three items on the shelf, then clear them into the trash.
    fn tick_qa_trash(&mut self, step: u8, now: Instant) {
        use crate::features::ListKind;
        match step {
            0 => {
                let mut ids = Vec::new();
                for t in ["Quarterly report", "https://example.com", "Grocery list"] {
                    if let Some(id) = self.f.shelf.add_text(t) {
                        self.f.appear(ListKind::Shelf, id);
                        ids.push(id);
                    }
                }
                self.isl.swell.set_cfg(0.0, crate::motion::BOUNCY);
                self.isl.swell.kick(-4.0);
                self.f.drop_fx = Some((now, ids));
                self.qa_trash = Some((now + Duration::from_millis(2400), 1));
            }
            1 => {
                if let Some(id) = self.f.shelf.items.first().map(|i| i.id) {
                    self.dispatch(Action::ShelfRemove(id));
                }
                self.qa_trash = Some((now + Duration::from_millis(1800), 2));
            }
            _ => {
                self.dispatch(Action::ShelfClear);
                self.qa_trash = None;
            }
        }
        self.dirty = true;
    }

    /// Next instant something time-based must happen (for the idle wait).
    pub fn next_deadline(&self) -> Option<Instant> {
        let now = Instant::now();
        let mut d: Vec<Instant> = Vec::new();
        d.extend(self.pop.deadline());
        d.extend(self.act.next_expiry());
        d.extend(self.input.open_due);
        d.extend(self.input.close_due);
        d.extend(self.input.pill_until.filter(|t| *t > now));
        d.extend(self.qa_trash.map(|q| q.0));
        d.extend(self.lock_check.map(|l| l.0));
        d.extend(self.lang_check.first().copied());
        d.extend(self.settings_save_due);
        d.extend(self.f.save_due);
        d.extend(self.media.paused_since.map(|p| p + Duration::from_secs(30)));
        let unix_to_instant = |t: i64| now + Duration::from_secs((t - unix_secs()).max(0) as u64);
        if let Some(e) = self.f.state.timer.end_ms {
            let ms = (e - crate::util::unix_ms()).max(0) as u64;
            d.push(
                now + Duration::from_millis(ms.saturating_sub(10_000).max(if ms > 10_000 {
                    0
                } else {
                    ms
                })),
            );
        }
        if self.settings.productivity.water && self.f.state.water.next_reminder > 0 {
            d.push(unix_to_instant(self.f.state.water.next_reminder));
        }
        if self.settings.productivity.todo_reminders {
            d.extend(self.f.todo.next_due().map(unix_to_instant));
        }
        // features that need a coarse minute ticker
        if self.settings.productivity.prayer
            || self.settings.calendar.enabled
            || self.settings.weather.enabled
        {
            d.push(now + Duration::from_secs(30));
        }
        if self.settings.appearance.mascot && self.isl.view == super::island::View::Idle {
            let appear = self.last_input
                + Duration::from_secs(self.settings.appearance.mascot_delay_min as u64 * 60);
            if appear > now {
                d.push(appear);
            } else if self.mascot.alpha.value > 0.5 {
                d.push(self.mascot.blink_at);
            }
        }
        if let Some((s, _)) = self.demo {
            d.push(s + Duration::from_millis(200));
        }
        d.into_iter().min()
    }

    fn check_lock(&mut self, vk: u32) {
        use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CAPITAL};
        let mut on = unsafe { GetKeyState(vk as i32) } & 1 != 0;
        let caps = vk == VK_CAPITAL.0 as u32;
        let prev = if caps { self.sys.caps } else { self.sys.num };
        if on == prev {
            // the background thread's key state can lag; the key was pressed, so it toggled
            on = !prev;
        }
        if caps {
            self.sys.caps = on;
        } else {
            self.sys.num = on;
        }
        if self.settings.popups.caps {
            let name = if caps { "Caps Lock" } else { "Num Lock" };
            let p = Popup::new(
                if caps { PopKind::Caps } else { PopKind::Num },
                if caps { Icon::Caps } else { Icon::NumLock },
                format!("{name} {}", if on { "on" } else { "off" }),
                "",
            )
            .tint(if on { self.accent()[0] } else { MIST });
            self.popup(p);
        }
    }

    fn check_language(&mut self) {
        use windows::Win32::Globalization::{
            GetLocaleInfoEx, LCIDToLocaleName, LOCALE_ALLOW_NEUTRAL_NAMES,
            LOCALE_SLOCALIZEDDISPLAYNAME,
        };
        use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyboardLayout;
        use windows::Win32::UI::WindowsAndMessaging::{
            GetForegroundWindow, GetWindowThreadProcessId,
        };
        let hkl = unsafe {
            let fg = GetForegroundWindow();
            let tid = GetWindowThreadProcessId(fg, None);
            GetKeyboardLayout(tid)
        };
        let lang = (hkl.0 as usize & 0xffff) as u32;
        if lang == self.sys.layout || lang == 0 {
            return;
        }
        let first = self.sys.layout == 0;
        self.sys.layout = lang;
        if first || !self.settings.popups.language {
            return;
        }
        let mut name = [0u16; 85];
        let n = unsafe { LCIDToLocaleName(lang, Some(&mut name), LOCALE_ALLOW_NEUTRAL_NAMES) };
        let tag = if n > 0 {
            crate::util::from_wide(&name)
        } else {
            String::new()
        };
        let code: String = tag
            .split('-')
            .next()
            .unwrap_or("")
            .to_uppercase()
            .chars()
            .take(2)
            .collect();
        let mut disp = [0u16; 128];
        let w = crate::util::wide(&tag);
        let dn = unsafe {
            GetLocaleInfoEx(
                windows::core::PCWSTR(w.as_ptr()),
                LOCALE_SLOCALIZEDDISPLAYNAME,
                Some(&mut disp),
            )
        };
        let display = if dn > 0 {
            crate::util::from_wide(&disp)
        } else {
            tag.clone()
        };
        self.popup(
            Popup::new(PopKind::Language, Icon::Keyboard, display, "Input language").badge(code),
        );
    }

    fn tick_timer(&mut self) {
        let t = &mut self.f.state.timer;
        if let Some(TimerEvent::Finished(phase)) = t.tick() {
            self.f.state_dirty = true;
            self.f.mark();
            self.remove_activity("timer");
            self.isl.pulse(self.accent()[1], 3);
            if self.settings.sound.timer_chime {
                self.requests.push(Request::Chime(Chime::TimerEnd));
            }
            let (title, sub) = match phase {
                Phase::Focus => ("Focus session complete", "Time for a break"),
                Phase::ShortBreak | Phase::LongBreak => ("Break's over", "Ready to focus?"),
                Phase::Custom => ("Timer done", ""),
            };
            let mut p = Popup::new(PopKind::TimerDone, Icon::Timer, title, sub).duration_ms(8000);
            p = match phase {
                Phase::Focus => p
                    .action("Start break", Action::Timer(TimerCmd::StartBreak))
                    .action("+5 min", Action::Timer(TimerCmd::Plus5)),
                Phase::ShortBreak | Phase::LongBreak => {
                    p.action("Start focus", Action::Timer(TimerCmd::StartFocus))
                }
                Phase::Custom => p.action("+5 min", Action::Timer(TimerCmd::Plus5)),
            };
            self.popup(p);
            if phase == Phase::Focus {
                self.mascot.hearts_until = Some(Instant::now() + Duration::from_secs(4));
            }
            return;
        }
        if t.running() {
            let left = t.remaining_ms();
            let kind = if left < 10_000 {
                ActKind::TimerEnding
            } else {
                ActKind::Focus
            };
            self.upsert_activity("timer", kind, None);
        } else if t.active() {
            self.upsert_activity("timer", ActKind::Focus, None);
        } else {
            self.remove_activity("timer");
        }
    }

    fn tick_water(&mut self) {
        let p = &self.settings.productivity;
        if !p.water {
            return;
        }
        let w = &mut self.f.state.water;
        w.roll();
        if w.next_reminder == 0 {
            w.schedule(p);
            self.f.state_dirty = true;
            return;
        }
        if w.due() {
            let m = local_now().minutes_of_day();
            let in_window = m >= p.water_start_min && m < p.water_end_min;
            // reschedule first so the reminder never repeats
            w.schedule(p);
            let ml = w.ml;
            self.f.state_dirty = true;
            self.f.mark();
            let p = &self.settings.productivity;
            if in_window && ml < p.water_goal_ml {
                let sub = format!("{} / {} ml today", ml, p.water_goal_ml);
                if self.settings.sound.reminder_chime
                    && !self.quiet_hours()
                    && !self.fullscreen_hidden()
                {
                    self.requests.push(Request::Chime(Chime::Reminder));
                }
                self.popup(
                    Popup::new(PopKind::Reminder, Icon::Droplet, "Time for some water", sub)
                        .action("Drank", Action::WaterDrank)
                        .action("Snooze 15 m", Action::WaterSnooze)
                        .action("Skip", Action::WaterSkip)
                        .duration_ms(8000)
                        .priority(50),
                );
            }
        }
    }

    fn tick_todo(&mut self) {
        if !self.settings.productivity.todo_reminders {
            return;
        }
        let due = self.f.todo.due_now();
        if !due.is_empty() {
            self.f.todo_dirty = true;
            self.f.mark();
        }
        for t in due {
            if self.settings.sound.reminder_chime {
                self.requests.push(Request::Chime(Chime::Reminder));
            }
            self.popup(
                Popup::new(
                    PopKind::Reminder,
                    Icon::CheckCircle,
                    t.title.clone(),
                    "Reminder",
                )
                .action("Done", Action::TodoToggle(t.id))
                .action("Snooze 10 m", Action::TodoSnooze(t.id))
                .duration_ms(8000),
            );
        }
    }

    fn tick_prayer(&mut self) {
        if !self.settings.productivity.prayer {
            self.remove_activity("prayer");
            return;
        }
        let settings = self.settings.clone();
        let Some(times) = self.f.prayer_times(&settings) else {
            return;
        };
        let now = local_now();
        let now_min = now.secs_of_day() / 60.0;
        let day = now.day_number();
        match crate::features::prayer::next_prayer(&times, now_min) {
            Some((idx, mins)) => {
                if mins <= 15.0 {
                    self.upsert_activity("prayer", ActKind::Prayer, None);
                } else {
                    self.remove_activity("prayer");
                }
                let rem = settings.productivity.prayer_reminder_min as f64;
                if mins <= rem && self.f.prayer_notified != Some((day, idx)) {
                    self.f.prayer_notified = Some((day, idx));
                    let name = times.prayers()[idx].0;
                    self.popup(
                        Popup::new(
                            PopKind::Reminder,
                            Icon::MoonStar,
                            format!("{name} in {} min", mins.ceil()),
                            fmt_hm(times.prayers()[idx].1 as u32, self.clock_24h()),
                        )
                        .duration_ms(6000),
                    );
                }
                if settings.productivity.prayer_pause_media
                    && mins <= 0.5
                    && self.f.prayer_paused != Some((day, idx))
                    && self.media.snap.playing
                {
                    self.f.prayer_paused = Some((day, idx));
                    self.media_cmd(MediaCmd::Pause);
                }
            }
            None => self.remove_activity("prayer"),
        }
    }

    fn tick_calendar(&mut self) {
        if !self.settings.calendar.enabled {
            return;
        }
        let stale = self
            .f
            .calendar
            .fetched
            .is_none_or(|f| f.elapsed() > Duration::from_secs(15 * 60));
        let has_source = !self.settings.calendar.ics_urls.is_empty()
            || crate::services::calendar::is_connected(&crate::services::calendar::GOOGLE)
            || crate::services::calendar::is_connected(&crate::services::calendar::MICROSOFT);
        if stale && has_source && !self.f.calendar.fetching && self.settings.privacy.net_calendar {
            self.f.calendar.fetching = true;
            if let Some(bus) = self.svc.bus.clone() {
                crate::services::calendar::refresh(
                    bus,
                    self.settings.calendar.ics_urls.clone(),
                    self.settings.calendar.google_client_id.clone(),
                    self.settings.calendar.microsoft_client_id.clone(),
                );
            }
        }
        let now = unix_secs();
        let next = self
            .f
            .calendar
            .events
            .iter()
            .filter(|e| !e.all_day && e.end > now)
            .min_by_key(|e| e.start)
            .cloned();
        match next {
            Some(e) if e.start - now <= 600 && e.start > now => {
                self.upsert_activity("event", ActKind::Event, None)
            }
            _ => self.remove_activity("event"),
        }
        let rem = self.settings.calendar.reminder_min as i64 * 60;
        for e in self.f.calendar.events.clone() {
            if e.all_day
                || e.start <= now
                || e.start - now > rem
                || self.f.calendar.reminded.contains(&e.id)
            {
                continue;
            }
            self.f.calendar.reminded.insert(e.id.clone());
            let mut p = Popup::new(
                PopKind::Reminder,
                Icon::Calendar,
                e.title.clone(),
                format!("in {} min · {}", (e.start - now + 59) / 60, e.calendar),
            )
            .duration_ms(8000);
            if e.join_url.is_some() {
                p = p.action("Join", Action::CalendarJoin(crate::util::fnv1a(&e.id)));
            }
            self.popup(p);
        }
        // meeting from a calendar event with a link that has started
        if self.f.meeting.is_none() {
            if let Some(e) = self.f.calendar.events.iter().find(|e| {
                e.join_url.is_some() && e.start <= now && e.end > now && now - e.start < 600
            }) {
                if self.f.privacy.uses.iter().any(|u| u.cap == Cap::Mic) {
                    self.f.meeting = Some(Meeting {
                        app: e.title.clone(),
                        started: Instant::now(),
                        from_calendar: true,
                    });
                    self.upsert_activity("call", ActKind::Call, None);
                }
            }
        } else if self.f.meeting.as_ref().is_some_and(|m| m.from_calendar)
            && !self.f.privacy.uses.iter().any(|u| u.cap == Cap::Mic)
        {
            self.f.meeting = None;
            self.remove_activity("call");
        }
    }

    fn tick_weather(&mut self) {
        if !self.settings.weather.enabled || !self.settings.privacy.net_weather {
            return;
        }
        let w = &self.settings.weather;
        if w.lat == 0.0 && w.lon == 0.0 {
            return;
        }
        // in the background at most hourly; pages request a 30-minute refresh
        let stale = self
            .f
            .weather
            .fetched
            .is_none_or(|f| f.elapsed() > Duration::from_secs(3600));
        if stale && !self.f.weather.fetching && self.f.weather.backoff.ready() {
            self.refresh_weather(false);
        }
    }

    pub fn refresh_weather(&mut self, force: bool) {
        let w = &self.settings.weather;
        if let Some(bus) = self.svc.bus.clone() {
            self.f.weather.fetching = true;
            crate::services::weather::fetch(bus, w.lat, w.lon, force);
        }
    }

    pub fn clock_24h(&self) -> bool {
        self.settings
            .general
            .clock_24h
            .unwrap_or_else(crate::util::locale_is_24h)
    }

    // ------------------------------------------------------------ pages

    /// Flips a Control Center switch (the UI springs follow the real state).
    fn control_toggle(&mut self, id: u8) {
        use crate::services::radios::RadioCmd;
        use crate::ui::control::*;
        match id {
            WIFI => {
                let on = !self.sys.radios.wifi.unwrap_or(self.sys.net.wifi);
                self.sys.radios.wifi = Some(on);
                if let Some(tx) = &self.svc.radios {
                    let _ = tx.send(RadioCmd::SetWifi(on));
                }
            }
            BLUETOOTH => {
                let on = !self.sys.radios.bluetooth.unwrap_or(false);
                self.sys.radios.bluetooth = Some(on);
                if let Some(tx) = &self.svc.radios {
                    let _ = tx.send(RadioCmd::SetBluetooth(on));
                }
            }
            FOCUS => {
                self.settings.quiet.focus = !self.settings.quiet.focus;
                self.save_settings_soon();
                self.requests.push(Request::UpdateTray);
            }
            DARK => {
                let dark = !self.sys.dark_mode;
                self.sys.dark_mode = dark;
                // the broadcast can wait on slow windows; keep it off the UI thread
                crate::services::spawn("dark-mode", move || {
                    crate::platform::system::set_dark_mode(dark)
                });
            }
            AWAKE => {
                self.sys.keep_awake = !self.sys.keep_awake;
                crate::platform::system::keep_awake(self.sys.keep_awake);
            }
            MIC => self.audio_cmd(AudioCmd::ToggleMicMute),
            _ => {}
        }
        self.requests.push(Request::Chime(super::Chime::Tap));
        self.dirty = true;
    }

    pub fn on_page_opened(&mut self, p: PageId) {
        if p == PageId::Control {
            self.sys.dark_mode = crate::platform::system::dark_mode();
            if let Some(tx) = &self.svc.radios {
                let _ = tx.send(crate::services::radios::RadioCmd::Refresh);
            }
        }
        self.f.viz_active.store(
            p == PageId::Music && self.settings.motion.visualiser && self.media.snap.playing,
            std::sync::atomic::Ordering::Relaxed,
        );
        self.f.sysmon_active.store(
            p == PageId::SysMon
                || p == PageId::Dashboard
                    && self.settings.widgets.tiles.iter().any(|t| t == "sysmon"),
            std::sync::atomic::Ordering::Relaxed,
        );
        match p {
            PageId::Music if self.settings.music.app_volume => self.audio_cmd(
                AudioCmd::QueryAppVolume(exe_for_app(&self.media.snap.app_id)),
            ),
            PageId::Weather | PageId::Dashboard => {
                if self.settings.weather.enabled
                    && self
                        .f
                        .weather
                        .fetched
                        .is_none_or(|f| f.elapsed() > Duration::from_secs(1800))
                    && !self.f.weather.fetching
                    && (self.settings.weather.lat != 0.0 || self.settings.weather.lon != 0.0)
                {
                    self.refresh_weather(false);
                }
            }
            PageId::Shelf => self.f.shelf.check_missing(),
            PageId::Todo => self.focus_input(InputId::Todo),
            PageId::Ask => self.focus_input(InputId::Ask),
            PageId::Water => {
                let p = &self.settings.productivity;
                self.f
                    .water_fill
                    .set((self.f.state.water.ml as f32 / p.water_goal_ml.max(1) as f32).min(1.0));
            }
            _ => {}
        }
        self.requests.push(Request::Announce(p.title().to_string()));
    }

    pub fn on_collapsed(&mut self) {
        self.f
            .viz_active
            .store(false, std::sync::atomic::Ordering::Relaxed);
        self.f
            .sysmon_active
            .store(false, std::sync::atomic::Ordering::Relaxed);
        if !self.f.ask.pinned_chat {
            self.f.ask.reset();
        }
        self.f.ask.mode = AskMode::Ask;
        self.f.save(true);
        self.requests.push(Request::TrimMemory);
    }

    pub fn focus_input(&mut self, id: InputId) {
        self.input.text_focus = Some(id);
        self.input.caret_blink = Instant::now();
        if !self.input.has_focus {
            self.requests.push(Request::TakeFocus);
        }
    }

    // ------------------------------------------------------------ actions

    /// Throws several items into the trash one after another (visible ones first, in reading order).
    fn throw_all(&mut self, k: crate::features::ListKind, ids: Vec<u64>) {
        let mut seen: Vec<(u64, f32, f32)> = ids
            .iter()
            .filter_map(|id| {
                self.f
                    .item_rects
                    .get(&(k, *id))
                    .map(|(r, _)| (*id, r.y, r.x))
            })
            .collect();
        seen.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.2.total_cmp(&b.2)));
        // the whole volley takes at most ~0.45 s no matter how many items there are
        let gap = if seen.len() > 1 {
            (0.45 / (seen.len() - 1) as f32).min(0.07)
        } else {
            0.0
        };
        for (i, (id, _, _)) in seen.iter().enumerate() {
            self.f.throw(k, *id, i as f32 * gap);
        }
        for id in ids {
            self.f.leave(k, id);
        }
    }

    pub fn dispatch(&mut self, a: Action) {
        self.dirty = true;
        self.last_input = Instant::now();
        if self.settings.sound.ui_sounds
            && !matches!(a, Action::None | Action::Slider(_) | Action::Focus(_))
        {
            self.requests.push(Request::Chime(Chime::Tap));
        }
        let s = self.media.snap.clone();
        match a {
            Action::None => {}
            Action::Open(p) => {
                if self.pop.current.is_some() && !self.expanded {
                    self.pop.dismiss(Instant::now());
                }
                self.open(p);
            }
            Action::PageDot(i) => {
                if let Some(p) = self.carousel().get(i).copied() {
                    self.open(p);
                }
            }
            Action::Collapse => self.collapse(),
            Action::TogglePin => self.pinned = !self.pinned,
            Action::SwapBubble => self.act.swap(),
            Action::OpenSettings => self.requests.push(Request::OpenSettings),
            Action::Media(k) => self.media_cmd(k.to_cmd(s.shuffle, s.repeat)),
            Action::Session(i) => {
                if let Some(sess) = s.sessions.get(i) {
                    self.settings.music.session_choice = sess.app_id.clone();
                    self.media_cmd(MediaCmd::Select(sess.app_id.clone()));
                    self.save_settings_soon();
                }
            }
            Action::Slider(_) => {}
            Action::LyricLine(i) => {
                if let LyricsStatus::Ready(crate::services::lyrics::LyricsResult::Synced(lines)) =
                    &self.lyrics.status
                {
                    if let Some(l) = lines.get(i) {
                        self.media_cmd(MediaCmd::SeekMs(l.0));
                        self.lyrics.scroll.set(i as f32);
                    }
                }
            }
            Action::LyricsTranslate => {
                self.lyrics.show_translation = !self.lyrics.show_translation;
                if self.lyrics.show_translation && self.lyrics.translation.is_none() {
                    self.translate_lyrics();
                }
            }
            Action::PopupButton(id, idx) => {
                let act = self
                    .pop
                    .current
                    .as_ref()
                    .filter(|p| p.id == id)
                    .and_then(|p| p.actions.get(idx as usize).map(|a| a.1.clone()));
                self.pop.dismiss(Instant::now());
                if let Some(a) = act {
                    self.dispatch(a);
                }
            }
            Action::PopupDismiss => {
                self.pop.dismiss(Instant::now());
            }
            Action::Timer(c) => self.timer_cmd(c),
            Action::WaterAdd | Action::WaterDrank => {
                let g = self.settings.productivity.water_glass_ml;
                self.f.state.water.add(g);
                self.after_water();
                if self.f.state.water.ml >= self.settings.productivity.water_goal_ml
                    && self.f.state.water.ml - g < self.settings.productivity.water_goal_ml
                {
                    self.mascot.hearts_until = Some(Instant::now() + Duration::from_secs(3));
                    self.requests.push(Request::Chime(Chime::Success));
                }
            }
            Action::WaterUndo => {
                self.f.state.water.undo();
                self.after_water();
            }
            Action::WaterSnooze => {
                self.f.state.water.snooze(15);
                self.f.state_dirty = true;
                self.f.mark();
            }
            Action::WaterSkip => {}
            Action::TodoToggle(id) => {
                let was_done = self
                    .f
                    .todo
                    .tasks
                    .iter()
                    .find(|t| t.id == id)
                    .map(|t| t.done)
                    .unwrap_or(false);
                self.f.todo.toggle(id);
                if !was_done {
                    let mut sp = Spring::unit(0.0, BOUNCY);
                    sp.set(1.0);
                    self.f.tick_anim.insert(id, sp);
                }
                self.f.todo_dirty = true;
                self.f.mark();
            }
            Action::TodoDelete(id) => self.f.leave(crate::features::ListKind::Todo, id),
            Action::TodoRow(_) | Action::Row(..) => {}
            Action::TodoFilter(i) => {
                self.input.todo_filter = i.min(3);
                self.input.scroll(PageId::Todo).snap(0.0);
            }
            Action::TodoClearDone => {
                let ids: Vec<u64> = self
                    .f
                    .todo
                    .tasks
                    .iter()
                    .filter(|t| t.done)
                    .map(|t| t.id)
                    .collect();
                for id in ids {
                    self.f.leave(crate::features::ListKind::Todo, id);
                }
            }
            Action::Ctl(id) => self.control_toggle(id),
            Action::LockPc => {
                self.collapse();
                crate::platform::system::lock();
            }
            Action::Screenshot => self
                .requests
                .push(Request::StartCapture(super::CaptureMode::Screenshot)),
            Action::TodoSnooze(id) => {
                self.f.todo.snooze(id, 10);
                self.f.todo_dirty = true;
                self.f.mark();
            }
            Action::Focus(id) => self.focus_input(id),
            Action::ShelfSelect(id) => {
                let ctrl =
                    unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(0x11) } < 0;
                if !ctrl {
                    self.f.shelf.selected.clear();
                }
                if !self.f.shelf.selected.insert(id) {
                    self.f.shelf.selected.remove(&id);
                }
            }
            Action::ShelfOpen(id) => {
                if let Some(it) = self.f.shelf.get(id) {
                    match &it.kind {
                        crate::features::store::ShelfKind::Link(u) => {
                            self.requests.push(Request::OpenUrl(u.clone()))
                        }
                        crate::features::store::ShelfKind::Text(t) => {
                            self.requests.push(Request::CopyText(t.clone()))
                        }
                        _ => {
                            if let Some(p) = it.path() {
                                self.requests.push(Request::OpenPath(p));
                            }
                        }
                    }
                }
            }
            Action::ShelfReveal(id) => {
                if let Some(p) = self.f.shelf.get(id).and_then(|i| i.path()) {
                    self.requests.push(Request::RevealPath(p));
                }
            }
            Action::ShelfCopy(id) => {
                if let Some(it) = self.f.shelf.get(id) {
                    match &it.kind {
                        crate::features::store::ShelfKind::Text(t)
                        | crate::features::store::ShelfKind::Link(t) => {
                            self.requests.push(Request::CopyText(t.clone()))
                        }
                        _ => {
                            if let Some(p) = it.path() {
                                self.requests.push(Request::CopyText(p));
                            }
                        }
                    }
                }
            }
            Action::ShelfShare(id) => self.requests.push(Request::ShareShelf(id)),
            Action::ShelfRemove(id) => {
                self.f.shelf.selected.remove(&id);
                self.f.throw(crate::features::ListKind::Shelf, id, 0.0);
            }
            Action::ShelfClear => {
                let ids: Vec<u64> = self.f.shelf.items.iter().map(|i| i.id).collect();
                self.f.shelf.selected.clear();
                self.throw_all(crate::features::ListKind::Shelf, ids);
            }
            Action::ShelfReceive => {
                if let Some(srv) = self.f.phone_server.take() {
                    srv.stop();
                    self.f.phone = Default::default();
                } else if let Some(bus) = self.svc.bus.clone() {
                    self.f.phone_server = crate::services::phone_drop::start(bus, Vec::new());
                }
            }
            Action::ShelfSendPhone => {
                let paths: Vec<std::path::PathBuf> = self
                    .f
                    .shelf
                    .items
                    .iter()
                    .filter(|i| {
                        self.f.shelf.selected.is_empty() || self.f.shelf.selected.contains(&i.id)
                    })
                    .filter_map(|i| i.path())
                    .map(std::path::PathBuf::from)
                    .collect();
                match &self.f.phone_server {
                    Some(srv) => {
                        if let Ok(mut f) = srv.send_files.lock() {
                            *f = paths;
                        }
                    }
                    None => {
                        if let Some(bus) = self.svc.bus.clone() {
                            self.f.phone_server = crate::services::phone_drop::start(bus, paths);
                        }
                    }
                }
            }
            Action::ClipCopy(id) => {
                if let Some(it) = self.f.clip.items.iter().find(|i| i.id == id).cloned() {
                    if it.kind == crate::features::store::ClipKind::Image {
                        self.requests.push(Request::CopyImage(id));
                    } else {
                        self.requests.push(Request::CopyText(it.text.clone()));
                    }
                    if self.settings.productivity.paste_on_select {
                        self.collapse();
                        self.requests.push(Request::PasteToPrevious);
                    } else {
                        self.popup(
                            Popup::new(
                                PopKind::Info,
                                Icon::Copy,
                                "Copied",
                                it.text.chars().take(40).collect::<String>(),
                            )
                            .duration_ms(1200),
                        );
                    }
                }
            }
            Action::ClipPin(id) => {
                self.f.clip.toggle_pin(id);
                self.f.clip_dirty = true;
                self.f.mark();
            }
            Action::ClipDelete(id) => self.f.throw(crate::features::ListKind::Clip, id, 0.0),
            Action::ClipClear => {
                let ids: Vec<u64> = self
                    .f
                    .clip
                    .items
                    .iter()
                    .filter(|i| !i.pinned)
                    .map(|i| i.id)
                    .collect();
                self.throw_all(crate::features::ListKind::Clip, ids);
            }
            Action::OpenUrl(u) => self.requests.push(Request::OpenUrl(u)),
            Action::CopyText(t) => {
                self.requests.push(Request::CopyText(t));
                self.popup(Popup::new(PopKind::Info, Icon::Copy, "Copied", "").duration_ms(1200));
            }
            Action::InstallUpdate => self.requests.push(Request::InstallUpdate),
            Action::PrayerPause => {
                self.settings.productivity.prayer_pause_media =
                    !self.settings.productivity.prayer_pause_media;
                self.save_settings_soon();
            }
            Action::OcrStart => self.requests.push(Request::StartCapture(CaptureMode::Ocr)),
            Action::OcrCopy => {
                if let Some(r) = &self.f.ocr.result {
                    self.requests.push(Request::CopyText(r.text.clone()));
                }
            }
            Action::OcrSearch => {
                if let Some(r) = &self.f.ocr.result {
                    let q: String = r.text.chars().take(200).collect();
                    self.requests.push(Request::OpenUrl(format!(
                        "https://www.bing.com/search?q={}",
                        crate::services::http::enc(&q)
                    )));
                }
            }
            Action::OcrTranslate => {
                if let Some(r) = self.f.ocr.result.clone() {
                    self.open(PageId::Ask);
                    self.f.ask.mode = AskMode::Translate;
                    self.input.text(InputId::Ask).set(&r.text);
                    self.ask_submit();
                }
            }
            Action::OcrOpenUrl(i) => {
                if let Some(r) = &self.f.ocr.result {
                    let all: Vec<String> = r
                        .urls
                        .iter()
                        .cloned()
                        .chain(r.emails.iter().map(|e| format!("mailto:{e}")))
                        .chain(
                            r.phones
                                .iter()
                                .map(|p| format!("tel:{}", p.replace(' ', ""))),
                        )
                        .collect();
                    if let Some(u) = all.get(i as usize) {
                        self.requests.push(Request::OpenUrl(u.clone()));
                    }
                }
            }
            Action::OcrSaveImage => self.requests.push(Request::SaveImage(0)),
            Action::AskSend => self.ask_submit(),
            Action::AskStop => self.f.ask.stop(),
            Action::AskCopy => {
                let t = if self.f.ask.mode == AskMode::Translate {
                    self.f.ask.translation.clone()
                } else {
                    self.f
                        .ask
                        .msgs
                        .iter()
                        .rev()
                        .find(|m| !m.user)
                        .map(|m| m.text.clone())
                        .unwrap_or_default()
                };
                if !t.is_empty() {
                    self.requests.push(Request::CopyText(t));
                }
            }
            Action::AskInsert => {
                let t = self
                    .f
                    .ask
                    .msgs
                    .iter()
                    .rev()
                    .find(|m| !m.user)
                    .map(|m| m.text.clone())
                    .unwrap_or_else(|| self.f.ask.translation.clone());
                if !t.is_empty() {
                    self.requests.push(Request::CopyText(t));
                    self.collapse();
                    self.requests.push(Request::PasteToPrevious);
                }
            }
            Action::AskRetry => {
                if let Some(pos) = self.f.ask.msgs.iter().rposition(|m| m.user) {
                    let q = self.f.ask.msgs[pos].text.clone();
                    self.f.ask.msgs.truncate(pos);
                    self.input.text(InputId::Ask).set(&q);
                    self.ask_submit();
                }
            }
            Action::AskNew => {
                self.f.ask.reset();
                self.focus_input(InputId::Ask);
            }
            Action::AskMode(m) => {
                self.f.ask.mode = match m {
                    1 => AskMode::Translate,
                    2 => AskMode::Palette,
                    _ => AskMode::Ask,
                };
                self.f.ask.error = None;
                self.update_palette();
                self.focus_input(InputId::Ask);
            }
            Action::AskChip(i) => {
                if let Some(c) = self.f.ask.chips.get_mut(i as usize) {
                    *c = !*c;
                }
            }
            Action::AskSwapLang => {
                let t = std::mem::take(&mut self.f.ask.translation);
                if !t.is_empty() {
                    self.input.text(InputId::Ask).set(&t);
                }
            }
            Action::PaletteItem(i) => self.run_palette(i as usize),
            Action::Tile(i) => {
                if let Some(id) = self.settings.widgets.tiles.get(i as usize).cloned() {
                    let p = crate::ui::dashboard::tile_page(&id);
                    self.open(p);
                }
            }
            Action::NotifClear => {
                let ids: Vec<u64> = self.f.notifs.items.iter().map(|n| n.id).collect();
                for id in ids {
                    self.f.leave(crate::features::ListKind::Notif, id);
                }
            }
            Action::NotifEnable => {
                self.requests
                    .push(Request::OpenUrl("ms-settings:privacy-notifications".into()));
            }
            Action::LiveButton(id, action) => {
                if let Some(it) = self.f.live.get(&id) {
                    live_api::send(
                        it.client,
                        &serde_json::json!({"event":"action","id":id,"action":action}).to_string(),
                    );
                }
            }
            Action::LiveAllow(exe, allow) => {
                self.settings.live.api_apps.insert(exe.clone(), allow);
                self.save_settings_soon();
                self.f.live.pending_approval.retain(|(_, e)| *e != exe);
                let queued: Vec<live_api::LiveMsg> = std::mem::take(&mut self.f.live.queued);
                for m in queued {
                    if m.exe.to_lowercase() == exe {
                        if allow {
                            self.on_live(m);
                        }
                    } else {
                        self.f.live.queued.push(m);
                    }
                }
            }
            Action::MeetingMute => {
                let keys = self.meeting_keys(true);
                if keys.is_empty() {
                    self.audio_cmd(AudioCmd::ToggleMicMute);
                } else {
                    self.requests.push(Request::SendKeys(keys));
                }
            }
            Action::MeetingLeave => {
                let keys = self.meeting_keys(false);
                if !keys.is_empty() {
                    self.requests.push(Request::SendKeys(keys));
                }
            }
            Action::MeetingCamera => {
                let app = self
                    .f
                    .meeting
                    .as_ref()
                    .map(|m| m.app.clone())
                    .unwrap_or_default();
                let keys: Vec<u16> = match app.as_str() {
                    "Teams" => vec![0x11, 0x10, 0x4F],
                    "Zoom" => vec![0x12, 0x56],
                    "Google Meet" => vec![0x11, 0x45],
                    _ => vec![],
                };
                if !keys.is_empty() {
                    self.requests.push(Request::SendKeys(keys));
                }
            }
            Action::DownloadOpen(id) | Action::DownloadReveal(id) | Action::DownloadShelf(id) => {
                if let Some(p) = self
                    .f
                    .downloads
                    .items
                    .iter()
                    .find(|d| d.id == id)
                    .and_then(|d| d.path.clone())
                {
                    match a {
                        Action::DownloadOpen(_) => self.requests.push(Request::OpenPath(p)),
                        Action::DownloadReveal(_) => self.requests.push(Request::RevealPath(p)),
                        _ => {
                            self.f.shelf.add_path(&p);
                            self.f.shelf_dirty = true;
                            self.f.mark();
                        }
                    }
                }
            }
            Action::NoteNew => {
                let nid = self.f.notes.new_note();
                self.f.appear(crate::features::ListKind::Note, nid);
                self.input.text(InputId::Note).set("");
                self.f.notes_dirty = true;
                self.focus_input(InputId::Note);
            }
            Action::NoteSelect(id) => {
                self.f.notes.current = Some(id);
                let t = self
                    .f
                    .notes
                    .current()
                    .map(|n| n.text.clone())
                    .unwrap_or_default();
                self.input.text(InputId::Note).set(&t);
                self.focus_input(InputId::Note);
            }
            Action::NotePin(id) => {
                for n in self.f.notes.notes.iter_mut() {
                    n.pinned = n.id == id && !n.pinned;
                }
                if self.f.notes.notes.iter().any(|n| n.pinned) {
                    self.upsert_activity("note", ActKind::Note, None);
                } else {
                    self.remove_activity("note");
                }
                self.f.notes_dirty = true;
                self.f.mark();
            }
            Action::NoteExport(id) => {
                if let Some(n) = self.f.notes.notes.iter().find(|n| n.id == id) {
                    let dir = std::env::var_os("USERPROFILE")
                        .map(|p| std::path::PathBuf::from(p).join("Documents"))
                        .unwrap_or_else(crate::util::data_dir);
                    let name: String = crate::features::store::Notes::first_line(n)
                        .chars()
                        .filter(|c| c.is_alphanumeric() || *c == ' ')
                        .take(40)
                        .collect();
                    let path = dir.join(format!(
                        "{}.md",
                        if name.trim().is_empty() {
                            "Note".into()
                        } else {
                            name.trim().to_string()
                        }
                    ));
                    if std::fs::write(&path, crate::features::store::Notes::to_markdown(n)).is_ok()
                    {
                        self.requests
                            .push(Request::RevealPath(path.to_string_lossy().to_string()));
                    }
                }
            }
            Action::NoteDelete(id) => {
                // the row animates out; it leaves the list when its spring settles
                self.f.leave(crate::features::ListKind::Note, id);
                if self.f.notes.current == Some(id) {
                    self.f.notes.current =
                        self.f.notes.notes.iter().find(|n| n.id != id).map(|n| n.id);
                    let t = self
                        .f
                        .notes
                        .current()
                        .map(|n| n.text.clone())
                        .unwrap_or_default();
                    self.input.text(InputId::Note).set(&t);
                }
                self.f.notes_dirty = true;
                self.f.mark();
            }
            Action::Onboarding(step) => {
                if step >= 3 {
                    self.settings.general.onboarded = true;
                    self.save_settings_soon();
                    self.collapse();
                    self.popup(
                        Popup::new(
                            PopKind::Info,
                            Icon::Sparkles,
                            "You're all set",
                            "Play a song or press a volume key",
                        )
                        .duration_ms(3500),
                    );
                } else {
                    self.onboarding_step = step;
                }
            }
            Action::OnboardingToggle(i) => {
                let p = &mut self.settings.productivity;
                match i {
                    0 => self.settings.general.open_mode = crate::settings::OpenMode::Hover,
                    1 => self.settings.general.open_mode = crate::settings::OpenMode::Click,
                    2 => {
                        self.settings.music.enabled = !self.settings.music.enabled;
                        self.settings.music.lyrics = self.settings.music.enabled;
                    }
                    3 => {
                        let on = !self.settings.popups.volume;
                        let pp = &mut self.settings.popups;
                        pp.volume = on;
                        pp.brightness = on;
                        pp.caps = on;
                        pp.language = on;
                        pp.battery = on;
                        pp.network = on;
                        pp.audio_device = on;
                        pp.bluetooth = on;
                    }
                    4 => p.water = !p.water,
                    5 => p.prayer = !p.prayer,
                    6 => p.clipboard = !p.clipboard,
                    7 => {
                        self.settings.general.start_with_windows =
                            !self.settings.general.start_with_windows;
                    }
                    _ => {}
                }
                self.save_settings_soon();
            }
            Action::Mascot => {
                self.mascot.wink_until = Some(Instant::now() + Duration::from_millis(400));
                self.mascot.bounce.kick(-60.0);
            }
            Action::WeatherRefresh => self.refresh_weather(true),
            Action::CalendarJoin(h) => {
                if let Some(u) = self
                    .f
                    .calendar
                    .events
                    .iter()
                    .find(|e| crate::util::fnv1a(&e.id) == h)
                    .and_then(|e| e.join_url.clone())
                {
                    self.requests.push(Request::OpenUrl(u));
                }
            }
            Action::CalendarRefresh => {
                self.f.calendar.fetched = None;
                self.tick_calendar();
            }
            Action::AnnotateTool(t) => self.f.annotate.tool = t,
            Action::AnnotateUndo => {
                self.f.annotate.shapes.pop();
            }
            Action::AnnotateDone => self.requests.push(Request::CopyImage(u64::MAX)),
            Action::AnnotateSave => self.requests.push(Request::SaveImage(u64::MAX)),
            Action::WidgetToggle(i) => {
                let kinds = crate::ui::widgets::KINDS;
                if let Some(k) = kinds.get(i as usize) {
                    let w = &mut self.settings.widgets.desktop;
                    if let Some(pos) = w.iter().position(|d| d.kind == *k) {
                        w.remove(pos);
                    } else {
                        w.push(crate::settings::DesktopWidget {
                            kind: k.to_string(),
                            x: 80 + 240 * (w.len() as i32 % 4),
                            y: 120 + 160 * (w.len() as i32 / 4),
                        });
                    }
                    self.save_settings_soon();
                    self.requests.push(Request::DesktopWidgets);
                }
            }
            Action::Privacy => self.open(PageId::Privacy),
            Action::ScrollTo(_) | Action::Settings(..) => {}
        }
    }

    fn after_water(&mut self) {
        let p = &self.settings.productivity;
        self.f.state.water.schedule(p);
        self.f
            .water_fill
            .set((self.f.state.water.ml as f32 / p.water_goal_ml.max(1) as f32).min(1.0));
        self.f.state_dirty = true;
        self.f.mark();
    }

    fn meeting_keys(&self, mute: bool) -> Vec<u16> {
        let app = self
            .f
            .meeting
            .as_ref()
            .map(|m| m.app.clone())
            .unwrap_or_default();
        match (app.as_str(), mute) {
            ("Teams", true) => vec![0x11, 0x10, 0x4D],
            ("Teams", false) => vec![0x11, 0x10, 0x48],
            ("Zoom", true) => vec![0x12, 0x41],
            ("Zoom", false) => vec![0x12, 0x51],
            ("Google Meet", true) => vec![0x11, 0x44],
            ("Discord", true) => vec![0x11, 0x10, 0x4D],
            ("Slack", true) => vec![0x11, 0x10, 0x20],
            _ => vec![],
        }
    }

    pub fn timer_cmd(&mut self, c: TimerCmd) {
        let p = self.settings.productivity.clone();
        let t = &mut self.f.state.timer;
        match c {
            TimerCmd::StartFocus => t.start(Phase::Focus, p.focus_min as f64),
            TimerCmd::Pause => t.pause(),
            TimerCmd::Resume => t.resume(),
            TimerCmd::Skip => {
                let next = if t.phase == Phase::Focus {
                    t.next_break(p.long_every)
                } else {
                    Phase::Focus
                };
                let mins = match next {
                    Phase::LongBreak => p.long_break_min,
                    Phase::ShortBreak => p.short_break_min,
                    _ => p.focus_min,
                };
                t.start(next, mins as f64);
            }
            TimerCmd::Reset => {
                t.reset();
                t.session = 0;
            }
            TimerCmd::Preset(m) => t.start(Phase::Custom, m as f64),
            TimerCmd::Plus5 => {
                if t.active() {
                    t.plus(5);
                } else {
                    t.start(t.phase, 5.0);
                }
            }
            TimerCmd::StartBreak => {
                let ph = t.next_break(p.long_every);
                t.start(
                    ph,
                    if ph == Phase::LongBreak {
                        p.long_break_min
                    } else {
                        p.short_break_min
                    } as f64,
                );
            }
        }
        self.f.timer_ring.set(t.progress());
        self.f.state_dirty = true;
        self.f.mark();
        self.tick_timer();
    }

    /// Slider value changes (v in 0..1). `release` is true on mouse-up.
    pub fn on_slider(&mut self, k: SliderKind, v: f32, release: bool) {
        let v = v.clamp(0.0, 1.0);
        match k {
            SliderKind::Seek => {
                self.media.seek_drag = Some(v);
                if release {
                    self.media.seek_drag = None;
                    let ms = (v * self.media.snap.duration_ms as f32) as i64;
                    self.media.snap.position_ms = ms;
                    self.media.snap.updated_ms = crate::util::unix_ms();
                    self.media_cmd(MediaCmd::SeekMs(ms));
                }
            }
            SliderKind::Volume => {
                self.sys.volume = v;
                self.isl.peek_value.set(v);
                self.audio_cmd(AudioCmd::SetVolume(v));
            }
            SliderKind::Brightness => {
                self.sys.brightness = Some((v * 100.0) as u32);
                self.isl.peek_value.set(v);
                if release || self.f.tick_anim.len() < 100 {
                    crate::app::state::set_brightness((v * 100.0) as u32);
                }
            }
            SliderKind::AppVolume => {
                self.media.app_volume = Some(v);
                self.audio_cmd(AudioCmd::SetAppVolume(
                    exe_for_app(&self.media.snap.app_id),
                    v,
                ));
            }
            SliderKind::SettingsSlider(_) => {}
        }
        if let Some(p) = self.pop.current.as_mut() {
            p.shown_at = Some(Instant::now());
        }
        self.dirty = true;
    }

    // ------------------------------------------------------------ Ask

    pub fn ask_submit(&mut self) {
        let text = self.input.text(InputId::Ask).text.trim().to_string();
        if text.is_empty() {
            return;
        }
        // "timer 12m" starts a one-off timer
        if let Some(m) = text
            .strip_prefix("timer ")
            .and_then(crate::features::timer::parse_duration)
        {
            let t = &mut self.f.state.timer;
            t.start(Phase::Custom, m);
            self.f.state_dirty = true;
            self.f.mark();
            self.input.text(InputId::Ask).take();
            self.popup(
                Popup::new(
                    PopKind::Info,
                    Icon::Timer,
                    format!("Timer · {}", crate::util::fmt_mmss((m * 60.0) as i64)),
                    "Started",
                )
                .duration_ms(1500),
            );
            return;
        }
        // palette prefixes
        if let Some(cmd) = text.strip_prefix('>') {
            self.f.ask.mode = AskMode::Palette;
            self.input.text(InputId::Ask).set(cmd.trim_start());
            self.update_palette();
            self.run_palette(self.f.ask.palette_sel);
            return;
        }
        if text.starts_with('=')
            || self.f.ask.mode != AskMode::Translate
                && crate::features::calc::evaluate(&text, &self.f.rates).is_some()
                && !text.starts_with('?')
        {
            if let Some(ans) = crate::features::calc::evaluate(&text, &self.f.rates) {
                self.requests
                    .push(Request::CopyText(ans.text.replace(',', "")));
                self.popup(
                    Popup::new(PopKind::Info, Icon::Calculator, ans.text, "Copied")
                        .duration_ms(1500),
                );
                return;
            }
        }
        if self.f.ask.mode == AskMode::Palette {
            self.run_palette(self.f.ask.palette_sel);
            return;
        }
        let text = text.trim_start_matches('?').trim().to_string();
        let mut context = String::new();
        if self.f.ask.chips[0] {
            if let Some(c) = self.f.clip.items.first() {
                context.push_str(&format!(
                    "Clipboard:\n{}\n\n",
                    c.text.chars().take(4000).collect::<String>()
                ));
            }
        }
        if self.f.ask.chips[1] {
            if let Some(r) = &self.f.ocr.result {
                context.push_str(&format!(
                    "Text from screen:\n{}\n\n",
                    r.text.chars().take(4000).collect::<String>()
                ));
            }
        }
        if self.f.ask.chips[2] && self.media.has_session() {
            context.push_str(&format!(
                "Current song: {} — {}\n\n",
                self.media.snap.title, self.media.snap.artist
            ));
        }
        if self.f.ask.chips[3] {
            if let Some(i) = self
                .f
                .shelf
                .items
                .iter()
                .find(|i| self.f.shelf.selected.contains(&i.id))
                .or(self.f.shelf.items.first())
            {
                let body = match &i.kind {
                    crate::features::store::ShelfKind::Text(t)
                    | crate::features::store::ShelfKind::Link(t) => t.clone(),
                    crate::features::store::ShelfKind::File { path, .. } => {
                        std::fs::read_to_string(path)
                            .map(|s| s.chars().take(6000).collect())
                            .unwrap_or_else(|_| format!("(file {})", i.name))
                    }
                    _ => format!("(image {})", i.name),
                };
                context.push_str(&format!("Shelf item \"{}\":\n{}\n\n", i.name, body));
            }
        }
        self.input.text(InputId::Ask).take();
        let ask = &mut self.f.ask;
        ask.request_id += 1;
        ask.error = None;
        ask.cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        ask.streaming = true;
        let (system, turns) = if ask.mode == AskMode::Translate {
            ask.translation.clear();
            (
                ai::translate_prompt(&self.settings.ai.translate_target),
                vec![(true, text)],
            )
        } else {
            ask.msgs.push(ChatMsg {
                user: true,
                text: format!("{context}{text}"),
            });
            ask.msgs.push(ChatMsg {
                user: false,
                text: String::new(),
            });
            let all: Vec<(bool, String)> = ask
                .msgs
                .iter()
                .filter(|m| !m.text.is_empty())
                .map(|m| (m.user, m.text.clone()))
                .collect();
            let turns: Vec<(bool, String)> = all[all.len().saturating_sub(12)..].to_vec();
            (ai::SYSTEM_PROMPT.to_string(), turns)
        };
        if let Some(bus) = self.svc.bus.clone() {
            ai::ask(
                bus,
                self.settings.ai.clone(),
                self.settings.privacy.net_ai,
                AiRequest {
                    id: ask.request_id,
                    turns,
                    system,
                },
                ask.cancel.clone(),
            );
        }
    }

    /// Translates the whole lyric block once per song (cached next to the lyrics).
    fn translate_lyrics(&mut self) {
        let key = self.lyrics.key;
        let dir = crate::util::sub_dir("cache/lyrics");
        if let Ok(b) = std::fs::read(dir.join(format!("{key:016x}.tr.json"))) {
            if let Ok(lines) = serde_json::from_slice::<Vec<String>>(&b) {
                self.lyrics.translation = Some(lines);
                return;
            }
        }
        let LyricsStatus::Ready(crate::services::lyrics::LyricsResult::Synced(lines)) =
            &self.lyrics.status
        else {
            return;
        };
        let block: Vec<String> = lines.iter().map(|l| l.1.clone()).collect();
        let lang = self.settings.ai.translate_target.clone();
        let system = format!("Translate each line of these song lyrics to {lang}{}. Keep exactly one output line per input line, in order, including empty lines. Output only the lines.", if self.settings.music.lyrics_romanise { " and add a romanised reading in brackets" } else { "" });
        self.f.ask.translation.clear();
        if let Some(bus) = self.svc.bus.clone() {
            ai::ask(
                bus,
                self.settings.ai.clone(),
                self.settings.privacy.net_ai,
                AiRequest {
                    id: u64::MAX - 1,
                    turns: vec![(true, block.join("\n"))],
                    system,
                },
                std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            );
        }
    }

    pub fn update_palette(&mut self) {
        let q = self
            .input
            .text_ref(InputId::Ask)
            .map(|t| t.text.clone())
            .unwrap_or_default();
        self.f.ask.calc = crate::features::calc::evaluate(q.trim_start_matches('='), &self.f.rates);
        if self.f.ask.mode == AskMode::Palette || q.starts_with('>') {
            let q = q.trim_start_matches('>').trim();
            self.f.ask.palette =
                crate::ui::ask::palette_search(q, &self.settings, &self.f.state.palette_freq);
            self.f.ask.palette_sel = 0;
        }
    }

    fn run_palette(&mut self, i: usize) {
        use crate::features::PaletteKind;
        let Some(item) = self.f.ask.palette.get(i).cloned() else {
            return;
        };
        let e = self
            .f
            .state
            .palette_freq
            .entry(item.title.clone())
            .or_insert((0, 0));
        e.0 += 1;
        e.1 = unix_secs();
        self.f.state_dirty = true;
        self.f.mark();
        match item.kind {
            PaletteKind::Launch(p) => {
                self.requests.push(Request::OpenPath(p));
                self.collapse();
            }
            PaletteKind::Url(u) | PaletteKind::Settings(u) => {
                self.requests.push(Request::OpenUrl(u));
                self.collapse();
            }
            PaletteKind::Command(c) => {
                crate::platform::run_command(&c);
                self.collapse();
            }
            PaletteKind::Action(a) => self.dispatch(a),
        }
    }

    // ------------------------------------------------------------ demo

    fn tick_demo(&mut self, start: Instant, step: usize, now: Instant) {
        let due = start + Duration::from_millis(2600 * step as u64);
        if now < due {
            return;
        }
        self.demo = Some((start, step + 1));
        match step % 12 {
            0 => {
                self.popup(Popup::new(PopKind::Volume, Icon::Volume2, "Volume", "48%").value(0.48))
            }
            1 => self.popup(
                Popup::new(PopKind::Brightness, Icon::Brightness, "Brightness", "70%").value(0.7),
            ),
            2 => self.popup(
                Popup::new(PopKind::Caps, Icon::Caps, "Caps Lock on", "").tint(self.accent()[0]),
            ),
            3 => self.popup(
                Popup::new(
                    PopKind::Language,
                    Icon::Keyboard,
                    "Nepali (Nepal)",
                    "Input language",
                )
                .badge("NE"),
            ),
            4 => self.popup(
                Popup::new(
                    PopKind::Battery,
                    Icon::Bolt,
                    "Charging",
                    "64% · 48 min to full",
                )
                .value(0.64)
                .tint(GOOD),
            ),
            5 => self.popup(Popup::new(
                PopKind::Network,
                Icon::Wifi3,
                "Home Wi-Fi",
                "Connected",
            )),
            6 => self.popup(Popup::new(
                PopKind::AudioDevice,
                Icon::Headphones,
                "Headphones",
                "Now playing through",
            )),
            7 => {
                self.timer_cmd(TimerCmd::Preset(1));
            }
            8 => self.open(PageId::Dashboard),
            9 => self.open(PageId::Timer),
            10 => self.collapse(),
            _ => {
                let mut p = Popup::new(
                    PopKind::Notification,
                    Icon::Bell,
                    "Demo notification",
                    "This is how a mirrored toast looks in Notchly.",
                )
                .duration_ms(3000);
                p.source = "Demo".into();
                self.popup(p);
            }
        }
    }
}

fn fmt_dur(secs: u32) -> String {
    if secs >= 3600 {
        format!("{} h {} min", secs / 3600, secs / 60 % 60)
    } else {
        format!("{} min", secs / 60)
    }
}
