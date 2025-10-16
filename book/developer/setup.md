# Setup

This guide walks you through setting up a ZisK program for distributed proof generation. You'll learn how to write a Rust program that can be executed by the ZisK distributed system.

## Code Changes

Writing a Rust program for ZisK is similar to writing a standard Rust program, with a few minor modifications. Follow these steps:

### Modify main.rs file

Add the following code to mark the main function as the entry point for ZisK:

```rust
#![no_main]
ziskos::entrypoint!(main);
```

### Modify Cargo.toml file

Add the ziskos crate as a dependency:

```toml
[dependencies]
ziskos = { git = "https://github.com/0xPolygonHermez/zisk.git" }
```

## Example Program

Let's show these changes using the example program from the Quickstart section.

### main.rs

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

### Cargo.toml

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

Once you have your ZisK program set up:

1. **Build your program** using `cargo-zisk build --release`
2. **Prepare input data** - See [I/O Model](./io_privacy_model.md) for details on input/output handling
3. **Set up the distributed system** - Follow the [Local Setup](../distributed/local_development.md) guide
4. **Deploy to production** - Use [Docker Deployment](../distributed/docker_deployment.md) for production environments

## See Also

- [Writing Programs](../getting_started/writing_programs.md) for general ZisK program development
- [I/O Model](./io_privacy_model.md) for input/output handling details
- [Local Setup](../distributed/local_development.md) for running the distributed system
- [Docker Deployment](../distributed/docker_deployment.md) for production deployment
- [Distributed README](../../distributed/README.md) for operator configuration
