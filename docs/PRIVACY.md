# Privacy policy

Notchly runs on your PC. It has no account, no telemetry and no analytics. Nothing you copy,
type, hear or see is sent anywhere unless you use a feature that obviously needs the network,
and every such feature can be switched off in **Settings › Privacy**.

## Network connections (complete list)

| Feature | Endpoint | What is sent |
|---|---|---|
| Synced lyrics | lrclib.net | song title, artist, album, duration |
| Currency rates | api.frankfurter.app | nothing (daily rate request) |
| Weather | api.met.no | coordinates rounded to 2 decimals |
| City search | geocoding-api.open-meteo.com | the city name you type |
| Calendar | Google Calendar API, Microsoft Graph, your ICS URLs | OAuth token / request for your events |
| Ask / Translate | your chosen provider (Ollama stays on this PC; Gemini, OpenAI or Anthropic only with your key) | your prompt and any context you attach |
| Updates | notchly.app/update.json | nothing (version check, at most daily) |
| Phone Drop | your local network only | files you choose to send/receive |

Every request uses HTTPS (except Ollama on localhost and Phone Drop on your LAN), identifies
itself as `Notchly/<version>`, and times out after 10 seconds.

## Data stored on your PC

Under `%APPDATA%\Notchly`: settings, timer/water state, to-dos, notes, shelf references, clipboard
history (with exclusions for password managers and apps you list), privacy-dot history (7 days),
caches (lyrics, weather, rates) and logs without personal content. API keys are encrypted with
Windows DPAPI (`secrets.bin`); calendar refresh tokens live in Windows Credential Manager.
Uninstalling offers to delete all of it.

## Permissions

The keyboard hook only reacts to Caps/Num Lock and language-switch keys and never records
keystrokes. Location is requested only when you ask for it. Phone Drop is LAN-only, PIN-protected
and stops after 5 minutes idle; it uses plain HTTP inside your network.
