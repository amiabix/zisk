# ROM (Read-Only Memory)

The ROM is a critical component of ZisK that stores program instructions and read-only data. It's responsible for providing instruction lookup and program counter management during execution.

## Architecture

### Core Components

1. **Instruction Storage**
   - Stores ZisK instructions
   - Maps program counter to instructions
   - Handles instruction alignment
   - Manages instruction spacing

2. **Read-Only Data Storage**
   - Stores program constants
   - Manages memory-mapped data
   - Handles multiple RO sections
   - Validates data access

3. **Instruction Fetch Unit**
   - Retrieves instructions by PC
   - Handles address alignment
   - Manages instruction lookup
   - Optimizes fetch performance

## Implementation Details

### Key Files

1. **zisk_rom.rs**
   ```rust
   pub struct ZiskRom {
       pub next_init_inst_addr: u64,
       pub insts: HashMap<u64, ZiskInstBuilder>,
       pub ro_data: Vec<RoData>,
       pub rom_entry_instructions: Vec<ZiskInst>,
       pub rom_instructions: Vec<ZiskInst>,
       pub rom_na_instructions: Vec<ZiskInst>,
       pub max_bios_pc: u64,
       pub max_program_pc: u64,
   }
   ```

2. **RoData Structure**
   ```rust
   pub struct RoData {
       pub from: u64,      // Address in program space
       pub length: usize,  // Size of data
       pub data: Vec<u8>,  // Actual data
   }
   ```

### Instruction Storage

1. **Address Space Organization**
   - ROM_ADDR: Base address for program instructions
   - ROM_ENTRY: Entry point address
   - ROM_ADDR_MAX: Maximum ROM address
   - Instruction spacing: 4 units (4096, 4100, 4104...)

2. **Instruction Mapping**
   - RISC-V to ZisK instruction translation
   - Variable instruction count per RISC-V instruction
   - Address alignment handling
   - Jump offset calculation

## Trace Generation

The ROM generates several types of traces:

1. **Instruction Trace**
   - Program counter
   - Instruction word
   - Instruction type
   - Operation details

2. **Data Access Trace**
   - RO data access
   - Memory addresses
   - Data values
   - Access patterns

3. **Program Flow Trace**
   - Instruction sequence
   - Jump targets
   - Control flow
   - Program boundaries

## Performance Optimizations

1. **Instruction Lookup**
   - Optimized instruction containers
   - Aligned instruction vector
   - Non-aligned instruction vector
   - Entry instruction vector

2. **Memory Access**
   - Aligned data access
   - Efficient data storage
   - Memory mapping
   - Access validation

## Debugging and Testing

1. **Debug Features**
   - Instruction inspection
   - Program counter tracking
   - Data access monitoring
   - Memory mapping validation

2. **Testing Tools**
   - Instruction test suite
   - Memory access tests
   - Program flow validation
   - Performance benchmarks

## Best Practices

1. **Program Development**
   - Use aligned instruction addresses
   - Optimize instruction spacing
   - Minimize RO data size
   - Efficient memory mapping

2. **Performance Optimization**
   - Align instructions
   - Optimize instruction lookup
   - Efficient data storage
   - Minimize memory access

## Next Steps

- Learn about [Processor](./processor.md) interaction
- Understand [RAM](./ram.md) operations
- Explore [Arithmetic Unit](./arithmetic.md) usage
- See how [Binary Unit](./binary.md) works
- Discover [Coprocessor](./coprocessors.md) capabilities
- Understand [System Bus](./bus.md) communication 