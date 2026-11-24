<#
.SYNOPSIS
  Signs an update manifest (update.json) so the app will accept it.

.DESCRIPTION
  Writes <Manifest>.sig: the base64 ECDSA P-256 / SHA-256 signature (IEEE P1363, r||s) of the
  manifest's exact bytes. The private key comes from the NOTCHLY_UPDATE_SIGNING_KEY environment
  variable (see update-key.ps1); it is never written to disk. Without the key the manifest is
  left unsigned and a warning is printed: apps built with NOTCHLY_UPDATE_PUBKEY then ignore it.

.EXAMPLE
  $env:NOTCHLY_UPDATE_SIGNING_KEY = "<from your password manager>"
  .\packaging\sign-update.ps1 -Manifest dist\update.json
#>
[CmdletBinding()]
param([Parameter(Mandatory)][string]$Manifest)

$ErrorActionPreference = "Stop"
if (-not $env:NOTCHLY_UPDATE_SIGNING_KEY) {
    Write-Warning "NOTCHLY_UPDATE_SIGNING_KEY is not set: $Manifest stays unsigned (apps with an update key will ignore it)"
    exit 0
}
$raw = [Convert]::FromBase64String($env:NOTCHLY_UPDATE_SIGNING_KEY.Trim())
if ($raw.Length -ne 96) { throw "NOTCHLY_UPDATE_SIGNING_KEY must be 96 bytes (D||X||Y), base64" }
$q = New-Object System.Security.Cryptography.ECPoint
$q.X = [byte[]]$raw[32..63]
$q.Y = [byte[]]$raw[64..95]
$p = New-Object System.Security.Cryptography.ECParameters
$p.Curve = [System.Security.Cryptography.ECCurve+NamedCurves]::nistP256
$p.D = [byte[]]$raw[0..31]
$p.Q = $q
$k = [System.Security.Cryptography.ECDsa]::Create($p)
$bytes = [IO.File]::ReadAllBytes((Resolve-Path $Manifest))
$sig = $k.SignData($bytes, [System.Security.Cryptography.HashAlgorithmName]::SHA256)
if ($sig.Length -ne 64) { throw "unexpected signature length $($sig.Length)" }
$out = "$((Resolve-Path $Manifest).Path).sig"
[IO.File]::WriteAllText($out, [Convert]::ToBase64String($sig))
Write-Host "Signed: $out"
