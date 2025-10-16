# Local Setup

> **Operators:** For configuration and deployment details, see [Distributed README](../../distributed/README.md)

---

This guide provides comprehensive instructions for setting up and running ZisK's distributed proof generation system in a local development environment. You'll learn how to build, configure, and test the system with practical examples.

## Prerequisites

### System Requirements

The distributed proving system has flexible hardware requirements depending on your use case. For basic development and testing, a minimum of 8GB RAM, 4-core CPU, and 10GB free disk space will suffice. However, for optimal performance and to handle larger proofs, we recommend 16GB or more RAM, 8+ core CPU, and 50GB+ free disk space. When working with large proofs that require significant computational resources, consider 32GB+ RAM, 16+ core CPU, and 100GB+ free disk space. NVIDIA GPU support is available for acceleration, though it requires CUDA to be installed.

On the software side, you'll need Rust 1.70 or higher with Cargo, and Git for repository access. For multi-node testing, an MPI implementation such as OpenMPI or MPICH is helpful but optional. Docker is useful for containerized development, and grpcurl is recommended for API testing, though both are optional for basic usage.

### Environment Setup

```bash
# Install Rust if not already installed
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Clone ZisK repository
git clone https://github.com/0xPolygonHermez/zisk.git
cd zisk

# Create cache directory
mkdir -p $HOME/.zisk/cache

# Install grpcurl for API testing (optional but recommended)
go install github.com/fullstorydev/grpcurl/cmd/grpcurl@latest
```

## Complete Working Example

To demonstrate the distributed proving system in action, let's walk through a complete example using the SHA-256 hashing program from the ZisK quickstart. This example will show you how to set up the system, run a distributed proof generation job, and monitor the results.

### Step 1: Create a Test Program

```bash
# Create a new ZisK project
cargo-zisk sdk new distributed_test
cd distributed_test

# Build the program
cargo-zisk build --release

# Create input file
echo -en "\x14\x00\x00\x00\x00\x00\x00\x00" > build/input.bin  # 20 iterations
```

### Step 2: Build Distributed System

```bash
# From the ZisK project root (not the test project)
cargo build --release --bin zisk-coordinator --bin zisk-worker

# Verify builds
ls -la target/release/zisk-*
```

The build process creates two main binaries: the coordinator service binary and the worker service binary, both located in the `target/release/` directory.

### Step 3: Set Up Development Environment

```bash
# Create development directories
mkdir -p ~/zisk-dev/{elf,inputs,proving-keys,proofs,logs}

# Copy your test program ELF
cp distributed_test/target/riscv64ima-zisk-zkvm-elf/release/distributed_test ~/zisk-dev/elf/

# Copy input file
cp distributed_test/build/input.bin ~/zisk-dev/inputs/

# Copy proving keys (if you have them)
cp -r $HOME/.zisk/provingKey ~/zisk-dev/proving-keys/
```

### Step 4: Start the System

**Terminal 1 - Start Coordinator**:
```bash
cd zisk  # Back to ZisK project root
cargo run --release --bin zisk-coordinator
```

**Terminal 2 - Start Worker**:
```bash
cd zisk  # Back to ZisK project root
cargo run --release --bin zisk-worker -- \
  --elf ~/zisk-dev/elf/distributed_test \
  --inputs-folder ~/zisk-dev/inputs \
  --compute-capacity 4
```

**Terminal 3 - Generate Proof**:
```bash
cd zisk  # Back to ZisK project root
cargo run --release --bin zisk-coordinator prove \
  --input input.bin \
  --compute-capacity 4
```

### Step 5: Monitor the System

```bash
# Check system status
grpcurl -plaintext 127.0.0.1:50051 zisk.distributed.api.v1.ZiskDistributedApi/SystemStatus

# List active jobs
grpcurl -plaintext -d '{"active_only": true}' \
  127.0.0.1:50051 zisk.distributed.api.v1.ZiskDistributedApi/JobsList

# List available workers
grpcurl -plaintext -d '{"available_only": true}' \
  127.0.0.1:50051 zisk.distributed.api.v1.ZiskDistributedApi/WorkersList
```

## Building from Source

### Core Binaries

```bash
# Build distributed proving binaries (from project root)
cargo build --release --bin zisk-coordinator --bin zisk-worker

# Verify builds
ls -la target/release/zisk-*
```

### Feature Flags

```bash
# Build with GPU support
cargo build --release --features gpu --bin zisk-coordinator --bin zisk-worker

# Build with additional debugging symbols
cargo build --bin zisk-coordinator --bin zisk-worker
```

### Build Verification

```bash
# Test coordinator build
./target/release/zisk-coordinator --help

# Test worker build  
./target/release/zisk-worker --help
```

## Configuration Management

The distributed system provides flexible configuration options through TOML files and command-line arguments. This section covers the most common configuration scenarios you'll encounter during development and production deployment.

### Coordinator Configuration

The coordinator can be configured using TOML files or command-line arguments. Here are practical examples for different environments.

#### Development Configuration (`coordinator-dev.toml`)

```toml
[service]
name = "ZisK Distributed Coordinator"
environment = "development"

[server]
host = "127.0.0.1"
port = 50051

[logging]
level = "debug"
format = "pretty"
file_path = "./logs/coordinator.log"

[coordinator]
max_workers_per_job = 10
max_total_workers = 100
phase1_timeout_seconds = 300
phase2_timeout_seconds = 600
```

#### Production Configuration (`coordinator-prod.toml`)

```toml
[service]
name = "ZisK Distributed Coordinator"
environment = "production"

[server]
host = "0.0.0.0"
port = 50051

[logging]
level = "info"
format = "json"
file_path = "/var/log/distributed/coordinator.log"

[coordinator]
max_workers_per_job = 20
max_total_workers = 5000
phase1_timeout_seconds = 600
phase2_timeout_seconds = 1200
webhook_url = "http://webhook.example.com/notify?job_id={$job_id}"
```

### Worker Configuration

Workers have their own configuration options that control how they connect to the coordinator and manage their computational resources.

#### Development Configuration (`worker-dev.toml`)

```toml
[worker]
worker_id = "worker-dev-1"
compute_capacity.compute_units = 4
environment = "development"
inputs_folder = "./inputs"

[coordinator]
url = "http://127.0.0.1:50051"

[connection]
reconnect_interval_seconds = 5
heartbeat_timeout_seconds = 30

[logging]
level = "debug"
format = "pretty"
file_path = "./logs/worker.log"
```

#### Production Configuration (`worker-prod.toml`)

```toml
[worker]
worker_id = "worker-prod-001"
compute_capacity.compute_units = 8
environment = "production"
inputs_folder = "/app/inputs"

[coordinator]
url = "http://coordinator:50051"

[connection]
reconnect_interval_seconds = 5
heartbeat_timeout_seconds = 30

[logging]
level = "info"
format = "json"
file_path = "/var/log/distributed/worker-001.log"
```

### Using Configuration Files

```bash
# Start coordinator with config file
cargo run --release --bin zisk-coordinator -- --config coordinator-dev.toml

# Start worker with config file
cargo run --release --bin zisk-worker -- --config worker-dev.toml \
  --elf ~/zisk-dev/elf/distributed_test \
  --inputs-folder ~/zisk-dev/inputs
```

## Multi-Worker Testing

Testing the distributed system with multiple workers on the same machine is an excellent way to validate the system's behavior before deploying to multiple physical machines. This approach allows you to verify that work is properly distributed, coordination works correctly, and the system handles various failure scenarios.

**Terminal 1 - Coordinator**:
```bash
cargo run --release --bin zisk-coordinator
```

**Terminal 2 - Worker 1**:
```bash
cargo run --release --bin zisk-worker -- \
  --worker-id worker-1 \
  --compute-capacity 4 \
  --elf ~/zisk-dev/elf/distributed_test \
  --inputs-folder ~/zisk-dev/inputs
```

**Terminal 3 - Worker 2**:
```bash
cargo run --release --bin zisk-worker -- \
  --worker-id worker-2 \
  --compute-capacity 4 \
  --elf ~/zisk-dev/elf/distributed_test \
  --inputs-folder ~/zisk-dev/inputs
```

**Terminal 4 - Generate Distributed Proof**:
```bash
cargo run --release --bin zisk-coordinator prove \
  --input input.bin \
  --compute-capacity 8  # Will use both workers
```

### Simulation Mode

For development and testing scenarios where you want to validate distributed behavior without multiple physical machines, the system supports simulation mode. In this mode, a single worker can simulate multiple nodes, allowing you to test the coordination logic and distributed workflows.

```bash
# Start coordinator
cargo run --release --bin zisk-coordinator

# Start single worker with higher capacity
cargo run --release --bin zisk-worker -- \
  --elf ~/zisk-dev/elf/distributed_test \
  --inputs-folder ~/zisk-dev/inputs \
  --compute-capacity 8

# Generate proof in simulation mode
cargo run --release --bin zisk-coordinator prove \
  --input input.bin \
  --compute-capacity 8 \
  --simulated-node 1
```

## API Testing and Monitoring

### Health Checks

```bash
# Basic health check
grpcurl -plaintext 127.0.0.1:50051 zisk.distributed.api.v1.ZiskDistributedApi/HealthCheck

# Service information
grpcurl -plaintext 127.0.0.1:50051 zisk.distributed.api.v1.ZiskDistributedApi/StatusInfo

# System status (workers, capacity, active jobs)
grpcurl -plaintext 127.0.0.1:50051 zisk.distributed.api.v1.ZiskDistributedApi/SystemStatus
```

### Job and Worker Management

```bash
# List active jobs
grpcurl -plaintext -d '{"active_only": true}' \
  127.0.0.1:50051 zisk.distributed.api.v1.ZiskDistributedApi/JobsList

# List available workers
grpcurl -plaintext -d '{"available_only": true}' \
  127.0.0.1:50051 zisk.distributed.api.v1.ZiskDistributedApi/WorkersList

# Get specific job status
grpcurl -plaintext -d '{"job_id": "job_123"}' \
  127.0.0.1:50051 zisk.distributed.api.v1.ZiskDistributedApi/JobStatus

# Launch a new proof job via gRPC
grpcurl -plaintext -d '{
  "block_id": "block_456",
  "compute_capacity": 4,
  "input_path": "input.bin"
}' 127.0.0.1:50051 zisk.distributed.api.v1.ZiskDistributedApi/LaunchProof
```

## Troubleshooting

When working with the distributed proving system, you may encounter various issues related to configuration, networking, resource allocation, or system setup. This section covers the most common problems and their solutions, helping you quickly resolve issues and get back to productive development.

### Worker Connection Issues

One of the most common issues is workers failing to connect to the coordinator. This typically manifests as a "Connection refused" error and can have several causes.

**Error**: `Failed to connect to coordinator: Connection refused`

**Solutions**:
```bash
# Check if coordinator is running
grpcurl -plaintext 127.0.0.1:50051 list

# Verify coordinator host/port configuration
netstat -tlnp | grep 50051

# Check firewall settings
sudo ufw status
```

### Input File Issues

Input file problems are another common source of errors, usually related to file paths, permissions, or missing files.

**Error**: `Input file not found at "./inputs/test.bin"`

**Solutions**:
```bash
# Verify file exists in worker's inputs folder
ls -la ~/zisk-dev/inputs/

# Check file permissions
chmod 644 ~/zisk-dev/inputs/input.bin

# Use filename only (not full path) when launching proofs
cargo run --release --bin zisk-coordinator prove --input input.bin --compute-capacity 4
```

### Capacity Issues

Capacity-related errors occur when the system doesn't have enough computational resources to handle a proof request.

**Error**: `Insufficient capacity: requested 8, available 4`

**Solutions**:
```bash
# Check available workers and capacity
grpcurl -plaintext 127.0.0.1:50051 zisk.distributed.api.v1.ZiskDistributedApi/SystemStatus

# Start additional workers
cargo run --release --bin zisk-worker -- --worker-id worker-2 --compute-capacity 4

# Reduce compute capacity in proof request
cargo run --release --bin zisk-coordinator prove --input input.bin --compute-capacity 4
```

### Configuration Problems

Configuration issues can prevent the system from starting properly or cause unexpected behavior.

**Error**: Configuration file not found or invalid

**Solutions**:
```bash
# Verify TOML syntax
toml-cli validate coordinator-dev.toml

# Check file permissions
ls -la coordinator-dev.toml

# Use CLI overrides to test specific values
cargo run --release --bin zisk-coordinator -- --port 50052
```

### Port Conflicts

Port conflicts occur when another process is already using the port that the coordinator or worker is trying to bind to.

**Error**: `Address already in use`

**Solutions**:
```bash
# Find process using the port
lsof -i :50051

# Kill the process
kill -9 <PID>

# Use different port
cargo run --release --bin zisk-coordinator -- --port 50052
```

### Debug Mode

When standard troubleshooting steps don't resolve the issue, enabling debug mode provides detailed logging that can help identify the root cause of problems.

```bash
# Coordinator with debug logging
RUST_LOG=debug cargo run --release --bin zisk-coordinator

# Worker with debug logging
RUST_LOG=debug cargo run --release --bin zisk-worker -- [options]

# Filter specific modules
RUST_LOG=zisk_distributed_coordinator=debug cargo run --release --bin zisk-coordinator
```

### Performance Profiling

For performance analysis and optimization, you can use standard profiling tools to understand system behavior and identify bottlenecks.

```bash
# Profile coordinator performance
cargo build --release --bin zisk-coordinator
perf record ./target/release/zisk-coordinator
perf report

# Memory usage monitoring
valgrind --tool=massif ./target/release/zisk-coordinator
```

## Development Workflow

### Iterative Development Cycle

1. **Code Changes**: Modify coordinator or worker source code
2. **Rebuild**: `cargo build --release --bin zisk-coordinator --bin zisk-worker`
3. **Restart Services**: Stop and restart coordinator/workers
4. **Test**: Run proof generation to verify changes
5. **Monitor**: Check logs and metrics for issues

### Hot Reloading Development

```bash
# Use cargo watch for automatic rebuilds
cargo install cargo-watch

# Terminal 1: Auto-rebuild and restart coordinator
cargo watch -x "run --release --bin zisk-coordinator"

# Terminal 2: Auto-rebuild and restart worker
cargo watch -x "run --release --bin zisk-worker -- --elf ~/zisk-dev/elf/distributed_test --inputs-folder ~/zisk-dev/inputs"
```

### Integration Testing

```bash
# Run unit tests
cargo test

# Run integration tests for distributed components
cargo test --package zisk-distributed-coordinator
cargo test --package zisk-distributed-worker

# Run end-to-end tests
cargo test --test integration_tests
```

### Custom Test Scripts

Create test scripts for common development scenarios:

```bash
#!/bin/bash
# test-distributed.sh

set -e

echo "Starting coordinator..."
cargo run --release --bin zisk-coordinator &
COORDINATOR_PID=$!

sleep 2

echo "Starting worker..."
cargo run --release --bin zisk-worker -- \
  --elf ~/zisk-dev/elf/distributed_test \
  --inputs-folder ~/zisk-dev/inputs &
WORKER_PID=$!

sleep 2

echo "Generating test proof..."
cargo run --release --bin zisk-coordinator prove \
  --input input.bin \
  --compute-capacity 4

echo "Cleaning up..."
kill $COORDINATOR_PID $WORKER_PID
```

This comprehensive local development guide provides everything you need to build, test, and debug ZisK's distributed proving system in a local environment.

## See Also

- [Overview & Architecture](./introduction.md) for system concepts and architecture
- [Docker Deployment](./docker_deployment.md) for production deployment
- [Distributed README](../../distributed/README.md) for operator configuration and troubleshooting

## Running the System Locally

### Terminal-Based Development

**Terminal 1: Start Coordinator**
```bash
# Run with default configuration
cargo run --release --bin zisk-coordinator

# Or with custom config
cargo run --release --bin zisk-coordinator -- --config config/coordinator/dev.toml

# Enable debug logging
RUST_LOG=debug cargo run --release --bin zisk-coordinator
```

**Terminal 2: Start Worker**
```bash
# Run worker with required parameters
cargo run --release --bin zisk-worker -- \
  --elf ./zisk-dev/elf/your_program.elf \
  --proving-key ./zisk-dev/proving-keys \
  --inputs-folder ./zisk-dev/inputs

# With custom configuration
cargo run --release --bin zisk-worker -- \
  --config config/worker/dev.toml \
  --elf ./zisk-dev/elf/your_program.elf \
  --proving-key ./zisk-dev/proving-keys \
  --inputs-folder ./zisk-dev/inputs
```

**Terminal 3: Generate Proof**
```bash
# Generate a proof using the coordinator's CLI
cargo run --release --bin zisk-coordinator prove \
  --input input1.json \
  --compute-capacity 4

# With custom coordinator URL
cargo run --release --bin zisk-coordinator prove \
  --coordinator-url http://127.0.0.1:50051 \
  --input input1.json \
  --compute-capacity 4
```

### Advanced Configuration Options

#### Coordinator CLI Options

```bash
cargo run --release --bin zisk-coordinator -- --help
```

**Key Options**:
- `--config <path>`: Path to TOML configuration file
- `--host <ip>`: Host address to bind to (default: 127.0.0.1)
- `--port <port>`: Port to listen on (default: 50051)
- `--webhook-url <url>`: URL for job completion webhooks

#### Worker CLI Options

```bash
cargo run --release --bin zisk-worker -- --help
```

**Key Options**:
- `--config <path>`: Path to TOML configuration file
- `--coordinator-url <url>`: Coordinator gRPC endpoint
- `--elf <path>`: Path to ELF file for execution
- `--proving-key <path>`: Directory containing proving keys
- `--inputs-folder <path>`: Directory containing input files
- `--compute-capacity <units>`: Worker compute capacity (default: 4)
- `--worker-id <id>`: Unique worker identifier

## Testing and Emulation

### Simulation Mode

Test the distributed system with a single worker simulating multiple nodes:

```bash
# Start coordinator
cargo run --release --bin zisk-coordinator

# Start single worker
cargo run --release --bin zisk-worker -- \
  --elf ./zisk-dev/elf/program.elf \
  --proving-key ./zisk-dev/proving-keys \
  --inputs-folder ./zisk-dev/inputs \
  --compute-capacity 8

# Generate proof in simulation mode (simulating 2 nodes)
cargo run --release --bin zisk-coordinator prove \
  --input test_input.json \
  --compute-capacity 8 \
  --simulated-node 1
```

### Multi-Worker Local Testing

Run multiple workers on the same machine for testing:

**Terminal 2: Worker 1**
```bash
cargo run --release --bin zisk-worker -- \
  --worker-id worker-1 \
  --compute-capacity 4 \
  --elf ./zisk-dev/elf/program.elf \
  --proving-key ./zisk-dev/proving-keys \
  --inputs-folder ./zisk-dev/inputs
```

**Terminal 3: Worker 2**
```bash
cargo run --release --bin zisk-worker -- \
  --worker-id worker-2 \
  --compute-capacity 4 \
  --elf ./zisk-dev/elf/program.elf \
  --proving-key ./zisk-dev/proving-keys \
  --inputs-folder ./zisk-dev/inputs
```

**Terminal 4: Generate Distributed Proof**
```bash
cargo run --release --bin zisk-coordinator prove \
  --input test_input.json \
  --compute-capacity 8  # Will use both workers
```

## API Testing with grpcurl

### Installation

```bash
# Install grpcurl for API testing
go install github.com/fullstorydev/grpcurl/cmd/grpcurl@latest
```

### Health Checks and Monitoring

```bash
# Basic health check
grpcurl -plaintext 127.0.0.1:50051 zisk.distributed.api.v1.ZiskDistributedApi/HealthCheck

# Service information
grpcurl -plaintext 127.0.0.1:50051 zisk.distributed.api.v1.ZiskDistributedApi/StatusInfo

# System status (workers, capacity, active jobs)
grpcurl -plaintext 127.0.0.1:50051 zisk.distributed.api.v1.ZiskDistributedApi/SystemStatus
```

### Job and Worker Management

```bash
# List active jobs
grpcurl -plaintext -d '{"active_only": true}' \
  127.0.0.1:50051 zisk.distributed.api.v1.ZiskDistributedApi/JobsList

# List available workers
grpcurl -plaintext -d '{"available_only": true}' \
  127.0.0.1:50051 zisk.distributed.api.v1.ZiskDistributedApi/WorkersList

# Get specific job status
grpcurl -plaintext -d '{"job_id": "job_123"}' \
  127.0.0.1:50051 zisk.distributed.api.v1.ZiskDistributedApi/JobStatus

# Launch a new proof job via gRPC
grpcurl -plaintext -d '{"block_id": "block_456", "compute_capacity": 4, "input_path": "test_input.json"}' \
  127.0.0.1:50051 zisk.distributed.api.v1.ZiskDistributedApi/LaunchProof
```

## Development Workflow

### Iterative Development Cycle

1. **Code Changes**: Modify coordinator or worker source code
2. **Rebuild**: `cargo build --release --bin zisk-coordinator --bin zisk-worker`
3. **Restart Services**: Stop and restart coordinator/workers
4. **Test**: Run proof generation to verify changes
5. **Monitor**: Check logs and metrics for issues

### Hot Reloading Development

```bash
# Use cargo watch for automatic rebuilds
cargo install cargo-watch

# Terminal 1: Auto-rebuild and restart coordinator
cargo watch -x "run --release --bin zisk-coordinator"

# Terminal 2: Auto-rebuild and restart worker  
cargo watch -x "run --release --bin zisk-worker -- --elf ./zisk-dev/elf/program.elf --proving-key ./zisk-dev/proving-keys --inputs-folder ./zisk-dev/inputs"
```

## Debugging and Troubleshooting

### Logging Configuration

#### Enable Debug Logging

```bash
# Environment variable approach
RUST_LOG=debug cargo run --release --bin zisk-coordinator
RUST_LOG=debug cargo run --release --bin zisk-worker -- [options]

# Configuration file approach (dev.toml)
[logging]
level = "debug"
format = "pretty"
file_path = "./logs/debug.log"
```

#### Structured Logging

```bash
# JSON formatted logs for analysis
RUST_LOG=info cargo run --release --bin zisk-coordinator -- --log-format json

# Filter specific modules
RUST_LOG=zisk_distributed_coordinator=debug cargo run --release --bin zisk-coordinator
```

### Common Development Issues

#### Worker Connection Issues

**Problem**: Worker can't connect to coordinator
```
ERROR Failed to connect to coordinator: Connection refused
```

**Solutions**:
- Verify coordinator is running: `grpcurl -plaintext 127.0.0.1:50051 list`
- Check coordinator host/port configuration
- Ensure firewall allows connections on port 50051

#### Input File Issues

**Problem**: Input file not found errors
```
ERROR Input file not found at "./inputs/test.json"
```

**Solutions**:
- Verify file exists in worker's `--inputs-folder` directory
- Check file permissions (worker needs read access)
- Use filename only (not full path) when launching proofs

#### Capacity Issues

**Problem**: Insufficient capacity for proof generation
```
ERROR Insufficient capacity: requested 8, available 4
```

**Solutions**:
- Start additional workers to increase total capacity
- Reduce `--compute-capacity` in proof request
- Check worker capacity with system status API

### Performance Profiling

```bash
# Profile coordinator performance
cargo build --release --bin zisk-coordinator
perf record ./target/release/zisk-coordinator
perf report

# Memory usage monitoring  
valgrind --tool=massif ./target/release/zisk-coordinator
```

## Integration Testing

### Automated Test Suite

```bash
# Run unit tests
cargo test

# Run integration tests for distributed components
cargo test --package zisk-distributed-coordinator
cargo test --package zisk-distributed-worker

# Run end-to-end tests
cargo test --test integration_tests
```

### Custom Test Scripts

Create test scripts for common development scenarios:

```bash
#!/bin/bash
# test-distributed.sh

set -e

echo "Starting coordinator..."
cargo run --release --bin zisk-coordinator &
COORDINATOR_PID=$!

sleep 2

echo "Starting worker..."
cargo run --release --bin zisk-worker -- \
  --elf ./test-data/test.elf \
  --proving-key ./test-data/proving-keys \
  --inputs-folder ./test-data/inputs &
WORKER_PID=$!

sleep 2

echo "Generating test proof..."
cargo run --release --bin zisk-coordinator prove \
  --input test.json \
  --compute-capacity 4

echo "Cleaning up..."
kill $COORDINATOR_PID $WORKER_PID
```

This development guide provides a comprehensive foundation for building, testing, and debugging ZisK's distributed proving system in a local environment.
