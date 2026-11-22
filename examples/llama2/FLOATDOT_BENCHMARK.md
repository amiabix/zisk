# FloatDot proving-time benchmark: stories260K in FP32

This benchmark proves the same TinyStories transformer twice, with and without the FloatDot
precompile, and compares the actual whole-VM proof generation time.

## Workload

- Model: Karpathy's `stories260K.bin` (sha256 `b0a507e7…2696`). These are its **original FP32 weights**,
  read from the llama2.c checkpoint with no int8 quantization. It has 5 layers, dim 64, 8 query heads
  and 4 KV heads, hidden 172, and a vocabulary of 512.
- Request: prompt "Once upon a" = tokens `[1, 403, 407, 261]` (BOS + 3), greedy, `max_new` 2.
  It generates `[378, 432]` (" time,"). That is 5 forward passes (positions 0–4), so attention sees
  up to 5 positions.
- Code: [`common/src/forward_f32.rs`](common/src/forward_f32.rs), a port of llama2.c's `run.c`. The guest
  selects it with option bit 4 (`F32_MODEL`), and bit 2 (`USE_FLOATDOT`) switches every dot product to
  FloatDot ([`guest/src/main.rs`](guest/src/main.rs)).

### What FloatDot replaces

Every dot product of the forward pass goes through one `WordDotFn`:

- all 8 learned matrices: `wq`, `wk`, `wv`, `wo`, `w1`, `w2`, `w3` and the classifier (shared with the embedding);
- the RMSNorm sums of squares, the attention scores `q·k`, and the attention outputs `Σ_t a_t v_t[i]`.

The two arms differ **only** in that function: `dot_words_soft` (plain `acc += a * b`) or
`dot_words_floatdot` (`syscall_fp32_dot`). Both arms get the same operands, packed two `f32` per
`u64` word, low half first:

- the weights are packed once when loading. The checkpoint's 28-byte header leaves them only 4-byte
  aligned, so both arms pay this copy;
- every input vector is packed once per matrix multiplication or RMSNorm;
- the keys are packed into the cache, and each value column is gathered into words.

Everything else is identical in both arms: RoPE, softmax, SiLU, `expf`/`sinf`/`cosf`/`powf` from `libm`,
and the residual adds.

Arithmetic: FP32 round-to-nearest-even, each product then each sum, in index order, starting from +0.0.
There is no FMA and no reassociation. FloatDot computes exactly this (`ziskos/entrypoint/src/syscalls/floatdot.rs`).
NaN payload parity is not claimed. None occur in this run.

### Bitwise equality, checked before proving

The guest commits `sha256(params ‖ tokens)` followed by `sha256(trace)`. The trace holds the bits of
every dot-product result, which covers every matrix-multiplication output, RMSNorm sum, attention score
and output, and every logit: 19,815 values.

| Check | Result |
|---|---|
| `cargo test -p llama2-common --features floatdot -- --include-ignored` | 12 passed |
| `tests/forward_f32.rs` | both arms equal an independent `run.c` port (plain slices, no packing, RoPE recomputed per layer) bit for bit, on random checkpoints and on `stories260K` |
| native soft = guest soft = guest FloatDot (`llama2-zisk --weights f32 --trace on`, local and Omarchy ziskemu) | tokens and trace digest equal |
| Mutation: `score * (1/√hs)` instead of `score / √hs` | caught by the `stories260K` test (trace value 4014). Not caught by the random-checkpoint test. |

Expected public output (256 bytes, sha256 `b6d011a8…2194`), identical for both arms.

### Fixture (frozen before any proof)

| File | sha256 |
|---|---|
| guest ELF (same ELF for both arms) | `7bd884797f9f4bd0369f805d0e7e14d844088b71fd3cd4176083f3d21ad3fc05` |
| `floatdot.stdin` (flags 28) | `3bdf706271e0e442f5c9004def562c3f5bf9177e56e7632c820e86b42bc25e5e` |
| `loops.stdin` (flags 24) | `35da19439e64d452a49efaebd771a62da1b3fb08bf6c8fec700f8bcd4196a353` |
| `expected-output.bin` | `b6d011a8f14782c70d2112ddacda5968ea781bc9f91abcd7e74933fabc422194` |

The two inputs differ in one byte, the option flags. Record 1 of both is the raw checkpoint (1,056,540 bytes).

### Emulator workload

| | Software FP32 | FloatDot FP32 |
|---|---|---|
| Guest steps | 165,876,159 | 14,519,746 |
| FloatDot calls / multiply-adds / AIR rows (host count) | 0 | 19,815 / 1,309,760 / 2,619,520 |
| Planned instances (`verify-constraints`) | 1,244 (Main 633, FloatDot 0) | 293 (Main 56, **FloatDot 161**) |

## Proof backend

- Experimental FloatDot whole-VM backend. Its source is a snapshot (`/home/abix/fp32-floatdot-20261115/source`,
  base `17138bfd`, not this branch). The bounded setup has Main at 2^16 rows and an 83 GB key. It is not
  compatible with release keys. The guest-side SDK in this branch matches it except for the unused `dot_i8` syscall.
- Settings for both arms: `prove --random-seed 0 --minimal-memory --number-threads-witness 8 --verify-proofs`,
  with `RAYON_NUM_THREADS=8`, and the Rust emulator (no assembly, no hints).
- Program key: derived from the ELF by `verify-constraints` in a fresh cache for each arm,
  `[13546498992430110765, 13221816415118903093, 1272479947493195189, 3604343059421563770]`.
- Leaf key: `vadcop_final.verkey.json` sha256 `41897ff8…f046`.
- Reload check: `cargo-zisk-dev verify --leaf-setup-vk`, then the backend's `floatdot_roundtrip` test. That
  test reloads the saved proof and checks the program key and all 64 public words. It also rejects a changed
  public output, a wrong program key and a damaged payload.

Harness: [`benchmark/floatdot.py`](benchmark/floatdot.py).

## Results

None yet: no proof of either arm has completed, so there is no proof timing (see the runs below).

## Runs that do not count

- **Omarchy CPU** (i9-14900HX, 31 GiB RAM): the FP32 FloatDot proof ran for 1 h 17 min while swapping 10–14 GB,
  overlapped the GPU build, and was then paused (SIGSTOP) on request. It has no timing.
- **Omarchy GPU** (RTX 4080 Laptop, 12 GB): a GPU build of the same backend started, but `prove --gpu` stopped
  after 28 s. The largest single proof, the FloatDot compressor (2^22 extended rows × 500 columns), needs
  19.12 GB of device memory, and only 11.45 GB is free.
- **int8 model** (first version of this benchmark, before the FP32 switch): there, FloatDot covers only
  attention and RMSNorm, saving 2.5% of the steps (40.60 M → 39.60 M). Its software proof was cancelled on request,
  so it has no proof timing.

## Limitations

- The fixture's public output binds the request, the tokens and the trace digest, not the model: its guest
  ELF predates the `sha256(checkpoint)` commit that the guest code now adds with `F32_MODEL`. This benchmark
  measures performance; it is not proof-carrying inference.
- One machine, one run per arm. Times depend on that machine's memory and swap.
- stories260K is tiny. These numbers say nothing about larger models such as Qwen.
