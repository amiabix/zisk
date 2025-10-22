# Setup

This guide shows you the minimal changes needed to make any Rust program compatible with ZisK's virtual machine. You'll learn how to modify your existing Rust code to run in ZisK's execution environment and generate proofs of your computations.

## Table of Contents

- [Required Changes](#required-changes)
- [Complete Example](#complete-example)
- [Next Steps](#next-steps)

---

## Required Changes

Converting a standard Rust program to work with ZisK requires only two simple modifications:

### 1. Update main.rs

Add these two lines to the top of your `main.rs`:

```rust
#![no_main]
ziskos::entrypoint!(main);

fn main() {
    // Your existing program logic here
}
```

**What these changes do:**
- `#![no_main]` - Tells Rust not to use the standard main function entry point
- `ziskos::entrypoint!(main)` - Registers your main function as the ZisK program entry point

### 2. Update Cargo.toml

Add the `ziskos` dependency:

```toml
[dependencies]
ziskos = { git = "https://github.com/0xPolygonHermez/zisk.git" }
```

**Note:** You can also specify a particular branch or tag if needed:
```toml
ziskos = { git = "https://github.com/0xPolygonHermez/zisk.git", branch = "main" }
```

---

## Complete Example

Here's a complete example that computes a SHA-256 hash:

```rust
#![no_main]
ziskos::entrypoint!(main);

use sha2::{Sha256, Digest};

fn main() {
    // Read input data from the input.bin file
    let input = ziskos::read_input();
    
    // Compute SHA-256 hash
    let mut hasher = Sha256::new();
    hasher.update(&input);
    let result = hasher.finalize();
    
    // Output the hash (first 32 bits) as public data
    ziskos::set_output(0, u32::from_be_bytes([result[0], result[1], result[2], result[3]]));
}
```

**What this example does:**
1. Reads private input data from `input.bin`
2. Computes a SHA-256 hash of the input
3. Publishes the first 32 bits of the hash as public output
4. Keeps the original input and full hash private

---

## Next Steps

- **[I/O Model](./io.md)** — Learn how I/O and privacy works with ZisK
- **[Build and Prove](../getting_started/build-and-prove.md)** — Complete development guide to help you build and prove your programs with ZisK
