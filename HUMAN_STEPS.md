# Human steps

Things only you can do (accounts, identity checks, destructive actions on GitHub). Do them
in this order. Each links to the detailed instructions in `RELEASE_SETUP.md`. Nothing here
costs money; if any page asks for a card, stop.

## 1. Release 0.3.1: done (2026-11-25)

- [x] v0.3.1 is published and marked **latest** (installer, portable exe, `SHA256SUMS.txt`,
      signed `update.json`). Checked: the update feed's signature verifies against the
      public key, the installer's SHA-256 matches the feed, and the released `notchly.exe`
      has the update key built in, so it can update itself from here on.
- [x] The website shows 0.3.1 (redeployed after the release).
- [ ] **Copies installed from v0.3.0 can't update themselves** (that build has no update
      key): install 0.3.1 once from the website. Later versions then arrive through the app.
- [ ] Optional cleanup (destructive, do it yourself if you want): delete the release
      **Notchly 0.3.0** (tag `Notchly`) on github.com, then the tag
      (`git push origin :refs/tags/Notchly` and `git tag -d Notchly`). It is no longer
      "latest" and the website shows each version once, so this is only tidiness.

Every later release: RELEASE_SETUP.md §8 (date the CHANGELOG, tag, then redeploy the
website).

## 2. Microsoft Store account and app reservation (RELEASE_SETUP.md §1)

- [ ] Register as an **individual** developer at <https://storedeveloper.microsoft.com>
      (government ID + selfie verification). Confirm no fee or card is requested.
- [ ] Reserve the name **Notchly**.
- [ ] Copy Product identity › `Package/Identity/Name` and `Package/Identity/Publisher`.

## 3. GitHub secrets and variables

Repository › Settings › Secrets and variables › Actions.

- [x] Update key pair: created 2026-11-25. The *secret* `NOTCHLY_UPDATE_SIGNING_KEY` and
      *variable* `NOTCHLY_UPDATE_PUBKEY` are set on GitHub.
  - [ ] **Copy `Documents\Notchly-update-signing-key.txt` into a password manager, then
        delete the file.** If the private key is lost, installed copies can't verify updates.
        (A new pair later means shipping one release that people install by hand.)
- [ ] *variable* `NOTCHLY_STORE_IDENTITY` = `Package/Identity/Name`
- [ ] *variable* `NOTCHLY_STORE_PUBLISHER` = `Package/Identity/Publisher`
- [ ] *variable* `NOTCHLY_GOOGLE_CLIENT_ID` and *secret* `NOTCHLY_GOOGLE_CLIENT_SECRET`
      (the values already in your `notchly.local.toml`)
- [ ] Optional: *variable* `NOTCHLY_MS_CLIENT_ID` (only if you register an Entra app, §5)

Then push the release tag (step 1).

## 4. Store submission (RELEASE_SETUP.md §1)

- [ ] From the release run: Actions › Release › artifact
      `notchly-v0.3.1-packages` › `Notchly-0.3.1.msix`.
- [ ] Partner Center submission: package, properties, age rating, listing and screenshots,
      `runFullTrust` justification. Submit.
- [ ] When live: *variable* `NOTCHLY_STORE_URL` = the Store link (release notes link it).
- [ ] Website: deploy with `VITE_STORE_URL=<the Store link>` (in `notchly-website/.env.local`,
      then `firebase deploy --only hosting`). The Store becomes the main download there.
- [ ] Install the Store version and check notification mirroring (TESTING.md §20).

## 5. Website support contact

- [ ] Choose a support email (Google verification needs one; it will be public). Set
      `VITE_SUPPORT_EMAIL` in `notchly-website/.env.local` and redeploy: the footer gains
      "Email support". If you also want it in the policy, add it to "Changes and contact" in
      `docs/PRIVACY.md` (the website shows that file). Until then, GitHub Issues is the contact.
- [ ] Optional: enable **Discussions** (repo › Settings › General › Features).

## 6. Google OAuth consent screen and verification (RELEASE_SETUP.md §3–4)

- [ ] Search Console: verify `https://notchly.web.app/` with the HTML-file method (put the
      file in the website's `public/`, deploy, Verify).
- [ ] Google Auth Platform › Branding: homepage `https://notchly.web.app`, privacy policy
      `https://notchly.web.app/privacy`, authorised domain `notchly.web.app`.
- [ ] Data access: only `calendar.calendarlist.readonly` and `calendar.events.readonly`.
- [ ] Record the demo video (unlisted YouTube) and submit for verification.

## 7. winget pull request (RELEASE_SETUP.md §6)

- [ ] After the v0.3.1 release exists: take `winget/` from the release artifact, put the
      three files under `manifests/i/icedamericano16/Notchly/0.3.1/` in a fork of
      `microsoft/winget-pkgs`, open the PR (or `wingetcreate submit`).
- [ ] Optional, Scoop: put `scoop/notchly.json` from the same artifact in your own bucket
      repo or open a PR to ScoopInstaller/Extras (RELEASE_SETUP.md §6, Scoop).

## 8. Optional: SignPath (RELEASE_SETUP.md §7)

- [ ] Only if you decide to relicense Notchly under an OSI-approved licence. Then apply at
      <https://signpath.org> and set `NOTCHLY_SIGNPATH_ENABLED=true` after acceptance.

## 9. Local cleanup (your call)

- [ ] Delete the empty `C:\Users\khagendra\Desktop\Notchy\website` folder (it is not in
      git; Windows refused to delete it while something had it open).

## 10. Before announcing

- [ ] Work through `TESTING.md` on a real PC: every row marked "needs manual verification".
