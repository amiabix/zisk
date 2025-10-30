# Installation Guide

ZisK can be installed from prebuilt binaries (recommended) or by building from source. This guide covers both options with clear prerequisites and step-by-step instructions.

## Table of Contents

- [Quick Start (Recommended)](#quick-start-recommended)
- [System Requirements](#system-requirements)
- [Installing Dependencies](#installing-dependencies)
- [Installation Options](#installation-options)
  - [Option 1: Prebuilt Binaries](#option-1-prebuilt-binaries-recommended)
  - [Option 2: Building from Source](#option-2-building-from-source)
- [Uninstalling ZisK](#uninstalling-zisk)

---

## Quick Start (Recommended)

For most users, we recommend the prebuilt binary installation:

```bash
curl https://raw.githubusercontent.com/0xPolygonHermez/zisk/main/ziskup/install.sh | bash
```

This will install ZisK with all necessary components. See [Prebuilt Binaries](#option-1-prebuilt-binaries-recommended) for detailed options.

---

## System Requirements

### Supported Platforms
- **Linux x86_64** (Ubuntu 22.04+)
- **macOS** (14+)

> **Note:** On macOS, proof generation is not yet optimized, so some proofs may take longer to generate.

### Prerequisites
- [Rust](https://www.rust-lang.org/tools/install)
- [Git](https://git-scm.com/book/en/v2/Getting-Started-Installing-Git)

## Installing Dependencies

### Ubuntu

Ubuntu 22.04 or higher is required.

Install all required dependencies with:
```bash
sudo apt-get install -y xz-utils jq curl build-essential qemu-system libomp-dev libgmp-dev nlohmann-json3-dev protobuf-compiler uuid-dev libgrpc++-dev libsecp256k1-dev libsodium-dev libpqxx-dev nasm libopenmpi-dev openmpi-bin openmpi-common libclang-dev clang gcc-riscv64-unknown-elf
```

ZisK uses shared memory to exchange data between processes. The system must be configured to allow enough locked memory per process:
```text
$ ulimit -l
unlimited
```
A way to achieve it is to edit the file `/etc/systemd/system.conf` and add the line `DefaultLimitMEMLOCK=infinity`. Reboot for changes to take effect.

### macOS

macOS 14 or higher is required.

You must have [Homebrew](https://brew.sh/) and [Xcode](https://developer.apple.com/xcode/) installed.

Install all required dependencies with:
```bash
brew reinstall jq curl libomp protobuf openssl nasm pkgconf open-mpi libffi nlohmann-json libsodium riscv-tools
```

---

## Installation Options

### Option 1: Prebuilt Binaries (Recommended)

The easiest way to install ZisK is using the automated installer:

#### Installation Steps

1. **Run the installer:**
    ```bash
    curl https://raw.githubusercontent.com/0xPolygonHermez/zisk/main/ziskup/install.sh | bash
    ```

2. **Choose setup option:**
    - **Install proving key (default)** – Required for generating and verifying proofs
    - **Install verify key** – Needed only if you want to verify proofs  
    - **None** – Choose this if you only want to compile programs and execute them using the ZisK emulator

3. **Verify installation:**
    ```bash
    # Check Rust toolchain
    rustup toolchain list
    # Should show: zisk
    
    # Check CLI tool
    cargo-zisk --version
    ```

#### Updating ZisK

To update to the latest version:
```bash
ziskup
```

Use flags to skip prompts: `--provingkey`, `--verifykey`, or `--nokey`

---

### Option 2: Building from Source

For developers who want to build ZisK from source or contribute to the project.

#### Build ZisK Tools

1. **Clone and build:**
    ```bash
    git clone https://github.com/0xPolygonHermez/zisk.git
    cd zisk
    cargo build --release
    ```

2. **Troubleshooting Ubuntu build:**
    If you encounter `'stddef.h' file not found` error:
    ```bash
    # Find stddef.h
    find /usr -name "stddef.h"
    
    # Set include paths (adjust path as needed)
    export C_INCLUDE_PATH=/usr/lib/gcc/x86_64-linux-gnu/13/include
    export CPLUS_INCLUDE_PATH=$C_INCLUDE_PATH
    
    # Try building again
    cargo build --release
    ```

3. **Install tools:**
    ```bash
    mkdir -p $HOME/.zisk/bin
    LIB_EXT=$([[ "$(uname)" == "Darwin" ]] && echo "dylib" || echo "so")
    cp target/release/cargo-zisk target/release/ziskemu target/release/riscv2zisk \
       target/release/zisk-coordinator target/release/zisk-worker \
       target/release/libzisk_witness.$LIB_EXT target/release/libziskclib.a $HOME/.zisk/bin
    ```

4. **Setup assembly files (Linux only):**
    ```bash
    mkdir -p $HOME/.zisk/zisk/emulator-asm
    cp -r ./emulator-asm/src $HOME/.zisk/zisk/emulator-asm
    cp ./emulator-asm/Makefile $HOME/.zisk/zisk/emulator-asm
    cp -r ./lib-c $HOME/.zisk/zisk
    ```

5. **Add to PATH:**
    ```bash
    PROFILE=$([[ "$(uname)" == "Darwin" ]] && echo ".zshenv" || echo ".bashrc")
    echo "export PATH=\"\$PATH:$HOME/.zisk/bin\"" >> $HOME/$PROFILE
    source $HOME/$PROFILE
    ```

6. **Install Rust toolchain:**
    ```bash
    cargo-zisk sdk install-toolchain
    ```

7. **Verify installation:**
    ```bash
    rustup toolchain list  # Should show 'zisk'
    ```

#### Build Setup Files (Advanced)

> **Note:** This process takes 45-60 minutes and requires [NodeJS 20.x+](https://nodejs.org/en/download)

**Prerequisites:** Complete the "Build ZisK Tools" section above.

1. **Clone dependencies:**
    ```bash
    cd ..  # Go to parent directory of zisk
    git clone https://github.com/0xPolygonHermez/pil2-compiler.git
    git clone https://github.com/0xPolygonHermez/pil2-proofman.git
    git clone https://github.com/0xPolygonHermez/pil2-proofman-js
    ```

2. **Install packages:**
    ```bash
    (cd pil2-compiler && npm i)
    (cd pil2-proofman-js && npm i)
    ```

3. **Generate setup data:**
    ```bash
    cd zisk
    
    # Generate fixed data
    cargo run --release --bin keccakf_fixed_gen
    cargo run --release --bin arith_frops_fixed_gen
    cargo run --release --bin binary_basic_frops_fixed_gen
    cargo run --release --bin binary_extension_frops_fixed_gen
    
    # Compile ZisK PIL
    node ../pil2-compiler/src/pil.js pil/zisk.pil \
        -I pil,../pil2-proofman/pil2-components/lib/std/pil,state-machines,precompiles \
        -o pil/zisk.pilout -u tmp/fixed -O fixed-to-file
    
    # Generate proving key (30-45 minutes)
    node ../pil2-proofman-js/src/main_setup.js \
        -a ./pil/zisk.pilout -b build \
        -t ../pil2-proofman/pil2-components/lib/std/pil \
        -u tmp/fixed -r
    
    # Install proving key
    cp -R build/provingKey $HOME/.zisk
    cargo-zisk check-setup -a
    ```

---

## Uninstalling ZisK

To completely remove ZisK from your system:

```bash
# Remove Rust toolchain
rustup uninstall zisk

# Remove ZisK files
rm -rf $HOME/.zisk
```

---

> **Next Steps:** After installation, check out the [Quickstart Guide](./quickstart.md) to create your first ZisK program.