param(
    [ValidateSet('All', 'Installer', 'Portable')]
    [string]$Mode = 'All',
    [string]$ConfigPath = ''
)

$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $root

# Extract version from single source of truth (tauri.conf.json)
$tauriConfig = Get-Content -Raw src-tauri/tauri.conf.json | ConvertFrom-Json
$version = $tauriConfig.version
if ([string]::IsNullOrWhiteSpace($version)) {
    throw 'Could not read version from src-tauri/tauri.conf.json'
}

Write-Host "============================================================" -ForegroundColor Cyan
Write-Host "   Lumen Ray - Release Build v$version (Mode: $Mode)" -ForegroundColor Cyan
Write-Host "============================================================" -ForegroundColor Cyan

# Check prerequisites
if (-not (Get-Command node -ErrorAction SilentlyContinue)) {
    throw 'Node.js is not found in PATH. Please install Node.js.'
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw 'Rust/Cargo is not found in PATH. Please install Rust.'
}

# Ensure verified bundled runtime is present
$runtimeRoot = Join-Path $root 'src-tauri/runtime'
$xrayExe = Join-Path $runtimeRoot 'xray/xray.exe'
$wintunDll = Join-Path $runtimeRoot 'drivers/wintun.dll'
if (-not (Test-Path -LiteralPath $xrayExe) -or -not (Test-Path -LiteralPath $wintunDll)) {
    Write-Host '[INFO] Preparing verified runtime...' -ForegroundColor Yellow
    powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'prepare-core.ps1')
}

# Keep running applications and unrelated release artifacts intact.
$releaseDir = Join-Path $root 'release'
if (-not (Test-Path -LiteralPath $releaseDir)) {
    New-Item -ItemType Directory -Force -Path $releaseDir | Out-Null
}

# Always use Tauri: a plain cargo --release build can still point at devUrl.
$tauriArgs = @('build')
if ($Mode -eq 'Portable') {
    $tauriArgs += '--no-bundle'
} else {
    $tauriArgs += @('--bundles', 'nsis')
}
if (-not [string]::IsNullOrWhiteSpace($ConfigPath)) {
    $tauriArgs += @('--config', $ConfigPath)
}

Write-Host '[1/3] Building application bundle via Tauri...' -ForegroundColor Green
& node (Join-Path $root 'node_modules/@tauri-apps/cli/tauri.js') @tauriArgs -- --locked
if ($LASTEXITCODE -ne 0) {
    throw "Tauri build failed with exit code $LASTEXITCODE"
}

# 1. Package Windows Setup installer (.exe)
if ($Mode -eq 'All' -or $Mode -eq 'Installer') {
    Write-Host '[2/3] Preparing Windows Setup installer (.exe)...' -ForegroundColor Green
    $nsisDir = Join-Path $root 'src-tauri/target/release/bundle/nsis'
    $setupCandidates = Get-ChildItem -LiteralPath $nsisDir -Filter '*.exe' | Where-Object { $_.Name -notmatch '(?i)uninst' }
    if (-not $setupCandidates -or $setupCandidates.Count -eq 0) {
        throw "No NSIS setup installer found in $nsisDir"
    }

    $setupSource = $setupCandidates[0].FullName
    $setupTarget = Join-Path $releaseDir "Lumen-Ray-$version-Setup.exe"
    Copy-Item -LiteralPath $setupSource -Destination $setupTarget -Force
    Write-Host "  -> Created Setup: $setupTarget ($( [math]::Round((Get-Item $setupTarget).Length / 1MB, 2) ) MB)" -ForegroundColor Cyan
}

# 2. Package Windows Portable (.zip)
if ($Mode -eq 'All' -or $Mode -eq 'Portable') {
    Write-Host '[3/3] Creating Windows Portable (.zip)...' -ForegroundColor Green
    $targetExe = Join-Path $root 'src-tauri/target/release/app.exe'
    if (-not (Test-Path -LiteralPath $targetExe)) {
        throw 'The Cargo binary app.exe was not produced; refusing to package a stale fallback executable'
    }

    $scratchDir = Join-Path ([IO.Path]::GetTempPath()) "lumen-portable-build-$([guid]::NewGuid().ToString('N'))"
    $portableAppDir = Join-Path $scratchDir 'Lumen'
    $portableRuntimeDir = Join-Path $portableAppDir 'runtime'

    try {
        New-Item -ItemType Directory -Force -Path $portableRuntimeDir | Out-Null

        # Copy main executable as "Lumen Ray.exe"
        Copy-Item -LiteralPath $targetExe -Destination (Join-Path $portableAppDir 'Lumen.exe') -Force

        # Copy runtime directory layout cleanly (xray, drivers, licenses)
        Copy-Item -LiteralPath (Join-Path $runtimeRoot 'xray') -Destination (Join-Path $portableRuntimeDir 'xray') -Recurse -Force
        Copy-Item -LiteralPath (Join-Path $runtimeRoot 'drivers') -Destination (Join-Path $portableRuntimeDir 'drivers') -Recurse -Force
        Copy-Item -LiteralPath (Join-Path $runtimeRoot 'licenses') -Destination (Join-Path $portableRuntimeDir 'licenses') -Recurse -Force

        # Startup diagnostics are inside Lumen.exe. Ship only the short readme.
        Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'portable/README.txt') -Destination (Join-Path $portableAppDir 'README.txt') -Force

        # Check the actual staged EXE, including embedded index.html, before distributing it.
        $check = Start-Process -FilePath (Join-Path $portableAppDir 'Lumen.exe') -ArgumentList '--lumen-startup-check' -WorkingDirectory $scratchDir -PassThru -WindowStyle Hidden
        if (-not $check.WaitForExit(60000)) {
            Stop-Process -Id $check.Id -Force -ErrorAction SilentlyContinue
            throw 'Portable startup check timed out'
        }
        if ($check.ExitCode -ne 0) {
            throw "Portable startup check failed with exit code $($check.ExitCode); no ZIP was produced"
        }

        # Create zip archive
        $zipTarget = Join-Path $releaseDir "Lumen-Ray-$version-Portable.zip"
        if (Test-Path -LiteralPath $zipTarget) {
            Remove-Item -LiteralPath $zipTarget -Force
        }

        Add-Type -AssemblyName System.IO.Compression.FileSystem
        [System.IO.Compression.ZipFile]::CreateFromDirectory($scratchDir, $zipTarget, [System.IO.Compression.CompressionLevel]::Optimal, $false)

        Write-Host "  -> Created Portable: $zipTarget ($( [math]::Round((Get-Item $zipTarget).Length / 1MB, 2) ) MB)" -ForegroundColor Cyan
    } finally {
        $resolvedScratch = [IO.Path]::GetFullPath($scratchDir)
        $tempPrefix = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
        if (-not $resolvedScratch.StartsWith($tempPrefix, [StringComparison]::OrdinalIgnoreCase) -or (Split-Path $resolvedScratch -Leaf) -notmatch '^lumen-portable-build-[0-9a-f]{32}$') {
            throw 'Portable scratch directory is outside the expected temporary directory'
        }
        if (Test-Path -LiteralPath $resolvedScratch) {
            Remove-Item -LiteralPath $resolvedScratch -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
}

Write-Host ""
Write-Host "============================================================" -ForegroundColor Green
Write-Host "   [SUCCESS] Release build finished successfully!" -ForegroundColor Green
Write-Host "============================================================" -ForegroundColor Green
Write-Host "Output directory: $releaseDir"
Get-ChildItem -LiteralPath $releaseDir -File | Format-Table Name, Length, LastWriteTime
