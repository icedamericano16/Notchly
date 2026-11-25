//! Carrying out what the UI asked for (Request), the clipboard watcher, drag-and-drop results and screen captures.

use super::*;

impl App {
    pub(super) fn capture(&mut self, mode: CaptureMode) {
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

    pub(super) fn on_clipboard(&mut self) {
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

    pub(super) fn process_requests(&mut self) {
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

    pub(super) fn drain_drops(&mut self) {
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

    pub(super) fn after_drop(&mut self, ids: Vec<u64>) {
        // the notch jiggles like jelly as it swallows the drop; the shelf celebrates
        self.st.isl.swell.set_cfg(0.0, motion::BOUNCY);
        self.st.isl.swell.kick(-4.0);
        if !ids.is_empty() {
            self.st.f.drop_fx = Some((std::time::Instant::now(), ids));
            self.st.f.drop_started = false;
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
}
