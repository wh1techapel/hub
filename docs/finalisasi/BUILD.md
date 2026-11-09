# Membangun Ulang Hub

Langkah untuk menghasilkan installer yang sama dari kode sumber, beserta versi alat yang dipakai
saat rilis 1.0.0 dan 1.0.1 dibangun (9 November 2026).

## Lingkungan

| Komponen | Versi |
|---|---|
| Sistem operasi | Windows 11 Home Single Language 10.0.26200 |
| Rust dan Cargo | 1.99.0 (MSVC) |
| Node.js dan npm | 24.14.0 dan 11.9.0 |
| Tauri CLI dan Tauri | 2.12.4 dan 2.12.5 |
| WebView2 | 157.0.4341.42 (bawaan Windows 11) |
| Pustaka utama | React 19.3.1, TypeScript 6.0.3, Vite 8.3.5, Vitest 5.0.5, TanStack Query 5.104.5, TanStack Router 1.170.45, rusqlite 0.40.2 (SQLite bawaan), axum 0.8.9, reqwest 0.13.5, rmcp 3.9.0 |

Versi pustaka dikunci oleh `Cargo.lock` dan `package-lock.json` di repositori.

## Langkah

```powershell
cd Hub                      # folder hasil git clone
npm ci                      # dependensi sesuai package-lock.json
npm run tauri build         # antarmuka, hub.exe, hub-mcp.exe, lalu installer NSIS
```

Hasil:

- `target\release\bundle\nsis\Hub_1.0.0_x64-setup.exe` (installer per pengguna, tanpa hak admin)
- `target\release\hub.exe` dan `target\release\hub-mcp.exe`

Pemeriksaan sebelum rilis (semuanya harus lulus):

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
npm run typecheck; npm run lint; npm test; npm run check:contrast
node scripts\smoke-native.mjs
```

## Sidik jari rilis

Dari commit F2 (1.0.0) dan F3 (1.0.1) di cabang `main` (lihat `git log`).

| Rilis | Berkas | Ukuran | SHA-256 |
|---|---|---|---|
| 1.0.1 | `Hub_1.0.1_x64-setup.exe` | 6.211.296 bita | `8adf243a3382305f8a00faf3ac02f9d1d88ddaaa010244c3f8cbb62b98226557` |
| 1.0.1 | `hub-mcp.exe` | 3.290.112 bita | `9d1fa9d1d0722048f8249c6376245c7eff74ee69e4f98c5e86de9d206535015d` |
| 1.0.0 | `Hub_1.0.0_x64-setup.exe` | 6.217.153 bita | `1dc26acd0802bcaaa1feb8bc4f6b13801f692be9e905de4237908f6d1a1c82d8` |
| 1.0.0 | `hub-mcp.exe` | 3.290.112 bita | `713b194160772b5a130671a9992510d74c7ef524ba6e236765926a65139bc87a` |

`hub.exe` di dalam installer berbeda hash dari `target\release\hub.exe`, karena Tauri menandai jenis
paket di dalam berkas saat membuat installer. Versinya dapat dicek di Properties > Details atau di
Pengaturan > Sistem & data.

Build Rust tidak sepenuhnya deterministik (jalur dan waktu build ikut tersimpan), jadi hash hasil
build ulang bisa berbeda walaupun kodenya sama. Yang dapat diulang adalah langkah dan hasil ujinya.

## Memasang

Jalankan installer. Program masuk ke `%LOCALAPPDATA%\Programs\Hub`, data tetap di
`%LOCALAPPDATA%\Hub`. Pembaruan menghentikan Hub dan jembatan `hub-mcp.exe` yang sedang berjalan,
mengganti programnya, dan tidak menyentuh data. Installer belum ditandatangani, sehingga Windows
SmartScreen bertanya pada pemasangan pertama ("More info", lalu "Run anyway").
