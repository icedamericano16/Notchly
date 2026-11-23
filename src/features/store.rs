//! Shelf, clipboard history, notes and notification history data.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::app::activity::Popup;
use crate::render::image::CachedBitmap;
use crate::util::unix_secs;

// ---------------------------------------------------------------- shelf

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ShelfKind {
    File {
        path: String,
        size: u64,
        mtime: i64,
        dir: bool,
    },
    Text(String),
    Link(String),
    /// PNG copy stored under shelf/.
    Image {
        file: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ShelfItem {
    pub id: u64,
    pub kind: ShelfKind,
    pub name: String,
    pub added: i64,
}

impl ShelfItem {
    pub fn path(&self) -> Option<String> {
        match &self.kind {
            ShelfKind::File { path, .. } => Some(path.clone()),
            ShelfKind::Image { file } => Some(
                crate::util::sub_dir("shelf")
                    .join(file)
                    .to_string_lossy()
                    .to_string(),
            ),
            _ => None,
        }
    }
    pub fn size(&self) -> u64 {
        match &self.kind {
            ShelfKind::File { size, .. } => *size,
            ShelfKind::Text(t) | ShelfKind::Link(t) => t.len() as u64,
            ShelfKind::Image { file } => {
                std::fs::metadata(crate::util::sub_dir("shelf").join(file))
                    .map(|m| m.len())
                    .unwrap_or(0)
            }
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Shelf {
    pub items: Vec<ShelfItem>,
    pub next_id: u64,
    #[serde(skip)]
    pub selected: HashSet<u64>,
    #[serde(skip)]
    pub thumbs: HashMap<u64, Option<CachedBitmap>>,
    #[serde(skip)]
    pub missing: HashSet<u64>,
}

pub const SHELF_COPY_CAP: u64 = 20 * 1024 * 1024;

impl Shelf {
    pub fn add(&mut self, kind: ShelfKind, name: String) -> Option<u64> {
        // dedupe files by path
        if let ShelfKind::File { path, .. } = &kind {
            if let Some(i) = self
                .items
                .iter()
                .find(|i| matches!(&i.kind, ShelfKind::File { path: p, .. } if p == path))
            {
                return Some(i.id);
            }
        }
        if matches!(
            kind,
            ShelfKind::Text(_) | ShelfKind::Link(_) | ShelfKind::Image { .. }
        ) {
            let copied: u64 = self
                .items
                .iter()
                .filter(|i| !matches!(i.kind, ShelfKind::File { .. }))
                .map(|i| i.size())
                .sum();
            if copied > SHELF_COPY_CAP {
                return None;
            }
        }
        self.next_id += 1;
        self.items.insert(
            0,
            ShelfItem {
                id: self.next_id,
                kind,
                name,
                added: unix_secs(),
            },
        );
        Some(self.next_id)
    }
    pub fn add_path(&mut self, path: &str) -> Option<u64> {
        let meta = std::fs::metadata(path).ok()?;
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let name = std::path::Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string());
        self.add(
            ShelfKind::File {
                path: path.to_string(),
                size: meta.len(),
                mtime,
                dir: meta.is_dir(),
            },
            name,
        )
    }
    pub fn add_text(&mut self, text: &str) -> Option<u64> {
        let t = text.trim();
        if t.is_empty() {
            return None;
        }
        let is_link = (t.starts_with("http://") || t.starts_with("https://"))
            && !t.contains(char::is_whitespace);
        let name: String = t.chars().take(40).collect();
        self.add(
            if is_link {
                ShelfKind::Link(t.to_string())
            } else {
                ShelfKind::Text(t.to_string())
            },
            name,
        )
    }
    pub fn remove(&mut self, id: u64) {
        if let Some(i) = self.items.iter().position(|i| i.id == id) {
            if let ShelfKind::Image { file } = &self.items[i].kind {
                let _ = std::fs::remove_file(crate::util::sub_dir("shelf").join(file));
            }
            self.items.remove(i);
        }
        self.selected.remove(&id);
        self.thumbs.remove(&id);
    }
    /// Marks file items whose path no longer exists or changed.
    pub fn check_missing(&mut self) {
        self.missing.clear();
        for i in &self.items {
            if let ShelfKind::File { path, .. } = &i.kind {
                if !std::path::Path::new(path).exists() {
                    self.missing.insert(i.id);
                }
            }
        }
    }
    pub fn auto_clear(&mut self, days: u32) {
        if days == 0 {
            return;
        }
        let cutoff = unix_secs() - days as i64 * 86400;
        let old: Vec<u64> = self
            .items
            .iter()
            .filter(|i| i.added < cutoff)
            .map(|i| i.id)
            .collect();
        for id in old {
            self.remove(id);
        }
    }
    pub fn get(&self, id: u64) -> Option<&ShelfItem> {
        self.items.iter().find(|i| i.id == id)
    }
}

// ---------------------------------------------------------------- clipboard

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ClipKind {
    Text,
    Link,
    Color,
    Image,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ClipItem {
    pub id: u64,
    pub kind: ClipKind,
    pub text: String,
    /// PNG file name in clipboard/ for images.
    pub image: Option<String>,
    pub pinned: bool,
    pub time: i64,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ClipHistory {
    pub items: Vec<ClipItem>,
    pub next_id: u64,
    #[serde(skip)]
    pub thumbs: HashMap<u64, Option<CachedBitmap>>,
    #[serde(skip)]
    pub ignore_seq: u32,
}

/// Detects a CSS-style hex colour.
pub fn is_color(s: &str) -> bool {
    let t = s.trim();
    let h = t.strip_prefix('#').unwrap_or("");
    (h.len() == 6 || h.len() == 3 || h.len() == 8) && h.chars().all(|c| c.is_ascii_hexdigit())
}

impl ClipHistory {
    pub fn add_text(&mut self, text: &str, max: usize) -> bool {
        let t = text.trim_end_matches('\0');
        if t.trim().is_empty() || t.len() > 200_000 {
            return false;
        }
        if let Some(pos) = self
            .items
            .iter()
            .position(|i| i.kind != ClipKind::Image && i.text == t)
        {
            // move to top
            let mut it = self.items.remove(pos);
            it.time = unix_secs();
            self.items.insert(0, it);
            return true;
        }
        let kind = if is_color(t) {
            ClipKind::Color
        } else if (t.starts_with("http://") || t.starts_with("https://"))
            && !t.trim().contains(char::is_whitespace)
        {
            ClipKind::Link
        } else {
            ClipKind::Text
        };
        self.next_id += 1;
        self.items.insert(
            0,
            ClipItem {
                id: self.next_id,
                kind,
                text: t.to_string(),
                image: None,
                pinned: false,
                time: unix_secs(),
            },
        );
        self.trim(max);
        true
    }
    pub fn add_image(&mut self, file: String, w: u32, h: u32, max: usize) {
        self.next_id += 1;
        self.items.insert(
            0,
            ClipItem {
                id: self.next_id,
                kind: ClipKind::Image,
                text: format!("Image {w}×{h}"),
                image: Some(file),
                pinned: false,
                time: unix_secs(),
            },
        );
        self.trim(max);
    }
    fn trim(&mut self, max: usize) {
        let dir = crate::util::sub_dir("clipboard");
        while self.items.iter().filter(|i| !i.pinned).count() > max {
            if let Some(pos) = self.items.iter().rposition(|i| !i.pinned) {
                let it = self.items.remove(pos);
                if let Some(f) = it.image {
                    let _ = std::fs::remove_file(dir.join(f));
                }
                self.thumbs.remove(&it.id);
            } else {
                break;
            }
        }
    }
    pub fn toggle_pin(&mut self, id: u64) {
        if let Some(i) = self.items.iter_mut().find(|i| i.id == id) {
            i.pinned = !i.pinned;
        }
    }
    pub fn delete(&mut self, id: u64) {
        if let Some(pos) = self.items.iter().position(|i| i.id == id) {
            let it = self.items.remove(pos);
            if let Some(f) = it.image {
                let _ = std::fs::remove_file(crate::util::sub_dir("clipboard").join(f));
            }
        }
    }
    /// Pinned first, then most recent, filtered by a case-insensitive query.
    pub fn filtered(&self, q: &str) -> Vec<&ClipItem> {
        let q = q.to_lowercase();
        let mut v: Vec<&ClipItem> = self
            .items
            .iter()
            .filter(|i| q.is_empty() || i.text.to_lowercase().contains(&q))
            .collect();
        v.sort_by_key(|i| (!i.pinned, std::cmp::Reverse(i.time)));
        v
    }
}

// ---------------------------------------------------------------- notes

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Note {
    pub id: u64,
    pub text: String,
    pub pinned: bool,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Notes {
    pub notes: Vec<Note>,
    pub next_id: u64,
    pub current: Option<u64>,
}

impl Notes {
    pub fn new_note(&mut self) -> u64 {
        self.next_id += 1;
        self.notes.insert(
            0,
            Note {
                id: self.next_id,
                text: String::new(),
                pinned: false,
                updated_at: unix_secs(),
            },
        );
        self.current = Some(self.next_id);
        self.next_id
    }
    pub fn current_mut(&mut self) -> Option<&mut Note> {
        let id = self.current?;
        self.notes.iter_mut().find(|n| n.id == id)
    }
    pub fn current(&self) -> Option<&Note> {
        let id = self.current?;
        self.notes.iter().find(|n| n.id == id)
    }
    pub fn first_line(n: &Note) -> String {
        n.text
            .lines()
            .find(|l| !l.trim().is_empty())
            .map(|l| {
                l.trim_start_matches("[ ]")
                    .trim_start_matches("[x]")
                    .trim()
                    .to_string()
            })
            .unwrap_or_else(|| "Empty note".into())
    }
    pub fn to_markdown(n: &Note) -> String {
        n.text
            .lines()
            .map(|l| {
                if let Some(r) = l.strip_prefix("[ ]") {
                    format!("- [ ]{r}")
                } else if let Some(r) = l.strip_prefix("[x]") {
                    format!("- [x]{r}")
                } else {
                    l.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// ---------------------------------------------------------------- notifications

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NotifRecord {
    pub id: u64,
    pub app: String,
    pub title: String,
    pub body: String,
    pub time: i64,
    pub suppressed: bool,
}

#[derive(Default)]
pub struct NotifHistory {
    pub items: Vec<NotifRecord>,
    pub next_id: u64,
    /// Notifications swallowed during quiet hours/focus/fullscreen (for the digest).
    pub suppressed: Vec<(String, String)>,
    pub access: Option<crate::services::notifications::Access>,
}

impl NotifHistory {
    pub fn record(&mut self, app: &str, title: &str, body: &str, suppressed: bool) {
        self.record_at(app, title, body, unix_secs(), suppressed);
    }

    /// Records a notification that arrived at `time` (kept newest first, no duplicates).
    pub fn record_at(&mut self, app: &str, title: &str, body: &str, time: i64, suppressed: bool) {
        if self.items.iter().any(|n| {
            n.app == app && n.title == title && n.body == body && (n.time - time).abs() < 120
        }) {
            return;
        }
        self.next_id += 1;
        let rec = NotifRecord {
            id: self.next_id,
            app: app.into(),
            title: title.into(),
            body: body.into(),
            time,
            suppressed,
        };
        let at = self.items.partition_point(|n| n.time > time);
        self.items.insert(at, rec);
        self.items.truncate(50);
    }
    pub fn suppressed(&mut self, p: &Popup) {
        self.suppressed.push((p.source.clone(), p.title.clone()));
        self.record(&p.source, &p.title, &p.subtitle, true);
    }
    /// "7 notifications from 3 apps" (and resets the counter).
    pub fn take_digest(&mut self) -> Option<(usize, usize)> {
        if self.suppressed.is_empty() {
            return None;
        }
        let n = self.suppressed.len();
        let apps: HashSet<&String> = self.suppressed.iter().map(|s| &s.0).collect();
        let a = apps.len().max(1);
        self.suppressed.clear();
        Some((n, a))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clip_dedupe_and_trim() {
        let mut c = ClipHistory::default();
        c.add_text("a", 2);
        c.add_text("b", 2);
        c.add_text("a", 2);
        assert_eq!(c.items[0].text, "a");
        assert_eq!(c.items.len(), 2);
        c.toggle_pin(c.items[1].id);
        c.add_text("c", 2);
        c.add_text("d", 2);
        assert!(c.items.iter().any(|i| i.pinned && i.text == "b"));
        assert_eq!(c.items.iter().filter(|i| !i.pinned).count(), 2);
    }
    #[test]
    fn colour_detect() {
        assert!(is_color("#19B8FF"));
        assert!(!is_color("19B8FF"));
        let mut c = ClipHistory::default();
        c.add_text("#fff", 10);
        assert_eq!(c.items[0].kind, ClipKind::Color);
        c.add_text("https://x.y/z", 10);
        assert_eq!(c.items[0].kind, ClipKind::Link);
    }
    #[test]
    fn shelf_text_and_links() {
        let mut s = Shelf::default();
        s.add_text("https://example.com");
        assert!(matches!(s.items[0].kind, ShelfKind::Link(_)));
        s.add_text("hello there");
        assert!(matches!(s.items[0].kind, ShelfKind::Text(_)));
    }
    #[test]
    fn digest_counts_apps() {
        let mut n = NotifHistory::default();
        let mut p = Popup::new(
            crate::app::activity::PopKind::Notification,
            crate::render::icons::Icon::Bell,
            "t",
            "b",
        );
        p.source = "A".into();
        n.suppressed(&p);
        n.suppressed(&p);
        p.source = "B".into();
        n.suppressed(&p);
        assert_eq!(n.take_digest(), Some((3, 2)));
        assert_eq!(n.take_digest(), None);
    }
    #[test]
    fn note_markdown() {
        let n = Note {
            id: 1,
            text: "Title\n[ ] a\n[x] b".into(),
            pinned: false,
            updated_at: 0,
        };
        assert_eq!(Notes::to_markdown(&n), "Title\n- [ ] a\n- [x] b");
        assert_eq!(Notes::first_line(&n), "Title");
    }
}
