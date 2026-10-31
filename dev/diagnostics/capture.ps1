[CmdletBinding()]
param(
    [string]$Executable = (Join-Path $PSScriptRoot '../../build/jellium-desktop.exe'),
    [Parameter(Mandatory)][string]$OutputDirectory,
    [ValidateScript({ $_ -gt 0 -and -not [double]::IsInfinity($_) })][double]$IntervalSeconds = 5,
    [ValidateScript({ $_ -gt 0 -and -not [double]::IsInfinity($_) })][double]$DurationSeconds,
    [switch]$Launch,
    [switch]$Once
)
$ErrorActionPreference = 'Stop'
function Get-NormalizedPath([string]$Value) {
    if ($Value.StartsWith('\\?\UNC\', [StringComparison]::OrdinalIgnoreCase)) { $Value = '\\' + $Value.Substring(8) }
    elseif ($Value.StartsWith('\\?\')) { $Value = $Value.Substring(4) }
    [IO.Path]::GetFullPath($Value)
}
$executablePath = Get-NormalizedPath (Resolve-Path -LiteralPath $Executable).Path
$outputPath = [IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path -LiteralPath $outputPath) { throw 'Choose a new output directory; existing captures are never overwritten.' }
# Access checks happen before launching or creating the capture.
$null = Get-CimInstance Win32_Process -Filter "Name = 'jellium-desktop.exe'"
New-Item -ItemType Directory -Path $outputPath | Out-Null
$utf8 = [Text.UTF8Encoding]::new($false)
$writer = [IO.StreamWriter]::new((Join-Path $outputPath 'samples.jsonl'), $false, $utf8)
$writer.AutoFlush = $true
$clock = [Diagnostics.Stopwatch]::StartNew()
function Write-Record($Record) { $writer.WriteLine(($Record | ConvertTo-Json -Compress -Depth 8)) }
function Get-TargetProcesses {
    @(Get-CimInstance Win32_Process -Filter "Name = 'jellium-desktop.exe'" |
        Where-Object { $_.ExecutablePath -and (Get-NormalizedPath $_.ExecutablePath) -eq $executablePath })
}
try {
    Write-Record @{ type = 'metadata'; schema = 1; time = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds(); intervalSeconds = $IntervalSeconds; automaticActions = [bool]$Launch }
    $launched = $null
    if ($Launch) {
        if ((Get-TargetProcesses).Count) { throw 'Close the existing app before using -Launch, or attach without -Launch (manual markers only).' }
        $previous = $env:JELLIUM_DIAGNOSTICS_EVENTS
        try {
            $env:JELLIUM_DIAGNOSTICS_EVENTS = Join-Path $outputPath 'actions.jsonl'
            $launched = Start-Process -FilePath $executablePath -WorkingDirectory (Split-Path $executablePath) -PassThru -WindowStyle Hidden
        } finally { $env:JELLIUM_DIAGNOSTICS_EVENTS = $previous }
    }
    Write-Host "Recording to $outputPath. Stop with Ctrl+C or create a STOP file in that directory."
    do {
        $sampleStart = $clock.Elapsed.TotalSeconds
        $time = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
        $processes = @()
        $processErrors = 0
        foreach ($item in (Get-TargetProcesses)) {
            try {
                $process = [Diagnostics.Process]::GetProcessById($item.ProcessId)
                try {
                    # Guard against PID reuse between the CIM and Process snapshots.
                    $created = ([DateTimeOffset]$item.CreationDate).ToUnixTimeMilliseconds()
                    # CIM timestamps have microsecond precision, Process.StartTime has 100 ns precision.
                    if ([Math]::Floor([decimal]$process.StartTime.ToUniversalTime().Ticks / 10) -ne [Math]::Floor([decimal]$item.CreationDate.ToUniversalTime().Ticks / 10)) { continue }
                    $role = 'browser'
                    if ($item.CommandLine -match '--type=([^\s"]+)') {
                        $role = switch ($Matches[1]) { 'renderer' { 'renderer' } 'gpu-process' { 'gpu' } default { 'utility' } }
                    }
                    $processes += @{ pid = [int]$item.ProcessId; created = $created; role = $role; privateBytes = $process.PrivateMemorySize64; workingSetBytes = $process.WorkingSet64; handles = $process.HandleCount; threads = $process.Threads.Count; cpuSeconds = $process.TotalProcessorTime.TotalSeconds }
                } finally { $process.Dispose() }
            } catch { $processErrors++ }
        }
        $ids = @($processes | ForEach-Object { $_.pid })
        $gpuMemory = $null
        $gpuEngines = $null
        try {
            $gpuMemory = @(Get-CimInstance Win32_PerfFormattedData_GPUPerformanceCounters_GPUProcessMemory | ForEach-Object {
                if ($_.Name -match '^pid_(\d+)_(luid_[\da-fx]+_[\da-fx]+_phys_\d+)$' -and [int]$Matches[1] -in $ids) {
                    @{ pid = [int]$Matches[1]; adapter = $Matches[2]; dedicatedBytes = [double]$_.DedicatedUsage; sharedBytes = [double]$_.SharedUsage }
                }
            })
        } catch { $gpuMemory = $null }
        try {
            $gpuEngines = @(Get-CimInstance Win32_PerfFormattedData_GPUPerformanceCounters_GPUEngine | ForEach-Object {
                if ($_.Name -match '^pid_(\d+)_(luid_[\da-fx]+_[\da-fx]+_phys_\d+)_eng_(\d+)_engtype_([a-zA-Z0-9_]+)$' -and [int]$Matches[1] -in $ids) {
                    @{ pid = [int]$Matches[1]; adapter = $Matches[2]; engine = $Matches[3]; kind = $Matches[4]; percent = [double]$_.UtilizationPercentage }
                }
            })
        } catch { $gpuEngines = $null }
        Write-Record @{ type = 'sample'; time = $time; elapsedSeconds = $sampleStart; collectionSeconds = $clock.Elapsed.TotalSeconds - $sampleStart; processes = $processes; processErrors = $processErrors; gpuMemory = $gpuMemory; gpuEngines = $gpuEngines }
        if ($Once -or (Test-Path (Join-Path $outputPath 'STOP')) -or ($launched -and $launched.HasExited)) { break }
        if ($DurationSeconds -and $clock.Elapsed.TotalSeconds -ge $DurationSeconds) { break }
        $remaining = $IntervalSeconds - ($clock.Elapsed.TotalSeconds - $sampleStart)
        if ($DurationSeconds) { $remaining = [Math]::Min($remaining, $DurationSeconds - $clock.Elapsed.TotalSeconds) }
        if ($remaining -gt 0) { Start-Sleep -Milliseconds ([int][Math]::Ceiling($remaining * 1000)) }
    } while (-not $DurationSeconds -or $clock.Elapsed.TotalSeconds -lt $DurationSeconds)
} catch {
    Write-Record @{ type = 'capture-error'; time = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds(); reason = 'collector-failed' }
    throw
} finally {
    Write-Record @{ type = 'capture-stop'; time = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() }
    $writer.Dispose()
    if ($launched) { $launched.Dispose() }
    Write-Host "Build the report: node dev/diagnostics/report.cjs `"$outputPath`""
}
