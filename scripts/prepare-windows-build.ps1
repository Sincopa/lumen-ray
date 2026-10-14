$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Push-Location -LiteralPath $root
try {
    if (-not (Test-Path -LiteralPath (Join-Path $root 'src-tauri/runtime/xray/xray.exe'))) {
        & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'prepare-core.ps1')
        if ($LASTEXITCODE -ne 0) { throw 'Runtime preparation failed.' }
    }
    # The signed Xray hash must be embedded by Rust before compilation, not patched afterwards.
    & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'sign-windows.ps1') -Mode Runtime
    if ($LASTEXITCODE -ne 0) { throw 'Runtime Authenticode signing failed.' }
    & node (Join-Path $root 'node_modules/vite/bin/vite.js') build
    if ($LASTEXITCODE -ne 0) { throw 'Frontend build failed.' }
} finally { Pop-Location }
