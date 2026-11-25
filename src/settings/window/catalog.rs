//! The catalogue of every setting: sections, cards and rows, with their controls.

use super::*;

pub fn sections() -> Vec<Section> {
    vec![
        Section {
            name: "Appearance",
            icon: Icon::Palette,
            tint: TINT_SHELF,
            group: Group::Personalise,
            desc: "Shape, material and colour of the notch",
            reset: Some(reset_appearance),
            preview: true,
            items: vec![
                card("Shape"),
                it(
                    "Style",
                    "Attached melts into the bezel; Floating detaches below it",
                    Ctl::Choice(
                        |_| opts(&["Attached", "Floating"]),
                        |s| (s.settings.appearance.style == Style::Floating) as usize,
                        |s, v| {
                            s.settings.appearance.style = if v == 1 {
                                Style::Floating
                            } else {
                                Style::Attached
                            }
                        },
                    ),
                ),
                it(
                    "Notch size",
                    "Scales everything",
                    Ctl::Choice(
                        |_| opts(&["Auto", "100%", "115%", "130%", "150%"]),
                        |s| {
                            [0, 100, 115, 130, 150]
                                .iter()
                                .position(|v| *v == s.settings.appearance.size)
                                .unwrap_or(0)
                        },
                        |s, v| s.settings.appearance.size = [0, 100, 115, 130, 150][v],
                    ),
                ),
                it(
                    "Minimal notch when idle",
                    "A thin line until something happens",
                    toggle!(appearance.minimal_idle),
                ),
                card("Material"),
                it(
                    "Material",
                    "Liquid Glass bends and blurs what's behind the notch",
                    Ctl::Choice(
                        |_| opts(&["Solid", "Glass", "Liquid Glass"]),
                        |s| match s.settings.appearance.material {
                            Material::Solid => 0,
                            Material::Glass => 1,
                            Material::LiquidGlass => 2,
                        },
                        |s, v| {
                            s.settings.appearance.material = match v {
                                1 => Material::Glass,
                                2 => Material::LiquidGlass,
                                _ => Material::Solid,
                            }
                        },
                    ),
                ),
                when(
                    it(
                        STRENGTH_LABEL,
                        "Clearest, Default or Tinted, and anything between",
                        Ctl::Slider {
                            min: 0.0,
                            max: 100.0,
                            step: 1.0,
                            get: |s| s.settings.appearance.glass.strength * 100.0,
                            set: |s, v| {
                                s.settings.appearance.glass.strength = detent(v / 100.0);
                                custom(s);
                            },
                            fmt: strength_fmt,
                        },
                    ),
                    glass_on,
                ),
                when(
                    it(
                        "Glass style",
                        "",
                        Ctl::Chips(
                            |_| GlassPreset::ALL.iter().map(|p| p.name().to_string()).collect(),
                            |s| {
                                GlassPreset::ALL
                                    .iter()
                                    .position(|p| *p == s.settings.appearance.glass.preset)
                                    .unwrap_or(usize::MAX)
                            },
                            |s, v| {
                                if let Some(p) = GlassPreset::ALL.get(v) {
                                    s.settings.appearance.glass.apply_preset(*p);
                                }
                            },
                        ),
                    ),
                    glass_on,
                ),
                when(it("Glass status", "", Ctl::Info(glass_status)), glass_on),
                card("Colour"),
                it("Mode", "Light mode uses its own theme", super::studio::mode_ctl()),
                it(
                    "Theme",
                    "Accent colours, surfaces and glass",
                    Ctl::Choice(
                        super::studio::theme_names,
                        |s| super::studio::theme_index(s, &s.settings.appearance.theme),
                        |s, v| super::studio::set_mode_theme(s, false, v),
                    ),
                ),
                when(
                    it(
                        "Light-mode theme",
                        "Used while light mode is on",
                        Ctl::Choice(
                            super::studio::theme_names,
                            |s| super::studio::theme_index(s, &s.settings.appearance.theme_light),
                            |s, v| super::studio::set_mode_theme(s, true, v),
                        ),
                    ),
                    |s| s.settings.appearance.theme_mode != ThemeMode::Dark,
                ),
                it(
                    "Edit in Theme Studio",
                    "Colours, glass, shape and sharing",
                    Ctl::Button("Open", |s| {
                        s.settings_nav = Some("Theme Studio");
                        String::new()
                    }),
                ),
                it(
                    "Adaptive colour",
                    "Recolour the glow from the album art",
                    toggle!(appearance.adaptive),
                ),
                card("Details"),
                it(
                    "Privacy dots",
                    "Orange mic, green camera, blue location",
                    toggle!(appearance.privacy_dots),
                ),
                it(
                    "Clock in the idle notch",
                    "Time plus headphones, Bluetooth and charging status",
                    toggle!(appearance.idle_clock),
                ),
                it(
                    "Mascot",
                    "Blink appears after a while idle",
                    toggle!(appearance.mascot),
                ),
                it(
                    "Mascot idle delay",
                    "",
                    slider!(1.0, 30.0, 1.0, mins, appearance.mascot_delay_min),
                ),
            ],
        },
        super::studio::section(),
        Section {
            name: "Glass & Fluid",
            icon: Icon::Waves,
            tint: TINT_SYSTEM,
            group: Group::Personalise,
            desc: "How the glass bends, blurs and moves",
            reset: Some(reset_glass),
            preview: true,
            items: vec![
                card("Fluid style"),
                it("Fluid styles", "", Ctl::Fluid),
                it("Fluid status", "", Ctl::Info(fluid_status)),
                card("Fluid controls"),
                it(
                    "Intensity",
                    "How visible the fluid is",
                    Ctl::Slider {
                        min: 0.0,
                        max: 100.0,
                        step: 5.0,
                        get: |s| s.settings.appearance.fluid.intensity * 100.0,
                        set: |s, v| s.settings.appearance.fluid.intensity = v / 100.0,
                        fmt: pct,
                    },
                ),
                it(
                    "Speed",
                    "On top of the global animation speed",
                    Ctl::Slider {
                        min: 25.0,
                        max: 200.0,
                        step: 5.0,
                        get: |s| s.settings.appearance.fluid.speed * 100.0,
                        set: |s, v| s.settings.appearance.fluid.speed = v / 100.0,
                        fmt: |v| format!("{:.2}×", v / 100.0),
                    },
                ),
                it(
                    "React to music",
                    "Flows faster while something plays",
                    toggle!(appearance.fluid.react_music),
                ),
                it(
                    "React to notifications",
                    "A brief glow and swell when something arrives",
                    toggle!(appearance.fluid.react_notifications),
                ),
                it(
                    "Pause fluid when idle",
                    "Still while the notch rests (no frames, no power)",
                    toggle!(appearance.fluid.pause_idle),
                ),
                card("Advanced glass"),
                when(
                    it(
                        "Glass is off",
                        "",
                        Ctl::Info(|_| {
                            crate::i18n::tr("Choose Glass or Liquid Glass in Appearance to use these")
                        }),
                    ),
                    |s| !glass_on(s),
                ),
                when(
                    it(
                        "Refraction",
                        "How much the edges bend what's behind",
                        Ctl::Slider {
                            min: 0.0,
                            max: 100.0,
                            step: 1.0,
                            get: |s| s.settings.appearance.glass.refraction * 100.0,
                            set: |s, v| {
                                s.settings.appearance.glass.refraction = v / 100.0;
                                custom(s);
                            },
                            fmt: pct,
                        },
                    ),
                    liquid_on,
                ),
                when(
                    it(
                        "Blur",
                        "",
                        Ctl::Slider {
                            min: 0.0,
                            max: 100.0,
                            step: 1.0,
                            get: |s| s.settings.appearance.glass.blur * 100.0,
                            set: |s, v| {
                                s.settings.appearance.glass.blur = v / 100.0;
                                custom(s);
                            },
                            fmt: pct,
                        },
                    ),
                    liquid_on,
                ),
                when(
                    it(
                        "Rim light",
                        "The bright edge facing the light",
                        Ctl::Slider {
                            min: 0.0,
                            max: 100.0,
                            step: 1.0,
                            get: |s| s.settings.appearance.glass.rim_light * 100.0,
                            set: |s, v| {
                                s.settings.appearance.glass.rim_light = v / 100.0;
                                custom(s);
                            },
                            fmt: pct,
                        },
                    ),
                    glass_on,
                ),
                when(
                    it(
                        "Motion response",
                        "How much the glass stretches and ripples as it moves",
                        Ctl::Slider {
                            min: 0.0,
                            max: 100.0,
                            step: 1.0,
                            get: |s| s.settings.appearance.glass.motion_response * 100.0,
                            set: |s, v| s.settings.appearance.glass.motion_response = v / 100.0,
                            fmt: pct,
                        },
                    ),
                    glass_on,
                ),
                when(
                    it(
                        "Glass tint",
                        "Where the glass colour comes from",
                        Ctl::Choice(
                            |_| opts(&["Theme", "Adaptive", "Custom"]),
                            |s| match s.settings.appearance.glass.tint_source {
                                TintSource::Theme => 0,
                                TintSource::Adaptive => 1,
                                TintSource::Custom => 2,
                            },
                            |s, v| {
                                s.settings.appearance.glass.tint_source = match v {
                                    1 => TintSource::Adaptive,
                                    2 => TintSource::Custom,
                                    _ => TintSource::Theme,
                                }
                            },
                        ),
                    ),
                    glass_on,
                ),
                when(
                    it(
                        "Custom tint",
                        "A colour like #1C1C24",
                        textf!(appearance.glass.custom_tint),
                    ),
                    |s| {
                        glass_on(s)
                            && s.settings.appearance.glass.tint_source == TintSource::Custom
                    },
                ),
                when(
                    it(
                        "Light follows the pointer",
                        "The sheen drifts toward the cursor",
                        toggle!(appearance.glass.specular_follows_pointer),
                    ),
                    glass_on,
                ),
                when(
                    it(
                        "Idle shimmer",
                        "A slow sheen every few seconds",
                        toggle!(appearance.glass.idle_shimmer),
                    ),
                    glass_on,
                ),
                when(
                    it(
                        "Show Notchly in screen recordings",
                        "The glass keeps the notch out of screen captures; this shows tinted glass instead",
                        toggle!(appearance.glass.show_in_recordings),
                    ),
                    glass_on,
                ),
                when(
                    it(
                        "Follow Windows transparency",
                        "Solid when transparency effects are off",
                        toggle!(appearance.glass.follow_system_transparency),
                    ),
                    glass_on,
                ),
                when(
                    it(
                        "Solid in High Contrast",
                        "System colours and strong edges",
                        toggle!(appearance.glass.follow_system_contrast),
                    ),
                    glass_on,
                ),
            ],
        },
        Section {
            name: "Motion",
            icon: Icon::Sparkles,
            tint: TINT_CONTROL,
            group: Group::Personalise,
            desc: "The speed and feel of every animation",
            reset: Some(reset_motion),
            preview: false,
            items: vec![
                card("Animation"),
                it(
                    "Animation speed",
                    "Every animation, slower or faster (100% = as designed)",
                    slider!(50.0, 200.0, 10.0, pct, motion.speed_pct),
                ),
                it(
                    "Reduce motion",
                    "No overshoot, no slides; also follows Windows",
                    toggle!(motion.reduce_motion),
                ),
                card("Visualiser"),
                it(
                    "Equaliser source",
                    "Audio follows the real level",
                    Ctl::Choice(
                        |_| opts(&["Audio", "Simulated"]),
                        |s| s.settings.motion.eq_source as usize,
                        |s, v| {
                            s.settings.motion.eq_source = if v == 1 {
                                EqSource::Simulated
                            } else {
                                EqSource::Audio
                            }
                        },
                    ),
                ),
                it(
                    "Audio visualiser",
                    "Bars from the actual sound (only while visible)",
                    toggle!(motion.visualiser),
                ),
                it(
                    "Visualiser ring",
                    "A ring around the album art",
                    toggle!(motion.visualiser_ring),
                ),
            ],
        },
        Section {
            name: "Dashboard & widgets",
            icon: Icon::Widgets,
            tint: TINT_WIDGETS,
            group: Group::Personalise,
            desc: "Home tiles and desktop widgets",
            reset: Some(reset_widgets),
            preview: false,
            items: vec![
                card("Countdown"),
                it(
                    "Countdown title",
                    "For the Countdown desktop widget",
                    textf!(widgets.countdown_title),
                ),
                it(
                    "Countdown date",
                    "YYYY-MM-DD",
                    Ctl::Text(
                        |s| {
                            let d = s.settings.widgets.countdown_day;
                            if d == 0 {
                                String::new()
                            } else {
                                let (y, m, dd) = crate::util::civil_from_days(d);
                                format!("{y:04}-{m:02}-{dd:02}")
                            }
                        },
                        |s, v| {
                            let parts: Vec<i64> =
                                v.trim().split('-').filter_map(|x| x.parse().ok()).collect();
                            s.settings.widgets.countdown_day = match parts.as_slice() {
                                [y, m, d] if (1..=12).contains(m) && (1..=31).contains(d) => {
                                    crate::util::days_from_civil(*y as i32, *m as u32, *d as u32)
                                }
                                _ => 0,
                            };
                        },
                    ),
                ),
                card("Desktop widgets"),
                it(
                    "Desktop: clock",
                    "Floating widget on the desktop (drag to move)",
                    Ctl::Toggle(|s| widget_on(s, 0), |s, v| widget_set(s, 0, v)),
                ),
                it(
                    "Desktop: world clocks",
                    "",
                    Ctl::Toggle(|s| widget_on(s, 1), |s, v| widget_set(s, 1, v)),
                ),
                it(
                    "Desktop: weather",
                    "",
                    Ctl::Toggle(|s| widget_on(s, 2), |s, v| widget_set(s, 2, v)),
                ),
                it(
                    "Desktop: calendar",
                    "",
                    Ctl::Toggle(|s| widget_on(s, 3), |s, v| widget_set(s, 3, v)),
                ),
                it(
                    "Desktop: system monitor",
                    "",
                    Ctl::Toggle(|s| widget_on(s, 4), |s, v| widget_set(s, 4, v)),
                ),
                it(
                    "Desktop: battery",
                    "",
                    Ctl::Toggle(|s| widget_on(s, 5), |s, v| widget_set(s, 5, v)),
                ),
                it(
                    "Desktop: now playing",
                    "",
                    Ctl::Toggle(|s| widget_on(s, 6), |s, v| widget_set(s, 6, v)),
                ),
                it(
                    "Desktop: countdown",
                    "",
                    Ctl::Toggle(|s| widget_on(s, 7), |s, v| widget_set(s, 7, v)),
                ),
                it(
                    "Desktop: sticky note",
                    "",
                    Ctl::Toggle(|s| widget_on(s, 8), |s, v| widget_set(s, 8, v)),
                ),
                it(
                    "Desktop: focus stats",
                    "",
                    Ctl::Toggle(|s| widget_on(s, 9), |s, v| widget_set(s, 9, v)),
                ),
            ],
        },
        Section {
            name: "Live activities",
            icon: Icon::Bolt,
            tint: TINT_TIMER,
            group: Group::Features,
            desc: "What the pill shows while something is happening",
            reset: Some(reset_live),
            preview: false,
            items: vec![
                card("Activities"),
                it(
                    "Dual island",
                    "Show a second activity as a bubble",
                    toggle!(live.dual_island),
                ),
                it(
                    "Music priority",
                    "Higher wins the main pill",
                    Ctl::Slider {
                        min: 10.0,
                        max: 95.0,
                        step: 5.0,
                        get: |s| s.priority_of(crate::app::activity::ActKind::Music) as f32,
                        set: |s, v| {
                            s.settings.live.priority.insert("music".into(), v as i32);
                        },
                        fmt: plain,
                    },
                ),
                it(
                    "Focus timer priority",
                    "",
                    Ctl::Slider {
                        min: 10.0,
                        max: 95.0,
                        step: 5.0,
                        get: |s| s.priority_of(crate::app::activity::ActKind::Focus) as f32,
                        set: |s, v| {
                            s.settings.live.priority.insert("focus".into(), v as i32);
                        },
                        fmt: plain,
                    },
                ),
                it(
                    "Downloads priority",
                    "",
                    Ctl::Slider {
                        min: 10.0,
                        max: 95.0,
                        step: 5.0,
                        get: |s| s.priority_of(crate::app::activity::ActKind::Transfer) as f32,
                        set: |s, v| {
                            s.settings.live.priority.insert("transfer".into(), v as i32);
                        },
                        fmt: plain,
                    },
                ),
                card("Live Activities API"),
                it(
                    "Live Activities API",
                    "Lets apps and scripts post to the notch",
                    toggle!(live.api_enabled),
                ),
                it(
                    "Allowed apps",
                    "",
                    Ctl::Info(|s| {
                        if s.settings.live.api_apps.is_empty() {
                            "No apps yet — the first message from an app asks you".into()
                        } else {
                            s.settings
                                .live
                                .api_apps
                                .iter()
                                .map(|(k, v)| {
                                    format!("{k}: {}", if *v { "allowed" } else { "denied" })
                                })
                                .collect::<Vec<_>>()
                                .join(" · ")
                        }
                    }),
                ),
                it(
                    "Revoke all apps",
                    "",
                    Ctl::Button("Revoke", |s| {
                        s.settings.live.api_apps.clear();
                        "All apps will be asked again".into()
                    }),
                ),
            ],
        },
        Section {
            name: "Pop-ups",
            icon: Icon::Bell,
            tint: TINT_CLIP,
            group: Group::Features,
            desc: "System pop-ups, notifications and sounds",
            reset: Some(reset_popups),
            preview: false,
            items: vec![
                card("System pop-ups"),
                it("Volume", "", toggle!(popups.volume)),
                it("Brightness", "", toggle!(popups.brightness)),
                it("Caps Lock / Num Lock", "", toggle!(popups.caps)),
                it("Input language", "", toggle!(popups.language)),
                it("Battery", "", toggle!(popups.battery)),
                it("Wi-Fi / network", "", toggle!(popups.network)),
                it("Audio device", "", toggle!(popups.audio_device)),
                it("Bluetooth", "", toggle!(popups.bluetooth)),
                it("Microphone mute", "", toggle!(popups.mic)),
                it(
                    "Pop-up duration",
                    "",
                    Ctl::Slider {
                        min: 800.0,
                        max: 4000.0,
                        step: 100.0,
                        get: |s| s.settings.popups.duration_ms as f32,
                        set: |s, v| s.settings.popups.duration_ms = v as u32,
                        fmt: |v| format!("{:.1} s", v / 1000.0),
                    },
                ),
                it(
                    "Low battery at",
                    "",
                    slider!(5.0, 50.0, 1.0, pct, popups.low_battery),
                ),
                it(
                    "Hide the Windows volume flyout",
                    "Opens the Windows setting (Notchly never hides system UI itself)",
                    Ctl::Button("Open", |_| {
                        crate::platform::shell_open("ms-settings:easeofaccess-display");
                        String::new()
                    }),
                ),
                card("Notifications"),
                it(
                    "Notifications",
                    "Mirror Windows notifications",
                    toggle!(popups.notifications),
                ),
                it(
                    "Muted apps",
                    "Never shown or kept. One app name per line",
                    listf!(popups.muted_apps),
                ),
                it(
                    "Quiet apps",
                    "Kept in the history, no pop-up",
                    listf!(popups.quiet_apps),
                ),
                it(
                    "Priority apps",
                    "Pop up even in quiet hours and Focus, for longer",
                    listf!(popups.priority_apps),
                ),
                it(
                    "Keep notification history",
                    "Otherwise cleared on exit",
                    toggle!(popups.keep_history),
                ),
                card("Sounds"),
                it(
                    "Timer chime",
                    "When a timer or focus session ends",
                    toggle!(sound.timer_chime),
                ),
                it(
                    "Reminder chime",
                    "Water, to-do and prayer reminders",
                    toggle!(sound.reminder_chime),
                ),
                it(
                    "Interface sounds",
                    "Soft clicks for buttons and switches",
                    toggle!(sound.ui_sounds),
                ),
            ],
        },
        Section {
            name: "Music",
            icon: Icon::Music,
            tint: TINT_MUSIC,
            group: Group::Features,
            desc: "Now playing, lyrics and the player",
            reset: Some(reset_music),
            preview: false,
            items: vec![
                card("Player"),
                it("Music", "Show what's playing", toggle!(music.enabled)),
                it(
                    "Preferred player",
                    "Favoured when several apps play (e.g. Spotify)",
                    textf!(music.preferred_player),
                ),
                it(
                    "Per-app volume",
                    "A volume slider for the player",
                    toggle!(music.app_volume),
                ),
                card("Lyrics"),
                it("Synced lyrics", "From LRCLIB, with lyrics.ovh as a fallback", toggle!(music.lyrics)),
                it(
                    "Lyrics on the music page",
                    "The CC button on the player does the same",
                    toggle!(music.captions),
                ),
                it(
                    "Lyrics in live view",
                    "The pill shows the current line",
                    toggle!(music.lyrics_live),
                ),
                it(
                    "Lyrics translation",
                    "Translate the whole song once",
                    toggle!(music.lyrics_translate),
                ),
                it(
                    "Romanisation",
                    "Readings for Devanagari, Japanese, Korean…",
                    toggle!(music.lyrics_romanise),
                ),
            ],
        },
        Section {
            name: "Productivity",
            icon: Icon::CheckCircle,
            tint: TINT_TODO,
            group: Group::Features,
            desc: "Focus timer, clipboard, prayer times and tools",
            reset: Some(reset_productivity),
            preview: false,
            items: vec![
                card("Focus timer"),
                it(
                    "Focus length",
                    "",
                    slider!(5.0, 90.0, 5.0, mins, productivity.focus_min),
                ),
                it(
                    "Short break",
                    "",
                    slider!(1.0, 30.0, 1.0, mins, productivity.short_break_min),
                ),
                it(
                    "Long break",
                    "",
                    slider!(5.0, 60.0, 5.0, mins, productivity.long_break_min),
                ),
                it(
                    "Long break every",
                    "sessions",
                    slider!(2.0, 8.0, 1.0, plain, productivity.long_every),
                ),
                card("To-do, shelf and notes"),
                it("To-do reminders", "", toggle!(productivity.todo_reminders)),
                it(
                    "Shelf auto-clear",
                    "0 = never",
                    Ctl::Slider {
                        min: 0.0,
                        max: 30.0,
                        step: 1.0,
                        get: |s| s.settings.productivity.shelf_autoclear_days as f32,
                        set: |s, v| s.settings.productivity.shelf_autoclear_days = v as u32,
                        fmt: |v| {
                            if v == 0.0 {
                                "Never".into()
                            } else {
                                format!("{v:.0} days")
                            }
                        },
                    },
                ),
                it("Quick notes", "", toggle!(productivity.notes)),
                card("Clipboard"),
                it("Clipboard history", "", toggle!(productivity.clipboard)),
                it(
                    "Items kept",
                    "",
                    slider!(10.0, 200.0, 10.0, plain, productivity.clipboard_max),
                ),
                it(
                    "Excluded apps",
                    "Never record copies from these (one per line)",
                    listf!(productivity.clipboard_excluded),
                ),
                it(
                    "Paste on select",
                    "Clicking an item pastes it into the previous app",
                    toggle!(productivity.paste_on_select),
                ),
                card("Prayer times"),
                it("Prayer times", "", toggle!(productivity.prayer)),
                it(
                    "Calculation method",
                    "",
                    Ctl::Choice(
                        |_| {
                            crate::features::prayer::METHODS
                                .iter()
                                .map(|m| m.name.to_string())
                                .collect()
                        },
                        |s| {
                            crate::features::prayer::METHODS
                                .iter()
                                .position(|m| m.id == s.settings.productivity.prayer_method)
                                .unwrap_or(0)
                        },
                        |s, v| {
                            s.settings.productivity.prayer_method =
                                crate::features::prayer::METHODS[v].id.into();
                            s.f.prayer_cache = None;
                        },
                    ),
                ),
                it(
                    "Hanafi Asr",
                    "Later Asr (shadow ×2)",
                    Ctl::Toggle(
                        |s| s.settings.productivity.asr_hanafi,
                        |s, v| {
                            s.settings.productivity.asr_hanafi = v;
                            s.f.prayer_cache = None;
                        },
                    ),
                ),
                it(
                    "Remind before prayer",
                    "",
                    slider!(0.0, 30.0, 1.0, mins, productivity.prayer_reminder_min),
                ),
                it(
                    "Pause media at prayer time",
                    "",
                    toggle!(productivity.prayer_pause_media),
                ),
                card("Tools"),
                it(
                    "OCR language",
                    "Installed Windows OCR languages",
                    Ctl::Choice(
                        |_| {
                            std::iter::once("Automatic".to_string())
                                .chain(
                                    crate::services::ocr::available_languages()
                                        .into_iter()
                                        .map(|l| l.1),
                                )
                                .collect()
                        },
                        |s| {
                            crate::services::ocr::available_languages()
                                .iter()
                                .position(|l| l.0 == s.settings.productivity.ocr_language)
                                .map(|i| i + 1)
                                .unwrap_or(0)
                        },
                        |s, v| {
                            s.settings.productivity.ocr_language = if v == 0 {
                                String::new()
                            } else {
                                crate::services::ocr::available_languages()
                                    .get(v - 1)
                                    .map(|l| l.0.clone())
                                    .unwrap_or_default()
                            }
                        },
                    ),
                ),
                it(
                    "Download tracker",
                    "Follows downloads in your Downloads folder",
                    toggle!(productivity.downloads),
                ),
                it(
                    "Extra download folders",
                    "One folder per line",
                    listf!(productivity.download_folders),
                ),
                it("System monitor", "", toggle!(productivity.sysmon)),
                it(
                    "High CPU alert",
                    "When CPU stays above 90% for 30 s",
                    toggle!(productivity.sysmon_alert),
                ),
            ],
        },
        Section {
            name: "Water",
            icon: Icon::Droplet,
            tint: TINT_WATER,
            group: Group::Features,
            desc: "Your daily goal and gentle reminders",
            reset: Some(reset_water),
            preview: false,
            items: vec![
                card("Reminders"),
                it(
                    "Water reminders",
                    "The same switch as the bell on the Water page",
                    Ctl::Toggle(
                        |s| s.settings.water.reminders,
                        |s, v| {
                            s.settings.water.reminders = v;
                            water_changed(s);
                        },
                    ),
                ),
                it(
                    "Reminder style",
                    "How a reminder shows itself",
                    Ctl::Choice(
                        |_| opts(&["Pop-up", "Pop-up + sound", "Silent glow"]),
                        |s| match s.settings.water.style {
                            ReminderStyle::Popup => 0,
                            ReminderStyle::PopupSound => 1,
                            ReminderStyle::Glow => 2,
                        },
                        |s, v| {
                            s.settings.water.style = match v {
                                0 => ReminderStyle::Popup,
                                2 => ReminderStyle::Glow,
                                _ => ReminderStyle::PopupSound,
                            }
                        },
                    ),
                ),
                it(
                    "Snooze length",
                    "For the Snooze button on a reminder",
                    Ctl::Choice(
                        |_| SNOOZE_CHOICES.iter().map(|m| format!("{m} min")).collect(),
                        |s| {
                            SNOOZE_CHOICES
                                .iter()
                                .position(|m| *m == s.settings.water.snooze_min)
                                .unwrap_or(1)
                        },
                        |s, v| s.settings.water.snooze_min = SNOOZE_CHOICES[v.min(2)],
                    ),
                ),
                it(
                    "Stop reminding once the goal is reached",
                    "Reminders start again tomorrow",
                    Ctl::Toggle(
                        |s| s.settings.water.stop_at_goal,
                        |s, v| {
                            s.settings.water.stop_at_goal = v;
                            water_changed(s);
                        },
                    ),
                ),
                card("Goal"),
                it(
                    "Daily goal",
                    "500 to 6000 ml",
                    Ctl::Stepper {
                        min: 500.0,
                        max: 6000.0,
                        step: 50.0,
                        get: |s| s.settings.water.goal_ml as f32,
                        set: |s, v| {
                            s.settings.water.goal_ml = v as u32;
                            water_changed(s);
                        },
                        fmt: ml,
                    },
                ),
                it(
                    "Glass size",
                    "What one tap on the Water page adds",
                    Ctl::Stepper {
                        min: 50.0,
                        max: 1000.0,
                        step: 25.0,
                        get: |s| s.settings.water.glass_ml as f32,
                        set: |s, v| {
                            s.settings.water.glass_ml = v as u32;
                            water_changed(s);
                        },
                        fmt: ml,
                    },
                ),
                card("Active hours"),
                it(
                    "Reminders from",
                    "",
                    Ctl::Stepper {
                        min: 0.0,
                        max: 1425.0,
                        step: 15.0,
                        get: |s| s.settings.water.start_min as f32,
                        set: |s, v| {
                            let w = &mut s.settings.water;
                            w.start_min = (v as u32).min(w.end_min.saturating_sub(15));
                            water_changed(s);
                        },
                        fmt: hm,
                    },
                ),
                it(
                    "Reminders until",
                    "",
                    Ctl::Stepper {
                        min: 15.0,
                        max: 1440.0,
                        step: 15.0,
                        get: |s| s.settings.water.end_min as f32,
                        set: |s, v| {
                            let w = &mut s.settings.water;
                            w.end_min = (v as u32).max(w.start_min + 15);
                            water_changed(s);
                        },
                        fmt: hm,
                    },
                ),
            ],
        },
        Section {
            name: "Ask & AI",
            icon: Icon::Sparkles,
            tint: TINT_PRAYER,
            group: Group::Features,
            desc: "The command palette and AI providers",
            reset: Some(reset_ai),
            preview: false,
            items: vec![
                card("Provider"),
                it(
                    "Provider",
                    "Auto tries Ollama first, then your keys",
                    Ctl::Choice(
                        |_| {
                            opts(&["Auto", "Ollama (local)", "Gemini", "OpenAI", "Anthropic"])
                        },
                        |s| match s.settings.ai.provider {
                            Provider::Auto | Provider::WindowsAi => 0,
                            Provider::Ollama => 1,
                            Provider::Gemini => 2,
                            Provider::OpenAi => 3,
                            Provider::Anthropic => 4,
                        },
                        |s, v| {
                            s.settings.ai.provider = [
                                Provider::Auto,
                                Provider::Ollama,
                                Provider::Gemini,
                                Provider::OpenAi,
                                Provider::Anthropic,
                            ][v.min(4)]
                        },
                    ),
                ),
                it("Ollama address", "", textf!(ai.ollama_url)),
                it(
                    "Ollama model",
                    "Empty = first installed model",
                    textf!(ai.ollama_model),
                ),
                it("Gemini model", "", textf!(ai.gemini_model)),
                it("OpenAI model", "", textf!(ai.openai_model)),
                it("Anthropic model", "", textf!(ai.anthropic_model)),
                card("API keys"),
                it(
                    "Gemini API key",
                    "Stored encrypted with Windows DPAPI",
                    Ctl::Secret("gemini"),
                ),
                it(
                    "OpenAI API key",
                    "Stored encrypted with Windows DPAPI",
                    Ctl::Secret("openai"),
                ),
                it(
                    "Anthropic API key",
                    "Stored encrypted with Windows DPAPI",
                    Ctl::Secret("anthropic"),
                ),
                card("Commands"),
                it(
                    "Translate to",
                    "Language name, e.g. English, Nepali",
                    textf!(ai.translate_target),
                ),
                it(
                    "Custom commands",
                    "One per line: Name=command or URL",
                    Ctl::List(
                        |s| {
                            s.settings
                                .ai
                                .commands
                                .iter()
                                .map(|c| format!("{}={}", c.name, c.command))
                                .collect()
                        },
                        |s, v| {
                            s.settings.ai.commands = v
                                .iter()
                                .filter_map(|l| {
                                    l.split_once('=').map(|(n, c)| UserCommand {
                                        name: n.trim().into(),
                                        command: c.trim().into(),
                                    })
                                })
                                .collect()
                        },
                    ),
                ),
            ],
        },
        Section {
            name: "Calendar & weather",
            icon: Icon::CloudSun,
            tint: TINT_WEATHER,
            group: Group::Features,
            desc: "Forecast, location and your calendars",
            reset: Some(reset_calendar),
            preview: false,
            items: vec![
                card("Weather"),
                it("Weather", "", toggle!(weather.enabled)),
                it(
                    "Units",
                    "",
                    Ctl::Choice(
                        |_| opts(&["Metric", "Imperial"]),
                        |s| s.settings.weather.units as usize,
                        |s, v| {
                            s.settings.weather.units = if v == 1 {
                                Units::Imperial
                            } else {
                                Units::Metric
                            }
                        },
                    ),
                ),
                card("Location"),
                it(
                    "City",
                    "Used for weather and prayer times",
                    textf!(weather.city),
                ),
                it(
                    "Find city",
                    "Looks the city up (Open-Meteo)",
                    Ctl::Button("Search", |s| {
                        let city = s.settings.weather.city.clone();
                        if city.trim().is_empty() {
                            return "Type a city first".into();
                        }
                        if !s.settings.privacy.net_geocode {
                            return "City search is off in Privacy".into();
                        }
                        if let Some(bus) = s.svc.bus.clone() {
                            crate::services::weather::geocode(bus, city);
                        }
                        "Searching…".into()
                    }),
                ),
                it(
                    "Location",
                    "",
                    Ctl::Info(|s| {
                        let w = &s.settings.weather;
                        if w.lat == 0.0 && w.lon == 0.0 {
                            "Not set".into()
                        } else {
                            format!("{:.2}, {:.2}", w.lat, w.lon)
                        }
                    }),
                ),
                it(
                    "Use Windows location",
                    "Asks Windows for permission",
                    Ctl::Button("Locate", |s| {
                        if let Some(bus) = s.svc.bus.clone() {
                            crate::app_location::locate(bus);
                        }
                        "Locating…".into()
                    }),
                ),
                it(
                    "Keep location updated",
                    "Refresh from Windows each time Notchly starts",
                    toggle!(weather.use_windows_location),
                ),
                card("Calendar"),
                it("Calendar", "", toggle!(calendar.enabled)),
                it(
                    "ICS URLs",
                    "Outlook, Google, iCloud, school timetables… one per line",
                    listf!(calendar.ics_urls),
                ),
                it(
                    "Google client ID",
                    if crate::build_info::GOOGLE_CLIENT_ID.is_some() {
                        "Optional: Notchly's own sign-in is built in"
                    } else {
                        "Your own OAuth desktop client ID (see the user guide)"
                    },
                    textf!(calendar.google_client_id),
                ),
                when(
                    it(
                        "Google client secret",
                        "Only with your own client ID",
                        Ctl::Secret("google_oauth_secret"),
                    ),
                    |s| !s.settings.calendar.google_client_id.trim().is_empty(),
                ),
                when(
                    it(
                        "Google Calendar",
                        "Read-only access to your calendar events",
                        Ctl::Account(|_| account_state(true), |s| oauth_toggle(s, true)),
                    ),
                    |s| can_connect(s, true),
                ),
                when(
                    it(
                        "Google Calendar",
                        "No sign-in in this build",
                        Ctl::Info(|_| ICS_GOOGLE.into()),
                    ),
                    |s| !can_connect(s, true),
                ),
                it(
                    "Microsoft client ID",
                    if crate::build_info::MS_CLIENT_ID.is_some() {
                        "Optional: Notchly's own sign-in is built in"
                    } else {
                        "Your own Microsoft Entra app (public client) ID (see the user guide)"
                    },
                    textf!(calendar.microsoft_client_id),
                ),
                when(
                    it(
                        "Outlook / Microsoft 365",
                        "Read-only access to your calendar",
                        Ctl::Account(|_| account_state(false), |s| oauth_toggle(s, false)),
                    ),
                    |s| can_connect(s, false),
                ),
                when(
                    it(
                        "Outlook / Microsoft 365",
                        "No sign-in in this build",
                        Ctl::Info(|_| ICS_OUTLOOK.into()),
                    ),
                    |s| !can_connect(s, false),
                ),
                it(
                    "Remind before events",
                    "",
                    slider!(0.0, 60.0, 5.0, mins, calendar.reminder_min),
                ),
            ],
        },
        Section {
            name: "General",
            icon: Icon::Settings,
            tint: TINT_SYSTEM,
            group: Group::System,
            desc: "Start-up, opening, language and clock",
            reset: Some(reset_general),
            preview: false,
            items: vec![
                card("Start-up"),
                it(
                    "Start with Windows",
                    "Launch Notchly when you sign in",
                    Ctl::Toggle(
                        |s| s.settings.general.start_with_windows,
                        |s, v| {
                            s.settings.general.start_with_windows = v;
                            crate::platform::set_autostart(v);
                        },
                    ),
                ),
                it(
                    "Display",
                    "Which monitor shows the notch",
                    Ctl::Choice(
                        |_| {
                            let mut v = vec!["Primary display".to_string()];
                            v.extend(crate::platform::monitors().iter().enumerate().map(
                                |(i, m)| {
                                    format!(
                                        "Display {} ({}×{})",
                                        i + 1,
                                        m.rect.right - m.rect.left,
                                        m.rect.bottom - m.rect.top
                                    )
                                },
                            ));
                            v
                        },
                        |s| s.settings.general.monitor as usize,
                        |s, v| s.settings.general.monitor = v as u32,
                    ),
                ),
                it(
                    "Show tray icon",
                    "Settings stay reachable by right-clicking the notch",
                    toggle!(general.show_tray),
                ),
                card("Opening"),
                it(
                    "Open on",
                    "Hover opens after a short dwell; click needs a click",
                    Ctl::Choice(
                        |_| opts(&["Hover", "Click"]),
                        |s| (s.settings.general.open_mode == OpenMode::Click) as usize,
                        |s, v| {
                            s.settings.general.open_mode = if v == 1 {
                                OpenMode::Click
                            } else {
                                OpenMode::Hover
                            }
                        },
                    ),
                ),
                it(
                    "Hover delay",
                    "How long the cursor rests before opening",
                    slider!(50.0, 500.0, 10.0, ms, general.hover_delay_ms),
                ),
                it(
                    "Close delay",
                    "How long after leaving before it closes",
                    slider!(150.0, 1500.0, 50.0, ms, general.close_delay_ms),
                ),
                it(
                    "Long-press time",
                    "How long to hold for a long press (the water bell's menu)",
                    slider!(300.0, 1500.0, 50.0, ms, general.long_press_ms),
                ),
                card("Language and clock"),
                it(
                    "Language",
                    "Interface language",
                    Ctl::Choice(
                        |_| opts(&["English", "हिन्दी", "नेपाली"]),
                        |s| {
                            ["en", "hi", "ne"]
                                .iter()
                                .position(|l| *l == s.settings.general.language)
                                .unwrap_or(0)
                        },
                        |s, v| s.settings.general.language = ["en", "hi", "ne"][v].into(),
                    ),
                ),
                it(
                    "Clock",
                    "12- or 24-hour time",
                    Ctl::Choice(
                        |_| opts(&["Automatic", "12-hour", "24-hour"]),
                        |s| match s.settings.general.clock_24h {
                            None => 0,
                            Some(false) => 1,
                            Some(true) => 2,
                        },
                        |s, v| {
                            s.settings.general.clock_24h = match v {
                                1 => Some(false),
                                2 => Some(true),
                                _ => None,
                            }
                        },
                    ),
                ),
                it(
                    "Nepali date (Bikram Sambat)",
                    "Shown under the clock",
                    toggle!(general.nepali_date),
                ),
                it(
                    "Nepali numerals",
                    "०१२३ instead of 0123",
                    toggle!(general.nepali_numerals),
                ),
                it(
                    "World clocks",
                    "One per line: City=+5:45",
                    Ctl::List(
                        |s| {
                            s.settings
                                .general
                                .world_clocks
                                .iter()
                                .map(|w| {
                                    format!(
                                        "{}={}{}:{:02}",
                                        w.city,
                                        if w.offset_min >= 0 { "+" } else { "-" },
                                        w.offset_min.abs() / 60,
                                        w.offset_min.abs() % 60
                                    )
                                })
                                .collect()
                        },
                        |s, v| {
                            s.settings.general.world_clocks = v
                                .iter()
                                .filter_map(|l| {
                                    let (c, o) = l.split_once('=')?;
                                    let o = o.trim();
                                    let neg = o.starts_with('-');
                                    let (h, m) = o
                                        .trim_start_matches(['+', '-'])
                                        .split_once(':')
                                        .unwrap_or((o.trim_start_matches(['+', '-']), "0"));
                                    let mins =
                                        h.parse::<i32>().ok()? * 60 + m.parse::<i32>().unwrap_or(0);
                                    Some(WorldClock {
                                        city: c.trim().into(),
                                        offset_min: if neg { -mins } else { mins },
                                    })
                                })
                                .take(4)
                                .collect()
                        },
                    ),
                ),
                card("Troubleshooting"),
                it(
                    "Debug logging",
                    "Writes input and camera / microphone events to the log",
                    toggle!(general.debug_log),
                ),
                it(
                    "Diagnostics",
                    "Live input events and camera / microphone detection",
                    Ctl::Button("Open", |s| {
                        s.open(crate::app::PageId::Diagnostics);
                        s.pinned = true;
                        String::new()
                    }),
                ),
            ],
        },
        Section {
            name: "Quiet hours",
            icon: Icon::MoonStar,
            tint: TINT_SHELF,
            group: Group::System,
            desc: "When the notch stays calm",
            reset: Some(reset_quiet),
            preview: false,
            items: vec![
                card("Focus"),
                it(
                    "Focus",
                    "Only calls, timers and critical battery show",
                    toggle!(quiet.focus),
                ),
                it(
                    "Allow incoming calls",
                    "Call notifications still show during quiet hours",
                    toggle!(quiet.allow_calls),
                ),
                it(
                    "Allow timers",
                    "Timer and focus alerts still show",
                    toggle!(quiet.allow_timers),
                ),
                card("Schedule"),
                it("Scheduled quiet hours", "", toggle!(quiet.schedule)),
                it("From", "", slider!(0.0, 1410.0, 30.0, hm, quiet.start_min)),
                it("Until", "", slider!(0.0, 1410.0, 30.0, hm, quiet.end_min)),
                card("Automatic"),
                it(
                    "Hide in fullscreen",
                    "Games, video and presentations",
                    toggle!(quiet.auto_fullscreen),
                ),
                it("Quiet during meetings", "", toggle!(quiet.auto_meetings)),
                it(
                    "Hide content while sharing",
                    "",
                    toggle!(quiet.hide_when_sharing),
                ),
                it(
                    "Never hide for",
                    "Apps (exe) that may stay fullscreen with the notch",
                    listf!(quiet.fullscreen_allow),
                ),
                it(
                    "Always hide for",
                    "Apps (exe) that always hide the notch",
                    listf!(quiet.fullscreen_deny),
                ),
            ],
        },
        Section {
            name: "Shortcuts",
            icon: Icon::Keyboard,
            tint: TINT_CONTROL,
            group: Group::System,
            desc: "Global keyboard shortcuts",
            reset: Some(reset_shortcuts),
            preview: false,
            items: vec![
                card("Shortcuts"),
                it("Open / close Notchly", "", hotkey!(toggle)),
                it("Ask / command palette", "", hotkey!(palette)),
                it("Smart OCR", "", hotkey!(ocr)),
                it("Clipboard history", "", hotkey!(clipboard)),
                it("Start / pause focus", "", hotkey!(focus)),
                it("Translate selection", "", hotkey!(translate)),
                it("Screenshot", "", hotkey!(screenshot)),
            ],
        },
        Section {
            name: "Privacy",
            icon: Icon::Shield,
            tint: TINT_TODO,
            group: Group::System,
            desc: "What leaves this PC, and your data",
            reset: Some(reset_privacy),
            preview: false,
            items: vec![
                card("Screen"),
                it(
                    "Screen behind the notch",
                    "Glass and Liquid Glass read it on the GPU only: never saved, logged or sent",
                    Ctl::Info(|s| {
                        crate::i18n::tr(if s.glass.tier.captures() {
                            "In use (glass)"
                        } else {
                            "Not in use"
                        })
                    }),
                ),
                card("Network"),
                it("Lyrics", "lrclib.net", toggle!(privacy.net_lyrics)),
                it(
                    "Currency rates",
                    "api.frankfurter.app (ECB)",
                    toggle!(privacy.net_currency),
                ),
                it("Weather", "api.met.no", toggle!(privacy.net_weather)),
                it(
                    "City search",
                    "geocoding-api.open-meteo.com",
                    toggle!(privacy.net_geocode),
                ),
                it(
                    "Calendar",
                    "Google, Microsoft Graph, your ICS URLs",
                    toggle!(privacy.net_calendar),
                ),
                it(
                    "Cloud AI",
                    "Only the provider you choose (Ollama stays on this PC)",
                    toggle!(privacy.net_ai),
                ),
                it(
                    "Update check",
                    "Daily, static feed",
                    toggle!(privacy.net_updates),
                ),
                card("Your data"),
                it(
                    "Clear caches",
                    "Lyrics, art, weather, rates",
                    Ctl::Button("Clear", |_| {
                        let _ = std::fs::remove_dir_all(crate::util::data_dir().join("cache"));
                        "Caches cleared".into()
                    }),
                ),
                it(
                    "Clear history",
                    "Clipboard, notifications, privacy log",
                    Ctl::Button("Clear", |s| {
                        s.f.clip.items.clear();
                        s.f.clip_dirty = true;
                        s.f.notifs.items.clear();
                        s.f.state.privacy_log.clear();
                        s.f.state_dirty = true;
                        s.f.save(true);
                        "History cleared".into()
                    }),
                ),
                it(
                    "Crash logs",
                    "Stored locally only",
                    Ctl::Button("Open folder", |_| {
                        crate::platform::shell_open(
                            &crate::util::log_dir().to_string_lossy(),
                        );
                        String::new()
                    }),
                ),
                card("Settings file"),
                it(
                    "Export settings",
                    "",
                    Ctl::Button("Export", |s| {
                        match crate::platform_dialogs::save_file("notchly-settings.toml") {
                            Some(p) => match store::export_to(&p, &s.settings) {
                                Ok(()) => "Exported".into(),
                                Err(e) => e.to_string(),
                            },
                            None => String::new(),
                        }
                    }),
                ),
                it(
                    "Import settings",
                    "",
                    Ctl::Button("Import", |s| {
                        match crate::platform_dialogs::open_file("Settings", "*.toml") {
                            Some(p) => match store::import_from(&p) {
                                Ok(n) => {
                                    s.settings = n;
                                    after_bulk_change(s);
                                    "Imported".into()
                                }
                                Err(e) => e,
                            },
                            None => String::new(),
                        }
                    }),
                ),
                it(
                    "Reset all settings",
                    "Everything back to how Notchly started (your data stays)",
                    Ctl::Confirm("Reset all", |s| {
                        s.settings.reset_all();
                        after_bulk_change(s);
                        crate::i18n::tr("All settings reset")
                    }),
                ),
                it(
                    "Settings sync folder",
                    "Mirror settings, themes, to-dos and notes (e.g. OneDrive)",
                    textf!(sync.folder),
                ),
                it("Sync", "", toggle!(sync.enabled)),
            ],
        },
        Section {
            name: "About",
            icon: Icon::Info,
            tint: TINT_HOME,
            group: Group::System,
            desc: "Version, updates and credits",
            reset: None,
            preview: false,
            items: vec![
                card("Notchly"),
                it(
                    "Version",
                    "",
                    Ctl::Info(|_| format!("Notchly {}", env!("CARGO_PKG_VERSION"))),
                ),
                it(
                    "Updates",
                    "",
                    Ctl::Button("Check now", |s| {
                        if let Some(bus) = s.svc.bus.clone() {
                            crate::updates::check(bus, true);
                        }
                        "Checking…".into()
                    }),
                ),
                it(
                    "Credits",
                    "",
                    Ctl::Info(|_| {
                        "Lyrics: LRCLIB · Weather data: MET Norway · Geocoding: Open-Meteo · Rates: ECB via Frankfurter · Prayer algorithm after the standard astronomical method".into()
                    }),
                ),
                it(
                    "Your data",
                    "Settings, shelf, notes and logs live here",
                    Ctl::Button("Open folder", |_| {
                        crate::platform::shell_open(&crate::util::data_dir().to_string_lossy());
                        String::new()
                    }),
                ),
            ]
            .into_iter()
            .chain(crate::build_info::WEBSITE.map(|_| {
                it(
                    "Website",
                    "",
                    Ctl::Button("Open", |_| {
                        if let Some(w) = crate::build_info::WEBSITE {
                            crate::platform::shell_open(w);
                        }
                        String::new()
                    }),
                )
            }))
            .chain(crate::build_info::SUPPORT_URL.map(|_| {
                it(
                    "Report a problem",
                    "",
                    Ctl::Button("Open", |_| {
                        if let Some(w) = crate::build_info::SUPPORT_URL {
                            crate::platform::shell_open(w);
                        }
                        String::new()
                    }),
                )
            }))
            .collect(),
        },
    ]
}

/// Without a client ID there is nothing a Connect button could do; the ICS link is the
/// zero-setup way to the same calendar.
const ICS_OUTLOOK: &str =
    "Outlook on the web › Settings › Calendar › Shared calendars › Publish a calendar, then paste the ICS link into ICS URLs";
const ICS_GOOGLE: &str =
    "Google Calendar › Settings › your calendar › Secret address in iCal format, then paste it into ICS URLs";

/// Whether a Connect button would work: a built-in or own client ID, or an account that is
/// already connected (so it can be disconnected).
pub(super) fn can_connect(s: &State, google: bool) -> bool {
    use crate::services::calendar::{has_builtin, is_connected, GOOGLE, MICROSOFT};
    let (p, own) = if google {
        (&GOOGLE, &s.settings.calendar.google_client_id)
    } else {
        (&MICROSOFT, &s.settings.calendar.microsoft_client_id)
    };
    has_builtin(p) || !own.trim().is_empty() || is_connected(p)
}

pub(super) fn account_state(google: bool) -> AccountState {
    use crate::services::calendar::{is_connected, signing_in, GOOGLE, MICROSOFT};
    let p = if google { &GOOGLE } else { &MICROSOFT };
    if is_connected(p) {
        AccountState::Connected
    } else if signing_in(p) {
        AccountState::Pending
    } else {
        AccountState::Disconnected
    }
}

pub(super) fn oauth_toggle(s: &mut State, google: bool) -> String {
    use crate::services::calendar::{
        has_builtin, is_connected, set_signing_in, sign_in, sign_out, signing_in, GOOGLE, MICROSOFT,
    };
    let p = if google { &GOOGLE } else { &MICROSOFT };
    if is_connected(p) {
        sign_out(p);
        // drop that account's events right away; the rest refresh
        let source = if google { "google" } else { "outlook" };
        s.f.calendar.events.retain(|e| e.source != source);
        s.f.calendar.fetched = None;
        return "Disconnected".into();
    }
    if signing_in(p) {
        return "Finish signing in in your browser".into();
    }
    let id = if google {
        s.settings.calendar.google_client_id.clone()
    } else {
        s.settings.calendar.microsoft_client_id.clone()
    };
    if id.trim().is_empty() && !has_builtin(p) {
        return "Add a client ID above first".into();
    }
    let bus = s.svc.bus.clone();
    let pid = p.id;
    set_signing_in(p, true);
    crate::services::spawn("oauth", move || {
        crate::services::com_mta();
        let p = if pid == "google" { &GOOGLE } else { &MICROSOFT };
        let r = sign_in(p, &id);
        set_signing_in(p, false);
        if let Some(b) = bus {
            let name = if pid == "google" {
                "Google Calendar"
            } else {
                "Outlook calendar"
            };
            b.send(crate::services::Event::OAuthDone(name.into(), r));
        }
    });
    "Finish signing in in your browser".into()
}
