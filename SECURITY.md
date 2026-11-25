# Security policy

## Reporting a vulnerability

Please report security problems privately, not in a public issue:

- **GitHub:** this repository › **Security** › **Report a vulnerability** (private
  vulnerability reporting).
- If that option isn't shown, open an issue titled "Security contact request" **without
  details**, and you'll be given a private way to send them.

Include what you found, the Notchly version (Settings › About) and steps to reproduce. You'll
get a reply within a week. Fixes ship as a new release; the in-app updater only installs
releases whose manifest is signed with the project's update key.

## Supported versions

Only the latest release gets fixes. The app updates itself from 0.3.1 on; 0.3.0 can't, so
install the latest version from <https://notchly.web.app/download>.

## What Notchly exposes

For reviewers, the attack surface and how each part is protected:

| Surface | Protection |
|---|---|
| Live Activities API (named pipe `notchly-live`) | Owner-only DACL, remote clients rejected, first-instance flag, 16 clients at most, size and rate limits |
| Phone Drop (LAN HTTP, on demand) | One-time 128-bit QR token, `HttpOnly`/`SameSite=Strict` session cookie, PIN fallback with a failure limit, rate limit, subnet check, size limit, Mark of the Web on received files, 5-minute idle stop. Plain HTTP: trusted Wi-Fi only |
| Calendar sign-in (loopback redirect) | `127.0.0.1` only, PKCE (S256), random `state`, 5-minute limit |
| Updates | ECDSA P-256 signed manifest, SHA-256 of the installer, HTTPS only, installed only after you confirm |
| Network requests | HTTPS only (Ollama on localhost excepted), 10 s timeout, 32 MB response limit |
| Stored secrets | API keys encrypted with DPAPI; calendar tokens in Windows Credential Manager |

Details: [`docs/KEYS_AND_SECRETS.md`](docs/KEYS_AND_SECRETS.md) and
[`docs/PRIVACY.md`](docs/PRIVACY.md).
