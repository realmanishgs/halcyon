# Measures Halcyon's performance budgets on this machine (release build):
#   - cold start: process start -> UI script running (budget < 1000 ms)
#   - idle CPU with nothing playing, app + its WebView2 processes (budget < 1 %)
#   - CPU while playing a 1080p H.264 file (informational)
# Usage: powershell -File scripts/perf-check.ps1   (after `npx tauri build --no-bundle`)
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
$exe = Join-Path $root "src-tauri\target\release\halcyon.exe"
$media = Join-Path $root "test-media\formats\mp4_h264_aac.mp4"
if (-not (Test-Path $exe)) { throw "Build first: npx tauri build --no-bundle" }
Get-Process halcyon -ErrorAction SilentlyContinue | Stop-Process -Force
$cores = [Environment]::ProcessorCount

function Tree($procId) {
  # halcyon.exe and every process it spawned (WebView2 runs out of process).
  $all = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId
  $ids = @($procId); $added = $true
  while ($added) {
    $added = $false
    foreach ($p in $all) { if ($ids -contains $p.ParentProcessId -and -not ($ids -contains $p.ProcessId)) { $ids += $p.ProcessId; $added = $true } }
  }
  $ids
}
function CpuSeconds($ids) { ($ids | ForEach-Object { (Get-Process -Id $_ -ErrorAction SilentlyContinue).CPU } | Measure-Object -Sum).Sum }
function CpuPercent($procId, $seconds) {
  $ids = Tree $procId
  $a = CpuSeconds $ids; Start-Sleep -Seconds $seconds; $b = CpuSeconds $ids
  [math]::Round(100 * ($b - $a) / $seconds / $cores, 2)
}

$starts = @()
foreach ($i in 1..3) {
  $out = New-TemporaryFile
  $env:HALCYON_PERF = "1"
  $p = Start-Process $exe -PassThru -RedirectStandardOutput $out
  $t = Get-Date
  while (((Get-Date) - $t).TotalSeconds -lt 15 -and -not (Select-String -Path $out -Pattern "ui_ready_ms" -Quiet)) { Start-Sleep -Milliseconds 50 }
  $line = Select-String -Path $out -Pattern "ui_ready_ms=(\d+)" | Select-Object -First 1
  if ($line) { $starts += [int]$line.Matches[0].Groups[1].Value }
  if ($i -lt 3) { Stop-Process -Id $p.Id -Force; Start-Sleep -Milliseconds 800 }
  Remove-Item $out -ErrorAction SilentlyContinue
}
Remove-Item Env:HALCYON_PERF
Start-Sleep -Seconds 5
$idle = CpuPercent $p.Id 20
$mem = [math]::Round(((Tree $p.Id) | ForEach-Object { (Get-Process -Id $_ -ErrorAction SilentlyContinue).WorkingSet64 } | Measure-Object -Sum).Sum / 1MB)
Stop-Process -Id $p.Id -Force; Start-Sleep -Milliseconds 800

$play = "n/a"
if (Test-Path $media) {
  $p = Start-Process $exe -ArgumentList "`"$media`"" -PassThru
  Start-Sleep -Seconds 3
  $play = CpuPercent $p.Id 3
  Stop-Process -Id $p.Id -Force
}

$cold = ($starts | Measure-Object -Average -Maximum)
"| Budget | Measured | Result |"
"|---|---|---|"
"| Cold start (process -> UI running) < 1000 ms | runs: $($starts -join ', ') ms (max $($cold.Maximum)) | $(if ($cold.Maximum -lt 1000) { 'PASS' } else { 'FAIL' }) |"
"| Idle CPU, nothing playing, app + WebView2 < 1 % | $idle % of $cores cores over 20 s | $(if ($idle -lt 1) { 'PASS' } else { 'FAIL' }) |"
"| Memory idle (working set, app + WebView2) | $mem MB | info |"
"| CPU playing 1080p H.264 | $play % | info |"
