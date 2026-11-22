//! The transformer forward pass, following llama2.c's `runq.c`.

use alloc::vec;
use alloc::vec::Vec;

use crate::fixed;
use crate::formats::{Format, quantize_group, shr_round_even};
use crate::model::{Model, QMatrixView};
use zisk_definitions::DOT_F8_MAX_N;

/// Computes the exact dot products of one matrix row `w` (format `wf`) and a
/// vector `x` (format `xf`), both of `n` values, in groups of `gs`: writes the
/// sum of each group to `out`, in units of the two formats (see
/// [`crate::formats`]). This is what the `dot_f8` precompile computes in one
/// call; [`forward`] calls it once per row of every matrix multiplication.
pub type DotFn =
    fn(w: &[u8], wf: Format, x: &[u8], xf: Format, n: usize, gs: usize, out: &mut [i64]);

/// Writes to `out` the exact dot product of each of the `rows` rows of `w`
/// (`rows * cols` values, format `wf`) with `x` (`cols` values, format `xf`).
pub type MatrixFn =
    fn(w: &[u8], wf: Format, x: &[u8], xf: Format, rows: usize, cols: usize, out: &mut [i64]);

/// [`DotFn`] in plain Rust.
///
/// For int8 this is llama2.c's inner loop. It sums in `i64`: with 32-bit
/// sums the compiler uses `addw`, which ZisK proves with the Binary machine
/// instead of the cheaper BinaryAdd. The other formats decode each value with
/// a table lookup.
pub fn dot_soft(w: &[u8], wf: Format, x: &[u8], xf: Format, n: usize, gs: usize, out: &mut [i64]) {
    if wf == Format::Int8 && xf == Format::Int8 {
        for ((wg, xg), o) in w[..n].chunks_exact(gs).zip(x[..n].chunks_exact(gs)).zip(out) {
            let mut acc: i64 = 0;
            for (&a, &b) in wg.iter().zip(xg) {
                acc += a as i8 as i64 * b as i8 as i64;
            }
            *o = acc;
        }
    } else {
        zisk_definitions::dot_f8_reference(wf.id(), w, xf.id(), x, n, gs, Some(out));
    }
}

/// [`MatrixFn`] in plain Rust.
pub fn matrix_soft(
    w: &[u8],
    wf: Format,
    x: &[u8],
    xf: Format,
    rows: usize,
    cols: usize,
    out: &mut [i64],
) {
    let n = rows * cols;
    zisk_definitions::dot_f8_reference_with(wf.id(), w, xf.id(), x, n, cols, true, Some(out));
}

/// How the matrix multiplications combine each group's dot product with the
/// group's two scales.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scales {
    /// In `f32`, as llama2.c: `val += ival · w_scale · x_scale` per group.
    Float,
    /// In fixed point (see [`crate::fixed`]): an exact integer sum per row,
    /// converted to `f32` once.
    Fixed,
}

/// How the matrix multiplications are computed.
#[derive(Clone, Copy)]
pub struct MatmulMode {
    /// The dot products of one row.
    pub dot: DotFn,
    /// The row sums of a whole matrix, for matrices whose rows are one group;
    /// `None` computes them with `dot`, row by row.
    pub matrix: Option<MatrixFn>,
    pub scales: Scales,
}

impl MatmulMode {
    /// Plain Rust, with the given scales.
    pub fn soft(scales: Scales) -> Self {
        MatmulMode { dot: dot_soft, matrix: None, scales }
    }
}

impl Default for MatmulMode {
    /// llama2.c's: [`dot_soft`] and [`Scales::Float`].
    fn default() -> Self {
        Self::soft(Scales::Float)
    }
}

/// An activation vector quantized in groups, like a matrix row, in the
/// model's activation format.
///
/// The values are stored in `u64` words, so they are 8-byte aligned, as the
/// `dot_f8` precompile requires.
struct QVec {
    words: Vec<u64>,
    n: usize,
    format: Format,
    /// The scale of one unit of each group.
    scales: Vec<f32>,
    /// The scales in fixed point, `scales[g] ≈ fixed_scales[g] · 2^-shift`,
    /// when quantized for [`Scales::Fixed`].
    fixed_scales: Vec<i32>,
    shift: i32,
}

impl QVec {
    fn new(n: usize, format: Format) -> Self {
        let words = vec![0; n.div_ceil(8)];
        QVec { words, n, format, scales: vec![0.0; n], fixed_scales: vec![0; n], shift: 0 }
    }

    fn bytes(&self) -> &[u8] {
        // SAFETY: the words hold at least `n` bytes.
        unsafe { core::slice::from_raw_parts(self.words.as_ptr().cast::<u8>(), self.n) }
    }

    /// Quantizes `x` in groups of `gs` (llama2.c's `quantize` for int8), and
    /// converts the scales to fixed point for [`Scales::Fixed`].
    fn quantize(&mut self, x: &[f32], gs: usize, scales: Scales) {
        let f = self.format;
        // SAFETY: as in `bytes`, and `&mut self` makes the borrow unique.
        let q = unsafe {
            core::slice::from_raw_parts_mut(self.words.as_mut_ptr().cast::<u8>(), self.n)
        };
        for (g, group) in x.chunks_exact(gs).enumerate() {
            self.scales[g] =
                quantize_group(f, group, &mut q[f.bytes(g * gs)..f.bytes((g + 1) * gs)]);
        }
        if scales == Scales::Fixed {
            let groups = x.len() / gs;
            self.shift =
                fixed::scales_to_fixed(&self.scales[..groups], &mut self.fixed_scales[..groups]);
        }
    }
}

/// `out = W x`, with exact dot products per group (llama2.c's `matmul`).
/// `sums` holds the group sums of one row, or the row sums of a matrix call
/// (at least `w.rows`).
fn matmul(out: &mut [f32], x: &QVec, w: &QMatrixView, mode: MatmulMode, sums: &mut [i64]) {
    let (n, gs) = (w.cols, w.gs);
    let (wf, xq) = (w.format, x.bytes());
    let groups = n / gs;
    let x_fixed = &x.fixed_scales[..groups];
    // int8 sums fit an i64 with the scales applied (see fixed::MAX_LEN); the
    // floats need an i128.
    let wide = wf != Format::Int8 || x.format != Format::Int8;
    let matrix = mode.matrix.filter(|_| groups == 1);
    let chunk = if matrix.is_some() { (DOT_F8_MAX_N as usize / n).max(1) } else { 1 };
    for (c, outs) in out[..w.rows].chunks_mut(chunk).enumerate() {
        if let Some(f) = matrix {
            let rows = outs.len();
            let ws = &w.q[wf.bytes(c * chunk * n)..wf.bytes((c * chunk + rows) * n)];
            f(ws, wf, xq, x.format, rows, n, &mut sums[..rows]);
        }
        for (k, o) in outs.iter_mut().enumerate() {
            let i = c * chunk + k;
            let sums: &[i64] = if matrix.is_some() {
                &sums[k..k + 1]
            } else {
                let row = &w.q[wf.bytes(i * n)..wf.bytes((i + 1) * n)];
                (mode.dot)(row, wf, xq, x.format, n, gs, &mut sums[..groups]);
                &sums[..groups]
            };
            *o = scale_row(sums, w, i, x, x_fixed, mode.scales, wide);
        }
    }
}

/// Row `i` of `W x` from its group sums.
fn scale_row(
    sums: &[i64],
    w: &QMatrixView,
    i: usize,
    x: &QVec,
    x_fixed: &[i32],
    scales: Scales,
    wide: bool,
) -> f32 {
    let groups = sums.len();
    let (g0, g1) = (i * groups, (i + 1) * groups);
    match scales {
        Scales::Float => {
            let mut val = 0.0f32;
            for ((&ival, &ws), &xs) in sums.iter().zip(&w.scales[g0..g1]).zip(&x.scales) {
                val += ival as f32 * ws * xs;
            }
            val
        }
        Scales::Fixed if !wide => {
            let mut acc: i64 = 0;
            for ((&ival, &ws), &xs) in sums.iter().zip(&w.fixed_scales[g0..g1]).zip(x_fixed) {
                acc += ival * (ws as i64 * xs as i64);
            }
            fixed::fixed_to_f32(acc, w.row_shift[i] + x.shift)
        }
        Scales::Fixed => {
            let mut acc: i128 = 0;
            for ((&ival, &ws), &xs) in sums.iter().zip(&w.fixed_scales[g0..g1]).zip(x_fixed) {
                acc += ival as i128 * (ws as i64 * xs as i64) as i128;
            }
            fixed::wide_fixed_to_f32(acc, w.row_shift[i] + x.shift)
        }
    }
}

fn rmsnorm(out: &mut [f32], x: &[f32], weight: &[f32]) {
    let mut ss = 0.0f32;
    for &v in x {
        ss += v * v;
    }
    ss /= x.len() as f32;
    ss += 1e-5;
    ss = 1.0 / libm::sqrtf(ss);
    for ((o, &v), &w) in out.iter_mut().zip(x).zip(weight) {
        *o = w * (ss * v);
    }
}

/// In-place softmax.
pub fn softmax(x: &mut [f32]) {
    softmax_with(x, libm::expf);
}

pub fn softmax_with(x: &mut [f32], exp: fn(f32) -> f32) {
    let max = x.iter().fold(x[0], |m, &v| m.max(v));
    let mut sum = 0.0f32;
    for v in x.iter_mut() {
        *v = exp(*v - max);
        sum += *v;
    }
    for v in x.iter_mut() {
        *v /= sum;
    }
}

/// Converts `v` to integers below 2^15 in magnitude that share the returned
/// exponent `e`: `v[i] ≈ out[i] · 2^e`, truncated toward zero. It works on the
/// `f32` bits, so it needs no software float operation; subnormals become 0.
fn to_block(v: &[f32], out: &mut [i32]) -> i32 {
    let exp = |x: f32| ((x.to_bits() >> 23) & 0xff) as i32;
    let emax = v.iter().map(|&x| exp(x)).max().unwrap_or(0);
    for (o, &x) in out.iter_mut().zip(v) {
        let (b, e) = (x.to_bits(), exp(x));
        let m = if e == 0 { 0 } else { ((b & 0x7f_ffff) | 0x80_0000) as i32 };
        let shift = emax - e + 9;
        let mag = if shift >= 31 { 0 } else { m >> shift };
        *o = if b >> 31 == 1 { -mag } else { mag };
    }
    emax - 150 + 9
}

/// `x · 2^e` by adding to the exponent field; 0 below the normal range.
fn ldexp(x: f32, e: i32) -> f32 {
    let b = x.to_bits();
    if b & 0x7fff_ffff == 0 {
        return x;
    }
    let ex = ((b >> 23) & 0xff) as i32 + e;
    if ex <= 0 {
        return 0.0;
    }
    if ex >= 255 {
        return f32::from_bits((b & 0x8000_0000) | 0x7f80_0000);
    }
    f32::from_bits((b & !(0xff << 23)) | ((ex as u32) << 23))
}

/// Activations and the KV cache for one sequence.
pub struct RunState {
    x: Vec<f32>,
    xb: Vec<f32>,
    xb2: Vec<f32>,
    hb: Vec<f32>,
    hb2: Vec<f32>,
    xq: QVec,
    hq: QVec,
    /// The group sums of one matrix row, or the row sums of a matrix call.
    sums: Vec<i64>,
    q: Vec<f32>,
    k: Vec<f32>,
    v: Vec<f32>,
    att: Vec<f32>,
    pub logits: Vec<f32>,
    key_cache: Vec<f32>,
    value_cache: Vec<f32>,
    /// RoPE frequency for each pair of dimensions within a head.
    rope_freq: Vec<f32>,
    rope_cos: Vec<f32>,
    rope_sin: Vec<f32>,
    /// How the matrix multiplications are computed.
    mode: MatmulMode,
    /// The exponential of the softmax and SiLU: `libm::expf` unless set.
    exp: fn(f32) -> f32,
    /// See [`Self::set_fixed_attention`]. The cache is also kept as block
    /// floating point, one exponent per head vector.
    fixed_attention: bool,
    key_int: Vec<i32>,
    value_int: Vec<i32>,
    key_exp: Vec<i32>,
    value_exp: Vec<i32>,
    q_scaled: Vec<f32>,
    q_int: Vec<i32>,
    weights: Vec<i32>,
    /// Attention with E4M3 dot products ([`Self::set_f8_attention`]): keys
    /// as `[layer][kv head][position][head_size]`, values transposed as
    /// `[layer][kv head][head_size][position]`, each position's vector with
    /// a power-of-two scale (`key_exp`, `value_exp`).
    f8_attention: bool,
    key_f8: Vec<u64>,
    value_f8: Vec<u64>,
    q_f8: Vec<u64>,
    weights_f8: Vec<u64>,
    f8_tmp: Vec<u8>,
    dots: Vec<i64>,
}

impl RunState {
    /// Computes the matrix multiplications as llama2.c ([`MatmulMode::default`]).
    pub fn new(model: &Model) -> Self {
        Self::with_mode(model, MatmulMode::default())
    }

    pub fn with_mode(model: &Model, mode: MatmulMode) -> Self {
        let c = &model.config;
        let (dim, kv_dim, hidden, hs) = (c.dim, c.kv_dim(), c.hidden_dim, c.head_size());
        let rope_freq =
            (0..hs).step_by(2).map(|hd| 1.0 / libm::powf(10000.0, hd as f32 / hs as f32)).collect();
        RunState {
            x: vec![0.0; dim],
            xb: vec![0.0; dim],
            xb2: vec![0.0; dim],
            hb: vec![0.0; hidden],
            hb2: vec![0.0; hidden],
            xq: QVec::new(dim, model.activations),
            hq: QVec::new(hidden, model.activations),
            sums: vec![0; dim.max(hidden).max(c.vocab_size)],
            q: vec![0.0; dim],
            k: vec![0.0; kv_dim],
            v: vec![0.0; kv_dim],
            att: vec![0.0; c.n_heads * c.seq_len],
            logits: vec![0.0; c.vocab_size],
            key_cache: vec![0.0; c.n_layers * c.seq_len * kv_dim],
            value_cache: vec![0.0; c.n_layers * c.seq_len * kv_dim],
            rope_freq,
            rope_cos: vec![0.0; hs / 2],
            rope_sin: vec![0.0; hs / 2],
            mode,
            exp: libm::expf,
            fixed_attention: false,
            key_int: vec![0; c.n_layers * c.seq_len * kv_dim],
            value_int: vec![0; c.n_layers * c.seq_len * kv_dim],
            key_exp: vec![0; c.n_layers * c.seq_len * c.n_kv_heads],
            value_exp: vec![0; c.n_layers * c.seq_len * c.n_kv_heads],
            q_scaled: vec![0.0; hs],
            q_int: vec![0; hs],
            weights: vec![0; c.seq_len],
            f8_attention: false,
            key_f8: vec![0; c.n_layers * c.seq_len * kv_dim / 8],
            value_f8: vec![0; c.n_layers * c.seq_len * kv_dim / 8],
            q_f8: vec![0; hs.div_ceil(8)],
            weights_f8: vec![0; c.seq_len.div_ceil(8)],
            f8_tmp: vec![0; hs],
            dots: vec![0; c.seq_len.max(hs)],
        }
    }

    /// Runs attention as matrix calls on an E4M3 copy of the cache: per head,
    /// one for the scores and one for the weighted sum of the values. Needs
    /// `head_size` and `seq_len` multiples of 8.
    pub fn set_f8_attention(&mut self, on: bool) {
        self.f8_attention = on;
    }

    /// E4M3 keys and values of layer `l`, position `pos` of the cache.
    fn cache_to_f8(&mut self, model: &Model, l: usize, pos: usize) {
        let c = &model.config;
        let (kv_dim, hs, nkv, seq) = (c.kv_dim(), c.head_size(), c.n_kv_heads, c.seq_len);
        let off = (l * seq + pos) * kv_dim;
        let (keys, values) = (bytes_mut(&mut self.key_f8), bytes_mut(&mut self.value_f8));
        for j in 0..nkv {
            let (a, e) = (off + j * hs, (l * seq + pos) * nkv + j);
            let base = (l * nkv + j) * seq * hs;
            let key = &mut keys[base + pos * hs..][..hs];
            self.key_exp[e] = encode_pow2(&self.key_cache[a..a + hs], key);
            self.value_exp[e] = encode_pow2(&self.value_cache[a..a + hs], &mut self.f8_tmp);
            for (i, &v) in self.f8_tmp.iter().enumerate() {
                values[base + i * seq + pos] = v;
            }
        }
    }

    /// Runs attention in integers, with 15-bit softmax weights. It avoids
    /// most software float operations; the results differ slightly from `f32`.
    pub fn set_fixed_attention(&mut self, on: bool) {
        self.fixed_attention = on;
    }

    /// Block floating point of layer `l`, position `pos` of the cache.
    fn cache_to_int(&mut self, model: &Model, l: usize, pos: usize) {
        let c = &model.config;
        let (kv_dim, hs) = (c.kv_dim(), c.head_size());
        let off = (l * c.seq_len + pos) * kv_dim;
        for j in 0..c.n_kv_heads {
            let (a, e) = (off + j * hs, (l * c.seq_len + pos) * c.n_kv_heads + j);
            self.key_exp[e] = to_block(&self.key_cache[a..a + hs], &mut self.key_int[a..a + hs]);
            self.value_exp[e] =
                to_block(&self.value_cache[a..a + hs], &mut self.value_int[a..a + hs]);
        }
    }

    /// Replaces the exponential of the softmax and SiLU.
    pub fn set_exp(&mut self, exp: fn(f32) -> f32) {
        self.exp = exp;
    }

    /// The key and value cache of positions `0..n` as `f32` LE bytes: for
    /// each layer, its `n` keys, then its `n` values.
    pub fn kv_bytes(&self, model: &Model, n: usize) -> Vec<u8> {
        let c = &model.config;
        let (kv_dim, seq) = (c.kv_dim(), c.seq_len);
        let mut out = Vec::with_capacity(c.n_layers * 2 * n * kv_dim * 4);
        for l in 0..c.n_layers {
            for cache in [&self.key_cache, &self.value_cache] {
                let rows = &cache[l * seq * kv_dim..][..n * kv_dim];
                out.extend(rows.iter().flat_map(|v| v.to_le_bytes()));
            }
        }
        out
    }

    /// Restores positions `0..n` of the cache from [`Self::kv_bytes`] and
    /// returns `n`, the number of positions `bytes` holds.
    pub fn set_kv(&mut self, model: &Model, bytes: &[u8]) -> usize {
        let c = &model.config;
        let (kv_dim, seq) = (c.kv_dim(), c.seq_len);
        let per_pos = c.n_layers * 2 * kv_dim * 4;
        assert!(bytes.len() % per_pos == 0, "kv cache: whole positions");
        let n = bytes.len() / per_pos;
        assert!(n <= seq, "kv cache: more positions than seq_len");
        let mut floats = bytes.chunks_exact(4).map(|b| f32::from_le_bytes(b.try_into().unwrap()));
        for l in 0..c.n_layers {
            for cache in [&mut self.key_cache, &mut self.value_cache] {
                for v in &mut cache[l * seq * kv_dim..][..n * kv_dim] {
                    *v = floats.next().unwrap();
                }
            }
        }
        for l in 0..c.n_layers {
            for pos in 0..n {
                if self.f8_attention {
                    self.cache_to_f8(model, l, pos);
                } else if self.fixed_attention {
                    self.cache_to_int(model, l, pos);
                }
            }
        }
        n
    }
}

fn bytes_mut(words: &mut [u64]) -> &mut [u8] {
    // SAFETY: u8 has no alignment or validity requirements, and `&mut`
    // makes the borrow unique.
    unsafe { core::slice::from_raw_parts_mut(words.as_mut_ptr().cast::<u8>(), words.len() * 8) }
}

/// Encodes `v` to E4M3 with one power-of-two scale, its largest magnitude
/// in [128, 256), and returns `e`: `v[i] ≈ units(q, i) · 2^e`.
fn encode_pow2(v: &[f32], q: &mut [u8]) -> i32 {
    let max = v.iter().map(|x| x.to_bits() & 0x7fff_ffff).max().unwrap_or(0);
    let top = (max >> 23) as i32;
    for (c, &x) in q.iter_mut().zip(v) {
        let b = x.to_bits();
        *c = e4m3_at(b, ((b >> 23) & 0xff) as i32 - top + 14);
    }
    top - 127 - 7 - Format::E4m3.unit_shift()
}

/// The E4M3 code of the `f32` with bits `b` scaled so that its E4M3
/// exponent field is `f` (below 16), ties to even; `f32` subnormals are 0.
/// [`Format::encode_float`] without its float scaling, for the cache.
fn e4m3_at(b: u32, f: i32) -> u8 {
    if (b >> 23) & 0xff == 0 {
        return 0;
    }
    let sig = (b & 0x7f_ffff) as u64 | 0x80_0000;
    let mag = if f >= 1 {
        // 4 significant bits; a carry moves to the next exponent by itself.
        ((f as u64) << 3) + shr_round_even(sig, 20) - 8
    } else if f >= -3 {
        shr_round_even(sig, (21 - f) as u32) // subnormal, units of 2^-9
    } else {
        0
    };
    let mag = mag.min(0x7e) as u8;
    if mag == 0 { 0 } else { mag | ((b >> 24) as u8 & 0x80) }
}

/// Attention of layer `l` at position `pos` with E4M3 dot products (see
/// [`RunState::set_f8_attention`]); writes each head's output to `s.xb`.
fn f8_attention(model: &Model, s: &mut RunState, l: usize, pos: usize, inv_sqrt_hs: f32) {
    let c = &model.config;
    let (hs, nkv, seq) = (c.head_size(), c.n_kv_heads, c.seq_len);
    let kv_mul = c.n_heads / nkv;
    let f = Format::E4m3;
    let matrix = s.mode.matrix.unwrap_or(matrix_soft);
    let p = pos + 1;
    let (k8, v8) = (bytes_mut(&mut s.key_f8), bytes_mut(&mut s.value_f8));
    for h in 0..c.n_heads {
        let j = h / kv_mul;
        let base = (l * nkv + j) * seq * hs;
        let exps = |e: &[i32], t: usize| e[(l * seq + t) * nkv + j];
        for (d, &x) in s.q_scaled.iter_mut().zip(&s.q[h * hs..(h + 1) * hs]) {
            *d = x * inv_sqrt_hs;
        }
        let q8 = bytes_mut(&mut s.q_f8);
        let eq = encode_pow2(&s.q_scaled, q8);
        matrix(&k8[base..base + p * hs], f, q8, f, p, hs, &mut s.dots[..p]);
        let att = &mut s.att[h * seq..h * seq + p];
        for (t, a) in att.iter_mut().enumerate() {
            *a = ldexp(s.dots[t] as f32, eq + exps(&s.key_exp, t));
        }
        // Softmax weights: 15-bit integers for the sum, and in E4M3 scaled
        // by each value's power of two relative to the largest, 2^emax.
        let emax = (0..p).map(|t| exps(&s.value_exp, t)).max().unwrap();
        let max = att.iter().fold(att[0], |m, &v| m.max(v));
        let w8 = bytes_mut(&mut s.weights_f8);
        let mut sum = 0i64;
        for (t, &a) in att.iter().enumerate() {
            let e = (s.exp)(a - max);
            sum += (e * 32768.0) as i32 as i64;
            // e · 2^(e_t - emax + 8): its E4M3 field is its f32 one - 120 + that.
            let b = e.to_bits();
            let shift = exps(&s.value_exp, t) - emax + 8;
            w8[t] = e4m3_at(b, ((b >> 23) & 0xff) as i32 - 120 + shift);
        }
        w8[p..seq].fill(0);
        matrix(&v8[base..base + hs * seq], f, w8, f, hs, seq, &mut s.dots[..hs]);
        // dots[i] · 2^(emax - 8 - 9) / (sum · 2^-15)
        let xb = &mut s.xb[h * hs..(h + 1) * hs];
        for (o, &d) in xb.iter_mut().zip(&s.dots[..hs]) {
            *o = ldexp(d as f32, emax - 2) / sum as f32;
        }
    }
}

/// Attention of layer `l` at position `pos` in fixed point (see
/// [`RunState::set_fixed_attention`]); writes each head's output to `s.xb`.
fn fixed_attention(model: &Model, s: &mut RunState, l: usize, pos: usize, inv_sqrt_hs: f32) {
    let c = &model.config;
    let (kv_dim, hs, nkv) = (c.kv_dim(), c.head_size(), c.n_kv_heads);
    let kv_mul = c.n_heads / nkv;
    let base = l * c.seq_len;
    for h in 0..c.n_heads {
        let j = h / kv_mul;
        for (d, &x) in s.q_scaled.iter_mut().zip(&s.q[h * hs..(h + 1) * hs]) {
            *d = x * inv_sqrt_hs;
        }
        let eq = to_block(&s.q_scaled, &mut s.q_int);
        let att = &mut s.att[h * c.seq_len..h * c.seq_len + pos + 1];
        for (t, a) in att.iter_mut().enumerate() {
            let k = &s.key_int[(base + t) * kv_dim + j * hs..][..hs];
            let dot: i64 = s.q_int.iter().zip(k).map(|(&x, &y)| x as i64 * y as i64).sum();
            *a = ldexp(dot as f32, eq + s.key_exp[(base + t) * nkv + j]);
        }
        let max = att.iter().fold(att[0], |m, &v| m.max(v));
        let mut sum = 0i64;
        for (w, &a) in s.weights.iter_mut().zip(att.iter()) {
            *w = ((s.exp)(a - max) * 32768.0) as i32;
            sum += *w as i64;
        }
        let emax = (0..=pos).map(|t| s.value_exp[(base + t) * nkv + j]).max().unwrap();
        let xb = &mut s.xb[h * hs..(h + 1) * hs];
        for (i, o) in xb.iter_mut().enumerate() {
            let mut acc = 0i64;
            for t in 0..=pos {
                let shift = emax - s.value_exp[(base + t) * nkv + j];
                if shift < 63 {
                    let v = s.value_int[(base + t) * kv_dim + j * hs + i] as i64;
                    acc += (s.weights[t] as i64 * v) >> shift;
                }
            }
            *o = ldexp(acc as f32, emax) / sum as f32;
        }
    }
}

/// Runs the model on `token` at position `pos` and leaves the next-token
/// logits in `s.logits`.
pub fn forward(model: &Model, s: &mut RunState, token: u32, pos: usize) {
    let c = &model.config;
    let (dim, kv_dim, hidden, hs) = (c.dim, c.kv_dim(), c.hidden_dim, c.head_size());
    let kv_mul = c.n_heads / c.n_kv_heads;
    let (gs_dim, gs_hidden) = (model.wq.gs, model.w2.gs);

    // Token embedding, dequantized.
    let e = &model.embedding;
    let t = token as usize;
    let row = &e.q[e.format.bytes(t * dim)..e.format.bytes((t + 1) * dim)];
    for i in 0..dim {
        s.x[i] = e.format.units(row, i) as f32 * e.scales[(t * dim + i) / e.gs];
    }

    // RoPE rotation for this position; the same for every layer and head.
    for (j, &freq) in s.rope_freq.iter().enumerate() {
        let val = pos as f32 * freq;
        s.rope_cos[j] = libm::cosf(val);
        s.rope_sin[j] = libm::sinf(val);
    }

    let inv_sqrt_hs = 1.0 / libm::sqrtf(hs as f32);
    for l in 0..c.n_layers {
        // Attention.
        rmsnorm(&mut s.xb, &s.x, &model.rms_att[l * dim..(l + 1) * dim]);
        s.xq.quantize(&s.xb, gs_dim, s.mode.scales);
        matmul(&mut s.q, &s.xq, &model.wq.rows(l * dim, dim), s.mode, &mut s.sums);
        matmul(&mut s.k, &s.xq, &model.wk.rows(l * kv_dim, kv_dim), s.mode, &mut s.sums);
        matmul(&mut s.v, &s.xq, &model.wv.rows(l * kv_dim, kv_dim), s.mode, &mut s.sums);

        for i in (0..dim).step_by(2) {
            let j = (i % hs) / 2;
            let (fcr, fci) = (s.rope_cos[j], s.rope_sin[j]);
            let (v0, v1) = (s.q[i], s.q[i + 1]);
            s.q[i] = v0 * fcr - v1 * fci;
            s.q[i + 1] = v0 * fci + v1 * fcr;
            if i < kv_dim {
                let (v0, v1) = (s.k[i], s.k[i + 1]);
                s.k[i] = v0 * fcr - v1 * fci;
                s.k[i + 1] = v0 * fci + v1 * fcr;
            }
        }

        let loff = l * c.seq_len * kv_dim;
        s.key_cache[loff + pos * kv_dim..loff + (pos + 1) * kv_dim].copy_from_slice(&s.k);
        s.value_cache[loff + pos * kv_dim..loff + (pos + 1) * kv_dim].copy_from_slice(&s.v);

        if s.f8_attention {
            s.cache_to_f8(model, l, pos);
            f8_attention(model, s, l, pos, inv_sqrt_hs);
        } else if s.fixed_attention {
            s.cache_to_int(model, l, pos);
            fixed_attention(model, s, l, pos, inv_sqrt_hs);
        } else {
            for h in 0..c.n_heads {
                let q = &s.q[h * hs..(h + 1) * hs];
                let att = &mut s.att[h * c.seq_len..h * c.seq_len + pos + 1];
                let kv_off = (h / kv_mul) * hs;
                for (t, a) in att.iter_mut().enumerate() {
                    let k = &s.key_cache[loff + t * kv_dim + kv_off..][..hs];
                    let mut score = 0.0f32;
                    for (&qi, &ki) in q.iter().zip(k) {
                        score += qi * ki;
                    }
                    *a = score * inv_sqrt_hs;
                }
                softmax_with(att, s.exp);
                let xb = &mut s.xb[h * hs..(h + 1) * hs];
                xb.fill(0.0);
                for (t, &a) in att.iter().enumerate() {
                    let v = &s.value_cache[loff + t * kv_dim + kv_off..][..hs];
                    for (o, &vi) in xb.iter_mut().zip(v) {
                        *o += a * vi;
                    }
                }
            }
        }

        s.xq.quantize(&s.xb, gs_dim, s.mode.scales);
        matmul(&mut s.xb2, &s.xq, &model.wo.rows(l * dim, dim), s.mode, &mut s.sums);
        for (x, &d) in s.x.iter_mut().zip(&s.xb2) {
            *x += d;
        }

        // Feed-forward: w2(silu(w1 x) * w3 x).
        rmsnorm(&mut s.xb, &s.x, &model.rms_ffn[l * dim..(l + 1) * dim]);
        s.xq.quantize(&s.xb, gs_dim, s.mode.scales);
        matmul(&mut s.hb, &s.xq, &model.w1.rows(l * hidden, hidden), s.mode, &mut s.sums);
        matmul(&mut s.hb2, &s.xq, &model.w3.rows(l * hidden, hidden), s.mode, &mut s.sums);
        for (h, &h2) in s.hb.iter_mut().zip(&s.hb2) {
            let mut val = *h;
            val *= 1.0 / (1.0 + (s.exp)(-val));
            *h = val * h2;
        }
        s.hq.quantize(&s.hb, gs_hidden, s.mode.scales);
        matmul(&mut s.xb, &s.hq, &model.w2.rows(l * dim, dim), s.mode, &mut s.sums);
        for (x, &d) in s.x.iter_mut().zip(&s.xb) {
            *x += d;
        }
    }

    let x = s.x.clone();
    rmsnorm(&mut s.x, &x, &model.rms_final);
    s.xq.quantize(&s.x, gs_dim, s.mode.scales);
    matmul(&mut s.logits, &s.xq, &model.wcls().all(), s.mode, &mut s.sums);
}

#[cfg(test)]
mod fixed_tests {
    use super::{ldexp, to_block};

    #[test]
    fn block_floating_point_round_trips_within_2_to_minus_14_of_the_largest() {
        let v = [3.75f32, -0.001, 0.0, 1e-30, -2.5, 0.125, 7.9, -7.99];
        let mut ints = [0i32; 8];
        let e = to_block(&v, &mut ints);
        let max = v.iter().fold(0.0f32, |m, &x| m.max(x.abs()));
        for (&x, &n) in v.iter().zip(&ints) {
            assert!(n.abs() < 1 << 15);
            let back = n as f32 * 2f32.powi(e);
            assert!((back - x).abs() <= max * 2f32.powi(-14), "{x} -> {back}");
        }
    }

    #[test]
    fn ldexp_scales_by_powers_of_two() {
        assert_eq!(ldexp(3.0, 4), 48.0);
        assert_eq!(ldexp(-3.0, -2), -0.75);
        assert_eq!(ldexp(0.0, 10), 0.0);
        assert_eq!(ldexp(1.0, -200), 0.0);
        assert_eq!(ldexp(1.0, 200), f32::INFINITY);
    }
}

#[cfg(test)]
mod matmul_tests {
    use core::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    static CALLS: AtomicUsize = AtomicUsize::new(0);

    /// [`matrix_soft`], counting the calls.
    fn counted(
        w: &[u8],
        wf: Format,
        x: &[u8],
        xf: Format,
        rows: usize,
        cols: usize,
        out: &mut [i64],
    ) {
        CALLS.fetch_add(1, Ordering::Relaxed);
        matrix_soft(w, wf, x, xf, rows, cols, out);
    }

    #[test]
    fn matrix_calls_give_the_row_by_row_result() {
        // One more row than one call takes, so the matrix is split in two calls.
        let (cols, f) = (16, Format::E4m3);
        let rows = DOT_F8_MAX_N as usize / cols + 1;
        let mut seed = 7u32;
        let q: Vec<u8> = (0..rows * cols)
            .map(|_| {
                seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
                (seed >> 16) as u8 & 0x7e // no NaN codes
            })
            .collect();
        let (scales, fixed_scales) = (vec![0.25f32; rows], vec![3i32; rows]);
        let row_shift = vec![2i32; rows];
        let w = QMatrixView {
            rows,
            cols,
            gs: cols,
            format: f,
            q: &q,
            scales: &scales,
            fixed_scales: &fixed_scales,
            row_shift: &row_shift,
        };
        let xs: Vec<f32> = (0..cols).map(|i| i as f32 * 0.37 - 2.0).collect();
        let mut sums = vec![0i64; rows];
        for s in [Scales::Float, Scales::Fixed] {
            let mut x = QVec::new(cols, f);
            x.quantize(&xs, cols, s);
            let (mut by_row, mut by_matrix) = (vec![0f32; rows], vec![0f32; rows]);
            matmul(&mut by_row, &x, &w, MatmulMode::soft(s), &mut sums);
            let mode = MatmulMode { matrix: Some(counted), ..MatmulMode::soft(s) };
            let before = CALLS.load(Ordering::Relaxed);
            matmul(&mut by_matrix, &x, &w, mode, &mut sums);
            assert_eq!(CALLS.load(Ordering::Relaxed) - before, 2);
            assert!(by_row.iter().zip(&by_matrix).all(|(a, b)| a.to_bits() == b.to_bits()));
            assert!(by_row.iter().any(|&v| v != 0.0));
        }
    }
}

#[cfg(test)]
mod f8_tests {
    use super::{e4m3_at, encode_pow2, ldexp};
    use crate::formats::Format;

    #[test]
    fn e4m3_at_is_encode_float_of_the_scaled_value() {
        let mut seed = 1u32;
        for _ in 0..200_000 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let x = f32::from_bits(seed & 0x80ff_ffff | 0x3c00_0000); // ±[2^-7, 2)
            let e = ((x.to_bits() >> 23) & 0xff) as i32;
            for f in -6..16 {
                let want = Format::E4m3.encode_float(ldexp(x, f - (e - 120)).to_bits());
                assert_eq!(e4m3_at(x.to_bits(), f), want, "{x} at {f}");
            }
        }
    }

    #[test]
    fn encode_pow2_round_trips_within_one_part_in_16() {
        let v = [3.75f32, -0.001, 0.0, 1e-30, -2.5, 0.125, 7.9, -7.99];
        let mut q = [0u8; 8];
        let e = encode_pow2(&v, &mut q);
        for (i, &x) in v.iter().enumerate() {
            let back = Format::E4m3.units(&q, i) as f32 * 2f32.powi(e);
            assert!((back - x).abs() <= x.abs() / 16.0 + 7.99 * 2f32.powi(-15), "{x} -> {back}");
        }
    }
}
