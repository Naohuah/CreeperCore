param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$TauriArgs
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "_msvc.ps1")
Enter-MsvcEnvironment

$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    & pnpm tauri @TauriArgs
    $code = $LASTEXITCODE
    if ($code -ne 0) {
        exit $code
    }
}
finally {
    Pop-Location
}
