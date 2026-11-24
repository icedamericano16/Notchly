# Human steps

Things only you can do (accounts, identity checks, destructive actions on GitHub). Do them
in this order. Each links to the detailed instructions in `RELEASE_SETUP.md`. Nothing here
costs money; if any page asks for a card, stop.

## 1. Fix the duplicate GitHub release (destructive: do it yourself)

GitHub has two releases for 0.3.0:
- `v0.3.0`: built by the release workflow (correct).
- `Notchly`: created by hand later, holding the same installer. Because it is newer,
  GitHub marks it **latest**, so the website shows it and
  `releases/latest/download/update.json` returns 404 for every installed copy (that
  release has no `update.json`).

Steps (web: github.com/icedamericano16/Notchly › Releases):
1. Open the release **Notchly 0.3.0** (tag `Notchly`) › **Delete** (trash icon).
2. Delete its tag: **Tags** › `Notchly` › **Delete tag**, or locally:
   `git push origin :refs/tags/Notchly` and `git tag -d Notchly`.
3. Re-release 0.3.0 with the new pipeline, so it has `SHA256SUMS.txt`, labelled downloads,
   the Store MSIX and (after step 3 below) a signed `update.json`:
   1. Delete the **v0.3.0** release (web), then its tag:
      `git push origin :refs/tags/v0.3.0` and `git tag -d v0.3.0`.
   2. When steps 2 and 3 below are done: `git tag v0.3.0` and `git push origin v0.3.0`.
      Actions › Release builds and publishes it.

   *Alternative that moves no tags:* keep `v0.3.0` as it is and release the fixes as
   `v0.3.1` (bump `Cargo.toml`, add a `## 0.3.1` changelog section, tag `v0.3.1`).

## 2. Microsoft Store account and app reservation (RELEASE_SETUP.md §1)

- [ ] Register as an **individual** developer at <https://storedeveloper.microsoft.com>
      (government ID + selfie verification). Confirm no fee or card is requested.
- [ ] Reserve the name **Notchly**.
- [ ] Copy Product identity › `Package/Identity/Name` and `Package/Identity/Publisher`.

## 3. GitHub secrets and variables

Repository › Settings › Secrets and variables › Actions.

- [ ] Run `powershell -File packaging\update-key.ps1` on your PC, then add:
  - [ ] *secret* `NOTCHLY_UPDATE_SIGNING_KEY` (first line; also save it in a password manager)
  - [ ] *variable* `NOTCHLY_UPDATE_PUBKEY` (second line)
- [ ] *variable* `NOTCHLY_STORE_IDENTITY` = `Package/Identity/Name`
- [ ] *variable* `NOTCHLY_STORE_PUBLISHER` = `Package/Identity/Publisher`
- [ ] *variable* `NOTCHLY_GOOGLE_CLIENT_ID` and *secret* `NOTCHLY_GOOGLE_CLIENT_SECRET`
      (the values already in your `notchly.local.toml`)
- [ ] Optional: *variable* `NOTCHLY_MS_CLIENT_ID` (only if you register an Entra app, §5)

Then push the release tag (step 1.3).

## 4. Store submission (RELEASE_SETUP.md §1)

- [ ] From the release run: Actions › Release › artifact
      `notchly-v0.3.0-store-and-winget` › `Notchly-0.3.0.msix`.
- [ ] Partner Center submission: package, properties, age rating, listing and screenshots,
      `runFullTrust` justification. Submit.
- [ ] When live: *variable* `NOTCHLY_STORE_URL` = the Store link. Also put the Store ID in
      the website (`notchly-website`, see its README) and redeploy.
- [ ] Install the Store version and check notification mirroring (TESTING.md §14).

## 5. Website support contact

- [ ] Choose the support email for the website and privacy policy (Google verification needs
      one). Then set it in the website repo (`src/content/site.ts` › `supportEmail`) and in
      `docs/PRIVACY.md`, and redeploy. Until then, GitHub Issues is the contact.
- [ ] Optional: enable **Discussions** (repo › Settings › General › Features).

## 6. Google OAuth consent screen and verification (RELEASE_SETUP.md §3–4)

- [ ] Search Console: verify `https://notchly.web.app/` with the HTML-file method (put the
      file in the website's `public/`, deploy, Verify).
- [ ] Google Auth Platform › Branding: homepage `https://notchly.web.app`, privacy policy
      `https://notchly.web.app/privacy`, authorised domain `notchly.web.app`.
- [ ] Data access: only `calendar.readonly`.
- [ ] Record the demo video (unlisted YouTube) and submit for verification.

## 7. winget pull request (RELEASE_SETUP.md §6)

- [ ] After the v0.3.0 release exists: take `winget/` from the release artifact, put the
      three files under `manifests/i/icedamericano16/Notchly/0.3.0/` in a fork of
      `microsoft/winget-pkgs`, open the PR (or `wingetcreate submit`).

## 8. Optional: SignPath (RELEASE_SETUP.md §7)

- [ ] Only if you decide to relicense Notchly under an OSI-approved licence. Then apply at
      <https://signpath.org> and set `NOTCHLY_SIGNPATH_ENABLED=true` after acceptance.

## 9. Local cleanup (your call)

- [ ] Delete the empty `C:\Users\khagendra\Desktop\Notchy\website` folder (it is not in
      git; Windows refused to delete it while something had it open).

## 10. Before announcing

- [ ] Work through `TESTING.md` on a real PC: every row marked "needs manual verification".
