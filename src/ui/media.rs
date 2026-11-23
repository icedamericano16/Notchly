//! Music and synced lyrics pages.

use super::{row, Reveal};
use crate::app::state::{LyricsStatus, State};
use crate::app::{Action, MediaCmdKey, SliderKind};
use crate::render::icons::Icon;
use crate::render::text::{self, Align};
use crate::render::{Canvas, Rect};
use crate::services::lyrics::LyricsResult;
use crate::theme::tokens::*;
use crate::util::fmt_mmss;

pub fn music(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    if !st.media.has_session() {
        row(cv, rv, 0, area, |cv| {
            cv.icon(
                Icon::Music,
                area.cx(),
                area.cy() - 14.0,
                26.0,
                cv.theme.mist,
            );
            cv.label(
                "Nothing playing",
                &text::TITLE,
                Rect::new(area.x, area.cy() + 4.0, area.w, 20.0),
                cv.theme.ink,
                Align::Center,
            );
            cv.label(
                "Play something in Spotify, a browser or any media app",
                &text::CAPTION,
                Rect::new(area.x, area.cy() + 24.0, area.w, 16.0),
                cv.theme.mist,
                Align::Center,
            );
        });
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
    let art = Rect::new(area.x, area.y + 2.0, 60.0, 60.0);
    let skip_art = rv.skip_art;
    row(cv, rv, 0, art, |cv| {
        if !skip_art {
            match st.media.art.as_mut() {
                Some(img) => cv.image(img, art, R_ART),
                None => {
                    cv.sq(art, R_ART, SURFACE_RAISED);
                    cv.icon(Icon::Music, art.cx(), art.cy(), 22.0, cv.theme.mist);
                }
            }
        }
        cv.zone(art, Action::Open(crate::app::PageId::Lyrics));
    });
    let tx = art.right() + 14.0;
    let tw = area.right() - tx;
    row(cv, rv, 1, Rect::new(tx, area.y, tw, 40.0), |cv| {
        // app chip (session picker)
        let chip_w = if s.sessions.len() > 1 {
            cv.measure(&s.app_name, &text::CAPTION) + 34.0
        } else {
            cv.measure(&s.app_name, &text::CAPTION) + 18.0
        };
        let chip = Rect::new(area.right() - chip_w, area.y + 2.0, chip_w, 20.0);
        cv.rrect(chip, 10.0, LINE);
        cv.text(
            &s.app_name,
            &text::CAPTION,
            Rect::new(chip.x + 9.0, chip.y, chip.w - 18.0, chip.h),
            cv.theme.mist,
            Align::Left,
        );
        if s.sessions.len() > 1 {
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
        cv.text(
            &s.title,
            &text::TITLE,
            Rect::new(tx, area.y + 2.0, tw - chip_w - 8.0, 20.0),
            cv.theme.ink,
            Align::Left,
        );
        cv.text(
            &s.artist,
            &text::BODY,
            Rect::new(tx, area.y + 22.0, tw - 8.0, 18.0),
            cv.theme.mist,
            Align::Left,
        );
        // equaliser top-right under the chip
        super::compact::eq(st, cv, area.right() - 14.0, area.y + 34.0, 12.0);
    });
    // progress
    let pr = Rect::new(tx, area.y + 48.0, tw, 14.0);
    row(cv, rv, 2, pr, |cv| {
        // One UI-style wave: it travels while playing and eases flat on pause
        let amp = st.f.wave_amp.value;
        let phase = st.started.elapsed().as_secs_f32() * 5.5;
        if s.can_seek && s.duration_ms > 0 {
            cv.wave_slider(
                pr,
                st.media.progress(),
                amp,
                phase,
                Action::Slider(SliderKind::Seek),
                "Seek",
            );
        } else {
            cv.wave_slider(pr, st.media.progress(), amp, phase, Action::None, "");
        }
        let pos = (st.media.progress() * s.duration_ms as f32) as i64 / 1000;
        cv.text(
            &fmt_mmss(pos),
            &text::CAPTION_TNUM,
            Rect::new(pr.x, pr.bottom(), 60.0, 14.0),
            cv.theme.mist,
            Align::Left,
        );
        cv.text(
            &format!("-{}", fmt_mmss((s.duration_ms / 1000 - pos).max(0))),
            &text::CAPTION_TNUM,
            Rect::new(pr.right() - 60.0, pr.bottom(), 60.0, 14.0),
            cv.theme.mist,
            Align::Right,
        );
    });
    // transport row
    let ty = area.bottom() - 22.0;
    let cx = area.cx() + 20.0;
    row(
        cv,
        rv,
        3,
        Rect::new(area.x, ty - 18.0, area.w, 36.0),
        |cv| {
            let ink = cv.theme.ink;
            let dim = cv.theme.mist;
            if s.can_shuffle {
                cv.button(
                    Icon::Shuffle,
                    cx - 108.0,
                    ty,
                    32.0,
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
                    cx - 54.0,
                    ty,
                    34.0,
                    Action::Media(MediaCmdKey::Prev),
                    "Previous",
                    ink,
                );
            }
            if s.can_play_pause {
                cv.button(
                    if s.playing { Icon::Pause } else { Icon::Play },
                    cx,
                    ty,
                    42.0,
                    Action::Media(MediaCmdKey::PlayPause),
                    if s.playing { "Pause" } else { "Play" },
                    ink,
                );
            }
            if s.can_next {
                cv.button(
                    Icon::Next,
                    cx + 54.0,
                    ty,
                    34.0,
                    Action::Media(MediaCmdKey::Next),
                    "Next",
                    ink,
                );
            }
            if s.can_repeat {
                let ic = if s.repeat == Some(1) {
                    Icon::RepeatOne
                } else {
                    Icon::Repeat
                };
                cv.button(
                    ic,
                    cx + 108.0,
                    ty,
                    32.0,
                    Action::Media(MediaCmdKey::Repeat),
                    "Repeat",
                    if s.repeat.unwrap_or(0) > 0 {
                        cv.theme.accent[0]
                    } else {
                        dim
                    },
                );
            }
            // per-app volume
            if st.settings.music.app_volume {
                if let Some(v) = st.media.app_volume {
                    let vr = Rect::new(area.x, ty - 7.0, 70.0, 14.0);
                    cv.icon(
                        crate::render::icons::volume_icon(v, false),
                        vr.x + 7.0,
                        ty,
                        14.0,
                        dim,
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
    // visualiser ring/bars (while visible)
    if st.settings.motion.visualiser {
        if let Some(b) = st.media.bands.clone() {
            if st.settings.motion.visualiser_ring {
                for (i, v) in b.iter().enumerate() {
                    let a = i as f32 / b.len() as f32 * std::f32::consts::TAU;
                    let (r0, r1) = (33.0, 33.0 + 8.0 * v);
                    let (c, sn) = (a.cos(), a.sin());
                    cv.line(
                        art.cx() + r0 * sn,
                        art.cy() - r0 * c,
                        art.cx() + r1 * sn,
                        art.cy() - r1 * c,
                        2.0,
                        cv.theme.accent[i * 3 / b.len()],
                    );
                }
            }
        }
    }
}

pub fn lyrics(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    let s = st.media.snap.clone();
    let head = Rect::new(area.x, area.y, area.w, 24.0);
    row(cv, rv, 0, head, |cv| {
        let art = Rect::new(head.x, head.y, 22.0, 22.0);
        if let Some(img) = st.media.art.as_mut() {
            cv.image(img, art, 6.0);
        }
        cv.text(
            &format!("{} · {}", s.title, s.artist),
            &text::CAPTION.weight(600),
            Rect::new(art.right() + 8.0, head.y, head.w - 120.0, 22.0),
            cv.theme.ink,
            Align::Left,
        );
        let tl = Rect::new(head.right() - 92.0, head.y + 1.0, 92.0, 20.0);
        cv.rrect(
            tl,
            10.0,
            if st.lyrics.show_translation {
                cv.theme.accent[1].with_alpha(0.35)
            } else {
                LINE
            },
        );
        cv.icon(Icon::Translate, tl.x + 13.0, tl.cy(), 12.0, cv.theme.mist);
        cv.label(
            "Translate",
            &text::CAPTION,
            Rect::new(tl.x + 22.0, tl.y, tl.w - 26.0, tl.h),
            cv.theme.mist,
            Align::Left,
        );
        cv.zone_named(tl, Action::LyricsTranslate, true, "Translate lyrics", 10.0);
    });
    let body = Rect::new(area.x, area.y + 28.0, area.w, area.h - 28.0);
    let status = st.lyrics.status.clone();
    match status {
        LyricsStatus::Idle | LyricsStatus::Loading => {
            let msg = if !st.media.has_session() {
                "Nothing playing"
            } else if !st.settings.music.lyrics {
                "Lyrics are off"
            } else {
                "Finding lyrics…"
            };
            row(cv, rv, 1, body, |cv| {
                cv.text(
                    &crate::i18n::tr(msg),
                    &text::BODY,
                    body,
                    cv.theme.mist,
                    Align::Center,
                );
            });
        }
        LyricsStatus::Ready(LyricsResult::Instrumental) => {
            row(cv, rv, 1, body, |cv| {
                cv.label(
                    "Instrumental",
                    &text::TITLE,
                    Rect::new(body.x, body.cy() - 20.0, body.w, 20.0),
                    cv.theme.ink,
                    Align::Center,
                );
                super::compact::eq(st, cv, body.cx(), body.cy() + 16.0, 20.0);
            });
        }
        LyricsStatus::Ready(LyricsResult::NotFound)
        | LyricsStatus::Ready(LyricsResult::Error(_)) => {
            row(cv, rv, 1, body, |cv| {
                cv.label(
                    "No lyrics found for this song",
                    &text::BODY,
                    body,
                    cv.theme.mist,
                    Align::Center,
                );
            });
        }
        LyricsStatus::Ready(LyricsResult::Plain(lines)) => {
            let scroll = st.input.scroll_value(crate::app::PageId::Lyrics);
            cv.push_clip(body);
            let mut y = body.y - scroll;
            cv.label(
                "Not synced",
                &text::MICRO,
                Rect::new(body.x, y, body.w, 14.0),
                FAINT,
                Align::Center,
            );
            y += 18.0;
            for l in &lines {
                if y > body.y - 20.0 && y < body.bottom() {
                    cv.text(
                        l,
                        &text::BODY,
                        Rect::new(body.x, y, body.w, 18.0),
                        cv.theme.ink,
                        Align::Center,
                    );
                }
                y += 19.0;
            }
            cv.pop_clip();
        }
        LyricsStatus::Ready(LyricsResult::Synced(lines)) => {
            let pos = st.media.position_ms();
            let cur = st.lyrics.current;
            let scroll = st.lyrics.scroll.value;
            let trans = if st.lyrics.show_translation {
                st.lyrics.translation.clone()
            } else {
                None
            };
            let line_h = if trans.is_some() { 34.0 } else { 26.0 };
            let center_y = body.cy();
            cv.push_clip(body);
            let first = (scroll - 4.0).floor().max(0.0) as usize;
            for (i, (t0, text_line)) in lines.iter().enumerate().skip(first).take(10) {
                let d = i as f32 - scroll;
                let y = center_y + d * line_h;
                let dist = d.abs();
                if y < body.y - line_h || y > body.bottom() + line_h {
                    continue;
                }
                let is_cur = Some(i) == cur;
                let size = if is_cur {
                    LYRIC_SIZE
                } else {
                    (LYRIC_DIM_SIZE + 0.5 * (1.0 - dist.min(1.0))).max(LYRIC_DIM_SIZE)
                };
                let style = if is_cur {
                    text::LYRIC
                } else {
                    text::TextStyle::new(text::Family::Display, size, 500)
                };
                let alpha = if is_cur {
                    1.0
                } else {
                    (0.55 - dist * 0.1).max(0.15)
                };
                let lr = Rect::new(
                    body.x,
                    y - line_h / 2.0,
                    body.w,
                    if trans.is_some() { 24.0 } else { line_h },
                );
                let shown = if text_line.is_empty() {
                    "♪"
                } else {
                    text_line.as_str()
                };
                if is_cur {
                    // karaoke sweep across the line's estimated duration
                    let next = lines.get(i + 1).map(|l| l.0).unwrap_or(t0 + 4000);
                    let frac =
                        ((pos + 250 - t0) as f32 / (next - t0).max(1) as f32).clamp(0.0, 1.0);
                    cv.text(
                        shown,
                        &style,
                        lr,
                        cv.theme.ink.with_alpha(0.45),
                        Align::Center,
                    );
                    let lw = cv.measure(shown, &style).min(lr.w);
                    let x0 = lr.cx() - lw / 2.0;
                    cv.push_clip(Rect::new(x0, lr.y, lw * frac, lr.h));
                    if let Some(b) = cv.accent_brush(x0, x0 + lw, 1.0) {
                        use windows::core::Interface;
                        cv.text_brush(shown, &style, lr, &b.cast().expect("brush"), Align::Center);
                    }
                    cv.pop_clip();
                } else {
                    cv.text(
                        shown,
                        &style,
                        lr,
                        cv.theme.ink.with_alpha(alpha),
                        Align::Center,
                    );
                }
                if let Some(tr) = trans.as_ref().and_then(|t| t.get(i)) {
                    cv.text(
                        tr,
                        &text::CAPTION,
                        Rect::new(body.x, lr.bottom() - 4.0, body.w, 14.0),
                        cv.theme.mist.with_alpha(if is_cur { 1.0 } else { 0.6 }),
                        Align::Center,
                    );
                }
                cv.zone(lr, Action::LyricLine(i));
            }
            cv.pop_clip();
        }
    }
}
