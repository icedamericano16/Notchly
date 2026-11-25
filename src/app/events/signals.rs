//! Reactions to service signals: media, battery, brightness, privacy, meetings, Live Activities, AI and downloads.

use super::*;

impl State {
    pub(super) fn on_media(&mut self, snap: crate::services::media::MediaSnapshot) {
        let track_changed = snap.art_key != self.media.snap.art_key;
        // the level meter runs only while something plays
        self.f
            .meter_active
            .store(snap.playing, std::sync::atomic::Ordering::Relaxed);
        if track_changed {
            self.lyrics.auto_delay_ms = 0;
            self.lyrics.probe = None;
        } else if self.lyrics.auto_delay_ms > 0 && snap.playing && self.media.snap.playing {
            // the player corrected its own clock (e.g. after buffering): take that much back
            // out of the auto-sync delay so it isn't applied twice
            let now = crate::util::unix_ms();
            let ours = self.media.position_ms();
            let theirs = snap.position_ms + (now - snap.updated_ms).max(0);
            let jump = ours - theirs;
            if jump > 300 && jump <= self.lyrics.auto_delay_ms + 600 {
                self.lyrics.auto_delay_ms = (self.lyrics.auto_delay_ms - jump).max(0);
            }
        }
        if !self.media.snap.playing && snap.playing && snap.position_ms < 1500 && !track_changed {
            // played from the top again
            self.lyrics.auto_delay_ms = 0;
            self.lyrics.probe = Some(Default::default());
        }
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
                self.lyrics.pick = 0;
                self.lyrics.meta = Default::default();
                if self.settings.music.lyrics {
                    self.request_lyrics(false);
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
            && !(self.expanded && self.page == PageId::Control)
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

    pub(super) fn on_privacy(&mut self, uses: Vec<crate::services::privacy::Use>) {
        let now = unix_secs();
        // apps that let go are no longer "closing"
        self.f
            .privacy
            .closing
            .retain(|k, _| uses.iter().any(|u| &u.key == k));
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
        if uses.is_empty() {
            self.f.privacy.since = None;
        } else if self.f.privacy.uses.is_empty() {
            self.f.privacy.since = Some(Instant::now());
        }
        self.f.privacy.uses = uses;
        self.check_meeting(Instant::now());
    }

    /// Meeting detection (features/meeting.rs): a desktop call app holding the microphone, or
    /// a browser holding it while one of its windows shows a call, in front or not. Runs on
    /// every privacy change and from `tick` (the end-of-call grace period, and a browser that
    /// took the microphone before its call window got its title).
    pub(crate) fn check_meeting(&mut self, now: Instant) {
        use crate::features::meeting;
        // name and exe path: live detection reports friendly names ("Microsoft Edge")
        let mic: Vec<String> = self
            .f
            .privacy
            .uses
            .iter()
            .filter(|u| u.cap == Cap::Mic)
            .map(|u| format!("{} {}", u.app, u.key))
            .collect();
        let native = mic.iter().find_map(|m| meeting::native_app(m));
        let browser_mic = mic.iter().any(|m| meeting::is_browser(m));
        let in_browser = if native.is_none() && browser_mic {
            crate::platform::window_titles(meeting::is_browser)
                .iter()
                .find_map(|t| meeting::provider_from_title(t))
        } else {
            None
        };
        let seen = meeting::Seen {
            detected: native.or(in_browser).map(String::from),
            in_browser: native.is_none() && in_browser.is_some(),
            browser_mic,
        };
        // a browser holds the mic but no window looks like a call (yet): look again soon
        self.f.meeting_recheck =
            (browser_mic && seen.detected.is_none()).then(|| now + Duration::from_secs(2));
        let calendar_call = self.f.meeting.as_ref().is_some_and(|m| m.from_calendar);
        match self.f.meeting_watch.update(&seen, now) {
            meeting::Change::Started(app) if !calendar_call => {
                self.f.meeting = Some(Meeting {
                    app,
                    started: now,
                    from_calendar: false,
                });
                self.upsert_activity("call", ActKind::Call, None);
                if self.settings.quiet.auto_meetings {
                    self.sup.meeting_quiet = true;
                }
            }
            meeting::Change::Renamed(app) => {
                if let Some(m) = self.f.meeting.as_mut().filter(|m| !m.from_calendar) {
                    m.app = app;
                }
            }
            meeting::Change::Ended if !calendar_call => {
                self.f.meeting = None;
                self.remove_activity("call");
                self.sup.meeting_quiet = false;
            }
            _ => {}
        }
    }

    pub(super) fn on_live(&mut self, m: live_api::LiveMsg) {
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

    pub(super) fn on_ai(&mut self, ev: AiEvent) {
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

    pub(super) fn on_download(&mut self, d: DownloadEvent) {
        if !self.settings.productivity.downloads {
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
}

fn fmt_dur(secs: u32) -> String {
    if secs >= 3600 {
        format!("{} h {} min", secs / 3600, secs / 60 % 60)
    } else {
        format!("{} min", secs / 60)
    }
}
