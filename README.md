# Android AArch64 Rust update-binary

A fully optimized, statically linked, stripped, and compile-time string-obfuscated `update-binary` rewritten in **Rust** for Android recovery environments (like TWRP). 

This project solves the standard 32-bit zip size limitations (which skip large partition files like 5GB+ `system.img` due to ZIP32 limits) by utilizing a custom-built, size-optimized 64-bit static `unzip_64` binary compiled directly from `p7zip v17.06` source code.

> [!NOTE]
> The configurations and branding headers (`ui_print`) included in this codebase are configured as an example for **MIUI**. For other ROMs, you can easily adjust them by modifying the branding header strings in `src/main.rs`.

---

## Key Performance Optimizations

1. **High-Performance Direct Buffering**:
   - Spawning `dd` and piping decompressions (e.g. `unzip | dd`) inside shell scripts creates massive bottlenecking and thousands of CPU context switches. 
   - This binary eliminates `dd` entirely, reading the stream directly from `unzip_64` and writing to partition mapper files using a highly-efficient **8MB RAM sequential buffer**.

2. **NAND Flash Write Optimization**:
   - Repeated calls to `fsync` (`conv=fsync` in `dd`) on small write chunks throttle UFS/eMMC speeds down to less than 5MB/s.
   - This implementation bypasses repeated cache flushes, writing data sequentially at full UFS hardware limits, and calls a single `fsync` (`sync_all()`) **exactly once** at the end of each partition flash to guarantee data integrity.

3. **Compile-Time String Obfuscation**:
   - Protects your console outputs and branding strings from static binary analysis (`strings update-binary`). 
   - Utilizes the `obfstr` macro to encrypt all raw strings during compilation with unique runtime decryption keys.

---

## Directory Structure

```
update-binary/
├── Cargo.toml            # Size-optimized Cargo configuration (LTO, opt-level=z, abort panic)
├── build.sh              # Automatic compile-from-source build script
├── .gitignore            # Git exclusion rules
├── src/
│   └── main.rs           # Fully optimized & obfuscated Rust source code
├── update-binary.sh      # Backup of the original shell script
└── out/                  # Output directory (generated after running build.sh)
    ├── update-binary     # Static, stripped, and obfuscated ARM64 Rust binary (453K)
    └── bin/
        └── unzip_64      # Static, stripped ARM64 p7zip v17.06 binary (3.4M)
```

---

## How to Build

The provided `build.sh` script automates compilation of both the Rust `update-binary` and the C++ `p7zip` from source for `aarch64`.

### Prerequisites

Ensure you have the Rust compiler (`rustup` / `cargo`) and the C++ cross-compiler toolchain installed:
```bash
sudo apt-get install -y gcc-aarch64-linux-gnu g++-aarch64-linux-gnu
```

### Execution

1. **Standard Build** (fast compilation, uses cache if `unzip_64` is already built):
   ```bash
   ./build.sh
   ```

2. **Clean Build** (forces complete recompilation of `p7zip v17.06` from source):
   ```bash
   ./build.sh clean
   ```

All output binaries ready to be bundled into your ROM/Recovery ZIP will be automatically generated and placed under the `out/` directory.

---

## Packaging into a ROM Zip

To package the generated tools into your flashable ROM/Recovery ZIP, map them as follows:
- Rename `out/update-binary` to `META-INF/com/google/android/update-binary`
- Place `out/bin/unzip_64` into the ZIP's `bin/unzip_64` directory.
