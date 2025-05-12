# ZisK Project Structure

This guide explains the organization of the ZisK codebase and how different components interact.

## Repository Structure

```
zisk/
├── book/                  # Documentation
├── cli/                   # Command-line interface tools
├── core/                  # Core ZisK functionality
├── data-bus/             # Data bus implementation
├── emulator/             # RISC-V emulator
├── emulator-asm/         # Assembly utilities
├── executor/             # Program execution engine
├── lib-c/                # C library bindings
├── pil/                  # PIL2 constraint system
├── precompiles/          # Precompiled functions
├── riscv/                # RISC-V instruction handling
├── rom-setup/            # ROM setup utilities
├── samples/              # Example programs
├── state-machines/       # State machine implementations
│   ├── arith/           # Arithmetic state machine
│   ├── binary/          # Binary operations state machine
│   ├── main/            # Main state machine
│   └── mem/             # Memory state machine
├── tools/                # Development tools
├── witness-computation/  # Witness generation
├── ziskclib/            # ZisK C library
├── ziskos/              # ZisK operating system
└── ziskup/              # ZisK utilities
```

## Core Components

### 1. Core (`core/`)
- Defines fundamental ZisK operations and types
- Handles RISC-V to ZisK transpilation
- Manages instruction context and memory

Key files:
- `zisk_ops.rs`: Operation definitions and implementations
- `zisk_inst.rs`: Instruction handling
- `zisk_rom.rs`: ROM management
- `riscv2zisk.rs`: RISC-V to ZisK conversion

### 2. State Machines (`state-machines/`)
- Implement different aspects of program execution
- Handle operation proofs and constraints

Components:
- **Main State Machine**: Coordinates execution
- **Arithmetic State Machine**: Handles math operations
- **Binary State Machine**: Manages bit operations
- **Memory State Machine**: Handles memory access

### 3. Emulator (`emulator/`)
- Provides RISC-V program emulation
- Generates execution traces
- Handles program state

### 4. Data Bus (`data-bus/`)
- Manages communication between components
- Handles operation data transfer
- Implements bus protocols

### 5. Precompiles (`precompiles/`)
- Optimized implementations of common operations
- Specialized cryptographic functions
- Performance-critical routines

## Compilation Flow

### 1. Supported Languages & Toolchain
- **Primary Language**: Rust
- **Target**: `riscv64ima-polygon-ziskos-elf`
- **Backend**: LLVM-based compilation pipeline

### 2. Compilation Pipeline
```
Rust Source → LLVM IR → RISC-V ELF → ROM Image → Execution Trace → STARK Proof
```

Detailed steps:
1. **Rust to LLVM IR**
   - Rust code is compiled to LLVM IR
   - Uses standard Rust compiler with custom target
   - Preserves program semantics and structure

2. **LLVM IR to RISC-V ELF**
   - LLVM backend generates RISC-V assembly
   - Creates ELF binary with program code
   - Includes necessary metadata and sections

3. **ELF to ROM Image**
   - ELF binary is processed for ROM setup
   - Generates initial state and constraints
   - Prepares for execution in ZisK environment

4. **Execution to Trace**
   - Program runs in ZisK environment
   - Generates execution trace
   - Captures all state transitions

5. **Trace to STARK Proof**
   - Trace is combined with PIL constraints
   - Proofman generates STARK proof
   - Creates verifiable proof of execution

### 3. Developer Tooling

#### Core Tools
- `cargo-zisk`: Main development tool
  - Builds RISC-V programs
  - Manages dependencies
  - Handles compilation pipeline

- `ziskemu`: Emulator
  - Tests programs before proof generation
  - Validates execution
  - Generates test traces

- `rom-setup`: ROM Preparation
  - Sets up ROM image
  - Generates initial state
  - Prepares constraint system

- `prove`: Proof Generation
  - Generates STARK proofs
  - Handles proof optimization
  - Manages proof files

- `verify`: Proof Verification
  - Verifies generated proofs
  - Validates execution correctness
  - Checks constraint satisfaction

#### Tool Usage Example
```bash
# Build program
cargo-zisk build --release

# Test in emulator
ziskemu -e target/.../program -i input.bin

# Setup ROM
cargo-zisk rom-setup -e target/.../program

# Generate proof
cargo-zisk prove -e target/.../program -i input.bin -o proof

# Verify proof
cargo-zisk verify -p proof/proofs/vadcop_final_proof.json -u proof/publics.json
```

### 4. Development Environment

#### Required Tools
- Rust toolchain (latest stable)
- LLVM toolchain
- ZisK development tools
- RISC-V toolchain

#### Configuration
- `rust-toolchain.toml`: Specifies Rust version
- `.zisk/config.toml`: ZisK-specific settings
- `Cargo.toml`: Project dependencies

## Development Workflow

### 1. Program Development
```