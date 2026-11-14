# Catatan Rilis Hub

## 1.0.3 (14 November 2026)

Lentera hadir di desktop: asisten ringan untuk pelajar, tanpa GPU.

- **Bar Lentera.** `Ctrl+Alt+L` dari aplikasi mana pun (atau `Ctrl+L`, Tanya Lentera di bilah
  samping, palet, dan tray) membuka bar Lentera di pojok kanan bawah layar. Tulis satu kalimat,
  dan Lentera (lewat Google Gemini) mengusulkan tugas, pengingat, jadwal, atau catatan sebagai
  kartu. Tidak ada yang tersimpan sebelum kamu menekan Simpan; Ubah mengganti isinya, Lewati
  membuangnya. Tanpa kunci Gemini atau saat offline, kalimatnya tetap bisa disimpan lewat Catat
  Cepat.
- **Dari Pesan.** Tempel pesan grup kelas (ekspor WhatsApp atau pesan yang disalin). Hub
  membersihkannya dulu di komputer (nama pengirim, jam kirim, nomor, mention, dan email dibuang;
  tanggal kirim disimpan) dan menunjukkannya sebelum dikirim.
- **Sesi Fokus.** Pomodoro tanpa internet untuk tugas, materi Belajar, atau apa saja: 25 menit
  fokus, 5 menit istirahat, istirahat panjang setiap 4 blok (bisa diubah di Pengaturan > Hari).
  Hitung mundurnya tampil di kapsul Lentera, dan setiap blok tercatat di Jejak; blok pada materi
  Belajar menjadi sesi belajarnya.
- Yang dikirim ke Google dari bar Lentera hanya kalimatmu, tanggal dan jam, serta nama Proyek dan
  Ruang; bar menyebutkannya di bawah kolom ketik.
- Pembaruan ini mengubah database (migrasi 0010). Hub menyimpan salinan database lama di folder
  `backups` sebelum mengubahnya.

## 1.0.2 (10 November 2026)

Tugas dan tonggak yang lebih mudah dipahami, dan perbaikan dari CI.

- **Tonggak sekarang berisi tugas.** Kemajuan proyek selalu dihitung dari tugasnya ("x dari y
  tugas selesai"), ada tonggak atau tidak. Sebelumnya kemajuan diam-diam pindah ke tonggak begitu
  ada satu tonggak. Di halaman proyek, tugas tanpa tonggak tampil paling atas, lalu satu kelompok
  per tonggak dengan tanggalnya, "x dari y tugas", dan baris tambah tugasnya sendiri. Tonggak
  tercapai saat semua tugasnya selesai; tonggak tanpa tugas dicentang sendiri. Tugas lama
  dimasukkan ke tonggak lewat kolom Tonggak di panel tugas.
- Pembaruan ini mengubah database (migrasi 0009). Hub menyimpan salinan database lama di folder
  `backups` sebelum mengubahnya. Tugas lama mulai tanpa tonggak; centang tonggak lama tetap.
- Jembatan `hub-mcp.exe` kini mengenali folder data yang ditulis dengan nama pendek 8.3 (misalnya
  `RUNNER~1`) dan tidak lagi mengiranya folder yang dialihkan. Ditemukan oleh CI di GitHub.
- README memuat spesifikasi Hub serta Hub + ULTRON, dan panduan pemasangan yang lebih jelas.

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
