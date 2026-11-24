//! Native Nitro state/Brotli integration; run nitro_tools build first.
use zisk_common::EmuTrace;
use zisk_riscv::RomOptPasses;
use zisk_transpiler_wasm::nitro;
use ziskemu::{Emu, EmuOptions};

fn witness() -> Vec<u8> {
    let mut payload = b"NZKWIT01".to_vec();
    payload.extend_from_slice(&[0; 172]);
    let mut input = (payload.len() as u64).to_le_bytes().to_vec();
    input.extend(payload);
    input.resize(input.len().next_multiple_of(8), 0);
    input
}

#[test]
#[ignore = "requires built native runtime ELF and float feature"]
fn native_state_initializes_and_finalizes_after_console_output() {
    let elf = std::fs::read("../target/nitro-runtime/nitro-runtime.elf").unwrap();
    let wasm = wat::parse_str(r#"(module
      (import "wavmio" "setGlobalStateU64" (func $set (param i32 i64)))
      (import "wasi_snapshot_preview1" "fd_write" (func $write (param i32 i32 i32 i32) (result i32)))
      (memory 1)
      (data (i32.const 32) "console only")
      (func (export "_start")
        (call $set (i32.const 0) (i64.const 5))
        (i32.store (i32.const 0) (i32.const 32))
        (i32.store (i32.const 4) (i32.const 12))
        (drop (call $write (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 16)))))"#).unwrap();
    let options = EmuOptions { max_steps: 30_000_000, ..Default::default() };
    let mut results = Vec::new();
    for passes in [RomOptPasses::NONE, RomOptPasses::default()] {
        let rom = nitro::wasm2rom_with_optimizations(&wasm, &elf, passes).unwrap();
        let mut emu = Emu::new(&rom);
        emu.run(witness(), &options, None::<fn(EmuTrace)>);
        assert!(emu.failure().is_none(), "{:?}", emu.failure());
        assert!(emu.terminated());
        let output = emu.get_output_8();
        assert_eq!(output.len(), 256);
        assert_eq!(&output[32..112], &[0; 80]);
        assert_eq!(&output[112..176], &[0; 64]);
        assert_eq!(&output[176..184], &5u64.to_le_bytes());
        assert_eq!(&output[184..192], &[0; 8]);
        assert_ne!(&output[..12], b"console only");
        results.push((output, emu.number_of_steps()));

        // A malformed witness must still trap after the native code is optimized.
        let mut invalid = witness();
        invalid[8] ^= 1;
        let mut emu = Emu::new(&rom);
        emu.run(invalid, &options, None::<fn(EmuTrace)>);
        assert!(emu.failure().is_some(), "malformed witness accepted with {passes:?}");
    }
    assert_eq!(results[0].0, results[1].0);
    assert!(results[1].1 < results[0].1, "optimizer did not reduce native replay steps");
}
