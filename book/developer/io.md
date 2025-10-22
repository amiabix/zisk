# I/O Model

ZisK enforces privacy through a controlled input/output model. This document explains how programs handle inputs and outputs, covering:

- How ZisK's input/output model works
- How dynamic data streams are handled
- How the system decides what becomes public and what stays private

## Table of Contents

- [Basic I/O Examples](#basic-io-examples)
- [Input Processing](#input-processing)
- [Privacy by Default](#privacy-by-default)
- [Making Data Public](#making-data-public)
- [Working with Dynamic Data Streams](#working-with-dynamic-data-streams)
- [Working with Complex Data](#working-with-complex-data)
- [Key Takeaways](#key-takeaways)

---

## Basic I/O Examples

To provide input data for ZisK, write that data in a binary file (e.g., `input.bin`).

If your program requires complex input data, consider using a serialization mechanism (like [`bincode`](https://crates.io/crates/bincode) crate) to store it in the `input.bin` file.

In your program, use the `ziskos::read_input()` function to retrieve the input data from the `input.bin` file:

```rust
// Read the input data as a byte array from ziskos
let input: Vec<u8> = read_input();
```

To write public output data, use the `ziskos::set_output()` function. Since the function accepts u32 values, split the output data into 32-bit chunks if necessary and increase the id parameter of the function in each call:

```rust
// Split 'hash' value into chunks of 32 bits and write them to ziskos output
for i in 0..8 {
    let val = byteorder::BigEndian::read_u32(&mut hash[i * 4..i * 4 + 4]);
    set_output(i, val);
}
```

---

## Input Processing

All program inputs flow through a single file called `input.bin`. Unlike traditional programs that can read files, prompt users, or make network requests at runtime, ZisK funnels everything through one file to ensure deterministic, consistent input.

**Input Flow:** build.rs (unrestricted environment) → input.bin (deterministic snapshot) → ZisK Memory (0x9000_0000) → read_input() (program logic)

### Preparing Input (build.rs)

Input preparation occurs outside ZisK in your `build.rs` script. You can fetch APIs, query databases, or process user input to build your input snapshot, then write the data to `input.bin`.

```rust
// build.rs - preparing input.bin
let secret_value: u64 = 42;
file.write_all(&secret_value.to_le_bytes())?;
```

This step is unrestricted since proof generation only requires the deterministic snapshot stored in `input.bin`.

### Reading Input (main.rs)

Inside ZisK, programs load the contents of `input.bin` using `read_input()`.

```rust
// main.rs - reading from input.bin
let input: Vec<u8> = read_input();
let secret_value = u64::from_le_bytes(input[0..8].try_into().unwrap());
```

Programs have access to the exact same data prepared in `build.rs`, ensuring deterministic execution.

---

## Privacy by Default

All input is private by default. ZisK doesn't distinguish between public and private during input. The distinction occurs when you explicitly call `set_output()`. Unless you choose to reveal a value, everything you process remains private.

## Making Data Public

Everything inside a ZisK program is private: inputs, intermediate values, and computations never leave the zkVM. To prove or share results, you need a way to selectively reveal information. `set_output()` is the only pathway to make data public.

The function takes two parameters:

- An index (0–63, `usize` type) that identifies the output slot
- A value (32-bit integer) to publish

ZisK maintains a small output memory region. The first 4 bytes store the output count, and each subsequent 4-byte slot holds one value. This structure enables easy storage, retrieval, and verification of public outputs during proof generation.

When you call `set_output()`:

1. You see immediate feedback in the console (e.g. `public 0: 420`) during development
2. The value is stored in the output memory region, ready for inclusion in the proof and `publics.json`

```rust
let result = secret_value * 10; // This stays private
set_output(0, result as u32);  // This becomes public
```

The index allows structured organization of multiple outputs:

```rust
set_output(0, timestamp as u32); // When computation happened
set_output(1, result as u32);    // The actual result
set_output(2, status_flag as u32); // Success/failure indicator
```

Only values explicitly passed to `set_output()` become public. Everything else remains private.

---

## Working with Dynamic Data Streams

ZisK's I/O model works with static data: prepare `input.bin`, read it with `read_input()`, and reveal results with `set_output()`. Real-world systems often rely on continuous inputs like market feeds, sensor readings, or on-chain events.

One approach treats live data as a series of snapshots staged before execution. This keeps program execution deterministic while changing how often and where snapshots are prepared.

### Patterns for Bridging Static and Dynamic Data

- **Build-Time Fetching**: Use Cargo's `build.rs` to pull fresh data before compilation. Each build cycle produces a new snapshot while program logic remains unchanged.
- **External Staging**: Let an external process maintain rolling snapshot files (e.g. "latest block state") that ZisK consumes at proof time.
- **Batching**: Accumulate data in fixed intervals (e.g. every 10 minutes) and snapshot each batch for deterministic processing.
- **Proxy Input Services**: Run background jobs or services that continuously write snapshots. ZisK reads the prepared file.
- **Event-Driven Builds**: Watch data sources and trigger re-compilation when they change, so proofs always reflect current state.

Dynamic data doesn't break ZisK's model. You move the variability into the snapshot preparation layer. Once the snapshot is in place, execution inside ZisK remains deterministic, private by default, and verifiable end-to-end.

---

## Working with Complex Data

ZisK's `set_output()` function only works with `u32` values. This design choice makes the system both flexible and verifiable by reducing everything to a fixed, uniform unit.

Larger values are broken down into 32-bit pieces, with each piece written to a different output slot.

For example, revealing a 256-bit hash requires splitting it into 8 chunks of 32 bits each and calling `set_output()` eight times with consecutive indices:

```rust
// Publishing a 256-bit hash as 8 x 32-bit chunks
for i in 0..8 {
    let val = byteorder::BigEndian::read_u32(&mut hash[i * 4..i * 4 + 4]);
    set_output(i, val);
}
```

This chunking pattern works for any complex structure. Inputs follow the same philosophy: in `build.rs`, you can encode arbitrarily complex data into `input.bin`, and in `main.rs` you parse it back into the structures your program needs. The simplicity of fixed-width outputs provides the foundation for handling complexity in a deterministic, verifiable way.

---

## Key Takeaways

- **Single source of truth**: All inputs are staged in `input.bin` (Static input file).
- **Privacy by default**: Nothing becomes public unless revealed with `set_output()`.
- **Structured outputs**: Up to 64 `u32` values can be exposed (indices 0-63), keeping things deterministic.
- **Deterministic execution**: Same inputs, same outputs, every time.
- **Developer control**: You decide exactly what's revealed to verifiers.
