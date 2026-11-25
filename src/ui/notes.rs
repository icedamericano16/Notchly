//! Notes: a sidebar of colour-coded note cards (search, pinned first, checklist progress,
//! a selection that glides between cards) beside a live-preview editor. The first line reads
//! as a bold title, `[ ]` lines are real checkboxes you can tick (even while typing), `- `
//! lines are bullets and finished items are struck through. The caret, selection and
//! click-to-place all come from DirectWrite hit-testing, so they land exactly, wrapped
//! lines included.

use super::productivity::clamp_scroll;
use super::{row, text_input, Reveal};
use crate::app::state::State;
use crate::app::{Action, InputId, PageId};
use crate::features::notetext::{self, LineKind};
use crate::features::store::{Note, Notes};
use crate::features::ListKind;
use crate::render::canvas::v2;
use crate::render::glass::GlassLayer;
use crate::render::icons::Icon;
use crate::render::text::{self, Align};
use crate::render::{Canvas, Rect};
use crate::theme::tokens::*;
use crate::util::{fmt_hm, local_now, unix_secs, unix_to_local, MONTHS, WEEKDAYS};
use windows::core::Interface;
use windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE;
use windows::Win32::Graphics::DirectWrite::*;

const SIDEBAR_W: f32 = 172.0;
const CARD_H: f32 = 46.0;
const CARD_PITCH: f32 = 50.0;
const FOOTER_H: f32 = 26.0;
const TITLE_SIZE: f32 = 16.0;

/// Each note keeps a colour of its own (by id): the card tab, the editor wash, the caret.
pub fn color(id: u64) -> Color {
    const P: [Color; 6] = [
        TINT_CLIP,
        TINT_WEATHER,
        TINT_TODO,
        TINT_MUSIC,
        TINT_PRAYER,
        TINT_WATER,
    ];
    P[(id.wrapping_add(5) % P.len() as u64) as usize]
}

pub fn draw(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    if st.f.notes.current.is_none() {
        if let Some((id, t)) = st.f.notes.notes.first().map(|n| (n.id, n.text.clone())) {
            st.f.notes.current = Some(id);
            st.input.text(InputId::Note).set(&t);
        }
    }
    // the editor field mirrors the open note: load it when they differ (first open after
    // launch, a sync from another PC), keeping the caret where it was when possible
    if let Some(t) = st.f.notes.current().map(|n| n.text.clone()) {
        let f = st.input.text(InputId::Note);
        if f.text != t {
            let caret = f.caret.min(t.chars().count());
            f.text = t;
            f.caret = caret;
            f.anchor = None;
        }
    }
    let all = st.f.notes.notes.clone();
    let cur = st.f.notes.current;
    let live = all
        .iter()
        .filter(|n| !st.f.is_leaving(ListKind::Note, n.id))
        .count();
    let cur_note = cur
        .and_then(|id| all.iter().find(|n| n.id == id))
        .filter(|n| !st.f.is_leaving(ListKind::Note, n.id))
        .cloned();
    let t = st.started.elapsed().as_secs_f32();

    row(cv, rv, 0, area, |cv| {
        header(cv, area, live, cur_note.as_ref())
    });
    let body = Rect::new(area.x, area.y + 32.0, area.w, area.h - 32.0);
    if all.is_empty() {
        st.input.note_editor = None;
        st.input.note_hit = None;
        row(cv, rv, 1, body, |cv| empty_state(cv, body, t, true));
        return;
    }

    let side = Rect::new(body.x, body.y, SIDEBAR_W, body.h);
    let ed = Rect::new(
        side.right() + 10.0,
        body.y,
        body.right() - side.right() - 10.0,
        body.h,
    );
    let search = Rect::new(side.x, side.y, side.w, 28.0);
    row(cv, rv, 1, search, |cv| {
        text_input(
            st,
            cv,
            InputId::NoteSearch,
            search,
            "Search notes",
            Some(Icon::Search),
        )
    });
    let q = st
        .input
        .text_ref(InputId::NoteSearch)
        .map(|f| f.text.clone())
        .unwrap_or_default();
    let mut shown: Vec<Note> = all
        .iter()
        .filter(|n| notetext::matches(&n.text, &q))
        .cloned()
        .collect();
    // pinned first; otherwise the order you made them in (stable sort)
    shown.sort_by_key(|n| !n.pinned);
    let list = Rect::new(
        side.x,
        search.bottom() + 8.0,
        side.w,
        side.bottom() - search.bottom() - 8.0,
    );
    sidebar(st, cv, list, rv, &shown, cur, !q.trim().is_empty());

    match cur_note {
        Some(n) => editor(st, cv, ed, rv, &n, t),
        None => {
            st.input.note_editor = None;
            st.input.note_hit = None;
            row(cv, rv, 2, ed, |cv| {
                cv.glass_tile(ed, 16.0, GlassLayer::Base);
                empty_state(cv, ed, t, false);
            });
        }
    }
}

// ---------------------------------------------------------------- header

fn header(cv: &mut Canvas, area: Rect, count: usize, cur: Option<&Note>) {
    // app chip: a little legal pad
    let chip = Rect::new(area.x, area.y - 1.0, 22.0, 22.0);
    if let Some(g) = cv.gradient(
        v2(chip.x, chip.y),
        v2(chip.right(), chip.bottom()),
        &[(0.0, Color::rgb(0xFFE066)), (1.0, TINT_TIMER)],
        1.0,
    ) {
        cv.rrect_brush(chip, 7.0, crate::render::canvas::as_brush(&g));
    }
    cv.icon(Icon::Note, chip.cx(), chip.cy(), 13.0, Color::rgb(0x3B2600));
    let tw = cv.label(
        "Notes",
        &text::TITLE,
        Rect::new(chip.right() + 8.0, area.y - 1.0, 120.0, 22.0),
        cv.theme.ink,
        Align::Left,
    );
    if count > 0 {
        let s = format!(
            "{count} {}",
            crate::i18n::tr(if count == 1 { "note" } else { "notes" })
        );
        cv.text(
            &s,
            &text::CAPTION,
            Rect::new(chip.right() + 16.0 + tw, area.y, 100.0, 21.0),
            FAINT,
            Align::Left,
        );
    }

    // New: an accent capsule with a plus
    let label = crate::i18n::tr("New");
    let w = cv.measure(&label, &text::CAPTION.weight(700)) + 34.0;
    let r = Rect::new(area.right() - w, area.y - 1.0, w, 24.0);
    let a = Action::NoteNew;
    let hover = cv.hovered(r);
    let pressed = cv.pressed.as_ref() == Some(&a) && hover;
    let (hv, pv) = cv.anim(&a, hover, pressed);
    cv.push_tf(0.0, 0.0, 1.0 + 0.03 * hv - 0.06 * pv, r.cx(), r.cy());
    if let Some(b) = cv.accent_brush(r.x, r.right(), 0.88 + 0.12 * hv) {
        cv.rrect_brush(r, 12.0, crate::render::canvas::as_brush(&b));
    }
    cv.icon(Icon::Plus, r.x + 13.0, r.cy(), 13.0, WHITE);
    cv.text(
        &label,
        &text::CAPTION.weight(700),
        Rect::new(r.x + 22.0, r.y, w - 26.0, r.h),
        WHITE,
        Align::Left,
    );
    cv.pop_tf();
    cv.zone_named(r, a, true, "New note", 12.0);

    // tools for the open note
    let Some(n) = cur else { return };
    let y = area.y + 11.0;
    let mut x = r.x - 18.0;
    let tools: [(Icon, Action, &str, Color); 4] = [
        (
            Icon::Trash,
            Action::NoteDelete(n.id),
            "Delete note",
            cv.theme.mist,
        ),
        (
            Icon::Download,
            Action::NoteExport(n.id),
            "Export as Markdown",
            cv.theme.mist,
        ),
        (
            Icon::Pin,
            Action::NotePin(n.id),
            if n.pinned {
                "Unpin from notch"
            } else {
                "Pin to notch"
            },
            if n.pinned { color(n.id) } else { cv.theme.mist },
        ),
        (
            Icon::CheckCircle,
            Action::NoteChecklist,
            "Checklist",
            cv.theme.mist,
        ),
    ];
    for (icon, action, name, c) in tools {
        if icon == Icon::Pin && n.pinned {
            cv.circle(x, y, 12.0, color(n.id).with_alpha(0.16));
        }
        cv.button(icon, x, y, 26.0, action, name, c);
        x -= 29.0;
    }
}

// ---------------------------------------------------------------- sidebar

fn sidebar(
    st: &mut State,
    cv: &mut Canvas,
    list: Rect,
    rv: &Reveal,
    notes: &[Note],
    cur: Option<u64>,
    searching: bool,
) {
    if notes.is_empty() {
        if searching {
            row(cv, rv, 2, list, |cv| {
                cv.icon(Icon::Search, list.cx(), list.y + 30.0, 20.0, FAINT);
                cv.label(
                    "No matching notes",
                    &text::CAPTION,
                    Rect::new(list.x, list.y + 46.0, list.w, 16.0),
                    cv.theme.mist,
                    Align::Center,
                );
            });
        }
        return;
    }
    let scroll = st.input.scroll_value(PageId::Notes);
    let h24 = st.clock_24h();

    // where the open note's card sits, so the selection can glide there
    let mut yy = 0.0;
    let mut sel_y = None;
    for n in notes {
        if Some(n.id) == cur {
            sel_y = Some(yy);
        }
        yy += CARD_PITCH * st.f.presence(ListKind::Note, n.id).clamp(0.0, 1.0);
    }
    cv.push_clip(Rect::new(list.x - 4.0, list.y, list.w + 8.0, list.h));
    if let (Some(t), Some(id)) = (sel_y, cur) {
        if (st.f.note_sel.target - t).abs() > 0.1 {
            st.f.note_sel.set(t);
        }
        let r = Rect::new(
            list.x,
            list.y - scroll + st.f.note_sel.value,
            list.w,
            CARD_H,
        );
        let c = color(id);
        row(cv, rv, 2, list, |cv| {
            cv.glass_tile(r, 13.0, GlassLayer::Raised);
            // a wash of the note's colour from the left edge
            if let Some(g) = cv.gradient(
                v2(r.x, r.y),
                v2(r.right(), r.y),
                &[(0.0, c.with_alpha(0.20)), (0.75, c.with_alpha(0.0))],
                1.0,
            ) {
                cv.rrect_brush(r, 13.0, crate::render::canvas::as_brush(&g));
            }
            cv.stroke_rrect(r.inset(0.5), 12.5, c.with_alpha(0.30), 1.0);
        });
    }
    let mut y = list.y - scroll;
    for (i, n) in notes.iter().enumerate() {
        let p = st.f.presence(ListKind::Note, n.id);
        let leaving = st.f.is_leaving(ListKind::Note, n.id);
        let (a, dx, dy, hf) = super::row_presence(p, leaving, i);
        let r = Rect::new(list.x + dx, y + dy, list.w, CARD_H);
        y += CARD_PITCH * hf;
        if a <= 0.01 || !cv.visible(r) {
            continue;
        }
        let sel = Some(n.id) == cur;
        cv.push_alpha(a);
        row(cv, rv, 2 + i.min(5), r, |cv| {
            card(cv, r, n, sel, leaving, h24)
        });
        cv.pop_alpha();
    }
    cv.pop_clip();
    clamp_scroll(st, PageId::Notes, y + scroll - list.bottom());
}

fn card(cv: &mut Canvas, r: Rect, n: &Note, sel: bool, leaving: bool, h24: bool) {
    let a = Action::NoteSelect(n.id);
    let hover = cv.hovered(r) && !leaving;
    let pressed = cv.pressed.as_ref() == Some(&a) && hover;
    let (hv, pv) = cv.anim(&a, hover, pressed);
    let hv = hv.clamp(0.0, 1.0);
    let c = color(n.id);
    if !sel && hv > 0.01 {
        cv.sq(r, 13.0, cv.theme.surface.with_alpha(hv));
    }
    cv.push_tf(0.0, 0.0, 1.0 - 0.025 * pv, r.cx(), r.cy());
    // colour tab: taller and solid on the open note
    let tab = if sel {
        r.h - 18.0
    } else {
        r.h - 26.0 + 4.0 * hv
    };
    cv.rrect(
        Rect::new(r.x + 8.0, r.cy() - tab / 2.0, 3.0, tab),
        1.5,
        c.with_alpha(if sel { 1.0 } else { 0.5 + 0.4 * hv }),
    );
    let tx = r.x + 19.0;
    let ts = text::CAPTION.size(10.0);
    let when = when_label(n.updated_at, h24);
    let ww = cv.measure(&when, &ts) + 2.0;
    cv.text(
        &when,
        &ts,
        Rect::new(r.right() - 10.0 - ww, r.y + 7.0, ww, 15.0),
        FAINT,
        Align::Right,
    );
    let mut title_r = r.right() - 14.0 - ww;
    if n.pinned {
        cv.icon(Icon::Pin, title_r - 5.0, r.y + 14.5, 10.0, c);
        title_r -= 13.0;
    }
    let blank = n.text.trim().is_empty();
    let title = if blank {
        crate::i18n::tr("New note")
    } else {
        Notes::first_line(n)
    };
    cv.text(
        &title,
        &text::CAPTION.weight(700),
        Rect::new(tx, r.y + 6.0, title_r - tx, 17.0),
        if blank { cv.theme.mist } else { cv.theme.ink },
        Align::Left,
    );
    // second line: checklist progress, then a preview of the text
    let (done, total) = notetext::checklist(&n.text);
    let mut px = tx;
    let sub = text::CAPTION.size(10.5);
    if total > 0 {
        let all_done = done == total;
        cv.ring(
            px + 5.0,
            r.y + 32.0,
            4.2,
            done as f32 / total as f32,
            1.8,
            Some(if all_done { TINT_TODO } else { c }),
        );
        let s = format!("{done}/{total}");
        let sw = cv.measure(&s, &sub.tnum()) + 2.0;
        cv.text(
            &s,
            &sub.tnum(),
            Rect::new(px + 13.0, r.y + 25.0, sw, 14.0),
            if all_done { TINT_TODO } else { cv.theme.mist },
            Align::Left,
        );
        px += 13.0 + sw + 6.0;
    }
    let preview = notetext::preview(&n.text);
    let preview = if preview.is_empty() && total == 0 {
        crate::i18n::tr("No additional text")
    } else {
        preview
    };
    cv.text(
        &preview,
        &sub,
        Rect::new(px, r.y + 25.0, r.right() - 10.0 - px, 14.0),
        FAINT.lerp(cv.theme.mist, 0.4),
        Align::Left,
    );
    cv.pop_tf();
    if !leaving {
        cv.zone_named(r, a, true, &title, 13.0);
    }
}

/// "now", "5m", "3:04 PM", "Yesterday", "Mon", "12 Nov".
fn when_label(t: i64, h24: bool) -> String {
    let now = unix_secs();
    let d = (now - t).max(0);
    if d < 60 {
        return crate::i18n::tr("now");
    }
    if d < 3600 {
        return format!("{}m", d / 60);
    }
    let lt = unix_to_local(t);
    let days = local_now().day_number() - lt.day_number();
    match days {
        0 => fmt_hm(lt.minutes_of_day(), h24),
        1 => crate::i18n::tr("Yesterday"),
        2..=6 => crate::i18n::tr(WEEKDAYS[lt.weekday as usize % 7]),
        _ => format!(
            "{} {}",
            lt.day,
            crate::i18n::tr(MONTHS[(lt.month as usize).saturating_sub(1) % 12])
        ),
    }
}

// ---------------------------------------------------------------- editor

fn utf16_of(s: &str, ci: usize) -> u32 {
    s.chars().take(ci).map(char::len_utf16).sum::<usize>() as u32
}

fn char_of(s: &str, u: u32) -> usize {
    let mut acc = 0u32;
    for (i, ch) in s.chars().enumerate() {
        if acc >= u {
            return i;
        }
        acc += ch.len_utf16() as u32;
    }
    s.chars().count()
}

struct Rich {
    layout: IDWriteTextLayout,
    height: f32,
}

/// Lays out the note with its formatting applied by range.
fn rich(cv: &mut Canvas, raw: &str, w: f32, c: Color) -> Option<Rich> {
    let disp = notetext::display(raw);
    let lines = notetext::lines(raw);
    let layout = cv.fresh_layout(&disp, &text::BODY, w, 100_000.0)?;
    let var = cv.variable_font();
    let dim = cv.new_brush(FAINT.lerp(cv.theme.mist, 0.3));
    let tint = cv.new_brush(c);
    let mut starts = Vec::with_capacity(lines.len());
    let mut acc = 0u32;
    for l in disp.split('\n') {
        let n = l.encode_utf16().count() as u32;
        starts.push((acc, n));
        acc += n + 1;
    }
    unsafe {
        // the first line is the title
        if let (Some(l0), Some(&(s, n))) = (lines.first(), starts.first()) {
            if l0.kind == LineKind::Plain && n > 0 {
                let rg = DWRITE_TEXT_RANGE {
                    startPosition: s,
                    length: n,
                };
                let _ = layout.SetFontSize(TITLE_SIZE, rg);
                bold(&layout, rg, TITLE_SIZE, var);
            }
        }
        for (l, &(s, n)) in lines.iter().zip(starts.iter()) {
            let m = l.marker as u32;
            match l.kind {
                LineKind::Check(true) if n > m => {
                    let rg = DWRITE_TEXT_RANGE {
                        startPosition: s + m,
                        length: n - m,
                    };
                    let _ = layout.SetStrikethrough(true, rg);
                    if let Some(b) = &dim {
                        if let Ok(u) = b.cast::<windows::core::IUnknown>() {
                            let _ = layout.SetDrawingEffect(&u, rg);
                        }
                    }
                }
                LineKind::Bullet => {
                    let rg = DWRITE_TEXT_RANGE {
                        startPosition: s,
                        length: 1,
                    };
                    bold(&layout, rg, BODY_SIZE, var);
                    if let Some(b) = &tint {
                        if let Ok(u) = b.cast::<windows::core::IUnknown>() {
                            let _ = layout.SetDrawingEffect(&u, rg);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    let mut m = DWRITE_TEXT_METRICS::default();
    unsafe {
        let _ = layout.GetMetrics(&mut m);
    }
    Some(Rich {
        layout,
        height: m.height,
    })
}

/// Bold for a range: through the weight axis on the variable font (a weight set any other way
/// would be overridden by the format's axis values), else the classic weight.
fn bold(layout: &IDWriteTextLayout, rg: DWRITE_TEXT_RANGE, size: f32, variable: bool) {
    use windows::core::Interface;
    unsafe {
        if let Some(l4) = layout
            .cast::<IDWriteTextLayout4>()
            .ok()
            .filter(|_| variable)
        {
            if l4.SetFontAxisValues(&text::axes(700, size), rg).is_ok() {
                return;
            }
        }
        let _ = layout.SetFontWeight(DWRITE_FONT_WEIGHT_BOLD, rg);
    }
}

fn hit_pos(l: &IDWriteTextLayout, u: u32) -> (f32, f32, f32) {
    let (mut x, mut y) = (0.0f32, 0.0f32);
    let mut m = DWRITE_HIT_TEST_METRICS::default();
    unsafe {
        let _ = l.HitTestTextPosition(u, false, &mut x, &mut y, &mut m);
    }
    (x, m.top, m.height.max(14.0))
}

fn hit_range(l: &IDWriteTextLayout, a: u32, b: u32) -> Vec<DWRITE_HIT_TEST_METRICS> {
    let mut n = 0u32;
    unsafe {
        let _ = l.HitTestTextRange(a, b - a, 0.0, 0.0, None, &mut n);
    }
    if n == 0 {
        return Vec::new();
    }
    let mut v = vec![DWRITE_HIT_TEST_METRICS::default(); n as usize];
    unsafe {
        if l.HitTestTextRange(a, b - a, 0.0, 0.0, Some(&mut v), &mut n)
            .is_err()
        {
            return Vec::new();
        }
    }
    v.truncate(n as usize);
    v
}

fn editor(st: &mut State, cv: &mut Canvas, r: Rect, rv: &Reveal, note: &Note, t: f32) {
    let c = color(note.id);
    let focused = st.input.text_focus == Some(InputId::Note);
    let field = st
        .input
        .text_ref(InputId::Note)
        .cloned()
        .unwrap_or_default();
    // switching notes: the page fades and rises in
    if st.f.note_shown != Some(note.id) {
        st.f.note_shown = Some(note.id);
        st.f.note_in.snap(0.0);
        st.f.note_in.set(1.0);
        st.f.note_scroll.snap(0.0);
        st.f.note_caret_seen = None;
    }
    let appear = st.f.note_in.value.clamp(0.0, 1.0);
    let fx = st.f.smooth_progress("note:focus", focused as u8 as f32);
    let recent = unix_secs() - note.updated_at < 1;
    let saved =
        st.f.smooth_progress("note:saved", if recent { 0.0 } else { 1.0 });
    let (done, total) = notetext::checklist(&field.text);
    let prog = st.f.smooth_progress(
        &format!("note:prog:{}", note.id),
        if total == 0 {
            0.0
        } else {
            done as f32 / total as f32
        },
    );
    let ta = Rect::new(
        r.x + 14.0,
        r.y + 11.0,
        r.w - 28.0,
        r.h - 11.0 - FOOTER_H - 4.0,
    );
    st.input.note_editor = Some(r);
    let scroll = st.f.note_scroll.value;
    let h24 = st.clock_24h();
    let blink = focused && (st.input.caret_blink.elapsed().as_millis() / 530).is_multiple_of(2);
    let lines = notetext::lines(&field.text);
    // tick animation per checklist line
    let ticks: Vec<f32> = lines
        .iter()
        .enumerate()
        .map(|(i, l)| match l.kind {
            LineKind::Check(d) => {
                st.f.smooth_progress(&format!("note:chk:{}:{i}", note.id), d as u8 as f32)
            }
            _ => 0.0,
        })
        .collect();

    let mut hit: Option<usize> = None;
    let mut follow: Option<f32> = None;
    let mut content_h = 0.0;
    row(cv, rv, 2, r, |cv| {
        // the sheet: the note's colour washes in from the top, the edge lights while editing
        cv.glass_tile(r, 16.0, GlassLayer::Base);
        if let Some(g) = cv.gradient(
            v2(r.x, r.y),
            v2(r.x, r.y + 72.0),
            &[
                (0.0, c.with_alpha(0.12 + 0.05 * fx)),
                (1.0, c.with_alpha(0.0)),
            ],
            1.0,
        ) {
            cv.rrect_brush(
                Rect::new(r.x, r.y, r.w, 72.0),
                16.0,
                crate::render::canvas::as_brush(&g),
            );
        }
        cv.stroke_rrect(
            r.inset(0.5),
            15.5,
            WHITE.with_alpha(0.05).lerp(c.with_alpha(0.5), fx),
            1.0,
        );
        cv.zone_named(r, Action::Focus(InputId::Note), true, "Note", 16.0);

        let Some(rt) = rich(cv, &field.text, ta.w, c) else {
            return;
        };
        content_h = rt.height;
        let ox = ta.x;
        let oy = ta.y - scroll + (1.0 - appear) * 8.0;
        cv.push_clip(Rect::new(
            r.x + 2.0,
            r.y + 4.0,
            r.w - 4.0,
            ta.bottom() - r.y - 2.0,
        ));
        cv.push_alpha(appear);

        // placeholder
        if field.text.is_empty() {
            cv.label(
                "Title",
                &text::BODY.size(TITLE_SIZE).weight(700),
                Rect::new(ox, oy, ta.w, 21.0),
                FAINT,
                Align::Left,
            );
            cv.label_top(
                "Start writing. [ ] makes a checklist item, - a bullet.",
                &text::CAPTION.lines(3),
                ox,
                oy + 26.0,
                ta.w,
                FAINT,
                Align::Left,
            );
        }

        // selection, behind the text
        if focused {
            if let Some((a, b)) = field.selection() {
                let (ua, ub) = (utf16_of(&field.text, a), utf16_of(&field.text, b));
                for m in hit_range(&rt.layout, ua, ub) {
                    cv.rrect(
                        Rect::new(ox + m.left, oy + m.top, m.width.max(3.0), m.height),
                        3.0,
                        c.with_alpha(0.30),
                    );
                }
            }
        }

        unsafe {
            cv.ctx.DrawTextLayout(
                v2(ox, oy),
                &rt.layout,
                cv.solid(cv.theme.ink),
                D2D1_DRAW_TEXT_OPTIONS_NONE,
            );
        }

        // checkboxes, drawn into the gaps the markers left
        for (i, l) in lines.iter().enumerate() {
            let LineKind::Check(_) = l.kind else { continue };
            let (x, top, h) = hit_pos(&rt.layout, utf16_of(&field.text, l.start));
            let (cx, cy) = (ox + x + 7.5, oy + top + h / 2.0);
            if cy < ta.y - 10.0 || cy > ta.bottom() + 10.0 {
                continue;
            }
            check_box(
                cv,
                cx,
                cy,
                ticks[i],
                c,
                Action::NoteCheck(note.id, i as u32),
            );
        }

        // caret
        let cu = utf16_of(&field.text, field.caret);
        let (x, top, h) = hit_pos(&rt.layout, cu);
        if st.f.note_caret_seen != Some(field.caret) && focused {
            follow = Some(top);
            let _ = h;
        }
        if blink {
            cv.rrect(
                Rect::new(ox + x - 0.8, oy + top + 1.0, 1.7, h - 2.0),
                0.85,
                c,
            );
        }

        // where a click would put the caret
        if let Some((px, py)) = cv.pointer() {
            if r.contains(px, py) {
                let (mut trailing, mut inside) = (Default::default(), Default::default());
                let mut m = DWRITE_HIT_TEST_METRICS::default();
                unsafe {
                    let _ = rt.layout.HitTestPoint(
                        px - ox,
                        py - oy,
                        &mut trailing,
                        &mut inside,
                        &mut m,
                    );
                }
                let u = m.textPosition + if trailing.as_bool() { m.length } else { 0 };
                hit = Some(char_of(&field.text, u));
            }
        }
        cv.pop_alpha();
        cv.pop_clip();

        // soft fades where the text runs past the edges, and a slim scroll thumb
        let over = rt.height - ta.h;
        if over > 1.0 {
            let bg = cv.theme.surface;
            if scroll < over - 1.0 && !cv.is_glass() {
                if let Some(g) = cv.gradient(
                    v2(0.0, ta.bottom() - 16.0),
                    v2(0.0, ta.bottom() + 2.0),
                    &[(0.0, bg.with_alpha(0.0)), (1.0, bg)],
                    1.0,
                ) {
                    cv.rrect_brush(
                        Rect::new(r.x + 2.0, ta.bottom() - 16.0, r.w - 4.0, 18.0),
                        0.0,
                        crate::render::canvas::as_brush(&g),
                    );
                }
            }
            let track = Rect::new(r.right() - 6.0, ta.y + 2.0, 2.5, ta.h - 4.0);
            let th = (track.h * ta.h / rt.height).max(16.0);
            let ty = track.y + (track.h - th) * (scroll / over).clamp(0.0, 1.0);
            cv.rrect(
                Rect::new(track.x, ty, track.w, th),
                1.25,
                WHITE.with_alpha(0.18),
            );
        }

        // footer: when it was edited, length, checklist progress, saved
        let fy = r.bottom() - FOOTER_H;
        cv.line(r.x + 14.0, fy, r.right() - 14.0, fy, 1.0, LINE);
        let fr = Rect::new(r.x + 14.0, fy + 1.0, r.w - 28.0, FOOTER_H - 2.0);
        let small = text::CAPTION.size(10.5);
        // right: saved
        let sl = crate::i18n::tr("Saved");
        let sw = cv.measure(&sl, &small) + 2.0;
        let mut right = fr.right();
        if saved > 0.02 {
            cv.push_alpha(saved);
            cv.text(
                &sl,
                &small,
                Rect::new(right - sw, fr.y, sw, fr.h),
                FAINT,
                Align::Right,
            );
            let pop = 1.0 + 0.25 * (saved * std::f32::consts::PI).sin();
            cv.push_tf(0.0, 0.0, pop, right - sw - 7.0, fr.cy());
            cv.icon(Icon::Check, right - sw - 7.0, fr.cy(), 10.0, TINT_TODO);
            cv.pop_tf();
            cv.pop_alpha();
        } else {
            // typing: three soft dots breathe where "Saved" will appear
            for k in 0..3 {
                let a = 0.35 + 0.45 * ((t * 6.0 - k as f32 * 0.7).sin() * 0.5 + 0.5);
                cv.circle(
                    right - 4.0 - k as f32 * 6.0,
                    fr.cy(),
                    1.6,
                    WHITE.with_alpha(a),
                );
            }
        }
        right -= sw + 20.0;
        // checklist progress bar
        if total > 0 {
            let s = format!("{done}/{total}");
            let tw = cv.measure(&s, &small.tnum()) + 2.0;
            let all_done = done == total;
            cv.text(
                &s,
                &small.tnum(),
                Rect::new(right - tw, fr.y, tw, fr.h),
                if all_done { TINT_TODO } else { cv.theme.mist },
                Align::Right,
            );
            let bar = Rect::new(right - tw - 52.0, fr.cy() - 2.0, 46.0, 4.0);
            cv.rrect(bar, 2.0, TRACK);
            if prog > 0.01 {
                let fill = Rect::new(bar.x, bar.y, bar.w * prog, bar.h);
                let end = if all_done { TINT_TODO } else { c };
                if let Some(g) = cv.gradient(
                    v2(bar.x, 0.0),
                    v2(bar.right(), 0.0),
                    &[(0.0, end.with_alpha(0.7)), (1.0, end)],
                    1.0,
                ) {
                    cv.rrect_brush(fill, 2.0, crate::render::canvas::as_brush(&g));
                }
            }
            right = bar.x - 10.0;
        }
        // left: edited · words
        let n = notetext::words(&field.text);
        let left = format!(
            "{} {} · {n} {}",
            crate::i18n::tr("Edited"),
            when_label(note.updated_at, h24),
            crate::i18n::tr(if n == 1 { "word" } else { "words" })
        );
        if right - fr.x > 40.0 {
            cv.text(
                &left,
                &small,
                Rect::new(fr.x, fr.y, right - fr.x, fr.h),
                FAINT,
                Align::Left,
            );
        }
    });

    st.input.note_hit = hit;
    // keep the caret in view when it moves (typing, arrows, a click)
    if let Some(top) = follow {
        st.f.note_caret_seen = Some(field.caret);
        let line_h = 20.0;
        let s = st.f.note_scroll.target;
        if top < s {
            st.f.note_scroll.set(top.max(0.0));
        } else if top + line_h > s + ta.h {
            st.f.note_scroll.set(top + line_h - ta.h + 4.0);
        }
    }
    let max = (content_h - ta.h + 6.0).max(0.0);
    if st.f.note_scroll.target > max {
        st.f.note_scroll.set(max);
    }
}

/// A rounded checkbox: the note's colour fills in and the tick draws in two strokes.
fn check_box(cv: &mut Canvas, cx: f32, cy: f32, p: f32, c: Color, a: Action) {
    let r = Rect::centered(cx, cy, 13.0, 13.0);
    let hover = cv.hovered(r.inset(-4.0));
    let (hv, pv) = cv.anim(&a, hover, cv.pressed.as_ref() == Some(&a) && hover);
    let pp = p.clamp(0.0, 1.0);
    let pop = 1.0 + 0.18 * (pp * std::f32::consts::PI).sin() - 0.1 * pv + 0.06 * hv.min(1.0);
    cv.push_tf(0.0, 0.0, pop, cx, cy);
    if pp < 0.99 {
        cv.stroke_rrect(
            r.inset(0.6),
            4.0,
            cv.theme.mist.lerp(c, hv.min(1.0)).with_alpha(1.0 - pp),
            1.4,
        );
    }
    if pp > 0.01 {
        let s = 0.55 + 0.45 * pp;
        cv.rrect(
            Rect::centered(cx, cy, 13.0 * s, 13.0 * s),
            4.0 * s,
            c.with_alpha(pp),
        );
        let ink = if c == TINT_CLIP { NOTCH } else { WHITE };
        let (a0, m, e) = (
            (cx - 3.6, cy + 0.2),
            (cx - 1.0, cy + 2.8),
            (cx + 3.8, cy - 2.6),
        );
        let t1 = (pp * 2.0).min(1.0);
        cv.line(
            a0.0,
            a0.1,
            a0.0 + (m.0 - a0.0) * t1,
            a0.1 + (m.1 - a0.1) * t1,
            1.7,
            ink,
        );
        if pp > 0.5 {
            let t2 = (pp - 0.5) * 2.0;
            cv.line(
                m.0,
                m.1,
                m.0 + (e.0 - m.0) * t2,
                m.1 + (e.1 - m.1) * t2,
                1.7,
                ink,
            );
        }
    }
    cv.pop_tf();
    cv.zone_named(r.inset(-4.0), a, true, "Toggle item", 5.0);
}

// ---------------------------------------------------------------- empty state

/// Two sheets drift gently: a sticky note with a ticked line in front of a blue card.
fn empty_state(cv: &mut Canvas, r: Rect, t: f32, with_button: bool) {
    let cx = r.cx();
    let cy = r.y + r.h * if with_button { 0.30 } else { 0.36 };
    let bob = (t * 1.5).sin() * 2.5;
    // back sheet
    cv.push_affine(-10.0, bob * 0.6 - 2.0, 1.0, 1.0, -0.16, cx, cy);
    let b = Rect::centered(cx, cy, 50.0, 60.0);
    cv.rrect(b, 9.0, TINT_WEATHER.with_alpha(0.28));
    for k in 0..3 {
        cv.rrect(
            Rect::new(
                b.x + 9.0,
                b.y + 16.0 + k as f32 * 10.0,
                30.0 - k as f32 * 6.0,
                3.0,
            ),
            1.5,
            WHITE.with_alpha(0.22),
        );
    }
    cv.pop_tf();
    // front sticky note
    cv.push_affine(10.0, bob, 1.0, 1.0, 0.08, cx, cy);
    let f = Rect::centered(cx, cy, 54.0, 62.0);
    if let Some(g) = cv.gradient(
        v2(f.x, f.y),
        v2(f.x, f.bottom()),
        &[(0.0, Color::rgb(0xFFE477)), (1.0, Color::rgb(0xFFC93C))],
        1.0,
    ) {
        cv.rrect_brush(f, 9.0, crate::render::canvas::as_brush(&g));
    }
    let ink = Color::rgb(0x6B4A00);
    cv.rrect(
        Rect::new(f.x + 9.0, f.y + 10.0, 26.0, 4.0),
        2.0,
        ink.with_alpha(0.75),
    );
    for k in 0..2 {
        let y = f.y + 24.0 + k as f32 * 13.0;
        let box_r = Rect::new(f.x + 9.0, y - 3.5, 7.0, 7.0);
        if k == 0 {
            cv.rrect(box_r, 2.0, ink.with_alpha(0.8));
            cv.line(
                box_r.x + 1.6,
                y,
                box_r.x + 3.0,
                y + 1.6,
                1.2,
                Color::rgb(0xFFE477),
            );
            cv.line(
                box_r.x + 3.0,
                y + 1.6,
                box_r.x + 5.6,
                y - 1.8,
                1.2,
                Color::rgb(0xFFE477),
            );
        } else {
            cv.stroke_rrect(box_r, 2.0, ink.with_alpha(0.7), 1.1);
        }
        cv.rrect(
            Rect::new(f.x + 20.0, y - 1.5, 22.0 - k as f32 * 6.0, 3.0),
            1.5,
            ink.with_alpha(if k == 0 { 0.35 } else { 0.6 }),
        );
    }
    cv.pop_tf();
    // a spark that orbits slowly
    let sa = t * 0.9;
    cv.icon(
        Icon::Sparkles,
        cx + 40.0 * sa.cos(),
        cy - 30.0 + 6.0 * sa.sin(),
        11.0,
        TINT_CLIP.with_alpha(0.55 + 0.35 * (t * 2.0).sin().abs()),
    );

    let ty = cy + 42.0;
    cv.label(
        if with_button {
            "No notes yet"
        } else {
            "Pick a note"
        },
        &text::BODY_MED,
        Rect::new(r.x, ty, r.w, 18.0),
        cv.theme.ink,
        Align::Center,
    );
    cv.label(
        "Capture ideas, checklists and reminders",
        &text::CAPTION,
        Rect::new(r.x + 8.0, ty + 18.0, r.w - 16.0, 16.0),
        cv.theme.mist,
        Align::Center,
    );
    if with_button {
        let w = cv.pill_width("New note", 110.0);
        cv.pill_button(
            "New note",
            Rect::new(cx - w / 2.0, ty + 44.0, w, 28.0),
            Action::NoteNew,
            true,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_round_trip() {
        let s = "a😀b\nc";
        assert_eq!(utf16_of(s, 2), 3);
        assert_eq!(char_of(s, 3), 2);
        assert_eq!(char_of(s, 99), s.chars().count());
        for i in 0..=s.chars().count() {
            assert_eq!(char_of(s, utf16_of(s, i)), i);
        }
    }

    #[test]
    fn colours_are_stable() {
        assert_eq!(color(1), TINT_CLIP);
        assert_eq!(color(1), color(7));
        assert_ne!(color(1), color(2));
    }
}
