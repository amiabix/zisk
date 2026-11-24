//! Run an explicitly compiled Nitro adapter against a separately checked expected claim.
use clap::Parser;
use std::{error::Error, fs, path::PathBuf, time::Instant};
use zisk_common::EmuTrace;
use ziskemu::{Emu, EmuOptions};

#[derive(Parser)]
struct Args {
    wasm: PathBuf,
    runtime: PathBuf,
    witness: PathBuf,
    expected: PathBuf,
    actual: PathBuf,
    /// Export the exact linked program, including the selected optimizations.
    program: Option<PathBuf>,
    /// Disable ROM post-pass optimizations for a comparison on identical inputs.
    #[arg(long)]
    no_optimize: bool,
    /// Collect the same cost statistics as ziskemu -X and save a CSV snapshot.
    #[arg(long)]
    save_stats: Option<String>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let wasm = fs::read(&args.wasm)?;
    let runtime = fs::read(&args.runtime)?;
    let stdin = fs::read(&args.witness)?;
    let expected = fs::read(&args.expected)?;
    if expected.len() != 256 {
        return Err("expected claim must be exactly 256 bytes".into());
    }
    let start = Instant::now();
    eprintln!("Compiling Nitro and native runtime ({} Wasm bytes)", wasm.len());
    let passes = if args.no_optimize {
        zisk_riscv::RomOptPasses::NONE
    } else {
        zisk_riscv::RomOptPasses::default()
    };
    eprintln!("ROM passes: {passes:?}");
    let rom = zisk_transpiler_wasm::nitro::wasm2rom_with_optimizations(&wasm, &runtime, passes)?;
    eprintln!("Compiled {} instructions in {:?}; executing", rom.insts.len(), start.elapsed());
    let options = EmuOptions {
        max_steps: 3_000_000_000,
        log_metrics: true,
        stats: args.save_stats.is_some(),
        save_stats: args.save_stats,
        ..Default::default()
    };
    let mut emu = Emu::new(&rom);
    emu.run(stdin, &options, None::<fn(EmuTrace)>);
    if let Some((reason, step, pc)) = emu.failure() {
        return Err(format!("execution failed: {reason:?}, step={step}, pc={pc:#x}, native return={:#x}, SP={:#x}, args={:x?}", emu.get_reg(1), emu.get_reg(2), (0..8).map(|i| emu.ctx.inst_ctx.mem.read(0xa0540200 + i * 8, 8)).collect::<Vec<_>>()).into());
    }
    if !emu.terminated() {
        return Err("execution exceeded the step limit".into());
    }
    let output = emu.get_output_8();
    eprintln!("Execution steps: {}", emu.number_of_steps());
    fs::write(&args.actual, &output)?;
    if output != expected {
        let first = output.iter().zip(&expected).position(|(a, b)| a != b);
        return Err(format!(
            "Nitro public claim mismatch at byte {first:?}; actual saved to {}",
            args.actual.display()
        )
        .into());
    }
    eprintln!("PASS: all 256 public bytes match; total {:?}", start.elapsed());
    if let Some(path) = args.program {
        fs::write(&path, zisk_core::ziskbin::rom_to_elf(&rom))?;
        eprintln!("Saved exact linked program to {}", path.display());
    }
    Ok(())
}
