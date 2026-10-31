# Resource diagnostics (Windows)

Build the client first (`just build`). Close the existing client, then in PowerShell:

```powershell
just diagnose build/diagnostics/session-01
```

This launches the client with an opt-in action timeline and samples every 5 seconds.
It ends when the launched app exits. Alternatively create `STOP` in the session
directory, or press Ctrl+C. Stopping capture does not terminate the app. The
collector creates a fresh directory and refuses to overwrite an existing capture.

To attach to the already running build (without automatic action markers):

```powershell
pwsh -File dev/diagnostics/capture.ps1 -OutputDirectory build/diagnostics/attached
```

Use `-Executable` to select another build, `-DurationSeconds` for an explicit
duration, `-IntervalSeconds` to override the approved default, or `-Once` for a
single snapshot. All processes with that executable path are included. Close other
instances of that build if testing a single session. CIM permissions and GPU
performance counters depend on Windows policy/driver; unavailable GPU counters
produce gaps, while failure to enumerate processes stops the collector visibly.

During capture, from a second terminal:

```powershell
pwsh -File dev/diagnostics/mark.ps1 -Directory build/diagnostics/session-01 -Action baseline
pwsh -File dev/diagnostics/mark.ps1 -Directory build/diagnostics/session-01 -Action switch-film
```

Other manual markers: play, pause, resume, seek, fullscreen, resize, idle, stop.
Automatic markers come from observed mpv events (load, file-loaded, file-end,
pause/resume, seek/restart) and native settings/About actions. Pause/resume includes
initial observed state; a marker is not proof of a user gesture or visible frame.
No synchronous mpv queries are made. The action writer uses tracing-appender's
bounded default queue; detected record drops are reported. Abrupt termination can
lose the final queued records, so the report checks for a clean app-stop marker.

Generate a self-contained HTML report (Node.js required):

```powershell
just diagnostic-report build/diagnostics/session-01
```

Open `report.html`. It works offline, with hover values, action lines, a UTC event
table and individual process/engine visibility controls. Regenerate after capture
and the app both stop to include the final action records. Raw `samples.jsonl`,
`actions.jsonl` and `manual.jsonl` remain available for further analysis. Only
numeric counters and fixed labels are exported: no ordinary application logs,
command lines, server addresses, media names or credentials. Process identities
include creation timestamps to distinguish PID reuse. GPU counter snapshots are
not atomic with process snapshots; collection duration is recorded in each sample.

Private bytes measure process commitment, not JS heap size. Working sets include
shared pages. GPU counters describe each process/adapter/engine separately; engine
percentages must not be summed into a total. Absence of a GPU instance is a gap,
not measured zero usage. Capture the same sequence after warm-up, compare recovery
after stopping playback, and repeat before attributing retained memory to a leak.
For attribution, follow up with JS heap snapshots/native or GPU profiling.

CI runs report/bridge regression tests, a Windows collector smoke test, Rust
format/clippy/tests. It cannot replace hardware playback, sleep/resume, GPU reset
or prolonged switching tests. No arbitrary memory or GPU pass/fail budget is set.
