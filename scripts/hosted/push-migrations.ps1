# Apply repository supabase/migrations to the hosted project.
# Requires Supabase CLI login (SUPABASE_ACCESS_TOKEN) — not the desktop publishable key.
# Never uses sb_secret_ / service_role. Never runs db reset.
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

if ($env:SUPABASE_ACCESS_TOKEN -and $env:SUPABASE_ACCESS_TOKEN.StartsWith("sb_secret_")) {
    throw "SUPABASE_ACCESS_TOKEN looks like a project secret key. Use a personal access token from supabase.com/dashboard/account/tokens."
}

$ref = "rbxtxtlssknjioaveytg"
Write-Host "Linking $ref (no reset)"
supabase link --project-ref $ref

Write-Host "Remote migration history:"
supabase migration list

Write-Host "Pushing repository migrations (no data wipe)"
supabase db push --linked
Write-Host "Hosted push finished"
