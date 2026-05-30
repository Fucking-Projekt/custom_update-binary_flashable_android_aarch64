#!/bin/bash
set -e

# Path to workspace
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$DIR"

echo "=================================================="
echo " Building Statically Linked & Stripped update-binary"
echo " Target: AArch64 (Android ARM64)"
echo "=================================================="

# Check if target is installed, if not try to add it
if ! rustup target list | grep -q "aarch64-unknown-linux-musl (installed)"; then
    echo "Adding rustup target aarch64-unknown-linux-musl..."
    rustup target add aarch64-unknown-linux-musl
fi

# Check for required cross-compilers and abort with instructions if any are missing
MISSING_PKGS=()
if ! command -v aarch64-linux-gnu-gcc &> /dev/null; then
    MISSING_PKGS+=("gcc-aarch64-linux-gnu")
fi
if ! command -v aarch64-linux-gnu-g++ &> /dev/null; then
    MISSING_PKGS+=("g++-aarch64-linux-gnu")
fi

if [ ${#MISSING_PKGS[@]} -gt 0 ]; then
    echo "=================================================="
    echo " Error: Required cross-compiler tools are missing!"
    echo " Missing package(s): ${MISSING_PKGS[*]}"
    echo " Please install them by running:"
    echo "   sudo apt-get update && sudo apt-get install -y ${MISSING_PKGS[*]}"
    echo "=================================================="
    exit 1
fi

# Linker configuration
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER="aarch64-linux-gnu-gcc"

echo "Building release..."
cargo build --release --target aarch64-unknown-linux-musl

# Locate output binary
BIN_PATH="target/aarch64-unknown-linux-musl/release/update-binary"

echo "Stripping binary..."
if command -v aarch64-linux-gnu-strip &> /dev/null; then
    aarch64-linux-gnu-strip --strip-all "$BIN_PATH"
elif command -v strip &> /dev/null; then
    strip --strip-all "$BIN_PATH"
else
    echo "Warning: Strip utility not found, keeping Cargo stripped output."
fi

# Create out directories
mkdir -p ./out/bin

# Copy update-binary to out directory
cp "$BIN_PATH" ./out/update-binary
chmod +x ./out/update-binary

# Compile 64-bit static ARM64 p7zip (7za) from source
if [ ! -f ./out/bin/unzip_64 ] || [ "$1" == "clean" ]; then
    echo "=================================================="
    echo " Compiling p7zip v17.06 from source (AArch64)..."
    echo "=================================================="
    TEMP_DIR="/tmp/p7zip-build"
    rm -rf "$TEMP_DIR"
    mkdir -p "$TEMP_DIR"
    
    echo "Fetching p7zip v17.06 source code..."
    if curl -sL https://github.com/p7zip-project/p7zip/archive/refs/tags/v17.06.tar.gz | tar -xz -C "$TEMP_DIR" --strip-components=1; then
        cd "$TEMP_DIR"
        cp makefile.linux_any_cpu makefile.machine
        
        echo "Running make 7za with -Os (Size Optimization) and static linking..."
        make 7za -j$(nproc) CC=aarch64-linux-gnu-gcc CXX=aarch64-linux-gnu-g++ OPTFLAGS="-Os -static" LDFLAGS="-static"
        
        # Return to workspace
        cd "$DIR"
        
        # Copy to out directory
        cp "$TEMP_DIR/bin/7za" ./out/bin/unzip_64
        
        # Strip the binary
        echo "Stripping unzip_64..."
        if command -v aarch64-linux-gnu-strip &> /dev/null; then
            aarch64-linux-gnu-strip --strip-all ./out/bin/unzip_64
        elif command -v strip &> /dev/null; then
            strip --strip-all ./out/bin/unzip_64
        fi
        
        chmod +x ./out/bin/unzip_64
        echo "Successfully compiled, stripped, and added unzip_64 to out/bin/"
    else
        echo "Error: Failed to fetch p7zip source code."
        exit 1
    fi
    rm -rf "$TEMP_DIR"
else
    echo "Static unzip_64 already compiled and cached in out/bin/unzip_64."
    
    # Ensure cached version is stripped
    if command -v aarch64-linux-gnu-strip &> /dev/null; then
        aarch64-linux-gnu-strip --strip-all ./out/bin/unzip_64
    elif command -v strip &> /dev/null; then
        strip --strip-all ./out/bin/unzip_64
    fi
fi

echo "=================================================="
echo " Build Success!"
echo " Output path: ./out/update-binary"
echo " Additional tools: ./out/bin/unzip_64"
echo " File details:"
file ./out/update-binary
ls -lh ./out/update-binary ./out/bin/unzip_64
echo "=================================================="
