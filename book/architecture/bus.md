# System Bus

The System Bus is the communication backbone of ZisK, facilitating data transfer and coordination between different components. It implements a publish-subscribe pattern where components can publish data to specific bus IDs and subscribe to receive data from those IDs.

## Architecture

### Core Components

1. **Data Bus**
   - Manages device connections
   - Routes messages between components
   - Handles data transfers
   - Maintains device mappings

2. **Bus Devices**
   - Subscribe to specific bus IDs
   - Process incoming data
   - Generate derived inputs
   - Track operation metrics

3. **Bus IDs**
   - Unique identifiers for different buses
   - ROM Bus (ID: 1)
   - Operation Bus (ID: 0)
   - Memory Bus (ID: 2)

## Implementation Details

### Key Files

1. **data_bus.rs**
   ```rust
   pub struct DataBus<D, BD: BusDevice<D>> {
       pub devices: Vec<Box<BD>>,
       devices_bus_id_map: Vec<Vec<usize>>,
       pending_transfers: VecDeque<(BusId, Vec<D>)>,
   }
   ```

2. **data_bus_operation.rs**
   ```rust
   pub struct OperationBusData<D>(std::marker::PhantomData<D>);
   ```

3. **data_bus_rom.rs**
   ```rust
   pub struct RomBusData<D>(std::marker::PhantomData<D>);
   ```

### Bus Device Interface

```rust
pub trait BusDevice<D>: Any + Send {
    fn process_data(&mut self, bus_id: &BusId, data: &[D]) -> Option<Vec<(BusId, Vec<D>)>>;
    fn bus_id(&self) -> Vec<BusId>;
    fn as_any(self: Box<Self>) -> Box<dyn Any>;
}
```

## Message Types

1. **Operation Messages**
   - Operation code
   - Operation type
   - Input parameters
   - Result data

2. **ROM Messages**
   - Program counter
   - Instruction data
   - Step information
   - End flags

3. **Memory Messages**
   - Memory address
   - Data payload
   - Operation type
   - Size information

## Trace Generation

The System Bus generates several types of traces:

1. **Operation Traces**
   - Operation execution
   - Parameter passing
   - Result collection
   - Error handling

2. **ROM Traces**
   - Instruction fetching
   - Program flow
   - Step tracking
   - End detection

3. **Memory Traces**
   - Read operations
   - Write operations
   - Address tracking
   - Data validation

## Performance Optimizations

1. **Message Routing**
   - Efficient device mapping
   - Direct message delivery
   - Batch processing
   - Priority handling

2. **Resource Management**
   - Device pooling
   - Memory reuse
   - Connection optimization
   - Queue management

## Debugging and Testing

1. **Debug Features**
   - Message inspection
   - Device monitoring
   - Route tracing
   - Error detection

2. **Testing Tools**
   - Message validation
   - Device testing
   - Performance benchmarks
   - Integration tests

## Best Practices

1. **Message Handling**
   - Validate message format
   - Handle errors gracefully
   - Process messages efficiently
   - Clean up resources

2. **Device Management**
   - Register devices properly
   - Handle device lifecycle
   - Monitor device health
   - Optimize connections

## Next Steps

- Learn about [Processor](./processor.md) interaction
- Understand [ROM](./rom.md) operations
- Explore [RAM](./ram.md) usage
- See how [Arithmetic Unit](./arithmetic.md) works
- Discover [Binary Unit](./binary.md) capabilities
- Understand [Coprocessors](./coprocessors.md) communication 