//! The light formatting notes understand, as plain text so it syncs and exports cleanly:
//! the first line is the title, `[ ] ` / `[x] ` start checklist items and `- ` starts a bullet.
//! The editor draws a "display" copy of the text in which those markers become a checkbox
//! gap or a bullet. Markers are swapped for the same number of characters (all in the Basic
//! Multilingual Plane), so caret and selection positions are identical in both.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    Plain,
    Check(bool),
    Bullet,
}

/// One line of a note. `start` and `len` are in chars; `marker` is how many chars at the
/// start of the line are formatting (0 for plain lines).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Line {
    pub kind: LineKind,
    pub start: usize,
    pub len: usize,
    pub marker: usize,
}

/// The gap a checkbox is drawn into (en spaces, about a checkbox wide at body size).
pub const CHECK_GAP: char = '\u{2002}';

fn classify(line: &str) -> (LineKind, usize) {
    for (p, done) in [("[ ]", false), ("[x]", true), ("[X]", true)] {
        if let Some(rest) = line.strip_prefix(p) {
            if rest.is_empty() {
                return (LineKind::Check(done), 3);
            }
            if rest.starts_with(' ') {
                return (LineKind::Check(done), 4);
            }
        }
    }
    if line.starts_with("- ") || line.starts_with("* ") || line.starts_with("• ") {
        return (LineKind::Bullet, 2);
    }
    (LineKind::Plain, 0)
}

pub fn lines(text: &str) -> Vec<Line> {
    let mut out = Vec::new();
    let mut start = 0;
    for l in text.split('\n') {
        let len = l.chars().count();
        let (kind, marker) = classify(l);
        out.push(Line {
            kind,
            start,
            len,
            marker,
        });
        start += len + 1;
    }
    out
}

/// The text as the editor draws it (same char count, markers replaced).
pub fn display(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    for (i, l) in text.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let (kind, marker) = classify(l);
        let rest: String = l.chars().skip(marker).collect();
        match kind {
            LineKind::Check(_) => out.extend(std::iter::repeat_n(CHECK_GAP, marker)),
            LineKind::Bullet => out.push_str("• "),
            LineKind::Plain => {}
        }
        out.push_str(&rest);
    }
    out
}

/// (done, total) checklist items.
pub fn checklist(text: &str) -> (usize, usize) {
    let ls = lines(text);
    let total = ls
        .iter()
        .filter(|l| matches!(l.kind, LineKind::Check(_)))
        .count();
    let done = ls
        .iter()
        .filter(|l| l.kind == LineKind::Check(true))
        .count();
    (done, total)
}

/// Ticks or unticks the checklist item on `line`. None if that line isn't one.
pub fn toggle_check(text: &str, line: usize) -> Option<String> {
    let mut parts: Vec<String> = text.split('\n').map(str::to_string).collect();
    let l = parts.get_mut(line)?;
    let (kind, _) = classify(l);
    let LineKind::Check(done) = kind else {
        return None;
    };
    l.replace_range(0..3, if done { "[ ]" } else { "[x]" });
    Some(parts.join("\n"))
}

fn line_at(text: &str, caret: usize) -> Option<Line> {
    lines(text)
        .into_iter()
        .find(|l| caret >= l.start && caret <= l.start + l.len)
}

fn byte_of(text: &str, ci: usize) -> usize {
    text.char_indices()
        .nth(ci)
        .map(|(b, _)| b)
        .unwrap_or(text.len())
}

/// Turns the caret's line into a checklist item, or back into plain text.
/// Returns the new text and caret.
pub fn toggle_checklist(text: &str, caret: usize) -> (String, usize) {
    let Some(l) = line_at(text, caret) else {
        return (text.to_string(), caret);
    };
    let b = byte_of(text, l.start);
    let mut s = text.to_string();
    match l.kind {
        LineKind::Check(_) => {
            let e = byte_of(text, l.start + l.marker);
            s.replace_range(b..e, "");
            (s, caret.saturating_sub(l.marker).max(l.start))
        }
        LineKind::Bullet => {
            let e = byte_of(text, l.start + l.marker);
            s.replace_range(b..e, "[ ] ");
            (s, caret + 2)
        }
        LineKind::Plain => {
            s.insert_str(b, "[ ] ");
            (s, caret + 4)
        }
    }
}

/// Enter inside a list: continues it with a fresh item, or ends it when the item is empty.
/// None means "just insert a newline".
pub fn on_enter(text: &str, caret: usize) -> Option<(String, usize)> {
    let l = line_at(text, caret)?;
    if l.marker == 0 || caret < l.start + l.marker {
        return None;
    }
    let mut s = text.to_string();
    if l.len == l.marker
        || text
            .chars()
            .skip(l.start + l.marker)
            .take(l.len - l.marker)
            .all(char::is_whitespace)
    {
        // empty item: drop the marker and leave the list
        let (b, e) = (byte_of(text, l.start), byte_of(text, l.start + l.len));
        s.replace_range(b..e, "");
        return Some((s, l.start));
    }
    let next = match l.kind {
        LineKind::Check(_) => "\n[ ] ",
        _ => "\n- ",
    };
    s.insert_str(byte_of(text, caret), next);
    Some((s, caret + next.chars().count()))
}

/// The first line with content after the title, without its marker (the card preview).
pub fn preview(text: &str) -> String {
    let mut it = text
        .split('\n')
        .map(|l| {
            let (_, m) = classify(l);
            l.chars().skip(m).collect::<String>().trim().to_string()
        })
        .filter(|l| !l.is_empty());
    it.next();
    it.next().unwrap_or_default()
}

/// Whether a note matches a search (case-insensitive; an empty query matches everything).
pub fn matches(text: &str, q: &str) -> bool {
    let q = q.trim();
    q.is_empty() || text.to_lowercase().contains(&q.to_lowercase())
}

/// Start and end (chars) of the line holding the caret.
pub fn line_bounds(text: &str, caret: usize) -> (usize, usize) {
    line_at(text, caret)
        .map(|l| (l.start, l.start + l.len))
        .unwrap_or((0, text.chars().count()))
}

/// The caret one line up or down, keeping its column where the line is long enough.
pub fn vertical(text: &str, caret: usize, up: bool) -> usize {
    let ls = lines(text);
    let Some(i) = ls
        .iter()
        .position(|l| caret >= l.start && caret <= l.start + l.len)
    else {
        return caret;
    };
    let col = caret - ls[i].start;
    let j = if up {
        match i.checked_sub(1) {
            Some(j) => j,
            None => return 0,
        }
    } else if i + 1 < ls.len() {
        i + 1
    } else {
        return text.chars().count();
    };
    ls[j].start + col.min(ls[j].len)
}

pub fn words(text: &str) -> usize {
    text.split_whitespace()
        .filter(|w| !matches!(*w, "[" | "]" | "[x]" | "[X]" | "-" | "*" | "•"))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_keeps_positions() {
        let t = "Shopping\n[ ] milk\n[x] eggs\n- bread\n[ ]\nplain [ ] text";
        let d = display(t);
        assert_eq!(d.chars().count(), t.chars().count());
        assert_eq!(d.encode_utf16().count(), t.encode_utf16().count());
        assert!(d.contains("• bread"));
        assert!(d.ends_with("plain [ ] text"));
        assert!(!d.contains("[x]"));
    }

    #[test]
    fn kinds_and_progress() {
        let t = "Title\n[ ] a\n[x] b\n[X] c\n- d\n[]x";
        let ls = lines(t);
        assert_eq!(ls[0].kind, LineKind::Plain);
        assert_eq!(ls[1].kind, LineKind::Check(false));
        assert_eq!(ls[2].kind, LineKind::Check(true));
        assert_eq!(ls[4].kind, LineKind::Bullet);
        assert_eq!(ls[5].kind, LineKind::Plain);
        assert_eq!(ls[1].start, 6);
        assert_eq!(checklist(t), (2, 3));
    }

    #[test]
    fn toggling() {
        let t = "T\n[ ] a\n[x] b";
        assert_eq!(toggle_check(t, 1).unwrap(), "T\n[x] a\n[x] b");
        assert_eq!(toggle_check(t, 2).unwrap(), "T\n[ ] a\n[ ] b");
        assert!(toggle_check(t, 0).is_none());
        assert!(toggle_check(t, 9).is_none());
        // the toolbar button adds / removes the marker on the caret's line
        assert_eq!(toggle_checklist("T\nmilk", 4), ("T\n[ ] milk".into(), 8));
        assert_eq!(toggle_checklist("T\n[ ] milk", 8), ("T\nmilk".into(), 4));
        assert_eq!(toggle_checklist("- milk", 3), ("[ ] milk".into(), 5));
    }

    #[test]
    fn enter_continues_lists() {
        assert_eq!(on_enter("[ ] milk", 8), Some(("[ ] milk\n[ ] ".into(), 13)));
        assert_eq!(on_enter("[x] milk", 8), Some(("[x] milk\n[ ] ".into(), 13)));
        assert_eq!(on_enter("- a", 3), Some(("- a\n- ".into(), 6)));
        // an empty item ends the list
        assert_eq!(on_enter("[ ] a\n[ ] ", 10), Some(("[ ] a\n".into(), 6)));
        assert_eq!(on_enter("plain", 5), None);
        // caret inside the marker: a plain newline
        assert_eq!(on_enter("[ ] a", 1), None);
    }

    #[test]
    fn caret_movement_and_search() {
        let t = "Title\nab\nlonger line";
        assert_eq!(line_bounds(t, 7), (6, 8));
        assert_eq!(vertical(t, 4, false), 8); // col 4 clamps to "ab"
        assert_eq!(vertical(t, 7, false), 10);
        assert_eq!(vertical(t, 10, true), 7);
        assert_eq!(vertical(t, 2, true), 0);
        assert_eq!(vertical(t, 12, false), t.chars().count());
        assert!(matches(t, " LONGER "));
        assert!(matches(t, ""));
        assert!(!matches(t, "zebra"));
    }

    #[test]
    fn preview_and_words() {
        assert_eq!(preview("Title\n\n[ ] milk\nmore"), "milk");
        assert_eq!(preview("Only"), "");
        assert_eq!(words("Title\n[ ] buy milk\n- eggs"), 4);
    }
}
