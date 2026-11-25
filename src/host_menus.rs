//! The tray icon and its menu, and the notch's right-click menu.

use super::*;

impl App {
    pub(super) fn update_tray(&mut self) {
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

    pub(super) fn tray_tip(&self) -> String {
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

    pub(super) fn context_menu(&mut self) {
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

    pub(super) fn tray_menu(&mut self) {
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
}
