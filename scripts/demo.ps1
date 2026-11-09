# Hub for a demonstration, without the owner's personal data.
#
#   powershell -ExecutionPolicy Bypass -File scripts\demo.ps1            # start the demo Hub
#   powershell -ExecutionPolicy Bypass -File scripts\demo.ps1 -Reset     # a fresh demo week for today
#
# The demo Hub keeps its data in its own folder (default ..\Hub-demo\data, beside this repository),
# filled by hub-seed with a demo week around the day it runs, so Hari Ini, Kalender, Proyek & Tugas,
# Belajar, File, and Jejak have something to show. It runs beside nothing: the owner's Hub is asked to
# close first, so Ctrl+Alt+H and the tray belong to the demo. Its local API listens on port 47741
# (the owner's Hub uses 47731), and ULTRON can be pointed at it with the two lines printed at the end.
#
# Studio in the demo Hub uses the real Gemini. Its key is kept per data folder in Windows Credential
# Manager, so it is saved once in the demo Hub (Pengaturan > Integrasi AI) and stays across -Reset.
# -Reset removes the demo folder's data, Studio projects included; the owner's own Hub is never
# touched. Plain ASCII on purpose: Windows PowerShell 5.1 reads a script without a BOM as ANSI.

param(
    [switch]$Reset,
    [string]$DataDir = (Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) 'Hub-demo\data'),
    [int]$Port = 47741
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot

# Hub: the installed one first (the release the demo is about), else this repository's build.
$installed = Join-Path $env:LOCALAPPDATA 'Programs\Hub\hub.exe'
$built = Join-Path $repo 'target\release\hub.exe'
$hubExe = if (Test-Path $installed) { $installed } elseif (Test-Path $built) { $built } else { $null }
if ($null -eq $hubExe) {
    Write-Host 'Hub belum terpasang dan belum dibangun. Jalankan dulu: npm run tauri build' -ForegroundColor Red
    exit 1
}

$seedExe = @((Join-Path $repo 'target\release\hub-seed.exe'), (Join-Path $repo 'target\debug\hub-seed.exe')) | Where-Object { Test-Path $_ } | Select-Object -First 1
if ($null -eq $seedExe) {
    Write-Host 'Membangun hub-seed (sekali saja)...'
    cargo build --release -p hub-core --bin hub-seed --manifest-path (Join-Path $repo 'Cargo.toml')
    $seedExe = Join-Path $repo 'target\release\hub-seed.exe'
}

# The owner's Hub (or an earlier demo Hub) holds the hotkey and the tray: close it first.
$running = Get-Process hub -ErrorAction SilentlyContinue
if ($null -ne $running) {
    $answer = Read-Host 'Hub sedang berjalan. Tutup dulu supaya demo memakai Ctrl+Alt+H dan tray? (y/n)'
    if ($answer -ne 'y') {
        Write-Host 'Demo dibatalkan. Tutup Hub lewat tray (Keluar dari Hub), lalu jalankan lagi.'
        exit 1
    }
    $running | Stop-Process -Force
    Start-Sleep -Seconds 2
}

if ($Reset -and (Test-Path $DataDir)) {
    $answer = Read-Host "Hapus semua data demo di $DataDir (termasuk proyek Studio demo)? (y/n)"
    if ($answer -ne 'y') {
        Write-Host 'Reset dibatalkan.'
        exit 1
    }
    Remove-Item -Recurse -Force $DataDir
}

if (-not (Test-Path (Join-Path $DataDir 'hub.db'))) {
    New-Item -ItemType Directory -Force $DataDir | Out-Null
    & $seedExe $DataDir
    if ($LASTEXITCODE -ne 0) {
        Write-Host 'hub-seed gagal; lihat pesannya di atas.' -ForegroundColor Red
        exit 1
    }
}

$env:HUB_DATA_DIR = $DataDir
$env:HUB_API_PORT = "$Port"
Start-Process -FilePath $hubExe
Remove-Item Env:HUB_DATA_DIR, Env:HUB_API_PORT

Write-Host ''
Write-Host "Hub demo berjalan: $hubExe"
Write-Host "Data demo: $DataDir"
Write-Host ''
Write-Host 'Untuk ULTRON, jalankan ULTRON dari jendela PowerShell yang berisi:'
Write-Host "  `$env:ULTRON_HUB_API_FILE = '$(Join-Path $DataDir 'api.json')'"
Write-Host "  `$env:ULTRON_HUB_START = '0'"
Write-Host ''
Write-Host 'Selesai demo: tutup Hub demo lewat tray (Keluar dari Hub), lalu buka Hub seperti biasa.'
