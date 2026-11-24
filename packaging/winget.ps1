<#
.SYNOPSIS
  Writes a winget manifest (multi-file, schema 1.9.0) for this release's installer into
  dist\winget\<PackageIdentifier>\<version>\. Submit it with a free pull request to
  microsoft/winget-pkgs (see RELEASE_SETUP.md); winget's own validation runs there.

.EXAMPLE
  .\packaging\winget.ps1 -Tag v0.3.0 -Repository icedamericano16/Notchly
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Tag,
    [Parameter(Mandatory)][string]$Repository,
    [string]$PackageIdentifier = "icedamericano16.Notchly",
    [string]$Publisher = "icedamericano16"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$version = $Tag.TrimStart("v")
$setupName = "Notchly-$version-setup.exe"
$setup = Join-Path $root "dist\$setupName"
if (-not (Test-Path $setup)) { throw "dist\$setupName not found: build first" }
$sha = (Get-FileHash $setup -Algorithm SHA256).Hash.ToUpper()
$url = "https://github.com/$Repository/releases/download/$Tag/$setupName"
# Inno Setup's uninstall key: AppId + "_is1" (packaging/inno/notchly.iss)
$productCode = "{6E0F5D9A-3C1B-4F7E-9B2A-4C6D8E1F0A31}_is1"
$schema = "1.9.0"

$dir = Join-Path $root "dist\winget\$PackageIdentifier\$version"
New-Item -ItemType Directory -Force $dir | Out-Null
$utf8 = New-Object Text.UTF8Encoding($false)

$versionYaml = @"
# yaml-language-server: `$schema=https://aka.ms/winget-manifest.version.$schema.schema.json
PackageIdentifier: $PackageIdentifier
PackageVersion: $version
DefaultLocale: en-US
ManifestType: version
ManifestVersion: $schema
"@

$installerYaml = @"
# yaml-language-server: `$schema=https://aka.ms/winget-manifest.installer.$schema.schema.json
PackageIdentifier: $PackageIdentifier
PackageVersion: $version
MinimumOSVersion: 10.0.19041.0
InstallerType: inno
Scope: user
InstallModes:
- interactive
- silent
UpgradeBehavior: install
ProductCode: '$productCode'
Installers:
- Architecture: x64
  InstallerUrl: $url
  InstallerSha256: $sha
ManifestType: installer
ManifestVersion: $schema
"@

$localeYaml = @"
# yaml-language-server: `$schema=https://aka.ms/winget-manifest.defaultLocale.$schema.schema.json
PackageIdentifier: $PackageIdentifier
PackageVersion: $version
PackageLocale: en-US
Publisher: $Publisher
PublisherUrl: https://github.com/$($Repository.Split('/')[0])
PublisherSupportUrl: https://github.com/$Repository/issues
PrivacyUrl: https://notchly.web.app/privacy
PackageName: Notchly
PackageUrl: https://notchly.web.app
License: Freeware
LicenseUrl: https://github.com/$Repository/blob/main/LICENSE
ShortDescription: A living Dynamic Island for Windows.
Description: Notchly turns the top-center of your screen into a true-black notch that shows music, timers, calls, downloads and notifications, and opens into a small control center on hover.
Tags:
- dynamic-island
- notch
- productivity
- widgets
ReleaseNotesUrl: https://github.com/$Repository/releases/tag/$Tag
ManifestType: defaultLocale
ManifestVersion: $schema
"@

[IO.File]::WriteAllText((Join-Path $dir "$PackageIdentifier.yaml"), $versionYaml + "`n", $utf8)
[IO.File]::WriteAllText((Join-Path $dir "$PackageIdentifier.installer.yaml"), $installerYaml + "`n", $utf8)
[IO.File]::WriteAllText((Join-Path $dir "$PackageIdentifier.locale.en-US.yaml"), $localeYaml + "`n", $utf8)
Write-Host "winget manifest: $dir"
