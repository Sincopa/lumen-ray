param()
$ErrorActionPreference = 'Stop'
$version = '26.9.9'
$sha256 = '244deaba2098c2964e49bba90df3707777e5f5f428a82d2f29604015f24beec2'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$runtimeRoot = Join-Path $root 'src-tauri/runtime'
$xrayDir = Join-Path $runtimeRoot 'xray'
$driversDir = Join-Path $runtimeRoot 'drivers'
$licensesDir = Join-Path $runtimeRoot 'licenses'
$cache = Join-Path $root 'artifacts/core-download'

New-Item -ItemType Directory -Force -Path $xrayDir, $driversDir, $licensesDir, $cache | Out-Null

$archive = Join-Path $cache 'Xray-windows-64.zip'
if (!(Test-Path -LiteralPath $archive)) {
    Invoke-WebRequest "https://github.com/XTLS/Xray-core/releases/download/v$version/Xray-windows-64.zip" -OutFile $archive -UseBasicParsing
}
if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $sha256) {
    throw 'Xray archive integrity verification failed'
}

Add-Type -AssemblyName System.IO.Compression.FileSystem
$zip = [IO.Compression.ZipFile]::OpenRead($archive)
try {
    # Extract xray.exe
    $xrayEntry = $zip.Entries | Where-Object FullName -eq 'xray.exe'
    if ($xrayEntry) {
        [IO.Compression.ZipFileExtensions]::ExtractToFile($xrayEntry, (Join-Path $xrayDir 'xray.exe'), $true)
    }

    # Extract wintun.dll into runtime/drivers/
    $wintunEntry = $zip.Entries | Where-Object FullName -eq 'wintun.dll'
    if ($wintunEntry) {
        [IO.Compression.ZipFileExtensions]::ExtractToFile($wintunEntry, (Join-Path $driversDir 'wintun.dll'), $true)
    }

    # Extract LICENSE into runtime/licenses/xray.txt
    $licenseEntry = $zip.Entries | Where-Object FullName -eq 'LICENSE'
    if ($licenseEntry) {
        [IO.Compression.ZipFileExtensions]::ExtractToFile($licenseEntry, (Join-Path $licensesDir 'xray.txt'), $true)
    }
} finally {
    $zip.Dispose()
}

# Download and verify Russian GeoIP and GeoSite database files (NetworkKeeper/v2ray-rules-ru)
$geoVersion = '202410152325'
$geoFiles = @{
    'geoip.dat'   = @{ url = "https://github.com/NetworkKeeper/v2ray-rules-ru/releases/download/$geoVersion/geoip.dat";   sha256 = '9315c8677c101db1097cc16a53c47cd315e576a6b50734b6a9eed72862e32402' }
    'geosite.dat' = @{ url = "https://github.com/NetworkKeeper/v2ray-rules-ru/releases/download/$geoVersion/geosite.dat"; sha256 = 'f404e3ef2f68a167cab0a4383bc239e9cdb831672982819d1549701263095b8f' }
}

foreach ($geoName in $geoFiles.Keys) {
    $geoPath = Join-Path $xrayDir $geoName
    $geoExpectedSha = $geoFiles[$geoName].sha256
    $geoCached = Join-Path $cache $geoName
    if (!(Test-Path -LiteralPath $geoCached) -or ((Get-FileHash -LiteralPath $geoCached -Algorithm SHA256).Hash.ToLowerInvariant() -ne $geoExpectedSha)) {
        Invoke-WebRequest $geoFiles[$geoName].url -OutFile $geoCached -UseBasicParsing
    }
    if ((Get-FileHash -LiteralPath $geoCached -Algorithm SHA256).Hash.ToLowerInvariant() -ne $geoExpectedSha) {
        throw "Integrity verification failed for $geoName"
    }
    Copy-Item -LiteralPath $geoCached -Destination $geoPath -Force
}

$binary = Join-Path $xrayDir 'xray.exe'
if (!(Test-Path -LiteralPath $binary)) { throw 'Missing verified Xray executable' }

$manifest = @{
    version = $version
    architecture = 'x86_64'
    archiveSha256 = $sha256
    files = @{
        'xray.exe'   = (Get-FileHash -LiteralPath (Join-Path $xrayDir 'xray.exe') -Algorithm SHA256).Hash.ToLowerInvariant()
        'geoip.dat'  = (Get-FileHash -LiteralPath (Join-Path $xrayDir 'geoip.dat') -Algorithm SHA256).Hash.ToLowerInvariant()
        'geosite.dat'= (Get-FileHash -LiteralPath (Join-Path $xrayDir 'geosite.dat') -Algorithm SHA256).Hash.ToLowerInvariant()
        'wintun.dll' = (Get-FileHash -LiteralPath (Join-Path $driversDir 'wintun.dll') -Algorithm SHA256).Hash.ToLowerInvariant()
        'xray.txt'   = (Get-FileHash -LiteralPath (Join-Path $licensesDir 'xray.txt') -Algorithm SHA256).Hash.ToLowerInvariant()
    }
}

$json = $manifest | ConvertTo-Json -Depth 4
[IO.File]::WriteAllText((Join-Path $xrayDir 'manifest.json'), $json, [Text.UTF8Encoding]::new($false))

# Clean up legacy resources/core folder if it exists
$legacyCore = Join-Path $root 'src-tauri/resources/core'
if (Test-Path -LiteralPath $legacyCore) {
    Remove-Item -LiteralPath $legacyCore -Recurse -Force
}
$legacyResources = Join-Path $root 'src-tauri/resources'
if ((Test-Path -LiteralPath $legacyResources) -and (Get-ChildItem -LiteralPath $legacyResources).Count -eq 0) {
    Remove-Item -LiteralPath $legacyResources -Force
}

Write-Output "Verified Xray $version (x86_64) prepared in $runtimeRoot."
