# Femboy - ColorOS Update Binary

Binary installer kustom untuk POCO X5 5G yang berfungsi sebagai `update-binary` dalam paket OTA.

## Fitur
- **Slot Detection**: Otomatis mendeteksi slot aktif (A/B) dan melakukan instalasi ke slot pasif.
- **Dynamic Partitions**: Melakukan mapping/unmapping partisi logical (system, vendor, etc) menggunakan `lptools`.
- **Payload Extraction**: Mengekstrak image langsung ke block device menggunakan `7za` static dan `dd`.
- **Static Binary**: Dikompilasi secara statis untuk berjalan di environment Recovery/TWRP tanpa dependency.

## Struktur Project
- `src/main.rs`: Source code installer.
- `build.sh`: Script kompilasi otomatis ke ARM64.
- `.cargo/config.toml`: Konfigurasi cross-compiler & static linking.

## Cara Build
```bash
chmod +x build.sh
./build.sh
```
Hasil binary akan muncul sebagai `./out/update-binary`.

## Penggunaan
Ganti `update-binary` bawaan di dalam zip OTA (`META-INF/com/google/android/update-binary`) dengan binary ini.
Pastikan tools pendukung (`lptools`, `bootctl`, `unzip_64`) tersedia di folder `bin/` di dalam zip.
