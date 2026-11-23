//! To-do: quick add with a live "when" preview, filter tabs with a sliding pill, card rows
//! with coloured due chips, animated check-off, snooze and clear-completed.

use super::productivity::clamp_scroll;
use super::{header, row, text_input, Reveal};
use crate::app::state::State;
use crate::app::{Action, InputId, PageId};
use crate::features::todo::{parse_task, Repeat, Task};
use crate::features::ListKind;
use crate::render::icons::Icon;
use crate::render::text::{self, Align};
use crate::render::{Canvas, Rect};
use crate::theme::tokens::*;
use crate::util::{fmt_hm, local_now, unix_secs, unix_to_local};

/// Row pitch in the list (also used by drag-to-reorder).
pub const ROW_PITCH: f32 = 44.0;
const FILTERS: [&str; 4] = ["All", "Today", "Upcoming", "Done"];

fn end_of_today() -> i64 {
    let now = local_now();
    crate::util::local_to_unix(now.day_number(), 24 * 60)
}

/// Whether a task shows under a filter.
pub fn matches(t: &Task, filter: u8) -> bool {
    let eod = end_of_today();
    match filter {
        1 => !t.done && t.due.is_some_and(|d| d < eod),
        2 => !t.done && t.due.is_some_and(|d| d >= eod),
        3 => t.done,
        _ => true,
    }
}

pub fn draw(st: &mut State, cv: &mut Canvas, area: Rect, rv: &Reveal) {
    let tasks = st.f.todo.tasks.clone();
    let filter = st.input.todo_filter.min(3);
    let now = unix_secs();
    let eod = end_of_today();
    let h24 = st.clock_24h();

    // header with today's progress ring
    let body = header(cv, area, Icon::CheckCircle, "To-do", "");
    let due_today = tasks
        .iter()
        .filter(|t| {
            t.due.is_some_and(|d| d < eod)
                && (!t.done || t.done_at.is_some_and(|d| d >= eod - 86_400))
        })
        .count();
    let done_today = st.f.todo.done_today();
    let total = due_today.max(done_today);
    row(cv, rv, 0, area, |cv| {
        let label = format!("{done_today}/{total} {}", crate::i18n::tr("today"));
        let lw = cv.measure(&label, &text::CAPTION) + 2.0;
        let rx = area.right() - lw;
        cv.text(
            &label,
            &text::CAPTION,
            Rect::new(rx, area.y, lw, 20.0),
            cv.theme.mist,
            Align::Right,
        );
        let frac = if total == 0 {
            0.0
        } else {
            done_today as f32 / total as f32
        };
        cv.ring(rx - 14.0, area.y + 10.0, 7.0, frac, 2.5, Some(TINT_TODO));
    });

    // quick add, with a preview of the time it understood
    let input = Rect::new(body.x, body.y, body.w - 42.0, 34.0);
    let pick = st.input.todo_pick;
    let picker_open = st.input.todo_picker;
    // calendar button beside the field: tinted when a date is picked, lit while open
    row(cv, rv, 0, input, |cv| {
        let bx = body.right() - 17.0;
        let by = input.cy();
        let lit = picker_open || pick.due(local_now()).is_some() || pick.repeat.is_some();
        cv.circle(
            bx,
            by,
            17.0,
            if lit {
                TINT_TODO.with_alpha(0.22)
            } else {
                cv.theme.surface
            },
        );
        cv.button(
            Icon::Calendar,
            bx,
            by,
            34.0,
            Action::TodoPicker,
            "Pick a date",
            if lit { TINT_TODO } else { cv.theme.mist },
        );
    });
    row(cv, rv, 0, input, |cv| {
        text_input(
            st,
            cv,
            InputId::Todo,
            input,
            "Add a task — try \"call mom 6pm\"",
            Some(Icon::Plus),
        )
    });
    let typed = st
        .input
        .text_ref(InputId::Todo)
        .map(|t| t.text.clone())
        .unwrap_or_default();
    // the chip shows the picked date, or the one read from the text
    let (due, rep) = match (pick.due(local_now()), pick.repeat) {
        (Some(d), r) => (Some(d), r),
        (None, Some(r)) => (None, Some(r)),
        (None, None) if !typed.trim().is_empty() => {
            let (_, d, r) = parse_task(&typed, local_now());
            (d, r)
        }
        _ => (None, None),
    };
    {
        let label = match (due, rep) {
            (Some(d), r) => {
                let mut s = when_label(d, h24);
                if let Some(r) = r {
                    s.push_str(if r == Repeat::Daily { " ↻" } else { " ↻w" });
                }
                Some(s)
            }
            (None, Some(r)) => Some(crate::i18n::tr(if r == Repeat::Daily {
                "Every day"
            } else {
                "Every week"
            })),
            _ => None,
        };
        if let Some(s) = label {
            let w = cv.measure(&s, &text::CAPTION.weight(600)) + 30.0;
            let chip = Rect::new(input.right() - w - 6.0, input.cy() - 11.0, w, 22.0);
            cv.rrect(chip, 11.0, TINT_TODO.with_alpha(0.16));
            cv.icon(Icon::Calendar, chip.x + 12.0, chip.cy(), 11.0, TINT_TODO);
            cv.text(
                &s,
                &text::CAPTION.weight(600),
                Rect::new(chip.x + 22.0, chip.y, w - 26.0, chip.h),
                TINT_TODO,
                Align::Left,
            );
        }
    }

    // date picker panel (springs open under the field and pushes the list down)
    if (st.f.todo_picker.target - if picker_open { 1.0 } else { 0.0 }).abs() > 0.5 {
        st.f.todo_picker.set(if picker_open { 1.0 } else { 0.0 });
    }
    let pp = st.f.todo_picker.value.clamp(0.0, 1.0);
    let panel_h = 166.0 * pp;
    if panel_h > 1.0 {
        let panel = Rect::new(body.x, input.bottom() + 8.0, body.w, 158.0);
        cv.push_clip(Rect::new(
            body.x - 4.0,
            input.bottom() + 4.0,
            body.w + 8.0,
            panel_h,
        ));
        cv.push_alpha(pp);
        cv.push_tf(0.0, -10.0 * (1.0 - pp), 1.0, panel.cx(), panel.y);
        picker(st, cv, panel, h24);
        cv.pop_tf();
        cv.pop_alpha();
        cv.pop_clip();
    }

    // filter tabs: a pill slides under the active one
    let seg = Rect::new(body.x, input.bottom() + 8.0 + panel_h, body.w, 26.0);
    let counts = [
        tasks.len(),
        tasks.iter().filter(|t| matches(t, 1)).count(),
        tasks.iter().filter(|t| matches(t, 2)).count(),
        tasks.iter().filter(|t| matches(t, 3)).count(),
    ];
    if (st.f.todo_seg.target - filter as f32).abs() > 0.01 {
        st.f.todo_seg.set(filter as f32);
    }
    let pill_x = st.f.todo_seg.value;
    row(cv, rv, 1, seg, |cv| {
        cv.rrect(seg, 13.0, cv.theme.surface);
        let sw = seg.w / 4.0;
        cv.rrect(
            Rect::new(
                seg.x + 2.0 + sw * pill_x,
                seg.y + 2.0,
                sw - 4.0,
                seg.h - 4.0,
            ),
            11.0,
            cv.theme.surface_raised,
        );
        for (i, name) in FILTERS.iter().enumerate() {
            let r = Rect::new(seg.x + sw * i as f32, seg.y, sw, seg.h);
            let active = i as u8 == filter;
            let label = format!("{}  {}", crate::i18n::tr(name), counts[i]);
            cv.text(
                &label,
                &text::CAPTION.weight(if active { 700 } else { 500 }),
                r,
                if active { cv.theme.ink } else { cv.theme.mist },
                Align::Center,
            );
            cv.zone_named(r, Action::TodoFilter(i as u8), true, name, 11.0);
        }
    });

    // list
    let list = Rect::new(
        body.x,
        seg.bottom() + 8.0,
        body.w,
        body.bottom() - seg.bottom() - 8.0,
    );
    let scroll = st.input.scroll_value(PageId::Todo);
    let dragging = st.f.todo_drag.map(|d| d.0);
    let visible: Vec<&Task> = tasks.iter().filter(|t| matches(t, filter)).collect();
    cv.push_clip(list);
    if visible.is_empty() {
        let (msg, icon) = match filter {
            1 => ("Nothing due today", Icon::Sun),
            2 => ("Nothing scheduled", Icon::Calendar),
            3 => ("No completed tasks yet", Icon::Check),
            _ => ("Nothing to do. Enjoy it.", Icon::CheckCircle),
        };
        row(cv, rv, 2, list, |cv| {
            cv.icon(
                icon,
                list.cx(),
                list.cy() - 14.0,
                24.0,
                TINT_TODO.with_alpha(0.7),
            );
            cv.label(
                msg,
                &text::BODY,
                Rect::new(list.x, list.cy() + 4.0, list.w, 18.0),
                cv.theme.mist,
                Align::Center,
            );
        });
    }
    if filter == 3 && !visible.is_empty() {
        let w = cv.pill_width("Clear completed", 120.0);
        let r = Rect::new(list.right() - w, list.y, w, 24.0);
        cv.pill_button("Clear completed", r, Action::TodoClearDone, false);
    }
    let mut y = list.y - scroll
        + if filter == 3 && !visible.is_empty() {
            32.0
        } else {
            0.0
        };
    let mut content = 0.0;
    for (i, t) in visible.iter().enumerate() {
        let p = st.f.presence(ListKind::Todo, t.id);
        let leaving = st.f.is_leaving(ListKind::Todo, t.id);
        let (a, dx, dy, hf) = super::row_presence(p, leaving, i);
        let r = Rect::new(list.x + dx, y + dy, list.w, ROW_PITCH - 6.0);
        y += ROW_PITCH * hf;
        content += ROW_PITCH * hf;
        if !cv.visible(r) || a <= 0.01 {
            continue;
        }
        let tick =
            st.f.tick_anim
                .get(&t.id)
                .map(|s| s.value)
                .unwrap_or(if t.done { 1.0 } else { 0.0 });
        cv.push_alpha(a);
        row(cv, rv, 2 + i.min(6), r, |cv| {
            task_row(
                cv,
                r,
                t,
                tick,
                leaving,
                dragging == Some(t.id),
                filter == 0,
                now,
                h24,
            )
        });
        cv.pop_alpha();
    }
    cv.pop_clip();
    st.input.todo_list_top = list.y - scroll;
    clamp_scroll(st, PageId::Todo, content + 32.0 - list.h);
}

/// Quick choices, a month grid, times and repeat.
fn picker(st: &State, cv: &mut Canvas, p: Rect, h24: bool) {
    cv.sq(p, 16.0, cv.theme.surface);
    let pick = st.input.todo_pick;
    let today = local_now().day_number();
    let inner = p.inset(10.0);

    // quick choices
    let qw = 88.0;
    let quick = [
        ("Today", 0u8, Some(today)),
        ("Tomorrow", 1, Some(today + 1)),
        ("This weekend", 2, None),
        ("Next week", 3, None),
    ];
    for (i, (name, q, day)) in quick.iter().enumerate() {
        let r = Rect::new(inner.x, inner.y + i as f32 * 28.0, qw, 24.0);
        let sel = day.is_some() && pick.day == *day;
        let a = Action::TodoPickQuick(*q);
        let (hv, _) = cv.anim(&a, cv.hovered(r), false);
        cv.rrect(
            r,
            12.0,
            if sel {
                TINT_TODO
            } else {
                cv.theme.surface_raised.lerp(LINE, 1.0 - hv.min(1.0))
            },
        );
        cv.label(
            name,
            &text::CAPTION.weight(600),
            r,
            if sel { NOTCH } else { cv.theme.ink },
            Align::Center,
        );
        cv.zone_named(r, a, true, name, 12.0);
    }
    let clear = Rect::new(inner.x, inner.y + 4.0 * 28.0 + 4.0, qw, 22.0);
    cv.label(
        "Clear",
        &text::CAPTION,
        clear,
        if pick.day.is_some() || pick.minutes.is_some() || pick.repeat.is_some() {
            CRITICAL
        } else {
            FAINT
        },
        Align::Center,
    );
    cv.zone_named(clear, Action::TodoPickClear, true, "Clear", 11.0);

    // month grid
    let (y, m) = st.input.todo_month;
    let (y, m) = if y == 0 {
        let (yy, mm, _) = crate::util::civil_from_days(today);
        (yy, mm)
    } else {
        (y, m)
    };
    let gx = inner.x + qw + 12.0;
    let cell = 22.0;
    let gw = cell * 7.0;
    cv.text(
        &format!("{} {y}", crate::util::MONTHS[m as usize - 1]),
        &text::CAPTION.weight(700),
        Rect::new(gx + 4.0, inner.y, gw - 44.0, 16.0),
        cv.theme.ink,
        Align::Left,
    );
    cv.button(
        Icon::ChevronLeft,
        gx + gw - 30.0,
        inner.y + 8.0,
        20.0,
        Action::TodoPickMonth(-1),
        "Previous month",
        cv.theme.mist,
    );
    cv.button(
        Icon::ChevronRight,
        gx + gw - 8.0,
        inner.y + 8.0,
        20.0,
        Action::TodoPickMonth(1),
        "Next month",
        cv.theme.mist,
    );
    for (i, d) in ["S", "M", "T", "W", "T", "F", "S"].iter().enumerate() {
        cv.text(
            d,
            &text::MICRO,
            Rect::new(gx + i as f32 * cell, inner.y + 18.0, cell, 12.0),
            FAINT,
            Align::Center,
        );
    }
    let first = crate::util::days_from_civil(y, m, 1);
    let lead = crate::util::weekday_of(first) as i64;
    let days_in = crate::util::days_from_civil(
        if m == 12 { y + 1 } else { y },
        if m == 12 { 1 } else { m + 1 },
        1,
    ) - first;
    for k in 0..days_in {
        let d = first + k;
        let idx = lead + k;
        let cx = gx + (idx % 7) as f32 * cell + cell / 2.0;
        let cy = inner.y + 40.0 + (idx / 7) as f32 * 19.0;
        let r = Rect::centered(cx, cy, cell - 2.0, 18.0);
        let sel = pick.day == Some(d);
        let past = d < today;
        let a = Action::TodoPickDay(d);
        let (hv, _) = cv.anim(&a, cv.hovered(r) && !past, false);
        if sel {
            cv.circle(cx, cy, 9.0, TINT_TODO);
        } else if hv > 0.01 {
            cv.circle(cx, cy, 9.0, LINE.with_alpha(hv.min(1.0) * 1.5));
        }
        if d == today && !sel {
            cv.stroke_circle(cx, cy, 9.0, TINT_TODO.with_alpha(0.8), 1.2);
        }
        cv.text(
            &(k + 1).to_string(),
            &text::CAPTION
                .size(10.5)
                .tnum()
                .weight(if sel { 700 } else { 500 }),
            r,
            if sel {
                NOTCH
            } else if past {
                FAINT
            } else {
                cv.theme.ink
            },
            Align::Center,
        );
        if !past {
            cv.zone_named(r, a, false, "", 9.0);
        }
    }

    // times + repeat
    let tx = gx + gw + 12.0;
    let tw = inner.right() - tx;
    cv.label(
        "Time",
        &text::MICRO,
        Rect::new(tx, inner.y, tw, 14.0),
        FAINT,
        Align::Left,
    );
    let times = [9 * 60u32, 12 * 60, 15 * 60, 18 * 60, 20 * 60, 21 * 60 + 30];
    let cw = (tw - 6.0) / 2.0;
    for (i, mins) in times.iter().enumerate() {
        let r = Rect::new(
            tx + (i % 2) as f32 * (cw + 6.0),
            inner.y + 16.0 + (i / 2) as f32 * 26.0,
            cw,
            22.0,
        );
        let sel = pick.minutes == Some(*mins);
        let a = Action::TodoPickTime(*mins);
        let (hv, _) = cv.anim(&a, cv.hovered(r), false);
        cv.rrect(
            r,
            11.0,
            if sel {
                TINT_TODO
            } else {
                cv.theme.surface_raised.lerp(LINE, 1.0 - hv.min(1.0))
            },
        );
        cv.text(
            &fmt_hm(*mins, h24),
            &text::CAPTION.weight(600).size(10.5),
            r,
            if sel { NOTCH } else { cv.theme.ink },
            Align::Center,
        );
        cv.zone_named(r, a, true, "Time", 11.0);
    }
    let ry = inner.y + 16.0 + 3.0 * 26.0 + 6.0;
    cv.label(
        "Repeat",
        &text::MICRO,
        Rect::new(tx, ry, tw, 14.0),
        FAINT,
        Align::Left,
    );
    let reps = [("Once", 0u8), ("Daily", 1), ("Weekly", 2)];
    let rw = tw / 3.0;
    let cur = match pick.repeat {
        Some(Repeat::Daily) => 1,
        Some(Repeat::Weekly) => 2,
        None => 0,
    };
    let seg = Rect::new(tx, ry + 16.0, tw, 22.0);
    cv.rrect(seg, 11.0, cv.theme.surface_raised);
    for (i, (name, v)) in reps.iter().enumerate() {
        let r = Rect::new(tx + i as f32 * rw, seg.y, rw, seg.h);
        if *v == cur {
            cv.rrect(r.inset(2.0), 9.0, TINT_TODO);
        }
        cv.label(
            name,
            &text::CAPTION.size(10.0).weight(600),
            r,
            if *v == cur { NOTCH } else { cv.theme.mist },
            Align::Center,
        );
        cv.zone_named(r, Action::TodoPickRepeat(*v), true, name, 9.0);
    }
}

fn when_label(d: i64, h24: bool) -> String {
    let lt = unix_to_local(d);
    let today = local_now().day_number();
    let time = fmt_hm(lt.minutes_of_day(), h24);
    if lt.day_number() == today {
        time
    } else if lt.day_number() == today + 1 {
        format!("{} {time}", crate::i18n::tr("Tomorrow"))
    } else if lt.day_number() == today - 1 {
        format!("{} {time}", crate::i18n::tr("Yesterday"))
    } else {
        format!(
            "{} {} {time}",
            &crate::util::MONTHS[lt.month as usize - 1][..3],
            lt.day
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn task_row(
    cv: &mut Canvas,
    r: Rect,
    t: &Task,
    tick: f32,
    leaving: bool,
    dragging: bool,
    can_drag: bool,
    now: i64,
    h24: bool,
) {
    if dragging {
        cv.push_tf(0.0, -2.0, 1.02, r.cx(), r.cy());
        cv.sq(r, 12.0, cv.theme.surface_raised);
        cv.pop_tf();
    } else {
        super::hover_card(cv, r, Action::Row(5, t.id), 12.0);
    }
    if can_drag {
        cv.zone(r, Action::TodoRow(t.id));
    }
    let hover = cv.hovered(r) && !leaving;

    // checkbox: a green disc fills and the tick draws in two strokes
    let cb = Rect::centered(r.x + 20.0, r.cy(), 20.0, 20.0);
    let pp = tick.clamp(0.0, 1.0);
    if pp > 0.0 {
        let pop = 1.0 + 0.18 * (tick - pp).abs();
        cv.push_tf(0.0, 0.0, pop, cb.cx(), cb.cy());
        cv.circle(
            cb.cx(),
            cb.cy(),
            10.0 * (0.6 + 0.4 * pp),
            TINT_TODO.with_alpha(pp),
        );
        let (a, m, e) = (
            (cb.x + 5.5, cb.cy() + 0.3),
            (cb.x + 8.8, cb.cy() + 3.6),
            (cb.right() - 5.0, cb.y + 6.5),
        );
        let t1 = (pp * 2.0).min(1.0);
        cv.line(
            a.0,
            a.1,
            a.0 + (m.0 - a.0) * t1,
            a.1 + (m.1 - a.1) * t1,
            2.0,
            WHITE,
        );
        if pp > 0.5 {
            let t2 = (pp - 0.5) * 2.0;
            cv.line(
                m.0,
                m.1,
                m.0 + (e.0 - m.0) * t2,
                m.1 + (e.1 - m.1) * t2,
                2.0,
                WHITE,
            );
        }
        cv.pop_tf();
    } else {
        let ring = if hover { TINT_TODO } else { cv.theme.mist };
        cv.stroke_circle(cb.cx(), cb.cy(), 9.0, ring, 1.6);
    }
    cv.zone_named(
        cb.inset(-6.0),
        Action::TodoToggle(t.id),
        true,
        &format!("Complete {}", t.title),
        10.0,
    );

    // title (struck through as it completes)
    let right_pad = if hover { 74.0 } else { 12.0 };
    let color = cv.theme.ink.lerp(FAINT, pp);
    let tx = r.x + 40.0;
    let mut due_w = 0.0;
    if let Some(d) = t.due {
        let overdue = !t.done && d < now;
        let today = unix_to_local(d).day_number() == local_now().day_number();
        let (bg, fg) = if t.done {
            (cv.theme.surface_raised, FAINT)
        } else if overdue {
            (CRITICAL.with_alpha(0.16), CRITICAL)
        } else if today {
            (TINT_TIMER.with_alpha(0.16), TINT_TIMER)
        } else {
            (cv.theme.surface_raised, cv.theme.mist)
        };
        let mut s = when_label(d, h24);
        if t.repeat.is_some() {
            s.push_str(match t.repeat {
                Some(Repeat::Daily) => " ↻",
                Some(Repeat::Weekly) => " ↻w",
                None => "",
            });
        }
        due_w = cv.measure(&s, &text::CAPTION.weight(600)) + 16.0;
        let chip = Rect::new(r.right() - right_pad - due_w, r.cy() - 10.0, due_w, 20.0);
        cv.rrect(chip, 10.0, bg);
        cv.text(&s, &text::CAPTION.weight(600), chip, fg, Align::Center);
    }
    let tw = cv.text(
        &t.title,
        &text::BODY,
        Rect::new(tx, r.y, r.right() - right_pad - due_w - tx - 8.0, r.h),
        color,
        Align::Left,
    );
    if pp > 0.0 {
        cv.line(tx, r.cy(), tx + tw * pp, r.cy(), 1.2, FAINT);
    }

    // hover actions: snooze (for dated, open tasks) and delete
    if hover {
        if t.due.is_some() && !t.done {
            cv.button(
                Icon::Timer,
                r.right() - 46.0,
                r.cy(),
                26.0,
                Action::TodoSnooze(t.id),
                "Snooze 10 min",
                cv.theme.mist,
            );
        }
        cv.button(
            Icon::Trash,
            r.right() - 18.0,
            r.cy(),
            26.0,
            Action::TodoDelete(t.id),
            "Delete",
            CRITICAL.with_alpha(0.9),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(due: Option<i64>, done: bool) -> Task {
        Task {
            id: 1,
            title: "t".into(),
            due,
            repeat: None,
            done,
            created: 0,
            done_at: None,
            notified: false,
            updated_at: 0,
        }
    }

    #[test]
    fn filters() {
        let now = unix_secs();
        let overdue = task(Some(now - 3600), false);
        let later = task(Some(end_of_today() + 3600), false);
        let undated = task(None, false);
        let done = task(Some(now - 60), true);
        assert!(matches(&overdue, 1) && !matches(&overdue, 2));
        assert!(matches(&later, 2) && !matches(&later, 1));
        assert!(!matches(&undated, 1) && !matches(&undated, 2) && matches(&undated, 0));
        assert!(matches(&done, 3) && !matches(&done, 1));
    }
}
