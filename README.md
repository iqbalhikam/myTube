# NoIklan (Rust / Android)

Aplikasi Android native yang ditulis dalam Rust (egui/eframe). Aplikasi ini menjadi klien YouTube tanpa iklan dengan membuka video melalui Brave (ad-block bawaan Brave), lengkap dengan beranda, pencarian, langganan, pustaka, playlist, riwayat, statistik, dan SponsorBlock info.

## Build APK

Prasyarat: Rust, Android SDK + NDK, Java 17, `cargo-apk`.

```bash
rustup target add aarch64-linux-android
cargo install cargo-apk
cargo apk build --release --lib
```

APK dihasilkan di `target/release/apk/NoIklan.apk`.

NDK harus mendukung API 34 (NDK r26+); cargo-apk membuang platform di luar rentang `NDK_MAX_PLATFORM_LEVEL` dan gagal dengan "Android SDK has no platforms installed".

## Rilis APK (GitHub Actions)

Workflow `.github/workflows/build-apk.yml` jalan saat push tag `v*.*.*`. Build release perlu keystore tetap agar update APK bisa menimpa versi lama, jadi siapkan dua secret repo:

1. Buat keystore sekali (simpan `noiklan-release.jks` dan password-nya di tempat aman):

   ```powershell
   keytool -genkeypair -v -keystore noiklan-release.jks -alias noiklan `
     -keyalg RSA -keysize 2048 -validity 10000
   ```

2. Ubah ke Base64 dan salin ke clipboard:

   ```powershell
   [Convert]::ToBase64String([IO.File]::ReadAllBytes("noiklan-release.jks")) | Set-Clipboard
   ```

3. Di GitHub: Settings > Secrets and variables > Actions, tambahkan:
   - `KEYSTORE_BASE64` — isi clipboard dari langkah 2
   - `KEYSTORE_PASSWORD` — password keystore

Pakai keystore yang sama untuk semua rilis; jika diganti, pengguna harus uninstall sebelum install versi baru.

## Jalankan di desktop (preview)

```bash
cargo run --bin noiklan-desktop
```

## Catatan

- Tampilan mengikuti YouTube: beranda rekomendasi, pencarian, Shorts, langganan, dan library.
- Beranda dan hasil pencarian diambil langsung dari YouTube (trending + search, region Indonesia); thumbnail memakai `i.ytimg.com`. Katalog lokal hanya dipakai sebagai fallback saat offline.
- Pemutaran video dilakukan oleh Brave agar iklan diblokir secara native oleh browser, bukan WebView HTML.
- Jika Brave tidak terpasang, aplikasi jatuh kembali ke browser default.
- Data lokal (playlist, riwayat, suka, langganan, pengaturan) disimpan sebagai JSON di direktori data aplikasi.
- Workflow bisa dijalankan manual (`workflow_dispatch`) dengan `publish` dimatikan untuk sekadar memverifikasi build tanpa membuat GitHub Release.
