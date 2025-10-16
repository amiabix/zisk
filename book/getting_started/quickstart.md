# ZisK Quickstart Guide

This guide provides step-by-step instructions for installing ZisK, creating a sample program, building it for the ZisK virtual machine, executing it, and generating a cryptographic proof of its execution.

## Workflow Overview

This quickstart demonstrates the complete ZisK workflow using a SHA-256 hashing program as an example. The same workflow applies to any computational program you develop:

1. **Installation** – Install ZisK and verify system requirements
2. **Project Creation** – Initialize the SHA-256 hashing example program
3. **Build** – Compile the program to the ZisK RISC-V instruction set
4. **Execute** – Test the program using the ZisK emulator
5. **Setup** – Generate program-specific configuration files
6. **Prove** – Generate a cryptographic proof of correct execution
7. **Verify** – Validate the generated proof

**Note:** SHA-256 hashing is used throughout this guide as a practical example. The same procedures apply when developing and proving custom programs.

---

## 1. Installation

ZisK currently supports **Linux x86_64** and **macOS** platforms (see note below).

**Note:** On **macOS**, proof generation is not yet optimized, so some proofs may take longer to generate.

**Ubuntu 22.04 or higher** is required.

**macOS 14 or higher** with [Xcode](https://developer.apple.com/xcode/) installed is required.

1. Make sure you have [Rust](https://www.rust-lang.org/tools/install) installed.

2. Install all required dependencies with:
    - **Ubuntu**:
        ```bash
        sudo apt-get install -y xz-utils jq curl build-essential qemu-system libomp-dev libgmp-dev nlohmann-json3-dev protobuf-compiler uuid-dev libgrpc++-dev libsecp256k1-dev libsodium-dev libpqxx-dev nasm libopenmpi-dev openmpi-bin openmpi-common
        ```
    - **macOS**:
        ```bash
        brew reinstall jq curl libomp protobuf openssl nasm pkgconf open-mpi libffi
        ```    

3. To install ZisK using ziskup, run the following command in your terminal:
    ```bash
    curl https://raw.githubusercontent.com/0xPolygonHermez/zisk/main/ziskup/install.sh | bash
    ```

## 2. Project Creation

The first step is to generate a new example project using the `cargo-zisk sdk new <name>` command. This command creates a new directory named `<name>` in your current directory. For example:
```bash
cargo-zisk sdk new sha_hasher
cd sha_hasher
```

This will create a project with the following structure:

```
.
├── build.rs
├── Cargo.toml
├── .gitignore
└── src
    └── main.rs
```

The example program takes a number `n` as input and computes the SHA-256 hash `n` times. 

The `build.rs` file generates an `input.bin` file containing the value of `n` (e.g., 20). This file is used in `main.rs` as input to calculate the hash.

### Test the Program Locally

You can run the program on your native architecture with the following command:
```bash
cargo run
```

**What this does:**
- Runs the `build.rs` script which generates the `input.bin` file containing test data
- Executes the program natively on your local machine to show you what the output should look like
- This is a dual-purpose step - it's both input generation AND local testing
The output will be:
```
public 0: 0x98211882
public 1: 0xbd13089b
public 2: 0x6ccf1fca
public 3: 0x81f7f0e4
public 4: 0xabf6352a
public 5: 0x0c39c9b1
public 6: 0x1f142cac
public 7: 0x233f1280
```

## 3. Build

The next step is to build the program using the `cargo-zisk` command to generate an ELF file (RISC-V), which will be used later to generate the proof. Execute:

```bash
cargo-zisk build --release
```

**What this does:**
- Compiles the Rust code to RISC-V 64 architecture
- Links it with the ziskos runtime library
- Generates an ELF executable file that ZisK can execute and prove
- Translates your high-level program logic into RISC-V instructions that ZisK can execute and prove

This command builds the program using the `zkvm` target. The resulting `sha_hasher` ELF file (without extension) is generated in the `./target/riscv64ima-zisk-zkvm-elf/release` directory.

## 4. Execute

Before generating a proof, you can test the program using the ZisK emulator to ensure its correctness. Specify the ELF file (using the `-e` or `--elf flag`) and the input file `input.bin` (using the `-i` or `--inputs` flag):

```bash
ziskemu -e target/riscv64ima-zisk-zkvm-elf/release/sha_hasher -i build/input.bin
```

**What this does:**
- Parses the input file (`build/input.bin`) to extract the input data
- Loads the compiled ELF into the ZisK emulator
- Executes the program using the parsed input data
- Captures the execution trace needed for later proof generation
- Validates program correctness by running it in the ZisK environment

The output will be:
```
98211882
bd13089b
6ccf1fca
81f7f0e4
abf6352a
0c39c9b1
1f142cac
233f1280
```

Alternatively, you can build and run the program with:

```bash
cargo-zisk run --release -i build/input.bin
```

## 5. Setup

Before generating a proof, you need to generate the program setup files. Execute:

```bash
cargo-zisk rom-setup -e target/riscv64ima-zisk-zkvm-elf/release/sha_hasher
```

**What this does:**
- Converts the RISC-V ELF program into a ZisK ROM format
- Generates the necessary files for proof generation
- Creates a mathematical representation of your program that ZisK can use to generate proofs
- Computes ELF hash for identification
- Converts ELF to assembly files using multiple methods (minimal traces, ROM histogram, memory operations)
- Generates a Merkle root from the ROM trace
- Saves the processed ROM data to `~/.zisk/cache/` for later use

## 6. Prove

Once the program setup is complete, you can generate and verify a proof using the `cargo-zisk prove` command by providing the ELF file (with the `-e` or `--elf` flag) and the input file (with the `-i` or `--input` flag).

To generate and verify a proof for the previously built ELF and input files, execute:

```bash
cargo-zisk prove -e target/riscv64ima-zisk-zkvm-elf/release/sha_hasher -i build/input.bin -o proof -a -y
```

### Command Structure

Let's break down each component of the prove command:

- **ELF File (`-e` flag)**: The compiled RISC-V binary of your program
- **Input File (`-i` flag)**: Binary data that your program will process (`build/input.bin`)
- **Output Directory (`-o` flag)**: Where your generated proof will be saved (`proof`)
- **Flags (`-a` and `-y`)**:
  - `-a` (Aggregation): Combines all individual proofs into one unified proof
  - `-y` (Verify): Automatically verifies your proof after generation

---

### Proof Generation Phases

The proof generation process consists of several phases:

1. **Initialization and Setup**: Sets up the proof generation environment and initializes proofman
2. **Witness Registration and Execution Planning**: Registers the witness and plans proof generation
3. **Trace Computation**: Executes the RISC-V program and computes the execution trace
4. **Proof Planning and Memory Analysis**: Analyzes the execution trace and plans proof generation
5. **AIR Instance Analysis**: Decomposes the program into specialized mathematical components
6. **Memory Usage Calculation**: Calculates total memory needed for proof generation
7. **Witness Generation**: Generates mathematical witness data using all available CPU cores
8. **Individual AIR Proof Generation**: Creates separate proofs for each AIR instance
9. **Recursive Proof Aggregation**: Combines all proofs into a single unified proof
10. **Final Verification**: Automatically verifies the generated proof

This command generates the proof in the `./proof` directory. If everything goes well, you will see a message similar to:

```
...
[INFO ] ProofMan:     ✓ Vadcop Final proof was verified
[INFO ]      stop <<< GENERATING_VADCOP_PROOF 91706ms
[INFO ] ProofMan: Proofs generated successfully
```

**Note**: You can use concurrent proof generation and GPU support to reduce proving time. For more details, refer to the [Writing Programs](./writing_programs.md) guide.

## 7. Verify

To verify a generated proof, use the following command:

```bash
cargo-zisk verify -p ./proof/vadcop_final_proof.bin
```

**What this does:**
- Verifies the generated proof using public verification keys
- Confirms the proof is valid
- Demonstrates that anyone can verify your computation without running the program themselves

---

## ZisK Architecture

ZisK decomposes computation into domain-specific state machines, each optimized for different mathematical requirements:

- **Main State Machine**: Orchestrates execution flow and coordinates all components
- **Arithmetic State Machine**: Handles mathematical operations (ArithFullSM, ArithTableSM, ArithRangeTableSM)
- **Binary State Machine**: Manages bitwise operations (BinaryBasicSM, BinaryExtensionSM, BinaryAddSM)
- **Memory State Machine**: Controls memory access (MemSM, MemAlignSM, InputDataSM, RomDataSM)
- **ROM State Machine**: Manages program storage and instruction fetching

This modular approach allows each state machine to use the most efficient proving techniques for its specific type of operations, rather than trying to prove everything with a single, general-purpose system.

## Developer Experience

ZisK provides an excellent developer experience with:

- **Single command pipeline**: `cargo-zisk build` → `cargo-zisk rom-setup` → `cargo-zisk prove` → `cargo-zisk verify`
- **Standard Rust development**: Write programs using familiar Rust syntax
- **Deterministic execution**: Predictable behavior simplifies debugging
- **Comprehensive tooling**: Built-in debugging, profiling, and optimization tools

The result is a zkVM that doesn't just generate proofs - it does so efficiently, scalably, and with a developer experience that makes zero-knowledge programming accessible to mainstream developers.
