//! Collapsed views: idle (mascot), live activities, pop-ups and the bubble.

use std::time::Instant;

use super::{row, Reveal};
use crate::app::activity::ActKind;
use crate::app::state::State;
use crate::app::{Action, PageId, SliderKind};
use crate::render::icons::Icon;
use crate::render::text::{self, Align};
use crate::render::{Canvas, Rect};
use crate::theme::tokens::*;
use crate::util::{fmt_mmss, unix_secs};

/// Equaliser bars.
pub fn eq(st: &State, cv: &mut Canvas, cx: f32, cy: f32, h: f32) {
    let w = 3.0;
    let gap = 2.5;
    let total = 4.0 * w + 3.0 * gap;
    let brush = cv.accent_brush(cx - total / 2.0, cx + total / 2.0, 1.0);
    for (i, s) in st.media.eq.iter().enumerate() {
        let bh = (h * s.value).max(3.0);
        let r = Rect::new(
            cx - total / 2.0 + i as f32 * (w + gap),
            cy - bh / 2.0,
            w,
            bh,
        );
        match &brush {
            Some(b) => {
                use windows::core::Interface;
                cv.rrect_brush(r, w / 2.0, &b.cast().expect("brush"));
            }
            None => cv.rrect(r, w / 2.0, cv.theme.ink),
        }
    }
}

/// A live activity's compact view.
pub fn live(st: &mut State, cv: &mut Canvas, area: Rect, key: &str, rv: &Reveal) {
    let Some(kind) = st.act.get(key).map(|a| a.kind) else {
        return;
    };
    let l = Rect::new(area.x + 4.0, area.y, 22.0, area.h);
    let r = Rect::new(area.right() - 26.0, area.y, 22.0, area.h);
    let mid = Rect::new(
        l.right() + 6.0,
        area.y,
        (r.x - l.right() - 12.0).max(0.0),
        area.h,
    );
    let cy = area.cy();
    let ink = cv.theme.ink;
    let mist = cv.theme.mist;
    row(cv, rv, 0, area, |cv| match kind {
        ActKind::Music => {
            let art = Rect::centered(l.cx(), cy, 20.0, 20.0);
            if !rv.skip_art {
                match st.media.art.as_mut() {
                    Some(img) => cv.image(img, art, R_ART_LIVE),
                    None => {
                        cv.rrect(art, R_ART_LIVE, SURFACE_RAISED);
                        cv.icon(Icon::Music, art.cx(), art.cy(), 12.0, mist);
                    }
                }
            }
            if st.settings.music.lyrics_live {
                if let Some(line) = st.lyrics_line_now() {
                    let t = st.lyrics.line_change.elapsed().as_secs_f32();
                    cv.push_alpha((t / 0.25).min(1.0));
                    cv.text(
                        &line,
                        &text::BODY_MED,
                        Rect::new(mid.x, mid.y, mid.w + 26.0, mid.h),
                        ink,
                        Align::Center,
                    );
                    cv.pop_alpha();
                    return;
                }
            }
            eq(st, cv, r.cx(), cy, 16.0);
        }
        ActKind::Focus | ActKind::TimerEnding => {
            let t = &st.f.state.timer;
            let left = t.remaining_ms();
            let ending = kind == ActKind::TimerEnding;
            if ending {
                let s = format!("{}", (left + 999) / 1000);
                cv.text(
                    &s,
                    &text::DISPLAY.size(22.0),
                    Rect::new(area.x, area.y, area.w, area.h),
                    CRITICAL,
                    Align::Center,
                );
                cv.icon(Icon::Timer, l.cx(), cy, 16.0, CRITICAL);
            } else {
                cv.ring(l.cx(), cy, 8.0, t.progress(), 2.5, None);
                let label = if t.running() {
                    fmt_mmss((left + 999) / 1000)
                } else {
                    format!("{} ❚❚", fmt_mmss(left / 1000))
                };
                cv.text(
                    &label,
                    &text::CAPTION_TNUM.size(13.0).weight(600),
                    Rect::new(mid.x, mid.y, mid.w + 26.0, mid.h),
                    ink,
                    Align::Right,
                );
            }
        }
        ActKind::Call => {
            cv.circle(l.cx(), cy, 10.0, GOOD);
            cv.icon(Icon::Phone, l.cx(), cy, 12.0, NOTCH);
            let (name, secs) =
                st.f.meeting
                    .as_ref()
                    .map(|m| (m.app.clone(), m.started.elapsed().as_secs() as i64))
                    .unwrap_or(("Call".into(), 0));
            cv.text(
                &name,
                &text::BODY_MED,
                Rect::new(mid.x, mid.y, mid.w - 40.0, mid.h),
                ink,
                Align::Left,
            );
            cv.text(
                &fmt_mmss(secs),
                &text::CAPTION_TNUM,
                Rect::new(mid.right() - 40.0, mid.y, 66.0, mid.h),
                GOOD,
                Align::Right,
            );
        }
        ActKind::Recording => {
            cv.circle(l.cx(), cy, 4.5, CRITICAL);
            let label = if st.f.ocr.busy {
                "Reading text…"
            } else {
                "REC"
            };
            cv.text(label, &text::MICRO, mid, CRITICAL, Align::Left);
        }
        ActKind::LiveApi => {
            let id = key.trim_start_matches("live:");
            let Some(it) = st.f.live.items.iter_mut().find(|i| i.id == id) else {
                return;
            };
            let color = it.color.unwrap_or(cv.theme.accent[0]);
            match (&mut it.image, it.icon) {
                (Some(img), _) => cv.image(img, Rect::centered(l.cx(), cy, 18.0, 18.0), 5.0),
                (None, Some(ic)) => cv.icon(ic, l.cx(), cy, 16.0, color),
                (None, None) => cv.circle(l.cx(), cy, 4.0, color),
            }
            let label = it.final_text.clone().unwrap_or_else(|| {
                if it.subtitle.is_empty() {
                    it.title.clone()
                } else {
                    format!("{} · {}", it.title, it.subtitle)
                }
            });
            cv.text(&label, &text::BODY_MED, mid, ink, Align::Left);
            if let Some(p) = it.progress {
                cv.ring(r.cx(), cy, 8.0, p, 2.5, Some(color));
            }
        }
        ActKind::Transfer => {
            let (name, prog) = if key == "phonedrop" {
                ("Phone Drop".to_string(), st.f.phone.progress)
            } else {
                let id: u64 = key.trim_start_matches("dl:").parse().unwrap_or(0);
                st.f.downloads
                    .items
                    .iter()
                    .find(|d| d.id == id)
                    .map(|d| {
                        (
                            d.name.clone(),
                            d.total.map(|t| d.bytes as f32 / t.max(1) as f32),
                        )
                    })
                    .unwrap_or(("Download".into(), None))
            };
            cv.icon(Icon::Download, l.cx(), cy, 16.0, cv.theme.accent[0]);
            cv.text(&name, &text::BODY_MED, mid, ink, Align::Left);
            match prog {
                Some(p) => {
                    cv.ring(r.cx(), cy, 8.0, p, 2.5, None);
                }
                None => {
                    // indeterminate shimmer
                    let t = st.started.elapsed().as_secs_f32();
                    cv.ring(r.cx(), cy, 8.0, 0.0, 2.5, None);
                    let a = (t * 1.6).fract();
                    cv.circle(
                        r.cx() + 8.0 * (a * std::f32::consts::TAU).sin(),
                        cy - 8.0 * (a * std::f32::consts::TAU).cos(),
                        2.0,
                        cv.theme.accent[0],
                    );
                }
            }
        }
        ActKind::Charging => {
            let b = &st.sys.battery;
            // the bolt pops in on the bouncy spring
            let pop = st
                .isl
                .sweep
                .map(|at| {
                    0.6 + 0.4
                        * crate::motion::step_response(
                            at.elapsed().as_secs_f32(),
                            crate::motion::BOUNCY,
                            &st.env,
                        )
                })
                .unwrap_or(1.0);
            cv.icon(Icon::Bolt, l.cx(), cy, 16.0 * pop, GOOD);
            cv.text(
                &format!("{}%", b.percent),
                &text::BODY_MED,
                Rect::new(mid.x, mid.y, mid.w + 26.0, mid.h),
                ink,
                Align::Right,
            );
            if let Some(at) = st.isl.sweep {
                let t = at.elapsed().as_secs_f32() / 0.7;
                let x = l.x - 10.0 + (area.w + 20.0) * t;
                if let Some(g) = cv.radial(
                    x,
                    cy,
                    30.0,
                    area.h,
                    GOOD.with_alpha(0.35),
                    GOOD.with_alpha(0.0),
                ) {
                    use windows::Win32::Graphics::Direct2D::D2D1_ELLIPSE;
                    unsafe {
                        cv.ctx.FillEllipse(
                            &D2D1_ELLIPSE {
                                point: crate::render::canvas::v2(x, cy),
                                radiusX: 30.0,
                                radiusY: area.h,
                            },
                            &g,
                        )
                    };
                }
            }
        }
        ActKind::Event => {
            let now = unix_secs();
            if let Some(e) =
                st.f.calendar
                    .events
                    .iter()
                    .filter(|e| e.start > now)
                    .min_by_key(|e| e.start)
            {
                cv.circle(l.cx(), cy, 4.0, Color::rgb(e.color));
                cv.text(
                    &e.title,
                    &text::BODY_MED,
                    Rect::new(mid.x, mid.y, mid.w - 50.0, mid.h),
                    ink,
                    Align::Left,
                );
                cv.text(
                    &format!("in {} min", (e.start - now + 59) / 60),
                    &text::CAPTION,
                    Rect::new(mid.right() - 50.0, mid.y, 76.0, mid.h),
                    mist,
                    Align::Right,
                );
            }
        }
        ActKind::Prayer => {
            let settings = st.settings.clone();
            if let Some(t) = st.f.prayer_times(&settings) {
                let now = crate::util::local_now().secs_of_day() / 60.0;
                if let Some((i, mins)) = crate::features::prayer::next_prayer(&t, now) {
                    cv.icon(Icon::MoonStar, l.cx(), cy, 16.0, cv.theme.accent[0]);
                    cv.text(
                        t.prayers()[i].0,
                        &text::BODY_MED,
                        Rect::new(mid.x, mid.y, mid.w - 50.0, mid.h),
                        ink,
                        Align::Left,
                    );
                    cv.text(
                        &fmt_mmss((mins * 60.0) as i64),
                        &text::CAPTION_TNUM,
                        Rect::new(mid.right() - 50.0, mid.y, 76.0, mid.h),
                        mist,
                        Align::Right,
                    );
                }
            }
        }
        ActKind::Note => {
            let line =
                st.f.notes
                    .notes
                    .iter()
                    .find(|n| n.pinned)
                    .map(crate::features::store::Notes::first_line)
                    .unwrap_or_default();
            cv.icon(Icon::Note, l.cx(), cy, 16.0, cv.theme.accent[0]);
            cv.text(
                &line,
                &text::BODY_MED,
                Rect::new(mid.x, mid.y, mid.w + 26.0, mid.h),
                ink,
                Align::Left,
            );
        }
        ActKind::SysAlert => {
            cv.icon(Icon::Cpu, l.cx(), cy, 16.0, WARN);
            let cpu = st.f.sysmon.last.map(|s| s.cpu).unwrap_or(0.0);
            cv.text(
                &format!("High CPU · {cpu:.0}%"),
                &text::BODY_MED,
                Rect::new(mid.x, mid.y, mid.w + 26.0, mid.h),
                ink,
                Align::Left,
            );
        }
    });
}

/// The secondary bubble: just an icon or progress ring.
pub fn bubble(st: &mut State, cv: &mut Canvas, b: Rect) {
    let Some(a) = st.act.secondary() else { return };
    let (cx, cy) = (b.cx(), b.y + b.h / 2.0);
    let acc = cv.theme.accent[0];
    match a.kind {
        ActKind::Music => {
            if let Some(img) = st.media.art.as_mut() {
                cv.image(img, Rect::centered(cx, cy, 18.0, 18.0), 9.0);
            } else {
                cv.icon(Icon::Music, cx, cy, 14.0, acc);
            }
        }
        ActKind::Focus | ActKind::TimerEnding => {
            cv.ring(cx, cy, 8.0, st.f.state.timer.progress(), 2.5, None)
        }
        ActKind::Call => cv.icon(Icon::Phone, cx, cy, 14.0, GOOD),
        ActKind::Transfer => cv.icon(Icon::Download, cx, cy, 14.0, acc),
        ActKind::LiveApi => {
            let id = a.key.trim_start_matches("live:").to_string();
            match st.f.live.get(&id).and_then(|i| i.progress) {
                Some(p) => cv.ring(cx, cy, 8.0, p, 2.5, None),
                None => cv.icon(
                    st.f.live
                        .get(&id)
                        .and_then(|i| i.icon)
                        .unwrap_or(Icon::Bell),
                    cx,
                    cy,
                    14.0,
                    acc,
                ),
            }
        }
        ActKind::Charging => cv.icon(Icon::Bolt, cx, cy, 14.0, GOOD),
        ActKind::Event => cv.icon(Icon::Calendar, cx, cy, 14.0, acc),
        ActKind::Prayer => cv.icon(Icon::MoonStar, cx, cy, 14.0, acc),
        ActKind::Recording => cv.circle(cx, cy, 4.0, CRITICAL),
        ActKind::Note => cv.icon(Icon::Note, cx, cy, 14.0, acc),
        ActKind::SysAlert => cv.icon(Icon::Cpu, cx, cy, 14.0, WARN),
    }
    cv.zone_named(b, Action::SwapBubble, false, "Swap activity", b.w / 2.0);
}

/// Pop-up (Peek) content: icon or badge, title/label, slider or subtitle, actions.
pub fn peek(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    let Some(p) = st.pop.current.clone() else {
        return;
    };
    let icon_scale = st.isl.peek_icon.value;
    let label_t = st.isl.peek_label.value.min(1.0);
    let ic = Rect::new(
        area.x,
        area.y + 2.0,
        34.0,
        if p.actions.is_empty() {
            area.h - 4.0
        } else {
            40.0
        },
    );
    let tint = p.tint.unwrap_or(cv.theme.ink);
    let id = p.id;
    let waves = if st.env.reduced {
        1.0
    } else {
        st.f.intro_progress(&format!("vol:{}:{:?}", p.id, p.icon))
    };
    let bell = super::bell_angle(st);
    let two_line = !p.actions.is_empty() || p.kind == crate::app::activity::PopKind::Notification;
    row(cv, rv, 0, area, |cv| {
        cv.push_tf(0.0, 0.0, icon_scale, ic.cx(), ic.cy());
        if let Some(b) = &p.badge {
            cv.rrect(
                Rect::centered(ic.cx(), ic.cy(), 30.0, 22.0),
                7.0,
                SURFACE_RAISED,
            );
            cv.text(
                b,
                &text::MICRO.size(11.0),
                Rect::centered(ic.cx(), ic.cy(), 30.0, 22.0),
                cv.theme.ink,
                Align::Center,
            );
        } else if p.kind == crate::app::activity::PopKind::Volume && p.icon != Icon::Volume0 {
            let w = waves;
            cv.icon(Icon::Volume0, ic.cx(), ic.cy(), 22.0, tint);
            cv.push_clip(Rect::new(
                ic.cx() - 11.0,
                ic.y,
                22.0 * (0.47 + 0.53 * w),
                ic.h,
            ));
            cv.icon(p.icon, ic.cx(), ic.cy(), 22.0, tint);
            cv.pop_clip();
        } else if p.icon == Icon::Bell {
            cv.icon_rot(p.icon, ic.cx(), ic.cy(), 22.0, tint, bell);
        } else {
            cv.icon(p.icon, ic.cx(), ic.cy(), 22.0, tint);
        }
        cv.pop_tf();
        let tx = ic.right() + 10.0 - 8.0 * (1.0 - label_t);
        cv.push_alpha(label_t);
        let tw = area.right() - tx;
        if p.value.is_some() {
            let title_w = cv.measure(&p.title, &text::BODY_MED).min(tw * 0.45);
            cv.text(
                &p.title,
                &text::BODY_MED,
                Rect::new(tx, ic.y, title_w + 2.0, ic.h),
                cv.theme.ink,
                Align::Left,
            );
            let v = st.isl.peek_value.value;
            let sx = tx + title_w + 12.0;
            let sr = Rect::new(sx, ic.y, (area.right() - sx - 44.0).max(20.0), ic.h);
            let kind = match p.kind {
                crate::app::activity::PopKind::Volume => Some(SliderKind::Volume),
                crate::app::activity::PopKind::Brightness => Some(SliderKind::Brightness),
                _ => None,
            };
            match kind {
                Some(k) => cv.slider(sr, v, Action::Slider(k), &p.title),
                None => cv.progress(Rect::new(sr.x, sr.cy() - 3.0, sr.w, 6.0), v, false),
            }
            cv.text(
                &p.subtitle,
                &text::CAPTION_TNUM,
                Rect::new(area.right() - 40.0, ic.y, 40.0, ic.h),
                cv.theme.mist,
                Align::Right,
            );
        } else if two_line {
            // notification: title + 2 lines, typed on over 120 ms
            // text types on over 120 ms (a character reveal, not per-letter objects)
            let reveal = (st.isl.peek_shown.elapsed().as_secs_f32() / 0.12).min(1.0);
            let n = (p.subtitle.chars().count() as f32 * reveal) as usize;
            let sub: String = p.subtitle.chars().take(n).collect();
            cv.text(
                &p.title,
                &text::BODY_MED,
                Rect::new(tx, area.y + 2.0, tw, 18.0),
                cv.theme.ink,
                Align::Left,
            );
            cv.text_top(
                &sub,
                &text::CAPTION.lines(2),
                tx,
                area.y + 21.0,
                tw,
                cv.theme.mist,
                Align::Left,
            );
        } else {
            let has_sub = !p.subtitle.is_empty();
            cv.text(
                &p.title,
                &text::BODY_MED,
                Rect::new(tx, ic.y + if has_sub { -8.0 } else { 0.0 }, tw, ic.h),
                cv.theme.ink,
                Align::Left,
            );
            if has_sub {
                cv.text(
                    &p.subtitle,
                    &text::CAPTION,
                    Rect::new(tx, ic.y + 10.0, tw, ic.h),
                    cv.theme.mist,
                    Align::Left,
                );
            }
        }
        cv.pop_alpha();
        if !p.actions.is_empty() {
            let n = p.actions.len() as f32;
            let bw = ((area.w - 34.0 - 8.0 * (n - 1.0)) / n).min(110.0);
            let total = bw * n + 8.0 * (n - 1.0);
            let mut x = area.right() - total;
            let y = area.bottom() - 28.0;
            for (i, (label, _)) in p.actions.iter().enumerate() {
                cv.pill_button(
                    label,
                    Rect::new(x, y, bw, 24.0),
                    Action::PopupButton(id, i as u8),
                    i == 0,
                );
                x += bw + 8.0;
            }
        }
    });
    let open = match p.kind {
        crate::app::activity::PopKind::Volume | crate::app::activity::PopKind::Brightness => None,
        crate::app::activity::PopKind::Notification => Some(PageId::Notifications),
        _ => None,
    };
    if let Some(page) = open {
        cv.zone(
            Rect::new(area.x, area.y, area.w * 0.6, area.h * 0.5),
            Action::Open(page),
        );
    }
}

/// Idle: the time and status glyphs (headphones, Bluetooth, charging…), cross-fading
/// with the Blink mascot when it appears.
pub fn idle(st: &mut State, cv: &mut Canvas, area: Rect) {
    let a = st.mascot.alpha.value;
    if st.settings.appearance.idle_clock && a < 0.99 {
        cv.push_alpha(1.0 - a);
        ambient(st, cv, area);
        cv.pop_alpha();
    }
    if a < 0.01 {
        return;
    }
    let now = Instant::now();
    let m = &st.mascot;
    cv.push_alpha(a);
    let cx = area.cx() + m.look.value * 2.0;
    let cy = area.cy() + m.bounce.value.clamp(-4.0, 4.0);
    let blink = m.blinking.map(|b| {
        let t = (now - b).as_secs_f32() / 0.14;
        1.0 - 0.9 * (t * std::f32::consts::PI).sin().max(0.0)
    });
    let low = st.sys.battery.present
        && !st.sys.battery.ac
        && st.sys.battery.percent <= st.settings.popups.low_battery;
    let sleepy = crate::util::local_now().hour < 5;
    let mut sy = blink.unwrap_or(1.0);
    if low {
        sy *= 0.45;
    }
    if sleepy {
        sy *= 0.6;
    }
    let hearts = m.hearts_until.is_some_and(|h| now < h);
    let wink = m.wink_until.is_some_and(|w| now < w);
    for (i, dx) in [-9.0f32, 9.0].iter().enumerate() {
        let ex = cx + dx;
        if hearts {
            cv.icon(Icon::Heart, ex, cy, 11.0, TINT_MUSIC);
        } else {
            let h = if wink && i == 1 {
                1.5
            } else {
                (EYE_H * sy).max(1.5)
            };
            cv.rrect(
                Rect::centered(ex, cy, EYE_W, h),
                (EYE_W / 2.0).min(h / 2.0),
                cv.theme.ink,
            );
        }
    }
    cv.pop_alpha();
    cv.zone(Rect::centered(cx, cy, 36.0, 20.0), Action::Mascot);
}

/// A small battery glyph filled to `frac`.
fn battery_glyph(cv: &mut Canvas, cx: f32, cy: f32, frac: f32, fill: Color) {
    let body = Rect::centered(cx - 1.0, cy, 18.0, 9.0);
    cv.stroke_rrect(body, 2.5, cv.theme.mist.with_alpha(0.8), 1.2);
    let inner = body.inset(2.0);
    cv.rrect(
        Rect::new(
            inner.x,
            inner.y,
            (inner.w * frac.clamp(0.08, 1.0)).max(1.5),
            inner.h,
        ),
        1.2,
        fill,
    );
    cv.rrect(
        Rect::centered(body.right() + 1.6, cy, 1.6, 3.6),
        0.8,
        cv.theme.mist.with_alpha(0.8),
    );
}

fn ambient(st: &State, cv: &mut Canvas, area: Rect) {
    use crate::app::island::Glyph;
    let now = crate::util::local_now();
    let h24 = st.clock_24h();
    let short = |m: u32| {
        let t = crate::util::fmt_hm(m, h24);
        t.split(' ').next().unwrap_or(&t).to_string()
    };
    let mins = now.minutes_of_day();
    let cur = short(mins);
    // the minute's digits roll up as it turns over
    let into = now.secs_of_day().rem_euclid(60.0) as f32;
    let roll = if st.env.reduced || into >= IDLE_ROLL_SECS {
        1.0
    } else {
        let k = into / IDLE_ROLL_SECS;
        1.0 - (1.0 - k).powi(3)
    };
    let prev = short((mins + 1439) % 1440);
    clock(cv, area, &cur, &prev, roll);

    let layout = st.idle_layout();
    let cy = area.cy();
    let draw = |cv: &mut Canvas, g: Glyph, x: f32| {
        let since = st
            .isl
            .glyph_since
            .get(&g)
            .map(|t| t.elapsed().as_secs_f32())
            .unwrap_or(10.0);
        // glyphs pop in on the bouncy spring
        let pop = 0.4 + 0.6 * crate::motion::step_response(since, crate::motion::BOUNCY, &st.env);
        cv.push_alpha((since / 0.12).min(1.0));
        // `x` is the glyph's left edge
        match g {
            Glyph::Charging => {
                cv.push_tf(0.0, 0.0, pop, x + 7.0, cy);
                cv.icon(Icon::Bolt, x + 7.0, cy, 14.0, GOOD);
                cv.pop_tf();
                cv.text(
                    &format!("{}%", st.sys.battery.percent),
                    &text::CAPTION_TNUM.weight(600).size(12.0),
                    Rect::new(x + 14.0, area.y, IDLE_CHARGING_W - 14.0, area.h),
                    GOOD,
                    Align::Left,
                );
            }
            Glyph::BatteryLow => {
                cv.push_tf(0.0, 0.0, pop, x + 11.0, cy);
                battery_glyph(
                    cv,
                    x + 11.0,
                    cy,
                    st.sys.battery.percent as f32 / 100.0,
                    CRITICAL,
                );
                cv.pop_tf();
            }
            _ => {
                let (ic, col) = match g {
                    Glyph::Headphones => (Icon::Headphones, cv.theme.ink),
                    Glyph::Bluetooth => (Icon::Bluetooth, TINT_WEATHER),
                    _ => (Icon::WifiOff, WARN),
                };
                cv.push_tf(0.0, 0.0, pop, x + 11.0, cy);
                cv.icon(ic, x + 11.0, cy, 15.0, col);
                cv.pop_tf();
            }
        }
        cv.pop_alpha();
    };
    // left group from the left edge inward, right group from the right edge (past the
    // privacy dots) inward
    let mut x = area.x + IDLE_PAD;
    for g in &layout.left {
        draw(cv, *g, x);
        x += g.width();
    }
    let dots_w = crate::app::island::IdleLayout::dots_w(layout.dots);
    let mut x = area.right() - IDLE_PAD - (dots_w - 10.0).max(0.0);
    for g in layout.right.iter().rev() {
        x -= g.width();
        draw(cv, *g, x);
    }
}

/// The idle clock, centred; digits that changed roll up out of view as the new ones rise in.
fn clock(cv: &mut Canvas, area: Rect, cur: &str, prev: &str, roll: f32) {
    let style = text::TITLE.size(14.0).tnum();
    let w = cv.measure(cur, &style);
    let x0 = area.cx() - w / 2.0;
    let r = Rect::new(x0, area.y, w + 4.0, area.h);
    if roll >= 0.999 || cur.chars().count() != prev.chars().count() {
        if roll >= 0.999 {
            cv.text(cur, &style, r, cv.theme.ink, Align::Left);
        } else {
            // a different length (9:59 → 10:00): the whole time rolls
            cv.push_clip(area);
            cv.push_alpha(1.0 - roll);
            cv.text(
                prev,
                &style,
                r.translate(0.0, -IDLE_ROLL_DIP * roll),
                cv.theme.ink,
                Align::Left,
            );
            cv.pop_alpha();
            cv.push_alpha(roll);
            cv.text(
                cur,
                &style,
                r.translate(0.0, IDLE_ROLL_DIP * (1.0 - roll)),
                cv.theme.ink,
                Align::Left,
            );
            cv.pop_alpha();
            cv.pop_clip();
        }
        return;
    }
    cv.push_clip(area);
    let mut prefix = String::new();
    for (c, p) in cur.chars().zip(prev.chars()) {
        let x = x0
            + if prefix.is_empty() {
                0.0
            } else {
                cv.measure(&prefix, &style)
            };
        let cw = cv.measure(&c.to_string(), &style) + 2.0;
        let cell = Rect::new(x, area.y, cw, area.h);
        if c == p {
            cv.text(&c.to_string(), &style, cell, cv.theme.ink, Align::Left);
        } else {
            cv.push_alpha(1.0 - roll);
            cv.text(
                &p.to_string(),
                &style,
                cell.translate(0.0, -IDLE_ROLL_DIP * roll),
                cv.theme.ink,
                Align::Left,
            );
            cv.pop_alpha();
            cv.push_alpha(roll);
            cv.text(
                &c.to_string(),
                &style,
                cell.translate(0.0, IDLE_ROLL_DIP * (1.0 - roll)),
                cv.theme.ink,
                Align::Left,
            );
            cv.pop_alpha();
        }
        prefix.push(c);
    }
    cv.pop_clip();
}
