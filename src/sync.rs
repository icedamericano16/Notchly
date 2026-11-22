//! Optional settings sync: mirrors settings, themes, to-dos and notes into a
//! folder (e.g. OneDrive). Files are last-writer-wins; to-dos and notes merge
//! per item by id and `updated_at`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::features::store::Notes;
use crate::features::todo::TodoList;
use crate::util::data_dir;

fn mtime(p: &Path) -> Option<SystemTime> {
    fs::metadata(p).and_then(|m| m.modified()).ok()
}

fn newer_copy(src: &Path, dst: &Path) {
    if src.exists() && mtime(src) > mtime(dst) {
        if let Some(parent) = dst.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::copy(src, dst);
    }
}

pub fn merge_todo(a: &mut TodoList, b: &TodoList) {
    for t in &b.tasks {
        match a
            .tasks
            .iter_mut()
            .find(|x| x.id == t.id && x.created == t.created)
        {
            Some(x) if t.updated_at > x.updated_at => *x = t.clone(),
            Some(_) => {}
            None => a.tasks.push(t.clone()),
        }
    }
    a.next_id = a.next_id.max(b.next_id);
}

pub fn merge_notes(a: &mut Notes, b: &Notes) {
    for n in &b.notes {
        match a.notes.iter_mut().find(|x| x.id == n.id) {
            Some(x) if n.updated_at > x.updated_at => *x = n.clone(),
            Some(_) => {}
            None => a.notes.push(n.clone()),
        }
    }
    a.next_id = a.next_id.max(b.next_id);
}

/// Pulls newer data from the sync folder at start-up.
pub fn pull(folder: &str) {
    let dir = PathBuf::from(folder);
    if folder.is_empty() || !dir.is_dir() {
        return;
    }
    let local = data_dir();
    newer_copy(&dir.join("settings.toml"), &local.join("settings.toml"));
    if let Ok(rd) = fs::read_dir(dir.join("themes")) {
        for e in rd.flatten() {
            newer_copy(&e.path(), &local.join("themes").join(e.file_name()));
        }
    }
    if let Ok(remote) = fs::read(dir.join("todo.json"))
        .map(|b| serde_json::from_slice::<TodoList>(&b).unwrap_or_default())
    {
        let mut l: TodoList = crate::util::load_json("todo.json");
        merge_todo(&mut l, &remote);
        crate::util::save_json("todo.json", &l);
    }
    if let Ok(remote) = fs::read(dir.join("notes.json"))
        .map(|b| serde_json::from_slice::<Notes>(&b).unwrap_or_default())
    {
        let mut l: Notes = crate::util::load_json("notes.json");
        merge_notes(&mut l, &remote);
        crate::util::save_json("notes.json", &l);
    }
}

/// Pushes local data to the sync folder.
pub fn push(folder: &str) {
    let dir = PathBuf::from(folder);
    if folder.is_empty() || fs::create_dir_all(&dir).is_err() {
        return;
    }
    let local = data_dir();
    for f in ["settings.toml", "todo.json", "notes.json"] {
        newer_copy(&local.join(f), &dir.join(f));
    }
    if let Ok(rd) = fs::read_dir(local.join("themes")) {
        for e in rd.flatten() {
            newer_copy(&e.path(), &dir.join("themes").join(e.file_name()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::store::Note;
    #[test]
    fn notes_lww() {
        let mut a = Notes {
            notes: vec![Note {
                id: 1,
                text: "old".into(),
                pinned: false,
                updated_at: 1,
            }],
            next_id: 1,
            current: None,
        };
        let b = Notes {
            notes: vec![
                Note {
                    id: 1,
                    text: "new".into(),
                    pinned: false,
                    updated_at: 2,
                },
                Note {
                    id: 2,
                    text: "x".into(),
                    pinned: false,
                    updated_at: 1,
                },
            ],
            next_id: 2,
            current: None,
        };
        merge_notes(&mut a, &b);
        assert_eq!(a.notes[0].text, "new");
        assert_eq!(a.notes.len(), 2);
    }
}
