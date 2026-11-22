# Packaging and release

## Direct installer (GitHub Releases)
1. `cargo build --release`
2. Sign `target\release\notchly.exe` (Azure Trusted Signing or an OV certificate).
3. Optional, for notification mirroring: create `packaging\sparse\logo.png`, set the Publisher in
   `packaging\sparse\AppxManifest.xml` to your signing subject, then
   `makeappx pack /d packaging\sparse /p dist\Notchly.Sparse.msix /nv` and sign it with the same cert.
4. `ISCC packaging\inno\notchly.iss` → `dist\Notchly-<version>-setup.exe` (per-user, no admin).
5. Publish `update.json` next to the installer:
   `{"version":"0.1.1","url":"https://…/Notchly-0.1.1-setup.exe","sha256":"<hex>","notes":"…"}`

## Microsoft Store
Fill Identity/Publisher in `packaging\msix\AppxManifest.xml` from Partner Center, add the
`Assets\*.png` logos, pack with `makeappx`, and upload. Package identity enables notification
mirroring and the `notchly` execution alias for the CLI.

## CI
`.github/workflows/ci.yml` runs fmt, clippy (-D warnings), tests, release build, the 4 MB size
check, and on tags: signing, Inno installer and MSIX artifacts.
