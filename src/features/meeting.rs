//! Meeting detection: which call app is using the microphone, and a small tracker that keeps
//! meeting mode alive while the call goes on even when its window is no longer in front.
//!
//! Desktop call apps are recognised by their process name or path. Calls in a browser are
//! recognised from the titles of the browser's top-level windows (all of them, not only the
//! foreground one); once a browser call is active it stays active for as long as that browser
//! keeps the microphone, so switching to another app or tab doesn't end it. A short grace
//! period absorbs one-poll gaps in the microphone signal.

use std::time::{Duration, Instant};

/// Desktop call apps: a substring of the process name/path → display name.
const NATIVE_APPS: &[(&str, &str)] = &[
    ("ms-teams", "Microsoft Teams"),
    ("teams", "Microsoft Teams"),
    ("zoom", "Zoom"),
    ("discord", "Discord"),
    ("slack", "Slack"),
    ("whatsapp", "WhatsApp"),
    ("skype", "Skype"),
    ("webex", "Webex"),
];

const BROWSERS: &[&str] = &["chrome", "msedge", "firefox", "brave", "opera", "vivaldi"];

/// How long the call may be missing (no app, no browser holding the mic) before meeting mode
/// ends.
pub const GRACE: Duration = Duration::from_secs(6);

/// The desktop call app named in `name_and_path` (lower-case app name and exe path), if any.
pub fn native_app(name_and_path: &str) -> Option<&'static str> {
    let l = name_and_path.to_lowercase();
    // a browser's path can contain "teams" etc. in a profile name; browsers are handled
    // separately from their window titles
    if is_browser(&l) {
        return None;
    }
    NATIVE_APPS
        .iter()
        .find(|(k, _)| l.contains(k))
        .map(|(_, name)| *name)
}

pub fn is_browser(name_or_path: &str) -> bool {
    let l = name_or_path.to_lowercase();
    BROWSERS.iter().any(|b| l.contains(b))
}

/// The call provider shown in a browser window title, most specific first. A title that
/// looks like a call but names no known provider gives the generic "Meeting".
pub fn provider_from_title(title: &str) -> Option<&'static str> {
    let t = title.to_lowercase();
    let has = |s: &str| t.contains(s);
    if has("meet.google.com")
        || has("google meet")
        || t.starts_with("meet - ")
        || t.starts_with("meet – ")
    {
        return Some("Google Meet");
    }
    if has("teams.microsoft.com") || has("teams.live.com") || has("microsoft teams") {
        return Some("Microsoft Teams");
    }
    if has("zoom.us")
        || has("zoom meeting")
        || has("zoom webinar")
        || has("zoom workplace")
        || t.starts_with("zoom")
    {
        return Some("Zoom");
    }
    if has("webex") {
        return Some("Webex");
    }
    if has("meet.jit.si") || has("jitsi") {
        return Some("Jitsi Meet");
    }
    if has("whereby") {
        return Some("Whereby");
    }
    if has("discord") {
        return Some("Discord");
    }
    if has("slack") && has("huddle") {
        return Some("Slack");
    }
    if has("meeting") || has("video call") || has("voice call") || has("huddle") || has("call with")
    {
        return Some("Meeting");
    }
    None
}

/// What this poll found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    /// A call app or browser call identified right now.
    pub detected: Option<String>,
    /// Whether the call was found in a browser.
    pub in_browser: bool,
    /// Whether any browser holds the microphone right now.
    pub browser_mic: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// Nothing to do.
    Same,
    Started(String),
    /// Still the same call, but now we know its provider better ("Meeting" → "Zoom").
    Renamed(String),
    Ended,
}

#[derive(Debug, Default)]
pub struct Tracker {
    active: Option<(String, bool)>,
    missing_since: Option<Instant>,
}

impl Tracker {
    #[cfg(test)]
    pub fn active(&self) -> Option<&str> {
        self.active.as_ref().map(|(n, _)| n.as_str())
    }

    /// When the current call ends unless it shows up again (the end of the grace period).
    pub fn deadline(&self) -> Option<Instant> {
        self.missing_since.map(|t| t + GRACE)
    }

    /// Feeds one poll. `now` is passed in so tests can step time.
    pub fn update(&mut self, seen: &Seen, now: Instant) -> Change {
        if let Some(name) = &seen.detected {
            self.missing_since = None;
            return match &mut self.active {
                None => {
                    self.active = Some((name.clone(), seen.in_browser));
                    Change::Started(name.clone())
                }
                Some((cur, in_browser)) => {
                    *in_browser = seen.in_browser;
                    // a specific provider replaces the generic name, never the other way
                    if cur != name && (cur == "Meeting" || name != "Meeting") {
                        *cur = name.clone();
                        Change::Renamed(name.clone())
                    } else {
                        Change::Same
                    }
                }
            };
        }
        let Some((_, in_browser)) = &self.active else {
            return Change::Same;
        };
        // a browser call whose tab or window is no longer visible: still a call while the
        // browser keeps the microphone
        if *in_browser && seen.browser_mic {
            self.missing_since = None;
            return Change::Same;
        }
        let since = *self.missing_since.get_or_insert(now);
        if now.duration_since(since) >= GRACE {
            self.active = None;
            self.missing_since = None;
            Change::Ended
        } else {
            Change::Same
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn providers_from_browser_titles() {
        let cases = [
            ("Meet - abc-defg-hij - Google Chrome", Some("Google Meet")),
            // Edge puts a zero-width space in its window titles
            ("Google Meet - Microsoft\u{200B} Edge", Some("Google Meet")),
            ("Zoom Meeting - Google Chrome", Some("Zoom")),
            ("Zoom - Personal - Microsoft Edge", Some("Zoom")),
            (
                "Meeting with Ana | Microsoft Teams - Microsoft Edge",
                Some("Microsoft Teams"),
            ),
            ("Calendar | Microsoft Teams", Some("Microsoft Teams")),
            ("Cisco Webex Meetings - Google Chrome", Some("Webex")),
            ("Jitsi Meet - Mozilla Firefox", Some("Jitsi Meet")),
            ("Whereby - Brave", Some("Whereby")),
            ("#general | Discord - Google Chrome", Some("Discord")),
            ("Huddle in #team | Slack - Google Chrome", Some("Slack")),
            ("Weekly sync meeting - Mozilla Firefox", Some("Meeting")),
            ("Inbox (3) - Gmail - Google Chrome", None),
            ("YouTube - Google Chrome", None),
        ];
        for (title, want) in cases {
            assert_eq!(provider_from_title(title), want, "{title}");
        }
    }

    #[test]
    fn the_old_bug_zoom_and_teams_are_not_google_meet() {
        assert_eq!(provider_from_title("Zoom Meeting"), Some("Zoom"));
        assert_eq!(
            provider_from_title("Microsoft Teams"),
            Some("Microsoft Teams")
        );
    }

    #[test]
    fn native_apps_but_not_browsers() {
        assert_eq!(
            native_app(r"zoom c:\users\a\appdata\roaming\zoom\bin\zoom.exe"),
            Some("Zoom")
        );
        assert_eq!(
            native_app("microsoft teams ms-teams.exe"),
            Some("Microsoft Teams")
        );
        assert_eq!(native_app("discord discord.exe"), Some("Discord"));
        // a browser profile called "teams" must not count as the Teams app
        assert_eq!(native_app(r"google chrome c:\teams\chrome.exe"), None);
        assert!(is_browser("Microsoft Edge msedge.exe"));
        assert!(!is_browser("zoom.exe"));
    }

    fn seen(detected: Option<&str>, in_browser: bool, browser_mic: bool) -> Seen {
        Seen {
            detected: detected.map(String::from),
            in_browser,
            browser_mic,
        }
    }

    #[test]
    fn browser_call_survives_switching_away() {
        let t0 = Instant::now();
        let mut tr = Tracker::default();
        assert_eq!(
            tr.update(&seen(Some("Google Meet"), true, true), t0),
            Change::Started("Google Meet".into())
        );
        // the user switches to another app / tab: the title is gone, the mic isn't
        for s in 1..60 {
            let r = tr.update(&seen(None, false, true), t0 + Duration::from_secs(s));
            assert_eq!(r, Change::Same);
        }
        assert_eq!(tr.active(), Some("Google Meet"));
        // the call ends: the browser lets go of the microphone
        let t1 = t0 + Duration::from_secs(60);
        assert_eq!(tr.update(&seen(None, false, false), t1), Change::Same);
        assert_eq!(
            tr.update(&seen(None, false, false), t1 + GRACE),
            Change::Ended
        );
        assert_eq!(tr.active(), None);
    }

    #[test]
    fn a_one_poll_gap_does_not_end_the_call() {
        let t0 = Instant::now();
        let mut tr = Tracker::default();
        tr.update(&seen(Some("Zoom"), false, false), t0);
        assert_eq!(
            tr.update(&seen(None, false, false), t0 + Duration::from_secs(2)),
            Change::Same
        );
        assert_eq!(
            tr.update(
                &seen(Some("Zoom"), false, false),
                t0 + Duration::from_secs(4)
            ),
            Change::Same
        );
        // the grace period restarts after the call came back
        assert_eq!(
            tr.update(&seen(None, false, false), t0 + Duration::from_secs(9)),
            Change::Same
        );
        assert_eq!(tr.active(), Some("Zoom"));
    }

    #[test]
    fn generic_name_is_upgraded_but_never_downgraded() {
        let t0 = Instant::now();
        let mut tr = Tracker::default();
        tr.update(&seen(Some("Meeting"), true, true), t0);
        assert_eq!(
            tr.update(&seen(Some("Zoom"), true, true), t0),
            Change::Renamed("Zoom".into())
        );
        assert_eq!(
            tr.update(&seen(Some("Meeting"), true, true), t0),
            Change::Same
        );
        assert_eq!(tr.active(), Some("Zoom"));
    }
}
