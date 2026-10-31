param(
    [Parameter(Mandatory)][string]$Directory,
    [Parameter(Mandatory)][ValidateSet('baseline', 'play', 'switch-film', 'pause', 'resume', 'seek', 'fullscreen', 'resize', 'idle', 'stop')][string]$Action
)
$ErrorActionPreference = 'Stop'
$path = (Resolve-Path -LiteralPath $Directory).Path
if (-not (Test-Path (Join-Path $path 'samples.jsonl'))) { throw 'Not a capture directory.' }
$record = @{ type = 'action'; time = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds(); action = $Action; source = 'manual' } | ConvertTo-Json -Compress
[IO.File]::AppendAllText((Join-Path $path 'manual.jsonl'), $record + "`n", [Text.UTF8Encoding]::new($false))
