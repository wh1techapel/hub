# What a laptop is, for Hub's compatibility test (docs/finalisasi/UJI_KOMPATIBILITAS.md).
#
#   powershell -ExecutionPolicy Bypass -File .\compat-info.ps1
#
# Reads only: Windows, processor, memory, graphics and screen, display scale, the WebView2 runtime,
# and Hub's installed version. Writes the same lines to compat-<computer name>.txt in the current
# folder. Nothing is sent anywhere. Plain ASCII on purpose: Windows PowerShell 5.1 reads a script
# without a BOM as ANSI.

$ErrorActionPreference = 'SilentlyContinue'

function Get-WebView2Version {
    $guid = '{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
    $keys = @(
        "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\$guid",
        "HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\$guid",
        "HKCU:\Software\Microsoft\EdgeUpdate\Clients\$guid"
    )
    foreach ($key in $keys) {
        $pv = (Get-ItemProperty -Path $key -Name pv).pv
        if ($pv -and $pv -ne '0.0.0.0') { return $pv }
    }
    return 'tidak ditemukan'
}

$os = Get-CimInstance Win32_OperatingSystem
$cpu = Get-CimInstance Win32_Processor | Select-Object -First 1
$computer = Get-CimInstance Win32_ComputerSystem
$gpus = Get-CimInstance Win32_VideoController
$dpi = (Get-ItemProperty -Path 'HKCU:\Control Panel\Desktop\WindowMetrics' -Name AppliedDPI).AppliedDPI
$scale = if ($dpi) { "$([math]::Round($dpi / 96 * 100)) %" } else { 'tidak diketahui' }
$hubExe = Join-Path $env:LOCALAPPDATA 'Programs\Hub\hub.exe'
$hub = if (Test-Path $hubExe) { (Get-Item $hubExe).VersionInfo.ProductVersion } else { 'belum terpasang' }

$lines = @(
    "Waktu         : $(Get-Date -Format 'yyyy-MM-dd HH:mm')",
    "Komputer      : $env:COMPUTERNAME ($($computer.Manufacturer) $($computer.Model))",
    "Windows       : $($os.Caption) $($os.Version) (build $($os.BuildNumber)), $($os.OSArchitecture)",
    "Prosesor      : $($cpu.Name.Trim()) ($($cpu.NumberOfCores) inti, $($cpu.NumberOfLogicalProcessors) utas)",
    "RAM           : $([math]::Round($computer.TotalPhysicalMemory / 1GB, 1)) GB",
    "Skala layar   : $scale"
)
foreach ($gpu in $gpus) {
    $mode = if ($gpu.CurrentHorizontalResolution) { "$($gpu.CurrentHorizontalResolution) x $($gpu.CurrentVerticalResolution)" } else { 'tidak aktif' }
    $lines += "Grafis        : $($gpu.Name) ($mode)"
}
$lines += "WebView2      : $(Get-WebView2Version)"
$lines += "Hub           : $hub"

$file = Join-Path (Get-Location) "compat-$env:COMPUTERNAME.txt"
$lines | Set-Content -Path $file -Encoding UTF8
$lines | ForEach-Object { Write-Host $_ }
Write-Host ''
Write-Host "Tersimpan di $file"
