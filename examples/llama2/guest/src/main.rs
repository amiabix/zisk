//! llama2 guest: generates text with a TinyStories model inside the zkVM.
//!
//! Reads three raw slices from its standard input:
//! 1. the model blob (layout in `llama2_common::checkpoint`), used in place,
//!    or with [`F32_MODEL`] the llama2.c checkpoint itself;
//! 2. the generation parameters (`llama2_common::params::GenParams`);
//! 3. guest options, one `u64` of flags: bit 0 ([`USE_DOT_I8`]) computes the
//!    dot products of the matrix multiplications with the experimental
//!    `dot_i8` precompile, which ZisK's emulator runs but cannot prove yet;
//!    bit 1 ([`FIXED_SCALES`]) applies the group scales in fixed point
//!    instead of `f32`; bit 2 ([`USE_FLOATDOT`]) computes the `f32` dot
//!    products outside the matrix multiplications with the FloatDot
//!    precompile; bit 3 ([`TRACE`]) also commits the forward passes' trace;
//!    bit 4 ([`F32_MODEL`]) runs the checkpoint's `f32` weights as llama2.c's
//!    `run.c` (`llama2_common::forward_f32`), where bit 2 makes every dot
//!    product, matrix multiplications included, use FloatDot. Bits 0 and 1
//!    must then be clear.
//!
//! Prints the generated token ids on one line, `tokens: <id> <id> ...`, for
//! the host to decode, and commits `sha256(params || generated tokens)` as
//! the public output, so the printed tokens can be checked against it. The
//! public output is limited to 64 words, too small for the tokens themselves.
//! With [`TRACE`], `sha256(trace)` follows it: the bits of every `f32` dot
//! product result FloatDot can replace and of every logit
//! (`llama2_common::forward::Trace`), and with [`F32_MODEL`] of every matrix
//! multiplication output too. With [`F32_MODEL`], `sha256(checkpoint)` comes
//! last: the hash of exactly the bytes `ModelF32::parse` reads, so the proof
//! binds the model.

#![no_main]

ziskos::entrypoint!(main);

use core::fmt::Write;

use llama2_common::forward::{DotFn, MatmulMode, Scales, dot_soft, f32_dot_floatdot};
use llama2_common::forward_f32::{ModelF32, dot_words_floatdot, dot_words_soft, generate_f32};
use llama2_common::generate::generate_traced;
use llama2_common::model::Model;
use llama2_common::params::GenParams;
use sha2::{Digest, Sha256};

use ziskos::syscalls::{SyscallDotI8Params, syscall_dot_i8};

/// Guest option: use the `dot_i8` precompile.
const USE_DOT_I8: u64 = 1;

/// Guest option: fixed-point scales in the matrix multiplications.
const FIXED_SCALES: u64 = 2;

/// Guest option: the FloatDot precompile for the `f32` dot products in
/// RMSNorm and attention.
const USE_FLOATDOT: u64 = 4;

/// Guest option: also commit the digest of the forward passes' trace.
const TRACE: u64 = 8;

/// Guest option: the `f32` model.
const F32_MODEL: u64 = 16;

/// The dot product with the `dot_i8` precompile, which needs 8-byte aligned
/// vectors and a length that is a multiple of 8. Groups of other sizes (4 in
/// `stories260K`'s `w2`) use the plain-Rust loop.
fn dot_precompile(x: &[i8], y: &[i8]) -> i32 {
    if x.len() % 8 != 0 {
        return dot_soft(x, y);
    }
    let params = SyscallDotI8Params { x: x.as_ptr(), y: y.as_ptr(), n: x.len() as u64 };
    // SAFETY: both slices hold `n` bytes; the emulator checks the alignment.
    unsafe { syscall_dot_i8(&params) }
}

fn main() {
    let blob = ziskos::io::read_slice();
    let params_bytes = ziskos::io::read_slice();
    let options = ziskos::io::read_slice();
    let flags = u64::from_le_bytes(options[..8].try_into().expect("guest options: 8 bytes"));

    let params = GenParams::from_bytes(&params_bytes);
    let trace = flags & TRACE != 0;

    let (tokens, trace) = if flags & F32_MODEL != 0 {
        assert_eq!(flags & (USE_DOT_I8 | FIXED_SCALES), 0, "int8 options with the f32 model");
        let model = ModelF32::parse(&blob);
        let dot = if flags & USE_FLOATDOT != 0 { dot_words_floatdot } else { dot_words_soft };
        generate_f32(&model, &params, dot, trace, |_| {})
    } else {
        let model = Model::parse(&blob);
        let dot: DotFn = if flags & USE_DOT_I8 != 0 { dot_precompile } else { dot_soft };
        let scales = if flags & FIXED_SCALES != 0 { Scales::Fixed } else { Scales::Float };
        let f32_dot = (flags & USE_FLOATDOT != 0).then_some(f32_dot_floatdot as _);
        let mode = MatmulMode { dot, scales, f32_dot };
        generate_traced(&model, &params, mode, trace, |_| {})
    };

    let mut line = String::from("tokens:");
    for t in &tokens {
        write!(line, " {t}").unwrap();
    }
    println!("{line}");

    let mut hasher = Sha256::new();
    hasher.update(&*params_bytes);
    for t in &tokens {
        hasher.update(t.to_le_bytes());
    }
    let mut output = hasher.finalize().to_vec();
    if flags & TRACE != 0 {
        let mut hasher = Sha256::new();
        for v in &trace {
            hasher.update(v.to_le_bytes());
        }
        output.extend_from_slice(&hasher.finalize());
    }
    if flags & F32_MODEL != 0 {
        output.extend_from_slice(&ziskos::zisklib::sha256(&blob));
    }
    ziskos::io::commit_slice(&output);
}
