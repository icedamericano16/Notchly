//! Music, with lyrics built in.
//!
//! The caption button (CC) shows or hides the lyrics panel. With it on, the page grows: a
//! compact player on top (art, title, wavy seek bar, transport) and a lyrics stage below,
//! washed in the album art's colours with slow aurora light. Synced lyrics sweep word by word
//! across the current line, neighbours fade and shrink with distance, interludes breathe as
//! three dots, and any line can be clicked to jump there. Plain lyrics auto-scroll with the
//! song. Tools: source badge, translation, timing nudge (−/+ 0.25 s), next match, web search.

use super::{row, Reveal};
use crate::app::state::{LyricsStatus, State};
use crate::app::{Action, MediaCmdKey, SliderKind};
use crate::render::icons::Icon;
use crate::render::text::{self, Align, Family, TextStyle};
use crate::render::{Canvas, Rect};
use crate::services::lyrics::LyricsResult;
use crate::theme::tokens::*;
use crate::util::fmt_mmss;

pub fn music(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    if !st.media.has_session() {
        empty(cv, area, rv, st.started.elapsed().as_secs_f32());
        return;
    }
    let s = st.media.snap.clone();
    let wave_target = if s.playing && !st.env.reduced {
        1.0
    } else {
        0.0
    };
    if (st.f.wave_amp.target - wave_target).abs() > 0.5 {
        st.f.wave_amp.set(wave_target);
    }
    if st.settings.music.captions && area.h > 220.0 {
        let player = Rect::new(area.x, area.y, area.w, 74.0);
        compact_player(st, cv, player, rv);
        let stage = Rect::new(
            area.x,
            player.bottom() + 8.0,
            area.w,
            area.bottom() - player.bottom() - 8.0,
        );
        row(cv, rv, 3, stage, |cv| lyrics_stage(st, cv, stage));
    } else {
        full_player(st, cv, area, rv);
    }
}

fn empty(cv: &mut Canvas, area: Rect, rv: &Reveal, t: f32) {
    row(cv, rv, 0, area, |cv| {
        // a note that floats gently
        let bob = (t * 1.6).sin() * 3.0;
        cv.circle(
            area.cx(),
            area.cy() - 18.0 + bob,
            22.0,
            TINT_MUSIC.with_alpha(0.14),
        );
        cv.icon(
            Icon::Music,
            area.cx(),
            area.cy() - 18.0 + bob,
            22.0,
            TINT_MUSIC,
        );
        cv.label(
            "Nothing playing",
            &text::TITLE,
            Rect::new(area.x, area.cy() + 10.0, area.w, 20.0),
            cv.theme.ink,
            Align::Center,
        );
        cv.label(
            "Play something in Spotify, a browser or any media app",
            &text::CAPTION,
            Rect::new(area.x, area.cy() + 30.0, area.w, 16.0),
            cv.theme.mist,
            Align::Center,
        );
    });
}

/// Title and artist to show: when the lyrics engine identified the song (YouTube titles like
/// "Song - Artist" uploaded by a channel), its match reads better than the raw metadata.
fn display_names(st: &State) -> (String, String) {
    let s = &st.media.snap;
    let m = &st.lyrics.meta;
    if m.source == "LRCLIB" && m.alt == 0 {
        if let Some((t, a)) = m.matched.split_once(" — ") {
            let t = crate::services::lyrics::strip_noise(t);
            let raw = s.raw_title.to_lowercase();
            // only when the match really is this video (its words are in the raw title)
            let words = crate::services::lyrics::tokens(&t);
            if !words.is_empty() && words.iter().all(|w| raw.contains(w.as_str())) {
                let artist = if s.artist.is_empty() || a.eq_ignore_ascii_case(&s.artist) {
                    a.to_string()
                } else {
                    format!("{a} · {}", s.artist)
                };
                return (t, artist);
            }
        }
    }
    (s.title.clone(), s.artist.clone())
}

/// The CC button: lit when the lyrics panel is on.
fn caption_button(st: &State, cv: &mut Canvas, cx: f32, cy: f32) {
    let on = st.settings.music.captions;
    let r = Rect::centered(cx, cy, 30.0, 22.0);
    let a = Action::LyricsToggle;
    let (hv, pv) = cv.anim(&a, cv.hovered(r), false);
    cv.push_tf(
        0.0,
        0.0,
        (1.0 + 0.05 * hv.min(1.0)) * (1.0 - 0.08 * pv),
        cx,
        cy,
    );
    cv.rrect(
        r,
        7.0,
        if on {
            WHITE.with_alpha(0.92)
        } else {
            LINE.with_alpha(1.0 + hv.min(1.0))
        },
    );
    cv.text(
        "CC",
        &text::CAPTION.weight(800).size(10.5),
        r,
        if on { NOTCH } else { cv.theme.mist },
        Align::Center,
    );
    cv.pop_tf();
    cv.zone_named(
        r,
        a,
        true,
        if on { "Hide lyrics" } else { "Show lyrics" },
        7.0,
    );
}

/// App chip (switches between players when there are several).
fn app_chip(st: &State, cv: &mut Canvas, right: f32, y: f32) -> f32 {
    let s = &st.media.snap;
    let multi = s.sessions.len() > 1;
    let chip_w = cv.measure(&s.app_name, &text::CAPTION) + if multi { 34.0 } else { 18.0 };
    let chip = Rect::new(right - chip_w, y, chip_w, 20.0);
    cv.rrect(chip, 10.0, LINE);
    cv.text(
        &s.app_name,
        &text::CAPTION,
        Rect::new(chip.x + 9.0, chip.y, chip.w - 18.0, chip.h),
        cv.theme.mist,
        Align::Left,
    );
    if multi {
        cv.icon(
            Icon::ChevronDown,
            chip.right() - 12.0,
            chip.cy(),
            11.0,
            cv.theme.mist,
        );
        let next = s.active.map(|a| (a + 1) % s.sessions.len()).unwrap_or(0);
        cv.zone_named(chip, Action::Session(next), true, "Switch player", 10.0);
    }
    chip_w
}

fn art(st: &mut State, cv: &mut Canvas, r: Rect, skip: bool) {
    if skip {
        return;
    }
    match st.media.art.as_mut() {
        Some(img) => cv.image(img, r, R_ART),
        None => {
            cv.sq(r, R_ART, SURFACE_RAISED);
            cv.icon(Icon::Music, r.cx(), r.cy(), r.w * 0.36, cv.theme.mist);
        }
    }
}

fn seek_bar(st: &State, cv: &mut Canvas, pr: Rect, times: bool) {
    let s = &st.media.snap;
    let amp = st.f.wave_amp.value;
    let phase = st.started.elapsed().as_secs_f32() * 5.5;
    let action = if s.can_seek && s.duration_ms > 0 {
        Action::Slider(SliderKind::Seek)
    } else {
        Action::None
    };
    cv.wave_slider(pr, st.media.progress(), amp, phase, action, "Seek");
    if times {
        let pos = (st.media.progress() * s.duration_ms as f32) as i64 / 1000;
        cv.text(
            &fmt_mmss(pos),
            &text::CAPTION_TNUM.size(10.5),
            Rect::new(pr.x, pr.bottom(), 60.0, 13.0),
            cv.theme.mist,
            Align::Left,
        );
        cv.text(
            &format!("-{}", fmt_mmss((s.duration_ms / 1000 - pos).max(0))),
            &text::CAPTION_TNUM.size(10.5),
            Rect::new(pr.right() - 60.0, pr.bottom(), 60.0, 13.0),
            cv.theme.mist,
            Align::Right,
        );
    }
}

fn transport(st: &State, cv: &mut Canvas, cx: f32, cy: f32, big: f32, gap: f32, extras: bool) {
    let s = &st.media.snap;
    let ink = cv.theme.ink;
    let dim = cv.theme.mist;
    if extras && s.can_shuffle {
        cv.button(
            Icon::Shuffle,
            cx - gap * 2.0,
            cy,
            30.0,
            Action::Media(MediaCmdKey::Shuffle),
            "Shuffle",
            if s.shuffle == Some(true) {
                cv.theme.accent[0]
            } else {
                dim
            },
        );
    }
    if s.can_prev {
        cv.button(
            Icon::Previous,
            cx - gap,
            cy,
            big * 0.8,
            Action::Media(MediaCmdKey::Prev),
            "Previous",
            ink,
        );
    }
    if s.can_play_pause {
        cv.circle(cx, cy, big / 2.0, WHITE.with_alpha(0.1));
        cv.button(
            if s.playing { Icon::Pause } else { Icon::Play },
            cx,
            cy,
            big,
            Action::Media(MediaCmdKey::PlayPause),
            if s.playing { "Pause" } else { "Play" },
            ink,
        );
    }
    if s.can_next {
        cv.button(
            Icon::Next,
            cx + gap,
            cy,
            big * 0.8,
            Action::Media(MediaCmdKey::Next),
            "Next",
            ink,
        );
    }
    if extras && s.can_repeat {
        let ic = if s.repeat == Some(1) {
            Icon::RepeatOne
        } else {
            Icon::Repeat
        };
        cv.button(
            ic,
            cx + gap * 2.0,
            cy,
            30.0,
            Action::Media(MediaCmdKey::Repeat),
            "Repeat",
            if s.repeat.unwrap_or(0) > 0 {
                cv.theme.accent[0]
            } else {
                dim
            },
        );
    }
}

/// Player without lyrics (the classic layout, plus the caption button).
fn full_player(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    let (title, artist) = display_names(st);
    let ar = Rect::new(area.x, area.y + 2.0, 60.0, 60.0);
    let skip = rv.skip_art;
    row(cv, rv, 0, ar, |cv| {
        art(st, cv, ar, skip);
        cv.zone_named(ar, Action::LyricsToggle, false, "Show lyrics", R_ART);
    });
    let tx = ar.right() + 14.0;
    let tw = area.right() - tx;
    row(cv, rv, 1, Rect::new(tx, area.y, tw, 40.0), |cv| {
        let chip_w = app_chip(st, cv, area.right(), area.y + 2.0);
        caption_button(st, cv, area.right() - chip_w - 22.0, area.y + 12.0);
        cv.text(
            &title,
            &text::TITLE,
            Rect::new(tx, area.y + 2.0, tw - chip_w - 46.0, 20.0),
            cv.theme.ink,
            Align::Left,
        );
        cv.text(
            &artist,
            &text::BODY,
            Rect::new(tx, area.y + 22.0, tw - 8.0, 18.0),
            cv.theme.mist,
            Align::Left,
        );
        super::compact::eq(st, cv, area.right() - 14.0, area.y + 34.0, 12.0);
    });
    let pr = Rect::new(tx, area.y + 48.0, tw, 14.0);
    row(cv, rv, 2, pr, |cv| seek_bar(st, cv, pr, true));
    let ty = area.bottom() - 22.0;
    row(
        cv,
        rv,
        3,
        Rect::new(area.x, ty - 18.0, area.w, 36.0),
        |cv| {
            transport(st, cv, area.cx() + 20.0, ty, 42.0, 54.0, true);
            if st.settings.music.app_volume {
                if let Some(v) = st.media.app_volume {
                    let vr = Rect::new(area.x, ty - 7.0, 70.0, 14.0);
                    cv.icon(
                        crate::render::icons::volume_icon(v, false),
                        vr.x + 7.0,
                        ty,
                        14.0,
                        cv.theme.mist,
                    );
                    cv.slider(
                        Rect::new(vr.x + 20.0, vr.y, 50.0, vr.h),
                        v,
                        Action::Slider(SliderKind::AppVolume),
                        "App volume",
                    );
                }
            }
        },
    );
    visualiser_ring(st, cv, ar);
}

/// Player on top of the lyrics stage: art, title, artist, transport and the seek bar.
fn compact_player(st: &mut State, cv: &mut Canvas, r: Rect, rv: &Reveal) {
    let s = st.media.snap.clone();
    let (title, artist) = display_names(st);
    let ar = Rect::new(r.x, r.y, 56.0, 56.0);
    let skip = rv.skip_art;
    row(cv, rv, 0, ar, |cv| art(st, cv, ar, skip));
    let tx = ar.right() + 12.0;
    let tr_w = 132.0;
    row(cv, rv, 1, Rect::new(tx, r.y, r.right() - tx, 56.0), |cv| {
        cv.text(
            &title,
            &text::TITLE.size(15.0),
            Rect::new(tx, r.y + 4.0, r.right() - tx - tr_w - 40.0, 20.0),
            cv.theme.ink,
            Align::Left,
        );
        cv.text(
            &artist,
            &text::CAPTION,
            Rect::new(tx, r.y + 24.0, r.right() - tx - tr_w - 40.0, 16.0),
            cv.theme.mist,
            Align::Left,
        );
        super::compact::eq(st, cv, tx + 6.0, r.y + 48.0, 9.0);
        cv.text(
            &s.app_name,
            &text::CAPTION.size(10.0),
            Rect::new(tx + 18.0, r.y + 41.0, 120.0, 14.0),
            FAINT,
            Align::Left,
        );
        transport(
            st,
            cv,
            r.right() - tr_w / 2.0 - 30.0,
            r.y + 22.0,
            34.0,
            40.0,
            false,
        );
        caption_button(st, cv, r.right() - 16.0, r.y + 22.0);
    });
    let pr = Rect::new(r.x, r.y + 62.0, r.w, 10.0);
    row(cv, rv, 2, pr, |cv| seek_bar(st, cv, pr, false));
}

fn visualiser_ring(st: &State, cv: &mut Canvas, ar: Rect) {
    if !st.settings.motion.visualiser || !st.settings.motion.visualiser_ring {
        return;
    }
    if let Some(b) = st.media.bands.clone() {
        for (i, v) in b.iter().enumerate() {
            let a = i as f32 / b.len() as f32 * std::f32::consts::TAU;
            let (r0, r1) = (33.0, 33.0 + 8.0 * v);
            let (c, sn) = (a.cos(), a.sin());
            cv.line(
                ar.cx() + r0 * sn,
                ar.cy() - r0 * c,
                ar.cx() + r1 * sn,
                ar.cy() - r1 * c,
                2.0,
                cv.theme.accent[i * 3 / b.len()],
            );
        }
    }
}

// ---------------------------------------------------------------- lyrics stage

fn ellipse(cv: &mut Canvas, x: f32, y: f32, rx: f32, ry: f32, c: Color) {
    if let Some(g) = cv.radial(x, y, rx, ry, c, c.with_alpha(0.0)) {
        unsafe {
            cv.ctx.FillEllipse(
                &windows::Win32::Graphics::Direct2D::D2D1_ELLIPSE {
                    point: crate::render::canvas::v2(x, y),
                    radiusX: rx,
                    radiusY: ry,
                },
                &g,
            )
        };
    }
}

/// The stage backdrop: the album's colours as a deep gradient with drifting aurora light.
fn backdrop(st: &State, cv: &mut Canvas, r: Rect, t: f32) -> Color {
    let pal = &st.media.palette;
    let c0 = pal.first().copied().unwrap_or(cv.theme.accent[0]);
    let c1 = pal.get(1).copied().unwrap_or(cv.theme.accent[1]);
    let c2 = pal.get(2).copied().unwrap_or(cv.theme.accent[2]);
    let base = c0.lerp(NOTCH, 0.82);
    if let Some(g) = cv.gradient(
        crate::render::canvas::v2(r.x, r.y),
        crate::render::canvas::v2(r.right(), r.bottom()),
        &[(0.0, c0.lerp(NOTCH, 0.72)), (1.0, c1.lerp(NOTCH, 0.86))],
        1.0,
    ) {
        use windows::core::Interface;
        cv.rrect_brush(r, 18.0, &g.cast().expect("brush"));
    } else {
        cv.sq(r, 18.0, base);
    }
    cv.push_clip(r.inset(1.0));
    let still = st.env.reduced;
    let k = if still { 0.0 } else { t };
    ellipse(
        cv,
        r.x + r.w * (0.25 + 0.12 * (k * 0.21).sin()),
        r.y + r.h * (0.3 + 0.2 * (k * 0.17).cos()),
        r.w * 0.45,
        r.h * 0.7,
        c1.with_alpha(0.20),
    );
    ellipse(
        cv,
        r.x + r.w * (0.75 + 0.1 * (k * 0.13).cos()),
        r.y + r.h * (0.65 + 0.2 * (k * 0.19).sin()),
        r.w * 0.4,
        r.h * 0.8,
        c2.with_alpha(0.16),
    );
    ellipse(
        cv,
        r.x + r.w * (0.5 + 0.2 * (k * 0.09).sin()),
        r.y + r.h * 0.1,
        r.w * 0.35,
        r.h * 0.4,
        c0.with_alpha(0.14),
    );
    cv.pop_clip();
    cv.stroke_rrect(r.inset(0.5), 17.5, WHITE.with_alpha(0.06), 1.0);
    base
}

fn lyrics_stage(st: &mut State, cv: &mut Canvas, r: Rect) {
    let t = st.started.elapsed().as_secs_f32();
    backdrop(st, cv, r, t);
    let hover = cv.hovered(r);
    let head = Rect::new(r.x + 12.0, r.y + 8.0, r.w - 24.0, 20.0);
    let body = Rect::new(
        r.x + 10.0,
        head.bottom() + 2.0,
        r.w - 20.0,
        r.bottom() - head.bottom() - 8.0,
    );
    let status = st.lyrics.status.clone();
    toolbar(st, cv, head, &status, hover);
    match status {
        _ if !st.settings.music.lyrics => message(
            cv,
            body,
            Icon::Text,
            "Lyrics are off",
            "Turn them on to see synced lyrics here",
            &[("Turn on", Action::LyricsToggle)],
        ),
        LyricsStatus::Idle | LyricsStatus::Loading => loading(cv, body, t),
        LyricsStatus::Ready(LyricsResult::Instrumental) => instrumental(cv, body, t),
        LyricsStatus::Ready(LyricsResult::NotFound) => {
            let q = st.lyrics.meta.query.clone();
            let sub = if q.is_empty() {
                crate::i18n::tr("Nothing matched this track")
            } else {
                format!("{} “{q}”", crate::i18n::tr("Searched for"))
            };
            message(
                cv,
                body,
                Icon::Search,
                "No lyrics found yet",
                &sub,
                &[
                    ("Try again", Action::LyricsRetry),
                    ("Search the web", Action::LyricsSearchWeb),
                ],
            )
        }
        LyricsStatus::Ready(LyricsResult::Error(e)) => message(
            cv,
            body,
            Icon::WifiOff,
            "Couldn't load lyrics",
            &e,
            &[("Try again", Action::LyricsRetry)],
        ),
        LyricsStatus::Ready(LyricsResult::Plain(lines)) => plain(st, cv, body, &lines),
        LyricsStatus::Ready(LyricsResult::Synced(lines)) => synced(st, cv, body, &lines, t),
    }
}

/// Source badge on the left; translate, timing and next-match tools on the right.
fn toolbar(st: &State, cv: &mut Canvas, head: Rect, status: &LyricsStatus, hover: bool) {
    let meta = &st.lyrics.meta;
    let kind = match status {
        LyricsStatus::Ready(LyricsResult::Synced(_)) => "Synced",
        LyricsStatus::Ready(LyricsResult::Plain(_)) => "Plain",
        LyricsStatus::Ready(LyricsResult::Instrumental) => "Instrumental",
        LyricsStatus::Loading => "Searching…",
        _ => "",
    };
    let badge = if meta.source.is_empty() {
        crate::i18n::tr(kind)
    } else {
        format!("{} · {}", meta.source, crate::i18n::tr(kind))
    };
    if !badge.trim().is_empty() {
        let w = cv.measure(&badge, &text::MICRO.weight(700)) + 22.0;
        let b = Rect::new(head.x, head.cy() - 9.0, w, 18.0);
        cv.rrect(b, 9.0, WHITE.with_alpha(0.1));
        let dot = if matches!(status, LyricsStatus::Ready(LyricsResult::Synced(_))) {
            GOOD
        } else {
            WHITE.with_alpha(0.5)
        };
        cv.circle(b.x + 8.0, b.cy(), 2.5, dot);
        cv.text(
            &badge,
            &text::MICRO.weight(700),
            Rect::new(b.x + 14.0, b.y, w - 16.0, b.h),
            WHITE.with_alpha(0.8),
            Align::Left,
        );
    }
    // tools: brighter while the pointer is over the stage
    let a = if hover { 0.95 } else { 0.55 };
    let ink = WHITE.with_alpha(a);
    let mut x = head.right() - 10.0;
    let synced = matches!(status, LyricsStatus::Ready(LyricsResult::Synced(_)));
    if synced {
        let tr_on = st.lyrics.show_translation;
        cv.circle(
            x,
            head.cy(),
            11.0,
            if tr_on {
                WHITE.with_alpha(0.22)
            } else {
                WHITE.with_alpha(0.0)
            },
        );
        cv.button(
            Icon::Translate,
            x,
            head.cy(),
            22.0,
            Action::LyricsTranslate,
            "Translate lyrics",
            ink,
        );
        x -= 26.0;
    }
    if meta.alts > 1 {
        cv.button(
            Icon::Skip,
            x,
            head.cy(),
            22.0,
            Action::LyricsNextMatch,
            "Try the next match",
            ink,
        );
        x -= 26.0;
    }
    if synced {
        // timing nudge: − value +
        let off = st.lyrics_offset();
        cv.button(
            Icon::Plus,
            x,
            head.cy(),
            20.0,
            Action::LyricsOffset(1),
            "Lyrics later",
            ink,
        );
        let label = if off == 0 {
            "0.0s".to_string()
        } else {
            format!("{:+.2}s", off as f32 / 1000.0)
        };
        let lw = cv.measure(&label, &text::MICRO.weight(700).tnum()) + 6.0;
        let lr = Rect::new(x - 12.0 - lw, head.y, lw, head.h);
        cv.text(
            &label,
            &text::MICRO.weight(700).tnum(),
            lr,
            if off == 0 {
                ink
            } else {
                Color::rgb(0xFFD60A).with_alpha(a)
            },
            Align::Center,
        );
        cv.zone_named(lr, Action::LyricsOffset(0), false, "Reset timing", 6.0);
        cv.button(
            Icon::Minus,
            lr.x - 10.0,
            head.cy(),
            20.0,
            Action::LyricsOffset(-1),
            "Lyrics earlier",
            ink,
        );
        // what auto-sync did (it lines the words up with when the music really starts)
        let auto = st.lyrics.auto_delay_ms;
        let listening = st.lyrics.probe.is_some();
        if auto > 0 || listening {
            let s = if listening {
                crate::i18n::tr("Syncing to the music…")
            } else {
                format!(
                    "{} +{:.1}s",
                    crate::i18n::tr("Auto-synced"),
                    auto as f32 / 1000.0
                )
            };
            let w = cv.measure(&s, &text::MICRO.weight(700)) + 14.0;
            let r = Rect::new(lr.x - 22.0 - w, head.cy() - 8.0, w, 16.0);
            cv.rrect(r, 8.0, GOOD.with_alpha(0.18));
            cv.text(
                &s,
                &text::MICRO.weight(700),
                r,
                GOOD.lerp(WHITE, 0.4),
                Align::Center,
            );
        }
    }
}

fn message(
    cv: &mut Canvas,
    body: Rect,
    icon: Icon,
    title: &str,
    sub: &str,
    buttons: &[(&str, Action)],
) {
    let cy = body.cy() - 22.0;
    cv.circle(body.cx(), cy - 6.0, 18.0, WHITE.with_alpha(0.1));
    cv.icon(icon, body.cx(), cy - 6.0, 18.0, WHITE.with_alpha(0.85));
    cv.label(
        title,
        &text::TITLE.size(15.0),
        Rect::new(body.x, cy + 18.0, body.w, 20.0),
        WHITE,
        Align::Center,
    );
    cv.text_top(
        sub,
        &text::CAPTION.lines(2),
        body.x + 16.0,
        cy + 39.0,
        body.w - 32.0,
        WHITE.with_alpha(0.6),
        Align::Center,
    );
    let widths: Vec<f32> = buttons
        .iter()
        .map(|(l, _)| cv.pill_width(l, 90.0))
        .collect();
    let total: f32 = widths.iter().sum::<f32>() + 8.0 * (buttons.len().max(1) - 1) as f32;
    let mut x = body.cx() - total / 2.0;
    for (i, (label, action)) in buttons.iter().enumerate() {
        cv.pill_button(
            label,
            Rect::new(x, cy + 70.0, widths[i], 26.0),
            action.clone(),
            i == 0,
        );
        x += widths[i] + 8.0;
    }
}

/// Skeleton lines with a shimmer sweeping across.
fn loading(cv: &mut Canvas, body: Rect, t: f32) {
    let widths = [0.62, 0.78, 0.5, 0.7];
    let sweep = (t * 0.8).fract();
    for (i, w) in widths.iter().enumerate() {
        let lw = body.w * w;
        let r = Rect::new(
            body.cx() - lw / 2.0,
            body.y + 26.0 + i as f32 * 30.0,
            lw,
            if i == 1 { 14.0 } else { 10.0 },
        );
        cv.rrect(
            r,
            r.h / 2.0,
            WHITE.with_alpha(if i == 1 { 0.16 } else { 0.09 }),
        );
        let sx = r.x + (sweep * 1.6 - 0.3) * r.w;
        cv.push_clip(r);
        ellipse(cv, sx, r.cy(), 40.0, 20.0, WHITE.with_alpha(0.18));
        cv.pop_clip();
    }
}

/// Notes drift up while an instrumental plays.
fn instrumental(cv: &mut Canvas, body: Rect, t: f32) {
    cv.label(
        "Instrumental",
        &text::TITLE.size(16.0),
        Rect::new(body.x, body.cy() - 10.0, body.w, 20.0),
        WHITE,
        Align::Center,
    );
    for i in 0..5 {
        let f = (t * 0.25 + i as f32 * 0.2).fract();
        let x = body.cx() + ((i as f32 * 1.7).sin() * 0.5) * body.w * 0.6;
        let y = body.bottom() - f * body.h;
        let a = (1.0 - (f - 0.5).abs() * 2.0).max(0.0) * 0.6;
        cv.icon(
            Icon::Music,
            x,
            y,
            12.0 + (i % 3) as f32 * 3.0,
            WHITE.with_alpha(a),
        );
    }
}

/// Plain lyrics scroll with the song's progress; the middle band is in focus.
fn plain(st: &State, cv: &mut Canvas, body: Rect, lines: &[String]) {
    let line_h = 24.0;
    let content = lines.len() as f32 * line_h;
    let progress = st.media.progress();
    let offset = (content - body.h * 0.5).max(0.0) * progress;
    let mid = body.cy();
    cv.push_clip(body);
    let mut y = body.y + body.h * 0.25 - offset;
    for l in lines {
        if y > body.y - line_h && y < body.bottom() + line_h {
            let d = ((y + line_h / 2.0 - mid) / (body.h * 0.5)).abs().min(1.0);
            let a = 1.0 - d * 0.75;
            let size = 15.0 - d * 2.0;
            cv.text(
                if l.is_empty() { "·" } else { l },
                &TextStyle::new(Family::Display, size, if d < 0.15 { 700 } else { 500 }),
                Rect::new(body.x, y, body.w, line_h),
                WHITE.with_alpha(a),
                Align::Center,
            );
        }
        y += line_h;
    }
    cv.pop_clip();
}

/// Synced lyrics: karaoke sweep on the current line, distance-faded neighbours, interlude
/// dots, click-to-seek.
fn synced(st: &State, cv: &mut Canvas, body: Rect, lines: &[(i64, String)], t: f32) {
    let pos = st.lyrics_pos();
    let cur = st.lyrics.current;
    let scroll = st.lyrics.scroll.value;
    let trans = if st.lyrics.show_translation {
        st.lyrics.translation.clone()
    } else {
        None
    };
    let line_h = if trans.is_some() { 46.0 } else { 34.0 };
    let center_y = body.y + body.h * 0.42;
    cv.push_clip(body);

    // interlude / intro: three dots that fill as the next line approaches
    let next_i = cur.map(|c| c + 1).unwrap_or(0);
    let gap_start = cur.map(|c| lines[c].0 + 3500).unwrap_or(0);
    let in_gap = match (cur, lines.get(next_i)) {
        (None, Some(n)) => n.0 - pos > 1500,
        (Some(c), Some(n)) => {
            lines[c].1.trim().is_empty()
                || (pos > gap_start && n.0 - pos > 1500 && n.0 - lines[c].0 > 7000)
        }
        _ => false,
    };

    let first = (scroll - 4.0).floor().max(0.0) as usize;
    for (i, (t0, text_line)) in lines.iter().enumerate().skip(first).take(11) {
        let d = i as f32 - scroll;
        let y = center_y + d * line_h;
        if y < body.y - line_h || y > body.bottom() + line_h {
            continue;
        }
        let dist = d.abs();
        let is_cur = Some(i) == cur;
        // size and alpha ease with distance from the focus line
        let near = (1.0 - dist).clamp(0.0, 1.0);
        let size = 14.5 + 6.0 * near;
        let weight = if near > 0.5 { 750 } else { 600 };
        let alpha = if is_cur {
            1.0
        } else {
            (0.5 - dist * 0.11).max(0.12)
        };
        let shown = if text_line.trim().is_empty() {
            "♪"
        } else {
            text_line.as_str()
        };
        // long lines shrink to fit
        let mut style = TextStyle::new(Family::Display, size, weight);
        let w = cv.measure(shown, &style);
        if w > body.w - 16.0 {
            style = TextStyle::new(
                Family::Display,
                (size * (body.w - 16.0) / w).max(11.0),
                weight,
            );
        }
        let lr = Rect::new(body.x, y - 13.0, body.w, 26.0);
        if is_cur && !(in_gap && text_line.trim().is_empty()) {
            let next = lines.get(i + 1).map(|l| l.0).unwrap_or(t0 + 4000);
            let span = (next - t0).clamp(800, 9000);
            let frac = ((pos + 60 - t0) as f32 / span as f32).clamp(0.0, 1.0);
            // soft glow behind the line
            ellipse(
                cv,
                lr.cx(),
                lr.cy(),
                body.w * 0.42,
                22.0,
                WHITE.with_alpha(0.06),
            );
            cv.text(shown, &style, lr, WHITE.with_alpha(0.38), Align::Center);
            let lw = cv.measure(shown, &style).min(lr.w);
            let x0 = lr.cx() - lw / 2.0;
            // the sweep: a crisp lit part plus a feathered edge
            cv.push_clip(Rect::new(x0, lr.y - 4.0, lw * frac, lr.h + 8.0));
            cv.text(shown, &style, lr, WHITE, Align::Center);
            cv.pop_clip();
            let edge = Rect::new(x0 + lw * frac, lr.y - 4.0, 10.0, lr.h + 8.0);
            cv.push_clip(edge);
            cv.text(shown, &style, lr, WHITE.with_alpha(0.65), Align::Center);
            cv.pop_clip();
        } else {
            cv.text(shown, &style, lr, WHITE.with_alpha(alpha), Align::Center);
        }
        if let Some(tr) = trans.as_ref().and_then(|t| t.get(i)) {
            cv.text(
                tr,
                &text::CAPTION.size(11.0),
                Rect::new(body.x, lr.bottom() - 2.0, body.w, 14.0),
                WHITE.with_alpha(if is_cur { 0.75 } else { alpha * 0.8 }),
                Align::Center,
            );
        }
        cv.zone_named(lr, Action::LyricLine(i), false, "", 8.0);
    }
    if in_gap {
        if let Some(n) = lines.get(next_i) {
            let start = cur.map(|c| lines[c].0).unwrap_or(0);
            let total = (n.0 - start).max(1) as f32;
            let p = ((pos - start) as f32 / total).clamp(0.0, 1.0);
            let y = center_y + (next_i as f32 - 0.5 - scroll) * line_h;
            let y = y.clamp(body.y + 14.0, body.bottom() - 14.0);
            let breathe = 1.0 + 0.12 * (t * 3.0).sin();
            for k in 0..3 {
                let x = body.cx() + (k as f32 - 1.0) * 16.0;
                let lit = ((p * 3.0) - k as f32).clamp(0.0, 1.0);
                cv.circle(x, y, 4.5 * breathe, WHITE.with_alpha(0.25 + 0.75 * lit));
            }
        }
    }
    cv.pop_clip();
}
