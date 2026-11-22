//! The transformer forward pass in `f32`, following llama2.c's `run.c`: the
//! checkpoint's float weights as they are, without int8 quantization.
//!
//! Every dot product goes through one [`WordDotFn`]: the matrix
//! multiplications (`wq`, `wk`, `wv`, `wo`, `w1`, `w2`, `w3` and the
//! classifier), the RMSNorm sums of squares, the attention scores and the
//! attention's weighted sums of the values. Its operands are packed two `f32`
//! per `u64` word, low half first, the layout the FloatDot precompile reads,
//! so [`dot_words_soft`] and [`dot_words_floatdot`] get the same operands.
//! The weights are packed once when loading (the checkpoint's 28-byte header
//! leaves them 4-byte aligned only), and each input vector once per use.

use alloc::vec;
use alloc::vec::Vec;

use crate::checkpoint::{Config, read_config};
use crate::forward::{Trace, softmax};
use crate::generate::{Forward, generate_steps};
use crate::params::GenParams;
use crate::read_f32;

/// Computes `((0 + x[0]·y[0]) + x[1]·y[1]) + …` over the first `n` values of
/// two packed vectors, every product and every sum rounded separately.
pub type WordDotFn = fn(&[u64], &[u64], usize) -> f32;

/// [`WordDotFn`] in plain Rust: `acc += a * b`, as llama2.c's loops.
pub fn dot_words_soft(x: &[u64], y: &[u64], n: usize) -> f32 {
    let lo = |w: u64| f32::from_bits(w as u32);
    let hi = |w: u64| f32::from_bits((w >> 32) as u32);
    let mut acc = 0.0f32;
    for (&a, &b) in x[..n / 2].iter().zip(&y[..n / 2]) {
        acc += lo(a) * lo(b);
        acc += hi(a) * hi(b);
    }
    if n % 2 == 1 {
        acc += lo(x[n / 2]) * lo(y[n / 2]);
    }
    acc
}

/// [`WordDotFn`] with the FloatDot precompile (`syscall_fp32_dot`), at most
/// 4096 values per call, each call starting from the previous one's result.
#[cfg(feature = "floatdot")]
pub fn dot_words_floatdot(x: &[u64], y: &[u64], n: usize) -> f32 {
    const MAX: usize = 4096;
    let words = n.div_ceil(2);
    let mut acc = 0u32; // +0.0, as the loops start
    for (i, (xc, yc)) in x[..words].chunks(MAX / 2).zip(y[..words].chunks(MAX / 2)).enumerate() {
        let len = (n - i * MAX).min(MAX);
        let r =
            ziskos::syscalls::syscall_fp32_dot(xc, yc, len, acc, 0).expect("valid FloatDot call");
        acc = r as u32; // the result; the high half holds the sticky flags
    }
    f32::from_bits(acc)
}

/// Packs `v` into `words`, two values per word, low half first. An odd last
/// value gets a zero high half.
fn pack(v: &[f32], words: &mut [u64]) {
    for (w, pair) in words.iter_mut().zip(v.chunks(2)) {
        let hi = pair.get(1).map_or(0, |h| h.to_bits());
        *w = pair[0].to_bits() as u64 | (hi as u64) << 32;
    }
}

/// A packed row-major matrix with an even number of columns: row `r` is
/// `words[r * cols / 2..][..cols / 2]`.
struct Matrix {
    words: Vec<u64>,
    cols: usize,
}

/// The parsed llama2.c checkpoint (layout in [`crate::checkpoint`]).
pub struct ModelF32 {
    pub config: Config,
    rms_att: Vec<f32>,
    rms_ffn: Vec<f32>,
    rms_final: Vec<f32>,
    embedding: Matrix,
    wq: Matrix,
    wk: Matrix,
    wv: Matrix,
    wo: Matrix,
    w1: Matrix,
    w2: Matrix,
    w3: Matrix,
    /// `None` when the classifier is the embedding.
    wcls: Option<Matrix>,
}

impl ModelF32 {
    /// Panics if `bytes` is not a llama2.c checkpoint with even dimensions.
    pub fn parse(bytes: &[u8]) -> Self {
        let (c, shared) = read_config(bytes).expect("not a llama2.c checkpoint");
        let (dim, hidden, layers, kv_dim) = (c.dim, c.hidden_dim, c.n_layers, c.kv_dim());
        let mut at = 28;
        let mut next = |n: usize| {
            at += 4 * n;
            assert!(at <= bytes.len(), "checkpoint is truncated");
            at - 4 * n
        };
        let floats =
            |from: usize, n: usize| (0..n).map(|i| read_f32(bytes, from + 4 * i)).collect();
        let matrix = |from: usize, rows: usize, cols: usize| {
            assert!(cols % 2 == 0, "odd matrix width");
            let b = &bytes[from..from + 4 * rows * cols];
            let words = b.chunks_exact(8).map(|w| u64::from_le_bytes(w.try_into().unwrap()));
            Matrix { words: words.collect(), cols }
        };
        let embedding = matrix(next(c.vocab_size * dim), c.vocab_size, dim);
        let rms_att = floats(next(layers * dim), layers * dim);
        let wq = matrix(next(layers * dim * dim), layers * dim, dim);
        let wk = matrix(next(layers * kv_dim * dim), layers * kv_dim, dim);
        let wv = matrix(next(layers * kv_dim * dim), layers * kv_dim, dim);
        let wo = matrix(next(layers * dim * dim), layers * dim, dim);
        let rms_ffn = floats(next(layers * dim), layers * dim);
        let w1 = matrix(next(layers * hidden * dim), layers * hidden, dim);
        let w2 = matrix(next(layers * dim * hidden), layers * dim, hidden);
        let w3 = matrix(next(layers * hidden * dim), layers * hidden, dim);
        let rms_final = floats(next(dim), dim);
        next(c.seq_len * c.head_size()); // freq_cis_real and freq_cis_imag
        let wcls = (!shared).then(|| matrix(next(c.vocab_size * dim), c.vocab_size, dim));
        assert!(c.head_size() % 2 == 0 && kv_dim % 2 == 0, "odd head size");
        ModelF32 {
            config: c,
            rms_att,
            rms_ffn,
            rms_final,
            embedding,
            wq,
            wk,
            wv,
            wo,
            w1,
            w2,
            w3,
            wcls,
        }
    }
}

/// `out[i] = row first_row + i of w · x`, with `x` packed.
fn matmul(dot: WordDotFn, out: &mut [f32], xw: &[u64], w: &Matrix, first_row: usize) {
    let half = w.cols / 2;
    for (i, o) in out.iter_mut().enumerate() {
        *o = dot(&w.words[(first_row + i) * half..][..half], xw, w.cols);
    }
}

/// llama2.c's `rmsnorm`, packing `x` into `xw` for its sum of squares.
fn rmsnorm(
    dot: WordDotFn,
    out: &mut [f32],
    x: &[f32],
    weight: &[f32],
    xw: &mut [u64],
    trace: &mut Trace,
) {
    pack(x, xw);
    let mut ss = dot(xw, xw, x.len());
    trace.push(&[ss]);
    ss /= x.len() as f32;
    ss += 1e-5;
    ss = 1.0 / libm::sqrtf(ss);
    for ((o, &v), &w) in out.iter_mut().zip(x).zip(weight) {
        *o = w * (ss * v);
    }
}

/// Activations and the KV cache for one sequence.
pub struct RunF32<'a> {
    model: &'a ModelF32,
    dot: WordDotFn,
    x: Vec<f32>,
    xb: Vec<f32>,
    xb2: Vec<f32>,
    hb: Vec<f32>,
    hb2: Vec<f32>,
    q: Vec<f32>,
    k: Vec<f32>,
    v: Vec<f32>,
    att: Vec<f32>,
    logits: Vec<f32>,
    /// The packed input of the current matrix multiplication or RMSNorm.
    xw: Vec<u64>,
    /// The packed query, attention weights and one column of the values.
    qw: Vec<u64>,
    aw: Vec<u64>,
    cw: Vec<u64>,
    /// The keys are packed when stored; the values are gathered by column.
    key_cache: Vec<u64>,
    value_cache: Vec<f32>,
    rope_freq: Vec<f32>,
    pub trace: Trace,
}

impl<'a> RunF32<'a> {
    pub fn new(model: &'a ModelF32, dot: WordDotFn) -> Self {
        let c = &model.config;
        let (dim, kv_dim, hidden, hs) = (c.dim, c.kv_dim(), c.hidden_dim, c.head_size());
        let rope_freq =
            (0..hs).step_by(2).map(|hd| 1.0 / libm::powf(10000.0, hd as f32 / hs as f32)).collect();
        RunF32 {
            model,
            dot,
            x: vec![0.0; dim],
            xb: vec![0.0; dim],
            xb2: vec![0.0; dim],
            hb: vec![0.0; hidden],
            hb2: vec![0.0; hidden],
            q: vec![0.0; dim],
            k: vec![0.0; kv_dim],
            v: vec![0.0; kv_dim],
            att: vec![0.0; c.seq_len],
            logits: vec![0.0; c.vocab_size],
            xw: vec![0; dim.max(hidden).div_ceil(2)],
            qw: vec![0; dim / 2],
            aw: vec![0; c.seq_len.div_ceil(2)],
            cw: vec![0; c.seq_len.div_ceil(2)],
            key_cache: vec![0; c.n_layers * c.seq_len * kv_dim / 2],
            value_cache: vec![0.0; c.n_layers * c.seq_len * kv_dim],
            rope_freq,
            trace: Trace(None),
        }
    }
}

impl Forward for RunF32<'_> {
    /// llama2.c's `forward`. Traces each RMSNorm sum of squares, the outputs
    /// of every matrix multiplication, every attention score and output, and
    /// the logits.
    fn forward(&mut self, token: u32, pos: usize) -> &mut [f32] {
        let (m, dot) = (self.model, self.dot);
        let s = self;
        let c = &m.config;
        let (dim, kv_dim, hidden, hs) = (c.dim, c.kv_dim(), c.hidden_dim, c.head_size());
        let kv_mul = c.n_heads / c.n_kv_heads;

        let row = &m.embedding.words[token as usize * dim / 2..][..dim / 2];
        for (pair, &w) in s.x.chunks_mut(2).zip(row) {
            pair[0] = f32::from_bits(w as u32);
            pair[1] = f32::from_bits((w >> 32) as u32);
        }

        let sqrt_hs = libm::sqrtf(hs as f32);
        for l in 0..c.n_layers {
            // Attention.
            let rms = &m.rms_att[l * dim..(l + 1) * dim];
            rmsnorm(dot, &mut s.xb, &s.x, rms, &mut s.xw, &mut s.trace);
            pack(&s.xb, &mut s.xw);
            matmul(dot, &mut s.q, &s.xw, &m.wq, l * dim);
            matmul(dot, &mut s.k, &s.xw, &m.wk, l * kv_dim);
            matmul(dot, &mut s.v, &s.xw, &m.wv, l * kv_dim);
            s.trace.push(&s.q);
            s.trace.push(&s.k);
            s.trace.push(&s.v);

            // RoPE.
            for i in (0..dim).step_by(2) {
                let val = pos as f32 * s.rope_freq[(i % hs) / 2];
                let (fcr, fci) = (libm::cosf(val), libm::sinf(val));
                let rotn = if i < kv_dim { 2 } else { 1 };
                for r in 0..rotn {
                    let v = if r == 0 { &mut s.q } else { &mut s.k };
                    let (v0, v1) = (v[i], v[i + 1]);
                    v[i] = v0 * fcr - v1 * fci;
                    v[i + 1] = v0 * fci + v1 * fcr;
                }
            }

            let loff = l * c.seq_len * kv_dim;
            pack(&s.k, &mut s.key_cache[(loff + pos * kv_dim) / 2..][..kv_dim / 2]);
            s.value_cache[loff + pos * kv_dim..][..kv_dim].copy_from_slice(&s.v);
            pack(&s.q, &mut s.qw);

            for h in 0..c.n_heads {
                let qh = &s.qw[h * hs / 2..][..hs / 2];
                let kv_off = (h / kv_mul) * hs;
                let att = &mut s.att[..pos + 1];
                for (t, a) in att.iter_mut().enumerate() {
                    let kh = &s.key_cache[(loff + t * kv_dim + kv_off) / 2..][..hs / 2];
                    let score = dot(qh, kh, hs);
                    s.trace.push(&[score]);
                    *a = score / sqrt_hs;
                }
                softmax(att);
                pack(att, &mut s.aw);
                for i in 0..hs {
                    // Column i of this head's values, packed like `pack`.
                    let value = |t: usize| match t <= pos {
                        true => s.value_cache[loff + t * kv_dim + kv_off + i].to_bits() as u64,
                        false => 0,
                    };
                    for (j, w) in s.cw[..(pos + 1).div_ceil(2)].iter_mut().enumerate() {
                        *w = value(2 * j) | value(2 * j + 1) << 32;
                    }
                    s.xb[h * hs + i] = dot(&s.aw, &s.cw, pos + 1);
                }
                s.trace.push(&s.xb[h * hs..(h + 1) * hs]);
            }

            pack(&s.xb, &mut s.xw);
            matmul(dot, &mut s.xb2, &s.xw, &m.wo, l * dim);
            s.trace.push(&s.xb2);
            for (x, &d) in s.x.iter_mut().zip(&s.xb2) {
                *x += d;
            }

            // Feed-forward: w2(silu(w1 x) * w3 x).
            let rms = &m.rms_ffn[l * dim..(l + 1) * dim];
            rmsnorm(dot, &mut s.xb, &s.x, rms, &mut s.xw, &mut s.trace);
            pack(&s.xb, &mut s.xw);
            matmul(dot, &mut s.hb, &s.xw, &m.w1, l * hidden);
            matmul(dot, &mut s.hb2, &s.xw, &m.w3, l * hidden);
            s.trace.push(&s.hb);
            s.trace.push(&s.hb2);
            for (h, &h2) in s.hb.iter_mut().zip(&s.hb2) {
                let mut val = *h;
                val *= 1.0 / (1.0 + libm::expf(-val));
                *h = val * h2;
            }
            pack(&s.hb, &mut s.xw);
            matmul(dot, &mut s.xb, &s.xw, &m.w2, l * dim);
            s.trace.push(&s.xb);
            for (x, &d) in s.x.iter_mut().zip(&s.xb) {
                *x += d;
            }
        }

        let x = s.x.clone();
        rmsnorm(dot, &mut s.x, &x, &m.rms_final, &mut s.xw, &mut s.trace);
        pack(&s.x, &mut s.xw);
        matmul(dot, &mut s.logits, &s.xw, m.wcls.as_ref().unwrap_or(&m.embedding), 0);
        s.trace.push(&s.logits);
        &mut s.logits
    }
}

/// [`crate::generate::generate_traced`] for the `f32` model, with `dot`
/// computing every dot product.
pub fn generate_f32(
    model: &ModelF32,
    p: &GenParams,
    dot: WordDotFn,
    trace: bool,
    on_token: impl FnMut(u32),
) -> (Vec<u32>, Vec<u32>) {
    let mut run = RunF32::new(model, dot);
    run.trace = Trace(trace.then(Vec::new));
    let out = generate_steps(&mut run, model.config.seq_len, p, on_token);
    (out, run.trace.0.unwrap_or_default())
}
