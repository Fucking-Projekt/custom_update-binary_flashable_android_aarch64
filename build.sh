#!/bin/bash
set -e

echo "[*] Femboy (update-binary) Cross-Compiler for ARM64"

# Function to check for passwordless sudo
can_sudo() {
    sudo -n true 2>/dev/null
}

# 1. Check & Install GCC Cross-Compiler
if ! command -v aarch64-linux-gnu-gcc &> /dev/null; then
    echo "[!] aarch64-linux-gnu-gcc not found."
    if can_sudo; then
        echo "[*] Sudo access available without password. Installing dependencies..."
        sudo apt-get update && sudo apt-get install -y gcc-aarch64-linux-gnu
    else
        echo "[!] Sudo not available or requires password. Please install manually:"
        echo "    sudo apt install gcc-aarch64-linux-gnu"
        exit 1
    fi
fi

# 2. Check & Install Rust/Cargo
if ! command -v cargo &> /dev/null; then
    echo "[!] Cargo/Rust not found."
    if can_sudo; then
        echo "[*] Installing Rust via apt..."
        sudo apt-get install -y rustc cargo
    else
        echo "[*] Attempting to install Rust via rustup (non-sudo)..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
    fi
fi

# 3. Add rustup target
echo "[*] Ensuring aarch64-unknown-linux-musl target is installed..."
if command -v rustup &> /dev/null; then
    rustup target add aarch64-unknown-linux-musl 2>/dev/null || true
else
    echo "[!] Warning: rustup not found, skipping target add. Ensure the target is available."
fi

# 4. Build
echo "[*] Compiling static binaries..."
cargo build --target aarch64-unknown-linux-musl --release

# 5. Strip
echo "[*] Stripping binaries..."
mkdir -p out
cp target/aarch64-unknown-linux-musl/release/femboy out/update-binary
cp target/aarch64-unknown-linux-musl/release/unzip_64 out/unzip_64

if command -v aarch64-linux-gnu-strip &> /dev/null; then
    aarch64-linux-gnu-strip out/update-binary
    aarch64-linux-gnu-strip out/unzip_64
else
    echo "[!] Warning: aarch64-linux-gnu-strip not found. Binaries will not be stripped."
fi

echo ""
echo "[+] Success! Binaries ready at: ./out/"
ls -lh out/
file out/*
