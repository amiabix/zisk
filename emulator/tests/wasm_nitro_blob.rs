//! Authenticate complete EIP-4844 blobs before either Wasm preimage reader sees bytes.
use c_kzg::{ethereum_kzg_settings, Blob, BYTES_PER_BLOB};
use sha2::{Digest, Sha256};
use std::sync::OnceLock;
use zisk_common::EmuTrace;
use zisk_transpiler_wasm::nitro;
use ziskemu::{Emu, EmuOptions, FailureReason, ZiskEmulator};

struct Fixture {
    key: Vec<u8>,
    value: Vec<u8>,
}

fn decode_hex<const N: usize>(value: &str) -> [u8; N] {
    assert_eq!(value.len(), N * 2);
    std::array::from_fn(|i| u8::from_str_radix(&value[i * 2..i * 2 + 2], 16).unwrap())
}

fn fixture() -> &'static Fixture {
    static FIXTURE: OnceLock<Fixture> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let mut blob = vec![0; BYTES_PER_BLOB];
        for (i, element) in blob.chunks_exact_mut(32).enumerate() {
            element.copy_from_slice(&Sha256::digest((i as u64).to_le_bytes()));
            element[0] = 0; // Canonical scalars, with nonzero data throughout the blob.
        }
        make_fixture(blob)
    })
}

fn make_fixture(mut value: Vec<u8>) -> Fixture {
    let settings = ethereum_kzg_settings(0);
    let blob = Blob::from_bytes(&value).unwrap();
    let commitment = settings.blob_to_kzg_commitment(&blob).unwrap().to_bytes();
    let proof = settings.compute_blob_kzg_proof(&blob, &commitment).unwrap().to_bytes();
    assert!(settings.verify_blob_kzg_proof(&blob, &commitment, &proof).unwrap());
    let mut key = Sha256::digest(commitment.as_ref()).to_vec();
    key[0] = 1;
    value.extend_from_slice(commitment.as_ref());
    value.extend_from_slice(proof.as_ref());
    Fixture { key, value }
}

fn witness(key: &[u8], value: &[u8]) -> Vec<u8> {
    let mut payload = b"NZKWIT01".to_vec();
    payload.extend_from_slice(&[0; 160]);
    payload.extend_from_slice(&1u32.to_le_bytes());
    payload.extend_from_slice(&[0; 8]);
    payload.push(2);
    payload.extend_from_slice(key);
    payload.extend_from_slice(&(value.len() as u32).to_le_bytes());
    payload.extend_from_slice(value);
    let mut input = (payload.len() as u64).to_le_bytes().to_vec();
    input.extend(payload);
    input.resize(input.len().next_multiple_of(8), 0);
    input
}

fn execute(key: &[u8], value: &[u8]) -> Result<Vec<u8>, String> {
    execute_with_forgery(key, value, None)
}

fn execute_with_forgery(
    key: &[u8],
    value: &[u8],
    forged_fcall: Option<u64>,
) -> Result<Vec<u8>, String> {
    let key_bytes: String = key.iter().map(|b| format!("\\{b:02x}")).collect();
    let wasm = wat::parse_str(format!(r#"(module
      (import "wavmio" "resolveTypedPreimage" (func $read (param i32 i32 i32 i32) (result i32)))
      (import "wavmio" "greedyResolveTypedPreimage" (func $greedy (param i32 i32 i32 i32 i32) (result i32)))
      (import "wavmio" "setGlobalStateBytes32" (func $set (param i32 i32)))
      (memory 1)
      (data (i32.const 129) "{key_bytes}")
      (func $assert (param $x i32) (if (i32.eqz (local.get $x)) (then unreachable)))
      (func (export "_start")
        (call $assert (i32.eq (call $read (i32.const 2) (i32.const 129) (i32.const 0) (i32.const 256)) (i32.const 32)))
        (call $set (i32.const 0) (i32.const 256))
        (call $assert (i32.eq (call $read (i32.const 2) (i32.const 129) (i32.const 131040) (i32.const 256)) (i32.const 32)))
        (call $set (i32.const 1) (i32.const 256))
        (call $assert (i32.eqz (call $read (i32.const 2) (i32.const 129) (i32.const 131072) (i32.const 65536))))
        (i32.store8 (i32.const 519) (i32.const 90))
        (call $assert (i32.eq (call $greedy (i32.const 2) (i32.const 129) (i32.const 131065) (i32.const 64) (i32.const 512)) (i32.const 7)))
        (call $assert (i32.eq (i32.load8_u (i32.const 519)) (i32.const 90)))
        (call $assert (i64.eq (i64.load (i32.const 512))
          (i64.or (i64.and (i64.load (i32.const 281)) (i64.const 0x00ffffffffffffff)) (i64.const 0x5a00000000000000))))
      ))"#)).unwrap();
    let path = std::env::var_os("NITRO_RUNTIME_ELF")
        .unwrap_or_else(|| "../target/nitro-runtime/nitro-runtime.elf".into());
    let elf = std::fs::read(path).expect("run nitro_tools build first");
    let rom = nitro::wasm2rom(&wasm, &elf).map_err(|e| e.to_string())?;
    let options = EmuOptions { max_steps: 150_000_000, ..Default::default() };
    if let Some(function) = forged_fcall {
        let mut emu = Emu::new(&rom);
        emu.ctx = emu.create_emu_context(witness(key, value), &options);
        let mut injected = false;
        while !emu.terminated() && emu.number_of_steps() < options.max_steps {
            let fcall = rom.get_instruction(emu.ctx.inst_ctx.pc).op_str == "fcall";
            emu.step_fast();
            if fcall && emu.ctx.inst_ctx.a == function && !injected {
                let ctx = &mut emu.ctx.inst_ctx;
                ctx.fcall.result[..ctx.fcall.result_size as usize].fill(0);
                ctx.mem.free_input = 0;
                injected = true;
            }
        }
        assert!(injected, "did not reach fcall {function}");
        assert_eq!(emu.failure().map(|(reason, _, _)| reason), Some(FailureReason::Trap));
        return Ok(Vec::new());
    }
    ZiskEmulator::process_rom(&rom, &witness(key, value), &options, None::<fn(EmuTrace)>)
        .map_err(|e| e.to_string())
}

#[test]
#[ignore = "requires built native runtime ELF and float feature"]
fn forged_pairing_line_hints_trap_during_blob_verification() {
    let f = fixture();
    for function in [14, 15] {
        execute_with_forgery(&f.key, &f.value, Some(function)).unwrap();
    }
}

#[test]
#[ignore = "requires LLVM RISC-V compiler and float feature"]
fn polynomial_evaluation_matches_ckzg_including_domain_points() {
    use std::{fs, path::Path, process::Command};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let out = root.join("target/nitro-blob-evaluation");
    fs::create_dir_all(&out).unwrap();
    // Compile the production evaluator with a test entry point; no test import enters the runtime.
    fs::write(
        out.join("evaluate.c"),
        r#"
        #include "blob.c"
        static Fr result;
        _Noreturn void runtime_trap(void) { __asm__ volatile("ebreak"); __builtin_unreachable(); }
        uint64_t nitro_dispatch(uint64_t op, const uint64_t *args, uint64_t size) {
            (void)size;
            uint8_t *p = (uint8_t *)(uintptr_t)args[0];
            if (op == 0) {
                if (args[1] != 32 + NITRO_BLOB_BYTES) runtime_trap();
                result = evaluate(p + 32, from_bytes(p));
            } else if (op == 1) {
                for (unsigned i = 0; i < 256; ++i) p[i] = 0;
                to_bytes(&result, p);
            } else runtime_trap();
            return 0;
        }
        void *memcpy(void *dst, const void *src, size_t n) {
            for (size_t i = 0; i < n; ++i) ((uint8_t *)dst)[i] = ((const uint8_t *)src)[i];
            return dst;
        }
        int memcmp(const void *a, const void *b, size_t n) {
            for (size_t i = 0; i < n; ++i)
                if (((const uint8_t *)a)[i] != ((const uint8_t *)b)[i]) return 1;
            return 0;
        }
    "#,
    )
    .unwrap();
    let cc = std::env::var("NITRO_CC").unwrap_or_else(|_| {
        if Path::new("/opt/homebrew/opt/llvm/bin/clang").exists() {
            "/opt/homebrew/opt/llvm/bin/clang".into()
        } else {
            "clang".into()
        }
    });
    let ld = std::env::var("NITRO_LD").unwrap_or_else(|_| {
        if Path::new("/opt/homebrew/opt/llvm/bin/ld.lld").exists() {
            "/opt/homebrew/opt/llvm/bin/ld.lld".into()
        } else {
            "ld.lld".into()
        }
    });
    let compile = Command::new(cc)
        .args([
            "--target=riscv64-unknown-elf",
            "-march=rv64imfd",
            "-mabi=lp64",
            "-mcmodel=medany",
            "-mno-relax",
            "-msmall-data-limit=0",
            "-O2",
            "-ffreestanding",
            "-fno-builtin",
            "-ffunction-sections",
            "-fdata-sections",
            "-I",
        ])
        .arg(root.join("transpilers/wasm/runtime"))
        .arg("-I")
        .arg(root.join("ziskasm/lang/c/include"))
        .arg("-c")
        .arg(out.join("evaluate.c"))
        .arg("-o")
        .arg(out.join("evaluate.o"))
        .output()
        .unwrap();
    assert!(compile.status.success(), "{}", String::from_utf8_lossy(&compile.stderr));
    let link = Command::new(ld)
        .args(["--no-relax", "--gc-sections", "-T"])
        .arg(root.join("transpilers/wasm/runtime/runtime.ld"))
        .arg(out.join("evaluate.o"))
        .arg("-o")
        .arg(out.join("evaluate.elf"))
        .output()
        .unwrap();
    assert!(link.status.success(), "{}", String::from_utf8_lossy(&link.stderr));
    let wasm = wat::parse_str("(module (memory 1) (func (export \"_start\")))").unwrap();
    let rom = nitro::wasm2rom(&wasm, &fs::read(out.join("evaluate.elf")).unwrap()).unwrap();
    let blob = Blob::from_bytes(&fixture().value[..BYTES_PER_BLOB]).unwrap();
    let mut points = vec![[0; 32], [0; 32]];
    points[1][31] = 1;
    let modulus_minus_one = "73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000000";
    let root_of_unity = "564c0a11a0f704f4fc3e8acfe0f8245f0ad1347b378fbf96e206da11a5d36306";
    for hex in [modulus_minus_one, root_of_unity] {
        points.push(decode_hex(hex));
    }
    for point in points {
        let (_, expected) =
            ethereum_kzg_settings(0).compute_kzg_proof(&blob, &c_kzg::Bytes32::new(point)).unwrap();
        let mut payload = point.to_vec();
        payload.extend_from_slice(blob.as_ref());
        let mut input = (payload.len() as u64).to_le_bytes().to_vec();
        input.extend(payload);
        let options = EmuOptions { max_steps: 30_000_000, ..Default::default() };
        let output =
            ZiskEmulator::process_rom(&rom, &input, &options, None::<fn(EmuTrace)>).unwrap();
        assert_eq!(&output[..32], expected.as_ref());
        // An early return at a domain point must not skip scalar validation elsewhere.
        input[8 + 32 + 65536..8 + 32 + 65568].fill(255);
        let error =
            ZiskEmulator::process_rom(&rom, &input, &options, None::<fn(EmuTrace)>).unwrap_err();
        assert!(error.to_string().to_lowercase().contains("trap"));
    }
}

#[test]
#[ignore = "requires built native runtime ELF and float feature"]
fn complete_blob_is_verified_before_chunked_and_greedy_reads() {
    let mut boundary = vec![0; BYTES_PER_BLOB];
    boundary[..32].copy_from_slice(&decode_hex::<32>(
        "73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000000",
    ));
    for f in [fixture(), &make_fixture(vec![0; BYTES_PER_BLOB]), &make_fixture(boundary)] {
        let result = execute(&f.key, &f.value).unwrap();
        assert_eq!(result.len(), 256);
        assert_eq!(&result[112..144], &f.value[..32]);
        assert_eq!(&result[144..176], &f.value[BYTES_PER_BLOB - 32..BYTES_PER_BLOB]);
    }
}

#[test]
#[ignore = "requires built native runtime ELF and float feature"]
fn forged_blob_commitment_proof_and_versioned_hash_trap() {
    let f = fixture();
    // Includes a byte that neither preimage reader requests.
    for index in [31, 65567, BYTES_PER_BLOB - 1, BYTES_PER_BLOB + 47, BYTES_PER_BLOB + 95] {
        let mut value = f.value.clone();
        value[index] ^= 1;
        let error = execute(&f.key, &value).unwrap_err();
        assert!(error.to_lowercase().contains("trap"), "byte {index}: {error}");
    }
    for index in [0, 31] {
        let mut key = f.key.clone();
        key[index] ^= 1;
        let error = execute(&key, &f.value).unwrap_err();
        assert!(error.to_lowercase().contains("trap"), "key byte {index}: {error}");
    }
}

#[test]
#[ignore = "requires built native runtime ELF and float feature"]
fn malformed_blob_lengths_scalars_and_points_trap() {
    let f = fixture();
    for len in [0, BYTES_PER_BLOB, f.value.len() - 1, f.value.len() + 1] {
        let mut value = f.value.clone();
        value.resize(len, 0);
        let error = execute(&f.key, &value).unwrap_err();
        assert!(error.to_lowercase().contains("trap"), "length {len}: {error}");
    }
    let mut value = f.value.clone();
    value[65536..65568].fill(255);
    assert!(execute(&f.key, &value).unwrap_err().to_lowercase().contains("trap"));
    value[65536..65568].copy_from_slice(&decode_hex::<32>(
        "73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001",
    ));
    assert!(execute(&f.key, &value).unwrap_err().to_lowercase().contains("trap"));
    let mut outside_subgroup = [0; 48];
    outside_subgroup[0] = 0x80; // (0, 2) is on the curve, outside the prime-order subgroup.
    let mut malformed_infinity = [0; 48];
    malformed_infinity[0] = 0xc0;
    malformed_infinity[47] = 1;
    for point in [[0; 48], outside_subgroup, malformed_infinity] {
        for offset in [BYTES_PER_BLOB, BYTES_PER_BLOB + 48] {
            let mut value = f.value.clone();
            value[offset..offset + 48].copy_from_slice(&point);
            // Keep the commitment hash correct so point validation must reject the input.
            let mut key = Sha256::digest(&value[BYTES_PER_BLOB..BYTES_PER_BLOB + 48]).to_vec();
            key[0] = 1;
            assert!(execute(&key, &value).unwrap_err().to_lowercase().contains("trap"));
        }
    }
}
