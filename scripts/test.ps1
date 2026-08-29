Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
Set-Location -LiteralPath (Split-Path -Parent $PSScriptRoot)
node .\node_modules\vitest\vitest.mjs run
$gcc = Get-ChildItem -Path "$env:LOCALAPPDATA\Microsoft\WinGet\Packages" -Filter gcc.exe -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty DirectoryName
$env:Path = "$env:USERPROFILE\.cargo\bin;$gcc;$env:Path"
$env:CARGO_TARGET_DIR = "E:\psc-target"
cargo +stable-x86_64-pc-windows-gnu test -p cafe-domain --manifest-path src-tauri\Cargo.toml
