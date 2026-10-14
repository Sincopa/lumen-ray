param(
    [ValidateSet('Initialize', 'ToolPath', 'Runtime', 'Sign', 'Verify')]
    [string]$Mode = 'Sign',
    [string[]]$Path = @(),
    [string]$ReportPath = '',
    [switch]$TauriHook,
    [uri]$TimestampUrl = 'http://timestamp.digicert.com'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$identityDirectory = Join-Path $env:LOCALAPPDATA 'Lumen/CodeSigning'
$identityPath = Join-Path $identityDirectory 'identity.json'
$publicCertificatePath = Join-Path $identityDirectory 'Lumen-SelfSigned.cer'
$certificateSubject = 'CN=Lumen Self-Signed Code Signing, O=Lumen'
$certificateName = 'Lumen Authenticode (self-signed)'
$evidenceDirectory = Join-Path $projectRoot 'artifacts/code-signing'
$uninstallerEvidence = Join-Path $evidenceDirectory 'nsis-uninstaller.exe'
New-Item -ItemType Directory -Path $evidenceDirectory -Force | Out-Null

function Find-SignTool {
    $sdkRoots = @("${env:ProgramFiles(x86)}/Windows Kits/10/bin", "$env:ProgramFiles/Windows Kits/10/bin")
    foreach ($sdkRoot in $sdkRoots) {
        if (-not (Test-Path -LiteralPath $sdkRoot)) { continue }
        foreach ($version in (Get-ChildItem -LiteralPath $sdkRoot -Directory | Sort-Object { try { [version]$_.Name } catch { [version]'0.0' } } -Descending)) {
            foreach ($architecture in @('x64', 'x86')) {
                $candidate = Join-Path $version.FullName "$architecture/signtool.exe"
                if (Test-Path -LiteralPath $candidate) { return $candidate }
            }
        }
    }
    $command = Get-Command signtool.exe -ErrorAction SilentlyContinue
    if ($command) { return $command.Source }
    throw 'Windows SDK SignTool is missing. Install the Windows SDK Signing Tools component and retry.'
}

function Get-SigningIdentity {
    $requestedThumbprint = $env:LUMEN_SIGNING_THUMBPRINT
    if (-not [string]::IsNullOrWhiteSpace($requestedThumbprint)) {
        if ($requestedThumbprint -notmatch '^[a-fA-F0-9]{40}$') { throw 'LUMEN_SIGNING_THUMBPRINT must be a certificate SHA-1 thumbprint.' }
        $certificate = Get-Item -LiteralPath "Cert:\CurrentUser\My\$requestedThumbprint" -ErrorAction SilentlyContinue
        if (-not $certificate) { throw 'The requested CI signing certificate is missing. Import the configured PFX before signing.' }
    } elseif ($env:GITHUB_ACTIONS -eq 'true' -or $env:CI -eq 'true') {
        throw 'CI requires LUMEN_SIGNING_THUMBPRINT and an imported signing certificate; refusing to generate a different identity.'
    } elseif (Test-Path -LiteralPath $identityPath) {
        $identity = Get-Content -LiteralPath $identityPath -Raw | ConvertFrom-Json
        $certificate = Get-Item -LiteralPath "Cert:\CurrentUser\My\$($identity.thumbprint)" -ErrorAction SilentlyContinue
        if (-not $certificate) { throw "Signing certificate is missing from CurrentUser/My: $($identity.thumbprint). Restore the original Windows user profile; do not silently rotate the signing identity." }
    } else {
        $certificate = Get-ChildItem -LiteralPath Cert:\CurrentUser\My -CodeSigningCert |
            Where-Object { $_.Subject -eq $certificateSubject -and $_.FriendlyName -eq $certificateName -and $_.HasPrivateKey } |
            Sort-Object NotAfter -Descending | Select-Object -First 1
        if (-not $certificate) {
            $certificate = New-SelfSignedCertificate -Type CodeSigningCert `
                -Subject $certificateSubject -FriendlyName $certificateName `
                -CertStoreLocation Cert:\CurrentUser\My -HashAlgorithm SHA256 `
                -KeyAlgorithm RSA -KeyLength 3072 -KeyUsage DigitalSignature `
                -KeyExportPolicy NonExportable -Provider 'Microsoft Software Key Storage Provider' `
                -NotBefore (Get-Date).AddMinutes(-10) -NotAfter (Get-Date).AddYears(3)
        }
        New-Item -ItemType Directory -Path $identityDirectory -Force | Out-Null
        @{ thumbprint = $certificate.Thumbprint; subject = $certificate.Subject; store = 'CurrentUser/My' } |
            ConvertTo-Json | Set-Content -LiteralPath $identityPath -Encoding UTF8
    }
    if (-not $certificate.HasPrivateKey -or $certificate.NotAfter -le (Get-Date) -or $certificate.NotBefore -gt (Get-Date)) {
        throw 'The Lumen signing certificate has expired, is not valid yet, or has no private key.'
    }
    if (-not ($certificate.EnhancedKeyUsageList | Where-Object { [string]$_.ObjectId -eq '1.3.6.1.5.5.7.3.3' })) {
        throw 'The Lumen certificate does not permit Code Signing.'
    }
    New-Item -ItemType Directory -Path $identityDirectory -Force | Out-Null
    $exportPath = $publicCertificatePath
    if ($requestedThumbprint) { $exportPath = Join-Path $identityDirectory "$requestedThumbprint.cer" }
    Export-Certificate -Cert $certificate -FilePath $exportPath -Force | Out-Null
    $trustScope = 'CurrentUser'
    if ($env:LUMEN_SIGNING_TRUST_SCOPE) { $trustScope = $env:LUMEN_SIGNING_TRUST_SCOPE }
    if ($trustScope -notin @('CurrentUser', 'LocalMachine')) { throw 'Invalid LUMEN_SIGNING_TRUST_SCOPE.' }
    # A hosted runner uses its temporary machine stores to avoid CurrentUser
    # root-import dialogs. Local builds keep the default CurrentUser scope.
    foreach ($storeName in @('Root', 'TrustedPublisher')) {
        if (-not (Test-Path -LiteralPath "Cert:\$trustScope\$storeName\$($certificate.Thumbprint)")) {
            Write-Host "Trusting release certificate in $trustScope/$storeName."
            Import-Certificate -FilePath $exportPath -CertStoreLocation "Cert:\$trustScope\$storeName" | Out-Null
        }
    }
    Copy-Item -LiteralPath $exportPath -Destination (Join-Path $evidenceDirectory 'Lumen-SelfSigned.cer') -Force
    return $certificate
}

function Backup-Artifact([string]$FilePath) {
    $hash = (Get-FileHash -LiteralPath $FilePath -Algorithm SHA256).Hash.ToLowerInvariant()
    $directory = Join-Path $evidenceDirectory "backups/$hash"
    $destination = Join-Path $directory (Split-Path $FilePath -Leaf)
    if (-not (Test-Path -LiteralPath $destination)) {
        New-Item -ItemType Directory -Path $directory -Force | Out-Null
        Copy-Item -LiteralPath $FilePath -Destination $destination
    }
}

function Test-LumenSignature([string]$FilePath) {
    $signature = Get-AuthenticodeSignature -LiteralPath $FilePath
    return ($signature.Status -eq 'Valid' -and $signature.SignerCertificate.Thumbprint -eq $signingCertificate.Thumbprint -and $null -ne $signature.TimeStamperCertificate)
}

function Invoke-SdkTool([string[]]$ToolArguments) {
    $previousPreference = $ErrorActionPreference
    try {
        # Windows PowerShell otherwise throws on native stderr before we can
        # report the affected file and SignTool's complete diagnostic.
        $ErrorActionPreference = 'Continue'
        $output = & $signTool @ToolArguments 2>&1
        $exitCode = $LASTEXITCODE
    } finally { $ErrorActionPreference = $previousPreference }
    if ($exitCode -ne 0) {
        throw "SignTool failed for $($ToolArguments[-1]) (exit $exitCode): $($output -join [Environment]::NewLine)"
    }
    return $output
}

function Invoke-FileSigning([string]$FilePath) {
    # NSIS passes DLLs and a temporary .tmp PE uninstaller to the same hook.
    # Check the format, since its temporary filename need not end in .exe.
    $peStream = [IO.File]::OpenRead($FilePath)
    $peReader = [IO.BinaryReader]::new($peStream)
    try {
        if ($peReader.ReadUInt16() -ne 0x5a4d) { throw "Refusing to sign a non-PE binary: $FilePath" }
        $peStream.Position = 0x3c
        $peHeader = $peReader.ReadUInt32()
        $peStream.Position = $peHeader
        if ($peReader.ReadUInt32() -ne 0x4550) { throw "Refusing to sign a non-PE binary: $FilePath" }
    } finally { $peReader.Dispose(); $peStream.Dispose() }
    if (-not (Test-LumenSignature $FilePath)) {
        $existingSignature = Get-AuthenticodeSignature -LiteralPath $FilePath
        if ($existingSignature.SignerCertificate -and $existingSignature.SignerCertificate.Thumbprint -ne $signingCertificate.Thumbprint) {
            throw "Refusing to replace another publisher's signature: $FilePath"
        }
        Backup-Artifact $FilePath
        $null = Invoke-SdkTool @('sign', '/s', 'My', '/sha1', $signingCertificate.Thumbprint, '/fd', 'SHA256', '/tr', $TimestampUrl.AbsoluteUri, '/td', 'SHA256', '/d', 'Lumen', $FilePath)
        Write-Host "Signed (SHA-256 + RFC 3161 timestamp): $FilePath"
    }
}

function Get-VerifiedSignature([string]$FilePath) {
    $null = Invoke-SdkTool @('verify', '/pa', '/all', '/v', $FilePath)
    if (-not (Test-LumenSignature $FilePath)) { throw "Invalid signer, signature or missing timestamp: $FilePath" }
    # Inspect the embedded CMS instead of relying on localized SignTool text.
    $stream = [IO.File]::OpenRead($FilePath)
    $reader = [IO.BinaryReader]::new($stream)
    try {
        $stream.Position = 0x3c
        $peOffset = $reader.ReadUInt32()
        $stream.Position = $peOffset + 24
        $magic = $reader.ReadUInt16()
        if ($magic -eq 0x20b) { $securityOffset = 112 + 32 } elseif ($magic -eq 0x10b) { $securityOffset = 96 + 32 } else { throw 'Unsupported PE format' }
        $stream.Position = $peOffset + 24 + $securityOffset
        $certificateOffset = $reader.ReadUInt32()
        $stream.Position = $certificateOffset
        $certificateSize = $reader.ReadUInt32()
        $stream.Position += 4
        $cms = [Security.Cryptography.Pkcs.SignedCms]::new()
        $cms.Decode($reader.ReadBytes($certificateSize - 8))
        $digestOid = $cms.SignerInfos[0].DigestAlgorithm.Value
        if ($digestOid -ne '2.16.840.1.101.3.4.2.1') { throw "The Authenticode file digest is not SHA-256: $FilePath" }
    } finally { $reader.Dispose(); $stream.Dispose() }
    $signature = Get-AuthenticodeSignature -LiteralPath $FilePath
    [pscustomobject]@{
        path = $FilePath
        status = [string]$signature.Status
        digestAlgorithm = 'SHA256'
        signerThumbprint = $signature.SignerCertificate.Thumbprint
        timestampSubject = $signature.TimeStamperCertificate.Subject
        sha256 = (Get-FileHash -LiteralPath $FilePath -Algorithm SHA256).Hash.ToLowerInvariant()
        signToolVerified = $true
    }
}

function Get-CurrentExecutables {
    $files = @()
    foreach ($relative in @('src-tauri/runtime', 'src-tauri/target/release/runtime', 'src-tauri/target/release/bundle', 'release')) {
        $directory = Join-Path $projectRoot $relative
        if (Test-Path -LiteralPath $directory) {
            $files += Get-ChildItem -LiteralPath $directory -Filter '*.exe' -Recurse -File |
                Where-Object { $_.FullName -notmatch '(?i)[\\/][^\\/]*(?:-old|-backup)[\\/]|[\\/][^\\/]*(?:before-fix|\.backup)\.exe$' } |
                Select-Object -ExpandProperty FullName
        }
    }
    $main = Join-Path $projectRoot 'src-tauri/target/release/app.exe'
    if (Test-Path -LiteralPath $main) { $files += $main }
    return @($files | Sort-Object -Unique)
}

$signTool = Find-SignTool
if ($Mode -eq 'ToolPath') { Write-Output $signTool; return }
Add-Type -AssemblyName System.Security
$signingCertificate = Get-SigningIdentity
if ($Mode -eq 'Initialize') {
    [Environment]::SetEnvironmentVariable('TAURI_WINDOWS_SIGNTOOL_PATH', $signTool, 'User')
    Write-Host "Certificate: $($signingCertificate.Thumbprint), valid until $($signingCertificate.NotAfter.ToString('yyyy-MM-dd'))"
    Write-Host "Public certificate: $publicCertificatePath"
    Write-Host "SignTool: $signTool"
    return
}

if ($Mode -eq 'Runtime') {
    $runtime = Join-Path $projectRoot 'src-tauri/runtime'
    $manifestPath = Join-Path $runtime 'xray/manifest.json'
    $binaryPath = Join-Path $runtime 'xray/xray.exe'
    $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    $originalHash = (Get-FileHash -LiteralPath $binaryPath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($originalHash -ne $manifest.files.'xray.exe') { throw 'Xray does not match the current runtime manifest. Run prepare-core.ps1 to restore the verified runtime.' }
    Invoke-FileSigning $binaryPath
    $signedHash = (Get-FileHash -LiteralPath $binaryPath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($signedHash -ne $manifest.files.'xray.exe') {
        Backup-Artifact $manifestPath
        $content = [IO.File]::ReadAllText($manifestPath)
        $content = [regex]::Replace($content, '("xray\.exe"\s*:\s*")[a-fA-F0-9]{64}("\s*)', "`${1}$signedHash`${2}")
        [IO.File]::WriteAllText($manifestPath, $content, [Text.UTF8Encoding]::new($false))
    }
    $Path = @($binaryPath)
} elseif ($Path.Count -eq 0) {
    $Path = @(Get-CurrentExecutables)
}

if ($Path.Count -eq 0) { throw 'No application executables found.' }
$results = @(foreach ($file in $Path) {
    $resolvedFile = (Resolve-Path -LiteralPath $file).Path
    if ($TauriHook -and (Split-Path $resolvedFile -Leaf) -eq 'app.exe') {
        # NSIS does not propagate !uninstfinalize failures. Clear the old proof
        # at the start of bundling; the final installer requires fresh proof.
        if (Test-Path -LiteralPath $uninstallerEvidence) { Remove-Item -LiteralPath $uninstallerEvidence -Force }
    }
    if ($TauriHook -and $resolvedFile -match '(?i)[\\/]bundle[\\/]nsis[\\/].*setup\.exe$') {
        if (-not (Test-Path -LiteralPath $uninstallerEvidence)) { throw 'NSIS uninstaller signing failed; refusing to sign/distribute the installer.' }
        $null = Get-VerifiedSignature $uninstallerEvidence
    }
    if ($Mode -eq 'Sign') { Invoke-FileSigning $resolvedFile }
    $verified = Get-VerifiedSignature $resolvedFile
    if ($TauriHook -and [IO.Path]::GetExtension($resolvedFile) -eq '.tmp') {
        Copy-Item -LiteralPath $resolvedFile -Destination $uninstallerEvidence -Force
        $verified | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $evidenceDirectory 'nsis-uninstaller.json') -Encoding UTF8
    }
    $verified
})
if ($ReportPath) {
    $report = @{ certificate = $signingCertificate.Thumbprint; certificateExpires = $signingCertificate.NotAfter.ToUniversalTime().ToString('o'); signTool = $signTool; checkedAt = [DateTime]::UtcNow.ToString('o'); files = $results }
    $report | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $ReportPath -Encoding UTF8
}
$results | Format-Table path, status, digestAlgorithm -AutoSize
