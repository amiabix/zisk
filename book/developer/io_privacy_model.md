# I/O Model

When people talk about privacy in zero-knowledge systems, it often sounds abstract. In ZisK, one of the clearest places to see how privacy is enforced is in the I/O model. The way programs handle inputs and outputs. To understand it, we'll look at three things:

- How ZisK's input/output model works
- How dynamic data streams are handled

## Input Data Handling

To provide input data for ZisK, you need to write that data in a binary file (e.g., `input.bin`).

If your program requires complex input data, consider using a serialization mechanism (like [`bincode`](https://crates.io/crates/bincode) crate) to store it in the `input.bin` file.

In your program, use the `ziskos::read_input()` function to retrieve the input data from the `input.bin` file:

```rust
// Read the input data as a byte array from ziskos
let input: Vec<u8> = read_input();
```

## Output Data Handling

To write public output data, use the `ziskos::set_output()` function. Since the function accepts u32 values, split the output data into 32-bit chunks if necessary and increase the id parameter of the function in each call:

```rust
// Split 'hash' value into chunks of 32 bits and write them to ziskos output
for i in 0..8 {
    let val = byteorder::BigEndian::read_u32(&mut hash[i * 4..i * 4 + 4]);
    set_output(i, val);
}
```
- How the system decides what becomes public and what stays private

## The zkVM I/O Challenge

Zero-knowledge virtual machines face a problem that ordinary programs don't: they have to run in a way that is both deterministic and verifiable, while still keeping some data hidden.

Think about running a regular program you can read files, ask the user for input, or call the API. This works differently with zkVMs. Why?

- A verifier needs to be able to check the computation without rerunning it.
- The same inputs must always lead to the same outputs.
- Some data must stay private, while other data needs to be public.

If you allowed free-form I/O at runtime, you'd break determinism and leak sensitive information.

## How ZisK Solves It

ZisK tackles this with a controlled I/O model that cleanly separates data preparation from program execution. Instead of letting the program talk directly to files or networks, all inputs are prepared up front in a static file (e.g. `input.bin`). During execution, the program only sees this prepared input and writes its outputs in a structured way.

At the code level, this shows up in the `ziskos` crate:

- `read_input()`: To load prepared input values
- `set_output(id, value)`: To write outputs in a way that can later be made public or kept private

This design guarantees that every run of the program is deterministic i.e. the same inputs always lead to the same outputs, and the verifier knows exactly which outputs should be visible.

Meanwhile, the developer has full control over what data is exposed and what remains hidden.

## Input Handling

In ZisK, all program inputs flow through a single file called `input.bin`. This is very different from traditional programs, which can read files, prompt users, or make network requests at runtime. By funneling everything through one file, ZisK ensures input is deterministic, consistent, and fully under your control.

### Preparing Input (build.rs)

The preparation step happens outside the zkVM in your `build.rs` script. Here, you can fetch APIs, query databases, or process user input anything you need to build your input snapshot. Once you're ready, you write the data into `input.bin`.

**SHA Example from the documentation:**

```rust
// build.rs [ preparing input.bin ]
let secret_value: u64 = 42;
file.write_all(&secret_value.to_le_bytes())?;
```

This step is unrestricted, you can use external resources freely, since proof generation only cares about the deterministic snapshot stored in `input.bin`.

### Reading Input (main.rs)

Inside the zkVM, your program doesn't reach out to files or networks. Instead, it loads the contents of `input.bin` using `read_input()`.

```rust
// main.rs – reading from input.bin
let input: Vec<u8> = read_input();
let secret_value = u64::from_le_bytes(input[0..8].try_into().unwrap());
```

At this point, your program has access to the exact same data you prepared in `build.rs`. Execution is guaranteed to be deterministic, since every run sees the same snapshot.

## Privacy by Default

Here's the key insight: All input is private by default. ZisK doesn't distinguish between public and private during input. That distinction happens later, when you explicitly call `set_output()`. Unless you choose to reveal a value, everything you process remains private. We'll dive into it as we go ahead.

## Making Data Public

So, everything inside a ZisK program is private inputs, intermediate values, and computations never leave the zkVM. But when you want to prove something or share a result, you need a way to selectively reveal information. That's where `set_output()` comes in it is the only pathway to make data public.

It takes two parameters:

- an index (from 0–63, `usize` type), which identifies the output slot, and
- a value (a 32-bit integer) to publish.

Behind the scenes, ZisK keeps a small output memory region. The first 4 bytes store the count of outputs, and each subsequent 4-byte slot holds one value. This structure makes it easy to store, retrieve, and verify public outputs during proof generation.

So, when you call `set_output()`, two things happen:

1. You see immediate feedback in the console (e.g. `public 0: 420`) while developing.
2. The value is stored in the output memory region, ready to be included in the proof and later in `publics.json`.

For example:

```rust
let result = secret_value * 10; // This stays private
set_output(0, result as u32);  // This becomes public
```

The index allows you to organize multiple outputs in a structured way:

```rust
set_output(0, timestamp as u32); // When computation happened
set_output(1, result as u32);    // The actual result
set_output(2, status_flag as u32); // Success/failure indicator
```

The important point is control only values you explicitly pass to `set_output()` become public. Everything else even if it was part of your computation remains private.

## Working with Dynamic Data Streams in ZisK

So far, we've looked at ZisK's I/O model in its simplest form: prepare an `input.bin`, read it deterministically with `read_input()`, and reveal results selectively with `set_output()`. This works perfectly when your data is static but real-world systems often rely on continuous inputs like market feeds, sensor readings, or any on-chain events.

One of the way is to treat live data as a series of snapshots that get staged before execution. In other words, you keep the program's execution deterministic, but change how often and where the snapshots are prepared.

### Patterns for Bridging Static and Dynamic Data

- **Build-Time Fetching**: Use Cargo's `build.rs` to pull in fresh data before compilation. Each build cycle produces a new snapshot, while the program logic stays unchanged.
- **External Staging**: Let an external process maintain rolling snapshot files (e.g. "latest block state"), which ZisK consumes at proof time.
- **Batching**: Accumulate data in fixed intervals (e.g. every 10 minutes) and snapshot each batch for deterministic processing.
- **Proxy Input Services**: Run background jobs or services that continuously write snapshots. ZisK then just reads the prepared file.
- **Event-Driven Builds**: Watch data sources and trigger re-compilation when they change, so proofs always reflect current state.

The key insight is that dynamic data doesn't break ZisK's model, you just move the variability into the snapshot preparation layer. Once the snapshot is in place, execution inside ZisK stays deterministic, private by default, and verifiable end-to-end.

## Working with Complex Data

One of the design choices in ZisK is that `set_output()` keeps things simple it only works with `u32` values. At first glance that might look limiting but in practice, it's what makes the system both flexible and verifiable. By reducing everything to a fixed, uniform unit, ZisK gives you a straightforward way to represent even the most complex outputs.

The answer is simple: split and chunk. Larger values are broken down into 32-bit pieces, and each piece is written to a different output slot.

For example, revealing a 256-bit hash means splitting it into 8 chunks of 32 bits each and calling `set_output()` eight times with consecutive indices.

```rust
// Publishing a 256-bit hash as 8 x 32-bit chunks
for i in 0..8 {
    let val = byteorder::BigEndian::read_u32(&mut hash[i * 4..i * 4 + 4]);
    set_output(i, val);
}
```

This chunking pattern works not just for hashes, but for any complex structure. Inputs follow the same philosophy: in `build.rs`, you can encode arbitrarily complex data into `input.bin`, and in `main.rs` you parse it back into the structures your program needs. The simplicity of fixed-width outputs becomes the foundation for handling complexity in a deterministic, verifiable way.

## The Privacy Model in Practice

By now, we've seen how ZisK programs handle inputs through `input.bin`, how outputs are revealed with `set_output()`, how complex data gets chunked into `u32`s, and even how dynamic streams can be staged as snapshots. All of these pieces fit together into ZisK's simple but powerful privacy model: everything stays private unless you choose to reveal it.

That means all inputs, intermediate calculations, memory operations, and internal state are hidden from verifiers. The only things that ever leave the VM are the values you explicitly publish with `set_output()`, along with the ROM root hash and input data hash, which are automatically included in the proof to bind execution to the correct program and inputs. More on that in the next part.

This gives you the developers precise control: verifiers see only what you want them to see, while the rest of the computation remains sealed inside the proof. This selective disclosure is what makes ZisK both practical and privacy-preserving you can prove correctness without leaking sensitive data.

## Key Takeaways

- **Single source of truth**: All inputs are staged in `input.bin` (Static input file).
- **Privacy by default**: Nothing becomes public unless revealed with `set_output()`.
- **Structured outputs**: Up to 64 `u32` values can be exposed (indices 0-63), keeping things deterministic.
- **Deterministic execution**: Same inputs, same outputs, every time.
- **Developer control**: You decide exactly what's revealed to verifiers.
