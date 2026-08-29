# Validates the CI-built unsigned NSIS installer.
# Does not require a code-signing certificate.
$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$nsisDir = Join-Path $root "src-tauri\target\release\bundle\nsis"
$releaseExe = Join-Path $root "src-tauri\target\release\playstation-cafe.exe"
$tauriConf = Get-Content (Join-Path $root "src-tauri\tauri.conf.json") -Raw | ConvertFrom-Json
$pkg = Get-Content (Join-Path $root "package.json") -Raw | ConvertFrom-Json
$expectedVersion = [string]$pkg.version
$productName = [string]$tauriConf.productName

if (-not (Test-Path $releaseExe)) {
    throw "Tauri release binary missing: $releaseExe"
}
$relInfo = Get-Item $releaseExe
if ($relInfo.Length -lt 1MB) {
    throw "Release binary is suspiciously small: $($relInfo.Length) bytes"
}

if (-not (Test-Path $nsisDir)) {
    throw "NSIS bundle directory missing: $nsisDir"
}

$installers = @(Get-ChildItem -Path $nsisDir -Filter "*-setup.exe" -File)
if ($installers.Count -lt 1) {
    $installers = @(Get-ChildItem -Path $nsisDir -Filter "*.exe" -File)
}
if ($installers.Count -lt 1) {
    throw "No NSIS installer .exe under $nsisDir"
}

$installer = $installers | Sort-Object Length -Descending | Select-Object -First 1
if ($installer.Length -lt 1MB) {
    throw "Installer is empty or too small: $($installer.FullName) ($($installer.Length) bytes)"
}

if ($installer.Name -notmatch [regex]::Escape($expectedVersion)) {
    throw "Installer name '$($installer.Name)' does not contain expected version $expectedVersion"
}

$sha = (Get-FileHash -Algorithm SHA256 -Path $installer.FullName).Hash.ToLowerInvariant()
$shaFile = Join-Path $root "nsis-sha256.txt"
@(
    "file=$($installer.Name)"
    "bytes=$($installer.Length)"
    "sha256=$sha"
    "version=$expectedVersion"
    "product=$productName"
    "signing=unsigned"
    "channel=pre-release"
    "commit=$env:PACKAGED_SHA"
) | Set-Content -Path $shaFile -Encoding utf8
Write-Host "NSIS SHA-256: $sha"
Write-Host "Installer: $($installer.FullName) ($($installer.Length) bytes)"

$forbiddenName = [regex]'(?i)(\.env($|\.)|\.pem$|\.sqlite($|\.)|backup|\.bak$|service_role|secret|credential|token\.json|id_rsa)'
$bundleRoot = Join-Path $root "src-tauri\target\release\bundle"
$leaks = @()
Get-ChildItem -Path $bundleRoot -Recurse -File | ForEach-Object {
    if ($_.Name -match $forbiddenName -and $_.Extension -ne ".exe") {
        $leaks += $_.FullName
    }
}
if ($leaks.Count -gt 0) {
    throw "Forbidden files in bundle:`n$($leaks -join "`n")"
}

function Find-AsciiHits([string]$path, [string[]]$needles) {
    $bytes = [System.IO.File]::ReadAllBytes($path)
    $ascii = [System.Text.Encoding]::ASCII.GetString($bytes)
    $hits = @()
    foreach ($n in $needles) {
        if ($ascii.IndexOf($n, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
            $hits += $n
        }
    }
    return $hits
}

# Deny-list tokens such as "service_role" can appear in compiled guards.
# Fail only on material that would be an actual packaged credential.
$fatalNeedles = @(
    "BEGIN RSA PRIVATE KEY",
    "BEGIN OPENSSH PRIVATE KEY",
    "BEGIN PRIVATE KEY",
    "-----BEGIN CERTIFICATE-----",
    "sb_secret_"
)
$binaryHits = @()
foreach ($bin in @($releaseExe, $installer.FullName)) {
    $binaryHits += Find-AsciiHits -path $bin -needles $fatalNeedles
}
$binaryHits = $binaryHits | Select-Object -Unique
if ($binaryHits.Count -gt 0) {
    throw "Release binary/installer contains secret-like strings: $($binaryHits -join ', ')"
}

$report = [ordered]@{
    installer          = $installer.Name
    installer_bytes    = $installer.Length
    sha256             = $sha
    version            = $expectedVersion
    product            = $productName
    identifier         = [string]$tauriConf.identifier
    release_exe        = "playstation-cafe.exe"
    release_exe_bytes  = $relInfo.Length
    signing            = "unsigned"
    channel            = "development/pre-release"
    production_ready   = $false
    forbidden_files    = @()
    secret_string_hits = @($binaryHits)
}
$reportPath = Join-Path $root "nsis-validation.json"
($report | ConvertTo-Json -Depth 6) | Set-Content -Path $reportPath -Encoding utf8
Write-Host "Wrote $shaFile and $reportPath"
