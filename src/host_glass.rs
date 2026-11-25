//! The notch window's glass and click-through: backdrop capture, fullscreen detection and when clicks pass to the app below.

use super::*;

impl App {
    pub(super) fn check_fullscreen(&mut self) {
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

    /// Re-reads the Windows switches the glass depends on, then re-picks the tier.
    pub(super) fn apply_glass(&mut self) {
        self.glass_env = GlassEnv {
            build: platform::windows_build(),
            transparency: platform::transparency_enabled(),
            remote: platform::remote_session(),
        };
        self.update_glass();
    }

    /// Picks this frame's glass tier. Every tier is drawn by Notchly itself over a transparent
    /// window, so there is no system backdrop and no window region to keep in sync.
    pub(super) fn update_glass(&mut self) {
        let a = &self.st.settings.appearance;
        let gs = a.glass.sanitized();
        let capture_ok = self
            .capture
            .as_ref()
            .is_none_or(|c| !matches!(c.status(), dup::Status::Failed(_)));
        let inputs = glass_backend::Inputs {
            material: a.material,
            build: self.glass_env.build,
            transparency: self.glass_env.transparency,
            // QA (debug builds): NOTCHLY_QA_IGNORE_SAVER lets the capture tiers be tested on a
            // laptop that is in energy saver
            battery_saver: self.st.sys.battery.saver
                && !(cfg!(debug_assertions)
                    && qa_mode()
                    && std::env::var("NOTCHLY_QA_IGNORE_SAVER").is_ok()),
            remote: self.glass_env.remote,
            high_contrast: self.st.high_contrast,
            follow_transparency: gs.follow_system_transparency,
            follow_contrast: gs.follow_system_contrast,
            show_in_recordings: gs.show_in_recordings,
            capture_ok,
            software: self.gfx.dev.as_ref().is_some_and(|d| d.warp),
        };
        let (tier, reason) = glass_backend::select(&inputs);
        if (tier, reason) != (self.st.glass.tier, self.st.glass.reason) {
            log(&format!(
                "glass: {} ({})",
                tier.name(),
                reason.map(|r| r.message()).unwrap_or("as chosen")
            ));
            if trace_glass() {
                log(&format!(
                    "glass trace: win={:?} scale={:.2} capture={:?}",
                    self.win_rect,
                    self.scale,
                    self.capture.as_ref().map(|c| c.status())
                ));
            }
        }
        self.st.glass.tier = tier;
        self.st.glass.reason = reason;
        // capture for the backdrop tiers (and while capture is failing, so it can recover),
        // only while the notch is on screen
        let want_capture = (tier.captures() || reason == Some(Reason::CaptureUnavailable))
            && !self.st.isl.window_hidden;
        if want_capture && self.capture.is_none() {
            let hwnd = self.hwnd.0 as isize;
            self.capture = Some(Capture::start(move || unsafe {
                let _ = PostMessageW(Some(HWND(hwnd as _)), WM_GLASS_FRAME, WPARAM(0), LPARAM(0));
            }));
            self.capture_seen = 0;
        } else if !want_capture && self.capture.is_some() {
            self.capture = None;
            self.receiver = None;
        }
        // the notch must never see itself: excluded from capture whenever we capture
        let exclude = self.capture.is_some();
        if exclude != self.excluded {
            self.excluded = exclude;
            unsafe {
                let _ = SetWindowDisplayAffinity(
                    self.hwnd,
                    if exclude {
                        WDA_EXCLUDEFROMCAPTURE
                    } else {
                        WDA_NONE
                    },
                );
            }
        }
        if tier != self.glass_tier {
            self.glass_tier = tier;
            self.st.dirty = true;
        }
    }

    /// Click-through outside the notch: the window's margins (room for shadows, glows and
    /// tooltips) pass the mouse to whatever is underneath (`WS_EX_TRANSPARENT` on a layered
    /// window). While the pointer sits in a margin we can't receive mouse messages, so a short
    /// timer polls the cursor until it re-enters the notch or leaves the window.
    pub(super) fn set_click_through(&mut self, on: bool) {
        if on == self.click_through {
            return;
        }
        self.click_through = on;
        unsafe {
            let ex = GetWindowLongW(self.hwnd, GWL_EXSTYLE) as u32;
            let ex = if on {
                ex | WS_EX_TRANSPARENT.0
            } else {
                ex & !WS_EX_TRANSPARENT.0
            };
            SetWindowLongW(self.hwnd, GWL_EXSTYLE, ex as i32);
            if on {
                SetTimer(Some(self.hwnd), TIMER_HIT, HIT_POLL_MS, None);
            } else {
                let _ = KillTimer(Some(self.hwnd), TIMER_HIT);
            }
        }
    }

    /// The mouse is busy with the notch (a press, a slider or window drag): never let go.
    pub(super) fn mouse_busy(&self) -> bool {
        self.st.input.pressed.is_some() || self.st.input.slider.is_some()
    }

    /// Re-checks the cursor while click-through is on.
    pub(super) fn poll_click_through(&mut self) {
        if !self.click_through {
            return;
        }
        let mut pt = POINT::default();
        if unsafe { GetCursorPos(&mut pt) }.is_err() {
            return;
        }
        let r = self.win_rect;
        let inside_window = pt.x >= r.0 && pt.y >= r.1 && pt.x < r.0 + r.2 && pt.y < r.1 + r.3;
        if !inside_window {
            // gone: back to normal so the next entry is seen
            self.set_click_through(false);
            return;
        }
        let (cx, cy) = self.to_canvas(pt.x - r.0, pt.y - r.1);
        if self.st.hits(cx, cy) {
            self.set_click_through(false);
            self.st.on_mouse_move(cx, cy);
            unsafe {
                let mut tme = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: self.hwnd,
                    dwHoverTime: 0,
                };
                let _ = TrackMouseEvent(&mut tme);
            }
        }
    }

    /// Liquid tier: tells the capture worker what to read (the canvas, on this display) and
    /// takes its newest frame. Returns the backdrop for this frame.
    pub(super) fn glass_backdrop(&mut self) -> Option<render::glass::Backdrop> {
        self.st.glass.backdrop_ready = false;
        if !self.glass_tier.captures() {
            return None;
        }
        let gen = self.gfx.generation;
        let s = self.scale;
        let dpi = 96.0 * s;
        let dev = self.gfx.device()?;
        let luid = match self.adapter {
            Some((g, l)) if g == gen => l,
            _ => {
                let l = dup::adapter_luid(&dev.dxgi)?;
                self.adapter = Some((gen, l));
                l
            }
        };
        let cap = self.capture.as_ref()?;
        // the canvas origin on screen (see place_window)
        let ox = self.win_rect.0 - self.offset_px.0 as i32;
        let oy = self.win_rect.1 - self.offset_px.1 as i32;
        let (cw, ch) = ((CANVAS_W * s).ceil() as i32, (CANVAS_H * s).ceil() as i32);
        // only changes near the notch itself need a new backdrop
        let m = (GLASS.sigma.1 * 2.0 * s) as i32;
        if self.own_rects[0] != self.win_rect {
            self.own_rects.rotate_right(1);
            self.own_rects[0] = self.win_rect;
        }
        cap.set_request(dup::Request {
            monitor: self.monitor.handle,
            rect: (ox, oy, cw, ch),
            watch: (
                self.win_rect.0 - m,
                self.win_rect.1 - m,
                self.win_rect.2 + 2 * m,
                self.win_rect.3 + 2 * m,
            ),
            own: self.own_rects,
            adapter: luid,
            animating: self.st.was_moving,
            hz: if self.glass_tier == Tier::Frosted {
                GLASS.frosted_hz
            } else {
                GLASS.backdrop_hz_static
            },
        });
        let p = cap.published()?;
        let reopen = self
            .receiver
            .as_ref()
            .is_none_or(|r| r.gen != p.gen || r.dev_gen != gen || r.dpi != dpi);
        if reopen {
            self.receiver = Receiver::open(&dev.d3d, &dev.ctx, p, gen, dpi);
        }
        let r = self.receiver.as_mut()?;
        r.take(&dev.d3d);
        self.st.glass.backdrop_ready = r.ready;
        r.ready.then(|| render::glass::Backdrop {
            bitmap: r.bitmap.clone(),
            slot: 0,
            transform: Some(windows_numerics::Matrix3x2::identity()),
        })
    }
}
