//! Download tracker: ReadDirectoryChangesW on Downloads (and user folders),
//! following growing temp files to estimate speed and completion.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Storage::FileSystem::*;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{FOLDERID_Downloads, SHGetKnownFolderPath, KF_FLAG_DEFAULT};

use super::{spawn, Bus, Event};
use crate::util::{fnv1a, wide};

#[derive(Debug, Clone, PartialEq)]
pub struct Download {
    pub id: u64,
    pub name: String,
    pub bytes: u64,
    pub speed: f64,
    pub total: Option<u64>,
    pub done: bool,
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DownloadEvent {
    Progress(Download),
    Done(Download),
    Cancelled(u64),
}

pub const TEMP_EXT: [&str; 6] = [
    "crdownload",
    "part",
    "partial",
    "download",
    "tmp",
    "opdownload",
];

pub fn is_temp(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| TEMP_EXT.iter().any(|t| t.eq_ignore_ascii_case(e)))
        .unwrap_or(false)
}

/// "Unconfirmed 1234.crdownload" / "file.zip.part" → display name.
pub fn display_name(p: &Path) -> String {
    let stem = p
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    if stem.starts_with("Unconfirmed ") || stem.starts_with(".com.google") {
        "Downloading…".into()
    } else {
        stem
    }
}

pub fn downloads_folder() -> Option<PathBuf> {
    unsafe {
        let p = SHGetKnownFolderPath(&FOLDERID_Downloads, KF_FLAG_DEFAULT, None).ok()?;
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as _));
        s.map(PathBuf::from)
    }
}

struct Track {
    started: Instant,
    last: Instant,
    last_bytes: u64,
    speed: f64,
    last_sent: Instant,
}

pub fn start(bus: Bus, extra: Vec<String>) {
    let mut folders: Vec<PathBuf> = downloads_folder().into_iter().collect();
    folders.extend(extra.into_iter().map(PathBuf::from));
    for f in folders {
        let b = bus.clone();
        spawn("downloads", move || watch(b, f));
    }
}

fn watch(bus: Bus, dir: PathBuf) {
    let w = wide(&dir.to_string_lossy());
    let h = match unsafe {
        CreateFileW(
            PCWSTR(w.as_ptr()),
            FILE_LIST_DIRECTORY.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            None,
        )
    } {
        Ok(h) => h,
        Err(_) => return,
    };
    let mut tracks: HashMap<PathBuf, Track> = HashMap::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut renamed_from: Option<PathBuf> = None;
    loop {
        let mut n = 0u32;
        let ok = unsafe {
            ReadDirectoryChangesW(
                h,
                buf.as_mut_ptr() as _,
                buf.len() as u32,
                false,
                FILE_NOTIFY_CHANGE_FILE_NAME
                    | FILE_NOTIFY_CHANGE_SIZE
                    | FILE_NOTIFY_CHANGE_LAST_WRITE,
                Some(&mut n),
                None,
                None,
            )
        };
        if ok.is_err() {
            break;
        }
        let mut off = 0usize;
        while n > 0 && off < n as usize {
            let info = unsafe { &*(buf.as_ptr().add(off) as *const FILE_NOTIFY_INFORMATION) };
            let len = info.FileNameLength as usize / 2;
            let name = unsafe {
                String::from_utf16_lossy(std::slice::from_raw_parts(info.FileName.as_ptr(), len))
            };
            let path = dir.join(&name);
            let id = fnv1a(&path.to_string_lossy());
            match info.Action {
                FILE_ACTION_ADDED | FILE_ACTION_MODIFIED if is_temp(&path) => {
                    let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                    let now = Instant::now();
                    let t = tracks.entry(path.clone()).or_insert(Track {
                        started: now,
                        last: now,
                        last_bytes: bytes,
                        speed: 0.0,
                        last_sent: now - std::time::Duration::from_secs(1),
                    });
                    let dt = (now - t.last).as_secs_f64();
                    if dt > 0.25 {
                        let inst = (bytes.saturating_sub(t.last_bytes)) as f64 / dt;
                        t.speed = if t.speed == 0.0 {
                            inst
                        } else {
                            t.speed * 0.7 + inst * 0.3
                        };
                        t.last = now;
                        t.last_bytes = bytes;
                    }
                    // ignore tiny temp files that never grow (editors, installers)
                    if (now - t.last_sent).as_millis() >= 500
                        && (bytes > 256 * 1024 || (now - t.started).as_secs() > 2)
                    {
                        t.last_sent = now;
                        bus.send(Event::Download(DownloadEvent::Progress(Download {
                            id,
                            name: display_name(&path),
                            bytes,
                            speed: t.speed,
                            total: None,
                            done: false,
                            path: None,
                        })));
                    }
                }
                FILE_ACTION_RENAMED_OLD_NAME => renamed_from = Some(path.clone()),
                FILE_ACTION_RENAMED_NEW_NAME => {
                    if let Some(old) = renamed_from.take() {
                        if tracks.remove(&old).is_some() && !is_temp(&path) {
                            let oid = fnv1a(&old.to_string_lossy());
                            let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                            let fname = path
                                .file_name()
                                .map(|s| s.to_string_lossy().to_string())
                                .unwrap_or_default();
                            bus.send(Event::Download(DownloadEvent::Done(Download {
                                id: oid,
                                name: fname,
                                bytes,
                                speed: 0.0,
                                total: Some(bytes),
                                done: true,
                                path: Some(path.to_string_lossy().to_string()),
                            })));
                        }
                    }
                }
                FILE_ACTION_REMOVED if tracks.remove(&path).is_some() => {
                    bus.send(Event::Download(DownloadEvent::Cancelled(id)));
                }
                _ => {}
            }
            if info.NextEntryOffset == 0 {
                break;
            }
            off += info.NextEntryOffset as usize;
        }
    }
    unsafe {
        let _ = CloseHandle(HANDLE(h.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn temp_detection() {
        assert!(is_temp(Path::new("a.zip.crdownload")));
        assert!(is_temp(Path::new("video.mp4.part")));
        assert!(!is_temp(Path::new("a.zip")));
        assert_eq!(display_name(Path::new("video.mp4.part")), "video.mp4");
    }
}
