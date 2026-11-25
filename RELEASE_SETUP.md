# Release setup (zero-cost path)

Everything here is free and needs no credit or debit card. The code is ready: the release
workflow, the signed update feed, Google sign-in, the ICS calendar option and the winget
manifest. What's left needs your accounts; `HUMAN_STEPS.md` is the short ordered checklist,
this page explains each step. Which values are public and which are secret:
`docs/KEYS_AND_SECRETS.md`.

**How Notchly is distributed**

| Channel | Signed? | Updates | Notifications mirroring | Cost |
|---|---|---|---|---|
| **Microsoft Store** (primary) | Yes, by Microsoft | Automatic, by the Store | Yes (package identity) | Free individual developer account |
| GitHub Releases installer (`Notchly-x.y.z-setup.exe`) | **No**: labelled "advanced / unsigned" | The app's own signed update feed | No (needs a signed build) | Free |
| GitHub portable exe (`notchly.exe`) | **No**: "advanced / portable, unsigned" | None | No | Free |
| winget (`winget install icedamericano16.Notchly`) | Uses the GitHub installer | `winget upgrade` | No | Free (a pull request) |

**Where values go.** GitHub: repository › **Settings** › **Secrets and variables** ›
**Actions**. *Secret* = hidden (Tier 2, or a Tier 1 value you'd rather not show).
*Variable* = visible. Never commit either. On your PC: copy `notchly.local.example.toml`
to `notchly.local.toml` (git-ignored).

---

## 1. Microsoft Store (primary channel)

The Store signs the package with Microsoft's certificate (no SmartScreen warning), hosts it,
updates it automatically and gives it package identity, which notification mirroring needs.

1. Register at <https://storedeveloper.microsoft.com> as an **individual** developer.
   Microsoft made individual registration free; **if the sign-up page asks for a card or a
   fee, stop**: that would break the zero-cost rule. Identity verification uses a
   government ID and a selfie.
2. Partner Center › **Apps and games** › **New product** › **MSIX or PWA app** › reserve the
   name **Notchly** (or the closest free name).
3. Open the app › **Product management** › **Product identity**. Copy:
   - `Package/Identity/Name` → GitHub *variable* `NOTCHLY_STORE_IDENTITY`
   - `Package/Identity/Publisher` → GitHub *variable* `NOTCHLY_STORE_PUBLISHER`
4. Push a release tag (section 7). The workflow's **artifact**
   `notchly-vX.Y.Z-packages` contains `Notchly-X.Y.Z.msix`.
5. Partner Center › **Start your submission**:
   - **Packages**: upload the `.msix`.
   - **Properties**: category Utilities & tools; privacy policy URL
     `https://notchly.web.app/privacy`; website `https://notchly.web.app`.
   - **Age ratings**: the questionnaire (no user-generated content, no purchases).
   - **Store listing**: description and screenshots (`docs/STORE_LISTING.md` has the text;
     take screenshots of the real app).
   - **Restricted capability `runFullTrust`**: justify it as "a desktop app (Win32) packaged
     as MSIX; it draws the notch with Direct2D and reads media, audio and system state."
     `userNotificationListener` is used to mirror notifications the user chooses to see.
   - Submit. Certification usually takes 1 to 3 business days.
6. When it's live, copy the Store URL (`https://apps.microsoft.com/detail/<StoreId>`) into
   the GitHub *variable* `NOTCHLY_STORE_URL`. The next release's notes then lead with it,
   and the website's main download button points there (see the website README).

Store copies never run Notchly's own updater: the Store updates them.

---

## 2. Update signing key (for the GitHub installer)

The installer build installs an update only if `update.json` is signed with your key.

1. On your PC, in the repository folder: `powershell -File packaging\update-key.ps1`
2. It prints two lines:
   - *secret* `NOTCHLY_UPDATE_SIGNING_KEY`: the first value (PRIVATE). Also keep a copy in a
     password manager: if you lose it, already installed copies can't verify updates from a
     new key.
   - *variable* `NOTCHLY_UPDATE_PUBKEY`: the second value (public). It's built into the app.
3. Nothing else: the workflow points the app at
   `https://github.com/<you>/<repo>/releases/latest/download/update.json`, and **fails the
   release** if the public key is set but the manifest isn't signed, if the two keys don't
   belong together, or if `update.json` names another version or download URL than the tag.

---

## 3. Privacy policy and homepage (for Google verification)

The website is static on Firebase Hosting's free Spark plan. The **full** privacy policy is
at `https://notchly.web.app/privacy`; the homepage is `https://notchly.web.app`.

To prove you own the domain (Google verification needs it):
1. <https://search.google.com/search-console> › **Add property** › **URL prefix** ›
   `https://notchly.web.app/`.
2. Choose **HTML file**, download it, put it in the website repo's `public/` folder, deploy
   (`firebase deploy --only hosting`), then click **Verify**.

---

## 4. Google Calendar sign-in (optional; ICS works without it)

1. <https://console.cloud.google.com> › **New project** (e.g. "Notchly") › **Create**.
2. **APIs & Services** › **Library** › **Google Calendar API** › **Enable**.
3. **Google Auth Platform** › **Get started**: app name `Notchly`, support email, audience
   **External**, contact email.
4. **Branding**: logo (120 × 120 px, `packaging/msix/Assets`), homepage
   `https://notchly.web.app`, privacy policy `https://notchly.web.app/privacy`, authorised
   domain `notchly.web.app`.
5. **Data access** › add only `https://www.googleapis.com/auth/calendar.calendarlist.readonly`
   and `https://www.googleapis.com/auth/calendar.events.readonly`.
6. **Clients** › **Create client** › type **Desktop app**. Copy the client ID and secret:
   - your PC: `notchly.local.toml` › `NOTCHLY_GOOGLE_CLIENT_ID`, `NOTCHLY_GOOGLE_CLIENT_SECRET`;
   - GitHub: *variable* `NOTCHLY_GOOGLE_CLIENT_ID`, *secret* `NOTCHLY_GOOGLE_CLIENT_SECRET`.
7. **Verification** (lifts the 100-test-user limit and the "unverified app" warning; tokens
   of apps in *Testing* expire after 7 days): **Audience** › **Publish app**, then
   **Verification centre**. You need the homepage, privacy policy and verified domain
   (section 3), a scope justification and a demo video:
   - *Justification*: "Notchly is a Windows desktop utility that shows the user's next
     calendar event in a notch at the top of their screen and reminds them before it starts.
     It reads events (title, time, location, meeting link) with calendar.calendarlist.readonly and calendar.events.readonly. Data is
     fetched directly to the user's PC, shown only to that user, never stored on any
     server, never shared, and never used for advertising or AI training."
   - *Video* (unlisted YouTube): Settings › Calendar & weather › Google Calendar ›
     Connect; the consent screen with the client ID visible in the address bar; Allow; the
     next event in the notch; Disconnect.

Forks and power users can use their own client ID instead (Settings › Calendar & weather).

---

## 5. Outlook / Microsoft 365 (optional)

**Default, no setup:** users add their Outlook calendar as an **ICS subscription**:
Outlook.com › Settings › Calendar › **Shared calendars** › **Publish a calendar** › choose
the calendar and "Can view all details" › copy the **ICS** link › paste it in Notchly ›
Settings › Calendar & weather › ICS URLs. Same for Google Calendar's "Secret address in
iCal format". No keys, no OAuth.

**One-click sign-in** (only if you want it): Microsoft Entra registration is free with any
Microsoft account.
1. <https://entra.microsoft.com> › **App registrations** › **New registration**: name
   `Notchly`; *Accounts in any organizational directory and personal Microsoft accounts*;
   redirect URI platform **Public client/native**, `http://127.0.0.1/`, then also add
   `http://localhost/`.
2. **API permissions** › Microsoft Graph › Delegated › `Calendars.Read`, `offline_access`.
3. Copy the **Application (client) ID** → GitHub *variable* `NOTCHLY_MS_CLIENT_ID` (and
   `notchly.local.toml`). There's no secret: it's a public client using PKCE.

Without a client ID (built in or the user's own), the Outlook "Connect" row is hidden and
the ICS option is offered instead: no dead buttons.

---

## 6. winget (free)

Every release workflow run produces a winget manifest in the artifact
`notchly-vX.Y.Z-packages` (`winget/icedamericano16.Notchly/X.Y.Z/*.yaml`).

1. Fork <https://github.com/microsoft/winget-pkgs>.
2. Copy the three YAML files to `manifests/i/icedamericano16/Notchly/X.Y.Z/` in your fork.
3. Open a pull request. The winget bots validate the manifest and scan the installer;
   answer any review comments. After it merges: `winget install icedamericano16.Notchly`.
   (Alternatively `winget install Microsoft.WingetCreate`, then
   `wingetcreate submit <folder>` does the fork and PR for you.)

Later versions: same steps with the new folder (or `wingetcreate update`).

### Scoop (free)

The same artifact has `scoop/notchly.json`: the portable `notchly.exe`, a Start menu
shortcut, and `checkver`/`autoupdate`, so a bucket updates it without you. A Scoop install
turns off Notchly's own updater ("Updates come from Scoop").

- **Your own bucket (no review):** create a public repo, e.g. `scoop-notchly`, put
  `notchly.json` in `bucket/`. Users run `scoop bucket add notchly
  https://github.com/icedamericano16/scoop-notchly` then `scoop install notchly`.
- **Extras bucket (wider reach):** open a pull request adding `bucket/notchly.json` to
  <https://github.com/ScoopInstaller/Extras> and follow its review checklist.

---

## 7. Optional: code signing for the GitHub downloads

The Store package is already signed by Microsoft. The GitHub installer and exe stay
unsigned unless you add one of these:

- **SignPath Foundation** (free): it signs only projects under an **OSI-approved open
  source licence**. Notchly's `LICENSE` is proprietary freeware, so it does **not**
  qualify today. If you ever relicense (e.g. MIT, Apache-2.0, GPL), apply at
  <https://signpath.org> with the repository URL; requirements: OSI licence, public
  repository, builds from CI, a release history, and a project description. After
  acceptance: *secret* `SIGNPATH_API_TOKEN`; *variables* `SIGNPATH_ORGANIZATION_ID`,
  `SIGNPATH_PROJECT_SLUG`, `SIGNPATH_POLICY_SLUG`, `NOTCHLY_PUBLISHER` (the certificate
  subject), and finally `NOTCHLY_SIGNPATH_ENABLED` = `true`. The step is disabled until then.
- **A .pfx certificate** you already have: *secrets* `NOTCHLY_PFX_BASE64` and
  `NOTCHLY_PFX_PASSWORD` (publisher read from the certificate).

Every signature is timestamped (RFC 3161) and checked with `signtool verify /pa`.

---

## 8. Cutting a release

1. Update `version` in `Cargo.toml` and add a `## x.y.z - date` section to `CHANGELOG.md`.
2. Commit, then tag and push: `git tag vX.Y.Z` then `git push origin main vX.Y.Z`.
3. **Actions** › **Release** runs. It:
   1. checks the tag against `Cargo.toml` and `CHANGELOG.md` (fails if they disagree);
   2. runs the tests;
   3. builds the exe, the installer, the Store MSIX (if the Store variables exist) and
      `update.json` (signed when the key secret exists);
   4. checks `update.json` against the tag, the installer's SHA-256 and the keys;
   5. writes `SHA256SUMS.txt`, the winget manifest, and release notes containing **only**
      this version's changelog section plus a labelled download list;
   6. publishes the GitHub Release (marked "latest") and uploads the MSIX and winget
      manifest as a workflow artifact.
4. Check the release has `Notchly-x.y.z-setup.exe`, `notchly.exe`, `update.json`,
   `update.json.sig` (if you set up section 2) and `SHA256SUMS.txt`.
5. Upload the MSIX to Partner Center (section 1) and open the winget PR (section 6).
6. Redeploy the website so it shows the new release (website repo:
   `firebase deploy --only hosting`).
7. Run the manual checks in `TESTING.md`.

**Never create a release by hand with a different tag name** (e.g. "Notchly"): GitHub
then treats it as "latest", the website shows it, and `releases/latest/download/update.json`
breaks for every installed copy.

## All GitHub variables and secrets

| Name | Kind | Section | Needed for |
|---|---|---|---|
| `NOTCHLY_UPDATE_SIGNING_KEY` | secret | 2 | signed update feed |
| `NOTCHLY_UPDATE_PUBKEY` | variable | 2 | signed update feed |
| `NOTCHLY_STORE_IDENTITY`, `NOTCHLY_STORE_PUBLISHER` | variables | 1 | Store MSIX |
| `NOTCHLY_STORE_URL` | variable | 1 | Store link in release notes |
| `NOTCHLY_GOOGLE_CLIENT_ID` | variable | 4 | one-click Google sign-in |
| `NOTCHLY_GOOGLE_CLIENT_SECRET` | secret | 4 | one-click Google sign-in |
| `NOTCHLY_MS_CLIENT_ID` | variable | 5 | one-click Outlook sign-in (optional) |
| `NOTCHLY_WEBSITE`, `NOTCHLY_SUPPORT_URL` | variables | | defaults: notchly.web.app, this repo's Issues |
| `NOTCHLY_SIGNPATH_ENABLED`, `SIGNPATH_*`, `NOTCHLY_PUBLISHER` | variables + secret | 7 | optional signing |
| `NOTCHLY_PFX_BASE64`, `NOTCHLY_PFX_PASSWORD` | secrets | 7 | optional signing |
