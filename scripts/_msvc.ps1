# Shared MSVC environment loader for the Windows development scripts.

function Enter-MsvcEnvironment {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path $vswhere)) {
        $vswhere = Join-Path $env:ProgramFiles "Microsoft Visual Studio\Installer\vswhere.exe"
    }
    if (-not (Test-Path $vswhere)) {
        throw "找不到 vswhere.exe。请安装 Visual Studio Build Tools（Desktop development with C++）。"
    }

    $installPath = & $vswhere -latest -products * `
        -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
        -property installationPath | Select-Object -First 1
    if (-not $installPath) {
        throw "没有找到带 C++ 工具链的 Visual Studio / Build Tools。"
    }

    $vcvars = Join-Path $installPath "VC\Auxiliary\Build\vcvars64.bat"
    if (-not (Test-Path $vcvars)) {
        throw "缺少 vcvars64.bat：$vcvars"
    }

    $environmentLines = & cmd.exe /d /s /c "`"$vcvars`" >nul && set"
    foreach ($line in $environmentLines) {
        if ($line -match '^([^=][^=]*)=(.*)$') {
            Set-Item -Path "Env:$($matches[1])" -Value $matches[2]
        }
    }

    $toolchain = "stable-x86_64-pc-windows-msvc"
    $installed = & rustup toolchain list 2>$null
    if ($LASTEXITCODE -ne 0 -or -not ($installed -match [regex]::Escape($toolchain))) {
        throw "缺少 Rust MSVC 工具链。请运行：rustup toolchain install $toolchain"
    }
    $env:RUSTUP_TOOLCHAIN = $toolchain
    Write-Host "[creeper-core] MSVC ready · $toolchain · $installPath" -ForegroundColor DarkGreen
}
