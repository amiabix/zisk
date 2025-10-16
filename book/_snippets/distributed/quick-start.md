## Quick Start

### 1. Build the System

```bash
# From the ZisK project root
cargo build --release --bin zisk-coordinator --bin zisk-worker
```

### 2. Start Coordinator

```bash
# Terminal 1: Start coordinator
cargo run --release --bin zisk-coordinator
```

### 3. Start Worker

```bash
# Terminal 2: Start worker (replace paths with your actual files)
cargo run --release --bin zisk-worker -- \
  --elf /path/to/your/program.elf \
  --inputs-folder /path/to/inputs \
  --compute-capacity 4
```

### 4. Generate Proof

```bash
# Terminal 3: Generate a proof
cargo run --release --bin zisk-coordinator prove \
  --input your_input.bin \
  --compute-capacity 4
```

### 5. Monitor System

```bash
# Check system status
grpcurl -plaintext 127.0.0.1:50051 zisk.distributed.api.v1.ZiskDistributedApi/SystemStatus

# List active jobs
grpcurl -plaintext -d '{"active_only": true}' \
  127.0.0.1:50051 zisk.distributed.api.v1.ZiskDistributedApi/JobsList
```
