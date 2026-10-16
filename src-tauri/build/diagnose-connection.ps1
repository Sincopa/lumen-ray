param(
    [string]$OutputDirectory = (Join-Path ([IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))) 'diagnostics')
)

$ErrorActionPreference = 'Stop'
$appData = Join-Path $env:LOCALAPPDATA 'app.lumen.desktop'
$appLog = Join-Path $appData 'logs\Lumen.log'
$sessionDirectory = Join-Path $OutputDirectory ("connection-" + (Get-Date -Format 'yyyyMMdd-HHmmss'))
New-Item -ItemType Directory -Path $sessionDirectory -Force | Out-Null
$report = Join-Path $sessionDirectory 'report.txt'
$lastLine = if (Test-Path -LiteralPath $appLog) { @(Get-Content -LiteralPath $appLog).Count } else { 0 }

function Write-Report([string]$message) {
    Add-Content -LiteralPath $report -Value $message -Encoding UTF8
    Write-Host $message
}

function Capture-Step([string]$step) {
    Write-Report ("[{0}] STEP {1}" -f (Get-Date -Format 'yyyy-MM-dd HH:mm:ss.fff'), $step)
    $app = @(Get-Process -Name Lumen -ErrorAction SilentlyContinue)
    $tun = Get-NetAdapter -Name LumenTun -ErrorAction SilentlyContinue
    $otherVpn = Get-NetAdapter -Name Meta -ErrorAction SilentlyContinue
    Write-Report ("APP_RUNNING={0} LUMEN_TUN={1} META_TUNNEL={2}" -f ($app.Count -gt 0), $(if ($tun) { $tun.Status } else { 'absent' }), $(if ($otherVpn) { $otherVpn.Status } else { 'absent' }))
    $routes = @(Get-NetRoute -InterfaceAlias LumenTun -ErrorAction SilentlyContinue | Where-Object { $_.DestinationPrefix -in @('0.0.0.0/0', '::/0') })
    Write-Report ("LUMEN_DEFAULT_ROUTES={0}" -f $routes.Count)
    foreach ($routeCheck in @(@('IPv4_CLOUDFLARE', '1.1.1.1'), @('IPv4_GOOGLE', '8.8.8.8'), @('IPv6', '2606:4700:4700::1111'))) {
        $selectedRoute = Find-NetRoute -RemoteIPAddress $routeCheck[1] -ErrorAction SilentlyContinue
        $owner = if (-not $selectedRoute) { 'none' } elseif ($selectedRoute.InterfaceAlias -eq 'LumenTun') { 'LumenTun' } else { 'other' }
        Write-Report ("EFFECTIVE_ROUTE_{0}={1}" -f $routeCheck[0], $owner)
    }
    if (Test-Path -LiteralPath $appLog) {
        $lines = @(Get-Content -LiteralPath $appLog)
        if ($lines.Count -lt $lastLine) { $script:lastLine = 0 }
        foreach ($line in @($lines | Select-Object -Skip $lastLine)) {
            if ($line -match 'VPN timing:|local startup completed|TUN prepared|preparation failed:|connection failed code=|Connection change failed|live server|routing (update (applied|skipped|failed|retry)|command failed)|route verification failed|route selection settled|VPN cleanup incomplete|backend stopped unexpectedly|Automatic recovery stopped|Windows TUN (adapter )?startup') {
                $safe = $line -replace '(?i)(https?|vless|trojan|ss)://\S+', '[url]'
                $safe = $safe -replace '\b(?:\d{1,3}\.){3}\d{1,3}\b', '[ip]'
                $safe = $safe -replace '\b[0-9a-f]{8}-[0-9a-f-]{27,}\b', '[id]'
                Write-Report $safe
            }
        }
        $script:lastLine = $lines.Count
    }
}

Write-Report 'Диагностика Lumen. Адреса подписок, ключи и исходные журналы не копируются.'
Write-Report 'Проверяйте приложение вручную. После каждого действия вернитесь сюда и нажмите Enter.'
Capture-Step 'baseline'
foreach ($step in @(
    'подключитесь с уже выбранным сервером',
    'во время подключения выберите другой сервер',
    'переключитесь обратно на первый сервер',
    'переключите режим Правила на Глобальный',
    'переключите режим Глобальный на Правила',
    'отключитесь и подключитесь снова'
)) {
    $answer = Read-Host "В Lumen: $step. После действия нажмите Enter (или введите пропуск)"
    if ($answer -eq 'пропуск' -or $answer -eq 'skip') {
        Write-Report ("[{0}] SKIPPED {1}" -f (Get-Date -Format 'yyyy-MM-dd HH:mm:ss.fff'), $step)
    } else {
        Capture-Step $step
    }
}
$playback = Read-Host 'Проверьте открытие сайта и воспроизведение видео через VPN. Работает? (да/нет/не проверял)'
$playbackResult = switch ($playback.Trim().ToLowerInvariant()) {
    { $_ -in @('да', 'yes') } { 'yes'; break }
    { $_ -in @('нет', 'no') } { 'no'; break }
    default { 'not_checked' }
}
Write-Report ("TRAFFIC_AND_VIDEO={0}" -f $playbackResult)
Write-Report ("REPORT={0}" -f $report)
