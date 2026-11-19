# NoIklan (Rust / Android)

Aplikasi Android native yang ditulis dalam Rust (egui/eframe). Aplikasi ini menjadi klien YouTube tanpa iklan dengan membuka video melalui Brave (ad-block bawaan Brave), lengkap dengan beranda, pencarian, langganan, pustaka, playlist, riwayat, statistik, dan SponsorBlock info.

## Build APK

Prasyarat: Rust, Android SDK + NDK, Java 17, `cargo-apk`.

```bash
rustup target add aarch64-linux-android
cargo install cargo-apk
cargo apk build --release
```

APK dihasilkan di `target/release/apk/NoIklan.apk`.

## Jalankan di desktop (preview)

```bash
cargo run --bin noiklan-desktop
```

## Catatan

- Pemutaran video dilakukan oleh Brave agar iklan diblokir secara native oleh browser, bukan WebView HTML.
- Jika Brave tidak terpasang, aplikasi jatuh kembali ke browser default.
- Data lokal (playlist, riwayat, suka, langganan, pengaturan) disimpan sebagai JSON di direktori data aplikasi.
