# Uji Manual Pemilik

Pemeriksaan yang hanya dapat dilakukan di laptop pemilik (registri Windows, sesi Windows, mikrofon,
akun Claude, perangkat USB). Sesi Claude tidak dapat menjangkaunya, jadi butir di sini **tidak boleh
ditandai LULUS sebelum pemilik mencobanya**.

Cara mengisi: jalankan langkahnya pada Hub yang terpasang (versi di Pengaturan > Sistem & data),
lalu tulis LULUS atau GAGAL dan catatan singkat (apa yang terlihat bila gagal). Tangkapan layar
untuk laporan disimpan dengan nama nomor butir, misalnya `M05.png`.

Versi yang diuji: ............ Tanggal: ............

## A. Windows dan tray

| No | Langkah | Hasil yang diharapkan | Hasil | Catatan |
|---|---|---|---|---|
| M01 | Nyalakan ulang Windows, lalu masuk. | Ikon Hub ada di tray, tanpa jendela. Klik ikon membuka Hub. | | |
| M02 | Klik kanan ikon tray, pilih "Keluar dari Hub". | Hub tertutup; ikon hilang dari tray. | | |
| M03 | Buka Chrome, tekan Ctrl+Alt+H, ketik `tes catat cepat besok 19.00`, Enter. | Jendela Catat Cepat muncul di atas Chrome; tugas tersimpan dan terlihat di Proyek & Tugas > Mendatang. | | |
| M04 | Matikan Wi-Fi, buka Hari Ini, Kalender, Proyek & Tugas. | Semua halaman berfungsi normal; Studio menyatakan sedang luring. | | |

## B. Pengingat dan notifikasi

| No | Langkah | Hasil yang diharapkan | Hasil | Catatan |
|---|---|---|---|---|
| M05 | Buat pengingat 2 menit dari sekarang (Kalender > Pengingat), tunggu. | Notifikasi Windows muncul dengan suara pengingat. | | |
| M06 | Pada notifikasi berikutnya, klik "Tunda 10 m". | Pengingat pindah 10 menit ke depan di Kalender > Pengingat. | | |
| M07 | Pada notifikasi berikutnya, klik "Selesai". | Pengingat tercatat selesai. | | |
| M08 | Biarkan hari belum ditutup sampai 21.00. | Satu notifikasi lembut Tutup Hari muncul. | | |

## C. Kalender dan File

| No | Langkah | Hasil yang diharapkan | Hasil | Catatan |
|---|---|---|---|---|
| M09 | Kalender > Minggu: seret tugas dari baki ke kisi jam dengan tetikus. | Muncul blok waktu yang tertaut ke tugas. | | |
| M10 | Seret satu PDF dari Explorer ke halaman File. | Berkas tersimpan, notifikasi kecil dengan Urungkan dan Beri catatan. | | |
| M11 | Buka satu proyek, seret satu berkas dari Explorer ke halamannya. | Berkas tersimpan dan tertaut ke proyek itu. | | |
| M12 | File > "Tambah file", pilih berkas. | Dialog Windows terbuka; berkas masuk ke daftar. | | |
| M13 | Seret berkas dari flashdisk ke File. | Berkas tersimpan dengan salinan di Vault. | | |
| M14 | Pindahkan berkas yang sudah dikenal Hub ke drive lain, lalu buka di Hub. | Berkas ditandai hilang; "Cari file" menemukannya lagi. | | |
| M15 | Lihat thumbnail berkas PDF dan Word di File. | Thumbnail tampil (bergantung aplikasi yang terpasang). | | |
| M16 | Pengaturan > Sistem & data > "Ekspor semua data". | Satu berkas JSON tersimpan di folder pilihan. | | |

## D. Hub Studio dan Claude Code

| No | Langkah | Hasil yang diharapkan | Hasil | Catatan |
|---|---|---|---|---|
| M17 | Studio > proyek dengan PRD > Ekspor Markdown. | Dialog Simpan Windows terbuka; berkas `.md` tersimpan. | | |
| M18 | Saat menyetujui rencana checkpoint, pilih folder repositori dengan tombol dialog. | Dialog folder Windows terbuka; folder terpilih tertulis. | | |
| M19 | Pengaturan > Integrasi AI > Claude Code: tekan "Lengkapi pemasangan" (atau "Pasang"). | Jembatan, server `hub`, dan skill dinyatakan terpasang. | | |
| M20 | Buka terminal di folder repositori proyek Studio, jalankan `claude`, ketik `/hub-connect`. | Skill tersedia; Claude terhubung ke proyek; Bangun menyatakan sesi "Terbuka sekarang", lalu "Ditutup" setelah Claude ditutup. | | |
| M21 | Ulangi M20 dari tab Code aplikasi Claude desktop dengan folder yang sama. | Sama seperti M20. | | |
| M22 | Bangun > Salin prompt "Dengan Hub", tempel di Claude Code. | Teks tertempel utuh. | | |
| M23 | Tutup jendela Hub (tetap di tray), lalu minta Claude Code mengirim laporan atau pertanyaan. | Notifikasi Windows muncul dan membuka proyek di Studio. | | |

## E. ULTRON

| No | Langkah | Hasil yang diharapkan | Hasil | Catatan |
|---|---|---|---|---|
| M24 | Keluar dari Hub, lalu jalankan ULTRON seperti biasa. | ULTRON menyalakan Hub ke tray tanpa jendela. | | |
| M25 | Ucapkan ke ULTRON: "tambahkan tugas belajar subnetting besok". | Tugas muncul di Hub dengan tanda merah ULTRON, tanggal kerja besok. | | |
| M26 | Tanyakan: "apa jadwalku besok?" | ULTRON membacakan jadwal dari Kalender Hub. | | |
| M27 | Buat pengingat 2 menit lagi lewat ULTRON, tunggu. | Hub menampilkan notifikasi, ULTRON mengucapkannya, tanpa notifikasi kedua. | | |
| M28 | Di folder ULTRON: `uv run --no-sync python scripts/migrate_to_hub.py`, lalu dengan `--apply`. | Uji coba menampilkan jumlah data; `--apply` memindahkannya sekali saja. | | |

## F. Demo

| No | Langkah | Hasil yang diharapkan | Hasil | Catatan |
|---|---|---|---|---|
| M31 | Keluar dari Hub, jalankan `powershell -ExecutionPolicy Bypass -File scriptsdemo.ps1`. | Hub demo terbuka berisi minggu contoh; Pengaturan > Sistem & data menunjukkan folder `Hub-demodata`. | | |
| M32 | Di Hub demo: simpan API key Gemini, lalu buat satu proyek Studio sampai rencana checkpoint disetujui. | Proyek demo siap; catat model yang dipakai dan lamanya. | | |
| M33 | Jalankan ULTRON dengan dua baris yang dicetak `demo.ps1`, ucapkan satu perintah tugas. | Tugas muncul di Hub demo dengan tanda ULTRON. | | |
| M34 | Rekam video cadangan alur demo (Win+Alt+R). | Video tersimpan dan diputar ulang dengan baik. | | |

## G. Rasa dan aksesibilitas

| No | Langkah | Hasil yang diharapkan | Hasil | Catatan |
|---|---|---|---|---|
| M29 | Pengaturan > Tampilan > Suara > Coba suara. | Suara terdengar halus, tidak mengganggu. | | |
| M30 | Windows: Pengaturan > Aksesibilitas > Efek visual > matikan Efek animasi; buka Hub. | Perpindahan halaman dan dialog tanpa gerak; cahaya Studio diam. | | |
