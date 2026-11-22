//! `forward_f32` with either dot product must match an independent port of
//! llama2.c's `run.c` bit for bit: same tokens, same trace of every
//! matrix-multiplication output, RMSNorm sum of squares, attention score and
//! output, and logit.
//!
//! The port reads `f32` slices straight from the checkpoint bytes, uses plain
//! `+=` loops without packing, and recomputes RoPE per layer as `run.c` does.
//!
//! `cargo test -p llama2-common --features floatdot --test forward_f32`
//! (`-- --ignored` also runs the downloaded `stories260K` on the benchmark prompt.)
#![cfg(feature = "floatdot")]

use llama2_common::checkpoint::read_config;
use llama2_common::forward_f32::{
    ModelF32, WordDotFn, dot_words_floatdot, dot_words_soft, generate_f32,
};
use llama2_common::generate::argmax;
use llama2_common::params::GenParams;

fn matmul(out: &mut [f32], x: &[f32], w: &[f32], trace: &mut Vec<u32>) {
    let n = x.len();
    for (i, o) in out.iter_mut().enumerate() {
        let mut val = 0.0f32;
        for j in 0..n {
            val += w[i * n + j] * x[j];
        }
        *o = val;
    }
    trace.extend(out.iter().map(|v| v.to_bits()));
}

fn rmsnorm(out: &mut [f32], x: &[f32], w: &[f32], trace: &mut Vec<u32>) {
    let mut ss = 0.0f32;
    for &v in x {
        ss += v * v;
    }
    trace.push(ss.to_bits());
    ss /= x.len() as f32;
    ss += 1e-5;
    ss = 1.0 / libm::sqrtf(ss);
    for j in 0..x.len() {
        out[j] = w[j] * (ss * x[j]);
    }
}

/// Greedy generation with `run.c`'s forward pass; returns tokens and trace.
fn reference(bytes: &[u8], p: &GenParams) -> (Vec<u32>, Vec<u32>) {
    let (c, shared) = read_config(bytes).unwrap();
    let f: Vec<f32> =
        bytes[28..].chunks_exact(4).map(|b| f32::from_le_bytes(b.try_into().unwrap())).collect();
    let (dim, hidden, layers, kv_dim, hs) =
        (c.dim, c.hidden_dim, c.n_layers, c.kv_dim(), c.head_size());
    let mut at = 0;
    let mut take = |n: usize| {
        at += n;
        &f[at - n..at]
    };
    let emb = take(c.vocab_size * dim);
    let rms_att = take(layers * dim);
    let wq = take(layers * dim * dim);
    let wk = take(layers * kv_dim * dim);
    let wv = take(layers * kv_dim * dim);
    let wo = take(layers * dim * dim);
    let rms_ffn = take(layers * dim);
    let w1 = take(layers * hidden * dim);
    let w2 = take(layers * dim * hidden);
    let w3 = take(layers * hidden * dim);
    let rms_final = take(dim);
    take(c.seq_len * hs);
    let wcls = if shared { emb } else { take(c.vocab_size * dim) };

    let (mut x, mut xb, mut xb2) = (vec![0.0f32; dim], vec![0.0f32; dim], vec![0.0f32; dim]);
    let (mut hb, mut hb2) = (vec![0.0f32; hidden], vec![0.0f32; hidden]);
    let (mut q, mut k, mut v) = (vec![0.0f32; dim], vec![0.0f32; kv_dim], vec![0.0f32; kv_dim]);
    let mut att = vec![0.0f32; c.seq_len];
    let mut logits = vec![0.0f32; c.vocab_size];
    let mut kc = vec![0.0f32; layers * c.seq_len * kv_dim];
    let mut vc = vec![0.0f32; layers * c.seq_len * kv_dim];
    let (mut tr, mut out) = (Vec::new(), Vec::new());

    let mut token = p.prompt[0];
    for pos in 0..c.seq_len {
        x.copy_from_slice(&emb[token as usize * dim..][..dim]);
        for l in 0..layers {
            rmsnorm(&mut xb, &x, &rms_att[l * dim..][..dim], &mut tr);
            matmul(&mut q, &xb, &wq[l * dim * dim..][..dim * dim], &mut tr);
            matmul(&mut k, &xb, &wk[l * kv_dim * dim..][..kv_dim * dim], &mut tr);
            matmul(&mut v, &xb, &wv[l * kv_dim * dim..][..kv_dim * dim], &mut tr);
            for i in (0..dim).step_by(2) {
                let head_dim = i % hs;
                let freq = 1.0 / libm::powf(10000.0, head_dim as f32 / hs as f32);
                let val = pos as f32 * freq;
                let (fcr, fci) = (libm::cosf(val), libm::sinf(val));
                let rotn = if i < kv_dim { 2 } else { 1 };
                for r in 0..rotn {
                    let vec = if r == 0 { &mut q } else { &mut k };
                    let (v0, v1) = (vec[i], vec[i + 1]);
                    vec[i] = v0 * fcr - v1 * fci;
                    vec[i + 1] = v0 * fci + v1 * fcr;
                }
            }
            let loff = l * c.seq_len * kv_dim;
            kc[loff + pos * kv_dim..][..kv_dim].copy_from_slice(&k);
            vc[loff + pos * kv_dim..][..kv_dim].copy_from_slice(&v);
            for h in 0..c.n_heads {
                let qh = &q[h * hs..][..hs];
                let kv_off = (h / (c.n_heads / c.n_kv_heads)) * hs;
                for t in 0..=pos {
                    let kh = &kc[loff + t * kv_dim + kv_off..][..hs];
                    let mut score = 0.0f32;
                    for i in 0..hs {
                        score += qh[i] * kh[i];
                    }
                    tr.push(score.to_bits());
                    att[t] = score / libm::sqrtf(hs as f32);
                }
                let a = &mut att[..=pos];
                let max = a.iter().fold(a[0], |m, &v| m.max(v));
                let mut sum = 0.0f32;
                for v in a.iter_mut() {
                    *v = libm::expf(*v - max);
                    sum += *v;
                }
                for v in a.iter_mut() {
                    *v /= sum;
                }
                let xh = &mut xb[h * hs..][..hs];
                xh.fill(0.0);
                for t in 0..=pos {
                    let vh = &vc[loff + t * kv_dim + kv_off..][..hs];
                    for i in 0..hs {
                        xh[i] += a[t] * vh[i];
                    }
                }
                tr.extend(xh.iter().map(|v| v.to_bits()));
            }
            matmul(&mut xb2, &xb, &wo[l * dim * dim..][..dim * dim], &mut tr);
            for i in 0..dim {
                x[i] += xb2[i];
            }
            rmsnorm(&mut xb, &x, &rms_ffn[l * dim..][..dim], &mut tr);
            matmul(&mut hb, &xb, &w1[l * hidden * dim..][..hidden * dim], &mut tr);
            matmul(&mut hb2, &xb, &w3[l * hidden * dim..][..hidden * dim], &mut tr);
            for i in 0..hidden {
                let mut val = hb[i];
                val *= 1.0 / (1.0 + libm::expf(-val));
                hb[i] = val * hb2[i];
            }
            matmul(&mut xb, &hb, &w2[l * dim * hidden..][..dim * hidden], &mut tr);
            for i in 0..dim {
                x[i] += xb[i];
            }
        }
        let xc = x.clone();
        rmsnorm(&mut x, &xc, rms_final, &mut tr);
        matmul(&mut logits, &x, wcls, &mut tr);
        token = if pos + 1 < p.prompt.len() {
            p.prompt[pos + 1]
        } else {
            let next = argmax(&logits);
            if next == 1 {
                break;
            }
            out.push(next);
            if out.len() == p.max_new as usize {
                break;
            }
            next
        };
    }
    (out, tr)
}

fn check(bytes: &[u8], p: &GenParams) -> (Vec<u32>, usize) {
    let model = ModelF32::parse(bytes);
    let (tokens, trace) = reference(bytes, p);
    assert!(!trace.is_empty());
    let dots: [(&str, WordDotFn); 2] = [("soft", dot_words_soft), ("floatdot", dot_words_floatdot)];
    for (name, dot) in dots {
        let (t, tr) = generate_f32(&model, p, dot, true, |_| {});
        assert_eq!(t, tokens, "{name} tokens");
        assert_eq!(tr.len(), trace.len(), "{name} trace length");
        let first = tr.iter().zip(&trace).position(|(a, b)| a != b);
        assert_eq!(first, None, "{name}: first differing trace value");
    }
    (tokens, trace.len())
}

/// A random checkpoint: dim 24, hidden 32, 2 layers, 4 heads of size 6 (not a
/// power of two, so dividing by its square root rounds) with 2 KV heads,
/// vocabulary 32, context 16; `shared` picks a shared classifier.
fn checkpoint(shared: bool) -> Vec<u8> {
    let (dim, hidden, layers, kv_dim, vocab, seq) = (24, 32, 2, 12, 32usize, 16);
    let mut b = Vec::new();
    let v = if shared { vocab as i32 } else { -(vocab as i32) };
    for h in [dim as i32, hidden, layers, 4, 2, v, seq] {
        b.extend_from_slice(&h.to_le_bytes());
    }
    let floats = vocab * dim * (1 + !shared as usize)
        + layers as usize
            * (2 * dim + 2 * dim * dim + 2 * kv_dim * dim + 3 * hidden as usize * dim)
        + dim
        + seq as usize * dim / 4;
    let mut s = 7u64;
    for _ in 0..floats {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let v = (s >> 40) as f32 / (1u64 << 23) as f32 - 1.0;
        b.extend_from_slice(&v.to_le_bytes());
    }
    b
}

#[test]
fn matches_run_c_on_random_checkpoints() {
    let p = GenParams { prompt: vec![1, 7, 11], max_new: 6, temperature: 0.0, seed: 42 };
    for shared in [true, false] {
        let (tokens, _) = check(&checkpoint(shared), &p);
        assert_eq!(tokens.len(), 6, "shared = {shared}");
    }
}

#[test]
fn soft_dot_handles_odd_lengths() {
    let x: Vec<u64> = (0..5u64).map(|i| (0x3f80_0000 << 32) | (0x4000_0000 + i)).collect();
    for n in 0..10 {
        let v = |w: &[u64], i: usize| f32::from_bits((w[i / 2] >> (32 * (i % 2))) as u32);
        let mut acc = 0.0f32;
        for i in 0..n {
            acc += v(&x, i) * v(&x, i);
        }
        assert_eq!(dot_words_soft(&x, &x, n).to_bits(), acc.to_bits(), "n = {n}");
        assert_eq!(dot_words_floatdot(&x, &x, n).to_bits(), acc.to_bits(), "n = {n}");
    }
}

#[test]
#[ignore = "needs models/stories260K.bin (see README)"]
fn matches_run_c_on_stories260k() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../models/stories260K.bin");
    let p = GenParams { prompt: vec![1, 403, 407, 261], max_new: 2, temperature: 0.0, seed: 42 };
    let (tokens, trace_len) = check(&std::fs::read(path).unwrap(), &p);
    println!("stories260K: tokens {tokens:?}, trace {trace_len} values");
}
