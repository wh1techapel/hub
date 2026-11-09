# Measures the release build against docs/architecture.md, "Resources": start time, and memory and
# idle CPU with the window open, just after it is closed, after the hidden window is let go, and
# when Hub starts in the tray.
#
#   npm run tauri build
#   powershell -ExecutionPolicy Bypass -File scripts\measure.ps1
#
# Runs on a fresh demo data folder (HUB_DATA_DIR), never the owner's. Processes: hub.exe and the
# WebView2 processes whose command line names that folder. Memory is the private working set (what
# Task Manager's Memory column shows); "commit" is private bytes, which for WebView2's GPU process
# is mostly driver reservations, not RAM in use. CPU is percent of one core.

param(
    [string]$Exe = (Join-Path $PSScriptRoot "..\target\release\hub.exe"),
    [string]$Seed = (Join-Path $PSScriptRoot "..\target\release\hub-seed.exe"),
    [int]$ReleaseAfter = 25
)

$ErrorActionPreference = "Stop"
Add-Type @"
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class HubWin {
    delegate bool EnumProc(IntPtr hwnd, IntPtr lParam);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc callback, IntPtr lParam);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr hwnd, StringBuilder text, int max);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd, uint msg, IntPtr w, IntPtr l);
    public static IntPtr Find(uint pid) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((hwnd, l) => {
            uint owner; GetWindowThreadProcessId(hwnd, out owner);
            if (owner != pid) return true;
            var title = new StringBuilder(64); GetWindowText(hwnd, title, 64);
            if (title.ToString() != "Hub") return true;
            found = hwnd; return false;
        }, IntPtr.Zero);
        return found;
    }
}
"@

function Get-HubSnapshot([int]$HubId, [string]$DataDir) {
    $all = Get-CimInstance Win32_Process
    $ids = @($HubId) + @($all | Where-Object { $_.Name -eq "msedgewebview2.exe" -and $_.CommandLine -like "*$DataDir*" } | ForEach-Object { [int]$_.ProcessId })
    $perf = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process
    $ids | ForEach-Object {
        $id = $_
        $process = Get-Process -Id $id -ErrorAction SilentlyContinue
        if ($process) {
            [pscustomobject]@{
                Id      = $id
                PrivWS  = ($perf | Where-Object IDProcess -eq $id | Select-Object -First 1).WorkingSetPrivate
                Commit  = $process.PrivateMemorySize64
                Cpu     = $process.TotalProcessorTime.TotalSeconds
            }
        }
    }
}

function Measure-State([string]$Name, [int]$HubId, [string]$DataDir, [int]$Seconds) {
    $before = Get-HubSnapshot $HubId $DataDir
    Start-Sleep -Seconds $Seconds
    $after = Get-HubSnapshot $HubId $DataDir
    $cpu = 0
    foreach ($row in $after) {
        $previous = $before | Where-Object Id -eq $row.Id
        if ($previous) { $cpu += $row.Cpu - $previous.Cpu }
    }
    [pscustomobject]@{
        State      = $Name
        Processes  = @($after).Count
        MemoryMB   = [math]::Round((($after | Measure-Object -Property PrivWS -Sum).Sum) / 1MB, 1)
        CommitMB   = [math]::Round((($after | Measure-Object -Property Commit -Sum).Sum) / 1MB, 1)
        IdleCpuPct = [math]::Round(100 * $cpu / $Seconds, 2)
    }
}

if (-not (Test-Path $Exe)) { throw "build first: $Exe is missing" }
$data = Join-Path ([IO.Path]::GetTempPath()) ("hub-measure-" + [guid]::NewGuid().ToString("N").Substring(0, 8))
New-Item -ItemType Directory -Path $data | Out-Null
if (Test-Path $Seed) { & $Seed $data | Out-Null }
$env:HUB_DATA_DIR = $data
# The local API on a test port, never the owner's (47731).
$env:HUB_API_PORT = "47782"
$env:HUB_RELEASE_AFTER_SECS = "$ReleaseAfter"
Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue

$rows = @()
$clock = [Diagnostics.Stopwatch]::StartNew()
$hub = Start-Process -FilePath $Exe -PassThru
$hwnd = [IntPtr]::Zero
while ($clock.ElapsedMilliseconds -lt 15000) {
    $hwnd = [HubWin]::Find([uint32]$hub.Id)
    if ($hwnd -ne [IntPtr]::Zero -and [HubWin]::IsWindowVisible($hwnd)) { break }
    Start-Sleep -Milliseconds 20
}
$startMs = $clock.ElapsedMilliseconds
try {
    Start-Sleep -Seconds 8
    $rows += Measure-State "window open, idle" $hub.Id $data 20
    [void][HubWin]::PostMessage($hwnd, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)   # WM_CLOSE: hide to tray
    $closed = [Diagnostics.Stopwatch]::StartNew()
    Start-Sleep -Seconds 2
    $rows += Measure-State "closed, kept for a quick return" $hub.Id $data 10
    while ($closed.Elapsed.TotalSeconds -lt $ReleaseAfter + 5) { Start-Sleep -Milliseconds 250 }
    $rows += Measure-State "closed, window let go" $hub.Id $data 10
} finally {
    Stop-Process -Id $hub.Id -Force -ErrorAction SilentlyContinue
}
Start-Sleep -Seconds 2
$tray = Start-Process -FilePath $Exe -ArgumentList "--tray" -PassThru
try {
    Start-Sleep -Seconds 8
    $rows += Measure-State "started in tray" $tray.Id $data 10
} finally {
    Stop-Process -Id $tray.Id -Force -ErrorAction SilentlyContinue
}
Get-CimInstance Win32_Process | Where-Object { $_.Name -eq "msedgewebview2.exe" -and $_.CommandLine -like "*$data*" } |
    ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }

"Start to visible window: $startMs ms"
$rows | Format-Table -AutoSize
"Data: $data"
