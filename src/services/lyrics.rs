//! Lyrics engine: finds synced lyrics for whatever is playing, including YouTube videos whose
//! "title" is really "Song - Artist" and whose "artist" is a channel.
//!
//! 1. Builds search candidates from the raw and cleaned title/artist (both split orders,
//!    noise removed: "(Official Video)", "| Lyrics", "ft. …", "slowed + reverb", hashtags…).
//! 2. Asks LRCLIB: exact lookups for the best pairs plus free-text search, then scores every
//!    hit by title/artist token overlap and duration, preferring synced lyrics.
//! 3. Falls back to lyrics.ovh for plain lyrics.
//!
//! Results (and misses, for a few days) are cached; the user can step through other matches.

use std::collections::HashSet;
use std::sync::mpsc::{channel, Sender};

use serde::{Deserialize, Serialize};

use super::{http, spawn, Bus, Event};
use crate::util::{fnv1a, log, prune_dir, sub_dir, unix_secs};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LyricsResult {
    Synced(Vec<(i64, String)>),
    Plain(Vec<String>),
    Instrumental,
    NotFound,
    Error(String),
}

/// Where a result came from and what it matched.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct LyricsMeta {
    /// "LRCLIB" or "lyrics.ovh".
    pub source: String,
    /// "Track — Artist" of the match.
    pub matched: String,
    /// What was searched for (shown when nothing is found).
    pub query: String,
    /// Which alternative this is (0 = best) and how many good matches there were.
    pub alt: usize,
    pub alts: usize,
    /// Length of the matched recording (s), when known.
    #[serde(default)]
    pub duration_s: f64,
}

#[derive(Debug, Clone)]
pub struct LyricsRequest {
    pub key: u64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_s: i64,
    /// Title / artist exactly as the player reported them (before clean-up).
    pub raw_title: String,
    pub raw_artist: String,
    /// Alternative match to use (0 = best); >0 or `force` skips the cache.
    pub pick: usize,
    pub force: bool,
}

pub fn request_key(title: &str, artist: &str) -> u64 {
    fnv1a(&format!(
        "{}|{}",
        title.to_lowercase().trim(),
        artist.to_lowercase().trim()
    ))
}

const CACHE_CAP: u64 = 50 * 1024 * 1024;
/// Misses are retried after this long (lyrics get added to LRCLIB all the time).
const MISS_TTL: i64 = 3 * 86_400;

#[derive(Serialize, Deserialize)]
struct Cached {
    result: LyricsResult,
    #[serde(default)]
    meta: LyricsMeta,
    #[serde(default)]
    at: i64,
}

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
            let path = dir.join(format!("v2-{:016x}.json", req.key));
            if req.pick == 0 && !req.force {
                if let Ok(bytes) = std::fs::read(&path) {
                    if let Ok(c) = serde_json::from_slice::<Cached>(&bytes) {
                        let stale_miss =
                            c.result == LyricsResult::NotFound && unix_secs() - c.at > MISS_TTL;
                        if !stale_miss || !allow_net {
                            let _ = std::fs::write(&path, &bytes); // touch for LRU
                            bus.send(Event::Lyrics {
                                key: req.key,
                                result: c.result,
                                meta: c.meta,
                            });
                            continue;
                        }
                    }
                }
            }
            if !allow_net {
                bus.send(Event::Lyrics {
                    key: req.key,
                    result: LyricsResult::NotFound,
                    meta: LyricsMeta::default(),
                });
                continue;
            }
            let (result, meta) = fetch(&req);
            if !matches!(result, LyricsResult::Error(_)) {
                let c = Cached {
                    result: result.clone(),
                    meta: meta.clone(),
                    at: unix_secs(),
                };
                if let Ok(bytes) = serde_json::to_vec(&c) {
                    let _ = std::fs::write(&path, bytes);
                }
                prune_dir(&dir, CACHE_CAP);
            }
            log(&format!(
                "lyrics: {} via {} (match {}/{})",
                match &result {
                    LyricsResult::Synced(l) => format!("{} synced lines", l.len()),
                    LyricsResult::Plain(l) => format!("{} plain lines", l.len()),
                    LyricsResult::Instrumental => "instrumental".into(),
                    LyricsResult::NotFound => "not found".into(),
                    LyricsResult::Error(e) => format!("error {e}"),
                },
                if meta.source.is_empty() {
                    "-"
                } else {
                    &meta.source
                },
                meta.alt + 1,
                meta.alts.max(1)
            ));
            bus.send(Event::Lyrics {
                key: req.key,
                result,
                meta,
            });
        }
    });
    tx
}

// ---------------------------------------------------------------- query building

const NOISE: &[&str] = &[
    "official music video",
    "official video",
    "official audio",
    "official lyric video",
    "official lyrics video",
    "official lyrics",
    "official visualizer",
    "official mv",
    "lyric video",
    "lyrics video",
    "with lyrics",
    "lyrics",
    "lyric",
    "audio",
    "video",
    "visualizer",
    "music video",
    "full song",
    "full video",
    "video song",
    "hd",
    "hq",
    "4k",
    "mv",
    "m/v",
    "slowed",
    "reverb",
    "slowed + reverb",
    "slowed and reverb",
    "sped up",
    "8d audio",
    "8d",
    "nightcore",
    "remastered",
    "live",
    "explicit",
    "clean",
];

/// Lowercase words, accents and punctuation dropped.
pub fn tokens(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .filter(|w| {
            ![
                "the",
                "a",
                "an",
                "ft",
                "feat",
                "featuring",
                "x",
                "and",
                "official",
            ]
            .contains(w)
        })
        .map(String::from)
        .collect()
}

/// Removes bracketed noise, hashtags, "ft." parts and channel tails.
pub fn strip_noise(s: &str) -> String {
    let mut t = s.replace(['“', '”', '"', '«', '»'], "");
    // bracketed parts: drop any that are noise (or that mention lyrics/video/audio/remix tags)
    loop {
        let mut changed = false;
        for (open, close) in [('(', ')'), ('[', ']'), ('【', '】'), ('{', '}')] {
            let mut search_from = 0;
            while let Some(a_rel) = t[search_from..].find(open) {
                let a = search_from + a_rel;
                let Some(b_rel) = t[a..].find(close) else {
                    break;
                };
                let b = a + b_rel;
                let inner = t[a + open.len_utf8()..b].trim().to_lowercase();
                let noisy = NOISE.iter().any(|n| inner.contains(n))
                    || inner.starts_with("ft")
                    || inner.starts_with("feat")
                    || inner.starts_with("prod")
                    || inner.contains("from ")
                    || inner.contains("cover");
                if noisy {
                    t.replace_range(a..b + close.len_utf8(), " ");
                    changed = true;
                } else {
                    search_from = b + close.len_utf8();
                }
            }
        }
        if !changed {
            break;
        }
    }
    // hashtags and trailing channel bits after a bar: "Song | Lyrics", "Song #shorts"
    let t: String = t
        .split_whitespace()
        .filter(|w| !w.starts_with('#'))
        .collect::<Vec<_>>()
        .join(" ");
    let mut t = t;
    for sep in [" ft. ", " feat. ", " ft ", " feat ", " featuring "] {
        if let Some(p) = t.to_lowercase().find(sep) {
            // "Song ft. X - Artist" keeps the part after a dash
            let rest = &t[p + sep.len()..];
            let tail = rest
                .find(" - ")
                .map(|d| rest[d..].to_string())
                .unwrap_or_default();
            t = format!("{}{}", &t[..p], tail);
        }
    }
    t.trim()
        .trim_matches(|c: char| c == '-' || c == '|' || c == '·' || c.is_whitespace())
        .to_string()
}

fn channelish(a: &str) -> bool {
    let l = a.to_lowercase();
    l.is_empty()
        || l.ends_with("vevo")
        || l.ends_with(" - topic")
        || [
            "records",
            "music",
            "official",
            "tv",
            "lyrics",
            "clouds",
            "beats",
            "studio",
            "entertainment",
            "films",
            "label",
            "channel",
            "mix",
            "vibes",
        ]
        .iter()
        .any(|k| l.contains(k))
}

/// Track / artist pairs to try, best guesses first, plus a free-text query.
pub fn candidates(r: &LyricsRequest) -> (Vec<(String, String)>, String) {
    let mut pairs: Vec<(String, String)> = Vec::new();
    fn push_to(pairs: &mut Vec<(String, String)>, t: &str, a: &str) {
        let (t, a) = (strip_noise(t), strip_noise(a));
        if t.len() >= 2
            && !pairs
                .iter()
                .any(|(pt, pa)| pt.eq_ignore_ascii_case(&t) && pa.eq_ignore_ascii_case(&a))
        {
            pairs.push((t, a));
        }
    }
    macro_rules! push {
        ($t:expr, $a:expr) => {
            push_to(&mut pairs, $t, $a)
        };
    }
    let raw_t = if r.raw_title.is_empty() {
        &r.title
    } else {
        &r.raw_title
    };
    let raw_a = if r.raw_artist.is_empty() {
        &r.artist
    } else {
        &r.raw_artist
    };
    let artist = raw_a.trim_end_matches(" - Topic").trim().to_string();
    let artist = if artist.to_lowercase().ends_with("vevo") {
        artist[..artist.len() - 4].trim().to_string()
    } else {
        artist
    };
    let title = strip_noise(raw_t);
    let mut split = None;
    for sep in [" - ", " – ", " — ", " | ", " ~ ", ": ", " · "] {
        if let Some(p) = title.find(sep) {
            let (l, rr) = (title[..p].trim(), title[p + sep.len()..].trim());
            // "Song - Artist | Channel": keep the first two parts
            let rr = rr.split(" | ").next().unwrap_or(rr).trim();
            if !l.is_empty() && !rr.is_empty() {
                split = Some((l.to_string(), rr.to_string()));
                break;
            }
        }
    }
    if let Some((l, rr)) = &split {
        if channelish(&artist) {
            // classic "Artist - Song", then the reversed "Song - Artist"
            push!(rr, l);
            push!(l, rr);
        } else if l.eq_ignore_ascii_case(&artist) {
            push!(rr, l);
        } else if rr.eq_ignore_ascii_case(&artist) {
            push!(l, rr);
        } else {
            push!(rr, l);
            push!(l, rr);
            push!(&title, &artist);
        }
    }
    if !channelish(&artist) {
        push!(&r.title, &r.artist);
        push!(&title, &artist);
    }
    if pairs.is_empty() {
        push!(&title, "");
    }
    let q = if channelish(&artist) {
        title.replace([' ', '-', '|', '–', '—', '·'], " ")
    } else {
        format!("{title} {artist}")
    };
    let q = q.split_whitespace().collect::<Vec<_>>().join(" ");
    (pairs, q)
}

// ---------------------------------------------------------------- search + scoring

#[derive(Deserialize, Clone)]
struct LrcLibItem {
    #[serde(default)]
    id: i64,
    #[serde(default, rename = "trackName")]
    track: String,
    #[serde(default, rename = "artistName")]
    artist: String,
    #[serde(default, rename = "syncedLyrics")]
    synced: Option<String>,
    #[serde(default, rename = "plainLyrics")]
    plain: Option<String>,
    #[serde(default)]
    instrumental: bool,
    #[serde(default)]
    duration: Option<f64>,
}

/// 0..1 overlap of query words found in the candidate's title + artist.
fn overlap(query: &[String], cand: &[String]) -> f32 {
    if query.is_empty() || cand.is_empty() {
        return 0.0;
    }
    let set: HashSet<&String> = cand.iter().collect();
    let hit = query.iter().filter(|w| set.contains(w)).count() as f32;
    let back: HashSet<&String> = query.iter().collect();
    let hit_back = cand.iter().filter(|w| back.contains(w)).count() as f32;
    // both directions, so a bare "Intro" doesn't match everything
    0.6 * hit / query.len() as f32 + 0.4 * hit_back / cand.len() as f32
}

fn score(i: &LrcLibItem, query: &[String], duration_s: i64) -> f32 {
    if !(i.instrumental || i.synced.is_some() || i.plain.is_some()) {
        return -1.0;
    }
    let cand = tokens(&format!("{} {}", i.track, i.artist));
    let mut s = overlap(query, &cand) * 10.0;
    // title words must be there: the track name has to share something with the query
    if overlap(query, &tokens(&i.track)) <= 0.0 {
        s -= 6.0;
    }
    if let (Some(d), true) = (i.duration, duration_s > 0) {
        let diff = (d - duration_s as f64).abs() as f32;
        // videos run longer than the track (intros, outros): be generous up to ~25 s
        s += if diff <= 3.0 {
            2.5
        } else if diff <= 25.0 {
            2.0 - diff / 25.0 * 2.0
        } else {
            -(diff / 60.0).min(3.0)
        };
    }
    if i.synced.as_deref().is_some_and(|x| !x.trim().is_empty()) {
        s += 1.5;
    }
    s
}

fn lrclib(url: &str) -> Result<Vec<LrcLibItem>, String> {
    let resp = http::get(url, &[])?;
    if resp.status == 404 {
        return Ok(Vec::new());
    }
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status));
    }
    if resp.body.trim_start().starts_with('[') {
        Ok(serde_json::from_str(&resp.body).unwrap_or_default())
    } else {
        Ok(serde_json::from_str::<LrcLibItem>(&resp.body)
            .map(|i| vec![i])
            .unwrap_or_default())
    }
}

/// Runs one search synchronously (for `notchly --lyrics-check`).
pub fn check(title: &str, artist: &str, duration_s: i64) -> (LyricsResult, LyricsMeta) {
    let (t, a) = super::media::clean_title(title, artist);
    fetch(&LyricsRequest {
        key: 0,
        title: t,
        artist: a,
        album: String::new(),
        duration_s,
        raw_title: title.into(),
        raw_artist: artist.into(),
        pick: 0,
        force: true,
    })
}

fn fetch(r: &LyricsRequest) -> (LyricsResult, LyricsMeta) {
    let (pairs, q) = candidates(r);
    let query_tokens = tokens(&q);
    let mut meta = LyricsMeta {
        query: pairs
            .first()
            .map(|(t, a)| {
                if a.is_empty() {
                    t.clone()
                } else {
                    format!("{t} — {a}")
                }
            })
            .unwrap_or_else(|| q.clone()),
        ..Default::default()
    };
    let mut items: Vec<LrcLibItem> = Vec::new();
    let mut seen = HashSet::new();
    let mut errors = 0;
    let mut add = |v: Vec<LrcLibItem>, items: &mut Vec<LrcLibItem>| {
        for i in v {
            if seen.insert(if i.id != 0 {
                i.id
            } else {
                fnv1a(&format!("{}{}", i.track, i.artist)) as i64
            }) {
                items.push(i);
            }
        }
    };
    // exact lookups for the two best pairs
    for (t, a) in pairs.iter().take(2) {
        if a.is_empty() {
            continue;
        }
        let mut url = format!(
            "https://lrclib.net/api/get?track_name={}&artist_name={}",
            http::enc(t),
            http::enc(a)
        );
        if !r.album.is_empty() && !channelish(a) {
            url.push_str(&format!("&album_name={}", http::enc(&r.album)));
        }
        if r.duration_s > 0 {
            url.push_str(&format!("&duration={}", r.duration_s));
        }
        match lrclib(&url) {
            Ok(v) => add(v, &mut items),
            Err(_) => errors += 1,
        }
    }
    // free-text search, then field search for the best pair
    match lrclib(&format!(
        "https://lrclib.net/api/search?q={}",
        http::enc(&q)
    )) {
        Ok(v) => add(v, &mut items),
        Err(_) => errors += 1,
    }
    if let Some((t, a)) = pairs.first() {
        let url = format!(
            "https://lrclib.net/api/search?track_name={}&artist_name={}",
            http::enc(t),
            http::enc(a)
        );
        if let Ok(v) = lrclib(&url) {
            add(v, &mut items);
        }
    }
    let mut scored: Vec<(f32, LrcLibItem)> = items
        .into_iter()
        .map(|i| (score(&i, &query_tokens, r.duration_s), i))
        .filter(|(s, _)| *s >= 5.0)
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    if !scored.is_empty() {
        let pick = r.pick % scored.len();
        let (_, best) = scored[pick].clone();
        meta.source = "LRCLIB".into();
        meta.matched = format!("{} — {}", best.track, best.artist);
        meta.alt = pick;
        meta.alts = scored.len();
        meta.duration_s = best.duration.unwrap_or(0.0);
        return (to_result(best), meta);
    }
    // plain lyrics from lyrics.ovh
    for (t, a) in pairs.iter().filter(|(_, a)| !a.is_empty()).take(2) {
        let url = format!(
            "https://api.lyrics.ovh/v1/{}/{}",
            http::enc(a),
            http::enc(t)
        );
        if let Ok(resp) = http::get(&url, &[]) {
            if resp.ok() {
                let v: serde_json::Value = serde_json::from_str(&resp.body).unwrap_or_default();
                if let Some(text) = v["lyrics"].as_str().filter(|s| s.trim().len() > 20) {
                    let lines: Vec<String> = text
                        .replace("\r\n", "\n")
                        .lines()
                        .map(|l| l.trim().to_string())
                        .skip_while(|l| l.to_lowercase().starts_with("paroles de la chanson"))
                        .collect();
                    meta.source = "lyrics.ovh".into();
                    meta.matched = format!("{t} — {a}");
                    meta.alts = 1;
                    return (LyricsResult::Plain(lines), meta);
                }
            }
        }
    }
    if errors >= 3 {
        return (
            LyricsResult::Error("Couldn't reach the lyrics service".into()),
            meta,
        );
    }
    (LyricsResult::NotFound, meta)
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

/// How long lyrics should wait when the music became audible `onset_ms` into the player's
/// clock. Songs start within half a second, so anything later is a silent intro or start-up
/// buffering. Videos much longer than the matched recording may carry a longer intro.
pub fn auto_delay(onset_ms: i64, track_s: f64, video_s: f64) -> i64 {
    let lead = onset_ms - 150;
    if lead < 500 {
        return 0;
    }
    let extra = if track_s > 0.0 {
        (video_s - track_s).max(0.0)
    } else {
        0.0
    };
    let cap = if extra > 12.0 {
        ((extra + 1.5) * 1000.0).min(45_000.0) as i64
    } else {
        12_000
    };
    lead.min(cap)
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
    fn req(title: &str, artist: &str, dur: i64) -> LyricsRequest {
        LyricsRequest {
            key: 0,
            title: title.into(),
            artist: artist.into(),
            album: String::new(),
            duration_s: dur,
            raw_title: title.into(),
            raw_artist: artist.into(),
            pick: 0,
            force: false,
        }
    }

    #[test]
    fn youtube_titles_become_song_and_artist() {
        // channel uploads: "Song - Artist" and "Artist - Song" are both tried
        let (p, q) = candidates(&req(
            "Dilruba Dilnasheen - Rito Riba (Lyrics)",
            "7clouds",
            200,
        ));
        assert!(p.contains(&("Rito Riba".into(), "Dilruba Dilnasheen".into())));
        assert!(p.contains(&("Dilruba Dilnasheen".into(), "Rito Riba".into())));
        assert_eq!(q, "Dilruba Dilnasheen Rito Riba");
        // an artist's own upload with noise and a feature
        let (p, _) = candidates(&req(
            "Arijit Singh - Kesariya (Official Video) ft. Someone",
            "Arijit Singh",
            260,
        ));
        assert_eq!(p[0], ("Kesariya".into(), "Arijit Singh".into()));
        // Spotify-style clean metadata stays as is
        let (p, _) = candidates(&req("Blinding Lights", "The Weeknd", 200));
        assert_eq!(p[0], ("Blinding Lights".into(), "The Weeknd".into()));
    }

    #[test]
    fn noise_is_stripped() {
        assert_eq!(strip_noise("Song [Official Music Video] #shorts"), "Song");
        assert_eq!(strip_noise("Song (slowed + reverb)"), "Song");
        assert_eq!(strip_noise("Song (Live at Wembley)"), "Song");
        assert_eq!(strip_noise("Song (Remix)"), "Song (Remix)");
    }

    #[test]
    fn scoring_prefers_the_right_track() {
        let q = tokens("Dilruba Dilnasheen Rito Riba");
        let good = LrcLibItem {
            id: 1,
            track: "Dilruba Dilnasheen".into(),
            artist: "Rito Riba".into(),
            synced: Some("[00:01.00]x".into()),
            plain: None,
            instrumental: false,
            duration: Some(198.0),
        };
        let wrong = LrcLibItem {
            id: 2,
            track: "Something Else".into(),
            artist: "Rito Riba".into(),
            synced: Some("[00:01.00]x".into()),
            plain: None,
            instrumental: false,
            duration: Some(198.0),
        };
        assert!(score(&good, &q, 205) > 5.0);
        assert!(score(&wrong, &q, 205) < score(&good, &q, 205));
        assert!(score(&wrong, &q, 205) < 5.0);
    }

    #[test]
    fn auto_sync_delay() {
        // music audible right away: no delay
        assert_eq!(auto_delay(300, 200.0, 201.0), 0);
        // a 3 s silent logo before the song
        assert_eq!(auto_delay(3150, 200.0, 203.0), 3000);
        // long silence but the video is barely longer than the track: capped at 12 s
        assert_eq!(auto_delay(30_000, 200.0, 202.0), 12_000);
        // a 25 s cinematic intro in a much longer video is allowed
        assert_eq!(auto_delay(25_150, 200.0, 226.0), 25_000);
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
