# fi-focus

Aplikasi pemutar white, pink, dan brown noise untuk Windows dan Android (Rust + Slint 1.18.1 + CPAL 0.18.2). Audio dibuat langsung di perangkat, tanpa file rekaman, model, atau koneksi jaringan. UI berbahasa Indonesia. Android saat ini hanya arm64-v8a. Jangan mengklaim noise sebagai terapi ADHD atau menjanjikan manfaat klinis.

## Prinsip
- Pertahankan aplikasi kecil: tiga pilihan suara, satu suara aktif, ketuk pilihan aktif untuk berhenti. Jangan menambah framework, dependensi, pengaturan, atau fitur tanpa kebutuhan tugas.
- Audio dihasilkan secara kontinu di callback CPAL. Hindari alokasi, I/O, logging per sampel, atau operasi blocking di callback.
- State filter harus bertahan antar-buffer. Jangan menginisialisasi ulang filter setiap callback atau setiap kanal.
- Sample rate dan jumlah kanal mengikuti konfigurasi default perangkat. Windows dan Android memakai generator dan state filter terpisah per kanal output (stereo independen pada perangkat stereo). Ini bukan binaural beats.
- Volume digital tidak menentukan SPL di telinga. Jangan menyebut level aman atau dosis ADHD hanya berdasarkan amplitudo sampel.

## Struktur
- `app/Cargo.toml` dan `app/Cargo.lock`: crate Rust, dependensi, target Windows/Android, profil rilis.
- `app/src/main.rs`: entry point executable Windows.
- `app/src/lib.rs`: generator noise, stream CPAL, state pemutaran, callback UI, entry point Android, dan minimize lintas platform.
- `app/build.rs`: kompilasi `src/app.slint` dan resource ikon Windows.
- `src/app.slint`: layout, tiga kartu suara, status error, tombol Minimalkan dan Tutup, drag jendela Windows.
- `src/icon.png`, `src/logo.png`, `app/icon.ico`: aset aplikasi.
- `android/app/src/main/java/com/fi/focus/MainActivity.kt`: NativeActivity, jembatan minimize di UI thread Android, dan foreground service mediaPlayback.
- `android/app/src/main/AndroidManifest.xml`: activity singleTask, library native, service, dan izin foreground service.
- `android/app/build.gradle.kts`: SDK, ABI, versi aplikasi, dan signing rilis lokal.
- `5mdt_noise/noise/`: sumber JavaScript referensi awal, tidak disertakan dalam folder ini. Filter referensi mereset state tiap blok; jangan menyalin perilaku itu ke Rust.
- `LICENSE-5mdt.txt`: atribusi/lisensi sumber referensi. Pertahankan saat mendistribusikan turunan.
- `dist/windows/fi-focus.exe` dan `dist/android/fi-focus-arm64.apk`: artefak distribusi.

## Audio
- Mode `0` berhenti, `1` white, `2` pink, `3` brown. State pilihan memakai atomik; stream dimiliki sepanjang pemutaran.
- White memakai xorshift32 dengan seed awal tetap, dipetakan ke sekitar ±1/30. Pembagian 30 mengatur amplitudo, bukan frekuensi atau tempo.
- Pink memakai filter Paul Kellett dengan tujuh state dan gain 0.33. `b[6]` diperbarui sesudah menghitung output. Target spektrum daya 1/f, sekitar −3 dB/oktaf.
- Brown memakai integrator bocor dengan `a = exp(-2*pi*5/sample_rate)` dan input `sqrt(1-a*a) * white`. Batas bawah 5 Hz mencegah drift DC, mempertahankan karakter antar-sample-rate, dan menyetarakan RMS teoretis sebelum gain dengan white. Target audibel mendekati −6 dB/oktaf; bukan integrator ideal sampai DC.
- Brown melewati high-pass Butterworth orde 2 pada 20 Hz (state f64 per kanal) untuk meredam sub-bass; gain tidak mengompensasi energi yang dibuang. Filter ini bukan jaminan SPL aman.
- Gain Brown dikalikan 2.2 untuk keseimbangan level dengan Pink pada Windows dan Android.
- RMS sama tidak berarti kenyaringan yang dirasakan sama. Gain Pink disesuaikan agar RMS mendekati white.
- Pergantian mode memakai fade-out lalu fade-in sekitar 20 ms per tahap. State pink/brown dipertahankan saat berganti mode; stream baru memulai state dari nol.
- Format output yang ditangani: F32, I16, U16. Format lain dan kegagalan perangkat harus masuk jalur error yang terlihat di UI.
- Timer UI 50 ms melepas stream setelah fade berhenti selesai dan menangani error stream. Jangan melepas stream sebelum fade selesai.

## UI dan lifecycle
- Jendela Windows tanpa frame, ukuran awal 420×356, minimum 320×356. Perubahan ukuran awal perlu sinkron antara Slint dan pengaturan native di Rust.
- Tombol `Minimalkan` berada di kiri `Tutup`. Windows memakai `Window::set_minimized(true)`.
- Android memanggil `MainActivity.minimizeApp()` lewat JNI; Kotlin menjalankan `moveTaskToBack(true)` di UI thread. Jangan menggantinya dengan quit atau finish activity.
- Foreground service mempertahankan notifikasi pemutaran. Generator dan stream tetap dimiliki Rust, bukan service Kotlin. Kelanjutan audio di latar belakang harus diuji pada perangkat; keberhasilan build saja tidak membuktikannya.
- Tutup mengakhiri event loop. Pertahankan pemulihan error dengan mengetuk pilihan suara kembali.
- Teks UI singkat, bahasa Indonesia. Pertahankan urutan white, pink, brown dan palet yang sudah ada.

## Perintah
Jalankan dari root proyek menggunakan PowerShell. Prasyarat: Rust dengan target `aarch64-linux-android`, toolchain Windows, JDK 17, Android SDK 37, dan NDK yang kompatibel. Contoh memakai NDK 28.2.13676358; periksa instalasi sebelum menjalankan. `ANDROID_HOME` dan `JAVA_HOME` harus menunjuk instalasi lokal.

```powershell
# Windows
cargo build --locked --release --manifest-path app/Cargo.toml
New-Item -ItemType Directory -Force dist/windows | Out-Null
Copy-Item app/target/release/fi-focus.exe dist/windows/fi-focus.exe

# Android native: target API 26 sesuai minSdk
$ndkBin = Join-Path $env:ANDROID_HOME 'ndk/28.2.13676358/toolchains/llvm/prebuilt/windows-x86_64/bin'
$env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = Join-Path $ndkBin 'aarch64-linux-android26-clang.cmd'
$env:CC_aarch64_linux_android = $env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER
$env:AR_aarch64_linux_android = Join-Path $ndkBin 'llvm-ar.exe'
cargo build --locked --release --target aarch64-linux-android --lib --manifest-path app/Cargo.toml
# Lanjut hanya jika cargo berhasil; jangan mengemas library lama.
New-Item -ItemType Directory -Force android/app/src/main/jniLibs/arm64-v8a | Out-Null
Copy-Item app/target/aarch64-linux-android/release/libfi_focus_lib.so android/app/src/main/jniLibs/arm64-v8a/libfi_focus_lib.so
Push-Location android
try { ./gradlew.bat assembleRelease } finally { Pop-Location }
# Salin hanya jika Gradle berhasil.
New-Item -ItemType Directory -Force dist/android | Out-Null
Copy-Item android/app/build/outputs/apk/release/app-release.apk dist/android/fi-focus-arm64.apk
```

Tutup executable Windows sebelum menimpanya. Jangan menjalankan kedua build Cargo bersamaan pada target directory yang sama; keduanya akan menunggu lock. APK harus dibangun setelah library Rust terbaru disalin ke jniLibs. Gradle dapat menghapus simbol native; bandingkan isi APK dengan hasil `stripped_native_libs`, bukan langsung dengan library sebelum stripping.

## Verifikasi
- Perubahan dokumentasi cukup diperiksa terhadap sumber dan perintah yang ada; tidak perlu compile ulang.
- Perubahan Rust/Slint harus lolos build Windows dan Android jika menyentuh kode bersama. Perubahan Kotlin/manifest juga perlu `assembleRelease`.
- Untuk perubahan generator, periksa PSD/kemiringan spektrum, RMS, peak, DC, kontinuitas antar-buffer, dan respons pada 44,1/48/96 kHz. Bedakan respons filter matematis, sampel generator, rekaman output perangkat, dan hasil dengar.
- Uji interaksi: mulai setiap warna, berhenti dengan ketuk ulang, pindah warna, Minimalkan, pulihkan aplikasi, Tutup, serta pemulihan perangkat audio gagal.
- Uji Windows dan Android secara terpisah. Audio tetap berjalan saat minimize Android harus diamati langsung, termasuk pemulihan lewat ikon/notifikasi.
- Saat mengemas, verifikasi hash artefak build terhadap salinan di dist. Pastikan APK berisi library native terbaru.
- Laporkan apa yang benar-benar diverifikasi. Jangan menyamakan build sukses dengan UI, audio, atau lifecycle yang sudah diuji.

## Privasi dan distribusi
- Jangan commit `dist/`, `app/target/`, cache Gradle, jniLibs hasil build, atau `fi-focus.keystore`. Periksa `.gitignore` saat menambah keluaran build baru.
- Signing memakai `android/keystore.properties` lokal yang di-ignore. Salin dari `android/keystore.properties.example` dan isi kredensial sendiri. Tanpa file ini, `assembleRelease` menghasilkan APK unsigned (`app-release-unsigned.apk`), bukan `app-release.apk`. Jangan mencetak kredensial atau mengganti keystore.
- Jangan menulis path profil pribadi, nama komputer, atau kredensial baru ke dokumentasi yang akan dipublikasikan. Gunakan environment variable untuk lokasi SDK/JDK.
- Jangan menambahkan telemetry, perekaman mikrofon, atau jaringan tanpa diminta.

## Workflow
- Commit dan push hanya jika diminta. Pesan commit singkat, bahasa Inggris. Periksa dahulu apakah workspace sudah berupa repository Git.
- Buat perubahan minimum sesuai gaya kode yang ada. Hindari reformat massal, refactor, dan cleanup di luar tugas.
- Analisis/review bersifat read-only kecuali perbaikan diminta. Jangan mengubah sumber referensi hanya karena kode native berubah.
- Balas dalam bahasa Indonesia, langsung ke inti. Jelaskan batas verifikasi dan ketidakpastian tanpa mengklaim hasil yang belum diamati.
