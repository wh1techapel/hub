# Catatan Rilis Hub

## 1.0.1 (9 November 2026)

Perbaikan kecil sebelum uji pengguna (F3).

- Proyek Hub yang dibuat saat rencana checkpoint disetujui kini memakai nama produk dari PRD
  (misalnya "Aplikasi Peminjaman Alat Lab TJKT"), bukan awal kalimat ide yang panjang. Dialog
  persetujuan menampilkan nama yang sama.
- Hasil pencarian Ctrl+K berupa Catatan membuka Jejak dengan filter Catatan.

## 1.0.0 (9 November 2026)

Rilis pertama yang dinyatakan stabil, disiapkan untuk PSAJ. Rincian setiap tahap ada di
[docs/checkpoints.md](docs/checkpoints.md); status dan keterbatasan ada di
[docs/finalisasi/](docs/finalisasi/).

### Isi

- **Produktivitas harian:** Hari Ini dengan Busur Hari, Sekarang, dan Fokus; Proyek & Tugas
  (Kotak Masuk, Mendatang, Semua, Selesai, proyek per Ruang dengan progres turunan); Kalender
  (hari, minggu, bulan, blok waktu, rutinitas, Pengingat); Catat Cepat (Ctrl+Alt+H, bahasa
  Indonesia dan Inggris dasar); Tutup Hari.
- **File dan belajar:** File (seret dari Explorer, Koleksi, Vault, thumbnail Windows, pelacakan
  berkas yang dipindah); Belajar (jalur, materi, timer, sesi, sasaran mingguan); Jejak (riwayat dan
  wawasan mingguan); pencarian Ctrl+K atas semua data.
- **ULTRON:** API lokal di 127.0.0.1 dengan token, cakupan izin per rute, dan batas laju. ULTRON
  menyimpan tugas, pengingat, catatan, dan berkasnya di Hub, membaca jadwal, dan mengucapkan
  pengingat dari Hub.
- **Hub Studio:** tiga tahap bersama Lentera (Gemini): Mau buat apa?, Tech stack, PRD dan bangun;
  PRD terstruktur yang diperiksa Hub, diagram alur pengguna, rencana checkpoint, dua versi prompt
  implementasi, dan Claude Code lewat jembatan MCP `hub-mcp.exe` dengan status sesi di Bangun.
- **Sistem:** data lokal di SQLite dengan cadangan harian dan salinan sebelum migrasi, ekspor semua
  data ke JSON, kunci Gemini di Windows Credential Manager, mulai bersama Windows ke tray, installer
  per pengguna tanpa hak admin, Pengaturan lima bagian, suara dan gerak halus yang menghormati
  pengaturan kurangi gerak.

### Diperbaiki dalam finalisasi (F1, uji nyata dengan Gemini asli)

- Balasan Lentera yang mengajukan pertanyaan kini mengakhiri gilirannya. Sebelumnya Lentera bisa
  mencatat jawabannya sendiri dan mengusulkan PRD sebelum pemilik menjawab.
- Penyusunan PRD tidak lagi ditolak Google ("too many states for serving"): batas jumlah fitur
  dijaga oleh pemeriksaan Hub, bukan oleh skema yang dikirim ke Gemini.
- `hub.log` mencatat alasan singkat Google bila permintaan ditolak.
- "Ganti model" muncul juga saat model sibuk atau permintaan ditolak.

### Diketahui

- Tier gratis Gemini membatasi jumlah permintaan per menit dan per hari untuk setiap model; satu
  alur Studio lengkap memakai sekitar 10 sampai 14 permintaan.
- Installer belum ditandatangani, sehingga Windows SmartScreen bertanya saat pemasangan pertama.
- Daftar uji yang hanya dapat dilakukan di laptop pemilik: [docs/finalisasi/MANUAL_TESTS.md](docs/finalisasi/MANUAL_TESTS.md).

### Riwayat tahap

| Tanggal | Tahap |
|---|---|
| 22 Oktober 2026 | CP1 riset dan desain, CP2 fondasi aplikasi |
| 23 Oktober 2026 | CP3 sistem produktivitas, CP4 File, Belajar, Jejak, pencarian |
| 24 Oktober 2026 | CP5 API lokal untuk ULTRON, ekspor, installer |
| 6 sampai 8 November 2026 | S1 sampai S6 Hub Studio (Gemini, PRD, roadmap, MCP, Claude Code) |
| 8 sampai 9 November 2026 | R1 sampai R7 revisi Studio tiga tahap dan navigasi |
| 9 November 2026 | P1 dan P2 Pengaturan, suara, gerak; U1 sampai U6 Studio seperti Gemini; L1 sampai L4 Lentera |
| 9 November 2026 | F1 uji nyata dan perbaikan; F2 rilis 1.0.0 |
