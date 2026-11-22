//! Ask / Translate / command palette page, light Markdown rendering and palette search.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::{row, text_input, Reveal};
use crate::app::state::State;
use crate::app::{Action, InputId, PageId, TimerCmd};
use crate::features::{AskMode, PaletteItem, PaletteKind};
use crate::render::icons::Icon;
use crate::render::text::{self, Align};
use crate::render::{Canvas, Rect};
use crate::settings::Settings;
use crate::theme::tokens::*;

pub fn draw(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    let mode = st.f.ask.mode;
    // tabs
    let tabs = [
        ("Ask", 0u8, Icon::Sparkles),
        ("Translate", 1, Icon::Translate),
        ("Commands", 2, Icon::Search),
    ];
    row(cv, rv, 0, area, |cv| {
        let mut x = area.x;
        for (label, id, icon) in tabs {
            let active = match mode {
                AskMode::Ask => id == 0,
                AskMode::Translate => id == 1,
                AskMode::Palette => id == 2,
            };
            let w = cv.measure(label, &text::CAPTION) + 34.0;
            let r = Rect::new(x, area.y, w, 22.0);
            if active {
                cv.rrect(r, 11.0, cv.theme.surface_raised);
            }
            cv.icon(
                icon,
                r.x + 13.0,
                r.cy(),
                12.0,
                if active {
                    cv.theme.accent[0]
                } else {
                    cv.theme.mist
                },
            );
            cv.text(
                label,
                &text::CAPTION,
                Rect::new(r.x + 23.0, r.y, w - 26.0, r.h),
                if active { cv.theme.ink } else { cv.theme.mist },
                Align::Left,
            );
            cv.zone_named(r, Action::AskMode(id), true, label, 11.0);
            x += w + 4.0;
        }
        let prov = if st.f.ask.provider.is_empty() {
            "Local first · Ollama".to_string()
        } else {
            st.f.ask.provider.clone()
        };
        cv.text(
            &prov,
            &text::CAPTION,
            Rect::new(area.right() - 200.0, area.y, 200.0, 22.0),
            FAINT,
            Align::Right,
        );
    });
    let input = Rect::new(area.x, area.y + 28.0, area.w, 36.0);
    let placeholder = match mode {
        AskMode::Ask => "Ask anything, or type maths like 12% of 340",
        AskMode::Translate => "Text to translate",
        AskMode::Palette => "Search apps, files, settings and actions",
    };
    row(cv, rv, 1, input, |cv| {
        text_input(
            st,
            cv,
            InputId::Ask,
            Rect::new(input.x, input.y, input.w - 44.0, input.h),
            placeholder,
            Some(if mode == AskMode::Palette {
                Icon::Search
            } else {
                Icon::Sparkles
            }),
        );
        if st.f.ask.streaming {
            cv.button(
                Icon::Square,
                input.right() - 18.0,
                input.cy(),
                32.0,
                Action::AskStop,
                "Stop",
                cv.theme.ink,
            );
        } else {
            cv.button(
                Icon::Send,
                input.right() - 18.0,
                input.cy(),
                32.0,
                Action::AskSend,
                "Send",
                cv.theme.accent[0],
            );
        }
    });
    let body = Rect::new(
        area.x,
        input.bottom() + 8.0,
        area.w,
        area.bottom() - input.bottom() - 8.0,
    );
    // live maths result
    if let Some(ans) = st.f.ask.calc.clone() {
        if mode != AskMode::Translate {
            let r = Rect::new(body.x, body.y, body.w, 26.0);
            row(cv, rv, 2, r, |cv| {
                cv.icon(
                    Icon::Calculator,
                    r.x + 10.0,
                    r.cy(),
                    14.0,
                    cv.theme.accent[0],
                );
                cv.text(
                    &format!("= {}", ans.text),
                    &text::TITLE,
                    Rect::new(r.x + 24.0, r.y, r.w - 120.0, r.h),
                    cv.theme.ink,
                    Align::Left,
                );
                cv.text(
                    "Enter copies",
                    &text::CAPTION,
                    Rect::new(r.right() - 100.0, r.y, 100.0, r.h),
                    FAINT,
                    Align::Right,
                );
            });
            if st.f.ask.msgs.is_empty() && mode == AskMode::Ask {
                return;
            }
        }
    }
    match mode {
        AskMode::Palette => palette(st, cv, body, rv),
        AskMode::Translate => {
            let t = st.f.ask.translation.clone();
            row(cv, rv, 2, body, |cv| {
                if t.is_empty() {
                    cv.text(
                        &format!(
                            "Translates to {} · change it in Settings › Ask & AI",
                            st.settings.ai.translate_target
                        ),
                        &text::CAPTION,
                        Rect::new(body.x, body.y, body.w, 18.0),
                        FAINT,
                        Align::Left,
                    );
                } else {
                    cv.text_top(
                        &t,
                        &text::BODY.lines(8),
                        body.x + 4.0,
                        body.y,
                        body.w - 8.0,
                        cv.theme.ink,
                        Align::Left,
                    );
                    let y = body.bottom() - 24.0;
                    cv.pill_button(
                        "Copy",
                        Rect::new(body.x, y, 64.0, 24.0),
                        Action::AskCopy,
                        true,
                    );
                    cv.pill_button(
                        "Swap",
                        Rect::new(body.x + 70.0, y, 64.0, 24.0),
                        Action::AskSwapLang,
                        false,
                    );
                    cv.pill_button(
                        "Insert",
                        Rect::new(body.x + 140.0, y, 64.0, 24.0),
                        Action::AskInsert,
                        false,
                    );
                }
            });
        }
        AskMode::Ask => chat(st, cv, body, rv),
    }
}

fn chat(st: &mut State, cv: &mut Canvas, body: Rect, rv: &Reveal) {
    let msgs = st.f.ask.msgs.clone();
    if msgs.is_empty() && st.f.ask.error.is_none() {
        // context chips
        row(cv, rv, 2, body, |cv| {
            cv.text(
                "Attach context",
                &text::MICRO,
                Rect::new(body.x, body.y, 200.0, 16.0),
                FAINT,
                Align::Left,
            );
            let mut x = body.x;
            for (i, (label, icon)) in [
                ("Clipboard", Icon::Clipboard),
                ("Screen text", Icon::ScanText),
                ("Current song", Icon::Music),
                ("Shelf file", Icon::Shelf),
            ]
            .iter()
            .enumerate()
            {
                let on = st.f.ask.chips[i];
                let w = cv.measure(label, &text::CAPTION) + 34.0;
                let r = Rect::new(x, body.y + 20.0, w, 24.0);
                cv.rrect(
                    r,
                    12.0,
                    if on {
                        cv.theme.accent[1].with_alpha(0.35)
                    } else {
                        LINE
                    },
                );
                cv.icon(
                    *icon,
                    r.x + 13.0,
                    r.cy(),
                    12.0,
                    if on { cv.theme.ink } else { cv.theme.mist },
                );
                cv.text(
                    label,
                    &text::CAPTION,
                    Rect::new(r.x + 23.0, r.y, w - 26.0, r.h),
                    cv.theme.ink,
                    Align::Left,
                );
                cv.zone_named(r, Action::AskChip(i as u8), true, label, 12.0);
                x += w + 6.0;
            }
            cv.text(
                "Tip: > runs commands, = does maths, ? asks AI",
                &text::CAPTION,
                Rect::new(body.x, body.y + 52.0, body.w, 16.0),
                FAINT,
                Align::Left,
            );
        });
        return;
    }
    let scroll = st.f.ask.scroll.value;
    let list = Rect::new(body.x, body.y, body.w, body.h - 30.0);
    cv.push_clip(list);
    let mut y = list.y - scroll;
    for m in &msgs {
        if m.user {
            let shown = m.text.lines().last().unwrap_or("").to_string();
            let w = (cv.measure(&shown, &text::BODY) + 24.0).min(list.w * 0.8);
            let r = Rect::new(list.right() - w, y, w, 28.0);
            cv.sq(r, 14.0, cv.theme.surface_raised);
            cv.text(
                &shown,
                &text::BODY,
                r.inset_xy(12.0, 0.0),
                cv.theme.ink,
                Align::Left,
            );
            y += 34.0;
        } else {
            let h = markdown(cv, &m.text, list.x + 2.0, y, list.w - 4.0);
            y += h + 10.0;
        }
    }
    if let Some(e) = &st.f.ask.error {
        let h = cv.text_top(
            e,
            &text::CAPTION.lines(4),
            list.x,
            y,
            list.w,
            WARN,
            Align::Left,
        );
        y += h + 6.0;
    }
    if st.f.ask.streaming {
        let t = st.started.elapsed().as_secs_f32();
        for i in 0..3 {
            let a = 0.3 + 0.7 * ((t * 5.0 - i as f32 * 0.7).sin() * 0.5 + 0.5);
            cv.circle(
                list.x + 6.0 + i as f32 * 9.0,
                y + 6.0,
                2.5,
                cv.theme.mist.with_alpha(a),
            );
        }
        y += 14.0;
    }
    cv.pop_clip();
    let content = y + scroll - list.y;
    // keep the latest tokens in view while streaming
    let max = (content - list.h).max(0.0);
    if st.f.ask.streaming || st.f.ask.scroll.target > max {
        st.f.ask.scroll.set(max);
    }
    row(cv, rv, 3, body, |cv| {
        let yb = body.bottom() - 24.0;
        let mut x = body.x;
        for (label, a) in [
            ("Copy", Action::AskCopy),
            ("Insert", Action::AskInsert),
            ("Retry", Action::AskRetry),
            ("New chat", Action::AskNew),
        ] {
            let w = cv.measure(label, &text::BODY_MED) + 22.0;
            cv.pill_button(label, Rect::new(x, yb, w, 24.0), a, false);
            x += w + 6.0;
        }
    });
}

/// Light Markdown: paragraphs, **bold**, `code`, - lists, ``` blocks with copy. Returns height.
pub fn markdown(cv: &mut Canvas, md: &str, x: f32, y0: f32, w: f32) -> f32 {
    let mut y = y0;
    let mut in_code = false;
    let mut code = String::new();
    for line in md.lines() {
        if line.trim_start().starts_with("```") {
            if in_code {
                let h = cv
                    .laid(&code, &text::MONO.lines(30), w - 20.0, Align::Left)
                    .map(|l| l.height)
                    .unwrap_or(14.0)
                    + 16.0;
                let r = Rect::new(x, y, w, h);
                cv.sq(r, 10.0, NOTCH.lerp(SURFACE_RAISED, 0.7));
                cv.text_top(
                    &code,
                    &text::MONO.lines(30),
                    x + 10.0,
                    y + 8.0,
                    w - 40.0,
                    cv.theme.ink,
                    Align::Left,
                );
                cv.button(
                    Icon::Copy,
                    r.right() - 14.0,
                    r.y + 14.0,
                    22.0,
                    Action::AskCopy,
                    "Copy code",
                    cv.theme.mist,
                );
                y += h + 6.0;
                code.clear();
            }
            in_code = !in_code;
            continue;
        }
        if in_code {
            if !code.is_empty() {
                code.push('\n');
            }
            code.push_str(line);
            continue;
        }
        let t = line.trim_end();
        if t.is_empty() {
            y += 6.0;
            continue;
        }
        let (bullet, body) = if let Some(r) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* "))
        {
            (true, r)
        } else {
            (false, t)
        };
        let heading = body.starts_with('#');
        let clean: String = body
            .trim_start_matches('#')
            .trim()
            .replace("**", "")
            .replace('`', "");
        let st = if heading || body.starts_with("**") && body.ends_with("**") {
            text::BODY_MED.lines(12)
        } else {
            text::BODY.lines(12)
        };
        let tx = if bullet { x + 14.0 } else { x };
        if bullet {
            cv.circle(x + 5.0, y + 9.0, 2.0, cv.theme.mist);
        }
        let h = cv.text_top(&clean, &st, tx, y, w - (tx - x), cv.theme.ink, Align::Left);
        y += h + 2.0;
    }
    if in_code && !code.is_empty() {
        let h = cv.text_top(
            &code,
            &text::MONO.lines(30),
            x + 10.0,
            y,
            w - 20.0,
            cv.theme.ink,
            Align::Left,
        );
        y += h;
    }
    y - y0
}

fn palette(st: &mut State, cv: &mut Canvas, body: Rect, rv: &Reveal) {
    let items = st.f.ask.palette.clone();
    let sel = st.f.ask.palette_sel;
    if items.is_empty() {
        row(cv, rv, 2, body, |cv| {
            cv.text(
                "Type to search apps, recent files, Windows settings and Notchly actions",
                &text::CAPTION,
                Rect::new(body.x, body.y, body.w, 18.0),
                FAINT,
                Align::Left,
            );
        });
        return;
    }
    cv.push_clip(body);
    for (i, it) in items.iter().take(8).enumerate() {
        let r = Rect::new(body.x, body.y + i as f32 * 32.0, body.w, 30.0);
        row(cv, rv, 2 + i.min(5), r, |cv| {
            let hover = cv.hovered(r);
            if i == sel || hover {
                cv.sq(r, 10.0, cv.theme.surface_raised);
            }
            cv.icon(
                it.icon,
                r.x + 16.0,
                r.cy(),
                15.0,
                if i == sel {
                    cv.theme.accent[0]
                } else {
                    cv.theme.mist
                },
            );
            cv.text(
                &it.title,
                &text::BODY_MED,
                Rect::new(r.x + 34.0, r.y, r.w * 0.55, r.h),
                cv.theme.ink,
                Align::Left,
            );
            cv.text(
                &it.subtitle,
                &text::CAPTION,
                Rect::new(r.x + r.w * 0.5, r.y, r.w * 0.5 - 10.0, r.h),
                FAINT,
                Align::Right,
            );
            cv.zone_named(r, Action::PaletteItem(i as u16), true, &it.title, 10.0);
        });
    }
    cv.pop_clip();
}

// ---------------------------------------------------------------- palette search

/// Fuzzy subsequence score (higher is better), or None if no match.
pub fn fuzzy(query: &str, target: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    let q: Vec<char> = query.to_lowercase().chars().collect();
    let t: Vec<char> = target.to_lowercase().chars().collect();
    let mut score = 0i32;
    let mut qi = 0;
    let mut last: Option<usize> = None;
    for (i, c) in t.iter().enumerate() {
        if qi < q.len() && *c == q[qi] {
            score += 10;
            if last == Some(i.wrapping_sub(1)) {
                score += 15; // consecutive
            }
            if i == 0 || !t[i - 1].is_alphanumeric() {
                score += 20; // word start
            }
            last = Some(i);
            qi += 1;
        }
    }
    if qi < q.len() {
        return None;
    }
    if t.starts_with(&q) {
        score += 50;
    }
    Some(score - (t.len() as i32 / 4))
}

fn start_menu_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(a) = std::env::var_os("APPDATA") {
        v.push(PathBuf::from(a).join(r"Microsoft\Windows\Start Menu\Programs"));
    }
    if let Some(p) = std::env::var_os("ProgramData") {
        v.push(PathBuf::from(p).join(r"Microsoft\Windows\Start Menu\Programs"));
    }
    v
}

fn walk_lnk(dir: &Path, out: &mut Vec<PathBuf>, depth: u32) {
    if depth > 3 || out.len() > 2000 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk_lnk(&p, out, depth + 1);
        } else if p
            .extension()
            .is_some_and(|x| x.eq_ignore_ascii_case("lnk") || x.eq_ignore_ascii_case("url"))
        {
            out.push(p);
        }
    }
}

fn cached_apps() -> Vec<PathBuf> {
    use std::sync::Mutex;
    use std::time::Instant;
    static CACHE: Mutex<Option<(Instant, Vec<PathBuf>)>> = Mutex::new(None);
    let mut c = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((t, v)) = c.as_ref() {
        if t.elapsed().as_secs() < 300 {
            return v.clone();
        }
    }
    let mut v = Vec::new();
    for d in start_menu_dirs() {
        walk_lnk(&d, &mut v, 0);
    }
    *c = Some((Instant::now(), v.clone()));
    v
}

const SETTINGS_PAGES: &[(&str, &str)] = &[
    ("Display settings", "ms-settings:display"),
    ("Sound settings", "ms-settings:sound"),
    ("Bluetooth & devices", "ms-settings:bluetooth"),
    ("Wi-Fi", "ms-settings:network-wifi"),
    ("Battery & power", "ms-settings:batterysaver"),
    ("Notifications", "ms-settings:notifications"),
    ("Focus / Do not disturb", "ms-settings:quiethours"),
    ("Windows Update", "ms-settings:windowsupdate"),
    ("Apps", "ms-settings:appsfeatures"),
    ("Default apps", "ms-settings:defaultapps"),
    ("Language & region", "ms-settings:regionlanguage"),
    ("Date & time", "ms-settings:dateandtime"),
    ("Personalization", "ms-settings:personalization"),
    ("Privacy & security", "ms-settings:privacy"),
    ("Storage", "ms-settings:storagesense"),
    ("Mouse", "ms-settings:mousetouchpad"),
    ("Keyboard", "ms-settings:keyboard"),
    ("Accounts", "ms-settings:yourinfo"),
    ("Night light", "ms-settings:nightlight"),
];

pub fn palette_search(
    q: &str,
    settings: &Settings,
    freq: &BTreeMap<String, (u32, i64)>,
) -> Vec<PaletteItem> {
    let mut out: Vec<(i32, PaletteItem)> = Vec::new();
    let boost = |title: &str| {
        freq.get(title)
            .map(|(n, t)| {
                (*n as i32 * 8).min(80)
                    + if crate::util::unix_secs() - t < 86400 {
                        30
                    } else {
                        0
                    }
            })
            .unwrap_or(0)
    };
    let mut push = |score: Option<i32>, item: PaletteItem| {
        if let Some(s) = score {
            let b = boost(&item.title);
            out.push((s + b, item));
        }
    };
    let actions: Vec<(&str, Action, Icon)> = vec![
        (
            "Start focus timer",
            Action::Timer(TimerCmd::StartFocus),
            Icon::Timer,
        ),
        ("Pause timer", Action::Timer(TimerCmd::Pause), Icon::Pause),
        ("Open shelf", Action::Open(PageId::Shelf), Icon::Shelf),
        (
            "Clipboard history",
            Action::Open(PageId::Clipboard),
            Icon::Clipboard,
        ),
        ("To-do list", Action::Open(PageId::Todo), Icon::CheckCircle),
        ("Capture text (OCR)", Action::OcrStart, Icon::ScanText),
        ("Log a glass of water", Action::WaterAdd, Icon::Droplet),
        ("Weather", Action::Open(PageId::Weather), Icon::CloudSun),
        ("Agenda", Action::Open(PageId::Calendar), Icon::Calendar),
        ("Notes", Action::Open(PageId::Notes), Icon::Note),
        ("System monitor", Action::Open(PageId::SysMon), Icon::Cpu),
        ("Notchly settings", Action::OpenSettings, Icon::Settings),
        (
            "Privacy history",
            Action::Open(PageId::Privacy),
            Icon::Shield,
        ),
        (
            "Notification history",
            Action::Open(PageId::Notifications),
            Icon::Bell,
        ),
        (
            "Desktop widgets",
            Action::Open(PageId::Widgets),
            Icon::Widgets,
        ),
        ("Downloads", Action::Open(PageId::Downloads), Icon::Download),
        ("Take a screenshot", Action::OcrStart, Icon::Crop),
    ];
    for (t, a, ic) in actions {
        push(
            fuzzy(q, t).map(|s| s + 15),
            PaletteItem {
                icon: ic,
                title: t.into(),
                subtitle: "Notchly".into(),
                kind: PaletteKind::Action(a),
            },
        );
    }
    for c in &settings.ai.commands {
        push(
            fuzzy(q, &c.name).map(|s| s + 25),
            PaletteItem {
                icon: Icon::Hammer,
                title: c.name.clone(),
                subtitle: "Command".into(),
                kind: PaletteKind::Command(c.command.clone()),
            },
        );
    }
    if q.is_empty() {
        out.sort_by_key(|x| std::cmp::Reverse(x.0));
        return out.into_iter().take(8).map(|x| x.1).collect();
    }
    for (t, uri) in SETTINGS_PAGES {
        push(
            fuzzy(q, t),
            PaletteItem {
                icon: Icon::Settings,
                title: t.to_string(),
                subtitle: "Windows Settings".into(),
                kind: PaletteKind::Settings(uri.to_string()),
            },
        );
    }
    for p in cached_apps() {
        let name = p
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        if name.to_lowercase().contains("uninstall") {
            continue;
        }
        push(
            fuzzy(q, &name).map(|s| s + 10),
            PaletteItem {
                icon: Icon::Widgets,
                title: name,
                subtitle: "App".into(),
                kind: PaletteKind::Launch(p.to_string_lossy().to_string()),
            },
        );
    }
    if let Some(a) = std::env::var_os("APPDATA") {
        let recent = PathBuf::from(a).join(r"Microsoft\Windows\Recent");
        if let Ok(rd) = std::fs::read_dir(recent) {
            for e in rd.flatten().take(300) {
                let p = e.path();
                if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("lnk")) {
                    let name = p
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    push(
                        fuzzy(q, &name),
                        PaletteItem {
                            icon: Icon::File,
                            title: name,
                            subtitle: "Recent file".into(),
                            kind: PaletteKind::Launch(p.to_string_lossy().to_string()),
                        },
                    );
                }
            }
        }
    }
    if q.starts_with("http") || q.contains('.') && !q.contains(' ') && q.len() > 3 {
        let url = if q.starts_with("http") {
            q.to_string()
        } else {
            format!("https://{q}")
        };
        out.push((
            40,
            PaletteItem {
                icon: Icon::Globe,
                title: url.clone(),
                subtitle: "Open link".into(),
                kind: PaletteKind::Url(url),
            },
        ));
    }
    out.push((
        -100,
        PaletteItem {
            icon: Icon::Search,
            title: format!("Search the web for \"{q}\""),
            subtitle: "Web".into(),
            kind: PaletteKind::Url(format!(
                "https://www.bing.com/search?q={}",
                crate::services::http::enc(q)
            )),
        },
    ));
    out.sort_by_key(|x| std::cmp::Reverse(x.0));
    out.dedup_by(|a, b| a.1.title == b.1.title);
    out.into_iter().take(8).map(|x| x.1).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fuzzy_ranks() {
        assert!(fuzzy("vsc", "Visual Studio Code").is_some());
        assert!(fuzzy("xyz", "Visual Studio Code").is_none());
        assert!(fuzzy("chr", "Chrome").unwrap() > fuzzy("chr", "Microsoft Character Map").unwrap());
    }
    #[test]
    fn actions_searchable() {
        let r = palette_search("focus", &Settings::default(), &BTreeMap::new());
        assert!(r.iter().any(|i| i.title == "Start focus timer"));
    }
}
