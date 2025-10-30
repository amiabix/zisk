# Setup

ZisK programs are written in standard Rust with two modifications, marking the entry point and adding the ziskos dependency. 

This guide walks you through those changes using the SHA-256 example.

## Table of Contents

- [Required Changes](#required-changes)
- [Complete Example](#complete-example)
- [Next Steps](#next-steps)

---

## Required Changes

Writing a Rust program for ZisK requires two modifications:

### 1. Update main.rs

Add these lines to mark the main function as the entry point for ZisK:

```rust
#![no_main]
ziskos::entrypoint!(main);

fn main() {
    // Your program logic here
}
```

### 2. Update Cargo.toml

Add the `ziskos` dependency:

```toml
[dependencies]
ziskos = { git = "https://github.com/0xPolygonHermez/zisk.git" }
```

You can also specify a particular branch or tag:

```toml
ziskos = { git = "https://github.com/0xPolygonHermez/zisk.git", branch = "main" }
```

##  Example

 Example program takes a number `n` as input and computes SHA-256 iteratively `n` times.

**main.rs:**

```rust
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

**Cargo.toml:**

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

## Next Steps

- **[I/O Model](./io.md)** — Learn how input/output and privacy works with ZisK
- **[Build and Prove](../getting_started/build-and-prove.md)** — Complete development guide for building and proving programs
