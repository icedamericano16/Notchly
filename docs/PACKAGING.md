# Packaging and release

One script builds everything: `packaging\build.ps1`. It reads the version from `Cargo.toml`,
builds the release exe, generates the logos (`packaging\make_assets.py`), packs the sparse
identity package and the Store MSIX, signs what needs signing, builds the installer and writes
`update.json`. Output goes to `dist\` (ignored by git).

## What you need once

| Item | Why | Where |
|---|---|---|
| Microsoft Store account | Signing, hosting, automatic updates and package identity (notification mirroring) for free | storedeveloper.microsoft.com (individual, free) |
| Code-signing certificate (optional) | Only for signed GitHub downloads; without it they are labelled "advanced / unsigned" | SignPath Foundation (free, OSI-licensed projects only) or a .pfx you already have |
| Windows 10/11 SDK | `makeappx.exe`, `signtool.exe` | Visual Studio Installer |
| Inno Setup 6 | The direct installer | `winget install JRSoftware.InnoSetup` |
| Python 3 | Logo generation (standard library only) | python.org or `winget install Python.Python.3.12` |
| A place to host files | Installer download + `update.json` | GitHub Releases works for both |

## Build-time settings

These are read when the exe is compiled (see `src/build_info.rs`). All are optional; leave one out
and the matching feature falls back gracefully.

On a development PC, put them in `notchly.local.toml` at the repo root (git-ignored), one
`NOTCHLY_… = "value"` per line; `build.rs` picks it up. Environment variables override the file.

| Variable | Effect |
|---|---|
| `NOTCHLY_PUBLISHER` | Set automatically by `build.ps1` from `-Publisher`; links the exe to the sparse package |
| `NOTCHLY_UPDATE_FEED` | URL of `update.json` (e.g. `https://github.com/you/notchly/releases/latest/download/update.json`). Unset = no update checks |
| `NOTCHLY_WEBSITE` | Adds a Website link in Settings › About |
| `NOTCHLY_SUPPORT_URL` | Adds "Report a problem" (About, crash notice) |
| `NOTCHLY_GOOGLE_CLIENT_ID` / `NOTCHLY_GOOGLE_CLIENT_SECRET` | One-click Google Calendar sign-in |
| `NOTCHLY_MS_CLIENT_ID` | One-click Outlook / Microsoft 365 sign-in |

### Calendar sign-in clients
- **Google**: Google Cloud Console › APIs & Services › Credentials › *OAuth client ID* › type
  **Desktop app**. Enable the Google Calendar API and add the scope
  `https://www.googleapis.com/auth/calendar.readonly` to the consent screen, then publish the
  consent screen. Google documents desktop-client secrets as non-confidential, so baking it in is fine.
- **Microsoft** (optional; ICS subscriptions need no setup): Microsoft Entra admin center ›
  App registrations › New › *Accounts in any organizational directory
  and personal Microsoft accounts*. Add the platform **Mobile and desktop applications** with
  redirect URI `http://localhost`, enable *Allow public client flows*, and add the delegated
  permission `Calendars.Read`. No secret is needed.

## Releases

```powershell
# local test (self-signed cert, trusted only on this PC)
.\packaging\build.ps1 -SelfSigned

# real release
$env:NOTCHLY_UPDATE_FEED = "https://github.com/you/notchly/releases/latest/download/update.json"
.\packaging\build.ps1 -Publisher "CN=Your Name, O=Your Name, C=NP" `
    -PfxPath C:\keys\notchly.pfx -PfxPassword (Read-Host -AsSecureString) `
    -DownloadBase "https://github.com/you/notchly/releases/download/v0.1.3"
```

Upload `dist\Notchly-<version>-setup.exe` and `dist\update.json` to the release. `-Publisher`
must equal the certificate subject exactly (`(Get-PfxCertificate C:\keys\notchly.pfx).Subject`).

### Microsoft Store
```powershell
.\packaging\build.ps1 -StorePublisher "CN=XXXXXXXX-…" -StoreIdentity "12345YourName.Notchly" -NoInstaller
```
Both values are on Partner Center › your app › Product identity. Upload `dist\Notchly-<version>.msix`;
the Store signs it. Store builds update through the Store and never run the update checker.

## CI
`.github/workflows/ci.yml` runs fmt, clippy (`-D warnings`), tests, the release build, the 4 MB
size check and a secret scan on every push and pull request. On a `v*` tag,
`.github/workflows/release.yml` builds, checks and publishes the release; the variables and
secrets it reads are listed at the end of `RELEASE_SETUP.md`.

## Release checklist
1. Bump `version` in `Cargo.toml`, add a `CHANGELOG.md` entry, commit, tag `vX.Y.Z`.
2. Smoke-test the installer on a clean Windows 11 and Windows 10 VM: install, notch appears,
   hover/expand, a pop-up, notification mirroring (if enabled), uninstall removes everything.
3. Check 100 %, 150 % and 200 % scaling, a second monitor, and a taskbar at the top.
4. Publish the release, then confirm an older build offers the update.
