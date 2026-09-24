param([int[]]$Concurrency = @(1, 4))

$ErrorActionPreference = 'Stop'
$core = Join-Path $PSScriptRoot '../src-tauri/resources/core/xray.exe'
if (-not (Test-Path -LiteralPath $core)) { throw 'Prepare the Xray core first.' }

$scratch = Join-Path ([IO.Path]::GetTempPath()) "lumen-probe-benchmark-$([guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory -Path $scratch | Out-Null
try {
    foreach ($count in $Concurrency) {
        if ($count -lt 1 -or $count -gt 16) { throw 'Concurrency must be between 1 and 16.' }
        $processes = [System.Collections.Generic.List[System.Diagnostics.Process]]::new()
        $listeners = [System.Collections.Generic.List[System.Net.Sockets.TcpListener]]::new()
        $ports = [System.Collections.Generic.List[int]]::new()
        try {
            for ($index = 0; $index -lt $count; $index++) {
                $listener = [System.Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
                $listener.Start()
                $listeners.Add($listener)
                $ports.Add(([Net.IPEndPoint]$listener.LocalEndpoint).Port)
            }
            foreach ($listener in $listeners) { $listener.Stop() }
            $clock = [System.Diagnostics.Stopwatch]::StartNew()
            for ($index = 0; $index -lt $count; $index++) {
                $configPath = Join-Path $scratch "probe-$index.json"
                $config = @{
                    log = @{ loglevel = 'warning' }
                    inbounds = @(@{ listen = '127.0.0.1'; port = $ports[$index]; protocol = 'socks'; settings = @{ auth = 'noauth' } })
                    outbounds = @(@{ protocol = 'freedom'; tag = 'direct' })
                }
                $config | ConvertTo-Json -Depth 8 -Compress | Set-Content -LiteralPath $configPath -Encoding Ascii
                $processes.Add((Start-Process -FilePath $core -ArgumentList "run -config `"$configPath`"" -PassThru -WindowStyle Hidden))
            }
            $ready = [System.Collections.Generic.HashSet[int]]::new()
            while ($ready.Count -lt $count -and $clock.ElapsedMilliseconds -lt 10000) {
                foreach ($port in $ports) {
                    if ($ready.Contains($port)) { continue }
                    $client = [System.Net.Sockets.TcpClient]::new()
                    try {
                        $attempt = $client.BeginConnect('127.0.0.1', $port, $null, $null)
                        if ($attempt.AsyncWaitHandle.WaitOne(25)) {
                            try { $client.EndConnect($attempt); [void]$ready.Add($port) } catch { }
                        }
                    } finally { $client.Dispose() }
                }
                Start-Sleep -Milliseconds 25
            }
            $startupMs = $clock.ElapsedMilliseconds
            Start-Sleep -Milliseconds 300
            $workingSetMb = 0.0
            foreach ($process in $processes) {
                $process.Refresh()
                if (-not $process.HasExited) { $workingSetMb += $process.WorkingSet64 / 1MB }
            }
            [pscustomobject]@{
                Processes = $count
                Ready = $ready.Count
                StartupMs = $startupMs
                WorkingSetMb = [math]::Round($workingSetMb, 1)
            }
        } finally {
            foreach ($process in $processes) {
                if (-not $process.HasExited) { $process.Kill(); $process.WaitForExit() }
                $process.Dispose()
            }
            foreach ($listener in $listeners) { $listener.Stop() }
        }
    }
} finally {
    Get-ChildItem -LiteralPath $scratch -File | Remove-Item -Force
    Remove-Item -LiteralPath $scratch
}
