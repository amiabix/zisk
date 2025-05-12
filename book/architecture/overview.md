# System Architecture Overview

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

## Key Features

### 1. Modular Design
- Each component is independent and focused on one task
- Makes the system easier to understand and debug
- Allows for better performance optimization

### 2. Trace-Based Execution
- Every operation generates a trace
- Traces are used to prove correct execution
- Makes it easy to verify what happened

### 3. Bus Communication
- All components talk through the System Bus
- Makes it easy to add new components
- Ensures everything is properly connected

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