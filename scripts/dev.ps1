$ErrorActionPreference = "Stop"
& (Join-Path $PSScriptRoot "tauri.ps1") dev @args
exit $LASTEXITCODE
