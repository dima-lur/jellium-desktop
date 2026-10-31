param([Parameter(Mandatory)][string]$OutputRoot)
$ErrorActionPreference = 'Stop'
# Exercise real Process metrics with controlled CIM identities/counters. The
# collector is invoked unchanged; no dependency on a Jellyfin server or GPU.
$fixtureProcess = [Diagnostics.Process]::GetCurrentProcess()
$fixtureExecutable = $fixtureProcess.MainModule.FileName
$fixtureGpuAvailable = $true
function Get-CimInstance {
    param([string]$ClassName, [string]$Filter)
    switch ($ClassName) {
        'Win32_Process' {
            [pscustomobject]@{ ProcessId = $fixtureProcess.Id; CreationDate = $fixtureProcess.StartTime; ExecutablePath = '\\?\' + $fixtureExecutable; CommandLine = '--type=renderer SECRET_NOT_FOR_EXPORT' }
        }
        'Win32_PerfFormattedData_GPUPerformanceCounters_GPUProcessMemory' {
            if (-not $fixtureGpuAvailable) { throw 'Counters unavailable' }
            [pscustomobject]@{ Name = "pid_$($fixtureProcess.Id)_luid_0x0_0xab_phys_0"; DedicatedUsage = 1024; SharedUsage = 2048 }
            [pscustomobject]@{ Name = 'pid_0_luid_0x0_0xab_phys_0'; DedicatedUsage = 99999; SharedUsage = 99999 }
        }
        'Win32_PerfFormattedData_GPUPerformanceCounters_GPUEngine' {
            if (-not $fixtureGpuAvailable) { throw 'Counters unavailable' }
            [pscustomobject]@{ Name = "pid_$($fixtureProcess.Id)_luid_0x0_0xab_phys_0_eng_1_engtype_VideoDecode"; UtilizationPercentage = 12 }
        }
        default { throw "Unexpected class: $ClassName" }
    }
}
try {
    foreach ($mode in @('available', 'unavailable')) {
        $fixtureGpuAvailable = $mode -eq 'available'
        $directory = Join-Path $OutputRoot $mode
        & "$PSScriptRoot/capture.ps1" -Executable $fixtureExecutable -OutputDirectory $directory -Once
        $raw = Get-Content (Join-Path $directory 'samples.jsonl') -Raw
        if ($raw.Contains('SECRET_NOT_FOR_EXPORT') -or $raw.Contains($fixtureExecutable)) { throw 'Sensitive fields exported' }
        $records = Get-Content (Join-Path $directory 'samples.jsonl') | ConvertFrom-Json
        $sample = $records | Where-Object type -eq sample
        if ($sample.processes.Count -ne 1 -or $sample.processes[0].privateBytes -le 0 -or $sample.processes[0].role -ne 'renderer') { throw 'Process metrics/identity missing' }
        if ($fixtureGpuAvailable) {
            if ($sample.gpuMemory.Count -ne 1 -or $sample.gpuMemory[0].dedicatedBytes -ne 1024 -or $sample.gpuEngines[0].percent -ne 12) { throw 'GPU attribution failed' }
        } elseif ($null -ne $sample.gpuMemory -or $null -ne $sample.gpuEngines) { throw 'Unavailable counters must be null' }
        & "$PSScriptRoot/mark.ps1" -Directory $directory -Action baseline
        node "$PSScriptRoot/report.cjs" $directory
        if ($LASTEXITCODE) { throw 'Report failed' }
        $refused = $false
        try { & "$PSScriptRoot/capture.ps1" -Executable $fixtureExecutable -OutputDirectory $directory -Once } catch { $refused = $true }
        if (-not $refused) { throw 'Existing capture was overwritten' }
    }
    Write-Host 'Collector contract tests passed.'
} finally { $fixtureProcess.Dispose() }
