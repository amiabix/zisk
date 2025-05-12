# ZisK Glossary

This glossary provides definitions for key terms and concepts used throughout the ZisK documentation and codebase.

## Core Concepts

### Zero-Knowledge Proofs
- **STARK**: Scalable Transparent Argument of Knowledge - A type of zero-knowledge proof system used by ZisK
- **Proof**: Cryptographic evidence that a computation was performed correctly without revealing the inputs
- **Verification**: Process of checking the validity of a zero-knowledge proof
- **Witness**: The private inputs and intermediate states of a computation that are proven but not revealed

### RISC-V
- **ELF**: Executable and Linkable Format - The binary format used for RISC-V programs
- **ROM**: Read-Only Memory - Contains the program code and initial state
- **RAM**: Random Access Memory - Used for program data and stack
- **Instruction Set**: The set of operations that can be executed by the RISC-V processor

## System Components

### State Machines
- **Main State Machine**: Coordinates execution and manages program flow
- **Arithmetic State Machine**: Handles mathematical operations
- **Binary State Machine**: Manages bit-level operations
- **Memory State Machine**: Controls memory access and storage

### Compilation
- **LLVM IR**: Intermediate Representation used by the LLVM compiler
- **Transpilation**: Conversion of RISC-V code to ZisK operations
- **ROM Setup**: Process of preparing the program for execution in ZisK
- **Trace**: Record of program execution used for proof generation

## Development Tools

### Command Line Tools
- **cargo-zisk**: Main development tool for building and managing ZisK programs
- **ziskemu**: Emulator for testing programs before proof generation
- **rom-setup**: Tool for preparing ROM images
- **prove**: Tool for generating STARK proofs
- **verify**: Tool for verifying generated proofs

### Files and Formats
- **input.bin**: Binary file containing program input data
- **proof.json**: JSON file containing the generated STARK proof
- **publics.json**: JSON file containing public inputs and outputs
- **PIL**: Polynomial Identity Language - Used for defining constraints

## Technical Terms

### Memory
- **Heap**: Dynamic memory allocation area
- **Stack**: Memory area for function calls and local variables
- **Memory Bus**: Communication channel for memory operations
- **Memory Layout**: Organization of program memory

### Operations
- **Basic Operations**: Simple arithmetic and logical operations
- **Binary Operations**: Bit-level manipulations
- **Comparison Operations**: Operations for comparing values
- **Special Operations**: Custom operations specific to ZisK

### Proof System
- **Constraint System**: Set of mathematical constraints that define valid computations
- **Proof Generation**: Process of creating a STARK proof
- **Proof Verification**: Process of validating a STARK proof
- **Recursion**: Process of combining multiple proofs into a single proof

## Development Concepts

### Program Structure
- **Main Program**: Entry point of a ZisK program
- **Build Script**: Script for generating input data and preparing the program
- **Dependencies**: External libraries and components used by the program
- **Configuration**: Settings that control program behavior

### Testing and Debugging
- **Unit Tests**: Tests for individual components
- **Integration Tests**: Tests for component interactions
- **Emulation**: Testing program execution in a simulated environment
- **Trace Analysis**: Examination of execution traces for debugging

## Performance Terms

### Optimization
- **Precompiles**: Optimized implementations of common operations
- **GPU Acceleration**: Using GPU for faster proof generation
- **Concurrent Proof Generation**: Generating multiple proofs in parallel
- **Memory Optimization**: Techniques for efficient memory usage

### Metrics
- **Proof Size**: Size of the generated STARK proof
- **Verification Time**: Time required to verify a proof
- **Memory Usage**: Amount of memory used during execution
- **Execution Time**: Time required to run the program

## Security Terms

### Cryptography
- **Zero-Knowledge**: Property of not revealing private information
- **Soundness**: Property that false statements cannot be proven
- **Completeness**: Property that true statements can always be proven
- **Verification Key**: Public key used to verify proofs

### System Security
- **Memory Safety**: Protection against memory-related vulnerabilities
- **Input Validation**: Checking input data for correctness
- **State Integrity**: Ensuring program state remains valid
- **Proof Security**: Security properties of the proof system 