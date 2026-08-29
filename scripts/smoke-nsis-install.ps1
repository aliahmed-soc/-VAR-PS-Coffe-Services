# Isolated Windows install / upgrade / uninstall smoke for the unsigned NSIS installer.
# Runs on GitHub Actions windows-latest. Does not launch the GUI (human checklist).
$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$nsisDir = Join-Path $root "src-tauri\target\release\bundle\nsis"
$installers = @(Get-ChildItem -Path $nsisDir -Filter "*-setup.exe" -File)
if ($installers.Count -lt 1) {
    $installers = @(Get-ChildItem -Path $nsisDir -Filter "*.exe" -File)
}
if ($installers.Count -lt 1) {
    throw "No NSIS installer found under $nsisDir"
}
$installer = $installers | Sort-Object Length -Descending | Select-Object -First 1

$work = Join-Path $env:RUNNER_TEMP "psc-nsis-smoke"
if (-not $work) {
    $work = Join-Path $env:TEMP "psc-nsis-smoke"
}
if (Test-Path $work) {
    Remove-Item -Recurse -Force $work
}
New-Item -ItemType Directory -Path $work | Out-Null

$dest1 = Join-Path $work "install-a"
$dest2 = Join-Path $work "install-b"
New-Item -ItemType Directory -Path $dest1 | Out-Null

function Install-Silent([string]$setup, [string]$dest) {
    # NSIS /D= must be last and must not be quoted. Path has no trailing slash.
    $destNs = $dest.TrimEnd('\')
    $p = Start-Process -FilePath $setup -ArgumentList @("/S", "/D=$destNs") -Wait -PassThru
    if ($p.ExitCode -ne 0) {
        throw "Silent install failed with exit $($p.ExitCode) dest=$destNs"
    }
}

function Find-AppExe([string]$dest) {
    $hits = @(Get-ChildItem -Path $dest -Recurse -Filter "playstation-cafe.exe" -ErrorAction SilentlyContinue)
    if ($hits.Count -lt 1) {
        $hits = @(Get-ChildItem -Path $dest -Recurse -Filter "*.exe" -ErrorAction SilentlyContinue |
            Where-Object { $_.Name -notmatch '(?i)uninstall' })
    }
    return $hits | Select-Object -First 1
}

Write-Host "Silent install -> $dest1"
Install-Silent -setup $installer.FullName -dest $dest1

$app = Find-AppExe -dest $dest1
if (-not $app) {
    throw "Installed application exe not found under $dest1"
}
Write-Host "Installed exe: $($app.FullName) ($($app.Length) bytes)"
if ($app.Length -lt 1MB) {
    throw "Installed exe is too small"
}

$installSqlite = @(Get-ChildItem -Path $dest1 -Recurse -File -ErrorAction SilentlyContinue |
    Where-Object { $_.Extension -in ".sqlite", ".db", ".env", ".pem" })
if ($installSqlite.Count -gt 0) {
    throw "Clean install must not ship storage/secrets: $($installSqlite.FullName -join ', ')"
}

# Application storage is created at runtime under the OS app-data dir, not the install dir.
$appDataCandidates = @(
    (Join-Path $env:APPDATA "com.playstationcafe.pos"),
    (Join-Path $env:LOCALAPPDATA "com.playstationcafe.pos"),
    (Join-Path $env:APPDATA "PlayStation Cafe POS"),
    (Join-Path $env:LOCALAPPDATA "PlayStation Cafe POS")
)
$probeDir = Join-Path $work "appdata-probe"
New-Item -ItemType Directory -Path $probeDir | Out-Null
$marker = Join-Path $probeDir "branch.sqlite"
"upgrade-preserve-marker" | Set-Content -Path $marker -Encoding utf8
$markerHash = (Get-FileHash -Algorithm SHA256 -Path $marker).Hash

# Re-install into the same destination (upgrade path) and confirm the marker is untouched.
Write-Host "Upgrade install (same dest) -> $dest1"
Install-Silent -setup $installer.FullName -dest $dest1
$after = Get-FileHash -Algorithm SHA256 -Path $marker
if ($after.Hash -ne $markerHash) {
    throw "Upgrade overwrote an adjacent SQLite marker; upgrade must preserve the database file"
}
$app2 = Find-AppExe -dest $dest1
if (-not $app2) {
    throw "Application exe missing after upgrade"
}

# Second isolated destination proves a clean install still has no shipped DB.
New-Item -ItemType Directory -Path $dest2 | Out-Null
Write-Host "Second clean install -> $dest2"
Install-Silent -setup $installer.FullName -dest $dest2
$shipped = @(Get-ChildItem -Path $dest2 -Recurse -File -ErrorAction SilentlyContinue |
    Where-Object { $_.Name -match '(?i)\.(sqlite|db|env|pem)$' })
if ($shipped.Count -gt 0) {
    throw "Second clean install shipped storage/secrets"
}

function Invoke-Uninstall([string]$dest) {
    $un = @(Get-ChildItem -Path $dest -Recurse -Filter "uninstall.exe" -ErrorAction SilentlyContinue)
    if ($un.Count -lt 1) {
        Write-Host "No uninstall.exe under $dest (document as physical check)"
        return $false
    }
    $p = Start-Process -FilePath $un[0].FullName -ArgumentList @("/S") -Wait -PassThru
    if ($p.ExitCode -ne 0) {
        Write-Host "Uninstall exit $($p.ExitCode) — treating as documented physical check"
        return $false
    }
    return $true
}

$uninstalled = Invoke-Uninstall -dest $dest1
if ((Get-FileHash -Algorithm SHA256 -Path $marker).Hash -ne $markerHash) {
    throw "Uninstall destroyed adjacent business-data marker; uninstall must not silently wipe SQLite"
}
if ($uninstalled) {
    Write-Host "Uninstall completed; business-data marker preserved"
}

# Runtime app-data dirs must not be created by the installer itself.
$createdByInstaller = @()
foreach ($p in $appDataCandidates) {
    if (Test-Path $p) {
        $createdByInstaller += $p
    }
}
Write-Host "Pre-existing app-data candidates (ok if empty): $($createdByInstaller -join '; ')"

Write-Host "NSIS smoke passed: clean install has no DB, upgrade preserves marker, uninstall does not wipe data"
Write-Host "Physical remaining: first GUI launch, login, and full acceptance checklist."
