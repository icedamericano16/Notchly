//! Ask (AI answers, maths, translation) and lyrics requests.

use super::*;

impl State {
    pub fn ask_submit(&mut self) {
        let text = self.input.text(InputId::Ask).text.trim().to_string();
        if text.is_empty() {
            return;
        }
        // "timer 12m" starts a one-off timer
        if let Some(m) = text
            .strip_prefix("timer ")
            .and_then(crate::features::timer::parse_duration)
        {
            let t = &mut self.f.state.timer;
            t.start(Phase::Custom, m);
            self.f.state_dirty = true;
            self.f.mark();
            self.input.text(InputId::Ask).take();
            self.popup(
                Popup::new(
                    PopKind::Info,
                    Icon::Timer,
                    format!("Timer · {}", crate::util::fmt_mmss((m * 60.0) as i64)),
                    "Started",
                )
                .duration_ms(1500),
            );
            return;
        }
        // palette prefixes
        if let Some(cmd) = text.strip_prefix('>') {
            self.f.ask.mode = AskMode::Palette;
            self.input.text(InputId::Ask).set(cmd.trim_start());
            self.update_palette();
            self.run_palette(self.f.ask.palette_sel);
            return;
        }
        if text.starts_with('=')
            || self.f.ask.mode != AskMode::Translate
                && crate::features::calc::evaluate(&text, &self.f.rates).is_some()
                && !text.starts_with('?')
        {
            if let Some(ans) = crate::features::calc::evaluate(&text, &self.f.rates) {
                self.requests
                    .push(Request::CopyText(ans.text.replace(',', "")));
                self.popup(
                    Popup::new(PopKind::Info, Icon::Calculator, ans.text, "Copied")
                        .duration_ms(1500),
                );
                return;
            }
        }
        if self.f.ask.mode == AskMode::Palette {
            self.run_palette(self.f.ask.palette_sel);
            return;
        }
        let text = text.trim_start_matches('?').trim().to_string();
        let mut context = String::new();
        if self.f.ask.chips[0] {
            if let Some(c) = self.f.clip.items.first() {
                context.push_str(&format!(
                    "Clipboard:\n{}\n\n",
                    c.text.chars().take(4000).collect::<String>()
                ));
            }
        }
        if self.f.ask.chips[1] {
            if let Some(r) = &self.f.ocr.result {
                context.push_str(&format!(
                    "Text from screen:\n{}\n\n",
                    r.text.chars().take(4000).collect::<String>()
                ));
            }
        }
        if self.f.ask.chips[2] && self.media.has_session() {
            context.push_str(&format!(
                "Current song: {} — {}\n\n",
                self.media.snap.title, self.media.snap.artist
            ));
        }
        if self.f.ask.chips[3] {
            if let Some(i) = self
                .f
                .shelf
                .items
                .iter()
                .find(|i| self.f.shelf.selected.contains(&i.id))
                .or(self.f.shelf.items.first())
            {
                let body = match &i.kind {
                    crate::features::store::ShelfKind::Text(t)
                    | crate::features::store::ShelfKind::Link(t) => t.clone(),
                    crate::features::store::ShelfKind::File { path, .. } => {
                        std::fs::read_to_string(path)
                            .map(|s| s.chars().take(6000).collect())
                            .unwrap_or_else(|_| format!("(file {})", i.name))
                    }
                    _ => format!("(image {})", i.name),
                };
                context.push_str(&format!("Shelf item \"{}\":\n{}\n\n", i.name, body));
            }
        }
        self.input.text(InputId::Ask).take();
        let ask = &mut self.f.ask;
        ask.request_id += 1;
        ask.error = None;
        ask.cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        ask.streaming = true;
        let (system, turns) = if ask.mode == AskMode::Translate {
            ask.translation.clear();
            (
                ai::translate_prompt(&self.settings.ai.translate_target),
                vec![(true, text)],
            )
        } else {
            ask.msgs.push(ChatMsg {
                user: true,
                text: format!("{context}{text}"),
            });
            ask.msgs.push(ChatMsg {
                user: false,
                text: String::new(),
            });
            let all: Vec<(bool, String)> = ask
                .msgs
                .iter()
                .filter(|m| !m.text.is_empty())
                .map(|m| (m.user, m.text.clone()))
                .collect();
            let turns: Vec<(bool, String)> = all[all.len().saturating_sub(12)..].to_vec();
            (ai::SYSTEM_PROMPT.to_string(), turns)
        };
        if let Some(bus) = self.svc.bus.clone() {
            ai::ask(
                bus,
                self.settings.ai.clone(),
                self.settings.privacy.net_ai,
                AiRequest {
                    id: ask.request_id,
                    turns,
                    system,
                },
                ask.cancel.clone(),
            );
        }
    }

    /// Translates the whole lyric block once per song (cached next to the lyrics).
    /// Asks the lyrics worker for the current song (`force` skips the cache: retry / next match).
    pub fn request_lyrics(&mut self, force: bool) {
        let s = &self.media.snap;
        if s.title.is_empty() {
            return;
        }
        self.lyrics.status = LyricsStatus::Loading;
        self.lyrics.translation = None;
        if let Some(tx) = &self.svc.lyrics {
            let _ = tx.send((
                LyricsRequest {
                    key: self.lyrics.key,
                    title: s.title.clone(),
                    artist: s.artist.clone(),
                    album: s.album.clone(),
                    duration_s: s.duration_ms / 1000,
                    raw_title: s.raw_title.clone(),
                    raw_artist: s.raw_artist.clone(),
                    pick: self.lyrics.pick,
                    force,
                },
                self.settings.privacy.net_lyrics,
            ));
        }
    }

    /// Song position on the lyrics clock: the player's position minus the auto-sync delay and
    /// the user's nudge (positive nudge = lyrics later).
    pub fn lyrics_pos(&self) -> i64 {
        self.media.position_ms() - self.lyrics.auto_delay_ms - self.lyrics_offset()
    }

    /// The user's timing nudge for the current song, in ms (positive = lyrics later).
    pub fn lyrics_offset(&self) -> i64 {
        self.f
            .state
            .lyrics_offsets
            .get(&format!("{:016x}", self.lyrics.key))
            .copied()
            .unwrap_or(0)
    }

    pub(super) fn translate_lyrics(&mut self) {
        let key = self.lyrics.key;
        let dir = crate::util::sub_dir("cache/lyrics");
        if let Ok(b) = std::fs::read(dir.join(format!("{key:016x}.tr.json"))) {
            if let Ok(lines) = serde_json::from_slice::<Vec<String>>(&b) {
                self.lyrics.translation = Some(lines);
                return;
            }
        }
        let LyricsStatus::Ready(crate::services::lyrics::LyricsResult::Synced(lines)) =
            &self.lyrics.status
        else {
            return;
        };
        let block: Vec<String> = lines.iter().map(|l| l.1.clone()).collect();
        let lang = self.settings.ai.translate_target.clone();
        let system = format!("Translate each line of these song lyrics to {lang}{}. Keep exactly one output line per input line, in order, including empty lines. Output only the lines.", if self.settings.music.lyrics_romanise { " and add a romanised reading in brackets" } else { "" });
        self.f.ask.translation.clear();
        if let Some(bus) = self.svc.bus.clone() {
            ai::ask(
                bus,
                self.settings.ai.clone(),
                self.settings.privacy.net_ai,
                AiRequest {
                    id: u64::MAX - 1,
                    turns: vec![(true, block.join("\n"))],
                    system,
                },
                std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            );
        }
    }
}
