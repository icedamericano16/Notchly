<#
.SYNOPSIS
  Fails a release when its pieces disagree: the git tag, Cargo.toml, CHANGELOG.md, the update
  manifest (dist\update.json) and the update-signing keys.

.DESCRIPTION
  Run twice by release.yml: with -Stage pre (before building: tag, Cargo.toml, CHANGELOG) and
  -Stage post (after packaging: update.json's version, download URL and SHA-256 must match
  the installer this tag publishes; if the app is built with an update PUBLIC key, the
  manifest must be signed by the matching PRIVATE key).

.EXAMPLE
  .\packaging\release-check.ps1 -Tag v0.3.0 -Repository icedamericano16/Notchly -Stage pre
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Tag,
    [Parameter(Mandatory)][string]$Repository,
    [ValidateSet("pre", "post")][string]$Stage = "post"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$problems = @()

$version = (Select-String -Path (Join-Path $root "Cargo.toml") -Pattern '^version\s*=\s*"([^"]+)"' |
        Select-Object -First 1).Matches[0].Groups[1].Value
Write-Host "Cargo.toml: $version   tag: $Tag"

if ("v$version" -ne $Tag) {
    $problems += "Tag $Tag does not match Cargo.toml version $version (expected v$version)"
}

if ($Stage -eq "pre") {
    $changelog = Get-Content (Join-Path $root "CHANGELOG.md")
    if (-not ($changelog | Select-String -Pattern "^## $([regex]::Escape($version))\b")) {
        $problems += "CHANGELOG.md has no '## $version' section (the release notes come from it)"
    }
} else {
    $dist = Join-Path $root "dist"
    $setupName = "Notchly-$version-setup.exe"
    $setup = Join-Path $dist $setupName
    $manifestPath = Join-Path $dist "update.json"
    if (-not (Test-Path $setup)) { $problems += "dist\$setupName is missing" }
    if (-not (Test-Path $manifestPath)) {
        $problems += "dist\update.json is missing"
    } else {
        $m = Get-Content $manifestPath -Raw | ConvertFrom-Json
        $expectedUrl = "https://github.com/$Repository/releases/download/$Tag/$setupName"
        if ($m.version -ne $version) { $problems += "update.json version '$($m.version)' != $version" }
        if ($m.url -ne $expectedUrl) { $problems += "update.json url '$($m.url)' != $expectedUrl" }
        if (Test-Path $setup) {
            $hash = (Get-FileHash $setup -Algorithm SHA256).Hash.ToLower()
            if ($m.sha256 -ne $hash) { $problems += "update.json sha256 does not match $setupName" }
        }

        # The app looks for update.json at the feed URL baked in at build time; for GitHub that
        # is releases/latest/download/update.json, which resolves to the release marked "latest".
        $feed = $env:NOTCHLY_UPDATE_FEED
        if ($feed -and $feed -notmatch '/releases/latest/download/update\.json$' -and $feed -notmatch [regex]::Escape("/releases/download/$Tag/update.json")) {
            Write-Warning "NOTCHLY_UPDATE_FEED ($feed) is not this repository's latest-release URL; make sure it serves this release's update.json"
        }

        $pub = $env:NOTCHLY_UPDATE_PUBKEY
        $sigPath = "$manifestPath.sig"
        if ($pub) {
            if (-not (Test-Path $sigPath)) {
                $problems += "The app is built with NOTCHLY_UPDATE_PUBKEY but update.json is unsigned: set the NOTCHLY_UPDATE_SIGNING_KEY secret"
            }
            $priv = $env:NOTCHLY_UPDATE_SIGNING_KEY
            if ($priv) {
                $raw = [Convert]::FromBase64String($priv.Trim())
                $pubFromPriv = [Convert]::ToBase64String([byte[]]$raw[32..95])
                if ($pubFromPriv -ne $pub.Trim()) {
                    $problems += "NOTCHLY_UPDATE_SIGNING_KEY does not belong to NOTCHLY_UPDATE_PUBKEY: shipped apps would reject every update"
                }
            }
        } else {
            Write-Warning "No NOTCHLY_UPDATE_PUBKEY: this build checks for updates but never installs them (RELEASE_SETUP.md, update key)"
        }
    }
}

if ($problems) {
    $problems | ForEach-Object { Write-Host "::error::$_" }
    throw "Release check failed ($($problems.Count) problem(s))"
}
Write-Host "Release check ($Stage) passed." -ForegroundColor Green
