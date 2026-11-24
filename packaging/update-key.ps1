<#
.SYNOPSIS
  Creates the update-signing key pair (ECDSA P-256). Run once, on your own PC.

.DESCRIPTION
  Prints two values:
    * NOTCHLY_UPDATE_SIGNING_KEY  - the PRIVATE key. Store it only as a GitHub Actions secret
                                    (or in a password manager). Never commit it.
    * NOTCHLY_UPDATE_PUBKEY       - the public key. Set it as a GitHub Actions variable; it is
                                    baked into the app, which then refuses any update whose
                                    manifest isn't signed by the private key.
  Both are base64: the private value is D||X||Y (96 bytes), the public value X||Y (64 bytes).
  Works in Windows PowerShell 5.1 and PowerShell 7.

.EXAMPLE
  .\packaging\update-key.ps1
#>
$ErrorActionPreference = "Stop"
$k = [System.Security.Cryptography.ECDsa]::Create([System.Security.Cryptography.ECCurve+NamedCurves]::nistP256)
$p = $k.ExportParameters($true)
$priv = [byte[]]($p.D + $p.Q.X + $p.Q.Y)
$pub = [byte[]]($p.Q.X + $p.Q.Y)
Write-Host "NOTCHLY_UPDATE_SIGNING_KEY (secret - keep private):" -ForegroundColor Yellow
[Convert]::ToBase64String($priv)
Write-Host ""
Write-Host "NOTCHLY_UPDATE_PUBKEY (public - goes into the app):" -ForegroundColor Cyan
[Convert]::ToBase64String($pub)
