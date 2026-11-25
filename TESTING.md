# Release checklist

Go through this before tagging **every** release, on a real PC with the **installer build**
(not `cargo run`). Section 0 is automatic; every other section **needs manual
verification**: it covers hardware, accounts or real-world behaviour that tests can't prove.
Fill in the last column with **P** (pass) or **F** (fail, plus a note), and record:

Tester / date / version / build (installer or Store) / Windows version / GPU:

`_________________________________________________________________________`

## 0. Automatic: CI and the release scripts (no manual action)

Check that these are green for the commit being tagged; don't repeat them by hand.

| Check | Where |
|---|---|
| `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test` | `ci.yml` (every push) |
| Release build under the 4 MB size budget | `ci.yml` |
| No hard-coded test counts or "~N MB" sizes in README / Store listing | `ci.yml` |
| No secrets in the full git history (gitleaks) | `ci.yml` › secrets |
| Tag = `v<Cargo version>`, CHANGELOG has a dated section for it (not "Unreleased") | `release.yml` › `release-check.ps1 -Stage pre` |
| `update.json` version, URL and SHA-256 match the installer; signature matches the public key | `release.yml` › `release-check.ps1 -Stage post` |
| Release notes contain only this version's section, plus downloads and SHA-256 sums | `release.yml` › `release-notes.ps1` |
| Website: format, lint, unit tests, build, Playwright smoke tests, gitleaks | website repo `ci.yml` |

**Before you start**
- Settings › General › **Debug logging** on. The logs are in `%LOCALAPPDATA%\Notchly\logs`.
- Keep the **Diagnostics** page handy (Settings › General › Diagnostics › Open). It shows
  live input events and camera / microphone detection.
- Default settings unless a row says otherwise (Settings › Privacy › Reset all settings).

## 1. Water bell: click, long-press, right-click · *needs manual verification*

| # | Steps | Expected | P/F |
|---|---|---|---|
| 1.1 | Open the Water page. Click the bell once (short press). | Reminders toggle once: the bell swings and the slash draws or clears. Diagnostics shows exactly one `click`. Settings › Water › Water reminders matches. | |
| 1.2 | Press and hold the bell. | After the long-press time (500 ms by default) the menu opens **while still held**. Releasing does nothing more. Diagnostics shows `long press`, no `click`. | |
| 1.3 | Right-click the bell. | The same menu opens; reminders don't toggle. Diagnostics shows `right click` only. | |
| 1.4 | Press the bell, drag off it, release. | Nothing happens. | |
| 1.5 | Set Settings › General › Long-press time to 1000 ms; repeat 1.2. | The menu now opens after about 1 s; a 0.6 s hold is a click. | |
| 1.6 | Menu › Snooze 30 min, then Off for today, then turn reminders back on. | The line under the counter says "Snoozed until …", then "Off for today", then the next reminder time. Nothing rings while off. | |

## 2. Camera indicator during a real call · *needs manual verification*

| # | Steps | Expected | P/F |
|---|---|---|---|
| 2.1 | Join a **Google Meet** call in Chrome or Edge with the camera on. | Within about 2 s a green camera dot appears. Control Center › In use lists the browser ("Google Chrome" / "Microsoft Edge"). Diagnostics: Camera source **live**. | |
| 2.2 | Same in **Zoom** (desktop app). | The dot appears; In use shows "Zoom". | |
| 2.3 | Same in **Microsoft Teams** (new Teams). | The dot appears; In use shows "Microsoft Teams". | |
| 2.4 | Open the **Windows Camera** app. | The dot appears; In use shows "Camera". | |
| 2.5 | Turn the camera off in the call, or end it. | The dot fades out about 2.5 s later, without flickering on and off. | |
| 2.6 | Control Center › Camera switch off, then start the call again. | The call can't use the camera (Windows blocks it). Switching it back on restores access. | |

## 3. Microphone indicator · *needs manual verification*

| # | Steps | Expected | P/F |
|---|---|---|---|
| 3.1 | Start a voice call or the Voice Recorder app. | An orange microphone dot within about 2 s; In use names the app. | |
| 3.2 | Mute in the app (the app keeps the device open). | The dot stays: Windows still reports the capture. That's expected. | |
| 3.3 | End the call. | The dot leaves after about 2.5 s. | |
| 3.4 | In use › the app › Close (or Force quit). | The app closes and the dot goes. | |

## 4. Multi-monitor · *needs manual verification*

| # | Steps | Expected | P/F |
|---|---|---|---|
| 4.1 | With two monitors, Settings › General › Display › Display 2. | The notch moves to the top centre of monitor 2 and opens on hover there. | |
| 4.2 | Unplug monitor 2. | The notch returns to the primary monitor with no crash and no ghost window. | |
| 4.3 | Plug it back in. | The notch goes back to monitor 2. | |
| 4.4 | Open Settings on monitor 2, resize it, close it, open it again. | It reopens at the same size and place. On a monitor it was never on, it opens centred at 1040 × 720. | |
| 4.5 | Try to make the Settings window smaller than its minimum. | It stops at 860 × 600 (DIP). | |

## 5. DPI scaling (100 / 125 / 150 / 200 %) · *needs manual verification*

Repeat at each scale (Windows Settings › Display › Scale). Changing it while Notchly runs is part of the test.

| # | Steps | Expected | 100 | 125 | 150 | 200 |
|---|---|---|---|---|---|---|
| 5.1 | Look at the idle notch and an expanded page. | Crisp text and icons; the notch hugs the screen top; no gaps or halo. | | | | |
| 5.2 | Hover just beside the notch (outside its shape). | The click goes through to the app underneath. | | | | |
| 5.3 | Open Settings; scroll Appearance, Theme Studio, Glass & Fluid. | Nothing clipped or overlapping; the preview island is centred. | | | | |
| 5.4 | Liquid Glass on. | The glass lines up with what's behind it (no offset). | | | | |

## 6. Sleep and resume · *needs manual verification*

| # | Steps | Expected | P/F |
|---|---|---|---|
| 6.1 | With music playing and Liquid Glass on, sleep the PC for 1 minute or more, then wake it. | The notch redraws correctly (no black or frozen glass). Music controls work. | |
| 6.2 | A timer was running before sleep. | It shows the correct remaining time after wake. | |
| 6.3 | Sleep across midnight (or change the date). | Water ml resets for the new day; history keeps yesterday. | |

## 7. Fullscreen game or video · *needs manual verification*

| # | Steps | Expected | P/F |
|---|---|---|---|
| 7.1 | Play a YouTube video in browser fullscreen (F). | The notch hides; it comes back when you leave fullscreen. | |
| 7.2 | Start a fullscreen game (exclusive or borderless). | The notch hides and causes no stutter. Pop-ups stay quiet. | |
| 7.3 | Add the game's exe to Quiet hours › Never hide for; repeat. | The notch stays visible over that game. | |

## 8. Reduced motion · *needs manual verification*

| # | Steps | Expected | P/F |
|---|---|---|---|
| 8.1 | Windows Settings › Accessibility › Visual effects › Animation effects **off**. | Notchly follows straight away: no overshoot or slides, and Fluid Styles hold a still frame (Glass & Fluid › Fluid status says so). | |
| 8.2 | Animations back on; Settings › Motion › Reduce motion on. | Same as 8.1. | |
| 8.3 | Motion › Animation speed 50 %, then 200 %. | Every animation is visibly slower, then faster. The setting survives a restart. | |

## 9. Themes and Theme Studio · *needs manual verification*

| # | Steps | Expected | P/F |
|---|---|---|---|
| 9.1 | Theme Studio › gallery: click Aurora, then Sunset, then Midnight. | Each applies immediately. Colours cross-fade smoothly with no grey midpoint. | |
| 9.2 | With a built-in theme active, change Accent start. | A toast says "Made a copy you can edit". A new "… copy" theme is active and edits apply live. | |
| 9.3 | Ctrl+Z, then Ctrl+Y. | The colour goes back, then forward again. | |
| 9.4 | Set Text to #3A3A3A. | Contrast shows a warning; **Fix** appears. Fix restores readable text and shows "AA ✓". | |
| 9.5 | In the picker, type `#12` in the hex field and press Enter. | The field shakes red; the colour doesn't change. | |
| 9.6 | Share › Copy code; Delete theme (confirm); Paste a share code. | The theme comes back ("Theme added"). | |
| 9.7 | Export…, then Import… the file. | Imported as a separate theme; built-ins are never overwritten. | |
| 9.8 | Mode › Follow Windows. Switch Windows between light and dark. | The theme switches between the dark and light-mode themes (Daylight by default). | |

## 10. Fluid Styles · *needs manual verification*

| # | Steps | Expected | P/F |
|---|---|---|---|
| 10.1 | Liquid Glass on. Glass & Fluid › Aurora; open a page. | Ribbons drift inside the glass, under the content, never outside the notch shape. | |
| 10.2 | Collapse to idle with *Pause fluid when idle* on; watch Task Manager for 30 s. | No movement; Notchly CPU about 0 %. | |
| 10.3 | Turn *Pause fluid when idle* off. | It keeps moving while idle at low CPU (under about 1 %). | |
| 10.4 | Switch the material to Glass. | Still frame only. | |

## 11. Settings: migration from an old version · *needs manual verification*

| # | Steps | Expected | P/F |
|---|---|---|---|
| 11.1 | Install 0.2.0. Change: water goal 3000 ml, animation speed Fast, theme Aurora, Liquid Glass. Quit. Install 0.3.0 over it. | All of it is kept: Water › Daily goal 3000 ml, Animation speed 180 %, Aurora, Liquid Glass. | |
| 11.2 | Quit. In `%APPDATA%\Notchly\settings.toml` set `hover_delay_ms = "fast"` and `size = 333`. Start. | It starts normally with those two at their defaults; everything else is kept. | |
| 11.3 | Make `settings.toml` invalid TOML (delete a `]`). Start. | Defaults load; the bad file is kept as `settings.bad`. | |
| 11.4 | Settings › Privacy › Export settings, Reset all settings (confirm), Import settings. | Reset restores the defaults (you aren't re-onboarded). Import brings everything back. | |
| 11.5 | Any section › Reset section › Reset; then Ctrl+Z. | The section returns to defaults, then comes back. | |

## 12. Full update from an older build · *needs manual verification*

Needs a real release pipeline (RELEASE_SETUP.md sections 1, 2 and 7).

| # | Steps | Expected | P/F |
|---|---|---|---|
| 12.1 | Install the previous release (built with the same `NOTCHLY_UPDATE_PUBKEY`). Publish the new tag. In the old build: Settings › About › Updates › Check now. | "Downloading Notchly x.y.z", then a prompt to install. Accepting installs silently and Notchly restarts on the new version. Settings are kept. | |
| 12.2 | On a test release, delete `update.json.sig` (or edit one character of `update.json`) and check again. | "The update isn't signed by Notchly"; nothing is downloaded. The log says the signature is missing or invalid. | |
| 12.3 | Check `Notchly-x.y.z-setup.exe` › Properties › Digital Signatures. | Your publisher, a timestamp, and "This digital signature is OK". | |
| 12.4 | Run the installer from a browser download. | No "unknown publisher" SmartScreen block (a new certificate can still show a reputation warning for a while). | |

## 13. Calendar sign-in (after RELEASE_SETUP.md sections 4 and 5) · *needs manual verification*

| # | Steps | Expected | P/F |
|---|---|---|---|
| 13.1 | Settings › Calendar & weather › Google Calendar › Connect. | The browser shows Google's consent screen with read-only calendar access. After allowing, "Connected", and the next event shows in the notch. | |
| 13.2 | Outlook / Microsoft 365 › Connect (personal account, then a work account). | The same, for both account types. | |
| 13.3 | Disconnect both. | Their Credential Manager entries (`Notchly/…`) are gone. | |

## 14. Meetings (browser and desktop) · *needs manual verification*

| # | Steps | Expected | P/F |
|---|---|---|---|
| 14.1 | Join a **Zoom** meeting in Chrome (web client) with the mic on. | Meeting mode names **Zoom**, not Google Meet. | |
| 14.2 | Same with **Teams** in Edge, then **Google Meet** in Chrome. | "Microsoft Teams", then "Google Meet". | |
| 14.3 | During a browser call, switch to another app and another tab for 2 minutes. | Meeting mode stays on the whole time. | |
| 14.4 | End the call (the browser releases the mic). | Meeting mode ends about 6 s later. | |
| 14.5 | Desktop Zoom or Teams call, app minimised. | Meeting mode stays on while the app holds the mic. | |

## 15. Downloads in progress · *needs manual verification*

| # | Steps | Expected | P/F |
|---|---|---|---|
| 15.1 | Download a large file (100 MB+) in Chrome, then in Edge and Firefox. | The download pill appears **while downloading**, with size and speed, within about 1–2 s. | |
| 15.2 | Let it finish. | The pill turns into "Download complete" with Open / Show / Shelf. | |
| 15.3 | Pause a download for 30 s, then resume; cancel another. | The pill stops refreshing while paused and comes back; a cancelled one disappears. | |
| 15.4 | Start Notchly while a download is already running. | It's picked up once the file grows. | |

## 16. Another city's time zone · *needs manual verification*

| # | Steps | Expected | P/F |
|---|---|---|---|
| 16.1 | Settings › Location: search **Tokyo** (from a PC in another zone). | Weather shows "Tokyo · GMT+9"; hourly times, sunrise and sunset are Tokyo's clock. | |
| 16.2 | Prayer times on, Tokyo set. | Times and the countdown match Tokyo's local times. | |
| 16.3 | Use Windows location (Locate). | The zone label disappears (this PC's clock). | |
| 16.4 | Upgrade from a version with a city saved before 0.3.1. | After the first weather refresh the zone label appears, with no popup. | |

## 17. Calendar subscriptions (ICS) · *needs manual verification*

| # | Steps | Expected | P/F |
|---|---|---|---|
| 17.1 | Outlook on the web › Shared calendars › Publish; paste the ICS link. | Events appear at the right times, including recurring meetings. | |
| 17.2 | A weekly Mon/Wed/Fri series, one occurrence deleted, one moved. | Mon/Wed/Fri shown; the deleted one is missing; the moved one is at its new time. | |
| 17.3 | A recurring event across a daylight-saving change. | Same local time before and after the change. | |
| 17.4 | Google "Secret address in iCal format". | Events appear. | |
| 17.5 | Build without client IDs: Settings › Calendar. | No Connect buttons; the ICS hints show instead. With your own client ID typed in, Connect appears. | |

## 18. Phone Drop · *needs manual verification*

| # | Steps | Expected | P/F |
|---|---|---|---|
| 18.1 | Start Phone Drop; scan the QR with a phone on the same Wi-Fi. | The page opens directly; the address bar shows no token. | |
| 18.2 | Scan the same QR with a second phone. | "Link already used": the PIN form. The PIN shown in Notchly lets it in. | |
| 18.3 | Send a photo from the phone; download a shared file on the phone. | The photo lands in the shelf; the file downloads. | |
| 18.4 | Type wrong PINs 8 times. | Phone Drop stops and Notchly says someone kept trying wrong PINs. | |
| 18.5 | Leave it idle 5 minutes. | It stops by itself. Both the page and the panel say "trusted Wi-Fi only". | |

## 19. Crash report · *needs manual verification*

| # | Steps | Expected | P/F |
|---|---|---|---|
| 19.1 | Quit Notchly; create `%LOCALAPPDATA%\Notchly\logs\crash-20990101-000000.txt` containing `panicked at src\x.rs:1:1: test`; start Notchly. | "Notchly restarted after a problem" with Report on GitHub / Copy report / Open folder. | |
| 19.2 | Report on GitHub. | A new-issue page opens pre-filled; your Windows user name doesn't appear in it. Nothing is submitted until you click Submit. | |
| 19.3 | Start Notchly again. | The notice doesn't repeat. | |

## 20. Microsoft Store build (MSIX) · *needs manual verification*

| # | Steps | Expected | P/F |
|---|---|---|---|
| 20.1 | Install from the Store (or sideload the CI's MSIX). | Starts; Settings › About shows the version. | |
| 20.2 | Allow notification access when asked. | Notifications from other apps are mirrored. | |
| 20.3 | Check for updates. | The self-updater is off (the Store updates the app). | |

## 21. Website after deploying · *needs manual verification*

| # | Steps | Expected | P/F |
|---|---|---|---|
| 21.1 | Open /download on the live site. | Version and SHA-256 match the GitHub release; files are labelled unsigned; with a Store URL configured, the Store button comes first. | |
| 21.2 | /changelog and /releases.atom. | The new version appears once; the feed has it. | |
| 21.3 | PageSpeed Insights (mobile) for / and /download. | Performance 90+, the others 100. | |
