//! Synced lyrics from LRCLIB with a local LRU cache (hits and misses).

use std::sync::mpsc::{channel, Sender};

use serde::{Deserialize, Serialize};

use super::{http, spawn, Bus, Event};
use crate::util::{fnv1a, log, prune_dir, sub_dir};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LyricsResult {
    Synced(Vec<(i64, String)>),
    Plain(Vec<String>),
    Instrumental,
    NotFound,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct LyricsRequest {
    pub key: u64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_s: i64,
}

pub fn request_key(title: &str, artist: &str) -> u64 {
    fnv1a(&format!(
        "{}|{}",
        title.to_lowercase().trim(),
        artist.to_lowercase().trim()
    ))
}

const CACHE_CAP: u64 = 50 * 1024 * 1024;

/// Starts the lyrics worker. `allow_net` false = cache only.
pub fn start(bus: Bus) -> Sender<(LyricsRequest, bool)> {
    let (tx, rx) = channel::<(LyricsRequest, bool)>();
    spawn("lyrics", move || {
        super::com_mta();
        let dir = sub_dir("cache/lyrics");
        while let Ok((mut req, allow_net)) = rx.recv() {
            // skip to the newest request if several queued up (fast track skipping)
            while let Ok(newer) = rx.try_recv() {
                req = newer.0;
            }
            let path = dir.join(format!("{:016x}.json", req.key));
            if let Ok(bytes) = std::fs::read(&path) {
                if let Ok(r) = serde_json::from_slice::<LyricsResult>(&bytes) {
                    // touch for LRU
                    let _ = std::fs::write(&path, &bytes);
                    bus.send(Event::Lyrics {
                        key: req.key,
                        result: r,
                    });
                    continue;
                }
            }
            if !allow_net {
                bus.send(Event::Lyrics {
                    key: req.key,
                    result: LyricsResult::NotFound,
                });
                continue;
            }
            let result = fetch(&req);
            if !matches!(result, LyricsResult::Error(_)) {
                if let Ok(bytes) = serde_json::to_vec(&result) {
                    let _ = std::fs::write(&path, bytes);
                }
                prune_dir(&dir, CACHE_CAP);
            }
            bus.send(Event::Lyrics {
                key: req.key,
                result,
            });
        }
    });
    tx
}

#[derive(Deserialize)]
struct LrcLibItem {
    #[serde(default, rename = "syncedLyrics")]
    synced: Option<String>,
    #[serde(default, rename = "plainLyrics")]
    plain: Option<String>,
    #[serde(default)]
    instrumental: bool,
    #[serde(default)]
    duration: Option<f64>,
}

fn fetch(r: &LyricsRequest) -> LyricsResult {
    let mut url = format!(
        "https://lrclib.net/api/get?track_name={}&artist_name={}",
        http::enc(&r.title),
        http::enc(&r.artist)
    );
    if !r.album.is_empty() {
        url.push_str(&format!("&album_name={}", http::enc(&r.album)));
    }
    if r.duration_s > 0 {
        url.push_str(&format!("&duration={}", r.duration_s));
    }
    match http::get(&url, &[]) {
        Ok(resp) if resp.ok() => {
            if let Ok(item) = serde_json::from_str::<LrcLibItem>(&resp.body) {
                return to_result(item);
            }
        }
        Ok(resp) if resp.status == 404 => {}
        Ok(resp) => return LyricsResult::Error(format!("HTTP {}", resp.status)),
        Err(e) => {
            log(&format!("lyrics fetch failed: {e}"));
            return LyricsResult::Error(e);
        }
    }
    // fallback: search, prefer the closest duration with synced lyrics
    let url = format!(
        "https://lrclib.net/api/search?track_name={}&artist_name={}",
        http::enc(&r.title),
        http::enc(&r.artist)
    );
    match http::get(&url, &[]) {
        Ok(resp) if resp.ok() => {
            let items: Vec<LrcLibItem> = serde_json::from_str(&resp.body).unwrap_or_default();
            let best = items
                .into_iter()
                .filter(|i| i.synced.is_some() || i.plain.is_some() || i.instrumental)
                .min_by_key(|i| {
                    let d = i
                        .duration
                        .map(|d| (d - r.duration_s as f64).abs() as i64)
                        .unwrap_or(999);
                    (i.synced.is_none() as i64) * 10_000 + d
                });
            match best {
                Some(i) => to_result(i),
                None => LyricsResult::NotFound,
            }
        }
        Ok(_) => LyricsResult::NotFound,
        Err(e) => LyricsResult::Error(e),
    }
}

fn to_result(i: LrcLibItem) -> LyricsResult {
    if i.instrumental {
        return LyricsResult::Instrumental;
    }
    if let Some(s) = i.synced.filter(|s| !s.trim().is_empty()) {
        let lines = parse_lrc(&s);
        if !lines.is_empty() {
            return LyricsResult::Synced(lines);
        }
    }
    if let Some(p) = i.plain.filter(|s| !s.trim().is_empty()) {
        return LyricsResult::Plain(p.lines().map(|l| l.trim().to_string()).collect());
    }
    LyricsResult::NotFound
}

/// Parses LRC text into (ms, line) pairs, sorted. Supports multiple stamps per line
/// and `[offset:±ms]`.
pub fn parse_lrc(s: &str) -> Vec<(i64, String)> {
    let mut out = Vec::new();
    let mut offset = 0i64;
    for raw in s.lines() {
        let mut rest = raw.trim();
        let mut stamps = Vec::new();
        while rest.starts_with('[') {
            let Some(end) = rest.find(']') else { break };
            let tag = &rest[1..end];
            rest = &rest[end + 1..];
            if let Some(v) = tag.strip_prefix("offset:") {
                offset = v.trim().parse().unwrap_or(0);
                continue;
            }
            if let Some(ms) = parse_stamp(tag) {
                stamps.push(ms);
            }
        }
        let text = rest.trim().to_string();
        for ms in stamps {
            out.push(((ms - offset).max(0), text.clone()));
        }
    }
    out.sort_by_key(|l| l.0);
    out
}

fn parse_stamp(tag: &str) -> Option<i64> {
    let (m, s) = tag.split_once(':')?;
    let m: i64 = m.trim().parse().ok()?;
    let (sec, frac) = match s.split_once(['.', ':']) {
        Some((a, b)) => (a, b),
        None => (s, "0"),
    };
    let sec: i64 = sec.trim().parse().ok()?;
    let frac_ms = match frac.len() {
        1 => frac.parse::<i64>().ok()? * 100,
        2 => frac.parse::<i64>().ok()? * 10,
        _ => frac.get(..3)?.parse::<i64>().ok()?,
    };
    Some(m * 60_000 + sec * 1000 + frac_ms)
}

/// Index of the current line for `pos_ms` (lines appear `lead_ms` early).
pub fn current_line(lines: &[(i64, String)], pos_ms: i64, lead_ms: i64) -> Option<usize> {
    let p = pos_ms + lead_ms;
    match lines.partition_point(|l| l.0 <= p) {
        0 => None,
        n => Some(n - 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_lrc() {
        let l = parse_lrc(
            "[ar:x]\n[00:01.50]Hello\n[00:03.20][00:10.00]Again\n[01:02.123]Long\n[00:05.00]",
        );
        assert_eq!(l[0], (1500, "Hello".into()));
        assert_eq!(l[1], (3200, "Again".into()));
        assert_eq!(l[2], (5000, "".into()));
        assert_eq!(l[3], (10000, "Again".into()));
        assert_eq!(l[4], (62123, "Long".into()));
    }
    #[test]
    fn offset_applies() {
        let l = parse_lrc("[offset:500]\n[00:02.00]x");
        assert_eq!(l[0].0, 1500);
    }
    #[test]
    fn finds_current() {
        let l = parse_lrc("[00:01.00]a\n[00:02.00]b\n[00:03.00]c");
        assert_eq!(current_line(&l, 500, 0), None);
        assert_eq!(current_line(&l, 1800, 250), Some(1));
        assert_eq!(current_line(&l, 1800, 0), Some(0));
        assert_eq!(current_line(&l, 99_000, 0), Some(2));
    }
}
