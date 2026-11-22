//! `f32_dot_floatdot` must give the plain loops' results bit for bit.
//!
//! On the host, `syscall_fp32_dot` runs the precompile's reference arithmetic
//! (`zisk_precomp_helpers::fp32_dot`), which the emulator and the prover use too.
//!
//! `cargo test -p llama2-common --features floatdot`
#![cfg(feature = "floatdot")]

use llama2_common::checkpoint::quantize_checkpoint;
use llama2_common::forward::{MatmulMode, f32_dot_floatdot};
use llama2_common::generate::generate_traced;
use llama2_common::model::Model;
use llama2_common::params::GenParams;

fn loop_dot(x: &[f32], y: &[f32]) -> f32 {
    let mut acc = 0.0f32;
    for (&a, &b) in x.iter().zip(y) {
        acc += a * b;
    }
    acc
}

/// Deterministic values in [-1, 1).
fn values(seed: u64, n: usize) -> Vec<f32> {
    let mut s = seed;
    (0..n)
        .map(|_| {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (s >> 40) as f32 / (1u64 << 23) as f32 - 1.0
        })
        .collect()
}

#[test]
fn matches_the_loop_across_lengths_and_chunks() {
    // Odd lengths, and lengths around the 512-value chunks.
    for n in [1, 2, 7, 8, 64, 511, 512, 513, 1024, 1500] {
        let mut x = values(n as u64, n);
        let y = values(n as u64 + 99, n);
        // Large and tiny magnitudes, so the order and rounding of every step matter.
        for (i, v) in x.iter_mut().enumerate() {
            *v *= [1e30, 1.0, 1e-30, -3.0e-39][i % 4];
        }
        assert_eq!(f32_dot_floatdot(&x, &y).to_bits(), loop_dot(&x, &y).to_bits(), "n = {n}");
    }
    // A sum that rounds to -0.0 in the loop starts from +0.0: 0 + (-0) = +0.
    assert_eq!(f32_dot_floatdot(&[-0.0], &[1.0]).to_bits(), loop_dot(&[-0.0], &[1.0]).to_bits());
}

/// A random llama2.c checkpoint: dim 16, hidden 24, 2 layers, 4 heads with 2
/// KV heads, vocabulary 32, context 16.
fn checkpoint() -> Vec<u8> {
    let (dim, hidden, layers, kv_dim, vocab, seq) = (16, 24, 2, 8, 32, 16);
    let mut b = Vec::new();
    for v in [dim, hidden, layers, 4, 2, vocab, seq] {
        b.extend_from_slice(&(v as i32).to_le_bytes());
    }
    let floats = vocab * dim
        + layers * (2 * dim + 2 * dim * dim + 2 * kv_dim * dim + 3 * hidden * dim)
        + dim
        + seq * dim / 4;
    for v in values(7, floats) {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b
}

#[test]
fn generation_traces_match_the_loops() {
    let blob = quantize_checkpoint(&checkpoint()).unwrap();
    let model = Model::parse(&blob);
    let p = GenParams { prompt: vec![1, 5, 9], max_new: 6, temperature: 0.0, seed: 42 };
    let mode = |f32_dot| MatmulMode { f32_dot, ..MatmulMode::default() };
    let (tokens, trace) = generate_traced(&model, &p, mode(None), true, |_| {});
    let (fd_tokens, fd_trace) =
        generate_traced(&model, &p, mode(Some(f32_dot_floatdot)), true, |_| {});

    // Per position: 2 RMSNorms per layer and the final one, the scores of
    // every head over pos + 1 positions, the attention outputs, the logits.
    let positions = p.prompt.len() - 1 + p.max_new as usize;
    let per_layer = |pos: usize| 2 + 4 * (pos + 1) + 16;
    let expected: usize = (0..positions).map(|pos| 2 * per_layer(pos) + 1 + 32).sum();
    assert_eq!(tokens.len(), p.max_new as usize);
    assert_eq!(trace.len(), expected);
    assert_eq!(fd_tokens, tokens);
    assert_eq!(fd_trace, trace);

    // The trace does see a change of summation order.
    fn reversed(x: &[f32], y: &[f32]) -> f32 {
        x.iter().zip(y).rev().fold(0.0, |acc, (&a, &b)| acc + a * b)
    }
    let (_, rev_trace) = generate_traced(&model, &p, mode(Some(reversed)), true, |_| {});
    assert_ne!(rev_trace, trace);
}
