//! Productivity pages.

use super::{bars, header, row, text_input, Reveal};
use crate::app::state::State;
use crate::app::{Action, InputId, PageId};
use crate::features::store::{ClipKind, ShelfKind};
use crate::render::glass::GlassLayer;
use crate::render::icons::Icon;
use crate::render::image::CachedBitmap;
use crate::render::text::{self, Align};
use crate::render::{shapes, Canvas, Rect};
use crate::theme::tokens::*;
use crate::util::{fmt_hm, fmt_mmss, local_now, unix_secs, unix_to_local};

pub fn water(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    let w = st.f.state.water.clone();
    let p = st.settings.productivity.clone();
    let glass = Rect::new(area.x + 4.0, area.y + 2.0, 74.0, area.h - 4.0);
    let fill = st.f.water_fill.value;
    let t = st.started.elapsed().as_secs_f32();
    let pct = (w.ml as f32 / p.water_goal_ml.max(1) as f32 * 100.0).round() as u32;
    let reached = w.ml >= p.water_goal_ml;
    row(cv, rv, 0, glass, |cv| {
        cv.glass_tile(glass, 18.0, GlassLayer::Base);
        let inner = glass.inset(3.0);
        let level = inner.bottom() - inner.h * fill.clamp(0.0, 1.0);
        let factory = cv.gfx.factory.clone();
        cv.push_clip(inner);
        // two waves at different speeds give the water depth
        for (k, amp, speed, alpha) in [(0, 3.0, 2.2, 0.45), (1, 2.2, -1.6, 0.85)] {
            let pts: Vec<(f32, f32)> = (0..=24)
                .map(|i| {
                    let x = inner.x + inner.w * i as f32 / 24.0;
                    let ph = (i as f32 / 24.0) * std::f32::consts::TAU * 1.3 + t * speed + k as f32;
                    (x, level + amp * ph.sin())
                })
                .chain([(inner.right(), inner.bottom()), (inner.x, inner.bottom())])
                .collect();
            if fill > 0.005 {
                if let Ok(g) = shapes::poly(&factory, &pts, true) {
                    cv.fill_geo(&g, TINT_WATER.with_alpha(alpha));
                }
            }
        }
        // bubbles drift up through the water
        if fill > 0.08 {
            for i in 0..6u32 {
                let seed = i as f32 * 1.618;
                let speed = 9.0 + (seed * 7.3).sin().abs() * 8.0;
                let span = (inner.bottom() - level).max(1.0);
                let rise = (t * speed + seed * 37.0) % span;
                let y = inner.bottom() - rise;
                let x = inner.x
                    + inner.w * (0.2 + 0.6 * ((seed * 3.1).sin() * 0.5 + 0.5))
                    + (t * 1.7 + seed).sin() * 2.0;
                let r = 1.2 + (seed * 5.0).cos().abs() * 1.4;
                cv.stroke_circle(
                    x,
                    y,
                    r,
                    WHITE.with_alpha(0.55 * (rise / span).min(1.0)),
                    0.9,
                );
            }
        }
        cv.pop_clip();
        // glass sheen
        cv.rrect(
            Rect::new(glass.x + 8.0, glass.y + 10.0, 4.0, glass.h * 0.45),
            2.0,
            WHITE.with_alpha(0.07),
        );
        cv.text(
            &format!("{pct}%"),
            &text::CAPTION.weight(700).tnum(),
            Rect::new(glass.x, glass.bottom() - 22.0, glass.w, 16.0),
            if fill > 0.15 {
                NOTCH.with_alpha(0.8)
            } else {
                cv.theme.mist
            },
            Align::Center,
        );
        cv.icon(
            if reached { Icon::Check } else { Icon::Droplet },
            glass.cx(),
            glass.y + 18.0,
            16.0,
            if reached { GOOD } else { TINT_WATER },
        );
    });
    let tx = glass.right() + 16.0;
    let bars_w = 112.0;
    let tw = area.right() - tx - bars_w - 12.0;
    row(cv, rv, 1, area, |cv| {
        let mlw = cv.text(
            &format!("{}", w.ml),
            &text::DISPLAY.size(32.0).tnum(),
            Rect::new(tx, area.y, tw, 38.0),
            cv.theme.ink,
            Align::Left,
        );
        cv.text(
            "ml",
            &text::BODY_MED,
            Rect::new(tx + mlw + 4.0, area.y + 14.0, 40.0, 20.0),
            cv.theme.mist,
            Align::Left,
        );
        let sub = if reached {
            crate::i18n::tr("Goal reached — nicely done")
        } else {
            format!(
                "{} ml {}",
                p.water_goal_ml.saturating_sub(w.ml),
                crate::i18n::tr("to go")
            )
        };
        cv.text(
            &sub,
            &text::CAPTION,
            Rect::new(tx, area.y + 38.0, tw, 16.0),
            if reached { GOOD } else { cv.theme.mist },
            Align::Left,
        );
    });
    row(cv, rv, 2, area, |cv| {
        let add = format!("+ {} ml", p.water_glass_ml);
        let w1 = cv.pill_width(&add, 92.0);
        cv.pill_button(
            &add,
            Rect::new(tx, area.y + 62.0, w1, 30.0),
            Action::WaterAdd,
            true,
        );
        cv.button(
            Icon::Undo,
            tx + w1 + 20.0,
            area.y + 77.0,
            30.0,
            Action::WaterUndo,
            "Undo",
            cv.theme.mist,
        );
        let next = if w.next_reminder > unix_secs() && !reached {
            format!(
                "{} {}",
                crate::i18n::tr("Next reminder"),
                fmt_hm(
                    unix_to_local(w.next_reminder).minutes_of_day(),
                    st.clock_24h()
                )
            )
        } else {
            String::new()
        };
        cv.text(
            &next,
            &text::CAPTION,
            Rect::new(tx, area.y + 98.0, tw + bars_w, 14.0),
            FAINT,
            Align::Left,
        );
    });
    row(cv, rv, 3, area, |cv| {
        let br = Rect::new(area.right() - bars_w, area.y + 18.0, bars_w, 60.0);
        cv.label(
            "This week",
            &text::MICRO,
            Rect::new(br.x, area.y, br.w, 14.0),
            FAINT,
            Align::Left,
        );
        bars(cv, br, &w.week(), p.water_goal_ml, true);
    });
}

pub fn clamp_scroll(st: &mut State, p: PageId, max: f32) {
    let s = st.input.scroll(p);
    let m = max.max(0.0);
    if s.target > m {
        s.set(m);
    }
}

fn file_icon(k: &ShelfKind) -> Icon {
    match k {
        ShelfKind::File { dir: true, .. } => Icon::Folder,
        ShelfKind::File { path, .. } => {
            let l = path.to_lowercase();
            if [".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp"]
                .iter()
                .any(|e| l.ends_with(e))
            {
                Icon::Image
            } else {
                Icon::File
            }
        }
        ShelfKind::Text(_) => Icon::Text,
        ShelfKind::Link(_) => Icon::Link,
        ShelfKind::Image { .. } => Icon::Image,
    }
}

pub fn fmt_size(b: u64) -> String {
    if b >= 1 << 30 {
        format!("{:.1} GB", b as f64 / (1u64 << 30) as f64)
    } else if b >= 1 << 20 {
        format!("{:.1} MB", b as f64 / (1u64 << 20) as f64)
    } else if b >= 1024 {
        format!("{:.0} KB", b as f64 / 1024.0)
    } else {
        format!("{b} B")
    }
}

fn phone_panel(st: &mut State, cv: &mut Canvas, body: Rect, rv: &Reveal) {
    let ph = st.f.phone.clone_view();
    row(cv, rv, 1, body, |cv| {
        let q = Rect::new(body.x + 4.0, body.y, body.h - 4.0, body.h - 4.0);
        cv.sq(q, 10.0, WHITE);
        if let Some(m) = &ph.0 {
            let n = m.len() as f32;
            let cell = (q.w - 12.0) / n;
            for (y, rowv) in m.iter().enumerate() {
                for (x, dark) in rowv.iter().enumerate() {
                    if *dark {
                        cv.rect(
                            Rect::new(
                                q.x + 6.0 + x as f32 * cell,
                                q.y + 6.0 + y as f32 * cell,
                                cell + 0.3,
                                cell + 0.3,
                            ),
                            NOTCH,
                        );
                    }
                }
            }
        }
        let tx = q.right() + 16.0;
        cv.label(
            "Phone Drop",
            &text::TITLE,
            Rect::new(tx, body.y + 4.0, 300.0, 20.0),
            cv.theme.ink,
            Align::Left,
        );
        cv.text(
            &ph.2,
            &text::CAPTION,
            Rect::new(tx, body.y + 26.0, body.right() - tx, 16.0),
            cv.theme.mist,
            Align::Left,
        );
        cv.text(
            &format!("PIN  {}", ph.1),
            &text::DISPLAY.size(22.0),
            Rect::new(tx, body.y + 48.0, 260.0, 28.0),
            cv.theme.ink,
            Align::Left,
        );
        if let Some(p) = ph.3 {
            cv.progress(
                Rect::new(tx, body.y + 84.0, body.right() - tx - 10.0, 4.0),
                p,
                true,
            );
        }
        cv.label(
            "Same Wi-Fi only · stops after 5 min idle",
            &text::CAPTION,
            Rect::new(tx, body.bottom() - 18.0, 300.0, 16.0),
            FAINT,
            Align::Left,
        );
    });
}

impl crate::features::PhoneState {
    fn clone_view(&self) -> (Option<Vec<Vec<bool>>>, String, String, Option<f32>) {
        (
            self.qr.clone(),
            self.pin.clone(),
            self.status.clone(),
            self.progress,
        )
    }
}

pub fn prayer(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    let settings = st.settings.clone();
    let Some(times) = st.f.prayer_times(&settings) else {
        row(cv, rv, 0, area, |cv| {
            cv.icon(
                Icon::MoonStar,
                area.cx(),
                area.cy() - 16.0,
                24.0,
                cv.theme.accent[0],
            );
            cv.label(
                "Set your location in Settings › Productivity",
                &text::BODY,
                Rect::new(area.x, area.cy() + 4.0, area.w, 18.0),
                cv.theme.mist,
                Align::Center,
            );
        });
        return;
    };
    let now = local_now().secs_of_day() / 60.0;
    let h24 = st.clock_24h();
    let arc = Rect::new(area.x, area.y, area.w * 0.52, area.h);
    row(cv, rv, 0, arc, |cv| {
        // day arc: sun travels from Fajr to Isha
        let (x0, x1) = (arc.x + 10.0, arc.right() - 10.0);
        let base = arc.y + arc.h * 0.62;
        let hgt = arc.h * 0.45;
        let pt = |f: f32| {
            (
                x0 + (x1 - x0) * f,
                base - hgt * (f * std::f32::consts::PI).sin(),
            )
        };
        let pts: Vec<(f32, f32)> = (0..=40).map(|i| pt(i as f32 / 40.0)).collect();
        let factory = cv.gfx.factory.clone();
        if let Ok(g) = shapes::poly(&factory, &pts, false) {
            cv.stroke_geo(&g, TRACK, 2.0);
        }
        cv.line(x0 - 6.0, base, x1 + 6.0, base, 1.0, LINE);
        let (start, end) = (times.fajr, times.isha);
        let f = (((now - start) / (end - start)) as f32).clamp(0.0, 1.0);
        let (sx, sy) = pt(f);
        if let Some(g) = cv.radial(
            sx,
            sy,
            14.0,
            14.0,
            WARN.with_alpha(0.5),
            WARN.with_alpha(0.0),
        ) {
            use windows::Win32::Graphics::Direct2D::D2D1_ELLIPSE;
            unsafe {
                cv.ctx.FillEllipse(
                    &D2D1_ELLIPSE {
                        point: crate::render::canvas::v2(sx, sy),
                        radiusX: 14.0,
                        radiusY: 14.0,
                    },
                    &g,
                )
            };
        }
        cv.circle(sx, sy, 5.0, WARN);
        if let Some((i, mins)) = crate::features::prayer::next_prayer(&times, now) {
            cv.text(
                &format!(
                    "{} in {}",
                    times.prayers()[i].0,
                    fmt_mmss((mins * 60.0) as i64)
                ),
                &text::TITLE,
                Rect::new(arc.x, base + 8.0, arc.w, 20.0),
                cv.theme.ink,
                Align::Center,
            );
        } else {
            cv.label(
                "See you at Fajr",
                &text::TITLE,
                Rect::new(arc.x, base + 8.0, arc.w, 20.0),
                cv.theme.ink,
                Align::Center,
            );
        }
        cv.text(
            crate::features::prayer::method(&settings.productivity.prayer_method).name,
            &text::CAPTION,
            Rect::new(arc.x, base + 28.0, arc.w, 14.0),
            FAINT,
            Align::Center,
        );
        let pause_on = settings.productivity.prayer_pause_media;
        cv.button(
            if pause_on { Icon::Pause } else { Icon::Music },
            arc.x + 12.0,
            arc.y + 12.0,
            24.0,
            Action::PrayerPause,
            if pause_on {
                "Media pauses at prayer time"
            } else {
                "Pause media at prayer time"
            },
            if pause_on {
                cv.theme.accent[0]
            } else {
                cv.theme.mist
            },
        );
    });
    let list = Rect::new(
        arc.right() + 12.0,
        area.y,
        area.right() - arc.right() - 12.0,
        area.h,
    );
    let next = crate::features::prayer::next_prayer(&times, now).map(|n| n.0);
    for (i, (name, m)) in times.prayers().iter().enumerate() {
        let r = Rect::new(list.x, list.y + i as f32 * 26.0, list.w, 24.0);
        row(cv, rv, 1 + i, r, |cv| {
            if next == Some(i) {
                cv.glass_tile(r, 10.0, GlassLayer::Raised);
            }
            cv.text(
                name,
                &text::BODY_MED,
                Rect::new(r.x + 10.0, r.y, r.w * 0.5, r.h),
                if next == Some(i) {
                    cv.theme.accent[0]
                } else {
                    cv.theme.ink
                },
                Align::Left,
            );
            let t = if m.is_finite() {
                fmt_hm(*m as u32, h24)
            } else {
                "—".into()
            };
            cv.text(
                &t,
                &text::CAPTION_TNUM.size(12.5),
                Rect::new(r.x, r.y, r.w - 10.0, r.h),
                cv.theme.mist,
                Align::Right,
            );
        });
    }
}

pub fn ocr(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    let body = header(cv, area, Icon::ScanText, "Text from screen", "Ctrl+Alt+T");
    let res = st.f.ocr.result.clone();
    let Some(res) = res else {
        row(cv, rv, 0, body, |cv| {
            cv.label(
                "Capture any region and Notchly copies its text — fully offline.",
                &text::BODY,
                Rect::new(body.x, body.y + 10.0, body.w, 20.0),
                cv.theme.mist,
                Align::Center,
            );
            cv.pill_button(
                "Capture text",
                Rect::centered(body.cx(), body.y + 56.0, 140.0, 30.0),
                Action::OcrStart,
                true,
            );
        });
        return;
    };
    let tr = Rect::new(body.x, body.y, body.w, body.h - 36.0);
    row(cv, rv, 0, tr, |cv| {
        cv.glass_tile(tr, 12.0, GlassLayer::Base);
        cv.push_clip(tr.inset(10.0));
        cv.text_top(
            &res.text,
            &text::BODY.lines(8),
            tr.x + 12.0,
            tr.y + 10.0,
            tr.w - 24.0,
            cv.theme.ink,
            Align::Left,
        );
        cv.pop_clip();
    });
    row(cv, rv, 1, body, |cv| {
        let y = body.bottom() - 26.0;
        let mut x = body.x;
        for (label, a, primary) in [
            ("Copy", Action::OcrCopy, true),
            ("Translate", Action::OcrTranslate, false),
            ("Search", Action::OcrSearch, false),
            ("Again", Action::OcrStart, false),
            ("Save image", Action::OcrSaveImage, false),
        ] {
            let w = cv.measure(label, &text::BODY_MED) + 26.0;
            cv.pill_button(label, Rect::new(x, y, w, 26.0), a, primary);
            x += w + 6.0;
        }
        let quick: Vec<String> = res
            .urls
            .iter()
            .chain(res.emails.iter())
            .chain(res.phones.iter())
            .cloned()
            .collect();
        for (i, q) in quick.iter().take(2).enumerate() {
            let w = (cv.measure(q, &text::CAPTION) + 24.0).min(150.0);
            if x + w > body.right() {
                break;
            }
            let r = Rect::new(x, y, w, 26.0);
            cv.rrect(r, 13.0, cv.theme.accent[1].with_alpha(0.25));
            cv.text(
                q,
                &text::CAPTION,
                r.inset_xy(10.0, 0.0),
                cv.theme.ink,
                Align::Center,
            );
            cv.zone_named(r, Action::OcrOpenUrl(i as u8), true, q, 13.0);
            x += w + 6.0;
        }
    });
}

pub fn shelf(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    shelf_body(st, cv, area, rv);
    super::trash::draw(st, cv, crate::features::ListKind::Shelf, area);
}

fn shelf_body(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    use crate::features::ListKind;
    let live =
        st.f.shelf
            .items
            .iter()
            .filter(|i| !st.f.is_leaving(ListKind::Shelf, i.id))
            .count();
    let body = header(cv, area, Icon::Shelf, "Shelf", "");
    row(cv, rv, 0, area, |cv| {
        let y = area.y + 10.0;
        let mut x = area.right();
        if live > 0 {
            let w = cv.pill_width("Clear all", 76.0);
            cv.pill_button(
                "Clear all",
                Rect::new(x - w, y - 11.0, w, 22.0),
                Action::ShelfClear,
                false,
            );
            x -= w + 8.0;
        }
        let phone_on = st.f.phone.running;
        cv.button(
            Icon::Qr,
            x - 13.0,
            y,
            26.0,
            Action::ShelfReceive,
            if phone_on {
                "Stop Phone Drop"
            } else {
                "Receive from phone"
            },
            if phone_on {
                cv.theme.accent[0]
            } else {
                cv.theme.mist
            },
        );
        x -= 30.0;
        if live > 0 {
            cv.button(
                Icon::Send,
                x - 13.0,
                y,
                26.0,
                Action::ShelfSendPhone,
                "Send to phone",
                cv.theme.mist,
            );
            cv.text(
                &format!(
                    "{live} {}",
                    crate::i18n::tr(if live == 1 { "item" } else { "items" })
                ),
                &text::CAPTION,
                Rect::new(area.x + 80.0, area.y, 120.0, 20.0),
                cv.theme.mist,
                Align::Left,
            );
        }
    });
    if st.f.phone.running {
        phone_panel(st, cv, body, rv);
        return;
    }
    if st.f.shelf.items.is_empty() {
        let swell = st.isl.swell.value;
        row(cv, rv, 1, body, |cv| {
            let r = body.inset(4.0);
            cv.stroke_rrect(r, R_TILE, TINT_SHELF.with_alpha(0.25 + 0.5 * swell), 1.5);
            cv.icon(
                Icon::Shelf,
                body.cx(),
                body.cy() - 12.0 - 4.0 * swell,
                24.0,
                TINT_SHELF.lerp(cv.theme.mist, 1.0 - swell),
            );
            cv.label(
                "Drop files, text or links on the notch",
                &text::BODY,
                Rect::new(body.x, body.cy() + 6.0, body.w, 18.0),
                cv.theme.mist,
                Align::Center,
            );
        });
        return;
    }
    let scroll = st.input.scroll_value(PageId::Shelf);
    let items = st.f.shelf.items.clone();
    st.f.item_rects.retain(|(k, _), _| *k != ListKind::Shelf);
    // the drop clock starts once the page content is showing, so the whole fall is seen
    if st.f.drop_fx.is_some() && !st.f.drop_started && rv.row(1) > 0.35 {
        if let Some((t, _)) = st.f.drop_fx.as_mut() {
            *t = std::time::Instant::now();
        }
        st.f.drop_started = true;
    }
    let drop_fx = st.f.drop_fx.clone().filter(|_| st.f.drop_started);
    let dropping: Vec<u64> =
        st.f.drop_fx
            .as_ref()
            .map(|(_, ids)| ids.clone())
            .unwrap_or_default();
    let mut bursts: Vec<(f32, f32, f32, u64)> = Vec::new();
    cv.push_clip(body);
    let cw = 96.0;
    let mut x = body.x - scroll;
    let mut content = 0.0;
    for (i, it) in items.iter().enumerate() {
        let p = st.f.presence(ListKind::Shelf, it.id);
        let leaving = st.f.is_leaving(ListKind::Shelf, it.id);
        let slot = (cw + 8.0) * p.clamp(0.0, 1.0);
        let r = Rect::new(x - (cw + 8.0 - slot) / 2.0, body.y + 2.0, cw, body.h - 4.0);
        x += slot;
        content += slot;
        if !cv.visible(r) || p <= 0.01 || st.f.thrown(ListKind::Shelf, it.id) {
            continue;
        }
        if !leaving {
            st.f.item_rects
                .insert((ListKind::Shelf, it.id), (r, file_icon(&it.kind)));
        }
        if drop_fx
            .as_ref()
            .is_some_and(|(_, ids)| ids.contains(&it.id))
        {
            bursts.push((r.cx(), r.bottom(), r.w, it.id));
        }
        let selected = st.f.shelf.selected.contains(&it.id);
        let missing = st.f.shelf.missing.contains(&it.id);
        st.f.shelf.thumbs.entry(it.id).or_insert_with(|| {
            it.path()
                .and_then(|pth| crate::platform::dragdrop_thumb(&pth, 128))
                .map(|px| CachedBitmap::new(std::sync::Arc::new(px)))
        });
        // dropped cards fall in with real gravity and bounce down to rest (squash on each
        // landing); other new cards grow in; off-screen deletes scale and fade
        let pc = p.min(1.0);
        let drop_index = dropping.iter().position(|d| *d == it.id);
        let (sx, sy, dy, alpha) = if leaving {
            let s = 0.6 + 0.4 * p;
            (s, s, 0.0, pc)
        } else if let Some(k) = drop_index {
            let t = match &drop_fx {
                Some((t0, _)) => t0.elapsed().as_secs_f32() - k as f32 * SOFT_DROP.stagger,
                None => 0.0, // waiting for the page to show
            };
            let (dy, squash, a) = super::trash::drop_bounce(t);
            // a contact shadow gathers under the card as it nears the surface
            let sh = super::trash::drop_shadow(t);
            if sh > 0.01 {
                let w = r.w * SOFT_DROP.shadow_width * (1.0 + 0.6 * squash.max(0.0));
                if let Some(br) = cv.radial(
                    r.cx(),
                    r.bottom(),
                    w,
                    w * 0.16,
                    NOTCH.with_alpha(SOFT_DROP.shadow_alpha * sh),
                    NOTCH.with_alpha(0.0),
                ) {
                    use windows::core::Interface;
                    if let Ok(br) = br.cast::<windows::Win32::Graphics::Direct2D::ID2D1Brush>() {
                        cv.rrect_brush(
                            Rect::centered(r.cx(), r.bottom(), w * 2.0, w * 0.32),
                            w * 0.16,
                            &br,
                        );
                    }
                }
            }
            // the card widens less than it shortens: it reads as soft, not rubbery
            (1.0 + squash * 0.6, 1.0 - squash, dy, a)
        } else {
            let s = 0.85 + 0.15 * pc;
            (s, s, 0.0, pc)
        };
        cv.push_alpha(alpha.min(1.0));
        cv.push_affine(0.0, dy, sx, sy, 0.0, r.cx(), r.bottom());
        row(cv, rv, 1 + i.min(5), r, |cv| {
            let hover = super::tile(cv, r, Action::ShelfSelect(it.id), &it.name) && !leaving;
            if selected {
                cv.focus_ring(r.inset(3.0), R_TILE - 3.0);
            }
            let thumb_r = Rect::new(r.x + 10.0, r.y + 10.0, r.w - 20.0, r.h - 52.0);
            match st.f.shelf.thumbs.get_mut(&it.id).and_then(|t| t.as_mut()) {
                Some(img) => cv.image(img, thumb_r, 8.0),
                None => cv.icon(
                    file_icon(&it.kind),
                    thumb_r.cx(),
                    thumb_r.cy(),
                    26.0,
                    TINT_SHELF.lerp(cv.theme.mist, 0.3),
                ),
            }
            cv.text(
                &it.name,
                &text::CAPTION.weight(600),
                Rect::new(r.x + 8.0, r.bottom() - 40.0, r.w - 16.0, 16.0),
                if missing { WARN } else { cv.theme.ink },
                Align::Center,
            );
            cv.text(
                &if missing {
                    "Missing".into()
                } else {
                    fmt_size(it.size())
                },
                &text::CAPTION.size(10.5),
                Rect::new(r.x + 8.0, r.bottom() - 24.0, r.w - 16.0, 14.0),
                cv.theme.mist,
                Align::Center,
            );
            // delete button: always reachable, prominent on hover
            let del = (r.right() - 13.0, r.y + 13.0);
            cv.circle(
                del.0,
                del.1,
                10.0,
                NOTCH.with_alpha(if hover { 0.85 } else { 0.55 }),
            );
            cv.button(
                Icon::Trash,
                del.0,
                del.1,
                20.0,
                Action::ShelfRemove(it.id),
                "Delete",
                if hover { CRITICAL } else { cv.theme.mist },
            );
            if hover {
                let y = r.y + 13.0;
                cv.button(
                    Icon::ExternalLink,
                    r.x + 14.0,
                    y,
                    20.0,
                    Action::ShelfOpen(it.id),
                    "Open",
                    cv.theme.ink,
                );
                cv.button(
                    Icon::Folder,
                    r.x + 14.0,
                    y + 22.0,
                    20.0,
                    Action::ShelfReveal(it.id),
                    "Show in folder",
                    cv.theme.ink,
                );
                cv.button(
                    Icon::Copy,
                    r.x + 36.0,
                    y,
                    20.0,
                    Action::ShelfCopy(it.id),
                    "Copy",
                    cv.theme.ink,
                );
                cv.button(
                    Icon::Send,
                    r.x + 36.0,
                    y + 22.0,
                    20.0,
                    Action::ShelfShare(it.id),
                    "Share",
                    cv.theme.ink,
                );
            }
        });
        cv.pop_tf();
        cv.pop_alpha();
    }
    cv.pop_clip();
    if let Some((t0, _)) = drop_fx {
        // the landing effects start as each card touches the cushion
        let t = t0.elapsed().as_secs_f32() - super::trash::DROP_IMPACT;
        for (k, (x, y, w, id)) in bursts.into_iter().enumerate() {
            super::trash::drop_burst(cv, x, y, w, t - k as f32 * SOFT_DROP.stagger, id);
        }
    }
    clamp_scroll(st, PageId::Shelf, content - body.w);
}

pub fn clipboard(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    clipboard_body(st, cv, area, rv);
    super::trash::draw(st, cv, crate::features::ListKind::Clip, area);
}

fn clipboard_body(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    use crate::features::ListKind;
    let body = header(cv, area, Icon::Clipboard, "Clipboard", "");
    let unpinned =
        st.f.clip
            .items
            .iter()
            .filter(|i| !i.pinned && !st.f.is_leaving(ListKind::Clip, i.id))
            .count();
    row(cv, rv, 0, area, |cv| {
        let mut right = area.right();
        if unpinned > 0 {
            let w = cv.pill_width("Clear", 60.0);
            cv.pill_button(
                "Clear",
                Rect::new(area.right() - w, area.y - 1.0, w, 22.0),
                Action::ClipClear,
                false,
            );
            right -= w + 8.0;
        }
        cv.text(
            "Ctrl+Alt+V",
            &text::CAPTION,
            Rect::new(right - 92.0, area.y, 92.0, 20.0),
            FAINT,
            Align::Right,
        );
    });
    let input = Rect::new(body.x, body.y, body.w, 32.0);
    row(cv, rv, 0, input, |cv| {
        text_input(
            st,
            cv,
            InputId::ClipSearch,
            input,
            "Search clipboard",
            Some(Icon::Search),
        )
    });
    let q = st
        .input
        .text_ref(InputId::ClipSearch)
        .map(|t| t.text.clone())
        .unwrap_or_default();
    let items: Vec<crate::features::store::ClipItem> =
        st.f.clip.filtered(&q).into_iter().cloned().collect();
    let list = Rect::new(
        body.x,
        input.bottom() + 8.0,
        body.w,
        body.bottom() - input.bottom() - 8.0,
    );
    let scroll = st.input.scroll_value(PageId::Clipboard);
    cv.push_clip(list);
    if items.is_empty() {
        cv.text(
            if q.is_empty() {
                "Copy something and it shows up here"
            } else {
                "No matches"
            },
            &text::BODY,
            list,
            cv.theme.mist,
            Align::Center,
        );
    }
    let mut y = list.y - scroll;
    let mut content = 0.0;
    st.f.item_rects.retain(|(k, _), _| *k != ListKind::Clip);
    for (i, it) in items.iter().enumerate() {
        let p = st.f.presence(ListKind::Clip, it.id);
        let leaving = st.f.is_leaving(ListKind::Clip, it.id);
        let full = if it.kind == ClipKind::Image {
            60.0
        } else {
            42.0
        };
        let hgt = full * p.clamp(0.0, 1.0);
        // leaving rows slide left and collapse; new rows drop in
        let dx = if leaving { -(1.0 - p) * 80.0 } else { 0.0 };
        let dy = if leaving {
            0.0
        } else {
            -(1.0 - p.min(1.0)) * 10.0
        };
        let r = Rect::new(list.x + dx, y + dy, list.w, full - 4.0);
        y += hgt;
        content += hgt;
        if !cv.visible(r) || p <= 0.01 || st.f.thrown(ListKind::Clip, it.id) {
            continue;
        }
        if !leaving && r.y >= list.y - r.h * 0.5 && r.bottom() <= list.bottom() + r.h * 0.5 {
            let icon = match it.kind {
                ClipKind::Link => Icon::Link,
                ClipKind::Image => Icon::Image,
                ClipKind::Color => Icon::Palette,
                ClipKind::Text => Icon::Text,
            };
            st.f.item_rects.insert((ListKind::Clip, it.id), (r, icon));
        }
        if it.kind == ClipKind::Image && !st.f.clip.thumbs.contains_key(&it.id) {
            let px = it.image.as_ref().and_then(|f| {
                crate::render::image::decode_file(&crate::util::sub_dir("clipboard").join(f), 160)
                    .ok()
            });
            st.f.clip
                .thumbs
                .insert(it.id, px.map(|p| CachedBitmap::new(std::sync::Arc::new(p))));
        }
        cv.push_alpha(p.min(1.0));
        row(cv, rv, 1 + i.min(6), r, |cv| {
            let hover = cv.hovered(r) && !leaving;
            let (hv, _) = cv.anim(&Action::ClipCopy(it.id), hover, false);
            cv.sq(
                r,
                12.0,
                cv.theme
                    .surface
                    .lerp(cv.theme.surface_raised, hv.clamp(0.0, 1.0)),
            );
            cv.zone_named(r, Action::ClipCopy(it.id), true, &it.text, 12.0);
            let ix = r.x + 18.0;
            match it.kind {
                ClipKind::Color => {
                    if let Some(c) = Color::parse(&it.text) {
                        cv.circle(ix, r.cy(), 8.0, c);
                        cv.stroke_circle(ix, r.cy(), 8.0, RING, 1.0);
                    }
                }
                ClipKind::Image => {
                    if let Some(img) = st.f.clip.thumbs.get_mut(&it.id).and_then(|t| t.as_mut()) {
                        cv.image(img, Rect::new(r.x + 8.0, r.y + 6.0, 64.0, r.h - 12.0), 6.0);
                    }
                }
                ClipKind::Link => cv.icon(Icon::Link, ix, r.cy(), 15.0, TINT_WEATHER),
                ClipKind::Text => cv.icon(Icon::Text, ix, r.cy(), 15.0, cv.theme.mist),
            }
            let tx = if it.kind == ClipKind::Image {
                r.x + 82.0
            } else {
                r.x + 36.0
            };
            let first = it.text.lines().next().unwrap_or("").trim();
            cv.text(
                first,
                &text::BODY,
                Rect::new(tx, r.y, r.right() - tx - 64.0, r.h),
                cv.theme.ink,
                Align::Left,
            );
            if it.pinned || hover {
                cv.button(
                    Icon::Pin,
                    r.right() - 44.0,
                    r.cy(),
                    24.0,
                    Action::ClipPin(it.id),
                    if it.pinned { "Unpin" } else { "Pin" },
                    if it.pinned { TINT_CLIP } else { cv.theme.mist },
                );
            }
            if hover {
                cv.button(
                    Icon::Trash,
                    r.right() - 18.0,
                    r.cy(),
                    24.0,
                    Action::ClipDelete(it.id),
                    "Delete",
                    CRITICAL.with_alpha(0.9),
                );
            }
        });
        cv.pop_alpha();
    }
    cv.pop_clip();
    clamp_scroll(st, PageId::Clipboard, content - list.h);
}
