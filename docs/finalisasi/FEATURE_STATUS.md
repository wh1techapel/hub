# Status Fitur Hub

Tanggal: 9 November 2026, commit `26626e6`. Status berdasarkan uji yang dijalankan ulang pada tanggal
itu (lihat `PRODUCT_AUDIT.md` bagian 2, bahan PSAJ yang tidak ikut repositori publik).

Arti status:

- **Selesai dan terverifikasi**: lulus uji otomatis dan uji asap jendela asli pada tanggal audit.
- **Berfungsi, perlu perbaikan**: bekerja, tetapi ada masalah yang perlu ditangani.
- **Implementasi parsial**: sebagian cakupan belum ada.
- **Belum diimplementasikan**: tidak ada di kode.
- **Tidak dapat diverifikasi**: membutuhkan lingkungan pemilik (registri, sesi Windows, kunci API,
  akun Claude, mikrofon) yang tidak dapat dijangkau sesi Claude. Tidak boleh ditandai lulus sebelum
  pemilik menguji.

Kolom "Uji pemilik" berisi pemeriksaan manual yang masih terbuka.

## A. Produktivitas inti

| Fitur | Status Aktual | Masalah / Uji pemilik | Prioritas | Tindakan |
|---|---|---|---|---|
| Hari Ini (Busur Hari, Sekarang, Fokus, Jadwal, Belajar) | Selesai dan terverifikasi | Tidak ada | - | Tangkapan layar untuk laporan |
| Proyek & Tugas (Kotak Masuk, Mendatang, Semua, Selesai, panel tugas, langkah, prioritas, urutan, hapus dengan Urungkan) | Selesai dan terverifikasi | Tidak ada | - | - |
| Proyek dan Ruang (progres dari tugas, tonggak berisi tugas, langkah berikutnya, riwayat) | Selesai dan terverifikasi | Tidak ada | - | - |
| Kalender (Hari, Minggu, Bulan, seret untuk membuat dan memindah, blok waktu) | Selesai dan terverifikasi | Uji pemilik: menyeret tugas dari baki Kalender ke kisi dengan tetikus di jendela asli | P1 | Daftar uji manual (F1) |
| Rutinitas (jadwal sekolah, lewati, pindah sekali) | Selesai dan terverifikasi | Jadwal sekolah asli belum dimasukkan pemilik (data, bukan kode) | P2 | Opsional untuk demo |
| Pengingat (penjadwal, notifikasi Windows, terlewat) | Selesai dan terverifikasi (penjadwal di uji asap) | Tidak dapat diverifikasi: tombol "Tunda 10 m" dan "Selesai" pada notifikasi Windows nyata; notifikasi Tutup Hari 21.00 | P1 | Daftar uji manual (F1) |
| Catat Cepat (pembaca bahasa Indonesia dan Inggris dasar) | Selesai dan terverifikasi | Tidak dapat diverifikasi: Ctrl+Alt+H dari aplikasi lain | P1 | Daftar uji manual (F1) |
| Tutup Hari | Selesai dan terverifikasi (bertahan setelah restart) | Tidak ada | - | - |
| File / Library (kantong seret, Koleksi, Vault, thumbnail Windows, pelacakan pindah) | Selesai dan terverifikasi (pencari tiruan dan NTFS asli) | Tidak dapat diverifikasi: seret nyata dari Explorer, dialog "Tambah file", berkas dari flashdisk, thumbnail PDF dan Office | P1 | Daftar uji manual (F1) |
| Belajar (jalur, materi, timer, sesi, sasaran mingguan, peta panas) | Selesai dan terverifikasi | Tidak ada | - | - |
| Jejak (riwayat, filter, wawasan mingguan) | Selesai dan terverifikasi | Tidak ada | - | - |
| Pencarian Ctrl+K (FTS5 lintas jenis data) | Selesai dan terverifikasi | Diperbaiki di 1.0.1: hasil Catatan membuka Jejak dengan filter Catatan (P2-02) | - | - |
| Catatan (journal) | Berfungsi, perlu perbaikan | Tidak ada tampilan sendiri; terlihat di Jejak dan pencarian | P3 | Masuk saran pengembangan |
| Penyimpanan lokal SQLite (migrasi 1 sampai 8, salinan sebelum migrasi, cadangan harian) | Selesai dan terverifikasi (data bertahan setelah restart di uji asap; uji naik versi skema) | WAL belum pernah di-checkpoint pada data pemilik (P3-01), aman | P3 | Catat di keterbatasan |
| Ekspor semua data (JSON) | Selesai dan terverifikasi (uji otomatis) | Uji pemilik: satu ekspor nyata | P1 | Daftar uji manual (F1) |
| Pengaturan lima bagian, cahaya dan tinta | Selesai dan terverifikasi | Tidak ada | - | - |
| Suara dan gerak mikro | Selesai dan terverifikasi (konteks audio dan animasi diukur) | Tidak dapat diverifikasi: rasa suara dan gerak, mode kurangi gerak Windows | P2 | Penilaian pemilik |
| Tray, satu instans, tutup ke tray | Selesai dan terverifikasi | Uji pemilik: "Keluar dari Hub" dari menu tray | P1 | Daftar uji manual (F1) |
| Mulai bersama Windows | Tidak dapat diverifikasi | Hub mulai di tray setelah Windows dinyalakan ulang | P1 | Daftar uji manual (F1) |
| Installer NSIS per pengguna | Selesai dan terverifikasi (F2: pasang dan copot bersih di folder uji; pembaruan 0.1.0 ke 1.0.0 di laptop pemilik dengan Hub dan jembatan berjalan) | Bug ditemukan dan diperbaiki: pembaruan tidak mengganti `hub-mcp.exe` yang sedang dipakai Claude Code. Belum ditandatangani (SmartScreen); belum diuji di laptop lain | P1 | Uji kompatibilitas (F3) |
| Versi aplikasi dan catatan rilis | Selesai dan terverifikasi (F2: versi 1.0.0 di semua komponen dan di Pengaturan > Sistem & data; CHANGELOG.md) | Tidak ada | - | - |

## B. Hub Studio

| Fitur | Status Aktual | Masalah / Uji pemilik | Prioritas | Tindakan |
|---|---|---|---|---|
| Kunci Gemini di Windows Credential Manager, Uji koneksi | Selesai dan terverifikasi (kunci asli diuji pemilik 8 Nov 2026) | Tidak ada | - | - |
| Tahap 1 Mau buat apa? (Lentera, set pertanyaan berkelompok, Ringkasan, Yang sudah jelas) | Selesai dan terverifikasi dengan Gemini asli (F1, 9 Nov 2026) | Bug ditemukan dan diperbaiki: balasan yang bertanya melanjutkan giliran dan mencatat keputusan karangan. Setelah perbaikan: hanya pertanyaan, satu putaran, tanpa usulan tahap (3.5 Flash dan 3.5 Flash-Lite) | - | Selesai (F1) |
| Tahap 2 Tech stack (pilih sendiri, rekomendasi, profil developer, persetujuan) | Selesai dan terverifikasi dengan Gemini asli (F1: rekomendasi tujuh lapisan dalam 24 detik) | Profil developer pemilik masih kosong | P2 | Pemilik mengisi profil |
| Tahap 3 PRD (tiga bagian terstruktur, validasi Hub, perbaikan otomatis, versi, revisi per bagian, keamanan, diagram alur, ekspor Markdown) | Selesai dan terverifikasi dengan Gemini asli (F1: versi 1 dalam 64 detik, 0 masalah) | Bug ditemukan dan diperbaiki: Google menolak skema daftar fitur (`maxItems`). Ekspor lewat dialog Simpan asli belum diuji | P1 | Uji pemilik M17 |
| Rencana checkpoint (roadmap) | Selesai dan terverifikasi dengan Gemini asli (F1: 6 checkpoint dalam 11 detik, 0 masalah) | Diperbaiki di 1.0.1: Proyek Hub baru memakai nama produk dari PRD (P2-04). Dialog folder asli belum diuji | P1 | Uji pemilik M18 |
| Prompt implementasi (Dengan Hub, Mandiri) | Selesai dan terverifikasi | Uji pemilik: Salin ke clipboard asli lalu tempel di Claude Code | P1 | Daftar uji manual (F1) |
| Bangun / Pantau (status checkpoint, laporan, setujui, minta perubahan, kerjakan sendiri) | Selesai dan terverifikasi | Set pertanyaan tak terjawab tetap tampil di tahap 3 (P2-01) | P2 | Cek dan perbaiki bila membingungkan (F3) |
| Jembatan MCP `hub-mcp.exe` dan skill `/hub-connect` | Selesai dan terverifikasi (sesi Claude Code CLI nyata F1: 16 giliran, 29 detik, USD 0,51, laporan diterima) | Uji pemilik: "Lengkapi pemasangan", lalu `/hub-connect` di CLI dan tab Code aplikasi desktop | P1 | Uji pemilik M19 sampai M21 |
| Status Claude Code (Jembatan, Sesi, Aktivitas) | Selesai dan terverifikasi dengan sesi Claude Code nyata (F1: "Terbuka sekarang", setiap alat, lalu "Ditutup") | Tidak ada | - | - |
| Pilihan model dan Ganti model | Selesai dan terverifikasi (daftar asli dilihat pemilik) | Tidak ada | - | - |
| Penanganan galat (kuota, model sibuk, luring, Coba lagi, Ganti model, Hentikan) | Selesai dan terverifikasi dengan Gemini asli (F1: 503 "high demand" dan kuota harian terjadi nyata) | Diperbaiki: "Ganti model" kini muncul untuk model sibuk dan permintaan ditolak; alasan Google tercatat di hub.log. Kuota gratis kecil (20 permintaan per hari untuk 3.5/3.6 Flash) | P1 | Rencana demo (F2) |
| Penyimpanan hasil Studio | Selesai dan terverifikasi (percakapan bertahan setelah restart) | Tidak ada | - | - |

## C. ULTRON

| Fitur | Status Aktual | Masalah / Uji pemilik | Prioritas | Tindakan |
|---|---|---|---|---|
| API lokal Hub (127.0.0.1, token, cakupan, batas laju, tolak peramban) | Selesai dan terverifikasi (31 uji `hub-api`, uji asap) | Tidak ada | - | - |
| ULTRON membuat tugas, pengingat, catatan; membaca daftar dan jadwal; mencari berkas | Selesai dan terverifikasi (uji asap ULTRON 13/13 pada tanggal audit) | Tidak ada | - | - |
| Pengingat Hub diucapkan ULTRON tanpa notifikasi kedua | Selesai dan terverifikasi (uji asap ULTRON) | Uji pemilik: ULTRON dijalankan normal dan mengucapkannya | P1 | Daftar uji manual (F1) |
| ULTRON menyalakan Hub bila Hub mati | Tidak dapat diverifikasi | Uji pemilik | P1 | Daftar uji manual (F1) |
| Status koneksi ULTRON di Hub ("terakhir terhubung", "belum pernah") | Selesai dan terverifikasi | Tidak ada | - | - |
| Hub tetap bekerja tanpa ULTRON | Selesai dan terverifikasi | Tidak ada | - | - |
| Otorisasi perubahan (cakupan per rute, hapus lunak, GUARD ULTRON, tanpa akses Studio) | Selesai dan terverifikasi (uji API) | Tidak ada | - | - |
| Perintah suara ujung ke ujung (wake word, STT, Ollama, TTS) | Tidak dapat diverifikasi | Membutuhkan mikrofon dan model lokal pemilik | P1 | Uji pemilik dan rekaman video demo (F1, F2) |
| Migrasi data ULTRON lama ke Hub | Selesai dan terverifikasi (uji asap ULTRON) | Uji pemilik: `migrate_to_hub.py --apply` sekali | P2 | Daftar uji manual (F1) |
| ULTRON membaca status Studio | Belum diimplementasikan (dirancang) | Di luar cakupan PSAJ | P3 | Saran pengembangan |
