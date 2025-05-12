# RAM (Random Access Memory)

The RAM is a crucial component of ZisK that manages dynamic data storage and memory operations. It's responsible for handling read/write operations, memory alignment, and data persistence during program execution.

## Architecture

### Core Components

1. **Memory Management**
   - Handles read/write operations
   - Manages memory sections
   - Ensures data persistence
   - Validates memory access

2. **Memory Sections**
   - Read sections for program data
   - Write section for dynamic data
   - System memory for registers
   - Output memory for results

3. **Memory Map**
   - ROM_ENTRY (0x1000): First BIOS instruction
   - ROM_ADDR (0x80000000): First program instruction
   - INPUT_ADDR (0x90000000): Program input data
   - SYS_ADDR (0xa0000000): System memory
   - OUTPUT_ADDR (0xa0010000): Output data
   - AVAILABLE_MEM_ADDR (0xa0020000): General purpose memory

## Implementation Details

### Key Files

1. **mem.rs**
   ```rust
   pub struct Mem {
       pub read_sections: Vec<MemSection>,
       pub write_section: MemSection,
       pub free_input: u64,
   }

   pub struct MemSection {
       pub start: u64,
       pub end: u64,
       pub real_end: u64,
       pub buffer: Vec<u8>,
   }
   ```

2. **Memory Constants**
   ```rust
   pub const INPUT_ADDR: u64 = 0x90000000;
   pub const MAX_INPUT_SIZE: u64 = 0x08000000;  // 128M
   pub const RAM_ADDR: u64 = 0xa0000000;
   pub const RAM_SIZE: u64 = 0x08000000;  // 128M
   pub const SYS_ADDR: u64 = RAM_ADDR;
   pub const SYS_SIZE: u64 = 0x10000;
   pub const OUTPUT_ADDR: u64 = SYS_ADDR + SYS_SIZE;
   pub const OUTPUT_MAX_SIZE: u64 = 0x10000;  // 64K
   ```

### Memory Operations

1. **Read Operations**
   - Aligned reads
   - Non-aligned reads
   - Width-based reads
   - Register reads

2. **Write Operations**
   - Aligned writes
   - Non-aligned writes
   - Width-based writes
   - Register writes

## Trace Generation

The RAM generates several types of traces:

1. **Memory Access Trace**
   - Read operations
   - Write operations
   - Memory addresses
   - Data values

2. **Register Trace**
   - Register reads
   - Register writes
   - Register values
   - Register dependencies

3. **System Trace**
   - System calls
   - Output operations
   - Memory initialization
   - Memory finalization

## Performance Optimizations

1. **Memory Access**
   - Aligned access optimization
   - Section-based management
   - Efficient buffer handling
   - Register caching

2. **Data Management**
   - Efficient section merging
   - Buffer reuse
   - Memory alignment
   - Access validation

## Debugging and Testing

1. **Debug Features**
   - Memory inspection
   - Access monitoring
   - Section tracking
   - Register inspection

2. **Testing Tools**
   - Memory test suite
   - Access pattern tests
   - Alignment tests
   - Performance benchmarks

## Best Practices

1. **Memory Usage**
   - Use aligned addresses
   - Optimize section sizes
   - Minimize memory fragmentation
   - Efficient buffer management

2. **Performance Optimization**
   - Align memory access
   - Optimize section layout
   - Efficient register usage
   - Minimize memory operations

## Next Steps

- Learn about [Processor](./processor.md) interaction
- Understand [ROM](./rom.md) operations
- Explore [Arithmetic Unit](./arithmetic.md) usage
- See how [Binary Unit](./binary.md) works
- Discover [Coprocessor](./coprocessors.md) capabilities
- Understand [System Bus](./bus.md) communication 