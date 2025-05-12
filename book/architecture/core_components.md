# Core Components

ZisK's architecture is built around several key components that work together to execute and prove programs. Each component has a specific role and generates its own execution trace.

## Component Overview

### 1. Processor
The heart of the system, responsible for:
- Fetching and decoding RISC-V instructions
- Managing program execution flow
- Coordinating between different components
- Handling register operations

Key files:
- `zisk_inst.rs`: Instruction definitions and handling
- `zisk_registers.rs`: Register management
- `zisk_ops.rs`: Operation implementations

### 2. ROM (Read-Only Memory)
Stores the program instructions and provides:
- Static program storage
- Instruction lookup
- Program counter management
- Instruction decoding support

Key files:
- `zisk_rom.rs`: ROM implementation and management
- `elf2rom.rs`: ELF binary to ROM conversion

### 3. RAM (Random Access Memory)
Handles dynamic data storage:
- Read/write operations
- Memory alignment
- Data persistence
- Memory access validation

Key files:
- `mem.rs`: Memory operations and management

### 4. Arithmetic Unit
Performs mathematical operations:
- Addition
- Subtraction
- Multiplication
- Field arithmetic over Goldilocks

### 5. Binary Unit
Handles bit-level operations:
- AND, OR, XOR
- Bit shifts
- Bit manipulation
- Boolean logic

### 6. Coprocessors
Specialized components for:
- Cryptographic operations
- Complex computations
- Domain-specific tasks
- Custom operations

### 7. System Bus
The communication backbone:
- Message routing
- Component coordination
- Operation dispatch
- Trace synchronization

## Component Interaction

```
                    ┌─────────────┐
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

## Trace Generation

Each component generates its own execution trace:

1. **Processor Trace**
   - Instruction execution
   - Program counter changes
   - Register updates
   - Operation dispatch

2. **ROM Trace**
   - Instruction storage
   - Program counter validation
   - Instruction decoding

3. **RAM Trace**
   - Memory operations
   - Data storage
   - Access patterns

4. **Arithmetic Trace**
   - Mathematical operations
   - Field arithmetic
   - Result validation

5. **Binary Trace**
   - Bit operations
   - Boolean logic
   - Shift operations

6. **Coprocessor Trace**
   - Specialized operations
   - Complex computations
   - Custom operations

## Component Details

### Processor
- Handles RISC-V instruction set
- Manages program execution
- Coordinates component interaction
- Generates execution traces

### ROM
- Stores program instructions
- Provides instruction lookup
- Validates program counter
- Supports instruction decoding

### RAM
- Manages dynamic memory
- Handles read/write operations
- Ensures memory consistency
- Validates memory access

### Arithmetic Unit
- Performs field arithmetic
- Handles mathematical operations
- Validates computation results
- Generates arithmetic traces

### Binary Unit
- Executes bit operations
- Handles boolean logic
- Manages bit manipulation
- Generates binary traces

### Coprocessors
- Implements specialized operations
- Handles complex computations
- Supports custom operations
- Generates coprocessor traces

### System Bus
- Routes messages between components
- Coordinates operation dispatch
- Ensures trace synchronization
- Manages component communication

## Next Steps

- Learn about the [Processor](./processor.md) in detail
- Understand the [ROM](./rom.md) implementation
- Explore [RAM](./ram.md) operations
- See how the [Arithmetic Unit](./arithmetic.md) works
- Discover the [Binary Unit](./binary.md) capabilities
- Learn about [Coprocessors](./coprocessors.md)
- Understand the [System Bus](./bus.md) communication 