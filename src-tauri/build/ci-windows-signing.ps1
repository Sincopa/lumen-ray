param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('Import', 'Verify', 'Cleanup')]
    [string]$Mode
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
if ([string]::IsNullOrWhiteSpace($env:RUNNER_TEMP)) { throw 'RUNNER_TEMP is required.' }
$runnerDirectory = [IO.Path]::GetFullPath($env:RUNNER_TEMP)
$statePath = Join-Path $runnerDirectory 'lumen-windows-signing.json'
$signer = Join-Path $PSScriptRoot 'sign-windows.ps1'

if ($Mode -eq 'Cleanup') {
    if (Test-Path -LiteralPath $statePath) {
        $state = Get-Content -LiteralPath $statePath -Raw | ConvertFrom-Json
        if ($state.thumbprint -notmatch '^[a-fA-F0-9]{40}$') { throw 'Invalid signing cleanup state.' }
        foreach ($entry in $state.createdStores) {
            if ($entry -in @('My', 'Root', 'TrustedPublisher')) { $entry = "CurrentUser/$entry" }
            if ($entry -notmatch '^(CurrentUser|LocalMachine)/(My|Root|TrustedPublisher)$') { throw 'Invalid cleanup store.' }
            $location, $store = $entry.Split('/')
            if ($store -eq 'My' -and $location -ne 'CurrentUser') { throw 'Unexpected private key store.' }
            $certificatePath = "Cert:\$location\$store\$($state.thumbprint)"
            if (Test-Path -LiteralPath $certificatePath) {
                if ($store -eq 'My') { Remove-Item -LiteralPath $certificatePath -DeleteKey -Force }
                else { Remove-Item -LiteralPath $certificatePath -Force }
            }
        }
        Remove-Item -LiteralPath $statePath -Force
    }
    Write-Host 'CI signing credentials removed from the runner.'
    return
}

if ($env:LUMEN_SIGNING_THUMBPRINT -notmatch '^[a-fA-F0-9]{40}$') { throw 'Set repository variable WINDOWS_CERTIFICATE_THUMBPRINT.' }
if ($Mode -eq 'Import') {
    if ([string]::IsNullOrWhiteSpace($env:WINDOWS_CERTIFICATE_PFX_BASE64) -or [string]::IsNullOrWhiteSpace($env:WINDOWS_CERTIFICATE_PASSWORD)) {
        throw 'Repository secrets WINDOWS_CERTIFICATE_PFX_BASE64 and WINDOWS_CERTIFICATE_PASSWORD are required.'
    }
    $pfxPath = Join-Path $runnerDirectory ('lumen-signing-' + [guid]::NewGuid().ToString('N') + '.pfx')
    try {
        Write-Host 'Validating the pinned release PFX.'
        [IO.File]::WriteAllBytes($pfxPath, [Convert]::FromBase64String($env:WINDOWS_CERTIFICATE_PFX_BASE64))
        $password = ConvertTo-SecureString $env:WINDOWS_CERTIFICATE_PASSWORD -AsPlainText -Force
        $pfx = Get-PfxData -FilePath $pfxPath -Password $password
        if ($pfx.EndEntityCertificates.Count -ne 1 -or $pfx.EndEntityCertificates[0].Thumbprint -ne $env:LUMEN_SIGNING_THUMBPRINT) {
            throw 'The PFX does not match the pinned release certificate.'
        }
        $trustScope = 'CurrentUser'
        if ($env:LUMEN_SIGNING_TRUST_SCOPE) { $trustScope = $env:LUMEN_SIGNING_TRUST_SCOPE }
        if ($trustScope -notin @('CurrentUser', 'LocalMachine')) { throw 'Invalid LUMEN_SIGNING_TRUST_SCOPE.' }
        $stores = @('CurrentUser/My', "$trustScope/Root", "$trustScope/TrustedPublisher")
        $createdStores = @($stores | Where-Object {
            $location, $store = $_.Split('/')
            -not (Test-Path -LiteralPath "Cert:\$location\$store\$env:LUMEN_SIGNING_THUMBPRINT")
        })
        @{ thumbprint = $env:LUMEN_SIGNING_THUMBPRINT; createdStores = $createdStores } | ConvertTo-Json | Set-Content -LiteralPath $statePath -Encoding UTF8
        # Omit -Exportable: the imported runner key cannot be exported again.
        Write-Host 'Importing the private key into CurrentUser/My.'
        Import-PfxCertificate -FilePath $pfxPath -Password $password -CertStoreLocation Cert:\CurrentUser\My | Out-Null
        Write-Host "Initializing $trustScope certificate trust and SignTool."
        $initializeArguments = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $signer, '-Mode', 'Initialize')
        if ($trustScope -eq 'LocalMachine') { $initializeArguments = @('-NonInteractive') + $initializeArguments }
        & powershell @initializeArguments
        if ($LASTEXITCODE -ne 0) { throw 'The imported release certificate failed validation.' }
        $toolPath = & powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $signer -Mode ToolPath
        if ($LASTEXITCODE -ne 0) { throw 'Windows SDK SignTool is missing.' }
        if ($env:GITHUB_ENV) { "TAURI_WINDOWS_SIGNTOOL_PATH=$toolPath" | Out-File -LiteralPath $env:GITHUB_ENV -Encoding UTF8 -Append }
        Write-Host "Imported release certificate: $env:LUMEN_SIGNING_THUMBPRINT"
    } finally {
        if (Test-Path -LiteralPath $pfxPath) { Remove-Item -LiteralPath $pfxPath -Force }
        $env:WINDOWS_CERTIFICATE_PFX_BASE64 = $null
        $env:WINDOWS_CERTIFICATE_PASSWORD = $null
    }
    return
}

$version = (Get-Content -LiteralPath (Join-Path $root 'src-tauri/tauri.conf.json') -Raw | ConvertFrom-Json).version
$setup = Join-Path $root "release/Lumen-Ray-$version-Setup.exe"
$bundledSetup = Join-Path $root "src-tauri/target/release/bundle/nsis/Lumen_${version}_x64-setup.exe"
if ((Get-FileHash -LiteralPath $setup -Algorithm SHA256).Hash -ne (Get-FileHash -LiteralPath $bundledSetup -Algorithm SHA256).Hash) {
    throw 'The published Setup differs from the installer used for the updater signature.'
}
$scratch = Join-Path $runnerDirectory ('lumen-ci-package-check-' + [guid]::NewGuid().ToString('N'))
try {
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    [IO.Compression.ZipFile]::ExtractToDirectory((Join-Path $root "release/Lumen-Ray-$version-Portable.zip"), $scratch)
    $portableRoot = Join-Path $scratch 'Lumen'
    foreach ($required in @('Lumen.exe', 'runtime/xray/xray.exe')) {
        if (-not (Test-Path -LiteralPath (Join-Path $portableRoot $required))) { throw "Portable package is missing $required" }
    }
    $manifest = Get-Content -LiteralPath (Join-Path $portableRoot 'runtime/xray/manifest.json') -Raw | ConvertFrom-Json
    $sourceManifest = Get-Content -LiteralPath (Join-Path $root 'src-tauri/runtime/xray/manifest.json') -Raw | ConvertFrom-Json
    $sourceHash = (Get-FileHash -LiteralPath (Join-Path $root 'src-tauri/runtime/xray/xray.exe') -Algorithm SHA256).Hash.ToLowerInvariant()
    $portableHash = (Get-FileHash -LiteralPath (Join-Path $portableRoot 'runtime/xray/xray.exe') -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($portableHash -ne $sourceHash -or $portableHash -ne $manifest.files.'xray.exe' -or $sourceHash -ne $sourceManifest.files.'xray.exe') { throw 'The portable signed Xray does not match the compiled runtime manifest.' }
    $files = @($setup, $bundledSetup, (Join-Path $root 'src-tauri/target/release/app.exe'), (Join-Path $root 'src-tauri/runtime/xray/xray.exe'), (Join-Path $root 'artifacts/code-signing/nsis-uninstaller.exe'))
    $files += @(Get-ChildItem -LiteralPath $portableRoot -Recurse -Filter '*.exe' -File | Select-Object -ExpandProperty FullName)
    & $signer -Mode Verify -Path $files -ReportPath (Join-Path $root 'artifacts/code-signing/ci-signatures.json')
    Copy-Item -LiteralPath (Join-Path $root 'artifacts/code-signing/Lumen-SelfSigned.cer') -Destination (Join-Path $root 'release/Lumen-SelfSigned.cer') -Force
    Write-Host 'All Windows release EXEs and extracted portable EXEs passed signature verification.'
} finally {
    $resolved = [IO.Path]::GetFullPath($scratch)
    $allowedPrefix = $runnerDirectory.TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
    if (-not $resolved.StartsWith($allowedPrefix, [StringComparison]::OrdinalIgnoreCase) -or (Split-Path $resolved -Leaf) -notmatch '^lumen-ci-package-check-[0-9a-f]{32}$') { throw 'Unexpected package verification directory.' }
    if (Test-Path -LiteralPath $resolved) { Remove-Item -LiteralPath $resolved -Recurse -Force }
}
