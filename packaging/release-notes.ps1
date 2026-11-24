<#
.SYNOPSIS
  Writes the GitHub release notes: ONLY this version's section of CHANGELOG.md (never the
  whole file), then a "Downloads" block that labels each file and lists its SHA-256.

.EXAMPLE
  .\packaging\release-notes.ps1 -Tag v0.3.0 -Out notes.md
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Tag,
    [string]$Out = "notes.md",
    [string]$StoreUrl = ""
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$version = $Tag.TrimStart("v")

function Get-ChangelogSection([string[]]$lines, [string]$v) {
    $start = $null
    for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match "^## $([regex]::Escape($v))\b") { $start = $i; break }
    }
    if ($null -eq $start) { return @() }
    $end = $lines.Count
    for ($j = $start + 1; $j -lt $lines.Count; $j++) {
        if ($lines[$j] -match '^## ') { $end = $j; break }
    }
    # drop the "## x.y.z - date" heading itself: the release title already says it
    return $lines[($start + 1)..($end - 1)]
}

$lines = Get-Content (Join-Path $root "CHANGELOG.md") -Encoding UTF8
$section = Get-ChangelogSection $lines $version
if (-not $section) { $section = @("Notchly $version") }

$dist = Join-Path $root "dist"
$downloads = @("", "## Downloads", "")
if ($StoreUrl) {
    $downloads += "- **Recommended: [Microsoft Store]($StoreUrl)**: signed by Microsoft, updates automatically, includes notification mirroring."
}
$setup = "Notchly-$version-setup.exe"
if (Test-Path (Join-Path $dist $setup)) {
    $downloads += "- ``$setup``: **advanced / unsigned** installer. Windows SmartScreen will warn (More info, then Run anyway). Notification mirroring is not available in this build."
}
if (Test-Path (Join-Path $dist "notchly.exe")) {
    $downloads += "- ``notchly.exe``: **advanced / portable, unsigned**: runs without installing; no automatic updates."
}
$downloads += "", "Verify a download in PowerShell: ``Get-FileHash .\<file>`` and compare with ``SHA256SUMS.txt``.", ""
$sums = Join-Path $dist "SHA256SUMS.txt"
if (Test-Path $sums) {
    $downloads += '```'
    $downloads += Get-Content $sums
    $downloads += '```'
}

($section + $downloads) -join "`n" | Set-Content -Path $Out -Encoding UTF8
Write-Host "Release notes: $Out ($($section.Count) changelog lines)"
