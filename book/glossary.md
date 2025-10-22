# Glossary

This glossary provides comprehensive definitions and references for ZisK terminology, commands, and technical concepts.

## Commands

### cargo-zisk Commands

| Command | Description | Key Flags |
|---------|-------------|-----------|
| `build` | Compile Rust program to ELF | `--release` |
| `run` | Build and execute in emulator | `-i`, `-m`, `-x` |
| `rom-setup` | Generate program setup files | `-e`, `-k` |
| `verify-constraints` | Verify all constraints | `-e`, `-i`, `-w`, `-k` |
| `prove` | Generate cryptographic proof | `-e`, `-i`, `-w`, `-k`, `-o`, `-a`, `-y` |
| `verify` | Verify generated proof | `-p`, `-s`, `-e`, `-k` |
| `clean` | Clean cache directory | |

### Common Flags

| Flag | Long Form | Description |
|------|-----------|-------------|
| `-e` | `--elf` | ELF file location |
| `-i` | `--input` | Input file location |
| `-w` | `--witness` | Witness library location |
| `-k` | `--proving-key` | Proving key directory |
| `-o` | `--output` | Output directory |
| `-a` | `--aggregation` | Generate final aggregated proof |
| `-y` | `--verify-proofs` | Verify proof after generation |
| `-m` | `--log-metrics` | Show performance metrics |
| `-x` | `--stats` | Show execution statistics |
| `-n` | `--max-steps` | Maximum execution steps |
| `-p` | `--proof` | Proof file location |
| `-s` | `--starkinfo` | STARK info file |
| `--release` | | Build in release mode |

