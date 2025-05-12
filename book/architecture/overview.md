# ZisK Architecture Overview

ZisK is a zero-knowledge virtual machine (zkVM) that enables verifiable computation of RISC-V programs. This document provides an overview of the system architecture.

## Core Components

### 1. Processor
- RISC-V instruction execution
- Program counter management
- Instruction decoding

### 2. Memory System
- ROM: Program code storage
- RAM: Data and stack management
- Memory bus for data transfer

### 3. State Machines
- Main state machine for coordination
- Arithmetic state machine
- Binary operations state machine
- Memory state machine

### 4. Proof System
- STARK-based zero-knowledge proofs
- Constraint system
- Proof generation and verification

## System Flow

1. **Program Loading**
   - RISC-V program loaded into ROM
   - Initial state setup
   - Memory initialization

2. **Execution**
   - Instruction fetch and decode
   - Operation execution
   - State transitions
   - Memory operations

3. **Proof Generation**
   - Execution trace generation
   - Constraint satisfaction
   - STARK proof creation

## Key Features

### 1. Zero-Knowledge
- Private computation
- Public verification
- No information leakage

### 2. RISC-V Compatibility
- Standard instruction set
- Memory model compliance
- System call support

### 3. Performance
- Optimized state machines
- Efficient proof generation
- Memory optimization

## Integration Points

### 1. External Systems
- Blockchain integration
- Smart contract interaction
- Custom applications

### 2. Development Tools
- Compiler toolchain
- Debugging utilities
- Testing framework

## Security Model

### 1. Proof Security
- Soundness guarantees
- Completeness properties
- Zero-knowledge proofs

### 2. System Security
- Memory safety
- State integrity
- Input validation

## High-Level Architecture

ZisK is built like a modular computer system, where each component has a specific job and they all work together through a shared communication channel (the System Bus). Here's how it all fits together:

```
                    ┌─────────────┐
                    │   Program   │
                    │  (RISC-V)   │
                    └──────┬──────┘
                           │
                    ┌──────▼──────┐
                    │    ROM      │
                    └──────┬──────┘
                           │
                    ┌──────▼──────┐
                    │  Processor  │
                    └──────┬──────┘
                           │
                    ┌──────▼──────┐
                    │ System Bus  │
                    └──────┬──────┘
                           │
        ┌─────────┬────────┴────────┬─────────┐
        │         │                 │         │
┌───────▼───┐ ┌───▼───┐       ┌────▼────┐ ┌──▼────┐
│   RAM     │ │ Arith │       │ Binary  │ │Coprocs│
└───────────┘ └───────┘       └─────────┘ └───────┘
```

## How It Works

1. **Program Loading**
   - Your RISC-V program is loaded into the ROM
   - The ROM acts like a read-only memory that holds your program's instructions

2. **Execution Flow**
   - The Processor fetches instructions from the ROM
   - Each instruction is decoded and sent to the appropriate component
   - All communication happens through the System Bus

3. **Component Interaction**
   - RAM: Handles data storage and retrieval
   - Arithmetic Unit: Performs math operations (add, multiply, etc.)
   - Binary Unit: Handles bit operations (AND, OR, XOR, etc.)
   - Coprocessors: Handle specialized operations (hashing, cryptography, etc.)

## Execution-to-Proof Lifecycle

1. **Program Compilation**
   ```
   Rust Code → RISC-V ELF → ROM Image
   ```

2. **Execution**
   ```
   ROM → Processor → System Bus → Components
   ```

3. **Trace Generation**
   ```
   Components → Traces → Constraint Check
   ```

4. **Proof Generation**
   ```
   Traces + Constraints → STARK Proof
   ```

5. **Verification**
   ```
   Proof + Public Inputs → Verification
   ```

## Why This Architecture?

1. **Flexibility**
   - Easy to add new components
   - Can handle different types of programs
   - Supports various proof generation strategies

2. **Performance**
   - Each component is optimized for its task
   - Can process operations in parallel
   - Efficient proof generation

3. **Developer Experience**
   - Clear separation of concerns
   - Easy to understand and debug
   - Well-defined interfaces between components

## Next Steps

- Learn about each [Core Component](./core_components.md) in detail
- Understand the [Constraint System](./constraints.md)
- Explore [Proof Generation](./proof_generation.md)
- See how [Recursion](./recursion.md) works 