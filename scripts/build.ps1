param(
    [switch]$Bundle,
    [switch]$SkipInstall
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "_msvc.ps1")
Enter-MsvcEnvironment

$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    if (-not $SkipInstall) {
        Write-Host "[creeper-core] pnpm install" -ForegroundColor DarkGreen
        & pnpm install --frozen-lockfile
        if ($LASTEXITCODE -ne 0) { throw "pnpm install failed" }
    }

    if ($Bundle) {
        Write-Host "[creeper-core] building Tauri bundle (NSIS)" -ForegroundColor DarkGreen
        & pnpm tauri build
    }
    else {
        Write-Host "[creeper-core] building portable exe" -ForegroundColor DarkGreen
        & pnpm tauri build --no-bundle
    }
    if ($LASTEXITCODE -ne 0) { throw "tauri build failed" }

    $candidates = @(
        (Join-Path $root "target\release\creeper-core-launcher.exe"),
        (Join-Path $root "src-tauri\target\release\creeper-core-launcher.exe")
    )
    $exe = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $exe) { throw "build finished but creeper-core-launcher.exe was not found" }

    $releaseDir = Join-Path $root "release"
    New-Item -ItemType Directory -Force -Path $releaseDir | Out-Null
    Copy-Item $exe (Join-Path $releaseDir "CreeperCore.exe") -Force

    if ($Bundle) {
        $nsis = Get-ChildItem (Join-Path $root "target\release\bundle\nsis") -Filter *.exe -ErrorAction SilentlyContinue |
            Select-Object -First 1
        if (-not $nsis) {
            $nsis = Get-ChildItem (Join-Path $root "src-tauri\target\release\bundle\nsis") -Filter *.exe -ErrorAction SilentlyContinue |
                Select-Object -First 1
        }
        if ($nsis) {
            Copy-Item $nsis.FullName (Join-Path $releaseDir $nsis.Name) -Force
            Write-Host "[creeper-core] installer: $(Join-Path $releaseDir $nsis.Name)" -ForegroundColor Green
        }
    }

    Write-Host "[creeper-core] portable: $(Join-Path $releaseDir 'CreeperCore.exe')" -ForegroundColor Green
}
finally {
    Pop-Location
}
