# Notchly

A living Dynamic Island for Windows 10/11. The top-centre of your screen becomes a
true-black notch that shows what matters right now — music with synced lyrics, timers,
calls, downloads, system changes, notifications — and opens into a small control centre
when you hover it.

**Notchly is completely free.** Every feature (Ask with local AI, translate, weather,
calendar, widgets, glass and floating styles, themes, Live Activities API, privacy dots,
meeting mode, visualiser, Phone Drop, screenshots, system monitor, notes…) is included,
with no licence, account or telemetry.

- Native Rust + Direct2D/DirectWrite on a DirectComposition swap chain (no web runtime)
- ~2.5 MB exe, 0 % CPU when idle, spring physics at the display's refresh rate
- Private by default: everything runs locally; every network feature is listed and toggleable

## Build

Requirements: Rust stable (MSVC toolchain), Windows 10 SDK, Visual Studio 2022 Build Tools
("Desktop development with C++").

```powershell
cargo build --release          # target\release\notchly.exe
cargo test                     # 99 unit tests
cargo clippy --all-targets -- -D warnings
```

`.cargo/config.toml` pins the linker to VS 2022 Build Tools because the newest Visual Studio
on the development PC lacked the desktop x64 libraries. Delete it on a machine with a complete
C++ workload (CI deletes it automatically).

Releases (signed installer, Store MSIX, update feed) are built with `packaging\build.ps1`;
see [docs/PACKAGING.md](docs/PACKAGING.md). Optional build-time settings such as the update feed
and calendar sign-in clients are listed in `src/build_info.rs`. Licence: freeware, see
[LICENSE](LICENSE) and [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## Run

```powershell
notchly                 # start (single instance; a second launch opens the notch)
notchly --settings      # open Settings
notchly --demo          # cycle through every state and pop-up (visual QA / screenshots)
notchly --perf          # print frames, CPU per frame, process CPU and RAM every 5 s
notchly live --help     # Live Activities CLI
```

Set `NOTCHLY_TRACE=1` to log per-frame view/geometry and fullscreen-detection decisions.

## Live Activities

Any app or script can post to the notch through the per-user named pipe
`\\.\pipe\notchly-live` (newline-delimited JSON), the `notchly live` CLI, or the tiny SDKs:

```powershell
notchly live start --id build-42 --title Build --subtitle compiling --icon hammer --progress 0.4 --action cancel:Cancel
notchly live update --id build-42 --progress 0.8
notchly live end --id build-42 --final "Done in 2m 14s"
```

- `sdk/python/notchly.py`, `sdk/node/notchly.js`
- `sdk/examples/`: build wrapper, yt-dlp hook, OBS recording script, terminal Pomodoro

The first message from a new executable asks you to Allow/Deny it. Limits: 8 activities,
10 updates/s each, icons ≤ 64 KB. Payloads are validated and never executed.

## Layout

```
src/main.rs        single instance, windows, message loop, frame scheduler, platform requests
src/app/           State, Activity Manager + pop-up queue, island choreography, input, events
src/motion/        springs (semi-implicit Euler, retargetable) and choreography constants
src/render/        D3D11/DXGI/DComp/D2D device + surfaces, canvas, notch/squircle shapes, icons, text
src/theme/         tokens, presets, theme packs, album-art adaptive colour
src/ui/            island shell, compact/live/peek views, dashboard and every page, widgets
src/services/      media, lyrics, audio, network, bluetooth, keyboard hook, privacy, AI, weather,
                   calendar, OCR, downloads, sysmon, visualiser, Live API, Phone Drop, …
src/features/      timer, water, to-do (+ NL time), shelf/clipboard/notes, prayer times,
                   maths/units/currency, Bikram Sambat
src/platform/      tray, hotkeys, clipboard, drag & drop, capture overlay, DPAPI/CredMan, UI Automation
src/settings/      versioned TOML model, store + migrations, Settings window
packaging/         manifest, MSIX, sparse package, Inno Setup script
```

Docs: [User guide](docs/USER_GUIDE.md) · [Privacy policy](docs/PRIVACY.md) ·
[Packaging](docs/PACKAGING.md) · [Store listing](docs/STORE_LISTING.md) ·
[Decisions](DECISIONS.md) · [Changelog](CHANGELOG.md)
