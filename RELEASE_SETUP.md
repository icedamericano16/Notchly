# Release setup

Everything in the code is ready: signing, the signed update feed, Google and Microsoft sign-in
and the release workflow. What's left needs your accounts. Do the sections in order; each
ends with the exact names to paste into GitHub or `config.toml`.

**Where values go.** GitHub: your repository › **Settings** › **Secrets and variables** ›
**Actions**. Use **New repository secret** for anything marked *secret* (hidden, never
printed in logs). Use the **Variables** tab › **New repository variable** for anything marked
*variable*. On your own PC: copy `config.example.toml` to `config.toml` (build settings) and
`.env.example` to `.env` (signing). Both real files are git-ignored. Never commit them.

---

## 1. Code-signing certificate

Without one, releases still build, but they are unsigned. Windows SmartScreen warns
("Windows protected your PC") and notification mirroring needs a signed build. Pick **one**
option.

| Option | Cost | Good for |
|---|---|---|
| A. Azure Trusted Signing | about US$10 a month | individuals and companies where Microsoft offers it (check eligibility when you sign up) |
| B. SignPath Foundation | free | open-source projects (they review the project) |
| C. A .pfx certificate | varies | a certificate you can export as .pfx (new public certificates usually can't be exported; they live in a hardware token or cloud HSM, so use A or B instead) |

### A. Azure Trusted Signing
1. Go to <https://portal.azure.com> and sign in or create an account. Search for **Trusted
   Signing Accounts** › **Create**. Choose a resource group, a name and a region, then
   **Review + create**.
2. Open the account › **Identity validations** › **New identity** › **Public**, and complete
   the validation. This can take a few days.
3. Open **Certificate profiles** › **Create** › **Public Trust**, and pick the validated
   identity. Note the profile name.
4. Open **Microsoft Entra ID** › **App registrations** › **New registration** (name it e.g.
   "notchly-signing") › **Register**. Under **Certificates & secrets** › **New client
   secret**, copy the *Value*.
5. Go back to the Trusted Signing account › **Access control (IAM)** › **Add role
   assignment** › **Trusted Signing Certificate Profile Signer**, and assign it to the app
   from step 4.
6. Add these to GitHub:
   - *secrets*: `AZURE_TENANT_ID` (Entra › Overview › Tenant ID), `AZURE_CLIENT_ID` (the
     app's Application ID), `AZURE_CLIENT_SECRET` (step 4).
   - *secrets*: `TRUSTED_SIGNING_ENDPOINT` (e.g. `https://eus.codesigning.azure.net` for East
     US; it's on the account's Overview), `TRUSTED_SIGNING_ACCOUNT`, `TRUSTED_SIGNING_PROFILE`.
   - *variable* `NOTCHLY_PUBLISHER`: the certificate subject exactly as it appears on the
     certificate profile, e.g. `CN=Your Name, O=Your Name, L=Kathmandu, C=NP`.

### B. SignPath (free for open source)
1. Apply at <https://signpath.org> (SignPath Foundation) with the repository URL. Once you're
   accepted, open **Projects** and create the project `notchly` with an artifact
   configuration for a *single PE file*, and add a signing policy (e.g. `release-signing`).
2. Under **Users**, create a CI user and copy its API token.
3. Add to GitHub: *secret* `SIGNPATH_API_TOKEN`; *variables* `SIGNPATH_ORGANIZATION_ID`,
   `SIGNPATH_PROJECT_SLUG`, `SIGNPATH_POLICY_SLUG`, and `NOTCHLY_PUBLISHER` (the certificate
   subject SignPath shows).

### C. A .pfx file
1. Export the certificate with its private key as `notchly.pfx`, with a password.
2. In Windows PowerShell, turn it into one line of text:
   `[Convert]::ToBase64String([IO.File]::ReadAllBytes("C:\keys\notchly.pfx")) | Set-Clipboard`
3. Add to GitHub: *secret* `NOTCHLY_PFX_BASE64` (paste) and *secret* `NOTCHLY_PFX_PASSWORD`.
   The publisher is read from the certificate automatically.
4. Optional *variable* `NOTCHLY_TIMESTAMP_URL`, if your CA asks for its own RFC 3161 server
   (DigiCert's is the default).

Every signature is timestamped (RFC 3161), so it stays valid after the certificate expires.
The build checks each signature with `signtool verify /pa`.

---

## 2. Update signing key (required for auto-update)

The app installs an update only if `update.json` is signed with your key.
1. On your PC, in the repository folder: `powershell -File packaging\update-key.ps1`
2. It prints two lines. Add them to GitHub:
   - *secret* `NOTCHLY_UPDATE_SIGNING_KEY`: the first value (PRIVATE). Also keep a copy in a
     password manager: if you lose it, already installed copies can't verify updates from a
     new key.
   - *variable* `NOTCHLY_UPDATE_PUBKEY`: the second value (public). It's built into the app.
3. Nothing else is needed: the release workflow points the app at
   `https://github.com/<you>/<repo>/releases/latest/download/update.json`. To host the feed
   elsewhere, set the *variable* `NOTCHLY_UPDATE_FEED`.

---

## 3. Privacy policy and homepage (for Google verification)

1. In GitHub, go to repository › **Settings** › **Pages** › **Build and deployment** ›
   **Deploy from a branch** › branch `main`, folder `/docs` › **Save**.
2. A minute later the policy is at `https://<you>.github.io/<repo>/privacy.html`. That is
   your **privacy policy URL**. Use your repository page or your own site as the
   **homepage**.
3. Edited `docs/PRIVACY.md`? Run `python packaging\privacy_html.py` and commit both files.
4. Google must be able to confirm you own the domain. Go to <https://search.google.com/search-console>
   › **Add property** › **URL prefix** › `https://<you>.github.io/` (or your own domain).
   Download the HTML verification file, put it in `/docs` (or your site root), push, then
   click **Verify**. A domain of your own is the smoothest route.

---

## 4. Google Calendar sign-in

1. Go to <https://console.cloud.google.com> › project picker › **New project** (e.g.
   "Notchly") › **Create**.
2. Go to **APIs & Services** › **Library** › search **Google Calendar API** › **Enable**.
3. Go to **Google Auth Platform** (formerly *OAuth consent screen*) › **Get started**:
   - **App information**: app name `Notchly`; user support email.
   - **Audience**: **External**.
   - **Contact information**: your email › **Create**.
4. On the **Branding** page: upload the logo (120 × 120 px, from `packaging/msix/Assets`).
   Set the **Application home page**, the **Application privacy policy link** (section 3) and
   **Authorised domains** (e.g. `<you>.github.io`) › **Save**.
5. On the **Data access** page: **Add or remove scopes** ›
   `https://www.googleapis.com/auth/calendar.readonly` › **Update** › **Save**.
6. On the **Audience** page, under **Test users**, use **Add users** for each Google
   account that should sign in before verification (up to 100). While the app is in
   *Testing*, tokens expire after 7 days, so you'd reconnect weekly.
7. On the **Clients** page: **Create client** › Application type **Desktop app** › name
   `Notchly desktop` › **Create**. Copy the **Client ID** and **Client secret** (Google
   doesn't treat a desktop app's secret as confidential, but keep it out of the repo anyway).
8. Put them in:
   - your PC: `config.toml` › `NOTCHLY_GOOGLE_CLIENT_ID`, `NOTCHLY_GOOGLE_CLIENT_SECRET`;
   - GitHub: *variable* `NOTCHLY_GOOGLE_CLIENT_ID`, *secret* `NOTCHLY_GOOGLE_CLIENT_SECRET`.
9. **Verification** (needed before the public can sign in without the "unverified app"
   warning, and to lift the 100-user cap): **Audience** › **Publish app**, then
   **Verification centre** › **Prepare for verification**. You'll need:
   - the homepage, privacy policy and verified domain (section 3);
   - a **scope justification**, e.g.: *"Notchly is a Windows desktop utility that shows the
     user's next calendar event in a notch at the top of their screen and reminds them before
     it starts. It reads events (title, time, location, meeting link) with calendar.readonly.
     Data is fetched directly to the user's PC, shown only to that user, never stored on any
     server, never shared, and never used for advertising or AI training."*;
   - a **demo video** (unlisted YouTube) showing:
     1. Settings › Calendar & weather › Google Calendar › Connect;
     2. the browser consent screen, with the client ID visible in the address bar;
     3. allowing access;
     4. the next event appearing in the notch;
     5. Disconnect.
   Review usually takes a few days to a few weeks. Google may email questions.

---

## 5. Outlook / Microsoft 365 sign-in

1. Go to <https://entra.microsoft.com> (any Microsoft account works) › **Applications** ›
   **App registrations** › **New registration**:
   - Name `Notchly`.
   - **Supported account types**: *Accounts in any organizational directory and personal
     Microsoft accounts* (this is what lets the app use the `common` endpoint).
   - **Redirect URI**: platform **Public client/native (mobile & desktop)**, value
     `http://127.0.0.1/` › **Register**.
2. Go to **Authentication** › **Add a platform** / **Add URI**, and also add
   `http://localhost/`. Loopback redirects ignore the port, which Notchly picks at sign-in.
   Leave *Allow public client flows* off (not needed for PKCE) › **Save**.
3. Go to **API permissions** › **Add a permission** › **Microsoft Graph** › **Delegated
   permissions** › tick **Calendars.Read** and **offline_access** › **Add permissions**. No
   admin consent is needed for these.
4. Go to **Overview** and copy the **Application (client) ID**. There's no secret: Notchly
   is a public client using PKCE.
5. Put it in:
   - your PC: `config.toml` › `NOTCHLY_MS_CLIENT_ID`;
   - GitHub: *variable* `NOTCHLY_MS_CLIENT_ID`.
6. Optional: **Branding & properties** › **Publisher verification** (needs a Microsoft
   AI Cloud Partner Program ID). It removes the "unverified" label on the consent screen.

---

## 6. Other variables (optional)

| GitHub variable | Value |
|---|---|
| `NOTCHLY_WEBSITE` | the homepage, shown in Settings › About |
| `NOTCHLY_SUPPORT_URL` | e.g. `https://github.com/<you>/<repo>/issues` |
| `NOTCHLY_STORE_PUBLISHER`, `NOTCHLY_STORE_IDENTITY` | Microsoft Store only: Partner Center › your app › **Product identity** (`Package/Identity/Publisher` and `Package/Identity/Name`). The workflow then also builds a Store `.msix`, which you upload in Partner Center. The Store signs it. |

---

## 7. Cutting a release

1. Update `version` in `Cargo.toml` and add a `## x.y.z — date` section to `CHANGELOG.md`.
2. Commit, then tag and push:
   `git tag v0.3.0` then `git push origin main v0.3.0`.
3. **Actions** › **Release** runs, which:
   1. checks the tag against `Cargo.toml` and runs the tests;
   2. builds and signs the exe, installer and packages;
   3. signs `update.json`;
   4. publishes the GitHub Release with that version's changelog section as notes.
4. Check the release has `Notchly-x.y.z-setup.exe`, `update.json` **and**
   `update.json.sig`. A missing `.sig` means `NOTCHLY_UPDATE_SIGNING_KEY` isn't set.
5. Run the manual checks in `TESTING.md`, including "update from an older build".
