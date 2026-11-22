//! Asks for a prompt, generates a continuation by running the llama2 guest
//! on ziskemu, and prints it. Also runs the model natively and reports
//! whether both runs produced the same tokens.
//!
//! ```text
//! cargo run --release --bin llama2-zisk -- [OPTIONS] [--ziskemu PATH] [--save-input PATH]
//!     [--dot-product soft|precompile] [--f32-dot loops|floatdot] [--trace off|on]
//!     [--weights int8|f32]
//! ```
//!
//! `--save-input PATH` writes the guest input for the first prompt and exits.
//!
//! `--dot-product precompile` makes the guest use the experimental `dot_i8`
//! precompile, which needs a `ziskemu` built from this repository (see
//! `--ziskemu`) and cannot be proven yet. The default, `soft`, is plain Rust.
//!
//! `--f32-dot floatdot` computes the `f32` dot products of RMSNorm and
//! attention with the FloatDot precompile, which needs a `ziskemu` built from
//! this repository. `--trace on` makes the guest also commit the digest of
//! every such dot product result and every logit, checked against native.
//!
//! `--weights f32` runs the checkpoint's `f32` weights instead of the int8
//! model (`llama2_common::forward_f32`). Then `--f32-dot floatdot` makes every
//! dot product use FloatDot, the matrix multiplications included.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use llama2_common::forward::{MatmulMode, Scales, dot_soft};
use llama2_common::forward_f32::{ModelF32, dot_words_soft, generate_f32};
use llama2_common::generate::generate_traced;
use llama2_common::model::Model;
use llama2_common::params::GenParams;
use llama2_host::{
    Result, TextPrinter, gen_params, load, output_digest, parse_options, print_model_info, prompts,
    trace_digest,
};
use sha2::{Digest, Sha256};

/// Guest ELF, built by `build.rs`.
const GUEST_ELF: &str = env!("ZISK_ELF_llama2-guest");

/// Appends one stdin record: a `u64` length, then the data padded to 8 bytes.
fn push_record(input: &mut Vec<u8>, data: &[u8]) {
    input.extend_from_slice(&(data.len() as u64).to_le_bytes());
    input.extend_from_slice(data);
    input.resize(input.len().next_multiple_of(8), 0);
}

struct EmuRun {
    tokens: Vec<u32>,
    committed: Vec<u8>,
    steps: Option<u64>,
    secs: f64,
}

/// Guest option flags (see the guest): use the `dot_i8` precompile, use
/// fixed-point scales, use the FloatDot precompile, commit the trace, and run
/// the `f32` model.
const USE_DOT_I8: u64 = 1;
const FIXED_SCALES: u64 = 2;
const USE_FLOATDOT: u64 = 4;
const TRACE: u64 = 8;
const F32_MODEL: u64 = 16;

/// The guest's input file: the model blob, the generation parameters, then
/// the guest option flags.
fn guest_input(blob: &[u8], params: &GenParams, flags: u64) -> Vec<u8> {
    let mut input = Vec::with_capacity(blob.len() + 4096);
    push_record(&mut input, blob);
    push_record(&mut input, &params.to_bytes());
    push_record(&mut input, &flags.to_le_bytes());
    input
}

fn run_ziskemu(
    ziskemu: &str,
    blob: &[u8],
    params: &GenParams,
    flags: u64,
    dir: &Path,
) -> Result<EmuRun> {
    let input_path = dir.join("input.bin");
    let output_path = dir.join("output.bin");
    std::fs::write(&input_path, guest_input(blob, params, flags))?;

    let start = Instant::now();
    let out = Command::new(ziskemu)
        .arg("-e")
        .arg(GUEST_ELF)
        .arg("-i")
        .arg(&input_path)
        .arg("-o")
        .arg(&output_path)
        .arg("-m")
        .output()
        .map_err(|e| format!("cannot run {ziskemu}: {e} (is ziskemu installed? see --ziskemu)"))?;
    let secs = start.elapsed().as_secs_f64();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    if !out.status.success() {
        return Err(format!("ziskemu failed ({}):\n{stdout}\n{stderr}", out.status).into());
    }

    let line = stdout
        .lines()
        .find_map(|l| l.strip_prefix("tokens:"))
        .ok_or_else(|| format!("the guest printed no tokens:\n{stdout}\n{stderr}"))?;
    let tokens = line.split_whitespace().map(str::parse).collect::<std::result::Result<_, _>>()?;
    let steps = parse_steps(&stdout).or_else(|| parse_steps(&stderr));
    let committed = std::fs::read(&output_path)?;
    Ok(EmuRun { tokens, committed, steps, secs })
}

/// Finds the step count in ziskemu's `-m` metrics.
fn parse_steps(text: &str) -> Option<u64> {
    text.lines().find_map(|l| {
        let l = l.to_lowercase();
        let rest = &l[l.find("steps")? + 5..];
        let digits: String = rest
            .chars()
            .skip_while(|c| !c.is_ascii_digit())
            .take_while(|c| c.is_ascii_digit() || *c == ',' || *c == '_')
            .filter(char::is_ascii_digit)
            .collect();
        digits.parse().ok()
    })
}

fn main() -> Result<()> {
    let o = parse_options(&[
        "--ziskemu",
        "--save-input",
        "--dot-product",
        "--f32-dot",
        "--trace",
        "--weights",
    ])?;
    let extra = |name: &str| o.extra.iter().find(|(n, _)| n == name).map(|(_, v)| v.clone());
    let ziskemu = extra("--ziskemu").unwrap_or_else(|| "ziskemu".to_string());
    let mut flags = match extra("--dot-product").as_deref() {
        None | Some("soft") => 0,
        Some("precompile") => USE_DOT_I8,
        Some(v) => return Err(format!("--dot-product must be soft or precompile, not {v}").into()),
    };
    if o.scales == Scales::Fixed {
        flags |= FIXED_SCALES;
    }
    match extra("--f32-dot").as_deref() {
        None | Some("loops") => {}
        Some("floatdot") => flags |= USE_FLOATDOT,
        Some(v) => return Err(format!("--f32-dot must be loops or floatdot, not {v}").into()),
    }
    match extra("--trace").as_deref() {
        None | Some("off") => {}
        Some("on") => flags |= TRACE,
        Some(v) => return Err(format!("--trace must be off or on, not {v}").into()),
    }
    match extra("--weights").as_deref() {
        None | Some("int8") => {}
        Some("f32") if flags & (USE_DOT_I8 | FIXED_SCALES) == 0 => flags |= F32_MODEL,
        Some("f32") => return Err("--weights f32 takes neither --dot-product nor --scales".into()),
        Some(v) => return Err(format!("--weights must be int8 or f32, not {v}").into()),
    }
    let l = load(&o)?;
    // The guest's model input: the int8 blob, or the checkpoint itself.
    let model_input = if flags & F32_MODEL != 0 { &l.checkpoint } else { &l.blob };

    // Write the guest input for the first prompt and exit, for running the
    // guest with cargo-zisk or ziskemu directly.
    if let Some(path) = extra("--save-input") {
        let text = prompts(&o).next().unwrap_or_default();
        let params = gen_params(&o, &l.tokenizer, &text);
        std::fs::write(&path, guest_input(model_input, &params, flags))?;
        println!("Saved guest input to {path}");
        return Ok(());
    }

    print_model_info(&o, &l);
    println!("Guest ELF {GUEST_ELF}");
    if flags & USE_DOT_I8 != 0 {
        println!("Dot products: dot_i8 precompile (experimental, emulator only)");
    }
    if o.scales == Scales::Fixed {
        println!("Scales: fixed point");
    }
    if flags & F32_MODEL != 0 {
        println!("Weights: f32");
    }
    if flags & USE_FLOATDOT != 0 {
        println!("f32 dot products: FloatDot precompile");
    }
    let model = Model::parse(&l.blob);
    let dir: PathBuf = std::env::temp_dir().join(format!("llama2-zisk-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;

    for text in prompts(&o) {
        let params = gen_params(&o, &l.tokenizer, &text);
        println!("[running on ziskemu...]");
        let run = match run_ziskemu(&ziskemu, model_input, &params, flags, &dir) {
            Ok(run) => run,
            Err(e) => {
                std::fs::remove_dir_all(&dir).ok();
                return Err(e);
            }
        };

        let mut printer = TextPrinter::start(&l.tokenizer, &text, &params.prompt);
        for &t in &run.tokens {
            printer.token(t);
        }
        println!();

        let digest_ok = run.committed.get(..32) == Some(&output_digest(&params, &run.tokens)[..]);
        let mode = MatmulMode { dot: dot_soft, scales: o.scales, f32_dot: None };
        let (native, trace) = if flags & F32_MODEL != 0 {
            generate_f32(&ModelF32::parse(&l.checkpoint), &params, dot_words_soft, true, |_| {})
        } else {
            generate_traced(&model, &params, mode, true, |_| {})
        };
        let steps = run.steps.map_or_else(|| "? steps".to_string(), |s| format!("{s} steps"));
        println!(
            "[ziskemu: {} prompt + {} generated tokens, {steps}, {:.2} s]",
            params.prompt.len(),
            run.tokens.len(),
            run.secs
        );
        println!(
            "[committed hash matches printed tokens: {}; same tokens as native: {}]",
            if digest_ok { "yes" } else { "NO" },
            if native == run.tokens { "yes" } else { "NO" }
        );
        if flags & TRACE != 0 {
            let same = run.committed.get(32..64) == Some(&trace_digest(&trace)[..]);
            println!(
                "[committed trace digest same as native: {}]",
                if same { "yes" } else { "NO" }
            );
        }
        if flags & F32_MODEL != 0 {
            let at = if flags & TRACE != 0 { 64 } else { 32 };
            let same = run.committed.get(at..at + 32) == Some(&Sha256::digest(&l.checkpoint)[..]);
            println!(
                "[committed model digest same as the checkpoint's sha256: {}]",
                if same { "yes" } else { "NO" }
            );
        }
    }
    std::fs::remove_dir_all(&dir).ok();
    Ok(())
}
