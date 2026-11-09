# Laporan Pengujian Hub

Versi yang diuji: **1.0.1** (commit F3 dan sesudahnya, cabang `main`). Tanggal: 9 November 2026.
Jenis pengujian dipisah sesuai sumber buktinya: otomatis, jendela asli (uji asap), nyata dengan
layanan asli, pemasangan, sumber daya, manual oleh pemilik, dan uji pengguna.

## 1. Lingkungan

| Komponen | Nilai |
|---|---|
| Perangkat | Lenovo LOQ 15IRH8 (82XV), Intel Core i5-13420H (8 inti, 12 utas), RAM 23,7 GB, Intel UHD Graphics dan NVIDIA RTX 4050 |
| Layar | 2560 x 1440, skala 150 % |
| Sistem operasi | Windows 11 Home Single Language 10.0.26200, 64-bit |
| WebView2 | 157.0.4341.42 |
| Alat | Rust dan Cargo 1.99.0, Node.js 24.14.0, npm 11.9.0, Tauri 2.12 |

## 2. Uji otomatis

Dijalankan ulang pada setiap checkpoint finalisasi (F1, F2, F3); semuanya lulus.

| Rangkaian | Jumlah | Hasil | Yang diuji |
|---|---|---|---|
| `hub-core` (Rust) | 375 | Lulus | Aturan tugas, proyek, kalender, rutinitas, pengingat, Catat Cepat, Tutup Hari, berkas, belajar, Jejak, pencarian, migrasi data, cadangan, ekspor, aturan Studio (PRD, rencana, laporan) |
| `hub-ai` (Rust) | 65 | Lulus | Klien Gemini terhadap Gemini tiruan: galat, aliran jawaban, fungsi, skema PRD, instruksi Lentera |
| `hub-api` (Rust) | 31 | Lulus | Setiap pertahanan API lokal (token, asal permintaan, Host, cakupan, batas laju, ukuran) dan rute ULTRON serta Claude Code |
| `hub-mcp` (Rust) | 12 | Lulus | Jembatan MCP yang asli lewat stdio terhadap API Hub yang asli |
| Aplikasi `hub` (Rust) | 14 | Lulus | Pemasangan Claude Code, tautan `hubapp://`, notifikasi |
| Antarmuka (Vitest) | 214 | Lulus | Setiap halaman, alur Studio, aksesibilitas (nama kontrol, satu h1, id unik) |
| Pemeriksaan tipe, lint, clippy | - | Lulus | TypeScript ketat, ESLint, Clippy dengan peringatan sebagai galat |
| Kontras warna | 368 pasangan | Lulus | Semua teks minimal 4,5:1 di semua cahaya dan tinta; terendah 4,57:1 |

Total uji Rust: 497 lulus, 0 gagal; 3 uji dijalankan manual (Credential Manager, probe pengalihan,
sesi Claude Code nyata).

## 3. Uji asap jendela asli

`node scripts/smoke-native.mjs`: build rilis dijalankan pada folder data uji, dikendalikan lewat
port DevTools WebView2, dengan Gemini tiruan dan jembatan MCP asli. **104/104 lulus** pada build
1.0.0 dan 1.0.1. Cakupan: Hari Ini terhadap inti, Catat Cepat, Kalender, pengingat yang berbunyi dan
ditunda, berkas (thumbnail Windows, Vault, berkas dipindah dan dihapus), sesi belajar, Jejak,
pencarian, Tutup Hari, tugas dari ULTRON lewat API dengan tandanya, penolakan permintaan dari halaman
web, Pengaturan, suara, gerak, data yang bertahan setelah restart, seluruh alur Studio sampai
checkpoint disetujui, status Claude Code, dan memori (Studio terbuka 132,8 sampai 141,3 MB, batas
180 MB).

## 4. Uji integrasi ULTRON

`scripts/smoke_hub.py` di repositori ULTRON terhadap `hub.exe` uji: **13/13 lulus** (9 November
2026). Tugas lewat suara masuk Hub sebagai milik ULTRON, "besok" menjadi tanggal kerja, "due Jumat"
menjadi tenggat, daftar dibaca dari Hub, tugas diselesaikan, catatan kegiatan, berkas yang dijatuhkan
tersimpan dan ditemukan lagi, jadwal dibaca, pengingat Hub diucapkan ULTRON tanpa notifikasi kedua,
migrasi data lama sekali jalan. Pada F2, klien ULTRON juga terbukti membaca Hub demo (port 47741).

## 5. Uji nyata dengan layanan asli

Di Hub milik pemilik, dengan kunci Gemini asli dan izin pemilik.

| Tanggal | Langkah | Model | Hasil |
|---|---|---|---|
| 8 Nov 2026 | Uji koneksi Gemini | 3.8 Flash | Terhubung |
| 8 Nov 2026 | Sesi Claude Code nyata (uji S5) | - | Lulus; 14 giliran, 35,6 detik, USD 0,47 |
| 9 Nov 2026 (F1) | Balasan pertama Lentera | 3.5 Flash | 34 detik; **bug ditemukan** (F1-B1) |
| 9 Nov 2026 (F1) | Rekomendasi tech stack | 3.8 Flash | 24 detik (dua percobaan sebelumnya 503 dari Google) |
| 9 Nov 2026 (F1) | Susun PRD | 3.8 Flash, 3.5 Flash | **Gagal**: skema ditolak Google (F1-B2) |
| 9 Nov 2026 (F1) | Susun PRD setelah perbaikan | 3.5 Flash-Lite | 64 detik, 0 masalah, 6 fitur |
| 9 Nov 2026 (F1) | Rencana checkpoint | 3.5 Flash-Lite | 11 detik, 6 checkpoint, 0 masalah |
| 9 Nov 2026 (F1) | Sesi Claude Code nyata lewat MCP | - | 16 giliran, 29 detik, USD 0,51; laporan diterima; status Bangun "Terbuka sekarang" lalu "Ditutup" |
| 9 Nov 2026 (F4) | Alur bersih dari ide sampai rencana disetujui | 3.5 Flash-Lite | Pertanyaan 7 detik (hanya pertanyaan, tanpa usulan tahap), jawaban 5 detik, tech stack 7 detik, PRD 44 detik (0 masalah, 4 fitur, 3 persyaratan keamanan, diagram 13 langkah), rencana sekitar 10 detik (6 checkpoint); Proyek Hub otomatis bernama "Aplikasi Peminjaman Alat Lab TJKT" |

Alat bantu `cargo run -p hub-ai --bin schema-probe` membuktikan skema daftar fitur PRD ditolak
Google sebelum perbaikan dan diterima sesudahnya.

## 6. Uji pemasangan

| Uji | Hasil |
|---|---|
| Pasang diam (`/S`) ke folder uji | 2,9 detik; `hub.exe` 1.0.0 dan `hub-mcp.exe` terpasang |
| Copot diam | Berkas program terhapus; folder data Hub utuh |
| Pembaruan 0.1.0 ke 1.0.0 di laptop pemilik | Berhasil, tetapi jembatan lama tertinggal (**bug F2-B1**), lalu diperbaiki |
| Pembaruan 1.0.0 ke 1.0.1 saat Hub dan jembatan berjalan | Berhasil; jembatan terganti (hash sama dengan build); data tidak berubah |

## 7. Sumber daya

Pengukuran `scripts/measure.ps1` pada laptop pengembang (`docs/architecture.md`, "Resources").

| Ukuran | Target | Hasil |
|---|---|---|
| Waktu sampai jendela tampil | di bawah 1,5 detik | 0,53 sampai 0,65 detik |
| Memori, jendela terbuka | di bawah 180 MB | 116 sampai 128 MB |
| Memori, mulai di tray | di bawah 40 MB | 3,1 MB |
| Memori, jendela dilepas setelah 10 menit | di bawah 40 MB | 5,6 MB |
| Memori, Studio terbuka | di bawah 180 MB | 132,8 sampai 141,3 MB |
| Bundel JavaScript | di bawah 400 KB (gzip) | 188 KB |
| Ukuran berkas | - | `hub.exe` 19,6 MB, `hub-mcp.exe` 3,3 MB, installer 5,9 MiB |

## 8. Bug yang ditemukan dan diperbaiki selama finalisasi

| ID | Ditemukan oleh | Masalah | Perbaikan |
|---|---|---|---|
| F1-B1 | Uji nyata Gemini | Lentera mencatat jawaban yang belum diberikan pengguna dan mengusulkan PRD | Pertanyaan mengakhiri giliran; usulan tahap ditolak selama ada pertanyaan |
| F1-B2 | Uji nyata Gemini | Susun PRD ditolak Google ("too many states") | Batas jumlah fitur dipindah dari skema ke pemeriksaan Hub |
| F1-B3 | Uji nyata Gemini | Alasan penolakan Google tidak tercatat | Dicatat di `hub.log` |
| F1-B4 | Uji nyata Gemini | Tidak ada "Ganti model" saat model sibuk | Ditambahkan |
| F2-B1 | Uji pemasangan | Pembaruan tidak mengganti jembatan yang sedang berjalan | Installer menghentikan jembatan dulu |
| P2-04 | Uji nyata | Proyek Hub baru bernama kalimat ide yang panjang | Memakai nama produk dari PRD |
| P2-02 | Audit | Hasil pencarian Catatan membuka Jejak tanpa filter | Jejak terbuka dengan filter Catatan |

## 9. Uji manual pemilik

Daftar 34 butir di `MANUAL_TESTS.md` (mulai bersama Windows, pintasan global, tombol notifikasi,
seret dari Explorer, `/hub-connect`, suara ULTRON, demo). **Belum dijalankan**; hasilnya belum boleh
ditulis sebagai lulus.

## 10. Uji pengguna dan kompatibilitas

Protokol di `UJI_PENGGUNA.md` dan `UJI_KOMPATIBILITAS.md`; data di `Rekap-Uji-Pengguna-Hub.xlsx`.
**Belum dilaksanakan** oleh kelompok.
