# Keys and secrets

Notchly's source is public and anyone can decompile the exe, so a value that ships inside
the app is **not** a secret. This page lists every key, credential and outside service the
app and the website use, which tier each belongs to, where it lives, and how to rotate it.
Nothing on this page may ever be committed with a real value.

## The three tiers

| Tier | Meaning | Where it may live |
|---|---|---|
| **1. Public by design** | Shipped inside the app; anyone can read it. Safe because the service is designed for public clients (OAuth installed-app flow, signature *verification* keys). | Injected at build time: `notchly.local.toml` on your PC (git-ignored), GitHub Actions variables/secrets in CI. Never in the repo. |
| **2. True secret** | Grants power to whoever holds it (signing releases, publishing). | Only as GitHub Actions **secrets**, used only inside workflows, plus a copy in your password manager. Never in the app, never on disk in the repo. |
| **3. Outside service** | Data from someone else's API. | Preference: (a) computed locally, (b) a free no-key service called from the user's PC with caching, (c) bring-your-own-key, off until the user adds a key, stored encrypted per user. |

## Tier 1: public by design

| Value | Build variable | Used for | Missing → |
|---|---|---|---|
| Google OAuth client ID (Desktop app type) | `NOTCHLY_GOOGLE_CLIENT_ID` | Google Calendar sign-in | "Connect" asks for the user's own client ID (Settings › Calendar & weather). |
| Google OAuth "client secret" | `NOTCHLY_GOOGLE_CLIENT_SECRET` | Sent with the token request. Google documents that a desktop client's secret is not confidential. Kept in a GitHub *secret* anyway so it isn't casually copied. | Same as above. |
| Microsoft public client ID | `NOTCHLY_MS_CLIENT_ID` | Outlook / Microsoft 365 sign-in (public client, no secret) | The Outlook "Connect" row is hidden; the ICS subscription is offered instead. |
| Update-signing **public** key | `NOTCHLY_UPDATE_PUBKEY` | Verifies `update.json.sig` before trusting an update | Updates are checked but never installed automatically ("This build can't verify updates"). |
| Update feed URL | `NOTCHLY_UPDATE_FEED` | Where `update.json` lives | No update checks. |
| Website / support links | `NOTCHLY_WEBSITE`, `NOTCHLY_SUPPORT_URL` | Links in Settings › About and on the crash notice | Links hidden. |
| Firebase web config | none | The website uses no Firebase SDK, only static Hosting, so there is no web config. | |

**How they get into the exe:** `build.rs` reads `notchly.local.toml` (or `config.toml`) and
passes each `NOTCHLY_*` value to `option_env!` (`src/build_info.rs`); environment
variables win. CI sets them from repository variables/secrets (`release.yml`). Copy
`notchly.local.example.toml` to start.

**OAuth flow:** installed-app flow only: the system browser, a loopback redirect to
`http://127.0.0.1:<random port>/`, PKCE (S256) and a random `state`
(`src/services/calendar.rs`). No embedded web view. Scopes: Google
`calendar.readonly`; Microsoft `Calendars.Read offline_access`.

**Use my own client ID:** Settings › Calendar & weather › "Google client ID" (+ "Google
client secret") and "Microsoft client ID". A user's ID wins over the built-in one, so forks
and power users can use their own Google Cloud / Entra app.

**Per-user tokens and keys (never in plain-text config):**
- OAuth refresh tokens: Windows Credential Manager (`Notchly/google`, `Notchly/microsoft`),
  deleted by "Disconnect".
- AI provider keys and a user's own Google client secret: encrypted with Windows DPAPI for
  the current user, in `%APPDATA%\Notchly\secrets.bin` (`src/platform/credentials.rs`).

## Tier 2: true secrets (CI only)

| Secret | GitHub secret name | Used by | Rotate |
|---|---|---|---|
| Update-signing **private** key | `NOTCHLY_UPDATE_SIGNING_KEY` | `release.yml` → `packaging/sign-update.ps1` signs `update.json` | Run `packaging/update-key.ps1`, replace the secret **and** the `NOTCHLY_UPDATE_PUBKEY` variable, release a new version. Copies built with the old public key can't verify manifests signed by the new key, so they must update by hand once: rotate only if the key leaks. |
| Code-signing credentials (optional, all empty today) | `NOTCHLY_PFX_BASE64` + `NOTCHLY_PFX_PASSWORD`, or `SIGNPATH_API_TOKEN` | `packaging/build.ps1` | Revoke at the issuer, issue new, replace the secrets. |
| `GITHUB_TOKEN` | provided by Actions | Publishing the release | Automatic per run; nothing to rotate. |
| Firebase deploy login | none (local `firebase login` on your PC) | `firebase deploy` from your PC | `firebase logout` / `firebase login`. GitHub never deploys the site. |

No runtime feature needs a true secret: there is no Notchly server.

## Tier 3: outside services

| Service | URL | Purpose | Key | Kind | Safeguards |
|---|---|---|---|---|---|
| Sunrise / sunset | none | Dashboard, weather | none | **(a) local** (NOAA algorithm, `src/features/sun.rs`, tested) | |
| Prayer times | none | Prayer page | none | **(a) local** (`src/features/prayer.rs`, 12 methods, tested) | |
| MET Norway | `api.met.no/weatherapi/locationforecast/2.0/compact` | Weather | none | (b) free | Identifying User-Agent with contact (their terms), coordinates rounded to 2 decimals, cached in `cache/weather.json`. |
| Open-Meteo geocoding | `geocoding-api.open-meteo.com/v1/search` | City search (name, coordinates, time zone) | none | (b) free | Only when the user searches. |
| LRCLIB, then lyrics.ovh | `lrclib.net/api/*`, `api.lyrics.ovh/v1/*` | Synced lyrics | none | (b) free | Cached per song. |
| Frankfurter | `api.frankfurter.app/latest` | Currency rates | none | (b) free | Cached for a day. |
| ICS subscriptions | user's URLs | Calendars without OAuth | none (the URL itself may be private) | (b) | URLs stay in the user's settings. |
| Google Calendar API, Microsoft Graph | `www.googleapis.com/calendar/v3`, `graph.microsoft.com/v1.0` | Calendar | user's OAuth token | Tier 1 client + per-user token | Read-only scopes. |
| OpenAI, Anthropic, Gemini | `api.openai.com`, `api.anthropic.com`, `generativelanguage.googleapis.com` | Ask / Translate | **user's own key** | **(c) BYOK**, off until a key is added | DPAPI-encrypted; core features never depend on it. |
| Ollama | `http://127.0.0.1:11434` (user-editable) | Local AI | none | local | |
| GitHub Releases | `github.com/…/releases/latest/download/update.json` (+`.sig`) | Update check, at most daily | none | (b) | Signature checked against the Tier 1 public key; installer SHA-256 checked. |
| Phone Drop | your LAN only | Phone ↔ PC files | one-time token in the QR code | local network | See docs/PRIVACY.md. |

Every request: HTTPS (except localhost and the LAN), `User-Agent: Notchly/<version>
(+https://github.com/icedamericano16/Notchly)`, 10-second timeout
(`src/services/http.rs`). Each network feature can be switched off in Settings › Privacy.

**Website:** a static site on Firebase Hosting (free Spark plan). Release data is fetched
once at deploy time on your PC and baked into the pages, so visitors' browsers make no API
calls.

## If a secret leaks

1. Treat it as compromised immediately, even if you deleted the commit: git history and
   forks keep it.
2. Rotate it (table above) **before** cleaning history.
3. Only then, if you want, rewrite history (`git filter-repo`) and force-push. That is a
   destructive step: do it deliberately.

The repositories are scanned with [gitleaks](https://github.com/gitleaks/gitleaks) in CI
on every push and pull request (`.github/workflows/ci.yml`).
