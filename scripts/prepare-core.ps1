param()
$ErrorActionPreference = 'Stop'
$version = '26.9.9'
$sha256 = '244deaba2098c2964e49bba90df3707777e5f5f428a82d2f29604015f24beec2'
$root = Split-Path $PSScriptRoot -Parent
$destination = Join-Path $root 'src-tauri/resources/core'
$cache = Join-Path $root 'artifacts/core-download'
New-Item -ItemType Directory -Force -Path $destination,$cache | Out-Null
$archive = Join-Path $cache 'Xray-windows-64.zip'
if (!(Test-Path -LiteralPath $archive)) {
    Invoke-WebRequest "https://github.com/XTLS/Xray-core/releases/download/v$version/Xray-windows-64.zip" -OutFile $archive -UseBasicParsing
}
if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $sha256) { throw 'Xray archive integrity verification failed' }
Add-Type -AssemblyName System.IO.Compression.FileSystem
$zip = [IO.Compression.ZipFile]::OpenRead($archive)
try {
    foreach ($name in @('xray.exe','wintun.dll','LICENSE')) {
        $entry = $zip.Entries | Where-Object FullName -eq $name
        if ($entry) { [IO.Compression.ZipFileExtensions]::ExtractToFile($entry, (Join-Path $destination $name), $true) }
    }
} finally { $zip.Dispose() }

# Download and verify Russian GeoIP and GeoSite database files (NetworkKeeper/v2ray-rules-ru)
$geoVersion = '202410152325'
$geoFiles = @{
    'geoip.dat'   = @{ url = "https://github.com/NetworkKeeper/v2ray-rules-ru/releases/download/$geoVersion/geoip.dat";   sha256 = '9315c8677c101db1097cc16a53c47cd315e576a6b50734b6a9eed72862e32402' }
    'geosite.dat' = @{ url = "https://github.com/NetworkKeeper/v2ray-rules-ru/releases/download/$geoVersion/geosite.dat"; sha256 = 'f404e3ef2f68a167cab0a4383bc239e9cdb831672982819d1549701263095b8f' }
}
foreach ($geoName in $geoFiles.Keys) {
    $geoPath = Join-Path $destination $geoName
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

$binary = Join-Path $destination 'xray.exe'
if (!(Test-Path -LiteralPath $binary)) { throw 'Missing verified Xray executable' }
$manifest = @{ version=$version; architecture='x86_64'; archiveSha256=$sha256; files=@{} }
Get-ChildItem -LiteralPath $destination -File | Where-Object Name -ne 'manifest.json' | ForEach-Object {
    $manifest.files[$_.Name] = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
}
$json = $manifest | ConvertTo-Json -Depth 4
[IO.File]::WriteAllText((Join-Path $destination 'manifest.json'), $json, [Text.UTF8Encoding]::new($false))
Write-Output "Verified Xray $version (x86_64) prepared."
