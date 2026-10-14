param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[a-zA-Z0-9_.-]+/[a-zA-Z0-9_.-]+$')]
    [string]$Repository
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (-not (Get-Command gh -ErrorAction SilentlyContinue)) { throw 'GitHub CLI is required.' }
& gh auth status *> $null
if ($LASTEXITCODE -ne 0) { throw 'Sign in to GitHub CLI before provisioning repository secrets.' }

# Encrypted backup stays outside the checkout. The password is protected by DPAPI.
$backupDirectory = Join-Path $env:LOCALAPPDATA ('Lumen/CodeSigningCI/' + $Repository.Replace('/', '-'))
$pfxPath = Join-Path $backupDirectory 'signing.pfx'
$passwordPath = Join-Path $backupDirectory 'password.dpapi'
$identityPath = Join-Path $backupDirectory 'identity.json'
New-Item -ItemType Directory -Path $backupDirectory -Force | Out-Null

if (Test-Path -LiteralPath $identityPath) {
    $identity = Get-Content -LiteralPath $identityPath -Raw | ConvertFrom-Json
    $securePassword = (Get-Content -LiteralPath $passwordPath -Raw).Trim() | ConvertTo-SecureString
} else {
    if ((Test-Path -LiteralPath $pfxPath) -or (Test-Path -LiteralPath $passwordPath)) {
        throw 'An incomplete CI signing backup exists. Restore it instead of rotating the release identity.'
    }
    $certificate = New-SelfSignedCertificate -Type CodeSigningCert `
        -Subject 'CN=Lumen Release Code Signing, O=Lumen' -FriendlyName "Lumen GitHub Actions ($Repository)" `
        -CertStoreLocation Cert:\CurrentUser\My -HashAlgorithm SHA256 `
        -KeyAlgorithm RSA -KeyLength 3072 -KeyUsage DigitalSignature `
        -KeyExportPolicy Exportable -Provider 'Microsoft Software Key Storage Provider' `
        -NotBefore (Get-Date).AddMinutes(-10) -NotAfter (Get-Date).AddYears(3)
    $randomBytes = New-Object byte[] 48
    $rng = [Security.Cryptography.RandomNumberGenerator]::Create()
    try { $rng.GetBytes($randomBytes) } finally { $rng.Dispose() }
    $securePassword = ConvertTo-SecureString ([Convert]::ToBase64String($randomBytes)) -AsPlainText -Force
    $securePassword | ConvertFrom-SecureString | Set-Content -LiteralPath $passwordPath -Encoding ASCII
    Export-PfxCertificate -Cert $certificate -FilePath $pfxPath -Password $securePassword -CryptoAlgorithmOption AES256_SHA256 | Out-Null
    Export-Certificate -Cert $certificate -FilePath (Join-Path $backupDirectory 'Lumen-Release.cer') | Out-Null
    $identity = @{ thumbprint = $certificate.Thumbprint; subject = $certificate.Subject; repository = $Repository; expires = $certificate.NotAfter.ToUniversalTime().ToString('o') }
    $identity | ConvertTo-Json | Set-Content -LiteralPath $identityPath -Encoding UTF8
}

$pfx = Get-PfxData -FilePath $pfxPath -Password $securePassword
if ($pfx.EndEntityCertificates.Count -ne 1 -or $pfx.EndEntityCertificates[0].Thumbprint -ne $identity.thumbprint -or $pfx.EndEntityCertificates[0].NotAfter -le (Get-Date)) {
    throw 'The stored CI PFX is invalid, expired, or differs from the recorded identity.'
}

$passwordPointer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($securePassword)
try {
    $plainPassword = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($passwordPointer)
    # Values go through stdin, never process arguments, logs, or repository files.
    [Convert]::ToBase64String([IO.File]::ReadAllBytes($pfxPath)) | & gh secret set WINDOWS_CERTIFICATE_PFX_BASE64 --repo $Repository
    if ($LASTEXITCODE -ne 0) { throw 'Could not save WINDOWS_CERTIFICATE_PFX_BASE64.' }
    $plainPassword | & gh secret set WINDOWS_CERTIFICATE_PASSWORD --repo $Repository
    if ($LASTEXITCODE -ne 0) { throw 'Could not save WINDOWS_CERTIFICATE_PASSWORD.' }
    & gh variable set WINDOWS_CERTIFICATE_THUMBPRINT --repo $Repository --body $identity.thumbprint
    if ($LASTEXITCODE -ne 0) { throw 'Could not save WINDOWS_CERTIFICATE_THUMBPRINT.' }
} finally {
    [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($passwordPointer)
    $plainPassword = $null
}
Write-Host "Configured signing secrets for $Repository. Certificate: $($identity.thumbprint)"
Write-Host "Encrypted local backup: $backupDirectory"
