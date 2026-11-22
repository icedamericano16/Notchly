//! Music: GlobalSystemMediaTransportControlsSessionManager on an MTA worker.

use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::Duration;

use windows::Foundation::TypedEventHandler;
use windows::Media::Control::*;
use windows::Media::MediaPlaybackAutoRepeatMode;
use windows::Storage::Streams::DataReader;

use super::{com_mta, spawn, Bus, Event};
use crate::render::image;
use crate::theme::adaptive;
use crate::util::{fnv1a, log};

#[derive(Debug, Clone, PartialEq)]
pub enum MediaCmd {
    PlayPause,
    Pause,
    Next,
    Prev,
    SeekMs(i64),
    Shuffle(bool),
    /// 0 none, 1 track, 2 list
    Repeat(u8),
    Select(String),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SessionInfo {
    pub app_id: String,
    pub name: String,
    pub playing: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MediaSnapshot {
    pub sessions: Vec<SessionInfo>,
    pub active: Option<usize>,
    pub app_id: String,
    pub app_name: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub playing: bool,
    pub position_ms: i64,
    pub duration_ms: i64,
    /// Unix ms when `position_ms` was valid.
    pub updated_ms: i64,
    pub shuffle: Option<bool>,
    pub repeat: Option<u8>,
    pub can_play_pause: bool,
    pub can_next: bool,
    pub can_prev: bool,
    pub can_seek: bool,
    pub can_shuffle: bool,
    pub can_repeat: bool,
    pub art_key: u64,
}

enum Msg {
    Cmd(MediaCmd),
    Refresh,
    Rebind,
}

/// Starts the media service; returns its command channel.
pub fn start(bus: Bus, initial_choice: String) -> Sender<MediaCmd> {
    let (ctx, crx) = channel::<MediaCmd>();
    let (tx, rx) = channel::<Msg>();
    let tx2 = tx.clone();
    spawn("media-cmd", move || {
        while let Ok(c) = crx.recv() {
            if tx2.send(Msg::Cmd(c)).is_err() {
                break;
            }
        }
    });
    spawn("media", move || {
        com_mta();
        let mgr = match GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
            .and_then(|op| op.join())
        {
            Ok(m) => m,
            Err(e) => {
                log(&format!("media manager unavailable: {e}"));
                return;
            }
        };
        let t = tx.clone();
        let _ = mgr.SessionsChanged(&TypedEventHandler::<
            GlobalSystemMediaTransportControlsSessionManager,
            SessionsChangedEventArgs,
        >::new(move |_, _| {
            let _ = t.send(Msg::Rebind);
            Ok(())
        }));
        let t = tx.clone();
        let _ = mgr.CurrentSessionChanged(&TypedEventHandler::<
            GlobalSystemMediaTransportControlsSessionManager,
            CurrentSessionChangedEventArgs,
        >::new(move |_, _| {
            let _ = t.send(Msg::Rebind);
            Ok(())
        }));
        let mut worker = Worker {
            mgr,
            tx: tx.clone(),
            bus,
            bound: Vec::new(),
            choice: initial_choice,
            last: MediaSnapshot::default(),
            art_key_sent: 0,
            art_missing_tries: 0,
        };
        worker.rebind();
        worker.publish();
        loop {
            match rx.recv() {
                Ok(first) => {
                    let mut rebind = matches!(first, Msg::Rebind);
                    let mut cmds = Vec::new();
                    if let Msg::Cmd(c) = first {
                        cmds.push(c);
                    }
                    // debounce bursts of property events
                    loop {
                        match rx.recv_timeout(Duration::from_millis(40)) {
                            Ok(Msg::Rebind) => rebind = true,
                            Ok(Msg::Cmd(c)) => cmds.push(c),
                            Ok(Msg::Refresh) => {}
                            Err(RecvTimeoutError::Timeout) => break,
                            Err(RecvTimeoutError::Disconnected) => return,
                        }
                    }
                    for c in cmds {
                        worker.command(c);
                        rebind = true;
                    }
                    if rebind {
                        worker.rebind();
                    }
                    worker.publish();
                }
                Err(_) => return,
            }
        }
    });
    ctx
}

struct Bound {
    session: GlobalSystemMediaTransportControlsSession,
    tokens: [i64; 3],
}

struct Worker {
    mgr: GlobalSystemMediaTransportControlsSessionManager,
    tx: Sender<Msg>,
    bus: Bus,
    bound: Vec<Bound>,
    choice: String,
    last: MediaSnapshot,
    art_key_sent: u64,
    art_missing_tries: u32,
}

impl Worker {
    fn rebind(&mut self) {
        for b in self.bound.drain(..) {
            let _ = b.session.RemoveMediaPropertiesChanged(b.tokens[0]);
            let _ = b.session.RemovePlaybackInfoChanged(b.tokens[1]);
            let _ = b.session.RemoveTimelinePropertiesChanged(b.tokens[2]);
        }
        let Ok(list) = self.mgr.GetSessions() else {
            return;
        };
        let n = list.Size().unwrap_or(0);
        for i in 0..n {
            let Ok(s) = list.GetAt(i) else { continue };
            macro_rules! mk {
                () => {{
                    let tx = self.tx.clone();
                    TypedEventHandler::new(move |_, _| {
                        let _ = tx.send(Msg::Refresh);
                        Ok(())
                    })
                }};
            }
            let a = s.MediaPropertiesChanged(&mk!()).unwrap_or(0);
            let b = s.PlaybackInfoChanged(&mk!()).unwrap_or(0);
            let c = s.TimelinePropertiesChanged(&mk!()).unwrap_or(0);
            self.bound.push(Bound {
                session: s,
                tokens: [a, b, c],
            });
        }
    }

    fn sessions(&self) -> Vec<(GlobalSystemMediaTransportControlsSession, SessionInfo)> {
        self.bound
            .iter()
            .map(|b| {
                let id = b
                    .session
                    .SourceAppUserModelId()
                    .map(|h| h.to_string())
                    .unwrap_or_default();
                let playing = b
                    .session
                    .GetPlaybackInfo()
                    .and_then(|p| p.PlaybackStatus())
                    .map(|s| s == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing)
                    .unwrap_or(false);
                (
                    b.session.clone(),
                    SessionInfo {
                        name: app_name(&id),
                        app_id: id,
                        playing,
                    },
                )
            })
            .collect()
    }

    /// Chosen session: the user's pick if present, else a playing one, else the OS current.
    fn active(&self) -> Option<(usize, GlobalSystemMediaTransportControlsSession)> {
        let all = self.sessions();
        if all.is_empty() {
            return None;
        }
        if !self.choice.is_empty() {
            if let Some(i) = all
                .iter()
                .position(|(_, s)| s.app_id == self.choice && s.playing)
            {
                return Some((i, all[i].0.clone()));
            }
        }
        if let Some(i) = all.iter().position(|(_, s)| s.playing) {
            return Some((i, all[i].0.clone()));
        }
        if !self.choice.is_empty() {
            if let Some(i) = all.iter().position(|(_, s)| s.app_id == self.choice) {
                return Some((i, all[i].0.clone()));
            }
        }
        if let Ok(cur) = self.mgr.GetCurrentSession() {
            let id = cur
                .SourceAppUserModelId()
                .map(|h| h.to_string())
                .unwrap_or_default();
            if let Some(i) = all.iter().position(|(_, s)| s.app_id == id) {
                return Some((i, all[i].0.clone()));
            }
        }
        Some((0, all[0].0.clone()))
    }

    fn command(&mut self, c: MediaCmd) {
        if let MediaCmd::Select(id) = &c {
            self.choice = id.clone();
            return;
        }
        let Some((_, s)) = self.active() else { return };
        let r = match c {
            MediaCmd::PlayPause => s.TryTogglePlayPauseAsync().and_then(|o| o.join()),
            MediaCmd::Pause => s.TryPauseAsync().and_then(|o| o.join()),
            MediaCmd::Next => s.TrySkipNextAsync().and_then(|o| o.join()),
            MediaCmd::Prev => s.TrySkipPreviousAsync().and_then(|o| o.join()),
            MediaCmd::SeekMs(ms) => s
                .TryChangePlaybackPositionAsync(ms.max(0) * 10_000)
                .and_then(|o| o.join()),
            MediaCmd::Shuffle(on) => s.TryChangeShuffleActiveAsync(on).and_then(|o| o.join()),
            MediaCmd::Repeat(m) => s
                .TryChangeAutoRepeatModeAsync(match m {
                    1 => MediaPlaybackAutoRepeatMode::Track,
                    2 => MediaPlaybackAutoRepeatMode::List,
                    _ => MediaPlaybackAutoRepeatMode::None,
                })
                .and_then(|o| o.join()),
            MediaCmd::Select(_) => Ok(true),
        };
        if let Err(e) = r {
            log(&format!("media command failed: {e}"));
        }
    }

    fn publish(&mut self) {
        let sessions = self.sessions();
        let mut snap = MediaSnapshot {
            sessions: sessions.iter().map(|s| s.1.clone()).collect(),
            ..Default::default()
        };
        let mut active_session = None;
        if let Some((idx, s)) = self.active() {
            snap.active = Some(idx);
            snap.app_id = snap.sessions[idx].app_id.clone();
            snap.app_name = snap.sessions[idx].name.clone();
            let props = s.TryGetMediaPropertiesAsync().and_then(|o| o.join()).ok();
            if let Some(p) = &props {
                let raw_title = p.Title().map(|h| h.to_string()).unwrap_or_default();
                let raw_artist = p.Artist().map(|h| h.to_string()).unwrap_or_default();
                let (t, a) = clean_title(&raw_title, &raw_artist);
                snap.title = t;
                snap.artist = a;
                snap.album = p.AlbumTitle().map(|h| h.to_string()).unwrap_or_default();
            }
            if let Ok(pi) = s.GetPlaybackInfo() {
                snap.playing = pi
                    .PlaybackStatus()
                    .map(|st| {
                        st == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing
                    })
                    .unwrap_or(false);
                snap.shuffle = pi.IsShuffleActive().and_then(|r| r.Value()).ok();
                snap.repeat = pi.AutoRepeatMode().and_then(|r| r.Value()).ok().map(|m| {
                    if m == MediaPlaybackAutoRepeatMode::Track {
                        1
                    } else if m == MediaPlaybackAutoRepeatMode::List {
                        2
                    } else {
                        0
                    }
                });
                if let Ok(c) = pi.Controls() {
                    snap.can_play_pause = c.IsPlayPauseToggleEnabled().unwrap_or(false)
                        || c.IsPlayEnabled().unwrap_or(false)
                        || c.IsPauseEnabled().unwrap_or(false);
                    snap.can_next = c.IsNextEnabled().unwrap_or(false);
                    snap.can_prev = c.IsPreviousEnabled().unwrap_or(false);
                    snap.can_seek = c.IsPlaybackPositionEnabled().unwrap_or(false);
                    snap.can_shuffle = c.IsShuffleEnabled().unwrap_or(false);
                    snap.can_repeat = c.IsRepeatEnabled().unwrap_or(false);
                }
            }
            if let Ok(tl) = s.GetTimelineProperties() {
                let start = tl.StartTime().map(|t| t.Duration).unwrap_or(0);
                let end = tl.EndTime().map(|t| t.Duration).unwrap_or(0);
                let pos = tl.Position().map(|t| t.Duration).unwrap_or(0);
                snap.duration_ms = ((end - start) / 10_000).max(0);
                snap.position_ms = ((pos - start) / 10_000).max(0);
                let upd = tl.LastUpdatedTime().map(|t| t.UniversalTime).unwrap_or(0);
                snap.updated_ms = if upd > 0 {
                    upd / 10_000 - 11_644_473_600_000
                } else {
                    crate::util::unix_ms()
                };
            }
            snap.art_key = fnv1a(&format!(
                "{}|{}|{}|{}",
                snap.app_id, snap.title, snap.artist, snap.album
            ));
            active_session = props.map(|p| (p, snap.art_key));
        }
        if snap != self.last {
            self.last = snap.clone();
            self.bus.send(Event::Media(snap));
        }
        if let Some((props, key)) = active_session {
            if key != self.art_key_sent {
                self.art_missing_tries = 0;
            }
            if key != self.art_key_sent || self.art_missing_tries > 0 && self.art_missing_tries < 4
            {
                let art = read_thumbnail(&props);
                if art.is_none() {
                    self.art_missing_tries += 1;
                } else {
                    self.art_missing_tries = 0;
                }
                let palette = art
                    .as_ref()
                    .map(|p| adaptive::palette(&p.data, 8))
                    .unwrap_or_default();
                self.bus.send(Event::MediaArt {
                    key,
                    pixels: art.map(Arc::new),
                    palette,
                });
                self.art_key_sent = key;
            }
        }
    }
}

fn read_thumbnail(
    p: &GlobalSystemMediaTransportControlsSessionMediaProperties,
) -> Option<image::Pixels> {
    let thumb = p.Thumbnail().ok()?;
    let stream = thumb.OpenReadAsync().ok()?.join().ok()?;
    let size = stream.Size().ok()? as u32;
    if size == 0 || size > 20 * 1024 * 1024 {
        return None;
    }
    let reader = DataReader::CreateDataReader(&stream).ok()?;
    let n = reader.LoadAsync(size).ok()?.join().ok()?;
    let mut buf = vec![0u8; n as usize];
    reader.ReadBytes(&mut buf).ok()?;
    image::decode(&buf, 256).ok()
}

/// Friendly player name from an AppUserModelID.
pub fn app_name(aumid: &str) -> String {
    let l = aumid.to_lowercase();
    let known = [
        ("spotify", "Spotify"),
        ("msedge", "Edge"),
        ("chrome", "Chrome"),
        ("firefox", "Firefox"),
        ("308046b0af4a39cb", "Firefox"),
        ("opera", "Opera"),
        ("brave", "Brave"),
        ("vivaldi", "Vivaldi"),
        ("zunemusic", "Media Player"),
        ("microsoft.media", "Media Player"),
        ("vlc", "VLC"),
        ("applemusic", "Apple Music"),
        ("itunes", "iTunes"),
        ("tidal", "TIDAL"),
        ("deezer", "Deezer"),
        ("youtube", "YouTube Music"),
        ("foobar", "foobar2000"),
        ("musicbee", "MusicBee"),
        ("discord", "Discord"),
    ];
    for (k, v) in known {
        if l.contains(k) {
            return v.to_string();
        }
    }
    let base = aumid.rsplit(['!', '\\']).next().unwrap_or(aumid);
    let base = base.trim_end_matches(".exe").trim_end_matches(".EXE");
    let base = base.split('_').next().unwrap_or(base);
    let base = base.rsplit('.').next().unwrap_or(base);
    if base.is_empty() {
        "Media".into()
    } else {
        base.to_string()
    }
}

/// Removes YouTube noise and splits "Artist - Title" when the artist is a channel.
pub fn clean_title(title: &str, artist: &str) -> (String, String) {
    let mut t = title.to_string();
    let noise = [
        "official music video",
        "official video",
        "official audio",
        "official lyric video",
        "official lyrics video",
        "lyric video",
        "lyrics video",
        "lyrics",
        "audio",
        "video oficial",
        "official visualizer",
        "visualizer",
        "hd",
        "hq",
        "4k",
        "mv",
        "m/v",
    ];
    // strip bracketed noise: (...) or [...]
    loop {
        let mut changed = false;
        for (open, close) in [('(', ')'), ('[', ']'), ('【', '】')] {
            if let (Some(a), Some(b)) = (t.rfind(open), t.rfind(close)) {
                if b > a {
                    let inner = t[a + open.len_utf8()..b].trim().to_lowercase();
                    if noise
                        .iter()
                        .any(|n| inner == *n || inner.starts_with(n) && inner.len() < n.len() + 6)
                    {
                        t.replace_range(a..b + close.len_utf8(), "");
                        t = t.trim().to_string();
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    let mut a = artist.trim().to_string();
    if let Some(stripped) = a.strip_suffix(" - Topic") {
        a = stripped.to_string();
    }
    let a_lower = a.to_lowercase();
    let channelish = a.is_empty()
        || a_lower.ends_with("vevo")
        || a_lower.contains("records")
        || a_lower.contains("music")
        || a_lower.contains("official")
        || a_lower.contains("tv");
    for sep in [" - ", " – ", " — ", " | "] {
        if let Some(pos) = t.find(sep) {
            let (left, right) = (
                t[..pos].trim().to_string(),
                t[pos + sep.len()..].trim().to_string(),
            );
            if !left.is_empty()
                && !right.is_empty()
                && (channelish || left.eq_ignore_ascii_case(&a))
            {
                return (right, left);
            }
        }
    }
    if a_lower.ends_with("vevo") {
        a = a[..a.len() - 4].trim().to_string();
    }
    (t.trim().to_string(), a)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cleans_youtube_noise() {
        assert_eq!(
            clean_title("Song Name (Official Video)", "Band"),
            ("Song Name".into(), "Band".into())
        );
        assert_eq!(
            clean_title("Song [Lyrics]", "Band - Topic"),
            ("Song".into(), "Band".into())
        );
        assert_eq!(
            clean_title("Artist X - Hit Song (Official Music Video)", "ArtistXVEVO"),
            ("Hit Song".into(), "Artist X".into())
        );
        assert_eq!(
            clean_title("Remix (feat. Y)", "Z"),
            ("Remix (feat. Y)".into(), "Z".into())
        );
        assert_eq!(
            clean_title("A - B", "Some Records"),
            ("B".into(), "A".into())
        );
    }
    #[test]
    fn names_apps() {
        assert_eq!(app_name("Spotify.exe"), "Spotify");
        assert_eq!(app_name("MSEdge"), "Edge");
        assert_eq!(
            app_name("Microsoft.ZuneMusic_8wekyb3d8bbwe!Microsoft.ZuneMusic"),
            "Media Player"
        );
        assert_eq!(app_name("foo.exe"), "foo");
    }
}
