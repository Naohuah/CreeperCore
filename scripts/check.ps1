$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "_msvc.ps1")
Enter-MsvcEnvironment

$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    Write-Host "[creeper-core] cargo test -p creeper-core" -ForegroundColor DarkGreen
    & cargo test -p creeper-core
    if ($LASTEXITCODE -ne 0) { throw "core tests failed" }

    Write-Host "[creeper-core] frontend type-check + production build" -ForegroundColor DarkGreen
    & pnpm build
    if ($LASTEXITCODE -ne 0) { throw "frontend build failed" }

    Write-Host "[creeper-core] all checks passed" -ForegroundColor Green
}
finally {
    Pop-Location
}
