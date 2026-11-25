//! The other windows: Settings and desktop widgets.

use super::*;

impl App {
    pub(super) fn render_settings(&mut self, dt: f32) {
        let Some(sw) = self.settings_win.as_mut() else {
            return;
        };
        let moving = sw.step(dt, &self.st);
        sw.apply_pending(&mut self.st);
        #[cfg(debug_assertions)]
        if let Some(p) = QA_SETTINGS_DUMP.with(|d| d.take()) {
            self.gfx.dump = Some(p.into());
            sw.dirty = true;
        }
        if sw.dirty || moving {
            sw.draw(&mut self.st, &mut self.gfx);
        }
    }

    pub(super) fn render_widgets(&mut self) {
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

    pub(super) fn sync_widgets(&mut self) {
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

    pub(super) fn open_settings(&mut self) {
        if let Some(sw) = &self.settings_win {
            unsafe {
                let _ = ShowWindow(sw.hwnd, SW_RESTORE);
                let _ = SetForegroundWindow(sw.hwnd);
            }
            return;
        }
        let dpi = self.monitor.dpi as f32;
        let s = dpi / 96.0;
        let m = &self.monitor;
        let (ww, wh) = (m.work.right - m.work.left, m.work.bottom - m.work.top);
        let w = ((theme::tokens::SET_W * s) as i32).min(ww);
        let h = ((theme::tokens::SET_H * s) as i32).min(wh);
        let mut x = m.work.left + (ww - w) / 2;
        let mut y = m.work.top + (wh - h) / 2;
        let (mut w, mut h) = (w, h);
        // where it was last time on this monitor, if that's still on screen
        if let Some([px, py, pw, ph]) = settings::store::window_place(&m.key) {
            let fits = pw >= (theme::tokens::SET_MIN_W * s * 0.9) as i32
                && ph >= (theme::tokens::SET_MIN_H * s * 0.9) as i32
                && px + pw / 2 > m.work.left
                && px + pw / 2 < m.work.right
                && py >= m.work.top - 8
                && py + 40 < m.work.bottom;
            if fits {
                (x, y, w, h) = (px, py, pw.min(ww), ph.min(wh));
            }
        }
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

    pub(super) fn is_settings(&self, h: HWND) -> bool {
        self.settings_win.as_ref().is_some_and(|s| s.hwnd == h)
    }
}
