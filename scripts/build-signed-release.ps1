param(
    [Parameter(Mandatory=$true)][uri]$Endpoint,
    [Parameter(Mandatory=$true)][string]$PublicKeyPath,
    [Parameter(Mandatory=$true)][string]$PrivateKeyPath
)
$ErrorActionPreference = 'Stop'
if ($Endpoint.Scheme -ne 'https') { throw 'An HTTPS update endpoint is required.' }
$project = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$privateFile = (Resolve-Path -LiteralPath $PrivateKeyPath).Path
if ($privateFile.StartsWith($project + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Keep the signing private key outside the repository.'
}
$env:LUMEN_UPDATE_ENDPOINT = $Endpoint.AbsoluteUri
$env:LUMEN_UPDATE_PUBLIC_KEY = [IO.File]::ReadAllText((Resolve-Path -LiteralPath $PublicKeyPath).Path).Trim()
$previousKey = $env:TAURI_SIGNING_PRIVATE_KEY
try {
    $env:TAURI_SIGNING_PRIVATE_KEY = [IO.File]::ReadAllText($privateFile).Trim()
    $configPath = Join-Path ([IO.Path]::GetTempPath()) ('lumen-public-build-' + [guid]::NewGuid() + '.json')
    @{ bundle = @{ createUpdaterArtifacts = $true } } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $configPath -Encoding UTF8
    Push-Location -LiteralPath $project
    try {
        $env:TAURI_WINDOWS_SIGNTOOL_PATH = & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'sign-windows.ps1') -Mode ToolPath
        if ($LASTEXITCODE -ne 0) { throw 'Windows SDK SignTool discovery failed.' }
        & node node_modules/@tauri-apps/cli/tauri.js build --bundles nsis --config $configPath
        if ($LASTEXITCODE -ne 0) { throw 'Signed release build failed.' }
    } finally { Pop-Location }
} finally {
    $env:TAURI_SIGNING_PRIVATE_KEY = $previousKey
    if ($configPath -and (Test-Path -LiteralPath $configPath)) { Remove-Item -LiteralPath $configPath }
}
