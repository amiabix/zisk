# I/O Model

ZisK enforces privacy through a controlled input/output model. This document explains how programs handle inputs and outputs.

## Table of Contents

- [Basic I/O Examples](#basic-io-examples)
- [Input Processing](#input-processing)
- [Privacy by Default](#privacy-by-default)
- [Making Data Public](#making-data-public)
- [Working with Complex Data](#working-with-complex-data)
- [Working with Dynamic Data Streams](#working-with-dynamic-data-streams)

---

## Basic I/O Examples

ZisK programs read input from a binary file prepared before execution.

If your program requires complex input data, consider using a serialization mechanism (like [`bincode`](https://crates.io/crates/bincode) crate) to serialize it to the binary file.

In your program, use the `ziskos::read_input()` function to retrieve the input data:

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

All program inputs flow through a single binary input file. Unlike traditional programs that can read files, prompt users, or make network requests at runtime, ZisK funnels everything through one file to ensure deterministic, consistent input.

**Input Flow:** build.rs (data preparation) → binary file (deterministic snapshot) → read_input() (program execution)

### Preparing Input (build.rs)

Input preparation occurs outside ZisK in your `build.rs` script. You can fetch APIs, query databases, or process user input to build your input snapshot, then write the data to your input file.

```rust
// build.rs - preparing input data
let secret_value: u64 = 42;
file.write_all(&secret_value.to_le_bytes())?;
```

This step is unrestricted since proof generation only requires the deterministic input snapshot.

### Reading Input (main.rs)

Inside ZisK, programs load input data using `read_input()`.

```rust
// main.rs - reading input data
let input: Vec<u8> = read_input();
let secret_value = u64::from_le_bytes(input[0..8].try_into().unwrap());
```

Programs receive the exact data prepared during the build step, ensuring deterministic execution.

---

## Privacy by Default

All input is private by default. ZisK doesn't distinguish between public and private during input. The distinction occurs when you explicitly call `set_output()`. Unless you choose to reveal a value, everything you process remains private.

## Making Data Public

To prove or share results, you need a way to selectively reveal information. `set_output()` is the only pathway to make data public.

The function takes two parameters:

- An index (0–63, `usize` type) that identifies the output slot
- A value (32-bit integer) to publish

ZisK maintains a small output memory region. The first 4 bytes store the output count, and each subsequent 4-byte slot holds one value. This structure enables easy storage, retrieval, and verification of public outputs during proof generation.

When you call `set_output()`, the value is stored in the output memory region, ready for inclusion in the proof and `publics.json`.

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

This chunking pattern works for any complex structure. Inputs follow the same philosophy: during preparation, you can encode arbitrarily complex data into the input file, and in your program you parse it back into the structures your program needs. The simplicity of fixed-width outputs provides the foundation for handling complexity in a deterministic, verifiable way.

---

## Working with Dynamic Data Streams

ZisK requires static input for deterministic execution. For systems with continuous data like market feeds or sensor readings, prepare a snapshot before proof generation. The snapshot can be created in your build script, by an external process, or triggered when source data changes. Program execution remains deterministic while the preparation layer handles variability.
