# Apply repository supabase/migrations to the hosted project.
# Requires SUPABASE_ACCESS_TOKEN and SUPABASE_DB_PASSWORD in process env.
# Never uses sb_secret_ / sb_publishable_ / service_role. Never runs db reset.
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

function Test-NotProjectKey {
    param([string]$Name, [string]$Value)
    if ([string]::IsNullOrWhiteSpace($Value)) {
        throw "Missing $Name. Need SUPABASE_ACCESS_TOKEN and SUPABASE_DB_PASSWORD."
    }
    if (
        $Value.StartsWith("sb_secret_") -or
        $Value.StartsWith("sb_publishable_") -or
        $Value -match "service_role"
    ) {
        throw "$Name looks like a project API key, not a deploy credential"
    }
}

Test-NotProjectKey "SUPABASE_ACCESS_TOKEN" $env:SUPABASE_ACCESS_TOKEN
Test-NotProjectKey "SUPABASE_DB_PASSWORD" $env:SUPABASE_DB_PASSWORD

$env:CI = "true"
$ref = "rbxtxtlssknjioaveytg"

function Invoke-LinkedSql {
    param([string[]]$Paths)
    $sql = ($Paths | ForEach-Object { Get-Content -Raw -Path $_ }) -join "`n"
    $sql | supabase db query --linked
    if ($LASTEXITCODE -ne 0) {
        throw "hosted SQL failed: $($Paths -join ', ')"
    }
}

Write-Host "Linking $ref (no reset, non-interactive)"
supabase link --project-ref $ref --yes
if ($LASTEXITCODE -ne 0) { throw "supabase link failed" }

Write-Host "Remote migration history:"
supabase migration list --linked
if ($LASTEXITCODE -ne 0) { throw "migration list failed" }

$help = supabase db push --help 2>&1 | Out-String
if ($help -match "--dry-run") {
    Write-Host "Dry-run:"
    supabase db push --linked --dry-run
    if ($LASTEXITCODE -ne 0) { throw "db push --dry-run failed" }
} else {
    Write-Host "db push --dry-run not supported; skipping"
}

Write-Host "Inspecting remote public schema"
Invoke-LinkedSql @("supabase/tests/hosted/inspect_remote.sql")

Write-Host "Pushing outstanding repository migrations (no data wipe, no seed)"
supabase db push --linked --yes
if ($LASTEXITCODE -ne 0) { throw "db push failed" }

Write-Host "Remote migration history after push:"
supabase migration list --linked

Write-Host "Validating hosted schema"
Invoke-LinkedSql @("supabase/tests/hosted/validate_schema.sql")

Write-Host "Hosted RLS matrix"
Invoke-LinkedSql @(
    "supabase/tests/hosted/auth_insert_helper.sql",
    "supabase/tests/hosted/rls_matrix.sql"
)

Write-Host "Hosted event acceptance"
Invoke-LinkedSql @(
    "supabase/tests/hosted/auth_insert_helper.sql",
    "supabase/tests/hosted/event_acceptance.sql"
)

Write-Host "Dropping hosted-test helpers"
Invoke-LinkedSql @("supabase/tests/hosted/cleanup_helpers.sql")

Write-Host "Hosted push finished"
