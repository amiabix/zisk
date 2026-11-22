//! The generation loop and the sampler, following llama2.c's `generate`.

use alloc::vec::Vec;

use crate::BOS;
use crate::forward::{MatmulMode, RunState, Trace, forward, softmax};
use crate::model::Model;
use crate::params::GenParams;

/// llama2.c's xorshift random generator.
struct Rng(u64);

impl Rng {
    fn next_u32(&mut self) -> u32 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) as u32
    }

    /// A random float in `[0, 1)`.
    fn next_f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / 16_777_216.0
    }
}

/// Index of the largest value (the first one on ties).
pub fn argmax(v: &[f32]) -> u32 {
    let mut best = 0;
    for (i, &x) in v.iter().enumerate() {
        if x > v[best] {
            best = i;
        }
    }
    best as u32
}

fn sample(logits: &mut [f32], temperature: f32, rng: &mut Rng) -> u32 {
    if temperature == 0.0 {
        return argmax(logits);
    }
    for l in logits.iter_mut() {
        *l /= temperature;
    }
    softmax(logits);
    let coin = rng.next_f32();
    let mut cdf = 0.0f32;
    for (i, &p) in logits.iter().enumerate() {
        cdf += p;
        if coin < cdf {
            return i as u32;
        }
    }
    logits.len() as u32 - 1
}

/// Runs the prompt through the model, then generates up to `p.max_new` tokens.
/// Calls `on_token` with each generated token as soon as it is chosen, and
/// returns all of them. Generation stops early at a BOS token, which starts a
/// new story, or when the context window is full.
pub fn generate(model: &Model, p: &GenParams, on_token: impl FnMut(u32)) -> Vec<u32> {
    generate_with(model, p, MatmulMode::default(), on_token)
}

/// [`generate`], with `mode` choosing how the matrix multiplications are
/// computed.
pub fn generate_with(
    model: &Model,
    p: &GenParams,
    mode: MatmulMode,
    on_token: impl FnMut(u32),
) -> Vec<u32> {
    generate_traced(model, p, mode, false, on_token).0
}

/// [`generate_with`], also returning the [`Trace`] of every forward pass
/// when `trace` is set (empty otherwise).
pub fn generate_traced(
    model: &Model,
    p: &GenParams,
    mode: MatmulMode,
    trace: bool,
    on_token: impl FnMut(u32),
) -> (Vec<u32>, Vec<u32>) {
    struct Run<'a, 'b>(&'a Model<'b>, RunState);
    impl Forward for Run<'_, '_> {
        fn forward(&mut self, token: u32, pos: usize) -> &mut [f32] {
            forward(self.0, &mut self.1, token, pos);
            &mut self.1.logits
        }
    }
    let mut run = Run(model, RunState::with_mode(model, mode));
    run.1.trace = Trace(trace.then(Vec::new));
    let out = generate_steps(&mut run, model.config.seq_len, p, on_token);
    (out, run.1.trace.0.unwrap_or_default())
}

/// One forward pass of a model on `token` at position `pos`, which returns
/// the next-token logits.
pub trait Forward {
    fn forward(&mut self, token: u32, pos: usize) -> &mut [f32];
}

/// The generation loop of [`generate`], for any model with a context of
/// `seq_len` positions.
pub fn generate_steps(
    model: &mut impl Forward,
    seq_len: usize,
    p: &GenParams,
    mut on_token: impl FnMut(u32),
) -> Vec<u32> {
    assert!(!p.prompt.is_empty(), "the prompt must contain at least BOS");
    let mut rng = Rng(p.seed | 1);
    let mut out = Vec::new();

    let mut token = p.prompt[0];
    let mut pos = 0;
    while pos < seq_len && out.len() < p.max_new as usize {
        let logits = model.forward(token, pos);
        let next = if pos + 1 < p.prompt.len() {
            p.prompt[pos + 1]
        } else {
            let next = sample(logits, p.temperature, &mut rng);
            if next == BOS {
                break;
            }
            on_token(next);
            out.push(next);
            next
        };
        token = next;
        pos += 1;
    }
    out
}
