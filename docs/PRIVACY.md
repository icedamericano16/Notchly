# Privacy policy

*Effective 24 November 2026. Applies to Notchly 0.3.0 and later for Windows.*

Notchly runs on your PC. It has no account, no telemetry and no analytics. Nothing you copy,
type, hear or see is sent anywhere unless you use a feature that obviously needs the network,
and every such feature can be switched off in **Settings › Privacy**.

## Network connections (complete list)

| Feature | Endpoint | What is sent |
|---|---|---|
| Synced lyrics | lrclib.net, then api.lyrics.ovh if LRCLIB has nothing | song title, artist, album, duration |
| Currency rates | api.frankfurter.app | nothing (daily rate request) |
| Weather | api.met.no | coordinates rounded to 2 decimals |
| City search | geocoding-api.open-meteo.com | the city name you type |
| Calendar | Google Calendar API, Microsoft Graph, your ICS URLs | OAuth token / request for your events |
| Ask / Translate | your chosen provider (Ollama stays on this PC; Gemini, OpenAI or Anthropic only with your key) | your prompt and any context you attach |
| Updates (installer build only) | the publisher's GitHub Releases (`update.json` and its signature) | nothing (version check, at most daily) |
| Phone Drop | your local network only | files you choose to send/receive |

Every request uses HTTPS (except Ollama on localhost and Phone Drop on your LAN), identifies
itself as `Notchly/<version>`, and times out after 10 seconds.

## Google Calendar and Outlook (Microsoft 365)

Connecting a calendar is optional and starts only when you click *Connect* in Settings ›
Calendar & weather. Sign-in happens in your own browser with Google or Microsoft; Notchly
never sees your password.

- **What Notchly asks for**: Google — `calendar.calendarlist.readonly` and `calendar.events.readonly` (see which calendars you have and read their events; nothing else, and nothing is written);
  Microsoft — `Calendars.Read` and `offline_access` (read your calendar; stay signed in).
  Notchly cannot create, change or delete anything.
- **What it reads**: the title, time, location and online-meeting link of events in the next
  few days, to show your next event in the notch and remind you before it starts.
- **Where it goes**: nowhere. Events are fetched directly from Google or Microsoft to your PC,
  shown, and kept only in memory (plus a short-lived local cache). Nothing is sent to the
  publisher or anyone else; there is no Notchly server.
- **Tokens**: the refresh token is stored in Windows Credential Manager under your Windows
  account. *Disconnect* deletes it; you can also revoke access any time at
  <https://myaccount.google.com/permissions> or <https://account.microsoft.com/privacy/app-access>.

**Google API Services User Data Policy.** Notchly's use and transfer of information received
from Google APIs to any other app adheres to the
[Google API Services User Data Policy](https://developers.google.com/terms/api-services-user-data-policy),
including the Limited Use requirements. Google user data is used only to show your own
calendar to you on your own PC; it is not transferred to anyone, not used for advertising, not
used to train AI or machine-learning models, and not read by any human.

## Reading Windows notifications

To mirror notifications, Notchly uses the Windows notification listener when the app has
package identity (Store, or the installer with notification mirroring). Otherwise it reads the
Action Center's own store (`%LOCALAPPDATA%\Microsoft\Windows\Notifications\wpndatabase.db`)
**read-only**: the same toasts you see in Windows, on this PC only. Nothing is written there and
nothing leaves the PC. Turn it off with *Mirror Windows notifications* in Settings › Pop-ups.

## Glass, Liquid Glass and the screen behind the notch

To draw Glass and Liquid Glass, Notchly reads the part of the screen directly behind the notch (Windows
Desktop Duplication) into a texture on your graphics card. Those pixels **stay on the GPU**:
they are never copied to the computer's memory, never written to disk, never logged and never
sent anywhere. Two numbers derived from them are read back — how bright it is behind the notch
(to keep text readable) and a 64-bin histogram of what changed (to skip redrawing when nothing
did) — and neither is stored.

While Glass or Liquid Glass reads the screen, Windows keeps the notch itself out of screenshots,
screen recordings and screen shares (`WDA_EXCLUDEFROMCAPTURE`), so the glass never sees itself.
If you want the notch to appear in recordings, turn on *Show Notchly in screen recordings*
(Settings › Glass & Fluid › Advanced glass); the notch then shows tinted glass, which reads
nothing. Choosing *Solid* stops all reading of the screen too. Settings › Privacy shows whether
it's in use.

## Camera, microphone and location indicators

The privacy dots and Control Center's *In use* tile ask Windows which apps are using these
right now: the audio engine (apps recording from a microphone), Media Foundation's sensor
activity monitor (apps streaming a camera) and the consent store Windows keeps for location.
Only app names and their file paths are read — never audio, video or positions — and the 7-day
history stays on this PC.

## Data stored on your PC

Under `%APPDATA%\Notchly`: settings, themes, timer/water state, to-dos, notes, shelf references,
clipboard history (with exclusions for password managers and apps you list), privacy-dot history
(7 days) and caches (lyrics, weather, rates). Logs without personal content are under
`%LOCALAPPDATA%\Notchly\logs`; *Debug logging* (off by default) adds input and camera /
microphone events (app names only) for troubleshooting. API keys are encrypted with
Windows DPAPI (`secrets.bin`); calendar refresh tokens live in Windows Credential Manager.
Notification history is kept only if you turn on *Keep notification history*; it is then
encrypted with DPAPI (`notifications.bin`) and deleted when you turn the option off. Crash
reports are written to the logs folder and are only shared if you copy and send one yourself.
Uninstalling offers to delete all of it.

## Permissions

The keyboard hook only reacts to Caps/Num Lock and language-switch keys and never records
keystrokes. Location is requested only when you ask for it. Phone Drop is LAN-only, PIN-protected
and stops after 5 minutes idle, or immediately after repeated wrong PINs; it uses plain HTTP
inside your network.

## Updates

The installer build checks the publisher's GitHub Releases at most once a day. It downloads a
small manifest and its signature and installs nothing unless the manifest is signed with the
publisher's key and the installer matches the manifest's SHA-256 checksum; even then it asks
you first. No identifier is sent.

## Children

Notchly is a general-purpose desktop utility, not directed at children, and collects no
personal information from anyone.

## Changes and contact

Changes to this policy are listed in the changelog and dated at the top of this page. Questions:
open an issue on the project's GitHub page.
