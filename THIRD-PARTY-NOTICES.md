# Lisensi dan atribusi

Kode asli fi-focus memakai [MIT License](LICENSE), Copyright (c) 2026 Alfi Maulana Akbar. Lisensi ini tidak mengganti lisensi komponen pihak ketiga.

## Referensi generator noise

Implementasi awal mengacu pada `5mdt_noise/noise`. Copyright © 2023 Vladimir @nett00n Budylnikov. Teks MIT asli dipertahankan di [LICENSE-5mdt.txt](LICENSE-5mdt.txt), termasuk ketika mendistribusikan turunannya. Folder referensi tidak disertakan dalam repositori ini.

Filter pink noise memakai koefisien Paul Kellet (juga dikenal sebagai Paul Kellett).

## Slint 1.18.1

[![Made with Slint](https://raw.githubusercontent.com/slint-ui/slint/master/logo/MadeWithSlint-logo-whitebg.png)](https://slint.dev)

Slint dan komponen internalnya dipakai melalui [Slint Royalty-free Desktop, Mobile, and Web Applications License 2.0](LICENSES/Slint-Royalty-free-2.0.md), bukan MIT. Ini berlaku untuk aplikasi desktop Windows dan mobile Android ini.

Badge atribusi di atas harus tetap tampil pada halaman publik yang mudah ditemukan. Saat menerbitkan repositori atau rilis, tampilkan juga badge ini pada README atau halaman unduhan. Tautan teks saja tidak menggantikan badge. Ketentuan lengkap dan pilihan lisensi lain tersedia pada [FAQ resmi Slint](https://github.com/slint-ui/slint/blob/master/FAQ.md).

## Dependensi langsung dan build tools

| Komponen | Lisensi upstream | Sumber |
| --- | --- | --- |
| CPAL 0.18.2 | Apache-2.0 | [RustAudio/cpal](https://github.com/RustAudio/cpal) |
| jni 0.21.1 | MIT atau Apache-2.0 | [jni-rs/jni-rs](https://github.com/jni-rs/jni-rs) |
| windows-sys 0.61.2 | MIT atau Apache-2.0 | [microsoft/windows-rs](https://github.com/microsoft/windows-rs) |
| windows 0.62.2 | MIT atau Apache-2.0 | [microsoft/windows-rs](https://github.com/microsoft/windows-rs) |
| raw-window-handle 0.6.2 | MIT atau Apache-2.0 atau Zlib | [rust-windowing/raw-window-handle](https://github.com/rust-windowing/raw-window-handle) |
| winresource 0.1.34 | MIT | [BenjaminRi/winresource](https://github.com/BenjaminRi/winresource) |
| slint-build 1.18.1 | Pilihan lisensi Slint di atas | [slint-ui/slint](https://github.com/slint-ui/slint) |
| Gradle Wrapper | Apache-2.0 | [gradle/gradle](https://github.com/gradle/gradle) |

Teks Apache-2.0 tersedia di [LICENSES/Apache-2.0.txt](LICENSES/Apache-2.0.txt). Salinan lisensi MIT dependensi langsung tersedia di [LICENSES/jni-MIT.txt](LICENSES/jni-MIT.txt), [LICENSES/windows-rs-MIT.txt](LICENSES/windows-rs-MIT.txt), dan [LICENSES/winresource-MIT.txt](LICENSES/winresource-MIT.txt). Header asli skrip Gradle dan lisensi di dalam JAR wrapper dipertahankan.

Dependensi Cargo tidak di-vendor. Versi resolusi ada dalam `app/Cargo.lock`; dependensi transitif beserta komponen native seperti Skia tetap memiliki lisensi masing-masing. Daftar ini bukan inventaris lengkap lisensi yang harus disertakan bersama EXE/APK. Sebelum distribusi biner, kumpulkan pula notice dan teks lisensi dependensi transitif yang benar-benar tertaut untuk tiap platform.

Binding `windows` memakai lisensi [windows-rs MIT](LICENSES/windows-rs-MIT.txt); lisensi MIT `raw-window-handle` tersedia di [LICENSES/raw-window-handle-MIT.txt](LICENSES/raw-window-handle-MIT.txt).

## Aset

Menurut keterangan pemilik proyek, logo dibuat dengan AI. `app-icon.png` dipertahankan sebagai gambar sumber beresolusi tinggi; `src/icon.png`, `src/logo.png`, `app/icon.ico`, dan ikon launcher Android adalah aset turunannya. Keterangan asal ini tidak menjamin eksklusivitas hak cipta atas hasil AI. Ikon SVG tombol dibuat sebagai bagian dari proyek.
