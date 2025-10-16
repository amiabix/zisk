# Writing Programs

This guide explains how to write, build, test, and prove Rust programs for execution within the ZisK virtual machine. Programs written for ZisK follow standard Rust conventions with minimal modifications to integrate with the ZisK execution environment.

> **For distributed proof generation:** If you're planning to use the distributed proving system, see the [Setup guide](../developer/setup.md) for specific instructions on writing programs for distributed execution.

## Overview and Prerequisites

### Program Development Model

Writing programs for ZisK involves developing Rust code that targets the ZisK RISC-V instruction set. The development workflow includes:

- Writing Rust code with ZisK-specific entry point declarations
- Handling input/output through ZisK's I/O interfaces
- Building the program to RISC-V bytecode using cargo-zisk
- Testing execution with the emulator
- Generating cryptographic proofs of execution

### Key Differences from Standard Rust

Programs for ZisK differ from standard Rust programs in only a few ways:

- The main function must be marked with the `#![no_main]` attribute and wrapped with the `ziskos::entrypoint!()` macro
- Input data is read using `ziskos::read_input()` instead of standard I/O
- Output is written using `ziskos::set_output()` instead of standard I/O

## Basic Setup

For basic ZisK program setup, you need to make minimal changes to a standard Rust program:

1. **Modify `main.rs`**: Add `#![no_main]` and `ziskos::entrypoint!(main);`
2. **Modify `Cargo.toml`**: Add the `ziskos` dependency

For detailed setup instructions and examples, see the [Setup guide](../developer/setup.md) in the Developer Guide section.

### Example program    

`main.rs`:
```rust
// This example program takes a number `n` as input and computes the SHA-256 hash `n` times sequentially.

// Mark the main function as the entry point for ZisK
#![no_main]
ziskos::entrypoint!(main);

use sha2::{Digest, Sha256};
use std::convert::TryInto;
use ziskos::{read_input, set_output};
use byteorder::ByteOrder;

fn main() {
    // Read the input data as a byte array from ziskos
    let input: Vec<u8> = read_input();

    // Get the 'n' value converting the input byte array into a u64 value
    let n: u64 = u64::from_le_bytes(input.try_into().unwrap());

    let mut hash = [0u8; 32];

    // Compute SHA-256 hashing 'n' times
    for _ in 0..n {
        let mut hasher = Sha256::new();
        hasher.update(hash);
        let digest = &hasher.finalize();
        hash = Into::<[u8; 32]>::into(*digest);
    }

    // Split 'hash' value into chunks of 32 bits and write them to ziskos output
    for i in 0..8 {
        let val = byteorder::BigEndian::read_u32(&mut hash[i * 4..i * 4 + 4]);
        set_output(i, val);
    }
}
```

`Cargo.toml`:
```toml
[package]
name = "sha_hasher"
version = "0.1.0"
edition = "2021"
default-run = "sha_hasher"

[dependencies]
byteorder = "1.5.0"
sha2 = "0.10.8"
ziskos = { git = "https://github.com/0xPolygonHermez/zisk.git" }
```

> **For detailed input/output handling:** See the [I/O Model](../developer/io_privacy_model.md) guide for comprehensive information on handling input data, output data, and the privacy model.

## Build

Before compiling your program for ZisK, you can test it on the native architecture just like any regular Rust program using the `cargo` command.

Once your program is ready to run on ZisK, compile it into an ELF file (RISC-V architecture), using the `cargo-zisk` CLI tool:

```bash
cargo-zisk build
```

This command compiles the program using the `zisk` target. The resulting `sha_hasher` ELF file (without extension) is generated in the `./target/riscv64ima-zisk-zkvm-elf/debug` directory.

For production, compile the ELF file with the `--release` flag, similar to how you compile Rust projects:

```bash
cargo-zisk build --release
```

In this case, the `sha_hasher` ELF file will be generated in the `./target/riscv64ima-zisk-zkvm-elf/release` directory.

## Execute

You can test your compiled program using the ZisK emulator (`ziskemu`) before generating a proof. Use the `-e` (`--elf`) flag to specify the location of the ELF file and the `-i` (`--inputs`) flag to specify the location of the input file:

```bash
cargo-zisk build --release
ziskemu -e target/riscv64ima-zisk-zkvm-elf/release/sha_hasher -i build/input.bin
```

Alternatively, you can build and execute the program in the ZisK emulator with a single command:

```bash
cargo-zisk run --release -i build/input.bin
```

If the program requires a large number of ZisK steps, you might encounter the following error:
```
Error during emulation: EmulationNoCompleted
Error: Error executing Run command
```

To resolve this, you can increase the number of execution steps using the `-n` (`--max-steps`) flag. For example:
```bash
ziskemu -e target/riscv64ima-zisk-zkvm-elf/release/sha_hasher -i build/input.bin -n 10000000000
```

## Metrics and Statistics

### Performance Metrics
You can get performance metrics related to the program execution in ZisK using the `-m` (`--log-metrics`) flag in the `cargo-zisk run` command or in `ziskemu` tool:

```bash
cargo-zisk run --release -i build/input.bin -m
```

Or

```bash
ziskemu -e target/riscv64ima-zisk-zkvm-elf/release/sha_hasher -i build/input.bin -m
```

The output will include details such as execution time, throughput, and clock cycles per step:
```
process_rom() steps=85309 duration=0.0009 tp=89.8565 Msteps/s freq=3051.0000 33.9542 clocks/step
98211882
bd13089b
6ccf1fca
...
```

### Execution Statistics
You can get statistics related to the program execution in Zisk using the `-x` (`--stats`) flag in the `cargo-zisk run` command or in `ziskemu` tool:

```bash
cargo-zisk run --release -i build/input.bin -x
```

Or

```bash
ziskemu -e target/riscv64ima-zisk-zkvm-elf/release/sha_hasher -i build/input.bin -x
```

The output will include details such as cost definitions, total cost, register reads/writes, opcode statistics, etc:
```
Cost definitions:
    AREA_PER_SEC: 1000000 steps
    COST_MEMA_R1: 0.00002 sec
    COST_MEMA_R2: 0.00004 sec
    COST_MEMA_W1: 0.00004 sec
    COST_MEMA_W2: 0.00008 sec
    COST_USUAL: 0.000008 sec
    COST_STEP: 0.00005 sec

Total Cost: 12.81 sec
    Main Cost: 4.27 sec 85308 steps
    Mem Cost: 2.22 sec 222052 steps
    Mem Align: 0.05 sec 2701 steps
    Opcodes: 6.24 sec 1270 steps (81182 ops)
    Usual: 0.03 sec 4127 steps
    Memory: 135563 a reads + 1625 na1 reads + 10 na2 reads + 84328 a writes + 524 na1 writes + 2 na2 writes = 137198 reads + 84854 writes = 222052 r/w

Opcodes:
    flag: 0.00 sec (0 steps/op) (89 ops)
    copyb: 0.00 sec (0 steps/op) (10568 ops)
    add: 1.12 sec (77 steps/op) (14569 ops)
    ltu: 0.01 sec (77 steps/op) (101 ops)
    ...
    xor: 1.06 sec (77 steps/op) (13774 ops)
    signextend_b: 0.03 sec (109 steps/op) (320 ops)
    signextend_w: 0.03 sec (109 steps/op) (320 ops)

98211882
bd13089b
6ccf1fca
...
```

## Prove

### Program Setup

Before generating a proof (or verifying the constraints), you need to generate the program setup files. This must be done the first time after building the program ELF file, or any time it changes:

```bash
cargo-zisk rom-setup -e target/riscv64ima-zisk-zkvm-elf/release/sha_hasher -k $HOME/.zisk/provingKey
```
In this command:

* `-e` (`--elf`) specifies the ELF file location.
* `-k` (`--proving-key`) specifies the directory containing the proving key. This is optional and defaults to `$HOME/.zisk/provingKey`.

The program setup files will be generated in the `cache` directory located at `$HOME/.zisk`.

To clean the `cache` directory content, use the following command:
```bash
cargo-zisk clean
```

### Verify Constraints

Before generating a proof (which can take some time), you can verify that all constraints are satisfied:

```bash
LIB_EXT=$([[ "$(uname)" == "Darwin" ]] && echo "dylib" || echo "so")
cargo-zisk verify-constraints -e target/riscv64ima-zisk-zkvm-elf/release/sha_hasher -i build/input.bin -w $HOME/.zisk/bin/libzisk_witness.$LIB_EXT -k $HOME/.zisk/provingKey
```
In this command:

* `-e` (`--elf`) specifies the ELF file location.
* `-i` (`--input`) specifies the input file location.
* `-w` (`--witness`) specifies the location of the witness library. This is optional and defaults to `$HOME/.zisk/bin/libzisk_witness.$LIB_EXT`.
* `-k` (`--proving-key`) specifies the directory containing the proving key. This is optional and defaults to `$HOME/.zisk/provingKey`.

If everything is correct, you will see an output similar to:

```
[INFO ] GlCstVfy: --> Checking global constraints
[INFO ] CstrVrfy: ··· ✓ All global constraints were successfully verified
[INFO ] CstrVrfy: ··· ✓ All constraints were verified
```

### Generate Proof

To generate a proof, run the following command:

```bash
LIB_EXT=$([[ "$(uname)" == "Darwin" ]] && echo "dylib" || echo "so")
cargo-zisk prove -e target/riscv64ima-zisk-zkvm-elf/release/sha_hasher -i build/input.bin -w $HOME/.zisk/bin/libzisk_witness.$LIB_EXT -k $HOME/.zisk/provingKey -o proof -a -y
```
In this command:

* `-e` (`--elf`) specifies the ELF file location.
* `-i` (`--input`) specifies the input file location.
* `-w` (`--witness`) specifies the location of the witness library. This is optional and defaults to `$HOME/.zisk/bin/libzisk_witness.$LIB_EXT`.
* `-k` (`--proving-key`) specifies the directory containing the proving key. This is optional and defaults to `$HOME/.zisk/provingKey`.
* `-o` (`--output`) determines the output directory (in this example `proof`).
* `-a` (`--aggregation`) indicates that a final aggregated proof (containing all generated sub-proofs) should be produced.
* `-y` (`--verify-proofs`) instructs the tool to verify the proof immediately after it is generated (verification can also be performed later using the `cargo-zisk verify` command).

If the process is successful, you should see a message similar to:

```
...
[INFO ] ProofMan:     ✓ Vadcop Final proof was verified
[INFO ]      stop <<< GENERATING_VADCOP_PROOF 91706ms
[INFO ] ProofMan: Proofs generated successfully
```

### Concurrent Proof Generation

Zisk proofs can be generated using multiple processes concurrently to improve performance and scalability. The standard MPI (Message Passing Interface) approach is used to launch these processes, which can run either on the same server or across multiple servers.

To execute a Zisk proof using multiple processes, use the following command:

```bash
mpirun --bind-to none -np <num_processes> -x OMP_NUM_THREADS=<num_threads_per_process> -x RAYON_NUM_THREADS=<num_threads_per_process> target/release/cargo-zisk <zisk arguments>
```
In this command:

* `<num_processes>` specifies the number of processes to launch.
* `<num_threads_per_process>` sets the number of threads used by each process via the `OMP_NUM_THREADS` and `RAYON_NUM_THREADS` environment variables.
* `--bind-to none` prevents binding processes to specific cores, allowing the operating system to schedule them dynamically for better load balancing.

Running a Zisk proof with multiple processes enables efficient workload distribution across multiple servers. **On a single server with many cores, splitting execution into smaller subsets of cores generally improves performance by increasing concurrency**. As a general rule, `<num_processes>` * `<num_threads_per_process>` should match the number of available CPU cores or double that if hyperthreading is enabled.

The total memory requirement increases proportionally with the number of processes. If each process requires approximately 25GB of memory, running P processes will require roughly (25 * P)GB of memory. Ensure that the system has sufficient available memory to accommodate all running processes.

### GPU Proof Generation

Zisk proofs can also be generated using GPUs to significantly improve performance and scalability. 
Follow these steps to enable GPU support:

1. GPU support is only available for NVIDIA GPUs.

2. Make sure the [CUDA Toolkit](https://developer.nvidia.com/cuda-downloads) is installed.

3. Build Zisk with GPU support enabled. 
    GPU support must be enabled at compile time. Follow the instructions in the **Build ZisK** section under **Option 2: Building from source** in the [Installation](./installation.md) guide, but replace the build command with:
    ```bash
    cargo build --release --features gpu
    ```

4. Build Zisk on the target GPU server. 
    It is recommended to compile Zisk directly on the server where it will be executed. The binary will be optimized for the local GPU architecture, which can lead to better runtime performance.

You can combine GPU-based execution with concurrent proof generation using multiple processes, as described in the **Concurrent Proof Generation** section. 

> **Note:** GPU memory is typically more limited than CPU memory. When combining GPU execution with concurrent proof generation, ensure that each process has sufficient memory available on the GPU to avoid out-of-memory errors.

### Verify Proof

To verify a generated proof, use the following command:

```bash
cargo-zisk verify -p ./proof/vadcop_final_proof.bin -s $HOME/.zisk/provingKey/zisk/vadcop_final/vadcop_final.starkinfo.json -e $HOME/.zisk/provingKey/zisk/vadcop_final/vadcop_final.verifier.bin -k $HOME/.zisk/provingKey/zisk/vadcop_final/vadcop_final.verkey.json
```

In this command:

* `-p` (`--proof`) specifies the final proof file generated with cargo-zisk prove.
* The remaining flags specify the files required for verification; they are optional, set by default to the files found in the `$HOME/.zisk` directory.

## See Also

- [Setup](../developer/setup.md) for writing programs specifically for distributed proof generation
- [I/O Model](../developer/io_privacy_model.md) for detailed input/output handling
- [Quickstart](./quickstart.md) for a complete walkthrough example
