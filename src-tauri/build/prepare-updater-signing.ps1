$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ([string]::IsNullOrWhiteSpace($env:TAURI_SIGNING_PRIVATE_KEY)) { throw 'TAURI_SIGNING_PRIVATE_KEY is required.' }
if ([string]::IsNullOrWhiteSpace($env:RUNNER_TEMP) -or [string]::IsNullOrWhiteSpace($env:GITHUB_ENV)) { throw 'RUNNER_TEMP and GITHUB_ENV are required.' }
$cli = Join-Path $PSScriptRoot '../../node_modules/@tauri-apps/cli/tauri.js'
$probe = Join-Path $env:RUNNER_TEMP ('lumen-updater-check-' + [guid]::NewGuid().ToString('N') + '.txt')
$previousPassword = $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD
$passwords = @('')
if (-not [string]::IsNullOrEmpty($previousPassword)) { $passwords = @($previousPassword, '') }
$selected = $false
try {
    [IO.File]::WriteAllText($probe, 'lumen updater signing check')
    foreach ($password in $passwords) {
        $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $password
        $arguments = @($cli, 'signer', 'sign', $probe)
        # An explicit empty flag prevents prompting when Windows removes an
        # empty environment variable. Non-empty passwords stay out of argv.
        if ([string]::IsNullOrEmpty($password)) { $arguments += '--password=' }
        $preference = $ErrorActionPreference
        try {
            $ErrorActionPreference = 'Continue'
            $null = & node @arguments 2>&1
            $code = $LASTEXITCODE
        } finally { $ErrorActionPreference = $preference }
        if ($code -ne 0) { continue }
        $delimiter = 'LUMEN_UPDATER_PASSWORD_' + [guid]::NewGuid().ToString('N')
        $environmentValue = @("TAURI_SIGNING_PRIVATE_KEY_PASSWORD<<$delimiter", $password, $delimiter, '') -join [Environment]::NewLine
        [IO.File]::AppendAllText($env:GITHUB_ENV, $environmentValue, [Text.UTF8Encoding]::new($false))
        $selected = $true
        if ([string]::IsNullOrEmpty($password)) { Write-Host 'Updater key verified with its legacy empty passphrase.' }
        else { Write-Host 'Updater key verified with the configured passphrase.' }
        break
    }
    if (-not $selected) { throw 'The updater key cannot be unlocked with the configured or legacy empty passphrase.' }
} finally {
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $previousPassword
    foreach ($file in @($probe, "$probe.sig")) {
        if (Test-Path -LiteralPath $file) { Remove-Item -LiteralPath $file -Force }
    }
}
