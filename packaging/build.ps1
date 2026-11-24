<#
.SYNOPSIS
  Builds a complete Notchly release into .\dist: signed exe, installer, sparse identity
  package (notification mirroring), Microsoft Store MSIX and update.json.

.EXAMPLE
  # local test release with a throw-away self-signed certificate (trusted on this PC only)
  .\packaging\build.ps1 -SelfSigned

.EXAMPLE
  # real release with your code-signing certificate (the publisher is read from it)
  .\packaging\build.ps1 -PfxPath C:\keys\notchly.pfx `
      -PfxPassword (Read-Host -AsSecureString) -DownloadBase "https://github.com/you/notchly/releases/download/v0.1.3"

.EXAMPLE
  # Store upload: the Store signs the package, so only the Partner Center identity is needed
  .\packaging\build.ps1 -StorePublisher "CN=XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX" -StoreIdentity "12345Publisher.Notchly"

.EXAMPLE
  # CI-style: everything from environment variables (see .env.example and RELEASE_SETUP.md)
  $env:NOTCHLY_PFX_BASE64 = [Convert]::ToBase64String([IO.File]::ReadAllBytes("C:\keys\notchly.pfx"))
  $env:NOTCHLY_PFX_PASSWORD = "..."
  .\packaging\build.ps1

.NOTES
  Optional build-time features (see src/build_info.rs) are read from the environment or
  config.toml: NOTCHLY_UPDATE_FEED, NOTCHLY_UPDATE_PUBKEY, NOTCHLY_WEBSITE, NOTCHLY_SUPPORT_URL,
  NOTCHLY_GOOGLE_CLIENT_ID, NOTCHLY_GOOGLE_CLIENT_SECRET, NOTCHLY_MS_CLIENT_ID.
  Signing: NOTCHLY_PFX_BASE64 / NOTCHLY_PFX_PASSWORD, TRUSTED_SIGNING_*, SIGNPATH_*,
  NOTCHLY_TIMESTAMP_URL; the update manifest: NOTCHLY_UPDATE_SIGNING_KEY.
#>
[CmdletBinding()]
param(
    # Subject of the code-signing certificate; must match the certificate exactly.
    [string]$Publisher = "",
    [string]$PfxPath = "",
    [SecureString]$PfxPassword,
    # Creates (once) and uses a self-signed test certificate in CurrentUser\My.
    [switch]$SelfSigned,
    # Partner Center values for the Store package (Product identity page).
    [string]$StorePublisher = "",
    [string]$StoreIdentity = "Notchly.Notchly",
    # Where the installer will be downloadable from (used in update.json).
    [string]$DownloadBase = "",
    [switch]$SkipBuild,
    [switch]$NoInstaller
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$dist = Join-Path $root "dist"
New-Item -ItemType Directory -Force $dist | Out-Null

# Runs a native tool; Windows PowerShell 5.1 would otherwise turn its stderr output
# (cargo progress, makeappx notes) into terminating errors under "Stop".
function Invoke-Native([string]$what, [scriptblock]$cmd) {
    $prev = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try { & $cmd 2>&1 | ForEach-Object { "$_" } | Write-Verbose } finally { $ErrorActionPreference = $prev }
    if ($LASTEXITCODE -ne 0) { throw "$what failed (exit $LASTEXITCODE)" }
}

function Find-SdkTool([string]$name) {
    $tool = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin\*\x64\$name" -ErrorAction SilentlyContinue |
        Sort-Object FullName | Select-Object -Last 1
    if (-not $tool) { throw "$name not found. Install the Windows 10/11 SDK." }
    $tool.FullName
}

# ------------------------------------------------------------------ version
$version = (Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$msixVersion = "$version.0"
Write-Host "Notchly $version" -ForegroundColor Cyan

# ------------------------------------------------------------------ certificate
# Signing providers, first match wins. Nothing here is ever written to the repo: secrets come
# from parameters or environment variables (CI secrets), and a missing certificate is a
# warning, not an error, so unsigned test builds still work.
#   -SelfSigned                         throw-away test certificate (this PC only)
#   TRUSTED_SIGNING_ENDPOINT (+ AZURE_*) Azure Trusted Signing
#   SIGNPATH_API_TOKEN (+ SIGNPATH_*)   SignPath (open-source or paid plan)
#   -PfxPath / NOTCHLY_PFX_BASE64       a .pfx code-signing certificate (+ NOTCHLY_PFX_PASSWORD)
$timestamp = if ($env:NOTCHLY_TIMESTAMP_URL) { $env:NOTCHLY_TIMESTAMP_URL } else { "http://timestamp.digicert.com" }
$signArgs = $null
$signer = "none"
if ($SelfSigned) {
    $subject = "CN=Notchly Test"
    $cert = Get-ChildItem Cert:\CurrentUser\My | Where-Object { $_.Subject -eq $subject } | Select-Object -First 1
    if (-not $cert) {
        Write-Host "Creating a self-signed test certificate ($subject)"
        $cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject $subject `
            -CertStoreLocation Cert:\CurrentUser\My -KeyUsage DigitalSignature `
            -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3", "2.5.29.19={text}")
    }
    # MSIX installs only trust certificates in TrustedPeople / Root
    $cer = Join-Path $dist "Notchly-Test.cer"
    Export-Certificate -Cert $cert -FilePath $cer | Out-Null
    Write-Host "To install the test packages, trust $cer once (admin): Import-Certificate -FilePath `"$cer`" -CertStoreLocation Cert:\LocalMachine\TrustedPeople" -ForegroundColor Yellow
    $Publisher = $subject
    $signArgs = @("/sha1", $cert.Thumbprint)
    $signer = "self-signed test certificate"
} elseif ($env:TRUSTED_SIGNING_ENDPOINT) {
    # Azure Trusted Signing: credentials from AZURE_TENANT_ID / AZURE_CLIENT_ID /
    # AZURE_CLIENT_SECRET; the account and profile from TRUSTED_SIGNING_ACCOUNT / _PROFILE
    if (-not $Publisher) { throw "-Publisher must match the Trusted Signing certificate subject" }
    if (-not (Get-Module -ListAvailable TrustedSigning)) {
        Install-Module TrustedSigning -Scope CurrentUser -Force -AllowClobber
    }
    $signArgs = @("trusted")
    $signer = "Azure Trusted Signing"
} elseif ($env:SIGNPATH_API_TOKEN) {
    # SignPath: the project's artifact configuration must accept a single PE file / MSIX
    foreach ($v in "SIGNPATH_ORGANIZATION_ID", "SIGNPATH_PROJECT_SLUG", "SIGNPATH_POLICY_SLUG") {
        if (-not (Get-Item "env:$v" -ErrorAction SilentlyContinue).Value) { throw "$v must be set for SignPath" }
    }
    if (-not $Publisher) { throw "-Publisher must match the SignPath certificate subject" }
    if (-not (Get-Module -ListAvailable SignPath)) {
        Install-Module SignPath -Scope CurrentUser -Force -AllowClobber
    }
    $signArgs = @("signpath")
    $signer = "SignPath"
} elseif ($PfxPath -or $env:NOTCHLY_PFX_BASE64) {
    if (-not $PfxPath) {
        # CI: the certificate arrives base64-encoded in a secret; it lives only in the
        # runner's temp folder for this job
        $PfxPath = Join-Path ([IO.Path]::GetTempPath()) "notchly-signing.pfx"
        [IO.File]::WriteAllBytes($PfxPath, [Convert]::FromBase64String($env:NOTCHLY_PFX_BASE64))
    }
    $plain = if ($PfxPassword) {
        [Runtime.InteropServices.Marshal]::PtrToStringAuto([Runtime.InteropServices.Marshal]::SecureStringToBSTR($PfxPassword))
    } elseif ($env:NOTCHLY_PFX_PASSWORD) { $env:NOTCHLY_PFX_PASSWORD } else { "" }
    if (-not $Publisher) {
        # the publisher must equal the certificate subject exactly; read it from the file
        $x = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2($PfxPath, $plain)
        $Publisher = $x.Subject
        Write-Host "Publisher (from the certificate): $Publisher"
    }
    $signArgs = @("/f", $PfxPath, "/p", $plain)
    $signer = "certificate $PfxPath"
} else {
    Write-Warning "No code-signing certificate (none of -SelfSigned, -PfxPath, NOTCHLY_PFX_BASE64, TRUSTED_SIGNING_ENDPOINT, SIGNPATH_API_TOKEN): the release will be UNSIGNED. SmartScreen will warn users and notification mirroring needs a signed build. See RELEASE_SETUP.md."
}
Write-Host "Signing: $signer (timestamp $timestamp)"

function Sign([string]$file) {
    if (-not $signArgs) { return }
    if ($signArgs[0] -eq "trusted") {
        Invoke-TrustedSigning -Endpoint $env:TRUSTED_SIGNING_ENDPOINT `
            -CodeSigningAccountName $env:TRUSTED_SIGNING_ACCOUNT `
            -CertificateProfileName $env:TRUSTED_SIGNING_PROFILE `
            -Files $file -FileDigest SHA256 `
            -TimestampRfc3161 "http://timestamp.acs.microsoft.com" -TimestampDigest SHA256
    } elseif ($signArgs[0] -eq "signpath") {
        Submit-SigningRequest -InputArtifactPath $file -CIUserToken $env:SIGNPATH_API_TOKEN `
            -OrganizationId $env:SIGNPATH_ORGANIZATION_ID -ProjectSlug $env:SIGNPATH_PROJECT_SLUG `
            -SigningPolicySlug $env:SIGNPATH_POLICY_SLUG -OutputArtifactPath $file `
            -WaitForCompletion -Force | Out-Null
    } else {
        # RFC 3161 timestamp: the signature stays valid after the certificate expires
        $signtool = Find-SdkTool "signtool.exe"
        Invoke-Native "signing $file" { & $signtool sign /fd SHA256 /tr $timestamp /td SHA256 @signArgs $file }
    }
    if ($signArgs[0] -ne "/sha1") {
        # a real certificate must chain to a trusted root and carry a timestamp
        $signtool = Find-SdkTool "signtool.exe"
        Invoke-Native "verifying the signature of $file" { & $signtool verify /pa /q $file }
    }
}

# ------------------------------------------------------------------ build
if (-not $SkipBuild) {
    # the publisher goes into the exe manifest so the sparse identity package attaches
    $env:NOTCHLY_PUBLISHER = $Publisher
    Write-Host "Building release..."
    Invoke-Native "cargo build" { cargo build --release }
}
$exe = Join-Path $root "target\release\notchly.exe"
Copy-Item $exe (Join-Path $dist "notchly.exe") -Force
Sign (Join-Path $dist "notchly.exe")

Invoke-Native "make_assets.py" { python packaging/make_assets.py }
$makeappx = Find-SdkTool "makeappx.exe"

function Stamp-Manifest([string]$src, [string]$dst, [string]$name, [string]$pub) {
    $xml = Get-Content $src -Raw
    # only the Identity element's Version (-replace is case-insensitive and would hit <?xml version?>)
    $xml = $xml -creplace '(<Identity [^>]*?)Version="[0-9.]+"', ('${1}Version="' + $msixVersion + '"')
    if ($pub) { $xml = $xml -replace 'Publisher="CN=REPLACE-WITH-[A-Z-]+"', ('Publisher="' + [Security.SecurityElement]::Escape($pub) + '"') }
    if ($name) { $xml = $xml -replace '<Identity Name="[^"]+"', ('<Identity Name="' + $name + '"') }
    Set-Content -Path $dst -Value $xml -Encoding UTF8
}

# ------------------------------------------------------------------ sparse identity package
if ($Publisher) {
    $sp = Join-Path $dist "sparse"
    Remove-Item $sp -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force $sp | Out-Null
    Copy-Item packaging\sparse\logo.png $sp
    Stamp-Manifest "packaging\sparse\AppxManifest.xml" (Join-Path $sp "AppxManifest.xml") "" $Publisher
    Invoke-Native "makeappx (sparse)" { & $makeappx pack /d $sp /p (Join-Path $dist "Notchly.Sparse.msix") /nv /o }
    Sign (Join-Path $dist "Notchly.Sparse.msix")
    Write-Host "Sparse identity package: dist\Notchly.Sparse.msix"
} else {
    Write-Host "No -Publisher: skipping the sparse package (notification mirroring needs a signed release)" -ForegroundColor Yellow
}

# ------------------------------------------------------------------ Store MSIX
$storePub = if ($StorePublisher) { $StorePublisher } else { $Publisher }
if ($storePub) {
    $st = Join-Path $dist "msix"
    Remove-Item $st -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force (Join-Path $st "Assets") | Out-Null
    Copy-Item $exe $st
    Copy-Item packaging\msix\Assets\*.png (Join-Path $st "Assets")
    Stamp-Manifest "packaging\msix\AppxManifest.xml" (Join-Path $st "AppxManifest.xml") $StoreIdentity $storePub
    $msix = Join-Path $dist "Notchly-$version.msix"
    Invoke-Native "makeappx (store)" { & $makeappx pack /d $st /p $msix /o }
    if (-not $StorePublisher) { Sign $msix }  # the Store re-signs uploads itself
    Write-Host "Store package: dist\Notchly-$version.msix"
}

# ------------------------------------------------------------------ installer + update feed
if (-not $NoInstaller) {
    $iscc = @("C:\Program Files (x86)\Inno Setup 6\ISCC.exe", "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe") |
        Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $iscc) {
        Write-Host "Inno Setup 6 not found (winget install JRSoftware.InnoSetup); skipping the installer" -ForegroundColor Yellow
    } else {
        $defines = @("/DAppVersion=$version", "/DExeDir=$dist")
        if ($env:NOTCHLY_WEBSITE) { $defines += "/DAppURL=$env:NOTCHLY_WEBSITE" }
        Invoke-Native "ISCC" { & $iscc @defines packaging\inno\notchly.iss }
        $setup = Join-Path $dist "Notchly-$version-setup.exe"
        Sign $setup
        $hash = (Get-FileHash $setup -Algorithm SHA256).Hash.ToLower()
        $base = if ($DownloadBase) { $DownloadBase.TrimEnd('/') } else { "https://REPLACE-WITH-DOWNLOAD-URL" }
        $feed = [ordered]@{
            version   = $version
            url       = "$base/Notchly-$version-setup.exe"
            sha256    = $hash
            notes     = "See CHANGELOG.md"
            published = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
        }
        # exact bytes, no BOM: the signature covers the file as served
        $json = $feed | ConvertTo-Json
        $manifest = Join-Path $dist "update.json"
        [IO.File]::WriteAllText($manifest, $json, (New-Object Text.UTF8Encoding($false)))
        & (Join-Path $PSScriptRoot "sign-update.ps1") -Manifest $manifest
        Write-Host "Installer: dist\Notchly-$version-setup.exe"
        Write-Host "Update feed: dist\update.json (+ .sig); the release workflow publishes both"
    }
}
Write-Host "Done." -ForegroundColor Green
