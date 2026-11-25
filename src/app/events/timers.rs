//! Periodic work driven by tick: timers, water, to-dos, prayer, calendar and weather.

use super::*;

impl State {
    pub(super) fn tick_timer(&mut self) {
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

    pub(super) fn tick_water(&mut self) {
        let p = &self.settings.water;
        let w = &mut self.f.state.water;
        if !p.reminders {
            // switched off (here, in Settings or anywhere): drop the schedule so nothing wakes
            if w.next_reminder != 0 {
                w.reminders_off();
                self.f.state_dirty = true;
            }
            return;
        }
        w.roll();
        if w.paused_today() {
            return;
        }
        if w.next_reminder == 0 {
            w.schedule(p);
            self.f.state_dirty = true;
            return;
        }
        if w.due() {
            let m = local_now().minutes_of_day();
            let in_window = m >= p.start_min && m < p.end_min;
            // reschedule first so the reminder never repeats
            w.snoozed_until = 0;
            w.schedule(p);
            let ml = w.ml;
            self.f.state_dirty = true;
            self.f.mark();
            let p = self.settings.water.clone();
            if in_window && (ml < p.goal_ml || !p.stop_at_goal) {
                // the page's bell rings and its glass sloshes
                self.f.water_bell = Some((Instant::now(), BELL_RING));
                if p.style == ReminderStyle::Glow {
                    // silent: the idle notch glows in the water colour for a moment
                    self.f.water_glow = Some(Instant::now());
                    self.dirty = true;
                    return;
                }
                let sub = format!("{} / {} ml today", ml, p.goal_ml);
                if p.style == ReminderStyle::PopupSound
                    && self.settings.sound.reminder_chime
                    && !self.quiet_hours()
                    && !self.fullscreen_hidden()
                {
                    self.requests.push(Request::Chime(Chime::Reminder));
                }
                let snooze = format!("{} {} m", crate::i18n::tr("Snooze"), p.snooze_min);
                self.popup(
                    Popup::new(PopKind::Reminder, Icon::Droplet, "Time for some water", sub)
                        .action("Drank", Action::WaterDrank)
                        .action(&snooze, Action::WaterSnooze)
                        .action("Skip", Action::WaterSkip)
                        .duration_ms(8000)
                        .priority(50),
                );
            }
        }
    }

    pub(super) fn tick_todo(&mut self) {
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

    pub(super) fn tick_prayer(&mut self) {
        if !self.settings.productivity.prayer {
            self.remove_activity("prayer");
            return;
        }
        let settings = self.settings.clone();
        let Some(times) = self.f.prayer_times(&settings) else {
            return;
        };
        // prayer times are in the city's clock
        let now = crate::util::place_now(&settings.weather.timezone);
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

    pub(super) fn tick_calendar(&mut self) {
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

    pub(super) fn tick_weather(&mut self) {
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
        let Some(bus) = self.svc.bus.clone() else {
            return;
        };
        // a city saved before zones were stored: find its zone once
        if w.timezone.is_empty()
            && !w.city.trim().is_empty()
            && !w.use_windows_location
            && self.settings.privacy.net_geocode
            && !self.f.weather.zone_lookup
        {
            self.f.weather.zone_lookup = true;
            crate::services::weather::lookup_zone(bus.clone(), w.city.clone(), w.lat, w.lon);
        }
        self.f.weather.fetching = true;
        let off = crate::util::place_offset_secs(&w.timezone);
        crate::services::weather::fetch(bus, w.lat, w.lon, off, force);
    }
}
