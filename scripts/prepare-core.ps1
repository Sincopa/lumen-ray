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
$binary = Join-Path $destination 'xray.exe'
if (!(Test-Path -LiteralPath $binary)) { throw 'Missing verified Xray executable' }
$manifest = @{ version=$version; architecture='x86_64'; archiveSha256=$sha256; files=@{} }
Get-ChildItem -LiteralPath $destination -File | Where-Object Name -ne 'manifest.json' | ForEach-Object {
    $manifest.files[$_.Name] = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
}
$json = $manifest | ConvertTo-Json -Depth 4
[IO.File]::WriteAllText((Join-Path $destination 'manifest.json'), $json, [Text.UTF8Encoding]::new($false))
Write-Output "Verified Xray $version (x86_64) prepared."
