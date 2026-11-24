//! Download tracker: ReadDirectoryChangesW on Downloads (and user folders) finds browser temp
//! files and their final rename; a small poller reads the temp files' sizes for progress,
//! because Windows doesn't report writes to a file while its writer keeps it open.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

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

/// How often the sizes of followed temp files are read.
const POLL: Duration = Duration::from_millis(700);
/// A download that hasn't grown for this long stops refreshing its pill (paused or stuck).
const STALL: Duration = Duration::from_secs(20);
/// A temp file that hasn't grown for this long is no longer followed; the next change
/// notification for it (at the latest when its writer closes it) picks it up again.
const FORGET: Duration = Duration::from_secs(120);
/// Below this size a temp file must have existed for a couple of seconds before it counts as
/// a download (editors and installers create short-lived small .tmp files).
const MIN_BYTES: u64 = 256 * 1024;

/// One followed temp file.
#[derive(Debug)]
struct Track {
    /// Stable across temp → temp renames ("Unconfirmed 1.crdownload" → "a.zip.crdownload").
    id: u64,
    started: Instant,
    last: Instant,
    last_bytes: u64,
    speed: f64,
    grew_at: Option<Instant>,
}

impl Track {
    fn new(id: u64, bytes: u64, now: Instant) -> Self {
        Self {
            id,
            started: now,
            last: now,
            last_bytes: bytes,
            speed: 0.0,
            grew_at: None,
        }
    }

    /// Records the file's size now. True when the in-progress pill should be refreshed.
    fn observe(&mut self, bytes: u64, now: Instant) -> bool {
        let dt = now.duration_since(self.last).as_secs_f64();
        if dt >= 0.25 {
            let inst = bytes.saturating_sub(self.last_bytes) as f64 / dt;
            self.speed = if self.speed == 0.0 {
                inst
            } else {
                self.speed * 0.7 + inst * 0.3
            };
            if bytes > self.last_bytes {
                self.grew_at = Some(now);
            }
            self.last = now;
            self.last_bytes = bytes;
        }
        let Some(grew) = self.grew_at else {
            return false;
        };
        now.duration_since(grew) < STALL
            && (bytes >= MIN_BYTES || now.duration_since(self.started) > Duration::from_secs(2))
    }

    fn forgotten(&self, now: Instant) -> bool {
        now.duration_since(self.grew_at.unwrap_or(self.started)) > FORGET
    }
}

type Tracks = Arc<Mutex<HashMap<PathBuf, Track>>>;

fn lock(t: &Tracks) -> MutexGuard<'_, HashMap<PathBuf, Track>> {
    t.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn start(bus: Bus, extra: Vec<String>) {
    let mut folders: Vec<PathBuf> = downloads_folder().into_iter().collect();
    folders.extend(extra.into_iter().map(PathBuf::from));
    for f in folders {
        let b = bus.clone();
        spawn("downloads", move || watch(b, f));
    }
}

/// Applies one directory change to the followed files. Returns the event to send, if any,
/// and whether a new file started being followed (so the poller must wake up).
fn on_change(
    tracks: &mut HashMap<PathBuf, Track>,
    action: FILE_ACTION,
    path: &Path,
    renamed_from: &mut Option<PathBuf>,
    now: Instant,
) -> (Option<DownloadEvent>, bool) {
    let size = |p: &Path| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
    match action {
        FILE_ACTION_ADDED | FILE_ACTION_MODIFIED if is_temp(path) => {
            if tracks.contains_key(path) {
                return (None, false);
            }
            let id = fnv1a(&path.to_string_lossy());
            tracks.insert(path.to_path_buf(), Track::new(id, size(path), now));
            (None, true)
        }
        FILE_ACTION_RENAMED_OLD_NAME => {
            *renamed_from = Some(path.to_path_buf());
            (None, false)
        }
        FILE_ACTION_RENAMED_NEW_NAME => {
            let Some(t) = renamed_from.take().and_then(|old| tracks.remove(&old)) else {
                return (None, false);
            };
            if is_temp(path) {
                // the browser picked the real name; same download
                tracks.insert(path.to_path_buf(), t);
                return (None, false);
            }
            let bytes = size(path);
            let name = path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let done = Download {
                id: t.id,
                name,
                bytes,
                speed: 0.0,
                total: Some(bytes),
                done: true,
                path: Some(path.to_string_lossy().to_string()),
            };
            (Some(DownloadEvent::Done(done)), false)
        }
        FILE_ACTION_REMOVED => match tracks.remove(path) {
            Some(t) => (Some(DownloadEvent::Cancelled(t.id)), false),
            None => (None, false),
        },
        _ => (None, false),
    }
}

/// Reads the sizes of followed temp files and sends progress. Change notifications alone
/// can't do this: while a browser keeps its download file open, NTFS reports no size or
/// last-write changes until the handle is closed, which happens just before the final rename.
fn poll(bus: Bus, tracks: Tracks, wake: Receiver<()>) {
    loop {
        let idle = lock(&tracks).is_empty();
        let woke = if idle {
            wake.recv().is_ok()
        } else {
            !matches!(wake.recv_timeout(POLL), Err(RecvTimeoutError::Disconnected))
        };
        if !woke {
            return;
        }
        let now = Instant::now();
        let mut out = Vec::new();
        lock(&tracks).retain(|path, tr| {
            let Ok(m) = std::fs::metadata(path) else {
                // gone or being renamed; the watcher sends Done/Cancelled
                return true;
            };
            if tr.observe(m.len(), now) {
                out.push(Download {
                    id: tr.id,
                    name: display_name(path),
                    bytes: m.len(),
                    speed: tr.speed,
                    total: None,
                    done: false,
                    path: None,
                });
            }
            !tr.forgotten(now)
        });
        for d in out {
            bus.send(Event::Download(DownloadEvent::Progress(d)));
        }
    }
}

fn watch(bus: Bus, dir: PathBuf) {
    let w = wide(&dir.to_string_lossy());
    // SAFETY: `w` is a NUL-terminated UTF-16 path that outlives the call; the handle is
    // closed below when the loop ends.
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
    let tracks: Tracks = Arc::default();
    {
        // downloads already running when Notchly starts; followed once they grow
        let now = Instant::now();
        let mut t = lock(&tracks);
        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let p = e.path();
            if is_temp(&p) {
                let bytes = e.metadata().map(|m| m.len()).unwrap_or(0);
                let id = fnv1a(&p.to_string_lossy());
                t.insert(p, Track::new(id, bytes, now));
            }
        }
    }
    let (kick, wake) = channel();
    {
        let (b, t) = (bus.clone(), tracks.clone());
        spawn("downloads-poll", move || poll(b, t, wake));
    }
    let _ = kick.send(());
    let mut buf = vec![0u8; 64 * 1024];
    let mut renamed_from: Option<PathBuf> = None;
    loop {
        let mut n = 0u32;
        // SAFETY: `h` is a valid directory handle opened with FILE_LIST_DIRECTORY, and `buf`
        // is a live, writable buffer of the length passed. The call is synchronous.
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
        while n > 0 && off + std::mem::size_of::<FILE_NOTIFY_INFORMATION>() <= n as usize {
            // SAFETY: the system wrote `n` bytes of DWORD-aligned FILE_NOTIFY_INFORMATION
            // records into `buf`; `off` follows their NextEntryOffset chain and is checked
            // against `n` above. FileName holds FileNameLength bytes of UTF-16.
            let (action, next, name) = unsafe {
                let info = &*(buf.as_ptr().add(off) as *const FILE_NOTIFY_INFORMATION);
                let len = info.FileNameLength as usize / 2;
                let name = String::from_utf16_lossy(std::slice::from_raw_parts(
                    info.FileName.as_ptr(),
                    len,
                ));
                (info.Action, info.NextEntryOffset, name)
            };
            let path = dir.join(&name);
            let (event, new) = on_change(
                &mut lock(&tracks),
                action,
                &path,
                &mut renamed_from,
                Instant::now(),
            );
            if new {
                let _ = kick.send(());
            }
            if let Some(e) = event {
                bus.send(Event::Download(e));
            }
            if next == 0 {
                break;
            }
            off += next as usize;
        }
    }
    // SAFETY: `h` came from CreateFileW above and is closed exactly once.
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

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    /// The old bug: progress came only from change notifications, which Windows doesn't send
    /// while the browser holds the file open, so no Progress was ever sent. Polled sizes now
    /// drive it: a growing file shows progress after one growth step.
    #[test]
    fn growing_file_reports_progress_from_polled_sizes() {
        let t0 = Instant::now();
        let mut t = Track::new(1, 0, t0);
        assert!(!t.observe(0, t0 + ms(700)), "nothing written yet");
        assert!(t.observe(400 * 1024, t0 + ms(1400)), "grew past 256 KB");
        assert!(t.speed > 0.0);
        assert!(t.observe(900 * 1024, t0 + ms(2100)));
    }

    /// What the poller relies on: the size is readable while the writer still has the file
    /// open (the directory entry and change notifications lag behind until it is closed).
    #[test]
    fn size_is_visible_while_the_writer_holds_the_file() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("notchly-dl-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("x.bin.crdownload");
        let mut f = std::fs::File::create(&p).unwrap();
        let chunk = vec![0u8; 128 * 1024];
        for i in 1..=4u64 {
            f.write_all(&chunk).unwrap();
            f.flush().unwrap();
            assert_eq!(std::fs::metadata(&p).unwrap().len(), i * 128 * 1024);
        }
        drop(f);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn small_temp_file_waits_before_counting() {
        let t0 = Instant::now();
        let mut t = Track::new(1, 0, t0);
        assert!(!t.observe(10 * 1024, t0 + ms(700)), "small and new");
        assert!(
            t.observe(20 * 1024, t0 + ms(2800)),
            "still growing after 2 s"
        );
    }

    #[test]
    fn untouched_temp_file_never_shows() {
        let t0 = Instant::now();
        let mut t = Track::new(1, 5 << 20, t0);
        for i in 1..10 {
            assert!(!t.observe(5 << 20, t0 + ms(700 * i)));
        }
        assert!(t.forgotten(t0 + FORGET + ms(1)));
    }

    #[test]
    fn stalled_download_stops_refreshing() {
        let t0 = Instant::now();
        let mut t = Track::new(1, 0, t0);
        assert!(t.observe(1 << 20, t0 + ms(700)));
        assert!(t.observe(1 << 20, t0 + ms(1400)), "just paused");
        assert!(!t.observe(1 << 20, t0 + STALL + ms(800)), "stalled");
        assert!(t.observe(2 << 20, t0 + STALL + ms(1500)), "resumed");
    }

    #[test]
    fn rename_chain_keeps_the_id_and_finishes() {
        let dir = Path::new(r"C:\nonexistent-notchly-test");
        let t0 = Instant::now();
        let mut tracks = HashMap::new();
        let mut from = None;
        let unconfirmed = dir.join("Unconfirmed 1234.crdownload");
        let named = dir.join("movie.mp4.crdownload");
        let done = dir.join("movie.mp4");

        let (e, new) = on_change(&mut tracks, FILE_ACTION_ADDED, &unconfirmed, &mut from, t0);
        assert_eq!((e, new), (None, true));
        let id = tracks[&unconfirmed].id;
        // further MODIFIED notifications don't restart the track
        let (_, new) = on_change(
            &mut tracks,
            FILE_ACTION_MODIFIED,
            &unconfirmed,
            &mut from,
            t0,
        );
        assert!(!new);

        on_change(
            &mut tracks,
            FILE_ACTION_RENAMED_OLD_NAME,
            &unconfirmed,
            &mut from,
            t0,
        );
        let (e, _) = on_change(
            &mut tracks,
            FILE_ACTION_RENAMED_NEW_NAME,
            &named,
            &mut from,
            t0,
        );
        assert_eq!(e, None);
        assert_eq!(
            tracks[&named].id, id,
            "same download after the browser names it"
        );

        on_change(
            &mut tracks,
            FILE_ACTION_RENAMED_OLD_NAME,
            &named,
            &mut from,
            t0,
        );
        let (e, _) = on_change(
            &mut tracks,
            FILE_ACTION_RENAMED_NEW_NAME,
            &done,
            &mut from,
            t0,
        );
        match e {
            Some(DownloadEvent::Done(d)) => {
                assert_eq!(d.id, id, "Done removes the same pill Progress created");
                assert_eq!(d.name, "movie.mp4");
                assert!(d.done);
            }
            other => panic!("expected Done, got {other:?}"),
        }
        assert!(tracks.is_empty());
    }

    #[test]
    fn cancelled_and_unrelated_files() {
        let dir = Path::new(r"C:\nonexistent-notchly-test");
        let t0 = Instant::now();
        let mut tracks = HashMap::new();
        let mut from = None;
        let part = dir.join("a.zip.part");
        on_change(&mut tracks, FILE_ACTION_ADDED, &part, &mut from, t0);
        let id = tracks[&part].id;
        let (e, _) = on_change(&mut tracks, FILE_ACTION_REMOVED, &part, &mut from, t0);
        assert_eq!(e, Some(DownloadEvent::Cancelled(id)));
        // ordinary files are not followed
        let (e, new) = on_change(
            &mut tracks,
            FILE_ACTION_ADDED,
            &dir.join("a.zip"),
            &mut from,
            t0,
        );
        assert_eq!((e, new), (None, false));
        assert!(tracks.is_empty());
    }
}
