<#
.SYNOPSIS
  Writes a Scoop manifest for this release into dist\scoop\notchly.json. It installs the
  portable notchly.exe (no installer), adds a Start menu shortcut, and keeps settings in
  %APPDATA%\Notchly as usual. checkver/autoupdate let Scoop buckets update it on their own.
  Submit it to a bucket with a free pull request (see RELEASE_SETUP.md).

.EXAMPLE
  .\packaging\scoop.ps1 -Tag v0.3.1 -Repository icedamericano16/Notchly
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Tag,
    [Parameter(Mandatory)][string]$Repository
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$version = $Tag.TrimStart("v")
$exe = Join-Path $root "dist\notchly.exe"
if (-not (Test-Path $exe)) { throw "dist\notchly.exe not found: build first" }
$sha = (Get-FileHash $exe -Algorithm SHA256).Hash.ToLower()
$base = "https://github.com/$Repository"

$manifest = [ordered]@{
    version     = $version
    description = "A living Dynamic Island for Windows: a notch with music, timers, calls and tools."
    homepage    = "https://notchly.web.app"
    license     = [ordered]@{ identifier = "Freeware"; url = "$base/blob/main/LICENSE" }
    notes       = @(
        "Notchly is unsigned; Windows SmartScreen may warn on first start.",
        "Updates come from Scoop (scoop update notchly); the app's own updater is off."
    )
    architecture = [ordered]@{
        "64bit" = [ordered]@{
            url  = "$base/releases/download/$Tag/notchly.exe"
            hash = $sha
        }
    }
    shortcuts   = @(, @("notchly.exe", "Notchly"))
    checkver    = [ordered]@{ github = $base }
    autoupdate  = [ordered]@{
        architecture = [ordered]@{
            "64bit" = [ordered]@{ url = "$base/releases/download/v`$version/notchly.exe" }
        }
    }
}

$dir = Join-Path $root "dist\scoop"
New-Item -ItemType Directory -Force $dir | Out-Null
$json = $manifest | ConvertTo-Json -Depth 6
[IO.File]::WriteAllText((Join-Path $dir "notchly.json"), $json + "`n", (New-Object Text.UTF8Encoding($false)))
Write-Host "Scoop manifest: dist\scoop\notchly.json ($version, sha256 $sha)"
