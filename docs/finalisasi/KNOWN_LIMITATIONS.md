# Keterbatasan Hub 1.0.1

Keterbatasan yang tersisa, ditulis apa adanya untuk Bab V laporan (keterbatasan dan saran).

## Hub Studio dan layanan AI

1. **Kuota gratis Gemini kecil.** Batas dihitung per model; untuk 3.5 Flash (dilayani sebagai
   3.6 Flash) tercatat 5 permintaan per menit dan 20 per hari. Satu alur Studio lengkap memakai
   sekitar 10 sampai 14 permintaan. Pada jam sibuk Google juga membalas 503 "high demand". Hub
   menampilkan alasannya dan tombol "Ganti model".
2. **Studio memerlukan internet**; bagian Hub lain berjalan tanpa internet.
3. **Privasi tier gratis.** Percakapan dan dokumen Studio dikirim ke Google, dan pada tier gratis
   Google dapat memakainya untuk pengembangan produknya. Hub menyebutkannya di Pengaturan dan di
   bawah kolom percakapan.
4. **Isi dokumen AI tidak dijamin benar.** Hub memeriksa bentuk dan aturan PRD dan rencana
   checkpoint (ID, ketergantungan, kriteria, uji), bukan kebenaran isinya. Contoh dari uji nyata F4:
   pengguna memilih jaringan lokal sekolah, tetapi tech stack yang direkomendasikan memakai hosting
   publik. Pengguna tetap harus meninjau dan menyetujui setiap tahap.
5. **Claude Code** memerlukan akun Claude berbayar. Obrolan biasa di aplikasi Claude (di luar tab
   Code) tidak didukung karena tidak punya folder proyek.

## ULTRON

6. ULTRON memerlukan model lokal (Ollama), mikrofon, dan spesifikasi laptop yang cukup. Jalur
   suara dari awal sampai akhir belum diuji ulang pada finalisasi (`MANUAL_TESTS.md` M24 sampai M27).
7. ULTRON belum dapat membaca status Hub Studio (dirancang, belum dibuat).

## Platform dan distribusi

8. Hanya Windows; diuji di Windows 11. Windows 10 belum diuji.
9. Installer belum ditandatangani, sehingga Windows SmartScreen bertanya saat pemasangan pertama.
10. Satu pengguna per komputer; tidak ada sinkronisasi antarperangkat, akun daring, atau aplikasi
    ponsel. Kalender tidak terhubung ke Google Calendar.

## Fitur

11. Isi berkas tidak ikut dicari; pencarian mencakup nama, catatan, dan Koleksi.
12. Catatan (`journal`) belum punya tampilan sendiri; tampil di Jejak dan pencarian.
13. Antarmuka hanya berbahasa Indonesia.

## Pengujian

14. Butir di `MANUAL_TESTS.md` belum dijalankan pemilik; fitur itu teruji otomatis, tetapi belum
    dinyatakan lulus di lingkungan nyata.
15. Uji pengguna (SUS) dan uji kompatibilitas di laptop lain belum dilaksanakan.

## Data

16. Data tersimpan di `%LOCALAPPDATA%\Hub`. Berkas `hub.db` harus selalu bersama `hub.db-wal` bila
    disalin manual; cara aman memindahkan data adalah "Ekspor semua data" atau cadangan harian.
